//! Le zoom du montage et de la timeline reste calé sur la tête de lecture (Anthony, 2026-10-19 :
//! « Le zoom sur la timeline doit zoomer sur la zone de la tête de lecture »).
//!
//! Tout geste de zoom de la bande du bas garde la tête de lecture au même endroit de l'écran et
//! fait défiler la bande en conséquence : le curseur du bas, les boutons + et −, Ctrl + molette, les
//! raccourcis (Alt+1, Alt+2). Si la tête est hors de la vue, on la ramène d'abord au milieu, puis
//! on zoome autour d'elle.
//!
//! Ctrl + molette zoome autour de la tête de lecture, pas de la souris (choix demandé) : on
//! garde ainsi toujours sous les yeux l'image que l'on regarde. Rien ne s'en trouve gêné, la
//! bande se déplace toujours à la molette sans Ctrl.
//!
//! Comment : un geste de zoom change l'échelle, puis note à quelle distance du bord gauche de la
//! vue la tête doit rester (`view_x_to_keep`). À l'image d'écran suivante, la bande a sa nouvelle
//! largeur : on mesure où la tête est tombée et on fait défiler de la différence (`scroll_delta`).

use eframe::egui::{vec2, Ui};

use crate::app::App;

/// Les deux bandes qui zooment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Strip {
    /// Le montage (storyboard).
    Montage,
    /// La timeline (animation).
    Timeline,
}

impl Strip {
    pub(crate) fn index(self) -> usize {
        match self {
            Strip::Montage => 0,
            Strip::Timeline => 1,
        }
    }
}

/// Ce qu'on sait de la bande après son dernier affichage : où est la tête de lecture dans la vue
/// (distance du bord gauche de la vue, hors de la vue si elle est négative ou plus grande que
/// `view_w`) et la largeur de la vue.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct StripGeo {
    pub head_view_x: f32,
    pub view_w: f32,
}

/// Une demande de défilement après un zoom : où la tête doit rester dans la vue, et l'image
/// d'écran où elle a été faite (une demande trop vieille, d'une bande cachée, est ignorée).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Anchor {
    pub view_x: f32,
    pub frame_nr: u64,
}

/// Images d'écran pendant lesquelles une demande de défilement reste valable.
pub(crate) const ANCHOR_LIFE: u64 = 4;

/// La distance du bord gauche de la vue où la tête de lecture doit rester : où elle est si elle est
/// dans la vue, sinon le milieu de la vue (on la ramène avant de zoomer autour d'elle).
pub(crate) fn view_x_to_keep(head_view_x: f32, view_w: f32) -> f32 {
    if (0.0..=view_w).contains(&head_view_x) {
        head_view_x
    } else {
        view_w / 2.0
    }
}

/// Le défilement du contenu à demander (positif : le contenu va vers la droite) pour que la tête de
/// lecture, à `head_x` (abscisse de la bande), tombe à `view_x` du bord gauche `view_left` de la vue.
pub(crate) fn scroll_delta(head_x: f32, view_left: f32, view_x: f32) -> f32 {
    view_left + view_x - head_x
}

/// L'abscisse de la tête de lecture dans la bande : `origin` (abscisse de l'image 0), `scale`
/// (largeur d'une image), `pos` (position de la tête en images).
pub(crate) fn head_x(origin: f32, scale: f32, pos: f32) -> f32 {
    origin + pos * scale
}

impl App {
    /// À appeler quand l'échelle de la bande change (curseur, boutons, molette, raccourci) : la
    /// tête de lecture y reste, au défilement de l'image d'écran suivante.
    pub(crate) fn anchor_strip_zoom(&mut self, strip: Strip) {
        let i = strip.index();
        let now = self.ctx.cumulative_pass_nr();
        // Un zoom en cours (curseur tiré) garde la place déjà demandée.
        if self.strip_anchor[i].is_some_and(|a| now.saturating_sub(a.frame_nr) <= ANCHOR_LIFE) {
            self.strip_anchor[i] = self.strip_anchor[i].map(|a| Anchor { frame_nr: now, ..a });
            return;
        }
        let g = self.strip_geo[i];
        self.strip_anchor[i] = Some(Anchor { view_x: view_x_to_keep(g.head_view_x, g.view_w), frame_nr: now });
    }

    /// Pendant l'affichage de la bande, avec sa nouvelle échelle : fait défiler pour garder la tête
    /// de lecture à sa place, puis retient où elle est. `origin`, `scale`, `pos` : voir `head_x`.
    pub(crate) fn follow_strip_anchor(&mut self, ui: &mut Ui, strip: Strip, origin: f32, scale: f32, pos: f32) {
        let i = strip.index();
        let view = ui.clip_rect();
        let x = head_x(origin, scale, pos);
        let now = self.ctx.cumulative_pass_nr();
        let mut head_view_x = x - view.left();
        if let Some(a) = self.strip_anchor[i].take() {
            if now.saturating_sub(a.frame_nr) <= ANCHOR_LIFE {
                let d = scroll_delta(x, view.left(), a.view_x);
                if d.abs() > 0.5 {
                    ui.scroll_with_delta(vec2(d, 0.0));
                    ui.ctx().request_repaint();
                }
                // Là où la tête sera à l'image d'écran suivante.
                head_view_x = a.view_x;
            }
        }
        self.strip_geo[i] = StripGeo { head_view_x, view_w: view.width() };
    }
}
