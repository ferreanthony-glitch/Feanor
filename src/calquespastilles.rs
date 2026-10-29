//! Les brosses perso créent leurs calques (Anthony, 2026-10-28, repris au §70 le 2026-10-29) : une brosse Personnage dessine dans « son » calque, une
//! brosse Décor dans le calque Décor du plan, une brosse Classique dans le calque courant (il n'y a plus de pastille « classique » ⊘).
//!
//! **2026-10-29 : le choix du calque d'un personnage.** La copie d'un calque de personnage (« Dupliquer le calque » comme le copier-coller) reste
//! liée au personnage. Choisir sa brosse : le calque courant s'il est de ce personnage (l'original ou une copie), sinon **le dernier calque de ce
//! personnage utilisé dans le plan** (le plus récemment dessiné ou choisi : `Etat::dernier_calque`, en mémoire, jamais dans le fichier), sinon le
//! plus haut dans la pile (`App::calque_pour_personnage`). Un trait fait avec la brosse choisie suit la même règle.
//!
//! # Le calque d'un personnage
//!
//! - **Vectoriel, nommé comme le personnage, avec son aplat.** Le premier trait fait avec la pastille d'un personnage, dans un plan qui n'a
//!   pas encore son calque, le crée : un calque vectoriel (`Layer::personnage` porte le numéro du personnage), posé juste au-dessus du
//!   calque courant, dont la sous-ligne « Aplat » est allumée à la couleur d'aplat du personnage (rien si le personnage n'a pas
//!   d'aplat). Les traits suivants, et ceux des autres plans, vont dans le calque du plan où ils commencent (`pastille_avant_trait`,
//!   à l'appui) ; le personnage qui n'a pas encore de calque dans ce plan en reçoit un à ce moment-là.
//! - **Choisir la pastille d'un personnage qui a déjà son calque dans le plan le rend courant** (`perso_choisir`).
//! - **On le retrouve par le numéro du personnage, pas par le nom** : renommer le personnage dans son panneau renomme ses calques
//!   (`Document::renommer_calques_de_personnage`), ceux dont le nom était encore celui du personnage (un calque que l'on a soi-même
//!   rebaptisé garde son nom). La copie d'un calque (« Dupliquer le calque ») n'est plus « le » calque du personnage.
//! - **Le calque et le premier trait ne font qu'une étape d'annulation**, par la même mécanique que le plan fait en dessinant dans une
//!   case vide et que le calque Décor (`Vignettes::created`, `finish_creation` au lâcher) ; un simple clic qui ne trace rien ne laisse
//!   pas de calque vide. Rien de lourd à l'appui : un cliché de la structure et un calque vide, comme pour le calque Décor.
//! - Pas pour la gomme ni le pot de peinture (ils agissent sur le calque courant), ni quand la sous-ligne d'un aplat est visée (on peint
//!   l'aplat du calque qu'on a choisi), ni pour une brosse Classique (elle dessine dans le calque courant).
//!
//! # Le calque Décor
//!
//! Voir `decor.rs` et `decorui.rs` : un seul calque Décor par plan, vectoriel d'origine, bitmap si Préférences > Vignettes
//! (« Calque Décor ») le demande ; un ancien décor bitmap reste bitmap.
//!
//! # Le fichier
//!
//! Le numéro du personnage est un champ de plus du calque dans le manifeste (`personnage`, absent : calque ordinaire), sans changer
//! `project::VERSION` : une version d'avant l'ouvre comme un calque vectoriel ordinaire, au nom du personnage.

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::fond::FondReglages;
use crate::personnagesui::Choix;

/// Le nom du calque d'un personnage dont le nom est `nom` (« Personnage » s'il n'en a pas).
pub(crate) fn nom_du_calque(nom: &str) -> String {
    if nom.trim().is_empty() {
        "Personnage".to_string()
    } else {
        nom.to_string()
    }
}

impl Document {
    /// Le rang du calque du personnage `id` dans le plan `index`, s'il en a un.
    pub fn calque_de_personnage(&self, index: usize, id: u64) -> Option<usize> {
        self.panels.get(index)?.layers.iter().position(|l| l.personnage == Some(id) && !l.folder)
    }

    /// Le personnage `id` passe du nom `ancien` au nom `nouveau` : ses calques, dans tous les plans, suivent — ceux qui portaient encore son
    /// nom. Rend le nombre de calques renommés.
    pub fn renommer_calques_de_personnage(&mut self, id: u64, ancien: &str, nouveau: &str) -> usize {
        let (ancien, nouveau) = (nom_du_calque(ancien), nom_du_calque(nouveau));
        if ancien == nouveau {
            return 0;
        }
        let mut n = 0;
        for l in self.panels.iter_mut().flat_map(|p| p.layers.iter_mut()) {
            if l.personnage == Some(id) && l.name == ancien {
                l.name.clone_from(&nouveau);
                n += 1;
            }
        }
        n
    }
}

impl App {
    /// Le personnage qui dessine, s'il y en a un (pas une classique ni un décor).
    pub(crate) fn personnage_actif(&self) -> Option<u64> {
        match self.perso.actif {
            Some(Choix::Personnage(id)) => Some(id),
            _ => None,
        }
    }

    /// Une brosse qui a son calque est choisie : un trait qui commence va dans ce calque (`pastille_avant_trait`).
    pub(crate) fn pastille_a_un_calque(&self) -> bool {
        self.decor_actif().is_some() || self.personnage_actif().is_some()
    }

    /// À l'appui d'un trait dans une case de la planche, ou sur le dessin : la brosse choisie prépare son calque. Une brosse Décor : le
    /// calque Décor du plan (`decor_avant_trait`) ; un personnage : son calque (`perso_calque_avant_trait`) ; une classique : rien, le trait
    /// va dans le calque courant.
    pub(crate) fn pastille_avant_trait(&mut self) {
        if self.decor_actif().is_some() {
            self.decor_avant_trait();
        } else {
            self.perso_calque_avant_trait();
        }
    }

    /// Le calque que le personnage `id` doit prendre dans le plan courant, s'il en a un (Anthony, 2026-10-29) : **le calque courant s'il est de
    /// ce personnage** (l'original ou une copie : elles gardent leur lien), sinon **le dernier calque de ce personnage utilisé dans ce plan**
    /// (le plus récemment dessiné ou choisi, `noter_calque_de_personnage`), sinon **le plus haut dans la pile**.
    pub(crate) fn calque_pour_personnage(&self, id: u64) -> Option<usize> {
        let layers = self.layers();
        let est_a_lui = |l: &Layer| l.personnage == Some(id) && !l.folder;
        if layers.get(self.cur_layer).is_some_and(est_a_lui) {
            return Some(self.cur_layer);
        }
        let dernier = self.doc.panels.get(self.cur_panel).and_then(|p| self.perso.dernier_calque.get(&(p.id, id)).copied());
        if let Some(i) = dernier.and_then(|lid| layers.iter().position(|l| l.id == lid && est_a_lui(l))) {
            return Some(i);
        }
        layers.iter().rposition(est_a_lui)
    }

    /// Le calque courant est celui d'un personnage : il devient le « dernier utilisé » de ce personnage dans ce plan (en mémoire, pas dans le
    /// fichier). Appelé quand on choisit un calque, quand une brosse en prend un et quand un trait commence.
    pub(crate) fn noter_calque_de_personnage(&mut self) {
        let Some(plan) = self.doc.panels.get(self.cur_panel).map(|p| p.id) else { return };
        let Some((pid, lid)) = self.layers().get(self.cur_layer).filter(|l| !l.folder).and_then(|l| l.personnage.map(|p| (p, l.id))) else { return };
        self.perso.dernier_calque.insert((plan, pid), lid);
    }

    /// Choisir la brosse d'un personnage rend courant son calque du plan (voir `calque_pour_personnage`).
    pub(crate) fn perso_rendre_son_calque(&mut self, id: u64) {
        if let Some(i) = self.calque_pour_personnage(id) {
            self.select_layer(i);
            self.noter_calque_de_personnage();
        }
    }

    /// Le calque du personnage qui dessine est le calque courant du plan où le trait commence, créé au besoin (une étape d'annulation avec le
    /// trait : `finish_creation`).
    fn perso_calque_avant_trait(&mut self) {
        let Some(id) = self.personnage_actif() else { return };
        if !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline) {
            return;
        }
        // Une couleur prise à la main lâche le personnage (`perso_avant_trait`) ; l'aplat visé se peint là où l'on a cliqué.
        if self.content_refuses() || self.aplat_en_cible() || self.color != self.perso.posee {
            return;
        }
        let Some((nom, fond)) = self.doc.personnages.iter().find(|p| p.id == id).map(|p| (p.name.clone(), p.fill)) else { return };
        self.end_stroke();
        if let Some(i) = self.calque_pour_personnage(id) {
            if i != self.cur_layer {
                self.select_layer(i);
            }
            self.noter_calque_de_personnage();
            return;
        }
        let snap = self.snapshot();
        let mut layer = Layer::new(nom_du_calque(&nom), LayerKind::Vector);
        layer.personnage = Some(id);
        // Son aplat : la couleur d'aplat du personnage (la sous-ligne « Aplat » du calque est allumée) ; sans aplat, le calque n'en a pas.
        layer.fond = fond.map(|couleur| FondReglages { couleur, ..self.new_fond() });
        // Juste au-dessus du calque courant, dans son dossier (comme « Nouveau calque »).
        layer.parent = self.layers().get(self.cur_layer).and_then(|l| l.parent);
        let at = (self.cur_layer + 1).min(self.layers().len());
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.noter_calque_de_personnage();
        self.aplat_cible = None;
        self.vsel = None;
        self.modified = true;
        if self.vignettes.created.is_none() {
            // Le calque et le premier trait : une étape (la création se joint au trait au lâcher, `finish_creation`).
            let titre = format!("Dessiner avec {}", nom_du_calque(&nom));
            self.avec_etape(&titre, |s| s.commit_structure(snap));
            self.vignettes.created = Some(crate::boardui::Created { base_len: self.history.undo_len() });
        } else {
            // Dans une case qu'on vient de faire : l'étape de la case défait tout, le calque avec.
            self.sync_couleurs();
            self.renderer.invalidate_all();
        }
        self.status = format!("« {} » : un calque vectoriel à son nom vient d'être créé pour ses traits.", nom_du_calque(&nom));
    }
}