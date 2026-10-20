//! Réglages de l'application (pas du projet), dans %APPDATA%\SHOT\reglages.toml.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::barres::ReglageBarres;
use crate::onion::OnionSettings;
use crate::workspace::{Espace, Layout, Workspace};

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
    /// le stylet bouge, sans attendre l'image suivante (le cercle, dessiné par SHOT, la suit).
    #[serde(default = "yes", rename = "curseur_systeme")]
    pub system_cursor: bool,
    /// Projet modèle de Storyboard Pro (14b) : l'export vers Storyboard Pro part d'une copie de ce
    /// projet vide (résolution, cadence, légendes, réglages), choisi une fois par Anthony.
    #[serde(default, rename = "modele_storyboard_pro", skip_serializing_if = "Option::is_none")]
    pub sboard_template: Option<PathBuf>,
    /// Dossier du dernier export vers Storyboard Pro.
    #[serde(default, rename = "dossier_export_storyboard_pro", skip_serializing_if = "Option::is_none")]
    pub sboard_export_dir: Option<PathBuf>,
    /// Dossier du dernier export pour le montage (EDL, XML, OTIO : `editorialui.rs`).
    #[serde(default, rename = "dossier_export_montage", skip_serializing_if = "Option::is_none")]
    pub montage_export_dir: Option<PathBuf>,
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
    /// Disposition de l'espace Storyboard (`Espace::Storyboard`), retenue au fil des changements. Le
    /// champ garde son nom d'avant les espaces liés aux modes : l'unique disposition d'un ancien
    /// fichier de réglages devient donc, sans conversion, celle du storyboard.
    #[serde(default, rename = "disposition", skip_serializing_if = "Option::is_none")]
    pub layout: Option<Layout>,
    /// Dispositions des espaces Animation et Montage (absentes : leur disposition d'origine,
    /// `Espace::defaut`), et celui des deux espaces du côté storyboard où l'on était (Storyboard ou
    /// Montage : le retour de l'animation y revient).
    #[serde(default, rename = "disposition_animation", skip_serializing_if = "Option::is_none")]
    pub animation_layout: Option<Layout>,
    #[serde(default, rename = "disposition_montage", skip_serializing_if = "Option::is_none")]
    pub montage_layout: Option<Layout>,
    #[serde(default, rename = "espace_storyboard")]
    pub storyboard_space: Espace,
    /// Commandes épinglées dans la barre de commandes, la barre du montage et celle de la timeline,
    /// en plus ou en moins de celles que chaque barre montre d'office (`barres.rs`).
    #[serde(default, rename = "barres", skip_serializing_if = "ReglageBarres::est_vide")]
    pub bars: ReglageBarres,
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
    /// Onglets de projets (`onglets.rs`) : Ouvrir, Nouveau, projets récents et imports ouvrent un nouvel
    /// onglet (sinon : le projet courant est remplacé, après la question habituelle).
    #[serde(default = "yes", rename = "ouvrir_dans_un_nouvel_onglet")]
    pub open_in_new_tab: bool,
    /// Les projets ouverts dans les onglets à la fin de la séance, dans l'ordre, et l'onglet actif : ils
    /// reviennent au lancement avec « Rouvrir le dernier projet au lancement ».
    #[serde(default, rename = "onglets_ouverts", skip_serializing_if = "Vec::is_empty")]
    pub session_tabs: Vec<PathBuf>,
    #[serde(default, rename = "onglet_actif", skip_serializing_if = "is_zero")]
    pub session_active: usize,
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
    /// Thème de l'interface (2026-10-18) : Gris sombre (par défaut), Gris clair, Sombre ou
    /// Personnalisé avec ses neuf couleurs (`theme.rs`, Fichier > Préférences…).
    #[serde(default, rename = "theme")]
    pub theme: crate::theme::ReglageTheme,
    /// Nom qui signe les commentaires de la timeline (`commentaires.rs`) ; vide : l'utilisateur de
    /// Windows.
    #[serde(default, rename = "auteur_commentaires", skip_serializing_if = "String::is_empty")]
    pub comment_author: String,
    /// Aides au cadrage de la caméra (tiers, grille d'or, spirale, diagonales, croix) et leur
    /// aspect (`cadrage.rs`).
    #[serde(default, rename = "aides_cadrage")]
    pub framing: crate::cadrage::FramingAids,
    /// Réglages de performance (2026-10-19) : effets, traits texturés, caches, fils, vignettes,
    /// animations (`diagnostic.rs`, Fichier > Préférences…, Aide > Diagnostic des performances).
    #[serde(default, rename = "performance")]
    pub perf: crate::diagnostic::PerfReglages,
    /// Ce que le diagnostic des performances a fait : proposé au premier lancement, dernier profil
    /// appliqué et réglages d'avant (« Revenir comme avant »).
    #[serde(default, rename = "diagnostic")]
    pub diagnostic: crate::diagnostic::EtatDiagnostic,
    /// Jeu de raccourcis (2026-10-19) : « SHOT » (par défaut) ou « Façon Storyboard Pro ». Le fichier
    /// personnel de raccourcis joue par-dessus (`keymap.rs`, Fichier > Préférences…, liste des
    /// raccourcis). Absent des anciens réglages : SHOT.
    #[serde(default, rename = "jeu_raccourcis", skip_serializing_if = "crate::keymap::PresetId::is_default")]
    pub keymap_preset: crate::keymap::PresetId,
    /// Les dossiers de sons de la bibliothèque de bruitages (`bruitages.rs`).
    #[serde(default, rename = "bruitages")]
    pub bruitages: crate::bruitages::BruitagesReglages,
    /// La génération sonore (`genreglages.rs`) : pour chaque rôle, local ou service en ligne. Jamais
    /// une clé : elles sont dans le Gestionnaire d'identifiants de Windows (`clessecretes.rs`).
    #[serde(default, rename = "generation_son")]
    pub generation_son: crate::genreglages::GenerationSonReglages,
    /// Opacité du passe-partout, en pour cent (0 à 100) : le voile du hors-champ autour du cadre de la caméra
    /// quand « Estomper le hors-champ » est allumé (`camtools.rs`, Fichier > Préférences…). Absente des anciens
    /// réglages : la valeur d'avant (`camtools::PASSE_PARTOUT_DEFAUT`).
    #[serde(default = "default_passe_partout", rename = "opacite_passe_partout")]
    pub passe_partout: u8,
}

fn default_passe_partout() -> u8 {
    crate::camtools::PASSE_PARTOUT_DEFAUT
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

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ffmpeg: None,
            autosave_minutes: default_autosave(),
            system_cursor: true,
            sboard_template: None,
            sboard_export_dir: None,
            montage_export_dir: None,
            onion: OnionSettings::default(),
            xsheet: Default::default(),
            flips: Default::default(),
            colors: Default::default(),
            layout: None,
            animation_layout: None,
            montage_layout: None,
            storyboard_space: Espace::Storyboard,
            bars: ReglageBarres::default(),
            workspaces: Vec::new(),
            montage_head_w: default_head_w(),
            paint_behind: false,
            fill_shapes: false,
            recent_projects: Vec::new(),
            reopen_last: true,
            open_in_new_tab: true,
            session_tabs: Vec::new(),
            session_active: 0,
            panes_fit: true,
            panel_bar: true,
            panes_fixed: Vec::new(),
            toucher: Default::default(),
            brushes_panel: Default::default(),
            new_project: Default::default(),
            theme: Default::default(),
            comment_author: String::new(),
            framing: Default::default(),
            perf: Default::default(),
            diagnostic: Default::default(),
            keymap_preset: Default::default(),
            bruitages: Default::default(),
            generation_son: Default::default(),
            passe_partout: default_passe_partout(),
        }
    }
}

impl Settings {
    /// La disposition retenue de cet espace de travail, s'il en a une.
    pub fn layout_of(&self, espace: Espace) -> Option<&Layout> {
        match espace {
            Espace::Storyboard => self.layout.as_ref(),
            Espace::Animation => self.animation_layout.as_ref(),
            Espace::Montage => self.montage_layout.as_ref(),
        }
    }

    /// Retient la disposition de cet espace de travail.
    pub fn set_layout_of(&mut self, espace: Espace, layout: Layout) {
        match espace {
            Espace::Storyboard => self.layout = Some(layout),
            Espace::Animation => self.animation_layout = Some(layout),
            Espace::Montage => self.montage_layout = Some(layout),
        }
    }

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
        s.framing = s.framing.sanitized();
        s.perf = s.perf.sanitized();
        s.bars = s.bars.nettoyees();
        // (Le côté storyboard n'est jamais l'animation.)
        if s.storyboard_space == Espace::Animation {
            s.storyboard_space = Espace::Storyboard;
        }
        s.generation_son = std::mem::take(&mut s.generation_son).sanitized();
        s.passe_partout = s.passe_partout.min(100);
        s
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path().ok_or("dossier %APPDATA% introuvable")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        // (Atomique : un plantage ou un disque plein ne laisse pas des réglages tronqués, que `load`
        // remplacerait par les réglages d'origine.)
        crate::project::write_atomic(&path, text.as_bytes())
    }
}
