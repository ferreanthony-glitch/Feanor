//! Fenêtres modulables dans l'application (5b) : contenu de chaque panneau, affichage par
//! egui_dock, ouverture et fermeture des panneaux.

use eframe::egui::{self, Color32, Id, WidgetText};

use crate::app::{App, BoardView, Mode};
use crate::dock::{self, DockLayout, Group, Pane};

/// Ce qu'egui_dock affiche dans chaque onglet.
struct Panes<'a> {
    app: &'a mut App,
}

impl egui_dock::TabViewer for Panes<'_> {
    type Tab = Pane;

    fn title(&mut self, tab: &mut Pane) -> WidgetText {
        format!("{}  {}", tab.icon(), self.app.pane_title(*tab)).into()
    }

    fn id(&mut self, tab: &mut Pane) -> Id {
        Id::new(("panneau", *tab))
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Pane) {
        let app = &mut *self.app;
        match tab {
            Pane::Canvas => {
                ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::from_gray(42));
                if app.mode == Mode::Storyboard && app.board_view == BoardView::Panels {
                    app.panels_view(ui);
                } else {
                    app.canvas_panel(ui);
                }
            }
            Pane::Tools => app.tool_bar(ui),
            Pane::Color => app.color_panel(ui),
            Pane::ToolOptions => app.tool_options_panel(ui),
            Pane::View => app.view_panel(ui),
            Pane::Guides => app.guides_ui(ui),
            Pane::Sheet => app.panel_inspector(ui),
            Pane::Layers => app.layers_panel(ui),
            Pane::Strip => match app.mode {
                Mode::Storyboard => app.montage_panel(ui),
                Mode::Animation => app.timeline_panel(ui),
            },
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

    /// Les panneaux, sous la barre de menus et au-dessus de la barre d'état.
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
        dock::from_state(state, &|i| {
            self.ctx
                .memory(|mem| mem.area_rect(dock::window_id(i)))
                .map(|r| egui::Rect::from_min_size(r.min, (r.size() - frame).max(egui::vec2(20.0, 20.0))))
        })
    }

    pub(crate) fn set_dock_layout(&mut self, layout: DockLayout) {
        self.dock = Some(dock::to_state(&layout.sanitized()));
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
}
