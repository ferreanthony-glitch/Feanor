//! SHOT démarre **sans projet ouvert** (Anthony, 2026-10-29, premier lancement au bureau : « le logiciel se lance
//! toujours avec un projet nouveau déjà ouvert et propose en même temps de créer ou d'ouvrir un projet. Il faut arbitrer
//! car si l'on choisit nouveau projet, alors on se retrouve avec deux nouveaux projets »).
//!
//! L'application garde toujours un document en mémoire (des dizaines de milliers de lignes écrivent `self.doc`) : « sans
//! projet » est donc un état, pas l'absence du document. Il se reconnaît à `sans_projet` : l'onglet affiché est celui de
//! l'accueil (`welcome`), sans fichier, sans changement, sans étape d'annulation. Dans cet état :
//! - la fenêtre est **l'accueil compact** (ccueilfenetre.rs, ccueil.rs) : une petite fenêtre qui ne montre que la page d'accueil
//!   (bandeau, formulaire « Nouveau projet », projets récents), sans menus ni barre d'état ni panneaux. Pendant qu'un projet arrive
//!   (ouverture, séance qui revient), un fond neutre ; avec plusieurs onglets, la bande des onglets et les menus restent, et la page
//!   remplit la zone de travail ;
//! - ni barre de commandes, d'outils ou de panneaux, ni panneaux ; les menus qui parlent d'un projet sont grisés (quand ils sont là) ;
//! - **aucune commande qui vise un projet ne part** : `run_action` laisse passer seulement `action_sans_projet` (créer,
//!   ouvrir, importer d'un scénario, réglages, aide, onglets, quitter), qu'elle vienne d'un raccourci,
//!   d'un menu, de la palette Ctrl+K ou d'un bouton. Le document caché ne change donc jamais, l'onglet reste « sans projet »,
//!   et fermer SHOT ne demande rien ;
//! - des fichiers lâchés sur la fenêtre : seuls les projets (`.shot`, `.pelure`) sont pris.
//!
//! Un projet qui arrive (Nouveau, Ouvrir, récents, importé, argument de la ligne de commande, séance rouverte, copie de
//! secours) prend la place de l'onglet sans projet (`Onglets::charger_projet`) : jamais deux projets. Fermer le dernier
//! projet remet cet état (`fermer_l_onglet_affiche`) et retient que la séance s'est finie ainsi (`Settings::session_vide`) :
//! « Rouvrir le dernier projet au lancement » ne le rouvre pas contre sa volonté.

use std::path::Path;

use eframe::egui;

use crate::app::App;

/// Le message de la barre d'état au lancement. Il parle de dessiner un plan : sans projet ouvert, la barre d'état ne le montre pas.
pub(crate) const STATUT_DE_DEPART: &str = "Storyboard : dessine le premier plan ; P pour un nouveau plan, double-clic sur un plan pour l'animer (F1 : raccourcis).";

/// Les commandes qui n'ont pas besoin d'un projet ouvert : créer, ouvrir, importer de quoi faire un projet, les réglages et
/// l'aide, les onglets (un onglet sans projet peut voisiner avec des projets) et quitter.
pub(crate) fn action_sans_projet(action: &str) -> bool {
    matches!(
        action,
        "fichier.nouveau"
            | "fichier.ouvrir"
            | "fichier.quitter"
            | "fichier.preferences"
            | "fichier.importer_scenario"
            | "fichier.fermer"
            | "onglet.nouveau"
            | "onglet.suivant"
            | "onglet.precedent"
            | "onglet.fermer_autres"
            | "aide.palette"
            | "aide.raccourcis"
            | "aide.a_propos"
            | "aide.diagnostic"
            | "aide.conseils"
            | "aide.regler_stylet"
            | "vue.plein_ecran"
            | "vue.performance"
            // (Le mode concentration dit lui-même qu'il attend un projet : `confort.rs`.)
            | "vue.concentration"
    )
}

/// Un fichier qu'on peut lâcher sur SHOT quand aucun projet n'est ouvert : un projet, d'où qu'il vienne.
pub(crate) fn est_un_projet_a_ouvrir(path: &Path) -> bool {
    crate::project::est_un_projet(path)
}

impl App {
    /// Aucun projet n'est ouvert dans l'onglet affiché : l'accueil, sur un fond neutre (voir l'en-tête de ce fichier).
    pub(crate) fn sans_projet(&self) -> bool {
        self.welcome && self.path.is_none() && self.import_source.is_none() && !self.modified && self.history.is_empty()
    }

    /// Cette commande est-elle sans effet parce qu'aucun projet n'est ouvert ?
    pub(crate) fn action_bloquee(&self, action: &str) -> bool {
        self.sans_projet() && !action_sans_projet(action)
    }

    /// Fichiers lâchés sur la fenêtre sans projet ouvert : seuls les projets s'ouvrent, le reste est ignoré (rien ne se
    /// pose dans le document caché).
    pub(crate) fn deposer_sans_projet(&mut self, ctx: &egui::Context) {
        let paths: Vec<_> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        let mut ignores = 0;
        for path in paths {
            if est_un_projet_a_ouvrir(&path) {
                self.import_dropped_project(&path);
            } else {
                ignores += 1;
            }
        }
        if ignores > 0 {
            self.status = "Crée ou ouvre d'abord un projet : SHOT n'a rien où poser ce fichier.".into();
        }
    }

    /// La zone de travail sans projet, le temps qu'un projet arrive : un fond neutre et calme (la page d'accueil, `accueil.rs`, la remplace sinon).
    pub(crate) fn zone_vide(&mut self, ctx: &egui::Context) {
        let cadre = egui::Frame::new().fill(crate::theme::fond());
        egui::CentralPanel::default().frame(cadre).show(ctx, |_| {});
    }
}
