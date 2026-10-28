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
    /// La fiche technique (`fiche.rs`, `ficheui.rs`) : colonnes choisies et leur ordre, format, séparateur, time
    /// code de départ, dernier dossier.
    #[serde(default, rename = "fiche_technique")]
    pub fiche: crate::fiche::Reglages,
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
    /// Disposition de l'espace Vignettes (le quatrième espace, `Espace::Vignettes`) ; absente : son origine.
    #[serde(default, rename = "disposition_vignettes", skip_serializing_if = "Option::is_none")]
    pub vignettes_layout: Option<Layout>,
    /// Le panneau « Vignettes » (2026-10-24) a été posé, une fois, dans la disposition retenue de l'espace Vignettes
    /// (`poser_le_panneau_vignettes`) : une disposition d'avant lui ne l'avait pas, et il se voit avec l'onglet. Fermé ensuite à la main,
    /// il reste fermé.
    #[serde(default, rename = "panneau_vignettes_pose", skip_serializing_if = "std::ops::Not::not")]
    pub panneau_vignettes_pose: bool,
    /// « Avec les annotations » des fenêtres d'export (la planche des Vignettes, les images, la vidéo, le GIF, le storyboard PDF) : coché,
    /// les calques d'annotation (de la planche et des plans) sont dans l'export. Décoché d'origine ; retenu par poste de travail, d'une séance à
    /// l'autre (`calqueannot.rs`).
    #[serde(default, rename = "export_avec_annotations", skip_serializing_if = "std::ops::Not::not")]
    pub export_annotations: bool,
    #[serde(default, rename = "espace_storyboard")]
    pub storyboard_space: Espace,
    /// Commandes épinglées dans la barre de commandes, la barre du montage et celle de la timeline,
    /// en plus ou en moins de celles que chaque barre montre d'office (`barres.rs`).
    #[serde(default, rename = "barres", skip_serializing_if = "ReglageBarres::est_vide")]
    pub bars: ReglageBarres,
    /// Quelles sortes de clés le montage et la timeline montrent (`legendecles.rs`) : un affichage, rien de plus.
    #[serde(default, rename = "cles_vues", skip_serializing_if = "crate::legendecles::FiltreCles::est_complet")]
    pub key_filter: crate::legendecles::FiltreCles,
    /// Espaces de travail enregistrés.
    #[serde(default, rename = "espaces")]
    pub workspaces: Vec<Workspace>,
    /// Largeur de la colonne de gauche du montage (caméra, plans, calques), qu'on étire pour
    /// voir les réglages des calques (Anthony, 2026-10-17).
    #[serde(default = "default_head_w", rename = "largeur_entetes_montage")]
    pub montage_head_w: f32,
    /// Pistes repliables du montage (`pistes.rs`) : le calque courant déplie sa piste tout seul
    /// (les autres gardent leur état).
    #[serde(default = "yes", rename = "deplier_calque_courant")]
    pub montage_auto_open: bool,
    /// Repliage des pistes du montage : commun à tous les plans (d'office) ou propre à chaque plan
    /// (`pistes.rs`, Préférences > Montage). Un réglage de l'application : rien n'est écrit dans le projet.
    #[serde(default, rename = "repliage_pistes", skip_serializing_if = "crate::pistes::ReplisPistes::est_commun")]
    pub montage_folds: crate::pistes::ReplisPistes,
    /// Pinceau et crayon : « Peint sur l'aplat » (`brushfill`, l'ancien « Peint derrière » : la clé garde son nom),
    /// retenu d'une séance à l'autre (Anthony, 2026-10-17).
    #[serde(default, rename = "peint_derriere")]
    pub paint_behind: bool,
    /// L'ancien « Remplir la forme » (retiré le 2026-10-23 : il faisait double emploi avec l'aplat sous le trait). Lu
    /// pour qu'un ancien fichier de réglages s'ouvre, jamais écrit, sans effet.
    #[serde(default, rename = "remplir_la_forme", skip_serializing)]
    #[allow(dead_code)]
    pub ancien_remplir_la_forme: bool,
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
    /// La liste des pinceaux (l'ancien panneau Pinceaux, 2026-10-18, depuis le 2026-10-24 dans les réglages de
    /// l'outil) : dépliée ou non, liste ou vignettes, la catégorie des réglages et les dossiers ouverts ou repliés.
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
    /// Opacité du passe-partout, en pour cent (0 à 100) : ce qui couvre le hors-champ autour du cadre de la caméra
    /// quand « Passe-partout » est allumé (`camtools.rs`, Fichier > Préférences…) ; 100 % : opaque, d'origine.
    /// Migration (2026-10-24) : l'ancien voile translucide gardait son opacité sous la clé `opacite_passe_partout` (27 %
    /// d'origine). Cette clé n'est plus lue : un passe-partout est opaque et noir d'origine, et un ancien fichier ne le
    /// rend pas gris à 27 %. La clé du nouveau réglage est donc autre.
    #[serde(default = "default_passe_partout", rename = "passe_partout_opacite")]
    pub passe_partout: u8,
    /// Le tremblement du trait par défaut (`tremblement.rs`, Préférences > Dessin) : éteint d'origine. Appliqué au rendu à tout
    /// calque vectoriel qui n'a pas le sien ; rien n'est écrit dans les projets.
    #[serde(default, rename = "tremblement_par_defaut", skip_serializing_if = "crate::tremblement::Defaut::est_defaut")]
    pub tremblement_defaut: crate::tremblement::Defaut,
    /// Le passe-partout est allumé (icône de la barre sous le dessin, commande `camera.masque`) : allumé d'origine,
    /// retenu d'une séance à l'autre sur ce poste.
    #[serde(default = "yes", rename = "passe_partout_actif")]
    pub passe_partout_actif: bool,
    /// Couleur du passe-partout (rouge, vert, bleu) : noir d'origine.
    #[serde(default = "default_passe_partout_couleur", rename = "passe_partout_couleur")]
    pub passe_partout_couleur: [u8; 3],
    /// Coller ce qui vient d'un projet d'une autre taille ou d'une autre cadence (`adaptation.rs`, `collage.rs`) : contenir
    /// ou remplir, images ou secondes, et poser la question ou non.
    #[serde(default, rename = "collage")]
    pub collage: crate::adaptation::ReglageCollage,
    /// Travailler à plusieurs sur un film partagé (`equipe.rs`, `cooperationui.rs`) : mon identité d'équipier, la durée d'une
    /// prise sans nouvelles, la prise automatique, les notifications.
    #[serde(default, rename = "equipe")]
    pub equipe: crate::equipe::ReglageEquipe,
    /// Les couleurs neutres de la barre des personnages (`personnages.rs`, `personnagesui.rs`) : un réglage de
    /// l'application, commun à tous les projets (les personnages, eux, sont ceux du projet).
    #[serde(default = "crate::personnages::neutres_par_defaut", rename = "couleurs_neutres")]
    pub neutral_colors: Vec<crate::personnages::Neutre>,
    /// L'onglet ouvert de la fenêtre Préférences… (`prefsui.rs`), retenu d'une séance à l'autre.
    #[serde(default, rename = "onglet_preferences", skip_serializing_if = "crate::prefsonglets::Onglet::est_defaut")]
    pub prefs_onglet: crate::prefsonglets::Onglet,
    /// Les conseils d'un seul passage déjà vus (`conseils.rs`) ; Préférences > Général > « Revoir les conseils » les oublie.
    #[serde(default, rename = "conseils", skip_serializing_if = "crate::conseils::ReglageConseils::est_vide")]
    pub conseils: crate::conseils::ReglageConseils,
    /// La taille de l'interface, en pour cent : 90, 100, 125 ou 150 (`echelle.rs`, Préférences > Apparence).
    #[serde(default = "crate::echelle::defaut", rename = "echelle_interface", skip_serializing_if = "crate::echelle::est_defaut")]
    pub echelle: u16,
    /// Les réglages des Vignettes (`vignettesreglages.rs`, Préférences > Vignettes) : débord du trait, aimant et couleur des
    /// cadres de caméra, « Report du décor », couleur et tailles des brosses du décor. Absents des anciens réglages : ceux d'avant.
    #[serde(default, rename = "vignettes", skip_serializing_if = "crate::vignettesreglages::ReglagesVignettes::est_defaut")]
    pub vignettes: crate::vignettesreglages::ReglagesVignettes,
    /// Le point fixe d'un coin ou d'un côté tiré, dans les boîtes de la Sélection et du Transformer (`selbox.rs`) : la
    /// poignée opposée (d'office) ou le centre. Absent des anciens réglages : la poignée opposée.
    #[serde(default, rename = "point_fixe_poignees", skip_serializing_if = "crate::selbox::PointFixe::est_defaut")]
    pub point_fixe: crate::selbox::PointFixe,
    /// Le centre d'une rotation prise hors d'un coin : le pivot (d'office) ou le coin opposé.
    #[serde(default, rename = "rotation_autour_de", skip_serializing_if = "crate::selbox::CentreRotation::est_defaut")]
    pub rotation_autour: crate::selbox::CentreRotation,
    /// La durée d'un plan neuf, en secondes (`dureeplans.rs`, Préférences > Montage) : 2 s d'origine. Un réglage de l'application,
    /// commun à tous les projets ; seuls les plans créés ensuite l'ont (les plans qui existent gardent leur durée).
    #[serde(default = "crate::dureeplans::secondes_d_origine", rename = "duree_nouveau_plan", skip_serializing_if = "crate::dureeplans::est_d_origine")]
    pub new_plan_secs: f32,
}

fn default_passe_partout() -> u8 {
    crate::camtools::PASSE_PARTOUT_DEFAUT
}

fn default_passe_partout_couleur() -> [u8; 3] {
    crate::camtools::PASSE_PARTOUT_COULEUR
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
            fiche: Default::default(),
            onion: OnionSettings::default(),
            xsheet: Default::default(),
            flips: Default::default(),
            colors: Default::default(),
            layout: None,
            animation_layout: None,
            montage_layout: None,
            vignettes_layout: None,
            panneau_vignettes_pose: false,
            export_annotations: false,
            storyboard_space: Espace::Storyboard,
            bars: ReglageBarres::default(),
            key_filter: Default::default(),
            workspaces: Vec::new(),
            montage_head_w: default_head_w(),
            montage_auto_open: true,
            montage_folds: Default::default(),
            paint_behind: false,
            ancien_remplir_la_forme: false,
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
            tremblement_defaut: Default::default(),
            passe_partout_actif: true,
            passe_partout_couleur: default_passe_partout_couleur(),
            collage: Default::default(),
            equipe: Default::default(),
            neutral_colors: crate::personnages::neutres_par_defaut(),
            prefs_onglet: Default::default(),
            conseils: Default::default(),
            echelle: crate::echelle::ECHELLE_DEFAUT,
            vignettes: Default::default(),
            point_fixe: Default::default(),
            rotation_autour: Default::default(),
            new_plan_secs: crate::dureeplans::secondes_d_origine(),
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
            Espace::Vignettes => self.vignettes_layout.as_ref(),
        }
    }

    /// Retient la disposition de cet espace de travail.
    pub fn set_layout_of(&mut self, espace: Espace, layout: Layout) {
        match espace {
            Espace::Storyboard => self.layout = Some(layout),
            Espace::Animation => self.animation_layout = Some(layout),
            Espace::Montage => self.montage_layout = Some(layout),
            Espace::Vignettes => self.vignettes_layout = Some(layout),
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
        // L'espace Montage a maintenant son propre agencement : l'ancienne disposition d'origine,
        // retenue sans que l'utilisateur y ait touché, cède la place à la nouvelle.
        if s.montage_layout.as_ref().is_some_and(crate::workspace::est_le_montage_d_avant) {
            s.montage_layout = None;
        }
        // (Le côté storyboard n'est jamais l'animation.)
        if s.storyboard_space == Espace::Animation {
            s.storyboard_space = Espace::Storyboard;
        }
        s.generation_son = std::mem::take(&mut s.generation_son).sanitized();
        s.passe_partout = s.passe_partout.min(100);
        s.tremblement_defaut = s.tremblement_defaut.sanitized();
        s.equipe = std::mem::take(&mut s.equipe).nettoye();
        s.neutral_colors = crate::personnages::neutres_nettoyees(std::mem::take(&mut s.neutral_colors));
        s.echelle = crate::echelle::echelle_valide(s.echelle);
        s.vignettes = std::mem::take(&mut s.vignettes).nettoyes();
        s.new_plan_secs = crate::dureeplans::secondes_valides(s.new_plan_secs);
        s.poser_le_panneau_vignettes();
        s
    }

    /// Une fois : le panneau « Vignettes » rejoint la disposition retenue de l'espace Vignettes si elle n'avait pas encore de panneau de ce
    /// nom (elle date d'avant lui). Sans disposition retenue, l'origine du jour l'a déjà. Une fois posé, c'est à l'utilisateur de le garder
    /// ou de le fermer.
    pub fn poser_le_panneau_vignettes(&mut self) {
        if std::mem::replace(&mut self.panneau_vignettes_pose, true) {
            return;
        }
        if let Some(l) = self.vignettes_layout.as_mut() {
            let mut d = l.dock_layout();
            if !d.contains(crate::dock::Pane::Vignettes) {
                d.open(crate::dock::Pane::Vignettes);
                l.panneaux = Some(d);
            }
        }
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
