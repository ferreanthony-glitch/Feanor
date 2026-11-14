//! **La vue Espace 3D suffit** (`docs/INTERFACE.md`, partie 158). Ouvrir la vue Espace 3D (le petit cube) sur un plan le
//! met en Espace 3D (`Panel::espace`, `docs/ESPACE-LIBRE.md`) tout seul, sans le bouton « Plan en Espace 3D », **quand ce
//! passage ne change rien** : ni un pixel de l'image filmée, ni un dessin, ni la façon dont les outils de dessin
//! touchent ses calques. Les calques 3D (Sol, Mur, Ciel) et « Nouvelle surface » font de même au lieu de refuser.
//!
//! Ce que le passage change, et quand (relevé de la partie 158, `Garde`) :
//!
//! - la caméra du plan n'est pas la caméra d'origine à l'une de ses images : l'image passerait par le changement de
//!   repère (des pixels différents au bord des formes, l'encre hors de la feuille plus filmée) ;
//! - une carte est au-delà de la borne des plans ordinaires (`Layer::passe_la_borne`) : elle ne serait plus montrée à
//!   la borne ;
//! - une carte est penchée : dans un plan en Espace 3D, le pinceau n'y peint plus hors de ses bords
//!   (`App::to_layer_opt`) ;
//! - l'ordre de la pile et celui des profondeurs ne disent pas la même chose : ce qui est devant passerait devant
//!   (`espace3d::ordre_par_distance`).
//!
//! Hors de ces cas, le plan en Espace 3D se compose par le chemin de toujours (`Document::repere_du_plan` ne rend rien),
//! dans l'ordre de la pile : les mêmes pixels. Dans ces cas, le plan reste ordinaire et la barre d'état dit pourquoi ; le
//! bouton « Plan en Espace 3D » l'y met quand même, en connaissance de cause.
//!
//! **Le retour** : un plan que la vue a mis en Espace 3D (marqué `Panel::espace_tout_seul`, jamais enregistré : le
//! fichier le garde ordinaire) redevient ordinaire quand on le quitte (autre plan, vue fermée, Ctrl+Z…) si rien en lui
//! n'a besoin de l'Espace 3D (`Document::garde_du_retour`) : regarder un plan dans la vue ne lui laisse rien, et l'outil
//! Caméra, la vue de dessus et les mouvements de caméra des Vignettes, réservés aux plans ordinaires, le retrouvent comme
//! avant. Dès qu'il s'en sert (un calque 3D, une caméra déplacée, une carte penchée…), la marque tombe : c'est un vrai plan
//! en Espace 3D, enregistré comme tel.
//!
//! **L'annulation** : ce passage n'est pas une étape (il ne change ni l'image ni le dessin, comme les vues ou
//! l'environnement). Une étape d'annulation qui ramène le plan ordinaire est suivie : s'il est encore dans la vue et que
//! rien ne s'y oppose, il y repasse. Les calques 3D et les surfaces mettent le plan en Espace 3D dans **leur** étape :
//! Ctrl+Z enlève le calque et rend le plan ordinaire d'un coup. Le bouton reste une étape, comme avant.

use crate::app::App;
use crate::document::Document;

/// Ce qui empêche un plan ordinaire de passer tout seul en Espace 3D : le passage changerait quelque chose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Garde {
    /// Le cadre du projet est plus grand que la feuille (`Document::accepte_plan_en_espace`).
    Cadre,
    /// La caméra du plan n'est pas la caméra d'origine à l'une de ses images.
    Camera,
    /// Une carte est au-delà de la borne des plans ordinaires, à l'une des images.
    Borne,
    /// Une carte est penchée, à l'une des images.
    Penchee,
    /// L'ordre de la pile et celui des profondeurs ne disent pas la même chose, à l'une des images.
    Ordre,
    /// Le plan a ce qu'un plan ordinaire n'a pas : un calque 3D, une trajectoire dans l'espace (pour le retour seulement).
    TroisD,
}

impl Garde {
    /// La phrase de la barre d'état quand le plan reste ordinaire : l'essentiel dans ses ~70 premiers caractères
    /// (`messages::resume`).
    pub(crate) fn phrase(self) -> &'static str {
        match self {
            Garde::Cadre => "Plan gardé ordinaire : le cadre du projet dépasse la feuille. Agrandis la feuille (Fichier > Réglages du projet) pour l'Espace 3D.",
            Garde::Camera => "Plan gardé ordinaire : sa caméra a bougé, la 3D changerait l'image. Bouton « Plan en Espace 3D » pour l'y mettre quand même.",
            Garde::Borne => "Plan gardé ordinaire : une carte est contre la caméra. Bouton « Plan en Espace 3D » pour l'y mettre quand même.",
            Garde::Penchee => "Plan gardé ordinaire : il a une carte penchée. Bouton « Plan en Espace 3D » pour l'y mettre quand même (le pinceau n'y peint plus hors de la carte).",
            Garde::Ordre => "Plan gardé ordinaire : sa pile n'est pas dans l'ordre des profondeurs. Bouton « Plan en Espace 3D » pour l'y mettre quand même.",
            Garde::TroisD => "Plan gardé ordinaire : il a un calque 3D ou un chemin dans l'espace. Bouton « Plan en Espace 3D » pour l'y remettre.",
        }
    }
}

/// La phrase de la barre d'état quand la vue vient de mettre le plan en Espace 3D.
pub(crate) const PASSE: &str = "Plan passé en Espace 3D tout seul : son image filmée ne change pas. Caméra libre, ce qui est devant cache ce qui est derrière.";

impl Document {
    /// Ce qui empêche le plan `index` d'être un plan en Espace 3D ou un plan ordinaire **sans que rien ne change**
    /// (`Garde`) ; rien : les deux donnent la même image, les mêmes dessins, les mêmes outils de dessin. La vue s'en sert
    /// pour mettre un plan ordinaire en Espace 3D.
    pub(crate) fn garde_de_l_espace(&self, index: usize) -> Option<Garde> {
        let p = self.panels.get(index)?;
        if !self.accepte_plan_en_espace() {
            return Some(Garde::Cadre);
        }
        if Self::a_de_l_espace(p) {
            return Some(Garde::TroisD);
        }
        // La caméra : celle de l'image (clés, celles d'un plan voisin, paliers), à chaque image. (Un effet de caméra vivant
        // la fait bouger : sans la recuire image par image.)
        if p.effet_camera.is_some() {
            return Some(Garde::Camera);
        }
        let origine = self.default_camera();
        if (0..p.frame_count().max(1)).any(|f| self.panel_camera_reelle(index, f) != origine) {
            return Some(Garde::Camera);
        }
        self.garde_des_cartes(index)
    }

    /// Ce qui garde en Espace 3D un plan que la vue y a mis tout seul (`Panel::espace_tout_seul`) : comme
    /// `garde_de_l_espace`, mais seule la caméra du plan lui-même compte (ses clés, son effet, sa trajectoire). Une caméra
    /// tenue d'un plan voisin, qui a bougé depuis, ne le garde pas : il revient à l'image d'un plan ordinaire, qu'il avait
    /// avant que la vue ne le montre.
    pub(crate) fn garde_du_retour(&self, index: usize) -> Option<Garde> {
        let p = self.panels.get(index)?;
        if Self::a_de_l_espace(p) {
            return Some(Garde::TroisD);
        }
        let origine = self.default_camera();
        if p.effet_camera.is_some() || p.camera.iter().any(|k| k.cam != origine) {
            return Some(Garde::Camera);
        }
        self.garde_des_cartes(index)
    }

    /// Un calque 3D, une trajectoire dans l'espace : ce qu'un plan ordinaire n'a pas.
    fn a_de_l_espace(p: &crate::document::Panel) -> bool {
        p.trajectoire_camera.is_some() || p.layers.iter().any(|l| l.calque_3d.is_some() || l.trajectoire.as_ref().is_some_and(|t| t.support.is_some()))
    }

    /// Ce que les cartes du plan `index` changent au passage : borne, carte penchée, ordre (`Garde`).
    fn garde_des_cartes(&self, index: usize) -> Option<Garde> {
        let p = self.panels.get(index)?;
        // Les cartes : à chaque image s'il y en a une d'animée (clés de position), sinon la première suffit.
        let n = p.frame_count().max(1);
        let images = if p.layers.iter().any(|l| !l.pos_keys.is_empty()) { n } else { 1 };
        let cartes = || p.layers.iter().filter(|l| crate::espace3d::is_card(l));
        if cartes().any(|l| (0..images).any(|f| l.passe_la_borne(f))) {
            return Some(Garde::Borne);
        }
        if cartes().any(|l| (0..images).any(|f| l.pose(f).is_tilted())) {
            return Some(Garde::Penchee);
        }
        // L'ordre de pose : celui de la pile dans un plan ordinaire, celui des distances dans un plan en Espace 3D (sous la
        // caméra d'origine). Les dossiers ne posent rien eux-mêmes : seul l'ordre de leurs calques compte.
        let world = crate::espace3d::Monde { libre: true, ..crate::espace3d::Monde::of(self) };
        let pile = crate::calqueannot::ordre(&p.layers, true);
        let sans_dossiers = |o: &[usize]| o.iter().copied().filter(|&li| !p.layers[li].folder).collect::<Vec<_>>();
        let attendu = sans_dossiers(&pile);
        if (0..images).any(|f| sans_dossiers(&crate::espace3d::ordre_par_distance(&world, &p.layers, &pile, f, None)) != attendu) {
            return Some(Garde::Ordre);
        }
        None
    }
}

/// Ce que la vue retient du passage tout seul (`Espace3dState::direct`). Ce qui a été mis en Espace 3D tout seul est
/// marqué dans le plan lui-même (`Panel::espace_tout_seul`) : la marque suit l'annulation, les versions, la duplication.
#[derive(Default)]
pub(crate) struct Direct {
    /// Le plan montré par la vue et l'empreinte de l'historique à la dernière image : rien à refaire tant qu'ils ne
    /// changent pas.
    vu: Option<(Option<u64>, (usize, usize, u64, u64))>,
    /// Les plans dont « Plan en Espace 3D » a été décoché à la main pendant la séance : la vue ne les y remet pas.
    refuses: Vec<u64>,
}

impl Direct {
    /// Le bouton « Plan en Espace 3D » vient de mettre le plan `id` en Espace 3D (`on`) ou de le rendre ordinaire : c'est
    /// un choix, la vue ne l'y remet plus d'elle-même.
    pub(crate) fn choix_a_la_main(&mut self, id: u64, on: bool) {
        self.refuses.retain(|&m| m != id);
        if !on {
            self.refuses.push(id);
        }
    }
}

impl App {
    /// À chaque image (`App::frame`) : le plan que montre la vue Espace 3D passe en Espace 3D si rien ne change ; un plan
    /// qu'elle y avait mis et qu'on a quitté redevient ordinaire s'il n'en a pas l'usage ; et un plan mis tout seul qui
    /// s'en sert maintenant (un calque 3D, une caméra déplacée…) devient un vrai plan en Espace 3D, enregistré comme tel.
    /// Ne fait rien tant que le plan montré et l'historique sont les mêmes qu'à l'image d'avant.
    pub(crate) fn e3_espace_direct(&mut self) {
        let ici = self.espace3d_vue_ouverte().then(|| self.panel().id);
        let cle = (ici, self.history.empreinte());
        let avant = self.espace3d.direct.vu.replace(cle);
        if avant == Some(cle) {
            return;
        }
        let nouveau_plan = avant.map_or(true, |(vu, _)| vu != ici);
        for pi in 0..self.doc.panels.len() {
            let p = &self.doc.panels[pi];
            if !(p.espace && p.espace_tout_seul) {
                continue;
            }
            let montre = Some(p.id) == ici;
            match self.doc.garde_du_retour(pi) {
                // (Il s'en sert : un vrai plan en Espace 3D, qui s'enregistre comme tel.)
                Some(_) => {
                    self.doc.panels[pi].espace_tout_seul = false;
                    self.modified = true;
                }
                // (Quitté sans s'en servir : ordinaire de nouveau. Rien d'enregistré n'a changé.)
                None if !montre => {
                    let p = &mut self.doc.panels[pi];
                    p.espace = false;
                    p.espace_tout_seul = false;
                }
                None => {}
            }
        }
        // Le plan montré.
        let Some(id) = ici else { return };
        let pi = self.cur_panel;
        if self.doc.panels[pi].espace || self.espace3d.direct.refuses.contains(&id) {
            return;
        }
        match self.doc.garde_de_l_espace(pi) {
            None => {
                // (Pas d'étape d'annulation, pas d'enregistrement : rien de l'image ni des dessins ne change, et le fichier
                // garde le plan ordinaire tant que rien ne s'en sert.)
                let p = &mut self.doc.panels[pi];
                p.espace = true;
                p.espace_tout_seul = true;
                if nouveau_plan {
                    self.status = PASSE.into();
                }
            }
            Some(g) => {
                if nouveau_plan {
                    self.status = g.phrase().into();
                }
            }
        }
    }

    /// Les calques 3D et « Nouvelle surface » sur un plan ordinaire : vrai si le plan peut passer en Espace 3D sans que
    /// rien ne change (`garde_de_l_espace`), ce que fera leur étape d'annulation ; faux sinon, et la barre d'état dit
    /// d'abord pourquoi (`refus` : le début de la phrase, « Sol refusé »).
    pub(crate) fn e3_espace_permis(&mut self, refus: &str) -> bool {
        if self.doc.plan_en_espace(self.cur_panel) {
            return true;
        }
        match self.doc.garde_de_l_espace(self.cur_panel) {
            None => true,
            Some(g) => {
                let phrase = g.phrase().strip_prefix("Plan gardé ordinaire : ").unwrap_or(g.phrase());
                self.status = format!("{refus} : {phrase}");
                false
            }
        }
    }
}