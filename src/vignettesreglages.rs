//! Les réglages des Vignettes (Préférences > Vignettes, Anthony, 2026-10-22) : ce qui, de la planche, se règle au goût de chacun.
//!
//! Ce sont des réglages de l'*application* (`reglages.toml`, clé `[vignettes]`), pas du projet : la marge, l'aimant et la
//! couleur des cadres sont une affaire de confort de la personne qui dessine, la couleur et les tailles des brosses du décor aussi.
//! Seul « Report du décor » touche à ce que fait le projet (à la jonction de deux plans) ; il se lit au moment de l'opération
//! (`Document::report_decor`) et n'est écrit nulle part dans le projet. Le nombre de rangées pâles et de colonnes restent là où ils
//! étaient (`Layout::board_min_rows`, `Layout::board_columns`, réglages de l'application eux aussi) : la fenêtre ne fait que les montrer.

use serde::{Deserialize, Serialize};

use crate::decor::{self, Genre};

/// Le bleu d'origine des cadres de caméra (`#378ADD`).
pub const CADRE_DEFAUT: [u8; 3] = [0x37, 0x8A, 0xDD];
/// Le débord d'origine du trait hors de sa case, en pour cent de la largeur de la case.
pub const DEBORD_DEFAUT: u8 = 12;
pub const DEBORD_MAX: u8 = 40;
/// L'aimant d'origine des cadres de caméra, en points d'écran ; 0 : éteint.
pub const AIMANT_DEFAUT: u8 = 6;
pub const AIMANT_MAX: u8 = 30;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesVignettes {
    /// Combien du trait se voit au-delà du cadre d'une case, de chaque côté : pour cent de sa largeur.
    #[serde(rename = "debord_pour_cent")]
    pub debord: u8,
    /// À quelle distance (points d'écran) l'aimant prend un cadre de caméra près d'un alignement ; 0 : pas d'aimant.
    #[serde(rename = "aimant_points")]
    pub aimant: u8,
    /// La couleur des cadres de caméra (A, B, C…), à l'écran et dans l'export de la planche.
    #[serde(rename = "couleur_cadre")]
    pub couleur_cadre: [u8; 3],
    /// En réunissant deux plans, le décor du premier se tient sur la suite.
    #[serde(rename = "report_decor")]
    pub report_decor: bool,
    /// La couleur des brosses du décor.
    #[serde(rename = "couleur_decor")]
    pub couleur_decor: [u8; 3],
    /// La taille des brosses du décor (crayon, feutre, aplat), en pixels du dessin.
    #[serde(rename = "tailles_decor")]
    pub tailles_decor: [f32; 3],
    /// La sorte du calque Décor que le premier trait d'une brosse du décor crée dans un plan : faux (d'origine), vectoriel ; vrai, bitmap. Ne
    /// change pas un calque Décor qui existe déjà (`Document::assurer_decor`).
    #[serde(rename = "calque_decor_bitmap")]
    pub decor_bitmap: bool,
}

impl Default for ReglagesVignettes {
    fn default() -> Self {
        ReglagesVignettes {
            debord: DEBORD_DEFAUT,
            aimant: AIMANT_DEFAUT,
            couleur_cadre: CADRE_DEFAUT,
            report_decor: true,
            couleur_decor: decor::COULEUR_DEFAUT,
            tailles_decor: Genre::ALL.map(Genre::taille_defaut),
            decor_bitmap: false,
        }
    }
}

impl ReglagesVignettes {
    /// Des valeurs ramenées dans leurs bornes (un fichier modifié à la main).
    pub fn nettoyes(mut self) -> Self {
        self.debord = self.debord.min(DEBORD_MAX);
        self.aimant = self.aimant.min(AIMANT_MAX);
        for (t, g) in self.tailles_decor.iter_mut().zip(Genre::ALL) {
            *t = if t.is_finite() { t.clamp(0.5, 400.0) } else { g.taille_defaut() };
        }
        self
    }

    pub fn est_defaut(&self) -> bool {
        *self == ReglagesVignettes::default()
    }

    /// Le débord, en part de la largeur d'une case.
    pub fn debord_fraction(&self) -> f32 {
        f32::from(self.debord.min(DEBORD_MAX)) / 100.0
    }

    /// L'aimant des cadres de caméra, en points d'écran ; rien s'il est éteint.
    pub fn aimant_points(&self) -> Option<f32> {
        (self.aimant > 0).then(|| f32::from(self.aimant.min(AIMANT_MAX)))
    }

    /// La taille de la brosse `g` du décor.
    pub fn taille_decor(&self, g: Genre) -> f32 {
        self.tailles_decor[g.rang()]
    }
}

/// Une couleur d'affichage.
pub fn en_couleur(c: [u8; 3]) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgb(c[0], c[1], c[2])
}
