//! Panneau « Pile d'effets » (13i) du calque courant : ajouter, activer, ordonner, retirer des
//! effets ; régler leurs paramètres ; créer et supprimer des clés (TVPaint 000025314–316), avec
//! la création automatique des clés. Avec des clés, changer un paramètre pose (ou met à jour) la
//! clé de l'image courante ; sans clé, il change l'effet sur tout le calque.
//!
//! Les clés de l'effet choisi (clic sur son nom, cadre de la couleur de sélection) se voient aussi
//! dans la timeline et le montage : des losanges en haut de la piste de son calque (`fxkeys.rs`).

use eframe::egui::{self, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::fx::{Effect, EffectKind, FAMILIES};

/// Pourquoi le tremblement du trait est grisé sur un calque bitmap (bulle des menus).
pub(crate) const BOIL_VECTOR_ONLY: &str =
    "Le tremblement du trait ne va que sur un calque vectoriel : il déplace un peu les points de chaque trait, d'une variante à l'autre. Choisis un calque vectoriel, ou crée-en un (Calques > Nouveau calque > Vectoriel).";

/// Pourquoi un effet du trait (`fxtrait.rs`) est grisé sur un calque bitmap (bulle des menus).
pub(crate) const TRAIT_VECTOR_ONLY: &str =
    "Cet effet agit sur les traits d'un dessin vectoriel (leur ordre, leur longueur, leur épaisseur) : il ne va que sur un calque vectoriel. Choisis un calque vectoriel, ou crée-en un (Calques > Nouveau calque > Vectoriel).";

/// Ce qu'un menu montre d'un effet pour un calque : rien (un dossier n'a pas d'effets), l'entrée
/// grisée avec la raison (Anthony, 2026-10-19 : « je ne trouve pas l'effet de tremblement » — il
/// ne se cachait que sur un calque bitmap), ou l'entrée utilisable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Offer {
    Absent,
    Disabled(&'static str),
    Enabled,
}

/// L'offre de l'effet `kind` pour un calque de la sorte `layer` (`folder` : c'est un dossier).
pub(crate) fn offer(kind: EffectKind, layer: crate::document::LayerKind, folder: bool) -> Offer {
    if folder {
        Offer::Absent
    } else if kind.vector_only() && layer != crate::document::LayerKind::Vector {
        Offer::Disabled(if kind.is_trait() { TRAIT_VECTOR_ONLY } else { BOIL_VECTOR_ONLY })
    } else {
        Offer::Enabled
    }
}

impl App {
    /// Ce que les menus montrent de l'effet `kind` pour le calque `li` (`offer`).
    pub(crate) fn fx_offer(&self, li: usize, kind: EffectKind) -> Offer {
        self.layers().get(li).map_or(Offer::Absent, |l| offer(kind, l.kind, l.folder))
    }

    /// L'effet `kind` est-il là et actif sur le calque `li` ?
    pub(crate) fn fx_on(&self, li: usize, kind: EffectKind) -> bool {
        self.layers().get(li).is_some_and(|l| l.fx.iter().any(|e| e.kind == kind && e.enabled))
    }

    /// L'effet `kind` peut-il aller sur le calque `li` ? (Le tremblement du trait : calques
    /// vectoriels seulement.)
    pub(crate) fn fx_allowed(&self, li: usize, kind: EffectKind) -> bool {
        self.fx_offer(li, kind) == Offer::Enabled
    }

    /// Un geste d'édition est en cours sur le calque courant : dessin, gomme, sélection,
    /// transformation, forme, texte. Son tremblement se tait le temps du geste (`build_plan`) : on
    /// dessine, on prend et on déplace ce qu'on voit, à la vraie place des traits.
    pub(crate) fn boil_editing(&self) -> bool {
        self.stroke.is_some()
            || self.vstroke.is_some()
            || self.verase.is_some()
            || self.vsel.as_ref().is_some_and(|s| !s.items.is_empty())
            || self.vdrag.is_some()
            || self.flottante.is_some()
            || self.multi.sel.is_some()
            || !self.multi.floats.is_empty()
            || self.cut_gesture.is_some()
            || self.envelope.is_some()
            || self.persp.is_some()
            || self.shape_drag.is_some()
            || self.shape_edit.is_some()
            || self.text_edit.is_some()
    }

    /// Règle « Animer à l'arrêt » du tremblement du calque `li` (il s'ajoute, réglages par défaut,
    /// s'il n'y est pas encore) : une étape d'annulation. Le réglage vaut aussi pour les clés.
    pub(crate) fn set_boil_rest(&mut self, li: usize, on: bool) {
        if !self.fx_allowed(li, EffectKind::Boil) || self.content_refuses() {
            return;
        }
        self.record_step();
        let fx = &mut self.layers_mut()[li].fx;
        let i = match fx.iter().position(|e| e.kind == EffectKind::Boil) {
            Some(i) => i,
            None => {
                fx.push(Effect::new(EffectKind::Boil));
                fx.len() - 1
            }
        };
        fx[i].enabled = true;
        fx[i].set_param(crate::tremblement::REST, if on { 1.0 } else { 0.0 });
        self.renderer.invalidate_all();
        self.status = format!("Tremblement du trait : {}.", if on { "le dessin bout aussi à l'arrêt" } else { "image fixe à l'arrêt" });
    }

    /// « Animer à l'arrêt » est-il réglé sur le tremblement du calque `li` ?
    pub(crate) fn boil_rest_on(&self, li: usize) -> bool {
        self.layers().get(li).is_some_and(|l| l.fx.iter().any(|e| e.kind == EffectKind::Boil && e.enabled && e.values_at(self.cur_frame).get(crate::tremblement::REST).is_some_and(|v| *v >= 0.5)))
    }

    /// Coche ou décoche l'effet `kind` du calque `li` (une étape d'annulation, le dessin ne
    /// change pas) : absent, il s'ajoute avec ses réglages par défaut ; présent, il s'active ou
    /// se désactive, ses réglages restent.
    pub(crate) fn toggle_fx(&mut self, li: usize, kind: EffectKind) {
        if li >= self.layers().len() || self.content_refuses() {
            return;
        }
        if !self.fx_allowed(li, kind) {
            self.status = format!("« {} » ne va que sur un calque vectoriel.", kind.label());
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
            if li == self.cur_layer {
                self.select_fx(i);
            }
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
                let kinds: Vec<EffectKind> = EffectKind::ALL.into_iter().filter(|k| k.family() == family).collect();
                for kind in kinds {
                    match self.fx_offer(li, kind) {
                        Offer::Absent => {}
                        // Grisé avec la raison plutôt que caché : on voit qu'il existe.
                        Offer::Disabled(why) => {
                            ui.add_enabled(false, egui::Checkbox::new(&mut false, kind.label())).on_disabled_hover_text(why);
                        }
                        Offer::Enabled => {
                            let on = self.fx_on(li, kind);
                            let resp = ui.checkbox(&mut on.clone(), kind.label());
                            let resp = if kind.hint().is_empty() { resp } else { resp.on_hover_text(kind.hint()) };
                            if resp.clicked() {
                                self.toggle_fx(li, kind);
                            }
                        }
                    }
                }
            });
        }
        // Copier-coller des effets d'un calque à un autre (`fxcopie.rs`).
        ui.separator();
        self.fx_clip_menu(ui, li, None, false);
    }

    /// Le sous-menu « Effets » d'un calque, puis son tremblement du trait à découvert : les menus
    /// d'un calque (panneau Calques, montage, timeline) ont tous ces deux entrées.
    pub(crate) fn layer_fx_items(&mut self, ui: &mut egui::Ui, li: usize) {
        if self.layers().get(li).is_none_or(|l| l.folder) {
            return;
        }
        ui.menu_button("Effets", |ui| self.fx_menu_items(ui, li));
        self.boil_menu_items(ui, li);
    }

    /// « Tremblement du trait » et « Animer à l'arrêt » (`tremblement.rs`) dans un menu de calque.
    /// Sur un calque bitmap, l'entrée est grisée et dit pourquoi ; sur un dossier, absente.
    pub(crate) fn boil_menu_items(&mut self, ui: &mut egui::Ui, li: usize) {
        let kind = EffectKind::Boil;
        let offer = self.fx_offer(li, kind);
        if offer == Offer::Absent {
            return;
        }
        let boil = self.fx_on(li, kind);
        let key = self.raccourci("fx.tremblement");
        let resp = ui.add_enabled(offer == Offer::Enabled, egui::Checkbox::new(&mut boil.clone(), format!("{}  Tremblement du trait", ph::WAVE_SINE)));
        let resp = match offer {
            Offer::Disabled(why) => resp.on_disabled_hover_text(why),
            _ => resp.on_hover_text(format!(
                "Le trait « bout » comme dans WigglyPaint : le dessin est montré en quelques variantes légèrement déplacées, qui se suivent. Les dessins ne changent pas ; intensité, fréquence, variantes et vitesse se règlent dans le panneau Effets. {key}"
            )),
        };
        if offer == Offer::Enabled && resp.clicked() {
            ui.close_menu();
            self.toggle_fx(li, kind);
        }
        if boil && offer == Offer::Enabled {
            let rest = self.boil_rest_on(li);
            if ui
                .checkbox(&mut rest.clone(), "Animer à l'arrêt")
                .on_hover_text("Le dessin continue de trembler quand on ne lit pas (l'écran se redessine alors à la cadence du tremblement). Éteint : l'image fixe montre la variante de l'image courante.")
                .clicked()
            {
                ui.close_menu();
                self.set_boil_rest(li, !rest);
            }
        }
    }

    /// Le tremblement du trait dans « Ajouter un effet » : en tête du menu, avant les familles. Déjà
    /// sur le calque, il est choisi dans la pile (actif) au lieu d'être ajouté une deuxième fois.
    fn add_boil_entry(&mut self, ui: &mut egui::Ui, li: usize) {
        let kind = EffectKind::Boil;
        let offer = self.fx_offer(li, kind);
        if offer == Offer::Absent {
            return;
        }
        let resp = ui.add_enabled(offer == Offer::Enabled, egui::Button::new(format!("{}  {}", ph::WAVE_SINE, kind.label())));
        let resp = match offer {
            Offer::Disabled(why) => resp.on_disabled_hover_text(why),
            _ => resp.on_hover_text("Le trait « bout » comme dans WigglyPaint (calques vectoriels). Les dessins ne changent pas."),
        };
        if offer == Offer::Enabled && resp.clicked() {
            ui.close_menu();
            if let Some(i) = self.layers()[li].fx.iter().position(|e| e.kind == kind) {
                if !self.layers()[li].fx[i].enabled {
                    self.toggle_fx(li, kind);
                }
                self.select_fx(i);
            } else {
                self.record_step();
                self.layers_mut()[li].fx.push(Effect::new(kind));
                self.select_fx(self.layers()[li].fx.len() - 1);
            }
        }
        ui.separator();
    }

    /// Le réglage « tremblement du trait » des outils de dessin vectoriels (Réglages du pinceau et
    /// du crayon) : un interrupteur pour le calque courant, comme l'action `fx.tremblement`.
    pub(crate) fn boil_tool_toggle(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        if self.fx_offer(li, EffectKind::Boil) != Offer::Enabled {
            return;
        }
        let on = self.fx_on(li, EffectKind::Boil);
        ui.horizontal(|ui| {
            if self
                .bouton_etat(ui, on, ph::WAVE_SINE, "Tremblement du trait", "Le trait de ce calque vectoriel « bout » comme dans WigglyPaint (variantes légèrement déplacées). Les dessins ne changent pas ; les réglages sont dans le panneau Effets.", "fx.tremblement")
                .clicked()
            {
                self.run_action("fx.tremblement");
            }
            ui.label(RichText::new(if on { "Le trait de ce calque bout." } else { "Le trait de ce calque est fixe." }).small().weak());
        });
    }

    /// Ajoute l'effet `kind` à la pile du calque `li`, ou, s'il y est déjà (les effets réservés au
    /// vectoriel ne sont qu'une fois par calque), l'active et le choisit (une étape d'annulation).
    pub(crate) fn add_effect_once(&mut self, li: usize, kind: EffectKind) {
        if self.content_refuses() {
            return;
        }
        self.record_step();
        let fx = &mut self.layers_mut()[li].fx;
        let i = match fx.iter().position(|e| e.kind == kind).filter(|_| kind.vector_only()) {
            Some(i) => {
                fx[i].enabled = true;
                i
            }
            None => {
                fx.push(Effect::new(kind));
                fx.len() - 1
            }
        };
        self.select_fx(i);
        self.renderer.invalidate_all();
    }

    /// La rangée d'icônes des effets du trait (`fxtrait.rs`) : un bouton par effet, enfoncé quand il
    /// est sur le calque et actif, grisé avec la raison sur un calque bitmap ; puis les préréglages.
    pub(crate) fn trait_strip(&mut self, ui: &mut egui::Ui, li: usize) {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Trait").weak());
            for kind in crate::fxtrait::KINDS {
                let offer = self.fx_offer(li, kind);
                let enabled = offer == Offer::Enabled;
                let on = self.fx_on(li, kind);
                let action = crate::fxtrait::action(kind);
                let key = self.raccourci(action);
                let about = match offer {
                    Offer::Disabled(why) => why,
                    _ => kind.hint(),
                };
                let resp = crate::bulles::etat_si(ui, enabled, on, crate::fxtrait::icon(kind), kind.label(), about, &key);
                crate::raccourcisui::shortcut_menu(&resp, action, &key);
                if enabled && resp.clicked() {
                    self.run_action(action);
                }
            }
            let usable = self.fx_offer(li, EffectKind::Write) == Offer::Enabled;
            let menu = ui.add_enabled_ui(usable, |ui| {
                ui.menu_button(RichText::new(ph::MAGIC_WAND).size(crate::bulles::TAILLE), |ui| {
                    for preset in crate::fxtrait::Preset::ALL {
                        let key = self.raccourci(preset.action());
                        let resp = crate::bulles::bulle(ui.button(preset.label()), preset.label(), preset.note(), &key);
                        if resp.clicked() {
                            ui.close_menu();
                            self.apply_trait_preset(preset);
                        }
                    }
                })
                .response
            });
            crate::bulles::bulle(menu.inner, "Préréglages du trait", "Des effets du trait tout réglés, avec leurs clés à partir de la tête de lecture : apparition écrite, croquis nerveux, poussière.", "");
        });
    }

    /// Pose les effets du préréglage `preset` sur le calque courant (une étape d'annulation) : ceux de
    /// même sorte sont remplacés, les clés partent de l'image courante.
    pub(crate) fn apply_trait_preset(&mut self, preset: crate::fxtrait::Preset) {
        let li = self.cur_layer;
        if self.fx_offer(li, EffectKind::Write) != Offer::Enabled {
            self.status = format!("« {} » ne va que sur un calque vectoriel.", preset.label());
            return;
        }
        if self.content_refuses() {
            return;
        }
        let built = preset.build(self.doc.fps, self.cur_frame, self.panel().frame_count());
        self.record_step();
        let stack = &mut self.layers_mut()[li].fx;
        for e in built {
            stack.retain(|x| x.kind != e.kind);
            stack.push(e);
        }
        let last = stack.len() - 1;
        self.select_fx(last);
        self.renderer.invalidate_all();
        self.status = format!("{} : posé sur le calque, les clés partent de l'image {}.", preset.label(), self.frame_label(self.cur_frame));
    }

    /// Pose ou met à jour la clé de l'effet `i` du calque courant à l'image courante, avec ses
    /// paramètres à cette image (une étape d'annulation).
    pub(crate) fn fx_create_key(&mut self) {
        let (li, f) = (self.cur_layer, self.cur_frame);
        let Some(i) = self.fx_index() else {
            self.status = "Choisis d'abord un effet dans la pile du calque (clic sur son nom, dans le panneau Effets).".into();
            return;
        };
        let Some((values, name)) = self.layers().get(li).and_then(|l| l.fx.get(i)).map(|e| (e.values_at(f), e.kind.label())) else {
            self.status = "Choisis d'abord un effet dans la pile du calque.".into();
            return;
        };
        if self.content_refuses() {
            return;
        }
        self.record_step();
        self.layers_mut()[li].fx[i].set_key(f, values);
        // La clé qu'on vient de créer, seule, devient le choix (elle se voit, se déplace, se copie
        // et se supprime dans le montage et la timeline).
        self.select_new_fx_key(li, i, f);
        self.status = format!("Clé de « {name} » à l'image {}.", self.frame_label(f));
    }

    /// Retire la clé de l'effet choisi à l'image courante.
    pub(crate) fn fx_delete_key(&mut self) {
        let (li, f) = (self.cur_layer, self.cur_frame);
        let Some(i) = self.fx_index() else {
            self.status = "Choisis d'abord un effet dans la pile du calque (clic sur son nom, dans le panneau Effets).".into();
            return;
        };
        let has =self.layers().get(li).and_then(|l| l.fx.get(i)).is_some_and(|e| e.key_at(f).is_some());
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
        let Some(e) = self.fx_index().and_then(|i| self.layer().fx.get(i)) else { return };
        let f = self.cur_frame;
        let to = if dir > 0 { e.keys.iter().map(|k| k.frame).find(|&k| k > f) } else { e.keys.iter().rev().map(|k| k.frame).find(|&k| k < f) };
        match to {
            Some(k) => self.goto_frame(k),
            None => self.status = if dir > 0 { "Pas de clé après." } else { "Pas de clé avant." }.into(),
        }
    }


    pub(crate) fn fx_panel(&mut self, ui: &mut egui::Ui) {
        // Un clic dans le panneau lui donne la main : Ctrl+C / Ctrl+V visent ses effets (`fxcopie.rs`).
        if ui.input(|i| i.pointer.any_pressed()) && ui.rect_contains_pointer(ui.clip_rect()) {
            self.take_fx_pane_focus();
        }
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
        // Calque d'effet : ce qu'il fait (et l'éditeur d'une incrustation).
        self.effect_layer_section(ui);
        ui.add_enabled_ui(!locked, |ui| self.fx_stack_ui(ui));
    }

    fn fx_stack_ui(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let f = self.cur_frame;
        // (À la ligne quand le panneau est étroit.)
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(format!("{}  Ajouter un effet", ph::PLUS), |ui| {
                // Le tremblement du trait d'abord (calques vectoriels ; grisé sur un bitmap, avec la
                // raison), puis par familles, comme le menu Filtre de Photoshop. Les effets réservés
                // au vectoriel (famille « Trait ») sont grisés avec la raison sur un calque bitmap.
                self.add_boil_entry(ui, li);
                for family in FAMILIES {
                    ui.menu_button(family, |ui| {
                        let kinds: Vec<EffectKind> = EffectKind::ALL.into_iter().filter(|k| k.family() == family && *k != EffectKind::Boil).collect();
                        for k in kinds {
                            match self.fx_offer(li, k) {
                                Offer::Absent => {}
                                Offer::Disabled(why) => {
                                    ui.add_enabled(false, egui::Button::new(format!("{}  {}", crate::fxtrait::icon(k), k.label()))).on_disabled_hover_text(why);
                                }
                                Offer::Enabled => {
                                    let label = if k.is_trait() { format!("{}  {}", crate::fxtrait::icon(k), k.label()) } else { k.label().to_string() };
                                    let resp = ui.button(label);
                                    let resp = if k.hint().is_empty() { resp } else { resp.on_hover_text(k.hint()) };
                                    if resp.clicked() {
                                        ui.close_menu();
                                        self.add_effect_once(li, k);
                                    }
                                }
                            }
                        }
                    });
                }
            });
            // Préréglages d'effets (impact frame, lueur, flash, tremblement…) : un calque d'effet ou
            // des effets sur ce calque, avec leurs clés (`prereglagesui.rs`).
            ui.menu_button(format!("{}  Préréglages", ph::SPARKLE), |ui| self.effect_preset_menu(ui))
                .response
                .on_hover_text("Des effets tout réglés, avec leurs clés à partir de la tête de lecture : impact frame, lueur, flash, tremblement de l'image, vignettage, grain, timecode…");
            let any = !self.layers()[li].fx.is_empty();
            let chosen = self.fx_index();
            ui.menu_button(format!("{}  Copier / coller", ph::COPY),|ui| self.fx_clip_menu(ui, li, chosen, true))
                .response
                .on_hover_text("Copier un effet (réglages et clés) ou tous les effets du calque, les coller sur le calque ou sur les calques choisis. Ctrl+C / Ctrl+V font de même quand ce panneau a la main.");
            if ui.add_enabled(any, egui::Button::new("Tout réinitialiser")).on_hover_text("Chaque effet reprend ses réglages par défaut, sans clé.").clicked() {
                self.record_step();
                for e in &mut self.layers_mut()[li].fx {
                    *e = Effect { enabled: e.enabled, ..Effect::new(e.kind) };
                }
            }
            if ui.add_enabled(any, egui::Button::new("Tout supprimer")).clicked() {
                self.record_step();
                self.deselect_fx();
                self.fx_key_sel.clear();
                self.layers_mut()[li].fx.clear();
            }
        });
        self.trait_strip(ui, li);
        let key = self.raccourci("fx.cle_auto");
        let resp = ui.checkbox(&mut self.fx_auto_key, "Création automatique des clés");
        crate::bulles::bulle(resp, "Création automatique des clés", "Une clé est posée à l'image courante à chaque changement d'un paramètre.", &key);
        ui.checkbox(&mut self.fx_keys_shown, "Clés d'effets dans la timeline et le montage")
            .on_hover_text("Interrupteur général. Les clés de l'effet choisi ci-dessous se montrent, en losanges violets en haut de la piste de son calque : clic pour les choisir, glisser pour les déplacer, Ctrl+C / Ctrl+V / Suppr. Aucun effet choisi, aucun losange.");
        ui.separator();
        let n = self.layers()[li].fx.len();
        if n == 0 {
            ui.label(RichText::new("Aucun effet sur ce calque.").weak());
            return;
        }
        if self.fx_index().is_none() {
            ui.label(RichText::new("Clique sur le nom d'un effet pour voir ses clés dans la timeline et le montage.").weak());
        }
        let mut remove = None;
        let mut swap = None;
        for i in 0..n {
            let e = self.layers()[li].fx[i].clone();
            let chosen = self.fx_index() == Some(i);
            // L'effet choisi se voit : cadre et fond de la couleur de sélection.
            let sel = ui.visuals().selection;
            let mut frame = egui::Frame::group(ui.style());
            if chosen {
                frame = frame.stroke(egui::Stroke::new(1.5_f32,sel.stroke.color)).fill(sel.bg_fill.gamma_multiply(0.35));
            }
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut on = e.enabled;
                    if crate::bulles::bulle(ui.checkbox(&mut on, ""), "Effet actif", "Éteint, l'effet ne s'applique plus ; il reste dans la pile.", "").changed() {
                        self.record_step();
                        self.layers_mut()[li].fx[i].enabled = on;
                    }
                    // Le nom prend toute la place libre : une grande cible à cliquer.
                    let w = (ui.available_width() - 4.0 * 26.0).max(60.0);
                    let hint = if chosen {
                        "Effet choisi : ses clés se voient dans la timeline et le montage, et les boutons de clé le visent. Cliquer de nouveau pour ne plus en choisir."
                    } else {
                        "Choisir cet effet : ses clés se voient alors dans la timeline et le montage (les boutons de clé le visent)."
                    };
                    let name = ui.add_sized(egui::vec2(w, ui.spacing().interact_size.y), egui::SelectableLabel::new(chosen, crate::bulles::encre_etat(RichText::new(e.kind.label()).strong(), chosen)));
                    let about = if e.kind.hint().is_empty() { hint.to_string() } else { format!("{}\n{}", e.kind.hint(), hint) };
                    let name_resp = crate::bulles::bulle(name, if chosen { "Effet choisi" } else { "Choisir l'effet" }, &about, "");
                    // Clic droit sur le nom : copier cet effet ou la pile, coller (`fxcopie.rs`).
                    name_resp.context_menu(|ui| self.fx_clip_menu(ui, li, Some(i), true));
                    if name_resp.clicked() {
                        if chosen {
                            self.deselect_fx();
                        } else {
                            self.select_fx(i);
                        }
                    }
                    let key = self.raccourci("fx.copier");
                    let resp = ui.add(egui::Button::new(ph::COPY).frame(false));
                    if crate::bulles::bulle(resp, "Copier l'effet", "Réglages et clés. Ctrl+C le copie aussi quand l'effet est choisi dans ce panneau ; Ctrl+V colle sur le calque, ou sur les calques choisis.", &key).clicked() {
                        self.fx_copy_effect(li, i);
                    }
                    let up = ui.add_enabled(i > 0, egui::Button::new(ph::ARROW_UP).frame(false));
                    if crate::bulles::bulle(up, "Monter l'effet", "Le placer plus tôt dans la pile du calque.", "").clicked() {
                        swap = Some((i, i - 1));
                    }
                    let down = ui.add_enabled(i + 1 < n, egui::Button::new(ph::ARROW_DOWN).frame(false));
                    if crate::bulles::bulle(down, "Descendre l'effet", "Le placer plus tard dans la pile du calque.", "").clicked() {
                        swap = Some((i, i + 1));
                    }
                    if crate::bulles::bulle(ui.add(egui::Button::new(ph::TRASH).frame(false)), "Supprimer l'effet", "Le retire de la pile du calque, avec ses clés.", "").clicked() {
                        remove = Some(i);
                    }
                });
                let values = e.values_at(f);
                egui::Grid::new(("parametres_effet", i)).num_columns(2).show(ui, |ui| {
                    for (k, p) in e.kind.params().iter().enumerate() {
                        let param_hint = e.kind.param_hint(k);
                        let name = ui.label(p.name);
                        if !param_hint.is_empty() {
                            name.on_hover_text(param_hint);
                        }
                        let mut v = values[k];
                        let resp = match e.kind.param_style(k) {
                            crate::fx::ParamStyle::Slider => ui.add(egui::Slider::new(&mut v, p.min..=p.max).suffix(p.unit)),
                            crate::fx::ParamStyle::Integer => ui.add(egui::Slider::new(&mut v, p.min..=p.max).suffix(p.unit).step_by(1.0).fixed_decimals(0)),
                            crate::fx::ParamStyle::Switch => {
                                let mut on = v >= 0.5;
                                let resp = ui.checkbox(&mut on, "");
                                let resp = if param_hint.is_empty() { resp } else { resp.on_hover_text(param_hint) };
                                v = if on { 1.0 } else { 0.0 };
                                resp
                            }
                        };
                        // Pendant le geste, l'image passe en aperçu (`fx::draft_level`) ; la pleine
                        // résolution revient au lâcher.
                        if resp.dragged() || resp.changed() {
                            crate::fx::note_interaction(ui.ctx());
                        }
                        if self.edited(&resp) {
                            let mut new = values.clone();
                            new[k] = v;
                            let auto = self.fx_auto_key;
                            let effect = &mut self.layers_mut()[li].fx[i];
                            if e.kind.param_style(k) == crate::fx::ParamStyle::Switch {
                                // (Un interrupteur ne s'anime pas : il vaut pour toutes les clés.)
                                effect.set_param(k, v);
                            } else if auto || !effect.keys.is_empty() {
                                effect.set_key(f, new);
                            } else {
                                effect.values = new;
                            }
                            self.select_fx(i);
                        }
                        ui.end_row();
                    }
                });
                ui.horizontal(|ui| {
                    let here = e.key_at(f).is_some();
                    let key = self.raccourci("fx.cle_creer");
                    let resp = ui.button(format!("{}  Créer une clé", ph::DIAMOND));
                    if crate::bulles::bulle(resp, "Créer une clé", "À l'image courante, avec les paramètres de l'effet.", &key).clicked() {
                        self.select_fx(i);
                        self.fx_create_key();
                    }
                    let key = self.raccourci("fx.cle_supprimer");
                    let resp = ui.add_enabled(here, egui::Button::new("Supprimer la clé"));
                    if crate::bulles::bulle(resp, "Supprimer la clé", "La clé de cet effet à l'image courante.", &key).clicked() {
                        self.select_fx(i);
                        self.fx_delete_key();
                    }
                    if !e.keys.is_empty() {
                        if crate::bulles::bulle(ui.button(ph::CARET_LEFT), "Clé précédente", "Aller à la clé d'effet d'avant.", "").clicked() {
                            self.select_fx(i);
                            self.fx_goto_key(-1);
                        }
                        if crate::bulles::bulle(ui.button(ph::CARET_RIGHT), "Clé suivante", "Aller à la clé d'effet d'après.", "").clicked() {
                            self.select_fx(i);
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
            // (Les clés choisies se repèrent par le rang de leur effet : le choix est lâché.)
            self.fx_key_sel.clear();
            self.fx_key_anchor = None;
            let chosen = self.fx_index();
            self.layers_mut()[li].fx.swap(a, b);
            // L'effet choisi reste choisi, à son nouveau rang.
            if let Some(c) = chosen {
                self.select_fx(crate::fxkeys::rank_after_swap(c, a, b));
            }
        }
        if let Some(i) = remove {
            self.record_step();
            self.fx_key_sel.clear();
            self.fx_key_anchor = None;
            let chosen = self.fx_index();
            self.layers_mut()[li].fx.remove(i);
            // Retiré, l'effet choisi ne l'est plus ; plus loin dans la pile, il monte d'un rang.
            match chosen.map(|c| crate::fxkeys::rank_after_remove(c, i)) {
                Some(Some(c)) => self.select_fx(c),
                Some(None) => self.deselect_fx(),
                None => {}
            }
        }
    }
}
