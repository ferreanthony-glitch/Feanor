//! Fenêtres modulables dans l'application (5b) : contenu de chaque panneau, affichage par
//! egui_dock, ouverture et fermeture des panneaux ; panneaux dans leur propre fenêtre Windows
//! (5g, par exemple sur un deuxième écran).

use eframe::egui::{self, Color32, Id, WidgetText};

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
        self.app.pane_ui(ui, *tab);
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
            Pane::Canvas | Pane::Strip => [false, false],
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
            Pane::Strip if self.mode == Mode::Storyboard => "Montage",
            Pane::Strip => "Timeline",
            Pane::Canvas if self.mode == Mode::Storyboard && self.board_view == BoardView::Panels => "Vignettes",
            p => p.tab_name(),
        }
    }

    /// Le contenu d'un panneau (dans un onglet ou dans sa propre fenêtre).
    pub(crate) fn pane_ui(&mut self, ui: &mut egui::Ui, pane: Pane) {
        match pane {
            Pane::Canvas => {
                ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::from_gray(42));
                if self.mode == Mode::Storyboard && self.board_view == BoardView::Panels {
                    self.board_view(ui);
                } else {
                    self.canvas_panel(ui);
                }
            }
            Pane::Tools => self.tool_bar(ui),
            Pane::Color => self.color_panel(ui),
            Pane::ToolOptions => self.tool_options_panel(ui),
            Pane::View => self.view_panel(ui),
            Pane::Guides => self.guides_ui(ui),
            Pane::Sheet => self.panel_inspector(ui),
            Pane::Layers => self.layers_panel(ui),
            Pane::Strip => match self.mode {
                Mode::Storyboard => self.montage_panel(ui),
                Mode::Animation => self.timeline_panel(ui),
            },
        }
    }

    /// Les panneaux, sous la barre de menus et au-dessus de la barre d'état ; puis ceux qui ont
    /// leur propre fenêtre.
    pub(crate) fn show_dock(&mut self, ctx: &egui::Context) {
        let mut state = self.dock.take().unwrap_or_else(|| dock::to_state(&self.layout.dock_layout()));
        let style = egui_dock::Style::from_egui(ctx.style().as_ref());
        egui_dock::DockArea::new(&mut state)
            .id(Id::new("panneaux"))
            .style(style)
            .show_leaf_collapse_buttons(false)
            .show_leaf_close_all_buttons(false)
            .show(ctx, &mut Panes { app: self });
        // (Un panneau peut avoir remplacé la disposition pendant l'affichage : on la garde.)
        if self.dock.is_none() {
            self.dock = Some(state);
        }
        if let Some(pane) = self.detach_request.take() {
            let mut layout = self.dock_layout();
            layout.detach(pane);
            self.set_dock_layout(layout);
        }
        self.show_detached(ctx);
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
                    if matches!(pane, Pane::Strip) {
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
        let layout = layout.sanitized();
        self.detached = layout.detachees.clone();
        self.dock = Some(dock::to_state(&layout));
    }

    /// Affiche ou masque un panneau (menu Affichage > Panneaux).
    pub(crate) fn toggle_pane(&mut self, pane: Pane) {
        let mut layout = self.dock_layout();
        if layout.contains(pane) {
            layout.close(pane);
        } else {
            layout.open(pane);
        }
        self.set_dock_layout(layout);
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
