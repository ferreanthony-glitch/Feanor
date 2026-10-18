//! Fenêtres modulables dans l'application (5b) : contenu de chaque panneau, affichage par
//! egui_dock, ouverture et fermeture des panneaux ; panneaux dans leur propre fenêtre Windows
//! (5g, par exemple sur un deuxième écran).

use eframe::egui::{self, Id, WidgetText};

use crate::app::{App, BoardView, Mode};
use crate::dock::{self, DetachedPanel, DockLayout, Group, Pane};

/// Ce qu'egui_dock affiche dans chaque onglet.
struct Panes<'a> {
    app: &'a mut App,
}

impl egui_dock::TabViewer for Panes<'_> {
    type Tab = Pane;

    fn title(&mut self, tab: &mut Pane) -> WidgetText {
        format!("{} {}", tab.icon(), self.app.pane_title(*tab)).into()
    }

    fn id(&mut self, tab: &mut Pane) -> Id {
        Id::new(("panneau", *tab))
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Pane) {
        // La hauteur du contenu, pour les panneaux qui s'ajustent (`ajuste.rs`) : egui_dock étend
        // la zone à tout l'onglet avant de nous la donner, on suit donc où le contenu s'arrête.
        let top = ui.cursor().top();
        self.app.pane_ui(ui, *tab);
        if crate::ajuste::measured(*tab) {
            let h = ui.cursor().top() - top;
            if h.is_finite() && h > 0.0 {
                self.app.pane_heights.insert(*tab, h);
            }
        }
    }

    /// Clic droit sur un onglet : le panneau dans sa propre fenêtre.
    fn context_menu(&mut self, ui: &mut egui::Ui, tab: &mut Pane, _surface: egui_dock::SurfaceIndex, _node: egui_dock::NodeIndex) {
        if *tab != Pane::Canvas
            && ui
                .button("Dans une fenêtre à part")
                .on_hover_text("Une vraie fenêtre, à poser sur un autre écran ; la fermer remet le panneau à sa place")
                .clicked()
        {
            self.app.detach_request = Some(*tab);
            ui.close_menu();
        }
    }

    /// Le dessin ne se ferme pas et ne flotte pas.
    fn closeable(&mut self, tab: &mut Pane) -> bool {
        *tab != Pane::Canvas
    }

    fn allowed_in_windows(&self, tab: &mut Pane) -> bool {
        *tab != Pane::Canvas
    }

    fn clear_background(&self, tab: &Pane) -> bool {
        *tab != Pane::Canvas
    }

    /// Le dessin et la bande gèrent eux-mêmes leur place ; les autres défilent.
    fn scroll_bars(&self, tab: &Pane) -> [bool; 2] {
        match tab {
            // Calques : l'en-tête (mode, opacité, commandes) reste en haut, seule la liste défile
            // (`layers_panel`). L'historique défile seul, pour rester sur la dernière étape. Pinceaux : la
            // taille et les onglets restent en haut, les boutons en bas, la liste défile entre les deux.
            Pane::Canvas | Pane::Montage | Pane::Timeline | Pane::XSheet | Pane::Layers | Pane::History | Pane::Brushes => [false, false],
            _ => [false, true],
        }
    }

    fn on_tab_button(&mut self, tab: &mut Pane, response: &egui::Response) {
        if tab.tab_name() != tab.name() {
            response.clone().on_hover_text(tab.name());
        }
    }
}

impl App {
    /// Nom affiché d'un panneau (la bande et le dessin changent selon ce qu'ils montrent).
    pub(crate) fn pane_title(&self, pane: Pane) -> &'static str {
        match pane {
            Pane::Montage | Pane::Timeline if self.mode == Mode::Animation => "Timeline",
            Pane::Montage | Pane::Timeline => "Montage",
            Pane::Canvas if self.mode == Mode::Storyboard && self.board_view == BoardView::Panels => "Vignettes",
            p => p.tab_name(),
        }
    }

    /// Le contenu d'un panneau (dans un onglet ou dans sa propre fenêtre).
    pub(crate) fn pane_ui(&mut self, ui: &mut egui::Ui, pane: Pane) {
        // Les étapes d'annulation enregistrées ici portent le nom de l'outil (dessin) ou du
        // panneau (panneau Historique) ; une commande lancée d'ici donne le sien.
        let name = match pane {
            Pane::Canvas => self.tool.label().to_string(),
            p => self.pane_title(p).to_string(),
        };
        let before = self.history.context.replace(name);
        self.pane_content(ui, pane);
        self.history.context = before;
    }

    fn pane_content(&mut self, ui: &mut egui::Ui, pane: Pane) {
        if !matches!(pane, Pane::Canvas | Pane::Montage | Pane::Timeline | Pane::XSheet) {
            fit_to_pane(ui);
        }
        match pane {
            Pane::Canvas => {
                ui.painter().rect_filled(ui.max_rect(), 0.0, crate::theme::FOND);
                if self.mode == Mode::Storyboard && self.board_view == BoardView::Panels {
                    self.board_view(ui);
                } else {
                    self.canvas_panel(ui);
                }
                // Le dessin a la main (aucune bande) : un cadre discret.
                let drawing = !self.panels_have_focus() && !self.images_focus && !self.key_focus;
                self.focus_mark(ui, drawing, false);
            }
            Pane::Tools => self.tool_bar(ui),
            Pane::Color => self.color_panel(ui),
            Pane::Brushes => self.brushes_panel(ui),
            Pane::ToolOptions => self.tool_options_panel(ui),
            Pane::View => self.view_panel(ui),
            Pane::Guides => self.guides_ui(ui),
            Pane::Sheet => self.panel_inspector(ui),
            Pane::Layers => self.layers_panel(ui),
            // Une seule bande : le montage en storyboard, la timeline en animation. Quand elle a la
            // main (copier, coller, supprimer visent ses plans, ses clés ou ses images), un cadre de
            // la couleur d'accent.
            Pane::Montage | Pane::Timeline => match self.mode {
                Mode::Storyboard => {
                    self.montage_panel(ui);
                    self.focus_mark(ui, self.panels_have_focus() || self.key_focus, true);
                }
                Mode::Animation => {
                    self.timeline_panel(ui);
                    self.focus_mark(ui, self.images_focus || self.key_focus, true);
                }
            },
            Pane::XSheet => self.xsheet_panel(ui),
            Pane::Effects => self.fx_panel(ui),
            Pane::History => self.history_panel(ui),
        }
    }

    /// Le cadre du panneau qui a la main (chantier 6 ; plus visible depuis le 2026-10-18, axe
    /// d'ergonomie 5) : les raccourcis, copier, coller et supprimer le visent. `strong` : celui
    /// d'une bande (montage, timeline) ; sinon celui, discret, du dessin.
    fn focus_mark(&self, ui: &egui::Ui, on: bool, strong: bool) {
        if on {
            let r = ui.max_rect();
            let (width, color) = if strong { (2.0, crate::theme::ACCENT) } else { (1.0, crate::theme::ACCENT.gamma_multiply(0.55)) };
            ui.painter().with_clip_rect(r).rect_stroke(r.shrink(width * 0.5), 0.0, egui::Stroke::new(width, color), egui::StrokeKind::Inside);
        }
    }

    /// Les panneaux, sous la barre de menus et au-dessus de la barre d'état ; puis ceux qui ont
    /// leur propre fenêtre.
    pub(crate) fn show_dock(&mut self, ctx: &egui::Context) {
        let mut state = self.dock.take().unwrap_or_else(|| dock::to_state(&self.layout.dock_layout()));
        // La bande suit l'image courante ; ce que l'on fait pendant l'affichage se suit à l'image
        // d'écran d'après.
        self.following = std::mem::take(&mut self.follow_frame);
        let style = crate::theme::dock(egui_dock::Style::from_egui(ctx.style().as_ref()));
        let sep = style.separator.width;
        let m = style.tab.tab_body.inner_margin;
        let body_margin = (m.top + m.bottom) as f32;
        egui_dock::DockArea::new(&mut state)
            .id(Id::new("panneaux"))
            .style(style)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(ctx, &mut Panes { app: self });
        // (Un panneau peut avoir remplacé la disposition pendant l'affichage : on la garde.)
        if self.dock.is_none() {
            self.fit_panes(ctx, &mut state, sep, body_margin);
            self.dock = Some(state);
        }
        if let Some(pane) = self.detach_request.take() {
            let mut layout = self.dock_layout();
            layout.detach(pane);
            self.set_dock_layout(layout);
        }
        self.show_detached(ctx);
        self.following = false;
    }

    /// Chaque panneau détaché dans sa fenêtre Windows. La fermer remet le panneau à sa place.
    fn show_detached(&mut self, ctx: &egui::Context) {
        let mut back = Vec::new();
        for i in 0..self.detached.len() {
            let d = self.detached[i].clone();
            let title = format!("Pelure — {}", d.panneau.name());
            let mut builder = egui::ViewportBuilder::default().with_title(title).with_min_inner_size([180.0, 120.0]);
            if d.largeur > 0.0 && d.hauteur > 0.0 {
                builder = builder.with_inner_size([d.largeur, d.hauteur]).with_position([d.x, d.y]);
            } else {
                builder = builder.with_inner_size([300.0, 420.0]);
            }
            let id = egui::ViewportId::from_hash_of(("panneau détaché", d.panneau));
            let pane = d.panneau;
            let (closed, rect) = ctx.show_viewport_immediate(id, builder, |ctx, _class| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if matches!(pane, Pane::Montage | Pane::Timeline | Pane::XSheet | Pane::History) {
                        self.pane_ui(ui, pane);
                    } else {
                        egui::ScrollArea::vertical().show(ui, |ui| self.pane_ui(ui, pane));
                    }
                });
                ctx.input(|i| {
                    let v = i.viewport();
                    let rect = match (v.outer_rect, v.inner_rect) {
                        (Some(o), Some(inner)) => Some((o.min, inner.size())),
                        _ => None,
                    };
                    (v.close_requested(), rect)
                })
            });
            if let Some((pos, size)) = rect {
                let d = &mut self.detached[i];
                (d.x, d.y, d.largeur, d.hauteur) = (pos.x, pos.y, size.x, size.y);
            }
            if closed {
                back.push(pane);
            }
        }
        if !back.is_empty() {
            let mut layout = self.dock_layout();
            for pane in back {
                layout.detachees.retain(|d| d.panneau != pane);
                layout.open(pane);
            }
            self.set_dock_layout(layout);
        }
    }

    /// La disposition des panneaux à l'écran.
    pub(crate) fn dock_layout(&self) -> DockLayout {
        let Some(state) = &self.dock else { return self.layout.dock_layout() };
        // La mémoire d'egui donne la fenêtre cadre compris ; on retient la taille de son contenu,
        // celle que l'on redonne à l'ouverture (sinon elle grandirait à chaque fois).
        let style = self.ctx.style();
        let (m, stroke) = (style.spacing.window_margin, style.visuals.window_stroke.width);
        let frame = egui::vec2(
            (m.left + m.right) as f32 + 2.0 * stroke,
            (m.top + m.bottom) as f32 + 2.0 * stroke,
        );
        let mut layout = dock::from_state(state, &|i| {
            self.ctx
                .memory(|mem| mem.area_rect(dock::window_id(i)))
                .map(|r| egui::Rect::from_min_size(r.min, (r.size() - frame).max(egui::vec2(20.0, 20.0))))
        });
        layout.detachees = self.detached.clone();
        layout.sanitized()
    }

    pub(crate) fn set_dock_layout(&mut self, layout: DockLayout) {
        let mut layout = layout.sanitized();
        // La feuille d'exposition n'existe qu'en animation : en storyboard, une disposition qui
        // l'ouvre (espace de travail, démarrage) la garde de côté pour le retour.
        if self.mode == Mode::Storyboard {
            if let Some(sans) = layout.sans_animation_seule() {
                self.exposition_masquee = Some(std::mem::replace(&mut layout, sans));
            }
        }
        self.detached = layout.detachees.clone();
        self.dock = Some(dock::to_state(&layout));
        // Nouvelle disposition : ses parts ne sont pas des bords tirés à la main.
        self.fit_set.clear();
    }

    /// Au changement de mode : la feuille d'exposition (animation seule) quitte l'espace de travail
    /// en storyboard et y revient, où elle était, en animation.
    pub(crate) fn masquer_exposition(&mut self) {
        let layout = self.dock_layout();
        if self.mode == Mode::Animation {
            if let Some(saved) = self.exposition_masquee.take() {
                let back = layout.retour_animation(&saved);
                self.set_dock_layout(back);
            }
        } else if layout.sans_animation_seule().is_some() {
            self.set_dock_layout(layout);
        }
    }

    /// Un panneau d'animation seule se refuse en storyboard (action, menu) : vrai, avec un mot dans
    /// la barre d'état, s'il faut s'arrêter là.
    pub(crate) fn refuse_hors_animation(&mut self, pane: Pane) -> bool {
        if pane.animation_only() && self.mode == Mode::Storyboard {
            self.status = format!("{} : seulement en animation (double-clic sur un plan, ou Tab).", pane.name());
            return true;
        }
        false
    }

    /// Affiche ou masque un panneau (menu Affichage > Panneaux).
    pub(crate) fn toggle_pane(&mut self, pane: Pane) {
        if self.refuse_hors_animation(pane) {
            return;
        }
        let mut layout = self.dock_layout();
        let opening = !layout.contains(pane);
        if opening {
            layout.open(pane);
        } else {
            layout.close(pane);
        }
        self.set_dock_layout(layout);
        // Un panneau qu'on ouvre passe devant les onglets de son groupe.
        if let (true, Some(state)) = (opening, self.dock.as_mut()) {
            if let Some(at) = state.find_tab(&pane) {
                state.set_active_tab(at);
            }
        }
    }

    /// Montre `pane` : ouvert s'il ne l'était pas, son onglet devant.
    pub(crate) fn show_pane(&mut self, pane: Pane) {
        if self.refuse_hors_animation(pane) {
            return;
        }
        if !self.pane_shown(pane) {
            self.toggle_pane(pane);
        } else if let Some(state) = self.dock.as_mut() {
            if let Some(at) = state.find_tab(&pane) {
                state.set_active_tab(at);
            }
        }
    }

    /// `pane` est-il dans l'espace de travail (affiché ou en onglet) ?
    pub(crate) fn pane_shown(&self, pane: Pane) -> bool {
        self.dock.as_ref().map_or_else(|| self.layout.dock_layout().contains(pane), |s| s.find_tab(&pane).is_some()) || self.detached.iter().any(|d| d.panneau == pane)
    }


    /// Affiche ou masque un groupe de panneaux (actions de 4b : outils, fiche, bande).
    pub(crate) fn toggle_pane_group(&mut self, group: Group) {
        let mut layout = self.dock_layout();
        layout.toggle_group(group);
        self.set_dock_layout(layout);
    }

    /// Panneaux dans leur propre fenêtre (pour les menus).
    pub(crate) fn detached_panes(&self) -> Vec<DetachedPanel> {
        self.detached.clone()
    }
}

/// Un panneau de côté rétréci reste utilisable (Anthony, 2026-10-17 : « ces fenêtres ne sont pas
/// optimisées pour rester lisibles et accessibles lorsqu'on réduit la taille ») : les curseurs et
/// les listes déroulantes prennent la largeur du panneau au lieu d'une largeur fixe.
pub(crate) fn fit_to_pane(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let s = ui.spacing_mut();
    // Un curseur, sa valeur et son nom tiennent dans la largeur.
    s.slider_width = (w - 120.0).clamp(36.0, 160.0);
    s.combo_width = (w * 0.55).clamp(60.0, 160.0);
}

/// Largeur d'une liste ou d'un champ : la place qui reste sur sa ligne, `max` au plus (au moins
/// 48 points), pour qu'un panneau rétréci ne le coupe pas.
pub(crate) fn fit_w(ui: &egui::Ui, max: f32) -> f32 {
    (ui.available_width() - 4.0).clamp(48.0, max)
}
