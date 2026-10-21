//! Espaces de travail (4b) : quels panneaux sont affichés et à quelle taille. Quatre dispositions
//! de Pelure (leurs commandes reprennent par le nom celles de Storyboard Pro 27, sans touche ;
//! leur contenu est propre à Pelure), les espaces enregistrés par l'utilisateur, et la
//! disposition en cours, retenue à la fermeture (AUTO_SAVE_LAYOUT = Y chez Anthony).
//!
//! Depuis 5b (fenêtres modulables), la disposition est l'arbre des panneaux (`panneaux`) ; les
//! champs d'avant (panneaux affichés, tailles en pixels) servent aux dispositions de Pelure et aux
//! espaces enregistrés avant 5b, convertis à l'ouverture (`DockLayout::from_legacy`).

use serde::{Deserialize, Serialize};

use crate::app::Mode;
use crate::dock::{DockLayout, Pane};

/// Disposition de la fenêtre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// Panneaux et leur place (5b) ; absent : tirée des champs ci-dessous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panneaux: Option<DockLayout>,
    /// Panneau Outils (à gauche).
    #[serde(rename = "outils")]
    pub tools: bool,
    /// Fiche du panneau et calques (à droite), et sa largeur.
    #[serde(rename = "fiche")]
    pub sheet: bool,
    #[serde(rename = "largeur_fiche")]
    pub sheet_width: f32,
    /// Bande du bas : montage (storyboard) ou timeline (animation), et leurs hauteurs.
    #[serde(rename = "bande")]
    pub strip: bool,
    #[serde(rename = "hauteur_montage")]
    pub montage_height: f32,
    #[serde(rename = "hauteur_timeline")]
    pub timeline_height: f32,
    /// Vue « Vignettes » du storyboard (sinon « Dessin »).
    pub vignettes: bool,
    /// Fenêtre « Vue de dessus » ouverte.
    #[serde(rename = "vue_dessus")]
    pub top_view: bool,
    /// Barre de commandes en haut et barre d'outils verticale à gauche (6a).
    #[serde(rename = "barre_commandes")]
    pub command_bar: bool,
    #[serde(rename = "barre_outils")]
    pub tool_bar: bool,
    /// Vue Vignettes : largeur des vignettes sur la planche (7).
    #[serde(rename = "taille_vignettes")]
    pub card_size: f32,
    /// Vue Vignettes (8) : colonnes (6 au plus), lignes par page (0 : une seule page), écart,
    /// une ligne par scène, notes nouvelles attachées à la vignette la plus proche.
    #[serde(rename = "colonnes_vignettes")]
    pub board_columns: usize,
    #[serde(rename = "lignes_vignettes")]
    pub board_rows: usize,
    #[serde(rename = "ecart_vignettes")]
    pub board_spacing: f32,
    #[serde(rename = "scene_a_la_ligne")]
    pub board_scene_breaks: bool,
    #[serde(rename = "attacher_notes")]
    pub board_attach: bool,
    /// Cadres de composition sur le dessin (9d) : zones de sécurité, et cadre d'un format de
    /// diffusion (largeur:hauteur ; absent : aucun).
    #[serde(rename = "cadres_securite")]
    pub safe_frames: bool,
    #[serde(rename = "cadre_format", skip_serializing_if = "Option::is_none")]
    pub frame_format: Option<[f32; 2]>,
    /// La barre des personnages au-dessus du dessin (`personnagesui.rs`) dans cet espace de travail : rien, c'est
    /// le choix de l'espace (`Espace::barre_personnages_par_defaut`) ; sinon ce que l'utilisateur y a réglé.
    #[serde(rename = "barre_personnages", skip_serializing_if = "Option::is_none")]
    pub character_bar: Option<bool>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            panneaux: None,
            tools: true,
            sheet: true,
            sheet_width: 230.0,
            strip: true,
            montage_height: 290.0,
            timeline_height: 210.0,
            vignettes: false,
            top_view: false,
            command_bar: true,
            tool_bar: true,
            card_size: 240.0,
            board_columns: 4,
            board_rows: 4,
            board_spacing: 18.0,
            board_scene_breaks: false,
            board_attach: false,
            safe_frames: false,
            frame_format: None,
            character_bar: None,
        }
    }
}

impl Layout {
    /// Tailles ramenées dans des bornes raisonnables (fichier modifié à la main…).
    pub fn sanitized(mut self) -> Layout {
        self.sheet_width = self.sheet_width.clamp(160.0, 900.0);
        self.montage_height = self.montage_height.clamp(140.0, 1200.0);
        self.timeline_height = self.timeline_height.clamp(110.0, 1200.0);
        self.card_size = if self.card_size.is_finite() {
            self.card_size.clamp(crate::board::CARD_MIN, crate::board::CARD_MAX)
        } else {
            240.0
        };
        self.board_columns = self.board_columns.clamp(1, crate::board::MAX_COLUMNS);
        self.board_rows = self.board_rows.min(crate::board::MAX_ROWS);
        self.board_spacing =
            if self.board_spacing.is_finite() { self.board_spacing.clamp(0.0, crate::board::MAX_SPACING) } else { 18.0 };
        self.panneaux = self.panneaux.map(DockLayout::sanitized);
        self.frame_format = self.frame_format.and_then(crate::frames::sanitized);
        self
    }

    /// Réglages de la planche de la vue Vignettes.
    pub fn board_settings(&self) -> crate::board::BoardSettings {
        crate::board::BoardSettings {
            card_w: self.card_size,
            columns: self.board_columns.clamp(1, crate::board::MAX_COLUMNS),
            rows: self.board_rows.min(crate::board::MAX_ROWS),
            spacing: self.board_spacing,
            scene_breaks: self.board_scene_breaks,
        }
    }

    /// Les panneaux et leur place, mis à jour s'ils viennent d'une version précédente
    /// (`DockLayout::upgraded`).
    pub fn dock_layout(&self) -> DockLayout {
        match &self.panneaux {
            Some(d) if d.version < crate::dock::LAYOUT_VERSION => d.clone().upgraded(),
            Some(d) => d.clone().sanitized(),
            None => DockLayout::from_legacy(self),
        }
    }

    /// Disposition enregistrée avant 6a (on lui préfère la nouvelle disposition par défaut).
    pub fn is_before_6a(&self) -> bool {
        self.panneaux.as_ref().is_some_and(|d| d.version < 1)
    }

    /// Cette disposition (celle d'un espace de travail) avec les réglages de la vue de `vue`, la
    /// disposition à l'écran : les cadres de composition et la planche des vignettes sont des choix
    /// de l'utilisateur, pas d'un espace ; seuls les panneaux et les barres changent d'un espace à
    /// l'autre.
    pub fn avec_les_reglages_de(self, vue: &Layout) -> Layout {
        Layout {
            card_size: vue.card_size,
            board_columns: vue.board_columns,
            board_rows: vue.board_rows,
            board_spacing: vue.board_spacing,
            board_scene_breaks: vue.board_scene_breaks,
            board_attach: vue.board_attach,
            safe_frames: vue.safe_frames,
            frame_format: vue.frame_format,
            ..self
        }
    }

    /// Une disposition lue dans les réglages, mise au goût du jour : celle d'avant 6a (barre d'outils
    /// verticale, calques à côté du dessin) devient la disposition par défaut, en gardant les
    /// panneaux qu'elle montrait.
    pub fn modernisee(self) -> Layout {
        if !self.is_before_6a() {
            return self;
        }
        let d = self.dock_layout();
        Layout {
            tools: d.contains(Pane::ToolOptions),
            sheet: d.contains(Pane::Layers) || d.contains(Pane::Sheet),
            strip: d.contains(Pane::Montage),
            panneaux: None,
            ..Layout::default()
        }
    }
}

/// Hauteur de la bande du montage dans l'espace Montage (points de la fenêtre de référence) : celle
/// du storyboard est de 290, celle de l'animation de 340.
const MONTAGE_BANDE: f32 = 440.0;

/// Les trois espaces de travail, un onglet chacun en haut à droite (INTERFACE.md, 6.1 et 14).
/// Chacun retient sa propre disposition : passer en Animation ouvre la disposition d'animation,
/// revenir en Storyboard remet celle du storyboard, et ce que l'utilisateur change dans l'une ne
/// touche pas les autres.
///
/// « Montage » n'est pas un troisième mode : il garde les raccourcis, les outils et le dessin du
/// storyboard (`Mode::Storyboard`) ; seules diffèrent sa disposition (le lecteur du film en haut, la
/// bande avec toutes les pistes en grand en bas, les panneaux de dessin repliés) et sa barre de
/// commandes (`Barre::MontageEdition`). On y va par son onglet, Alt+M, Affichage > Espaces de
/// travail ou la palette (Ctrl+K) ; Tab en revient au storyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Espace {
    #[default]
    #[serde(rename = "storyboard")]
    Storyboard,
    #[serde(rename = "animation")]
    Animation,
    #[serde(rename = "montage")]
    Montage,
}

impl Espace {
    pub const TOUS: [Espace; 3] = [Espace::Storyboard, Espace::Animation, Espace::Montage];

    pub fn nom(self) -> &'static str {
        match self {
            Espace::Storyboard => "Storyboard",
            Espace::Animation => "Animation",
            Espace::Montage => "Montage",
        }
    }

    /// La barre des personnages (`personnagesui.rs`) est-elle montrée d'office dans cet espace ? Oui en Storyboard
    /// (et dans l'espace Vignettes, quand il viendra : le croquis rapide est son travail) ; non en Animation et en
    /// Montage, où l'on peut l'allumer (Affichage > Barre des personnages) ; l'espace retient son choix.
    pub fn barre_personnages_par_defaut(self) -> bool {
        match self {
            Espace::Storyboard => true,
            Espace::Animation | Espace::Montage => false,
        }
    }

    /// L'action qui y mène (`keymap::ACTIONS`).
    pub fn action(self) -> &'static str {
        match self {
            Espace::Storyboard => "espace.storyboard",
            Espace::Animation => "espace.animation",
            Espace::Montage => "espace.montage",
        }
    }

    pub fn de_action(action: &str) -> Option<Espace> {
        Espace::TOUS.into_iter().find(|e| e.action() == action)
    }

    /// Le mode dans lequel l'espace se voit.
    pub fn mode(self) -> Mode {
        match self {
            Espace::Animation => Mode::Animation,
            Espace::Storyboard | Espace::Montage => Mode::Storyboard,
        }
    }

    /// Sa disposition d'origine (« Rétablir la disposition », et la première fois qu'on y va).
    /// - Storyboard : la disposition standard (le dessin, les calques et les effets, les réglages et
    ///   la fiche du plan, le montage en bas).
    /// - Animation : la même, avec la bande (la timeline) plus haute et les calques plus larges.
    /// - Montage : le travail de montage (`DockLayout::pour_le_montage`). Le lecteur du film en grand
    ///   en haut, vu à travers la caméra ; la bande avec toutes ses pistes en bas, plus haute que
    ///   celle du storyboard ; à droite la fiche du plan, et en onglets les bruitages, les
    ///   suggestions sonores, les commentaires et l'historique. Ni calques, ni effets, ni réglages
    ///   de l'outil, ni couleur, ni pinceaux, ni guides, ni barre d'outils verticale.
    ///
    /// Les Références et la feuille d'exposition restent à la demande partout (barre des panneaux) ;
    /// la feuille d'exposition n'existe qu'en animation.
    pub fn defaut(self) -> Layout {
        let standard = Layout::default();
        match self {
            Espace::Storyboard => standard,
            Espace::Animation => Layout { montage_height: 340.0, sheet_width: 250.0, ..standard },
            Espace::Montage => Layout {
                tools: false,
                sheet: true,
                montage_height: MONTAGE_BANDE,
                tool_bar: false,
                panneaux: Some(DockLayout::pour_le_montage(MONTAGE_BANDE)),
                ..standard
            },
        }
    }
}

/// La disposition de l'espace Montage telle qu'elle était avant son onglet : la bande en grand, la
/// fiche du plan à droite, les bruitages et les suggestions sonores. Une disposition retenue qui n'a
/// pas bougé depuis est remplacée par la nouvelle (`Settings::load`) ; une disposition que
/// l'utilisateur a changée reste la sienne.
fn montage_d_avant_l_onglet() -> Layout {
    let base = Layout { tools: false, sheet: true, montage_height: 420.0, ..Layout::default() };
    let mut dock = DockLayout::from_legacy(&Layout { sheet: false, ..base.clone() });
    dock.open(Pane::Sheet);
    dock.open(Pane::Bruitages);
    dock.open(Pane::SuggestionsSonores);
    Layout { panneaux: Some(dock), ..base }
}

/// `layout` est la disposition d'origine de l'espace Montage d'avant son onglet, inchangée.
pub fn est_le_montage_d_avant(layout: &Layout) -> bool {
    layout.dock_layout().close_to(&montage_d_avant_l_onglet().dock_layout())
}

/// Un espace de travail nommé.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "disposition")]
    pub layout: Layout,
}

/// Dispositions de Pelure : (action, nom, disposition).
pub fn builtin() -> Vec<(&'static str, &'static str, Layout)> {
    let standard = Layout::default();
    vec![
        ("espace.standard", "Standard", standard.clone()),
        (
            "espace.dessin",
            "Dessin",
            Layout {
                sheet: false,
                montage_height: 160.0,
                timeline_height: 130.0,
                ..standard.clone()
            },
        ),
        (
            "espace.timeline",
            "Timeline",
            Layout {
                montage_height: 440.0,
                timeline_height: 380.0,
                ..standard.clone()
            },
        ),
        (
            "espace.ensemble",
            "Vue d'ensemble",
            Layout {
                tools: false,
                vignettes: true,
                ..standard
            },
        ),
    ]
}

/// Ajoute ou remplace (même nom, sans tenir compte des majuscules) un espace enregistré.
pub fn save_workspace(list: &mut Vec<Workspace>, name: &str, layout: Layout) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    match list.iter_mut().find(|w| w.name.to_lowercase() == name.to_lowercase()) {
        Some(w) => w.layout = layout,
        None => list.push(Workspace {
            name: name.to_string(),
            layout,
        }),
    }
}
