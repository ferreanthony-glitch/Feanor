//! Partie 201 (`docs/INTERFACE.md` partie 201) : **qui a la main dans la vue 3D**, le décor ou les cartes. Après les parties 184 et
//! 190, Anthony ne savait pas si un geste allait déplacer une carte ou un objet du décor. Discret et clair :
//!
//! - **une étiquette dans le coin haut gauche de la vue** : ce que les poignées et un glisser déplacent (« Décor : Tree oak »,
//!   « Carte : Perso 1 », « Lampe : Lampe 1 », « Caméra du plan »), à la couleur d'identification de l'objet s'il en a une (partie
//!   199), sinon à celle de sa sorte (le gris bleuté du décor, l'encre de la carte, la teinte de la lampe, la couleur des caméras) ;
//! - **ce qu'un clic prendrait**, sous elle, en plus pâle, quand le pointeur est sur autre chose que ce qui est choisi (« Un clic
//!   prend : Carte : Fond ») ;
//! - **le carré central du manipulateur** à la même couleur.
//!
//! Dans l'onglet 3D et dans les vues Espace 3D du Storyboard (plein écran et à côté), chaque vue (`e3_peindre_qui_a_la_main`).

use eframe::egui::{self, pos2, Align2, Color32, FontId, Pos2, Rect, Stroke};

use crate::app::App;
use crate::espace3dgizmo::CAMERA_ID;
use crate::espace3dmath::View;

/// La couleur d'un objet du décor sans couleur d'identification.
pub const DECOR: Color32 = Color32::from_rgb(150, 170, 200);

// Ce que la dernière vue a écrit (essais) : l'étiquette, et ce qu'un clic prendrait.
#[cfg(test)]
thread_local! {
    pub(crate) static ECRIT: std::cell::RefCell<(Option<String>, Option<String>)> = const { std::cell::RefCell::new((None, None)) };
}

impl App {
    /// Le nom et la couleur de ce que désigne le numéro de sélection `s` (un objet du décor, une lampe, une carte, la caméra).
    pub(crate) fn e3_nom_de_la_selection(&self, s: u64) -> Option<(String, Color32)> {
        if s == CAMERA_ID {
            return Some(("Caméra du plan".into(), crate::theme::camera()));
        }
        if let Some(id) = crate::poignees3d::objet_of(s) {
            let o = self.e3_objet(id)?;
            let c = o.couleur.and_then(crate::couleurs3d::couleur).map_or(DECOR, crate::couleurs3d::color32);
            return Some((format!("Décor : {}", o.nom), c));
        }
        if let Some(id) = crate::miseenscene3dui::lampe_of(s) {
            let l = self.o3_lampe(id)?;
            return Some((format!("Lampe : {}", l.nom), crate::couleurs3d::color32(l.couleur)));
        }
        let li = self.e3_card_layer(s)?;
        let l = &self.layers()[li];
        Some((format!("Carte : {}", l.name), crate::layerheads::encre_de_sorte(l)))
    }

    /// **Qui a la main** : ce que les poignées et un glisser déplacent (le premier de ce qui est choisi ; « et N autres »), et sa
    /// couleur. Rien : rien n'est choisi.
    pub(crate) fn e3_qui_a_la_main(&self) -> Option<(String, Color32)> {
        let sel = &self.espace3d.sel;
        // (Le décor d'abord : c'est lui qu'on confondait avec les cartes.)
        let premier = sel.iter().copied().find(|&s| crate::poignees3d::objet_of(s).is_some()).or_else(|| sel.first().copied())?;
        let (nom, c) = self.e3_nom_de_la_selection(premier)?;
        let autres = sel.iter().filter(|&&s| s != premier && self.e3_nom_de_la_selection(s).is_some()).count();
        Some((if autres > 0 { format!("{nom} et {autres} autre{}", if autres > 1 { "s" } else { "" }) } else { nom }, c))
    }

    /// Peint l'étiquette de la vue de rectangle `r` (prise de vue `view`, image dans `draw`) : qui a la main, et, le pointeur sur
    /// autre chose, ce qu'un clic prendrait. Rien pendant un geste.
    pub(crate) fn e3_peindre_qui_a_la_main(&mut self, ctx: &egui::Context, painter: &egui::Painter, r: Rect, draw: Rect, view: &View, coin: Pos2) {
        let main = self.e3_qui_a_la_main();
        let sous = if self.espace3d.drag.is_none() {
            ctx.pointer_hover_pos().filter(|p| draw.contains(*p) && r.contains(*p)).and_then(|p| {
                // (Le rayon contre les triangles du décor, seulement quand le pointeur a bougé : gardé pour la vue, `ctx.data`.)
                let cle = egui::Id::new(("qui_a_la_main", r.min.x.to_bits(), r.min.y.to_bits()));
                if let Some((q, s)) = ctx.data(|d| d.get_temp::<(Pos2, Option<u64>)>(cle)) {
                    if q == p {
                        return s;
                    }
                }
                let center = [draw.width() / 2.0, draw.height() / 2.0];
                let ray = view.ray([p.x - draw.left(), p.y - draw.top()], center);
                let s = self.e3_pick_tout(&ray);
                ctx.data_mut(|d| d.insert_temp(cle, (p, s)));
                s
            })
        } else {
            None
        };
        let sous = sous.filter(|s| !self.espace3d.sel.contains(s)).and_then(|s| self.e3_nom_de_la_selection(s));
        #[cfg(test)]
        ECRIT.with(|e| *e.borrow_mut() = (main.as_ref().map(|m| m.0.clone()), sous.as_ref().map(|s| s.0.clone())));
        let mut y = coin.y;
        if let Some((nom, c)) = &main {
            let g = painter.layout_no_wrap(nom.clone(), FontId::proportional(12.0), *c);
            let fond = Rect::from_min_size(pos2(coin.x, y), g.size() + egui::vec2(16.0, 6.0));
            painter.rect_filled(fond, 4.0, Color32::from_black_alpha(150));
            painter.circle_filled(pos2(fond.left() + 6.0, fond.center().y), 3.0, *c);
            painter.galley(pos2(fond.left() + 12.0, fond.top() + 3.0), g, *c);
            painter.rect_stroke(fond, 4.0, Stroke::new(1.0_f32, c.gamma_multiply(0.5)), egui::StrokeKind::Inside);
            y = fond.bottom() + 3.0;
        }
        if let Some((nom, c)) = &sous {
            painter.text(pos2(coin.x + 2.0, y), Align2::LEFT_TOP, format!("Un clic prend : {nom}"), FontId::proportional(11.0), c.gamma_multiply(0.75));
        }
    }

    /// La couleur du carré central du manipulateur : celle de qui a la main (rien : le gris d'avant).
    pub(crate) fn e3_couleur_de_la_main(&self) -> Option<Color32> {
        self.e3_qui_a_la_main().map(|m| m.1)
    }
}
