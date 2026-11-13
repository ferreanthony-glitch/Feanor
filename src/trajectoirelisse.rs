//! Le curseur « Lissage » de l'outil Trajectoire (`docs/INTERFACE.md`, partie 151). Anthony, 2026-11-12 : « de façon
//! générale la courbe tracée pour une trajectoire doit pouvoir être lissée dans les réglages d'outils ».
//!
//! **Où.** Dans les réglages de l'outil (panneau Réglages), sur la feuille comme dans la vue Espace 3D.
//!
//! **Sur quoi.** Sur les trajectoires de ce qui est choisi (la règle d'Anthony « la sélection agit sur tout ») : sur la
//! feuille, les calques choisis qui suivent une trajectoire tracée sur la feuille ; dans l'Espace 3D, les cartes choisies et
//! la caméra. Sa valeur est aussi celle des prochains tracés (`Etat::lissage`).
//!
//! **Non destructif.** Chaque trajectoire garde son chemin brut (`Trajectoire::brut`) ; le chemin montré et suivi est ce
//! chemin lissé (`Trajectoire::lisser_a`, moyenne glissante, les deux bouts fixes). Le curseur se remonte ou se redescend à
//! tout moment ; 0 rend le trait tel qu'il a été tracé. Les marques gardent leur place relative le long du chemin ; les
//! clés sont reposées comme quand on tire le chemin : à chaque image d'écran en clés d'interpolation et pour les cartes de
//! l'Espace 3D, au lâcher du curseur pour les dessins des clés d'images (aucun dessin n'est refait pendant le geste).
//!
//! **Une étape d'annulation** par geste sur le curseur (`App::edited`).

use eframe::egui;

use crate::app::App;
use crate::trajectoire::{Cles, Trajectoire};
use crate::trajectoire3d::Sujet;

/// Le nom du réglage, et de l'étape d'annulation.
pub(crate) const LISSAGE: &str = "Lissage";
const NOM_ETAPE: &str = "Lisser la trajectoire";
const AIDE: &str = "Adoucit la courbe du chemin, de 0 % (le trait tel que tracé) à 100 %, pour toutes les trajectoires de ce qui est choisi et pour les prochains tracés. Le trait d'origine est gardé : on peut remonter ou redescendre le curseur à tout moment. Les bouts du chemin et les marques restent à leur place.";

impl App {
    /// Les calques choisis du plan courant qui suivent une trajectoire tracée sur la feuille, le calque courant d'abord.
    pub(crate) fn trajectoire_calques_lissables(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self.chosen_layers().into_iter().filter(|&li| self.layers()[li].trajectoire.as_ref().is_some_and(|t| t.sur_la_feuille())).collect();
        if let Some(k) = out.iter().position(|&li| li == self.cur_layer) {
            out.swap(0, k);
        }
        out
    }

    /// Le curseur « Lissage » des réglages de l'outil : `espace`, pour les trajectoires dans l'espace de ce qui est choisi
    /// (cartes, caméra) ; sinon pour celles des calques choisis sur la feuille. Rend la place du curseur à l'écran.
    pub(crate) fn trajectoire_lissage_ui(&mut self, ui: &mut egui::Ui, espace: bool) -> egui::Rect {
        let (calques, sujets): (Vec<usize>, Vec<Sujet>) = if espace {
            (Vec::new(), self.traj_sujets_des_reglages().into_iter().filter(|&s| self.e3_traj_de(s).is_some()).collect())
        } else {
            (self.trajectoire_calques_lissables(), Vec::new())
        };
        let actuel = match (calques.first(), sujets.first()) {
            (Some(&li), _) => self.layers()[li].trajectoire.as_ref().map(|t| t.lissage),
            (None, Some(&s)) => self.e3_traj_de(s).map(|t| t.lissage),
            _ => None,
        }
        .unwrap_or(self.traj.lissage);
        let mut pct = (actuel * 100.0).round();
        let resp = ui.add(egui::Slider::new(&mut pct, 0.0..=100.0).step_by(1.0).fixed_decimals(0).suffix(" %").text(LISSAGE));
        let resp = crate::bulles::bulle(resp, LISSAGE, AIDE, "");
        let rien = calques.is_empty() && sujets.is_empty();
        if rien {
            // (Rien à lisser : le curseur règle seulement les prochains tracés, hors de l'annulation, comme la sorte des clés.)
            if resp.changed() {
                self.traj.lissage = pct / 100.0;
            }
            return resp.rect;
        }
        let avant = self.history.context.replace(NOM_ETAPE.to_string());
        let change = self.edited(&resp);
        self.history.context = avant;
        if change {
            self.trajectoire_lisser(&calques, &sujets, pct / 100.0);
        }
        // Le geste est fini (curseur lâché, clic, flèche du clavier) : les dessins des clés d'images sont reposés.
        if resp.drag_stopped() || (change && !resp.dragged()) {
            self.trajectoire_lissage_fini(&calques);
        }
        resp.rect
    }

    /// Lisse à `lissage` les trajectoires des calques `calques` (feuille) et des sujets `sujets` (espace), et le retient
    /// pour les prochains tracés. Les clés suivent sans rien refaire de lourd : en clés d'images, les dessins attendent la
    /// fin du geste (`trajectoire_lissage_fini`).
    pub(crate) fn trajectoire_lisser(&mut self, calques: &[usize], sujets: &[Sujet], lissage: f32) {
        self.traj.lissage = lissage.clamp(0.0, 1.0);
        let l = self.traj.lissage;
        for &li in calques {
            let Some(mut t) = self.layers().get(li).and_then(|x| x.trajectoire.clone()) else { continue };
            t.lisser_a(l);
            let interpolation = t.cles == Cles::Interpolation;
            self.layers_mut()[li].trajectoire = Some(t);
            if interpolation {
                self.trajectoire_recalculer(li);
            }
        }
        for &s in sujets {
            let Some(mut t) = self.e3_traj_de(s).cloned() else { continue };
            t.lisser_a(l);
            self.e3_traj_poser(s, t);
        }
        self.modified = true;
        self.renderer.invalidate_params();
        self.status = format!("{LISSAGE} du chemin : {} %.", (l * 100.0).round());
    }

    /// Le geste sur le curseur est fini : les calques en clés d'images reposent leurs dessins d'après le chemin lissé.
    pub(crate) fn trajectoire_lissage_fini(&mut self, calques: &[usize]) {
        for &li in calques {
            if self.layers().get(li).and_then(|l| l.trajectoire.as_ref()).is_some_and(|t: &Trajectoire| t.cles == Cles::Images) {
                self.trajectoire_recalculer(li);
            }
        }
    }
}
