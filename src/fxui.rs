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
        Offer::Disabled(match kind {
            EffectKind::StrokeGradient | EffectKind::Nib => crate::fxmatiere::bitmap_why(kind),
            k if k.is_trait() => TRAIT_VECTOR_ONLY,
            _ => BOIL_VECTOR_ONLY,
        })
    } else {
        Offer::Enabled
    }
}

/// Quels effets d'une pile de `n` effets sont dépliés dans le panneau Effets, d'après ce qui l'était (`avant`, un
/// booléen par effet ; `None` : le calque n'a pas encore été vu dans la séance) et l'effet choisi `choisi` :
/// - un nouvel effet (la pile a grandi : ajout, collage, préréglage) s'ouvre, tous les autres se replient ;
/// - la pile a rétréci (suppression, annuler) : les autres gardent leur état ;
/// - un calque vu pour la première fois : ses un ou deux effets sont ouverts ; au-delà, tout est replié sauf
///   l'effet choisi.
/// L'état n'est gardé que pour la séance : ni dans le projet, ni dans les réglages.
pub(crate) fn plis_apres(avant: Option<&[bool]>, n: usize, choisi: Option<usize>) -> Vec<bool> {
    match avant {
        None if n <= 2 => vec![true; n],
        None => (0..n).map(|i| choisi == Some(i)).collect(),
        Some(v) if v.len() == n => v.to_vec(),
        Some(v) if v.len() < n => (0..n).map(|i| i >= v.len()).collect(),
        Some(v) => v[..n].to_vec(),
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

    /// Le bouton « fx » de la ligne du calque `li` se montre : le calque a une pile d'effets, ou ses effets
    /// sont éteints (il faut pouvoir les rallumer). Pas pour un dossier.
    pub(crate) fn fx_switch_shown(&self, li: usize) -> bool {
        self.layers().get(li).is_some_and(|l| !l.folder && (!l.fx.is_empty() || !l.fx_enabled))
    }

    /// Allume ou éteint d'un coup toute la pile d'effets du calque `li` (une étape d'annulation) : la pile
    /// n'est pas touchée (réglages, clés, effets cochés), elle ne s'applique simplement plus. Enregistré avec le
    /// calque (`effets_eteints`).
    pub(crate) fn toggle_layer_fx(&mut self, li: usize) {
        if li >= self.layers().len() || self.content_refuses() {
            return;
        }
        if self.layers()[li].folder {
            self.status = "Un dossier n'a pas d'effets : ils sont sur ses calques.".into();
            return;
        }
        self.end_stroke();
        let on = self.layers()[li].fx_enabled;
        self.record_step();
        self.layers_mut()[li].fx_enabled = !on;
        self.renderer.invalidate_all();
        self.status = if on {
            "Effets du calque éteints (la pile est gardée, le bouton fx les rallume).".into()
        } else {
            "Effets du calque rallumés.".into()
        };
    }

    /// Le bouton « fx » d'une ligne de calque : allumé quand les effets du calque jouent, éteint (barré) sinon.
    /// Rien si le calque n'a pas d'effets (`fx_switch_shown`). `rect` : où le poser (ligne de la timeline), ou
    /// `None` pour le placer dans la mise en page (panneau Calques).
    pub(crate) fn fx_switch(&mut self, ui: &mut egui::Ui, li: usize, rect: Option<egui::Rect>, ink: egui::Color32) -> Option<egui::Response> {
        if !self.fx_switch_shown(li) {
            return None;
        }
        let on = self.layers()[li].fx_enabled;
        let n = self.layers()[li].fx.len();
        let key = self.raccourci("calque.effets");
        let text = RichText::new("fx").size(13.0).italics();
        let text = if on { text.strong().color(ink) } else { text.strikethrough().color(ink.gamma_multiply(0.45)) };
        let button = egui::Button::new(text).frame(false).min_size(egui::vec2(18.0, 0.0));
        let resp = match rect {
            Some(r) => ui.put(r, button),
            None => ui.add(button),
        };
        let about = if on {
            format!("Les effets du calque jouent ({n}). Clic : les éteindre d'un coup ; la pile (réglages, clés) est gardée.")
        } else {
            format!("Les effets du calque sont éteints ({n} gardé{}). Clic : les rallumer.", if n > 1 { "s" } else { "" })
        };
        let resp = crate::bulles::bulle(resp, if on { "Effets allumés" } else { "Effets éteints" }, &about, &key);
        crate::raccourcisui::shortcut_menu(&resp, "calque.effets", &key);
        Some(resp)
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
        // L'interrupteur « fx » de la ligne du calque, aussi ici (la colonne du montage n'a pas la place de son bouton).
        self.fx_switch_item(ui, li);
        self.boil_menu_items(ui, li);
    }

    /// « Effets du calque allumés » : la case de l'interrupteur « fx » du calque `li`, dans un menu (rien si le calque
    /// n'a pas d'effets, `fx_switch_shown`). Le menu « … » du panneau Calques et le clic droit d'un calque l'ont.
    pub(crate) fn fx_switch_item(&mut self, ui: &mut egui::Ui, li: usize) {
        if !self.fx_switch_shown(li) {
            return;
        }
        let on = self.layers()[li].fx_enabled;
        let resp = ui.checkbox(&mut on.clone(), "Effets du calque allumés").on_hover_text("Éteint, aucun effet de ce calque ne joue ; la pile est gardée. Bouton fx de la ligne du calque.");
        if resp.clicked() {
            ui.close_menu();
            self.toggle_layer_fx(li);
        }
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
        let i = match fx.iter().position(|e| e.kind == kind).filter(|_| kind.single()) {
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

    /// Une entrée de menu d'effet (menus déroulants « Trait » et « Matière ») : icône et nom, allumée
    /// quand l'effet est sur le calque et actif, grisée avec la raison sur un calque qui ne le prend pas.
    /// Clic : coche ou décoche l'effet (l'action de l'effet : une étape d'annulation). Clic droit :
    /// lui assigner un raccourci.
    fn fx_menu_entry(&mut self, ui: &mut egui::Ui, li: usize, kind: EffectKind, icon: &str, action: &'static str) {
        // Un effet que ce calque ne prend pas ne se montre pas (caché plutôt que grisé, 2026-10-21).
        if self.fx_offer(li, kind) != Offer::Enabled {
            return;
        }
        let on = self.fx_on(li, kind);
        let key = self.raccourci(action);
        let label = crate::bulles::encre_etat(RichText::new(format!("{icon}  {}", kind.label())), on);
        let resp = ui.add(egui::Button::new(label).selected(on).min_size(egui::vec2(ui.available_width(), 0.0)));
        let resp = crate::bulles::bulle(resp, kind.label(), kind.hint(), &key);
        crate::raccourcisui::shortcut_menu(&resp, action, &key);
        if resp.clicked() {
            ui.close_menu();
            self.run_action(action);
        }
    }

    /// Combien d'effets de la liste `kinds` sont sur le calque `li` et actifs (le chiffre du bouton du menu).
    pub(crate) fn fx_count_on(&self, li: usize, kinds: &[EffectKind]) -> usize {
        kinds.iter().filter(|k| self.fx_on(li, **k)).count()
    }

    /// Le menu déroulant « Trait » (`fxtrait.rs`) : le tremblement du trait en tête (même famille, depuis le
    /// 2026-10-21), puis tous les effets du trait, icône et nom, puis dans une section les préréglages. Un effet allumé est sur le calque et actif ; grisé avec la raison dans la
    /// bulle sur un calque bitmap. Il remplace la rangée d'icônes d'avant (Anthony, 2026-10-20 : « un seul
    /// menu déroulant »).
    pub(crate) fn trait_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let n = self.fx_count_on(li, &crate::fxtrait::menu_kinds());
        let title = if n > 0 { format!("Trait ({n}) {}", ph::CARET_DOWN) } else { format!("Trait {}", ph::CARET_DOWN) };
        let menu = ui.menu_button(title, |ui| {
            // Le tremblement du trait en tête (même famille), puis les effets du trait.
            for kind in crate::fxtrait::menu_kinds() {
                self.fx_menu_entry(ui, li, kind, crate::fxtrait::icon(kind), crate::fxtrait::action(kind));
            }
            ui.separator();
            ui.label(RichText::new(format!("{}  Préréglages", ph::MAGIC_WAND)).weak());
            let usable = self.fx_offer(li, EffectKind::Write) == Offer::Enabled;
            for preset in crate::fxtrait::Preset::ALL {
                let key = self.raccourci(preset.action());
                let note = if usable { preset.note() } else { TRAIT_VECTOR_ONLY };
                let resp = crate::bulles::bulle(ui.add_enabled(usable, egui::Button::new(preset.label())), preset.label(), note, &key);
                if usable && resp.clicked() {
                    ui.close_menu();
                    self.apply_trait_preset(preset);
                }
            }
        });
        crate::bulles::bulle(menu.response, "Effets du trait", "Tremblement, écriture, crayonné, respiration, ondulation, pointillés, désagrégation, et des préréglages tout réglés. Les effets cochés sont sur ce calque.", "");
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

    /// Le menu déroulant « Matière » (`fxmatiere.rs`) : comme « Trait », juste à côté. Le liseré, le trait
    /// sec et l'encre qui bave vont sur toutes les sortes de calques ; le dégradé et le changement de
    /// plume, sur le vectoriel seulement (grisés avec la raison sur un bitmap).
    pub(crate) fn matiere_menu(&mut self, ui: &mut egui::Ui, li: usize) {
        let n = self.fx_count_on(li, &crate::fxmatiere::KINDS);
        let title = if n > 0 { format!("Matière ({n}) {}", ph::CARET_DOWN) } else { format!("Matière {}", ph::CARET_DOWN) };
        let menu = ui.menu_button(title, |ui| {
            for kind in crate::fxmatiere::KINDS {
                self.fx_menu_entry(ui, li, kind, crate::fxmatiere::icon(kind), crate::fxmatiere::action(kind));
            }
            ui.separator();
            ui.label(RichText::new(format!("{}  Préréglages", ph::MAGIC_WAND)).weak());
            let usable = self.fx_offer(li, EffectKind::Outline) == Offer::Enabled;
            for preset in crate::fxmatiere::Preset::ALL {
                let key = self.raccourci(preset.action());
                // Un préréglage qui demande un calque vectoriel ne se montre pas sur un bitmap.
                let ok = usable && (!preset.vector_only() || self.fx_offer(li, EffectKind::Nib) == Offer::Enabled);
                if !ok {
                    continue;
                }
                let resp = crate::bulles::bulle(ui.add(egui::Button::new(preset.label())), preset.label(), preset.note(), &key);
                if resp.clicked() {
                    ui.close_menu();
                    self.apply_matter_preset(preset);
                }
            }
        });
        crate::bulles::bulle(menu.response, "Effets de matière", "Liseré, trait sec, encre qui bave, dégradé le long du trait, changer de plume, et des préréglages tout réglés. Les effets cochés sont sur ce calque.", "");
    }

    /// Pose les effets du préréglage de matière `preset` sur le calque courant (une étape d'annulation) :
    /// ceux de même sorte sont remplacés.
    pub(crate) fn apply_matter_preset(&mut self, preset: crate::fxmatiere::Preset) {
        let li = self.cur_layer;
        let built = preset.build();
        if built.iter().any(|e| self.fx_offer(li, e.kind) == Offer::Absent) {
            return;
        }
        if built.iter().any(|e| !self.fx_allowed(li, e.kind)) {
            self.status = format!("« {} » ne va que sur un calque vectoriel.", preset.label());
            return;
        }
        if self.content_refuses() {
            return;
        }
        self.record_step();
        let stack = &mut self.layers_mut()[li].fx;
        for e in built {
            stack.retain(|x| x.kind != e.kind);
            stack.push(e);
        }
        let last = stack.len() - 1;
        self.select_fx(last);
        self.renderer.invalidate_all();
        self.status = format!("{} : posé sur le calque (les dessins n'ont pas changé).", preset.label());
    }

    /// « Appliquer pour de bon » : remplace les traits vectoriels du calque `li` par ceux que redessine la plume
    /// de l'effet « Changer de plume » (une étape d'annulation nommée) et retire l'effet. Les zones remplies et
    /// les textes restent tels quels. Les traits ne sont pas rejoués ici : ceux que l'effet a déjà redessinés
    /// gardent leurs pixels, les autres sont rendus en rapide puis exactement en arrière-plan (`rejeu.rs`).
    pub(crate) fn bake_nib(&mut self, li: usize) {
        let frame = self.cur_frame;
        let found = self.layers().get(li).and_then(|l| l.fx.iter().position(|e| e.kind == EffectKind::Nib).map(|i| (l.kind, l.frames.clone(), i, l.fx[i].values_at(frame))));
        let Some((kind, frames, i, values)) = found else {
            self.status = "Pas d'effet « Changer de plume » sur ce calque.".into();
            return;
        };
        let Some(nib) = crate::fxmatiere::NibCtx::resolve(&crate::fxmatiere::NibS::of(&values)) else {
            self.status = "La plume choisie n'existe plus : choisis-en une autre avant de l'appliquer.".into();
            return;
        };
        if kind != crate::document::LayerKind::Vector || self.content_refuses() {
            return;
        }
        let mut cels: Vec<crate::document::CelId> = Vec::new();
        for id in frames.iter().flatten() {
            if !cels.contains(id) {
                cels.push(*id);
            }
        }
        // Étape d'annulation : la structure d'avant (l'effet) et le dessin d'avant de chaque dessin changé.
        let mut edits = vec![self.snapshot()];
        let (w, h) = (self.doc.width, self.doc.height);
        let mut changed = Vec::new();
        for cel in cels {
            let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
            let Some(before) = c.vector.clone() else { continue };
            let mut baked = before.clone();
            if crate::fxmatiere::bake_nib(&mut baked, &nib) == 0 {
                continue;
            }
            // (Les pixels : ceux du cache des traits redessinés ; ce qui manque, en rapide d'abord.)
            c.raster = baked.render_quick(w, h);
            c.vector = Some(baked);
            c.version += 1;
            edits.push(crate::history::Edit::Vector { cel, drawing: before });
            changed.push(cel);
        }
        self.layers_mut()[li].fx.remove(i);
        self.deselect_fx();
        self.fx_key_sel.clear();
        self.named_step("Plume appliquée pour de bon", |app| app.history.push(crate::history::Edit::Group(edits)));
        self.modified = true;
        self.renderer.invalidate_all();
        for cel in &changed {
            self.rejeu_cel(*cel);
        }
        let plural = if changed.len() > 1 { "s" } else { "" };
        let name = crate::fxmatiere::nib_name(crate::fxmatiere::NibS::of(&values).id).unwrap_or_default();
        self.status = format!("Plume « {name} » appliquée pour de bon : {} dessin{plural} refait{plural}, l'effet est retiré (Ctrl+Z pour revenir).", changed.len());
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
        // L'interrupteur « fx » de la ligne du calque est éteint : on le dit ici, et on le rallume.
        if !self.layers()[li].fx_enabled {
            ui.horizontal(|ui| {
                ui.colored_label(crate::theme::avertissement(), "Les effets de ce calque sont éteints (bouton fx).");
                if crate::bulles::bulle(ui.small_button("Rallumer"), "Rallumer les effets", "La pile du calque joue de nouveau.", &self.raccourci("calque.effets")).clicked() {
                    self.toggle_layer_fx(li);
                }
            });
        }
        // (À la ligne quand le panneau est étroit.)
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(format!("{}  Ajouter un effet", ph::PLUS), |ui| {
                // Par familles, comme le menu Filtre de Photoshop. Les effets réservés au vectoriel
                // (famille « Trait », où est aussi le tremblement du trait) ne se montrent pas sur un calque
                // bitmap (on cache ce qui ne peut pas servir plutôt que de le griser, 2026-10-21) : une
                // famille sans effet utilisable disparaît.
                for family in FAMILIES {
                    let kinds: Vec<EffectKind> = EffectKind::ALL.into_iter().filter(|k| k.family() == family && self.fx_offer(li, *k) == Offer::Enabled).collect();
                    if kinds.is_empty() {
                        continue;
                    }
                    ui.menu_button(family, |ui| {
                        for k in kinds {
                            let label = if k.is_trait() || k == EffectKind::Boil { format!("{}  {}", crate::fxtrait::icon(k), k.label()) } else { k.label().to_string() };
                            let resp = ui.button(label);
                            let resp = if k.hint().is_empty() { resp } else { resp.on_hover_text(k.hint()) };
                            if resp.clicked() {
                                ui.close_menu();
                                self.add_effect_once(li, k);
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
            // Rien à réinitialiser ni à supprimer sans effet : les deux boutons ne se montrent qu'alors.
            let any = !self.layers()[li].fx.is_empty();
            if any && ui.button("Tout réinitialiser").on_hover_text("Chaque effet reprend ses réglages par défaut, sans clé.").clicked() {
                self.record_step();
                for e in &mut self.layers_mut()[li].fx {
                    *e = Effect { enabled: e.enabled, ..Effect::new(e.kind) };
                }
            }
            if any && ui.button("Tout supprimer").clicked() {
                self.record_step();
                self.deselect_fx();
                self.fx_key_sel.clear();
                self.layers_mut()[li].fx.clear();
            }
        });
        // Les effets du trait et de matière : un menu déroulant chacun (icône et nom, préréglages en section).
        // Ceux du trait ne vont que sur un calque vectoriel : sur un autre, le menu se cache et une ligne dit où les
        // trouver (au lieu d'un menu grisé, ou de rien : on cherchait le tremblement, 2026-10-19).
        ui.horizontal_wrapped(|ui| {
            if self.fx_offer(li, EffectKind::Boil) == Offer::Enabled {
                self.trait_menu(ui, li);
            } else {
                ui.label(RichText::new("Effets du trait (tremblement…) : sur un calque vectoriel.").small().weak());
            }
            self.matiere_menu(ui, li);
        });
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
            ui.label(RichText::new("Clique sur le nom d'un effet pour le choisir : ses clés se voient dans la timeline et le montage, et Ctrl+C le copie (Ctrl ou Maj + clic : plusieurs).").weak());
        }
        // Les effets choisis (Ctrl / Maj + clic) : tous sont encadrés ; le principal montre ses clés.
        let picked = self.fx_selected();
        let mut remove = None;
        let mut swap = None;
        let mut bake = false;
        // Les effets dépliés : un nouvel effet s'ouvre, les autres se replient (séance seulement).
        let layer_id = self.layers()[li].id;
        let flags = plis_apres(self.effets_ui.open.get(&layer_id).map(Vec::as_slice), n, self.fx_index());
        if self.effets_ui.open.get(&layer_id) != Some(&flags) {
            self.effets_ui.open.insert(layer_id, flags);
        }
        let mut toggle_open = None;
        // Le menu du clic droit sur le nom d'un effet se montre au milieu de la pile : « Remplacer les effets » (ou un collage) la
        // change, et les effets d'après se lisaient à un rang qui n'existait plus (passe de débogage, 2026-10-22). La pile s'arrête
        // là, et les clics de cette image, qui visaient les rangs d'avant, sont lâchés ; l'image suivante la refait.
        let kinds: Vec<EffectKind> = self.layers()[li].fx.iter().map(|e| e.kind).collect();
        let tient = |app: &App| app.cur_layer == li && app.layers().get(li).is_some_and(|l| l.id == layer_id && l.fx.len() == n && l.fx.iter().zip(&kinds).all(|(e, k)| e.kind == *k));
        for i in 0..n {
            if !tient(self) {
                ui.ctx().request_repaint();
                return;
            }
            let e = self.layers()[li].fx[i].clone();
            let chosen = picked.contains(&i);
            let opened = self.effets_ui.open.get(&layer_id).and_then(|v| v.get(i)).copied().unwrap_or(true);
            // L'effet choisi se voit : cadre et fond de la couleur de sélection.
            let sel = ui.visuals().selection;
            let mut frame = egui::Frame::group(ui.style());
            if chosen {
                frame = frame.stroke(egui::Stroke::new(1.5_f32,sel.stroke.color)).fill(sel.bg_fill.gamma_multiply(0.35));
            }
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    // La flèche : replie l'effet (seul son nom reste) ou le déplie (séance seulement).
                    let (caret, caret_nom) = if opened { (ph::CARET_DOWN, "Replier l'effet") } else { (ph::CARET_RIGHT, "Déplier l'effet") };
                    let resp = ui.add(egui::Button::new(RichText::new(caret).size(crate::bulles::TAILLE - 2.0)).frame(false));
                    if crate::bulles::bulle(resp, caret_nom, "Ses réglages et ses clés se montrent ou se cachent ; l'effet joue toujours.", "").clicked() {
                        toggle_open = Some(i);
                    }
                    let mut on = e.enabled;
                    if crate::bulles::bulle(ui.checkbox(&mut on, ""), "Effet actif", "Éteint, l'effet ne s'applique plus ; il reste dans la pile.", "").changed() {
                        self.record_step();
                        self.layers_mut()[li].fx[i].enabled = on;
                    }
                    // Le nom prend toute la place libre : une grande cible à cliquer.
                    let w = (ui.available_width() - 26.0).max(60.0);
                    let hint = if chosen {
                        "Effet choisi : Ctrl+C le copie avec ses réglages et ses clés ; Ctrl+V sur un autre calque le colle. Cliquer de nouveau pour ne plus en choisir. Ctrl ou Maj + clic : choisir plusieurs effets."
                    } else {
                        "Choisir cet effet : ses clés se voient dans la timeline et le montage, et Ctrl+C le copie (avec ses réglages et ses clés). Ctrl ou Maj + clic : en choisir plusieurs."
                    };
                    let name = ui.add_sized(egui::vec2(w, ui.spacing().interact_size.y), egui::SelectableLabel::new(chosen, crate::bulles::encre_etat(RichText::new(e.kind.label()).strong(), chosen)));
                    let about = if e.kind.hint().is_empty() { hint.to_string() } else { format!("{}\n{}", e.kind.hint(), hint) };
                    let name_resp = crate::bulles::bulle(name, if chosen { "Effet choisi" } else { "Choisir l'effet" }, &about, "");
                    // Clic droit sur le nom : copier cet effet ou la pile, coller (`fxcopie.rs`).
                    name_resp.context_menu(|ui| self.fx_clip_menu(ui, li, Some(i), true));
                    // Le clic le choisit et donne la main au panneau : Ctrl+C copie les effets choisis.
                    if name_resp.clicked() {
                        let m = ui.input(|inp| inp.modifiers);
                        self.fx_click_name(i, m.command || m.ctrl, m.shift);
                    }
                    // (Pas de bouton Copier : Ctrl+C copie les effets choisis, Ctrl+V les colle ; le clic droit sur le nom
                    // aussi, `fxcopie.rs`.)
                    if crate::bulles::bulle(ui.add(egui::Button::new(ph::TRASH).frame(false)), "Supprimer l'effet", "Le retire de la pile du calque, avec ses clés.", "").clicked() {
                        remove = Some(i);
                    }
                });
                // Replié : seul l'en-tête reste ; l'effet joue toujours.
                if opened {
                let values = e.values_at(f);
                egui::Grid::new(("parametres_effet", i)).num_columns(2).show(ui, |ui| {
                    for (k, p) in e.kind.params().iter().enumerate() {
                        // Le vert et le bleu d'une couleur sont dans son sélecteur.
                        if crate::fxmatiere::color_tail(e.kind, k) {
                            continue;
                        }
                        let param_hint = e.kind.param_hint(k);
                        // Une couleur (liseré, arrêts du dégradé) : un sélecteur de couleur, pas trois curseurs.
                        if crate::fxmatiere::color_start(e.kind, k).is_some() {
                            let name = ui.label(crate::fxmatiere::color_label(e.kind, k));
                            if !param_hint.is_empty() {
                                name.on_hover_text(param_hint);
                            }
                            let mut rgb = [values[k] as u8, values[k + 1] as u8, values[k + 2] as u8];
                            let resp = ui.color_edit_button_srgb(&mut rgb);
                            let resp = if param_hint.is_empty() { resp } else { resp.on_hover_text(param_hint) };
                            if resp.dragged() || resp.changed() {
                                crate::fx::note_interaction(ui.ctx());
                            }
                            if self.edited(&resp) {
                                let mut new = values.clone();
                                for c in 0..3 {
                                    new[k + c] = rgb[c] as f32;
                                }
                                let auto = self.fx_auto_key;
                                let effect = &mut self.layers_mut()[li].fx[i];
                                if auto || !effect.keys.is_empty() {
                                    effect.set_key(f, new);
                                } else {
                                    effect.values = new;
                                }
                                self.select_fx(i);
                            }
                            ui.end_row();
                            continue;
                        }
                        let name = ui.label(p.name);
                        if !param_hint.is_empty() {
                            name.on_hover_text(param_hint);
                        }
                        let mut v = values[k];
                        let mut nib_changed = false;
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
                            // Le choix d'une plume : la liste des réglages de pinceau de la bibliothèque.
                            crate::fx::ParamStyle::Nib => {
                                let names: Vec<String> = self.brushes.presets.iter().filter(|p| p.target == crate::brushtex::Target::Brush).map(|p| p.name.clone()).collect();
                                let choices = crate::fxmatiere::nib_choices(&names);
                                let current = v.round().max(0.0) as u32;
                                let shown = choices.iter().find(|(id, _)| *id == current).map_or_else(|| "(plume introuvable)".to_string(), |(_, n)| n.clone());
                                let mut picked = current;
                                let resp = egui::ComboBox::from_id_salt(("plume", i))
                                    .selected_text(shown)
                                    .show_ui(ui, |ui| {
                                        for (id, name) in &choices {
                                            ui.selectable_value(&mut picked, *id, name.as_str());
                                        }
                                    })
                                    .response;
                                let resp = if param_hint.is_empty() { resp } else { resp.on_hover_text(param_hint) };
                                if picked != current {
                                    v = picked as f32;
                                    nib_changed = true;
                                }
                                resp
                            }
                        };
                        // Pendant le geste, l'image passe en aperçu (`fx::draft_level`) ; la pleine
                        // résolution revient au lâcher.
                        if resp.dragged() || resp.changed() {
                            crate::fx::note_interaction(ui.ctx());
                        }
                        let changed = if nib_changed {
                            self.record_step();
                            true
                        } else {
                            self.edited(&resp)
                        };
                        if changed {
                            let mut new = values.clone();
                            new[k] = v;
                            let auto = self.fx_auto_key;
                            let effect = &mut self.layers_mut()[li].fx[i];
                            if matches!(e.kind.param_style(k), crate::fx::ParamStyle::Switch | crate::fx::ParamStyle::Nib) {
                                // (Un interrupteur ou un choix ne s'anime pas : il vaut pour toutes les clés.)
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
                // « Changer de plume » : remplacer pour de bon les traits par ceux de la plume.
                if e.kind == EffectKind::Nib {
                    let key = self.raccourci(crate::fxmatiere::BAKE_ACTION);
                    let resp = ui.button(format!("{}  Appliquer pour de bon", ph::CHECK));
                    crate::raccourcisui::shortcut_menu(&resp, crate::fxmatiere::BAKE_ACTION, &key);
                    if crate::bulles::bulle(resp, "Appliquer pour de bon", "Remplace les traits du calque par ceux de la plume (leur tracé et leur pression sont gardés) et retire l'effet. Une étape d'annulation : Ctrl+Z revient aux traits d'origine.", &key).clicked() {
                        bake = true;
                    }
                }
                ui.horizontal(|ui| {
                    let here = e.key_at(f).is_some();
                    let key = self.raccourci("fx.cle_creer");
                    let resp = ui.button(format!("{}  Créer une clé", ph::DIAMOND));
                    if crate::bulles::bulle(resp, "Créer une clé", "À l'image courante, avec les paramètres de l'effet.", &key).clicked() {
                        self.select_fx(i);
                        self.fx_create_key();
                    }
                    // Pas de clé ici : rien à supprimer, le bouton se cache au lieu de se griser.
                    if here {
                        let key = self.raccourci("fx.cle_supprimer");
                        let resp = ui.button("Supprimer la clé");
                        if crate::bulles::bulle(resp, "Supprimer la clé", "La clé de cet effet à l'image courante.", &key).clicked() {
                            self.select_fx(i);
                            self.fx_delete_key();
                        }
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
                    // Sa place dans la pile : plus tôt ou plus tard (le haut ou le bas de la pile cache la flèche).
                    if i > 0 && crate::bulles::bulle(ui.add(egui::Button::new(ph::ARROW_UP).frame(false)), "Monter l'effet", "Le placer plus tôt dans la pile du calque.", "").clicked() {
                        swap = Some((i, i - 1));
                    }
                    if i + 1 < n && crate::bulles::bulle(ui.add(egui::Button::new(ph::ARROW_DOWN).frame(false)), "Descendre l'effet", "Le placer plus tard dans la pile du calque.", "").clicked() {
                        swap = Some((i, i + 1));
                    }
                });
                }
            });
        }
        if !tient(self) {
            ui.ctx().request_repaint();
            return;
        }
        // Un effet replié ou déplié : l'état n'est que de la séance (`plis_apres`).
        if let Some(i) = toggle_open {
            if let Some(flag) = self.effets_ui.open.get_mut(&layer_id).and_then(|v| v.get_mut(i)) {
                *flag = !*flag;
            }
        }
        if let Some((a, b)) = swap {
            if let Some(v) = self.effets_ui.open.get_mut(&layer_id) {
                if a < v.len() && b < v.len() {
                    v.swap(a, b);
                }
            }
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
            if let Some(v) = self.effets_ui.open.get_mut(&layer_id) {
                if i < v.len() {
                    v.remove(i);
                }
            }
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
        if bake {
            self.bake_nib(li);
        }
    }
}
