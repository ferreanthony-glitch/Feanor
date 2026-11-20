//! Les panneaux de côté, leurs commandes et la règle de leur clic. Ce fut la barre des panneaux (2026-10-18) : une fine bande
//! d'icônes sur le bord droit de la fenêtre, une icône par panneau de côté, qui l'ouvrait, le mettait devant ou le fermait
//! (partie 160 : elle servait d'onglets). Depuis la partie 194, la colonne est une pile de panneaux et **Propriétés montre ses
//! onglets en icônes verticales** (`colonneui.rs`) : la barre n'est plus dessinée. Restent ici ce qu'elle portait et qui sert
//! toujours : chaque panneau lié à une action (`vue.fiche`…), son nom et sa description (les bulles des onglets de Propriétés et
//! du menu « + Panneau »), les panneaux qui servent à chaque espace de travail, et la règle du clic de ces actions (fermé :
//! l'ouvrir ; caché : le montrer ; visible : le fermer), raccourcis et palette (Ctrl+K) compris.

use egui_dock::{DockState, Node};

use crate::app::App;
use crate::dock::Pane;
use crate::workspace::Espace;

/// Une icône de la barre.
pub(crate) struct Entree {
    pub pane: Pane,
    /// L'action qui ouvre, met devant ou ferme ce panneau (`keymap::ACTIONS`).
    pub action: &'static str,
    pub nom: &'static str,
    pub description: &'static str,
}

impl Entree {
    /// L'icône se voit-elle dans ce mode ? La feuille d'exposition n'existe qu'en animation.
    pub(crate) fn visible(&self, animation: bool) -> bool {
        animation || !self.pane.animation_only()
    }
}

/// Les icônes de la barre dans un mode, en groupes (un groupe vide n'est pas rendu).
#[cfg(test)]
pub(crate) fn icones(animation: bool) -> Vec<Vec<&'static Entree>> {
    GROUPES.iter().map(|g| g.iter().filter(|e| e.visible(animation)).collect::<Vec<_>>()).filter(|g| !g.is_empty()).collect()
}

const fn entree(pane: Pane, action: &'static str, nom: &'static str, description: &'static str) -> Entree {
    Entree { pane, action, nom, description }
}

/// Les icônes, en groupes séparés par un trait : le dessin, le plan, puis l'historique.
pub(crate) const GROUPES: &[&[Entree]] = &[
    &[
        entree(Pane::ToolOptions, "vue.reglages", "Réglages de l'outil", "Taille, opacité et options de l'outil courant. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Color, "vue.couleur", "Couleur", "Roue, curseurs, couleurs récentes et nuanciers. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Personnages, "vue.personnages", "Brosses dédiées", "Les brosses dédiées : personnages du projet, classiques et décors, chacune avec sa couleur de trait et son aplat. Un clic l'ouvre ou le met devant, un autre le ferme."),
        // Le panneau « Espace 3D » (`espace3dpanneau.rs`, parties 150 et 152) : un onglet du groupe des réglages de l'outil.
        entree(Pane::Espace3d, "vue.espace3d_panneau", "Panneau Espace 3D","Les réglages de la vue Espace 3D : les cartes (les calques du plan) et la caméra du plan. Un onglet à côté de Réglages : il passe devant quand la vue s'ouvre, et Réglages revient devant quand elle se ferme. Un clic l'ouvre ou le met devant, un autre le ferme."),
    ],
    &[
        // (Plus d'icône Calques, partie 160 : les calques sont dans la colonne de la timeline ; l'action ue.calques la montre.)
        // (Partie 197 : la Scène en est la vue en arbre, du film jusqu'aux calques ; elle ne remplace pas la colonne.)
        entree(Pane::Scene, "vue.scene", "Scène", "Du film jusqu'aux calques : les séquences, les plans et leurs calques en arbre, comme la colonne de la timeline mais sans le temps. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Effects, "fx.panneau", "Effets", "La pile d'effets du calque courant. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Sheet, "vue.fiche", "Fiche du plan", "Textes, durée et caméra du plan. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::XSheet, "vue.feuille_exposition", "Feuille d'exposition", "Le plan image par image, calque par calque. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Prereglages, "vue.prereglages", "Préréglages", "Les mouvements de caméra tout faits, en vignettes qui bougent au survol : les glisser sur la piste Caméra de la timeline, ou cliquer pour les poser à la tête de lecture. Un clic l'ouvre ou le met devant, un autre le ferme."),
    ],
    &[
        entree(Pane::Scenario, "vue.scenario", "Scénario", "Le scénario importé, lisible comme un scénario ; le passage du plan courant est surligné, un clic sur une ligne mène au plan. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Comments, "vue.commentaires", "Commentaires", "Les commentaires laissés sur la timeline : la liste, et le texte, l'auteur et la couleur du fanion choisi. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::History, "vue.historique", "Historique", "Les étapes d'annulation, par leur nom. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Bruitages, "vue.bruitages", "Bruitages", "Tes dossiers de sons : les chercher, les écouter et les poser dans le montage. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::References, "vue.references", "Références", "Un tableau d'images et de vidéos à regarder en dessinant ; se détache dans sa fenêtre pour un deuxième écran. Un clic l'ouvre ou le met devant, un autre le ferme."),
        entree(Pane::Bibliotheque, "vue.bibliotheque", "Bibliothèque", "Explorer tes dossiers et voir, lire ou écouter tes images, vidéos et sons dans un aperçu ; les glisser dans le projet. Un clic l'ouvre ou le met devant, un autre le ferme."),
    ],
];

/// Les panneaux que la barre propose d'office dans un espace de travail : ceux qui servent à ce travail-là
/// (simplification des barres, 2026-10-21). Les autres restent à la palette (Ctrl+K) et au menu Affichage >
/// Espaces de travail > Panneaux ; un panneau ouvert à la main, lui, a son icône tant qu'il est ouvert, et comme
/// chaque espace retient sa disposition, il la retrouve la fois suivante : la barre ne se bat jamais contre
/// l'utilisateur.
/// - Vignettes : « rien de distraction, que du dessin et des références » : les réglages (avec la liste des pinceaux),
///   la couleur, les références.
/// - Storyboard : dessiner (réglages avec la liste des pinceaux, couleur, personnages), les calques et leurs effets, la
///   fiche du plan, les références.
/// - Animation : comme le storyboard, sans les personnages, avec la feuille d'exposition.
/// - Montage : la fiche du plan, les bruitages, les commentaires et l'historique.
/// - Storyboard, Animation et Montage : la bibliothèque de fichiers (explorer, aperçu des images, vidéos et sons).
/// - Storyboard et Animation : le panneau Espace 3D (les réglages de la vue Espace 3D), qui est dans leur disposition
///   d'origine, à côté de la couleur.
/// - Storyboard et Animation : le panneau Préréglages (partie 170), les mouvements de caméra à glisser sur la timeline.
/// - Storyboard et Animation : le panneau Scène (partie 197), du film jusqu'aux calques.
///
/// Les guides n'y sont plus : ils sont dans le menu de la règle, au-dessus du dessin (`cadrageui.rs`).
pub(crate) fn panneaux_d_office(espace: Espace) -> &'static [Pane] {
    match espace {
        Espace::Vignettes => &[Pane::Vignettes, Pane::ToolOptions, Pane::Color, Pane::Scenario, Pane::References],
        Espace::Storyboard => &[Pane::Scene, Pane::ToolOptions, Pane::Color, Pane::Personnages, Pane::Espace3d, Pane::Effects, Pane::Sheet, Pane::Prereglages, Pane::Scenario, Pane::References, Pane::Bibliotheque],
        Espace::Animation => &[Pane::Scene, Pane::ToolOptions, Pane::Color, Pane::Espace3d, Pane::Effects, Pane::Sheet, Pane::XSheet, Pane::Prereglages, Pane::References, Pane::Bibliotheque],
        Espace::Montage => &[Pane::Sheet, Pane::Bruitages, Pane::Comments, Pane::History, Pane::Bibliotheque],
    }
}

/// Les icônes de la barre dans un espace de travail, en groupes (un groupe vide n'est pas rendu) : les panneaux
/// d'office de l'espace et ceux qui sont ouverts (`ouvert`), sans la feuille d'exposition hors animation.
pub(crate) fn icones_de(espace: Espace, animation: bool, ouvert: &dyn Fn(Pane) -> bool) -> Vec<Vec<&'static Entree>> {
    let d_office = panneaux_d_office(espace);
    let mut groupes: Vec<Vec<&'static Entree>> = GROUPES
        .iter()
        .map(|g| g.iter().filter(|e| e.visible(animation) && (d_office.contains(&e.pane) || ouvert(e.pane))).collect::<Vec<_>>())
        .filter(|g| !g.is_empty())
        .collect();
    // Le panneau « Vignettes » : une icône à part, en tête, dans l'espace Vignettes (ou tant qu'il est ouvert ailleurs).
    let vignettes: Vec<&'static Entree> = HORS_BARRE.iter().filter(|e| e.pane == Pane::Vignettes && (d_office.contains(&e.pane) || ouvert(e.pane))).collect();
    if !vignettes.is_empty() {
        groupes.insert(0, vignettes);
    }
    groupes
}

/// Les panneaux qui s'ouvrent et se ferment comme ceux de la barre, sans icône dans la barre : ceux
/// des fonctions à l'essai, rangés dans le menu Labo (Anthony, 2026-10-20).
pub(crate) const HORS_BARRE: &[Entree] = &[
    entree(
        Pane::SuggestionsSonores,
        "vue.suggestions_sonores",
        "Suggestions sonores",
        "FEANOR lit le storyboard et propose des bruitages, des ambiances et des voix témoins, que tu écoutes, acceptes ou supprimes. Un clic l'ouvre ou le met devant, un autre le ferme.",
    ),
    // Le panneau « Planche » a son icône dans l'espace Vignettes seulement (`icones_de`), pas dans les douze de la barre.
    entree(
        Pane::Vignettes,
        "vue.vignettes_panneau",
        "Planche",
        "Le sens de lecture de la planche (en lignes, ou à la japonaise) et les textes montrés à côté des cases. Un clic l'ouvre ou le met devant, un autre le ferme.",
    ),
    // « Mon panneau » (essai du menu Labo, `monpanneau.rs`) : pas d'icône dans la barre, comme les suggestions sonores.
    entree(
        Pane::MonPanneau,
        "vue.mon_panneau",
        "Mon panneau (essai)",
        "Tes commandes, tes brosses et tes outils perso rangés en un seul panneau. Un clic l'ouvre ou le met devant, un autre le ferme.",
    ),
];

/// L'icône (de la barre ou du menu Labo) qu'une action ouvre ou ferme.
pub(crate) fn entree_de_action(action: &str) -> Option<&'static Entree> {
    GROUPES.iter().flat_map(|g| g.iter()).chain(HORS_BARRE).find(|e| e.action == action)
}

/// L'entrée d'un panneau (son action, son nom, sa description), s'il en a une.
pub(crate) fn entree_de_pane(pane: Pane) -> Option<&'static Entree> {
    GROUPES.iter().flat_map(|g| g.iter()).chain(HORS_BARRE).find(|e| e.pane == pane)
}

/// Le panneau qu'une action ouvre ou ferme, si c'en est une de la barre (ou du menu Labo).
pub(crate) fn pane_de_action(action: &str) -> Option<Pane> {
    entree_de_action(action).map(|e| e.pane)
}

/// Ce que fait un clic sur l'icône d'un panneau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Clic {
    /// Fermé : on l'ouvre, devant les onglets de son groupe.
    Ouvrir,
    /// Ouvert mais caché derrière un autre onglet : on le met devant.
    Avancer,
    /// Ouvert et visible : on le ferme.
    Fermer,
}

/// La règle du clic : `ouvert` : le panneau est dans l'espace de travail ; `devant` : on le voit
/// (onglet actif de son groupe, ou fenêtre à part).
pub(crate) fn regle_clic(ouvert: bool, devant: bool) -> Clic {
    match (ouvert, devant) {
        (false, _) => Clic::Ouvrir,
        (true, false) => Clic::Avancer,
        (true, true) => Clic::Fermer,
    }
}

/// `pane` est-il l'onglet actif de son groupe dans `state` ? (Faux s'il n'y est pas.)
pub(crate) fn est_devant(state: &DockState<Pane>, pane: Pane) -> bool {
    match state.find_tab(&pane) {
        Some((surface, node, tab)) => matches!(&state[surface][node], Node::Leaf { active, .. } if *active == tab),
        None => false,
    }
}

/// Ce que l'icône d'un panneau montre dans la barre (partie 160 : elle remplace les onglets écrits).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EtatIcone {
    /// Fermé : l'icône est plate.
    Ferme,
    /// Ouvert, caché derrière un autre onglet de son groupe : plate, avec un petit point.
    Derriere,
    /// Ouvert et visible : allumée.
    Devant,
}

impl App {
    /// Le panneau `pane` se voit-il (onglet actif de son groupe, ou fenêtre à part) ? Sans disposition encore, rien n'est caché.
    pub(crate) fn panneau_devant(&self, pane: Pane) -> bool {
        if self.detached.iter().any(|d| d.panneau == pane) {
            return true;
        }
        // Dans la pile de la colonne (partie 194) : devant dans son panneau (l'onglet devant de Propriétés), le panneau déplié.
        if self.dock.is_some() && self.colonne.contient(pane) {
            return self.colonne.devant(pane);
        }
        self.dock.as_ref().is_none_or(|s| est_devant(s, pane))
    }

    /// L'état de `pane` (ce que montrait son icône dans l'ancienne barre des panneaux ; depuis la partie 194, son onglet de
    /// Propriétés allumé ou non) : fermé, caché (un autre onglet devant, son panneau replié) ou visible.
    pub(crate) fn etat_dans_la_barre(&self, pane: Pane) -> EtatIcone {
        match (self.pane_shown(pane), self.panneau_devant(pane)) {
            (false, _) => EtatIcone::Ferme,
            (true, false) => EtatIcone::Derriere,
            (true, true) => EtatIcone::Devant,
        }
    }

    /// Ouvre, met devant ou ferme `pane` selon `regle_clic` (son action : menu, raccourci, palette).
    pub(crate) fn clic_panneau(&mut self, pane: Pane) {
        let ouvert = self.pane_shown(pane);
        // Une fenêtre à part se voit toujours ; sans disposition encore, rien n'est caché.
        let devant = self.panneau_devant(pane);
        match regle_clic(ouvert, devant) {
            Clic::Ouvrir | Clic::Fermer => self.toggle_pane(pane),
            Clic::Avancer => self.show_pane(pane),
        }
    }
}
