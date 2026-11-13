//! Continuer un chemin avec Maj (`docs/INTERFACE.md`, partie 151). Anthony, 2026-11-12 : « maintenir Maj + tracer une
//! nouvelle trajectoire depuis la fin ou le début d'une trajectoire existante continue la trajectoire déjà tracée ».
//!
//! **Le geste.** Avec l'outil Trajectoire, Maj tenue, un appui près d'un **bout** du chemin (son début ou sa fin, à
//! `TOL_ECRAN` pixels d'écran), puis un trait : au lâcher, le trait prolonge le chemin (`Trajectoire::prolonger`). Depuis
//! le début, il est ajouté devant ; depuis la fin, derrière. Maj + appui ailleurs sur le chemin le déplace toujours en bloc.
//! Un geste = une étape d'annulation ; Échap l'abandonne (rien n'a changé pendant le trait).
//!
//! **Ce qui suivait le chemin continue de le suivre** : les marques gardent leur place (leurs fractions sont recalculées
//! sur la nouvelle longueur) ; le déplacement se mesure toujours depuis le même point (`Trajectoire::reference`, l'ancien
//! départ quand on prolonge par devant) ; le temps se répartit de nouveau en parts égales entre les jalons, selon la sorte
//! des clés du chemin, comme pour un chemin neuf (le départ et la fin restent sur leurs images). Le lissage du chemin
//! s'applique au morceau ajouté.
//!
//! **Sur la feuille**, le chemin du calque courant ; **dans l'Espace 3D**, celui de tout ce qui est choisi, le trait restant
//! sur le plan qui porte le chemin.

use eframe::egui::Pos2;

use crate::app::App;
use crate::espace3ddessin::Regard3d;
use crate::trajectoire::{distance, Support, Trajectoire};
use crate::trajectoire3d::GesteEspace;
use crate::trajectoireui::TOL_ECRAN;

/// Ce que dit la barre d'état quand le geste commence.
pub(crate) const PROLONGER: &str = "Trajectoire : trace la suite du chemin depuis ce bout (Maj) ; lâcher la pose. Échap : abandonner.";
/// … et quand le trait est trop court.
pub(crate) const TROP_COURT: &str = "Trajectoire : le trait est trop court pour prolonger le chemin. Maj + un trait depuis un bout du chemin le prolonge.";

/// Le bout du chemin (vu à l'écran par `ecran`) sous le point `p` de l'écran, à `TOL_ECRAN` près : vrai pour le début, faux
/// pour la fin ; le plus proche s'ils y sont tous les deux. Rien : `p` n'est près d'aucun bout.
pub(crate) fn bout_sous(t: &Trajectoire, ecran: &dyn Fn([f32; 2]) -> Option<Pos2>, p: Pos2) -> Option<bool> {
    let d = |q: [f32; 2]| ecran(q).map_or(f32::INFINITY, |e| e.distance(p));
    let (debut, fin) = (d(t.points[0]), d(*t.points.last()?));
    if debut.min(fin) > TOL_ECRAN {
        return None;
    }
    Some(debut <= fin)
}

impl App {
    /// Sur la feuille : le bout du chemin du calque courant sous le point `stage` de la feuille (vrai : son début). Rien pour
    /// une trajectoire tracée dans l'espace, ou loin des bouts.
    pub(crate) fn trajectoire_bout_sous(&self, stage: Pos2) -> Option<bool> {
        let t = self.layers().get(self.cur_layer)?.trajectoire.as_ref().filter(|t| t.sur_la_feuille())?;
        let p = self.stage_to_screen(stage);
        bout_sous(t, &|q| Some(self.stage_to_screen(Pos2::new(q[0], q[1]))), p)
    }

    /// Sur la feuille, au lâcher : le trait `points` (pixels de la feuille) prolonge le chemin du calque `li` (`devant` :
    /// depuis son début), en une étape d'annulation ; ses clés et ses dessins sont reposés.
    pub(crate) fn trajectoire_prolonger(&mut self, li: usize, points: &[[f32; 2]], devant: bool) {
        let Some(mut t) = self.layers().get(li).and_then(|l| l.trajectoire.clone()) else { return };
        // (Un trait plus court qu'un clic, à l'écran comme sur la feuille, ne prolonge rien.)
        let longueur: f32 = points.windows(2).map(|w| distance(w[0], w[1])).sum();
        if longueur < crate::trajectoire::LONGUEUR_MIN || !t.prolonger(points, devant) {
            self.status = TROP_COURT.into();
            return;
        }
        let snap = self.snapshot();
        self.layers_mut()[li].trajectoire = Some(t);
        self.trajectoire_recalculer(li);
        self.trajectoire_valider(snap, "Prolonger la trajectoire");
        self.status = self.trajectoire_resume(li, if devant { "Chemin prolongé par son début" } else { "Chemin prolongé par sa fin" });
    }

    /// Dans l'Espace 3D, à l'appui (Maj tenue) au point `pos` de l'écran : le geste qui prolonge le chemin `t` (posé sur
    /// `support`, vu par `regard`), si l'appui est près d'un de ses bouts et que le regard touche le plan du chemin.
    pub(crate) fn e3_traj_prolonger_depuis(t: &Trajectoire, support: Support, regard: &Regard3d, pos: Pos2) -> Option<GesteEspace> {
        let devant = bout_sous(t, &|p| regard.ecran(support.monde(p)), pos)?;
        let ray = regard.rayon(pos);
        let depart = support.couper(ray.o, ray.d)?;
        Some(GesteEspace::Prolonger { support, devant, ecran: vec![pos], plan: vec![depart] })
    }

    /// Dans l'Espace 3D, au lâcher : le trait (`ecran` : à l'écran, `plan` : dans le plan du chemin) prolonge le chemin de
    /// tout ce qui est choisi et en suit un (`devant` : depuis son début), en une étape d'annulation.
    pub(crate) fn e3_traj_prolonger(&mut self, ecran: &[Pos2], plan: &[[f32; 2]], devant: bool) {
        let longueur: f32 = ecran.windows(2).map(|w| w[0].distance(w[1])).sum();
        if longueur < crate::trajectoire::LONGUEUR_MIN {
            self.status = TROP_COURT.into();
            return;
        }
        let sujets = self.e3_traj_sujets_libres();
        if sujets.is_empty() {
            // (La barre d'état dit déjà pourquoi : `e3_traj_sujets_libres`.)
            return;
        }
        let snap = self.snapshot();
        let mut n = 0;
        for s in sujets {
            let Some(mut t) = self.e3_traj_de(s).cloned() else { continue };
            if t.prolonger(plan, devant) {
                self.e3_traj_poser(s, t);
                n += 1;
            }
        }
        if n == 0 {
            self.status = TROP_COURT.into();
            return;
        }
        self.modified = true;
        self.renderer.invalidate_params();
        self.commit_named(snap, "Prolonger la trajectoire");
        self.status = self.e3_traj_resume(if devant { "Chemin prolongé par son début" } else { "Chemin prolongé par sa fin" });
    }
}
