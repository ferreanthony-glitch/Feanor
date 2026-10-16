//! Réglages de l'application (pas du projet), dans %APPDATA%\Pelure\reglages.toml.

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
}

fn default_autosave() -> u32 {
    1
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
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("Pelure").join("reglages.toml"))
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
