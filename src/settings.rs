//! Réglages de l'application (pas du projet), dans %APPDATA%\SHOT\reglages.toml.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::onion::OnionSettings;
use crate::workspace::{Layout, Workspace};

#[derive(Serialize, Deserialize)]
pub struct Settings {
    /// ffmpeg choisi à la main (sinon : cherché automatiquement à chaque export).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffmpeg: Option<PathBuf>,
    /// Copie de secours toutes les N minutes (0 = jamais). 1 par défaut, comme
    /// AUTO_SAVE_DELAY dans les préférences Storyboard Pro d'Anthony.
    #[serde(default = "default_autosave", rename = "copie_secours_minutes")]
    pub autosave_minutes: u32,
    /// Croix du système sur le dessin, en plus du cercle du pinceau : Windows la déplace dès que
    /// le stylet bouge, sans attendre l'image suivante (le cercle, dessiné par Pelure, la suit).
    #[serde(default = "yes", rename = "curseur_systeme")]
    pub system_cursor: bool,
    /// Projet modèle de Storyboard Pro (14b) : l'export vers Storyboard Pro part d'une copie de ce
    /// projet vide (résolution, cadence, légendes, réglages), choisi une fois par Anthony.
    #[serde(default, rename = "modele_storyboard_pro", skip_serializing_if = "Option::is_none")]
    pub sboard_template: Option<PathBuf>,
    /// Dossier du dernier export vers Storyboard Pro.
    #[serde(default, rename = "dossier_export_storyboard_pro", skip_serializing_if = "Option::is_none")]
    pub sboard_export_dir: Option<PathBuf>,
    /// Pelure d'oignon.
    #[serde(default, rename = "pelure")]
    pub onion: OnionSettings,
    /// Ce que montre la feuille d'exposition (13g).
    #[serde(default, rename = "feuille_expo")]
    pub xsheet: crate::xsheet::XSheetSettings,
    /// Les flips (13h).
    #[serde(default, rename = "flips")]
    pub flips: crate::flips::FlipSettings,
    /// Couleurs d'avant-plan et d'arrière-plan, curseurs, roue, couleurs récentes et nuanciers
    /// (passe couleur).
    #[serde(default, rename = "couleurs")]
    pub colors: crate::couleur::ColorSettings,
    /// Disposition de la fenêtre à la dernière fermeture.
    #[serde(default, rename = "disposition", skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Espaces de travail enregistrés.
    #[serde(default, rename = "espaces")]
    pub workspaces: Vec<Workspace>,
    /// Largeur de la colonne de gauche du montage (caméra, plans, calques), qu'on étire pour
    /// voir les réglages des calques (Anthony, 2026-10-17).
    #[serde(default = "default_head_w", rename = "largeur_entetes_montage")]
    pub montage_head_w: f32,
    /// Pinceau et crayon : peint derrière, remplir la forme (`brushfill`), retenus d'une séance à
    /// l'autre (Anthony, 2026-10-17).
    #[serde(default, rename = "peint_derriere")]
    pub paint_behind: bool,
    #[serde(default, rename = "remplir_la_forme")]
    pub fill_shapes: bool,
    /// Projets récents, le plus récent d'abord (`recents.rs`), et rouvrir le dernier au lancement.
    #[serde(default, rename = "projets_recents", skip_serializing_if = "Vec::is_empty")]
    pub recent_projects: Vec<PathBuf>,
    #[serde(default = "yes", rename = "rouvrir_dernier_projet")]
    pub reopen_last: bool,
    /// Les panneaux de réglages prennent la hauteur de leur contenu (`ajuste.rs`), sauf entre les
    /// paires (premier panneau du haut, premier du bas) dont on a tiré le bord à la main.
    #[serde(default = "yes", rename = "panneaux_ajustes")]
    pub panes_fit: bool,
    /// La barre des panneaux, sur le bord droit de la fenêtre (`barrepanneaux.rs`).
    #[serde(default = "yes", rename = "barre_panneaux")]
    pub panel_bar: bool,
    #[serde(default, rename = "panneaux_tires_a_la_main", skip_serializing_if = "Vec::is_empty")]
    pub panes_fixed: Vec<(crate::dock::Pane, crate::dock::Pane)>,
    /// Toucher du stylet (sensation des brosses, 2026-10-18) : courbe de pression globale,
    /// plancher de taille, traits fins, bord, seuil d'appui ; commun à tous les pinceaux.
    #[serde(default, rename = "toucher")]
    pub toucher: crate::brush::Toucher,
    /// Panneau Pinceaux (2026-10-18) : l'onglet, liste ou vignettes, la catégorie des réglages et les
    /// dossiers ouverts ou repliés.
    #[serde(default, rename = "panneau_pinceaux")]
    pub brushes_panel: crate::pinceaux::Panneau,
    /// Fenêtre « Nouveau projet » (2026-10-18) : dernières valeurs, dernier dossier et préréglages
    /// de l'utilisateur (`nouveau.rs`).
    #[serde(default, rename = "nouveau_projet")]
    pub new_project: crate::nouveau::NewProjectSettings,
}

fn default_autosave() -> u32 {
    1
}

fn default_head_w() -> f32 {
    crate::soundtrack::HEADERS_W
}

fn yes() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ffmpeg: None,
            autosave_minutes: default_autosave(),
            system_cursor: true,
            sboard_template: None,
            sboard_export_dir: None,
            onion: OnionSettings::default(),
            xsheet: Default::default(),
            flips: Default::default(),
            colors: Default::default(),
            layout: None,
            workspaces: Vec::new(),
            montage_head_w: default_head_w(),
            paint_behind: false,
            fill_shapes: false,
            recent_projects: Vec::new(),
            reopen_last: true,
            panes_fit: true,
            panel_bar: true,
            panes_fixed: Vec::new(),
            toucher: Default::default(),
            brushes_panel: Default::default(),
            new_project: Default::default(),
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        crate::donnees::roaming().map(|d| d.join("reglages.toml"))
    }

    /// Réglages enregistrés ; défauts si le fichier manque ou se lit mal.
    pub fn load() -> Settings {
        let mut s: Settings = Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default();
        s.onion.migrate();
        s
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("dossier %APPDATA% introuvable")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{} : {e}", path.display()))
    }
}
