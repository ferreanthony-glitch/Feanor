//! L'état de l'accueil et du confort (passe « simplicité », branche `simpl-accueil`, 2026-10-21) : ce que SHOT tient en
//! mémoire pour le mode concentration, les conseils d'un seul passage, l'aide-mémoire des raccourcis et la fenêtre du
//! réglage du stylet. Rien de tout cela n'est dans le projet : c'est l'état de l'application, partagé par tous les
//! onglets de projets (`onglets_tests::PARTAGES`).
//!
//! Le détail de chaque fonction est dans son fichier : `concentration.rs`, `conseils.rs`, `aidetouches.rs`,
//! `reglagestylet.rs` ; la taille de l'interface est dans `echelle.rs`.

use crate::conseils::Conseil;

#[derive(Default)]
pub(crate) struct Confort {
    /// Le mode concentration est allumé : seuls le dessin et une barre minimale se montrent. C'est une couche posée
    /// par-dessus les panneaux, qui ne touche à aucune disposition : l'éteindre rend exactement ce qu'on avait.
    pub concentration: bool,
    /// Les conseils d'un seul passage peuvent s'afficher (vrai dans l'application lancée par l'utilisateur ; faux
    /// pour les essais, qui ne veulent aucune bulle par-dessus l'écran sauf s'ils la demandent).
    pub conseils_actifs: bool,
    /// Le conseil à l'écran (jamais deux à la fois).
    pub conseil: Option<Conseil>,
    /// L'aide-mémoire des raccourcis est montrée : sa touche est tenue.
    pub aide_touches: bool,
    /// La fenêtre « Régler le stylet à ma main ».
    pub stylet: Option<crate::reglagestylet::Reglage>,
    /// La courbe de pression d'avant le dernier « Appliquer » : la fenêtre se ferme à l'application, « Rétablir » la rend à
    /// l'ouverture suivante.
    pub stylet_avant: Option<crate::brush::Curve>,
    /// La palette de mélange du panneau Couleur (`melangeur.rs`) : au poste, pas au projet ; lue dans le dossier des
    /// réglages à la première ouverture du volet (`palette_ouverte`, replié d'office).
    pub palette: Option<crate::melangeur::PaletteMelange>,
    pub palette_ouverte: bool,
    /// Les images de l'accueil : le tirage de l'apparition en cours et les miniatures des Préférences (`accueilimages.rs`).
    pub accueil: crate::accueilimages::AccueilImages,
}

impl Confort {
    pub(crate) fn new(reel: bool) -> Self {
        Confort { conseils_actifs: reel, ..Default::default() }
    }
}

impl crate::app::App {
    /// Les commandes de ce chantier. Vrai si `action` en était une.
    pub(crate) fn confort_action(&mut self, action: &str) -> bool {
        match action {
            "vue.concentration" => self.basculer_concentration(),
            // Tenue : la carte est dessinée tant que la touche est enfoncée (`aidetouches.rs`), l'appui lui-même ne fait rien.
            "aide.aide_memoire" => {}
            "aide.conseils" => self.revoir_les_conseils(),
            "aide.regler_stylet" => self.ouvrir_reglage_stylet(),
            _ => return false,
        }
        true
    }
}
