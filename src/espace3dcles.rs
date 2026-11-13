//! Les boutons de clés du panneau Réglages de l'Espace 3D (`docs/INTERFACE.md`, partie 145). Anthony, 2026-11-12 :
//! « quand la vue Espace 3D est ouverte, le panneau Réglages n'a aucun bouton de clés, ni pour la caméra ni pour les
//! cartes […] j'aimerais qu'un bouton y apparaisse ». (Depuis la partie 150, ces réglages de la vue, et donc ces boutons,
//! sont dans le panneau « Espace 3D », `espace3dpanneau.rs` : le panneau Réglages montre toujours l'outil en main.)
//!
//! Ce sont les boutons des panneaux 2D, avec leurs commandes, leurs icônes et leurs bulles :
//! - sur la ligne « Caméra du plan » : « Clé de caméra ici » (`camera.cle_ici`) et « Supprimer la clé de caméra »
//!   (`camera.cle_supprimer`), ceux des réglages de l'outil Caméra (`camtools.rs`) ;
//! - sur la ligne de la carte choisie (ou des cartes choisies) : « Clé de position » (`calque.cle_position`) et
//!   « Supprimer la clé d'ici » (`calque.cle_position_supprimer`), ceux des réglages de l'outil Transformer
//!   (`poskeysui.rs`).
//!
//! Le losange est **allumé** quand l'image courante porte une clé : dans cette vue, c'est le seul endroit du panneau
//! qui le dit. La corbeille à côté la retire.
//!
//! La règle « la sélection agit sur tout ce qui est choisi » : tant que la vue Espace 3D est ouverte et qu'au moins
//! une carte y est choisie, les commandes de clés de position (poser, lisser, supprimer, tout supprimer) visent les
//! calques des cartes choisies, et non le seul calque courant (`App::pos_targets`). Une étape d'annulation pour toutes.
//! Une clé de position garde toute la pose de la carte : place, échelle, étirements, rotation, inclinaisons et écart de
//! profondeur (`poskeys::PosKey`).

use eframe::egui;
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::poskeys;

/// Ce qu'on dit quand les cartes choisies dans l'Espace 3D ne peuvent recevoir aucune clé de position.
pub(crate) const CARTES_VERROUILLEES: &str = "Clé de position : la carte choisie est verrouillée (elle, son dossier ou sa position). Rien n'est changé.";

/// Ce qu'on dit quand le losange de la caméra, déjà allumé, est cliqué.
pub(crate) const CLE_DE_CAMERA_DEJA_LA: &str = "Il y a déjà une clé de caméra à cette image : déplace la caméra pour la régler, ou retire la clé avec la corbeille.";

impl App {
    /// Les calques des cartes choisies dans la vue Espace 3D, dans l'ordre du choix. Rien (`None`) quand la vue n'est pas
    /// ouverte ou qu'aucune carte n'y est choisie (la caméra seule ne compte pas) : les commandes de clés visent alors le
    /// calque courant, comme partout ailleurs.
    pub(crate) fn e3_cartes_choisies(&self) -> Option<Vec<usize>> {
        if !(self.espace3d.on && !self.camera_view) {
            return None;
        }
        let lis: Vec<usize> = self.espace3d.sel.iter().filter_map(|&s| self.e3_card_layer(s)).collect();
        (!lis.is_empty()).then_some(lis)
    }

    /// Parmi les cartes choisies (`e3_cartes_choisies`), celles dont la pose peut changer : ni verrouillées (elles ou leur
    /// dossier), ni de position verrouillée. C'est la liste que visent les commandes de clés de position.
    pub(crate) fn e3_cartes_des_cles(&self) -> Option<Vec<usize>> {
        let layers = self.layers();
        self.e3_cartes_choisies().map(|lis| lis.into_iter().filter(|&li| layers[li].drawable() && !crate::folders::locked_by_folder(layers, li)).collect())
    }

    /// Le losange de la caméra : pose une clé de caméra à l'image courante (`camera.cle_ici` : la caméra telle qu'elle y
    /// est, l'image ne change pas). Déjà une clé ici : rien ne change, la barre d'état le dit.
    pub(crate) fn espace3d_cle_camera_ici(&mut self) {
        if self.camera_key_here().is_some() {
            self.status = CLE_DE_CAMERA_DEJA_LA.into();
        } else {
            self.run_action("camera.cle_ici");
        }
    }

    /// Les deux boutons de clés de la caméra, sur la ligne « Caméra du plan » du panneau Espace 3D.
    pub(crate) fn espace3d_boutons_cle_camera(&mut self, ui: &mut egui::Ui) {
        let ici = self.camera_key_here().is_some();
        let poser = self
            .bouton_etat(ui, ici, ph::DIAMOND, "Clé de caméra ici", "Une clé de caméra à l'image courante (allumé : la clé y est). Change d'image, déplace la caméra : elle va d'une clé à l'autre.", "camera.cle_ici")
            .clicked();
        let retirer = self.bouton_si(ui, ici, ph::TRASH, "Supprimer la clé de caméra", "Supprimer la clé de caméra de cette image.", "camera.cle_supprimer").clicked();
        if poser {
            self.espace3d_cle_camera_ici();
        }
        if retirer {
            self.run_action("camera.cle_supprimer");
        }
    }

    /// Les deux boutons de clés de position, sur la ligne de la carte choisie (ou des cartes choisies) du panneau
    /// Espace 3D. Le losange est allumé quand toutes les cartes choisies ont une clé à l'image courante ; la corbeille
    /// s'offre dès qu'une carte libre en a une.
    pub(crate) fn espace3d_boutons_cle_position(&mut self, ui: &mut egui::Ui) {
        let f = self.cur_frame;
        let layers = self.layers();
        let a_cle = |li: usize| layers.get(li).is_some_and(|l| poskeys::key_at(&l.pos_keys, f).is_some());
        let choisies = self.e3_cartes_choisies().unwrap_or_default();
        let toutes = !choisies.is_empty() && choisies.iter().all(|&li| a_cle(li));
        let une = self.e3_cartes_des_cles().unwrap_or_default().into_iter().any(a_cle);
        let poser = self
            .bouton_etat(ui, toutes, ph::DIAMOND, "Clé de position", "Poser une clé avec la pose de la carte à cette image (allumé : la clé y est). Change d'image, déplace la carte : elle s'anime, et va d'une clé à l'autre.", "calque.cle_position")
            .clicked();
        let retirer = self.bouton_si(ui, une, ph::TRASH, "Supprimer la clé d'ici", "Retire la clé de position de cette image.", "calque.cle_position_supprimer").clicked();
        if poser {
            self.pos_key_here();
        }
        if retirer {
            self.pos_keys_delete_here();
        }
    }
}
