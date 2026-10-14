//! Espaces de travail (4b) : quels panneaux sont affichés et à quelle taille. Quatre dispositions
//! de Pelure (leurs commandes reprennent par le nom celles de Storyboard Pro 27, sans touche ;
//! leur contenu est propre à Pelure), les espaces enregistrés par l'utilisateur, et la
//! disposition en cours, retenue à la fermeture (AUTO_SAVE_LAYOUT = Y chez Anthony).

use serde::{Deserialize, Serialize};

/// Disposition de la fenêtre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
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
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            tools: true,
            sheet: true,
            sheet_width: 250.0,
            strip: true,
            montage_height: 290.0,
            timeline_height: 210.0,
            vignettes: false,
            top_view: false,
        }
    }
}

impl Layout {
    /// Tailles ramenées dans des bornes raisonnables (fichier modifié à la main…).
    pub fn sanitized(mut self) -> Layout {
        self.sheet_width = self.sheet_width.clamp(160.0, 900.0);
        self.montage_height = self.montage_height.clamp(140.0, 1200.0);
        self.timeline_height = self.timeline_height.clamp(110.0, 1200.0);
        self
    }
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
