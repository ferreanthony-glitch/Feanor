//! **Tous les plans sont dans l'espace** (`docs/INTERFACE.md`, parties 158 et 159). Il n'y a plus deux sortes de plans ni
//! de bouton « Plan en Espace 3D » : la vue de la scène (le petit cube) montre et règle n'importe quel plan, et Sol, Mur,
//! Ciel ou une surface se posent sur n'importe quel plan, sans refus. La règle d'Anthony (2026-11-14) : « la 3D ne doit
//! jamais se sentir tant qu'on ne bascule pas dans la vue de la "scène 3D". N'importe quelle personne qui prend le logiciel
//! en main doit se dire que c'est de la 2D. »
//!
//! Ce qui tient cette règle, plan par plan (format 56) :
//!
//! - **le chemin d'image** : un plan se compose par le chemin de toujours (le multiplan, puis le cadre sur la feuille) tant
//!   que sa caméra reste dans les anciennes limites (`Document::camera_dans_les_anciennes_limites`) et qu'aucune carte n'a
//!   passé la borne d'autrefois. Seules les images où la vue de la scène a mis la caméra hors de ces limites
//!   (`Panel::camera_libre`), ou une carte au-delà de la borne, passent par le changement de repère (`Document::repere_du_plan`).
//!   Un plan dessiné à plat ne change donc d'aucun pixel ;
//! - **l'ordre des calques** : celui de la pile, comme dans tout logiciel de dessin, sauf si le plan a « ce qui est devant
//!   cache ce qui est derrière » (`Panel::ordre_distance`), un réglage de la vue de la scène ;
//! - **les outils de la feuille** (outil Caméra, vue de dessus, mouvements de caméra des Vignettes) : les mêmes gestes, le
//!   même résultat, sur tout plan ; la caméra que la feuille règle reste bridée comme avant, celle que la vue de la scène pose
//!   est libre (`App::set_current_camera_libre`).
//!
//! Les plans d'un projet du format 55 en Espace 3D gardent leur image au pixel près (`Panel::espace`).

use crate::app::App;
use crate::document::Document;

/// Les mots qui disent la 3D (partie 159) : hors de la vue de la scène, aucun texte de l'interface ne doit les montrer.
pub(crate) const MOTS_DE_LA_3D: [&str; 8] = ["3D", "3d", "en mètres", "carte inclinée", "cartes (", "Plan en Espace", "plan en Espace", "scène 3D"];

/// Le texte `texte` parle de la 3D (`MOTS_DE_LA_3D`, et le mot « carte » d'une carte de l'espace) : il n'a pas sa place hors
/// de la vue de la scène. (« carte graphique », « carte dédiée », la petite carte de la visite : pas de la 3D.)
pub(crate) fn dit_la_3d(texte: &str) -> bool {
    if MOTS_DE_LA_3D.iter().any(|m| texte.contains(m)) {
        return true;
    }
    // (« dans l'espace » : la scène, pas l'espace de travail.)
    if texte.match_indices("dans l'espace").any(|(i, m)| !texte[i + m.len()..].starts_with(" de travail")) {
        return true;
    }
    let bas = texte.to_lowercase();
    bas.match_indices("carte").any(|(i, _)| {
        let (avant, suite) = (&bas[..i], &bas[i..]);
        // (Un mot entier : « carte », « cartes », pas « écarte » ni « cartel ».)
        let mot = avant.chars().next_back().is_none_or(|c| !c.is_alphanumeric()) && suite[5..].chars().next().is_none_or(|c| c == 's' || !c.is_alphanumeric());
        let autre_carte = ["carte graphique", "cartes graphiques", "carte dédiée", "carte virtuelle", "carte en bas", "carte s'en va"].iter().any(|ok| suite.starts_with(ok)) || avant.ends_with("petite ");
        mot && !autre_carte
    })
}

impl Document {
    /// L'ordre de la pile du plan `index` est déjà celui des distances à sa caméra, à toutes ses images (une seule s'il n'a
    /// aucun calque animé) : passer à « ce qui est devant cache ce qui est derrière » ne change rien de son image.
    pub(crate) fn pile_dans_l_ordre_des_distances(&self, index: usize) -> bool {
        let Some(p) = self.panels.get(index) else { return true };
        let n = p.frame_count().max(1);
        let images = if p.layers.iter().any(|l| !l.pos_keys.is_empty() || l.trajectoire.is_some()) || p.camera.len() > 1 || p.trajectoire_camera.is_some() || p.effet_camera.is_some() { n } else { 1 };
        let world = crate::espace3d::Monde::du_plan(self, index);
        let pile = crate::calqueannot::ordre(&p.layers, true);
        // (Les dossiers ne posent rien eux-mêmes : seul l'ordre de leurs calques compte.)
        let sans_dossiers = |o: &[usize]| o.iter().copied().filter(|&li| !p.layers[li].folder).collect::<Vec<_>>();
        let attendu = sans_dossiers(&pile);
        (0..images).all(|f| {
            let repere = self.repere_du_plan(index, f);
            sans_dossiers(&crate::espace3d::ordre_par_distance(&world, &p.layers, &pile, f, repere.as_ref())) == attendu
        })
    }
}

/// Ce que la barre d'état ajoute quand un calque de la scène (Sol, Mur, Ciel, une surface) se pose sur un plan qui garde
/// l'ordre de sa pile parce que l'ordre des distances changerait son image.
pub(crate) const PILE_GARDEE: &str = "La pile garde son ordre : « Devant cache derrière » (panneau Espace 3D) pour ordonner par la distance.";

impl App {
    /// Un calque de la scène (Sol, Mur, Ciel, une surface) va se poser sur le plan courant, dans l'étape d'annulation en
    /// cours : si le plan compose encore dans l'ordre de sa pile et que l'ordre des distances ne changerait rien de son image,
    /// il passe à « ce qui est devant cache ce qui est derrière » (le calque neuf prend alors sa place selon sa distance, un
    /// sol derrière ce qui est debout dessus). Sinon il garde sa pile : vrai, la barre d'état le dira (`PILE_GARDEE`).
    pub(crate) fn e3_ordre_pour_un_calque_de_la_scene(&mut self) -> bool {
        let pi = self.cur_panel;
        if self.doc.ordre_par_la_distance(pi) {
            return false;
        }
        if self.doc.pile_dans_l_ordre_des_distances(pi) {
            self.doc.panels[pi].ordre_distance = true;
            return false;
        }
        true
    }

    /// Règle la caméra du plan courant depuis **la vue de la scène** : comme `set_current_camera`, mais la caméra est prise
    /// telle quelle (elle se retourne, bascule jusqu'à la verticale, passe la feuille). Si elle sort ainsi des anciennes
    /// limites, le plan le retient (`Panel::camera_libre`) : à ces images, l'image passe par le changement de repère. Dans
    /// les anciennes limites, rien ne change : le chemin de toujours.
    pub(crate) fn set_current_camera_libre(&mut self, cam: crate::camera::Camera) {
        let pi = self.cur_panel;
        if !self.doc.panels[pi].espace && !self.doc.camera_dans_les_anciennes_limites(&cam) && !self.doc.panels[pi].camera_libre {
            self.doc.panels[pi].camera_libre = true;
            self.renderer.invalidate_all();
        }
        self.poser_la_camera(cam);
    }
}
