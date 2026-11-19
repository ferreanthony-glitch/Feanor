//! Plage de lecture du film : la lecture du montage reste entre deux images du film, qui peuvent être dans des plans
//! différents (de l'image 12 du plan 3 à l'image 5 du plan 7), en boucle ou non. Les points d'entrée et de sortie d'un plan
//! (`cues.rs`, 13d) bornent la lecture du plan seul ; la plage, elle, borne la lecture du film entier.
//!
//! Chaque bout est accroché à un plan (son identifiant) et à une image de ce plan, pas à un numéro d'image du film : allonger,
//! raccourcir, ajouter ou supprimer un plan avant la plage la déplace avec son contenu, comme les signets. Un bout dont le plan
//! a disparu ne compte plus ; un bout au-delà de la fin de son plan (le plan a été raccourci) se pose sur sa dernière image.
//!
//! La plage est un réglage de la séance, comme la lecture en boucle : elle n'est ni dans le projet ni dans l'annulation.
//!
//! Commandes : « Début de la plage ici », « Fin de la plage ici », « Plage = plans choisis », « Enlever la plage » (clic droit
//! sur la règle du montage, menu Storyboard, palette). Sur la règle, le film hors de la plage est assombri, entre deux crochets.

use crate::app::App;
use crate::document::Document;

/// Un bout de la plage : une image d'un plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ancre {
    /// Identifiant du plan (`Panel::id`).
    pub plan: u64,
    /// Image dans ce plan, comptée depuis 0.
    pub image: usize,
}

impl Ancre {
    /// L'ancre de l'image `global` du film.
    pub(crate) fn de_global(doc: &Document, global: usize) -> Option<Ancre> {
        if doc.panels.is_empty() {
            return None;
        }
        let (p, f) = doc.locate(global.min(doc.total_frames().saturating_sub(1)));
        Some(Ancre { plan: doc.panels.get(p)?.id, image: f })
    }

    /// L'image du film où l'ancre tombe aujourd'hui ; rien si son plan n'existe plus.
    pub(crate) fn globale(&self, doc: &Document) -> Option<usize> {
        let i = doc.panels.iter().position(|p| p.id == self.plan)?;
        let len = doc.panels[i].frame_count().max(1);
        Some(doc.panel_start(i) + self.image.min(len - 1))
    }
}

/// La plage de lecture du film. Les deux bouts sont facultatifs : sans début, elle part du début du film ; sans fin, elle va
/// jusqu'au bout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PlageFilm {
    pub debut: Option<Ancre>,
    /// Dernière image lue (comprise).
    pub fin: Option<Ancre>,
}

impl PlageFilm {
    /// Une plage posée (au moins un bout).
    pub(crate) fn posee(&self) -> bool {
        self.debut.is_some() || self.fin.is_some()
    }

    /// Les images [début, fin) du film que la plage laisse lire, si elle borne vraiment quelque chose : rien sans plage, sans
    /// film, ou si ses bouts se croisent (le plan du début a été déplacé après celui de la fin), comme pour les points d'un plan.
    pub(crate) fn bornes(&self, doc: &Document) -> Option<(usize, usize)> {
        let total = doc.total_frames();
        if !self.posee() || total == 0 {
            return None;
        }
        let a = self.debut.and_then(|d| d.globale(doc)).unwrap_or(0);
        let b = self.fin.and_then(|f| f.globale(doc)).map_or(total, |f| f + 1);
        (b > a && (a, b) != (0, total)).then_some((a, b))
    }
}

/// Les images [début, fin) que lit la lecture du film : la plage, ou tout le film.
pub(crate) fn lecture_du_film(plage: &PlageFilm, doc: &Document) -> (usize, usize) {
    plage.bornes(doc).unwrap_or((0, doc.total_frames().max(1)))
}

impl App {
    /// Les images [début, fin) du film que lit la lecture du montage.
    pub(crate) fn film_play_range(&self) -> (usize, usize) {
        lecture_du_film(&self.plage_film, &self.doc)
    }

    /// Pose la tête sur l'image `global` du film sans arrêter la lecture (`goto_global` l'arrête).
    pub(crate) fn place_global(&mut self, global: usize) {
        let (p, f) = self.doc.locate(global.min(self.doc.total_frames().saturating_sub(1)));
        if p != self.cur_panel {
            self.set_panel(p);
        }
        self.cur_frame = f;
    }

    /// Début (`fin` : fin) de la plage à l'image courante du film. Un autre bout qui tomberait avant le début (après la fin)
    /// est enlevé : la plage va alors jusqu'au bout du film (part de son début).
    pub(crate) fn plage_ici(&mut self, fin: bool) {
        let g = self.global_frame();
        let Some(ancre) = Ancre::de_global(&self.doc, g) else { return };
        let autre = if fin { self.plage_film.debut } else { self.plage_film.fin };
        let croise = autre.and_then(|a| a.globale(&self.doc)).is_some_and(|a| if fin { a > g } else { a < g });
        let label = format!("l'image {} du film", g + 1);
        if fin {
            self.plage_film.fin = Some(ancre);
            if croise {
                self.plage_film.debut = None;
            }
        } else {
            self.plage_film.debut = Some(ancre);
            if croise {
                self.plage_film.fin = None;
            }
        }
        let quoi = if fin { "Fin" } else { "Début" };
        let enleve = match (croise, fin) {
            (true, true) => " (le début, qui était après, est enlevé)",
            (true, false) => " (la fin, qui était avant, est enlevée)",
            _ => "",
        };
        self.status = format!("{quoi} de la plage de lecture du film à {label}{enleve}.");
    }

    /// La plage couvre les plans choisis, du début du premier à la fin du dernier.
    pub(crate) fn plage_plans_choisis(&mut self) {
        let ids = self.board_selection();
        let indices: Vec<usize> = self.doc.panels.iter().enumerate().filter(|(_, p)| ids.contains(&p.id)).map(|(i, _)| i).collect();
        let (Some(&premier), Some(&dernier)) = (indices.first(), indices.last()) else {
            self.status = "Choisis d'abord des plans (Maj + clic dans le montage).".into();
            return;
        };
        let fin = self.doc.panels[dernier].frame_count().max(1) - 1;
        self.plage_film = PlageFilm {
            debut: Some(Ancre { plan: self.doc.panels[premier].id, image: 0 }),
            fin: Some(Ancre { plan: self.doc.panels[dernier].id, image: fin }),
        };
        let n = dernier - premier + 1;
        self.status = if n == 1 { "La lecture du film est limitée au plan choisi.".into() } else { format!("La lecture du film est limitée à {n} plans.") };
    }

    /// Enlève la plage : la lecture du montage reprend tout le film.
    pub(crate) fn enlever_plage(&mut self) {
        self.plage_film = PlageFilm::default();
        self.status = "Plage de lecture du film enlevée : la lecture prend tout le film.".into();
    }

    /// Les entrées de la plage dans un menu du montage (clic droit sur la règle, qui y a posé la tête de lecture).
    pub(crate) fn plage_menu_items(&mut self, ui: &mut eframe::egui::Ui) {
        self.action_item(ui, "Début de la plage de lecture ici", "plage.debut");
        self.action_item(ui, "Fin de la plage de lecture ici", "plage.fin");
        let n = self.board_selection().len();
        let choisis = if n > 1 { format!("Plage de lecture : les {n} plans choisis") } else { "Plage de lecture : ce plan".to_string() };
        self.action_item(ui, &choisis, "plage.plans_choisis");
        if self.plage_film.posee() {
            self.action_item(ui, "Enlever la plage de lecture", "plage.enlever");
        }
    }

    /// La plage sur la règle du montage : le film hors de la plage assombri, un crochet à chaque bout. `x_of` : l'abscisse d'une
    /// image du film ; `regle` : la bande de la règle ; `bas` : où s'arrête le voile (le bas de la bande des plans).
    pub(crate) fn peindre_plage(&self, painter: &eframe::egui::Painter, regle: eframe::egui::Rect, bas: f32, x_of: &dyn Fn(usize) -> f32) {
        use eframe::egui::{pos2, Color32, Rect, Stroke};
        let Some((a, b)) = self.plage_film.bornes(&self.doc) else { return };
        let total = self.doc.total_frames();
        let voile = Color32::from_black_alpha(80);
        let (xa, xb) = (x_of(a), x_of(b));
        if a > 0 {
            painter.rect_filled(Rect::from_min_max(pos2(x_of(0), regle.top()), pos2(xa, bas)), 0.0, voile);
        }
        if b < total {
            painter.rect_filled(Rect::from_min_max(pos2(xb, regle.top()), pos2(x_of(total), bas)), 0.0, voile);
        }
        // (Les crochets, comme ceux des points d'entrée et de sortie du plan dans la timeline.)
        let trait_ = Stroke::new(2.0_f32, crate::theme::image_courante());
        let (haut, bas_regle) = (regle.top() + 2.0, regle.bottom() - 2.0);
        for (x, sens) in [(xa + 1.0, 1.0_f32), (xb - 1.0, -1.0_f32)] {
            painter.line_segment([pos2(x, haut), pos2(x, bas_regle)], trait_);
            painter.line_segment([pos2(x, haut), pos2(x + 5.0 * sens, haut)], trait_);
            painter.line_segment([pos2(x, bas_regle), pos2(x + 5.0 * sens, bas_regle)], trait_);
        }
    }
}
