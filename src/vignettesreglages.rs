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
    /// Ancienne couleur des brosses du décor : **lue, jamais écrite** (2026-10-29 : chaque brosse perso Décor a ses couleurs, voir
    /// `Settings::brosses_decor` et `Settings::reprendre_les_anciens_pinceaux_de_decor`).
    #[serde(rename = "couleur_decor", skip_serializing)]
    pub couleur_decor: [u8; 3],
    /// Anciennes tailles des pinceaux du décor (crayon, feutre, aplat) : **lues, jamais écrites** (elles sont désormais celles des brosses Décor).
    #[serde(rename = "tailles_decor", skip_serializing)]
    pub tailles_decor: [f32; 3],
    /// La sorte du calque Décor que le premier trait d'une brosse du décor crée dans un plan : faux (d'origine), vectoriel ; vrai, bitmap. Ne
    /// change pas un calque Décor qui existe déjà (`Document::assurer_decor`).
    #[serde(rename = "calque_decor_bitmap")]
    pub decor_bitmap: bool,
    /// Taille des numéros de plan, en pour cent de la taille d'origine (100 : celle d'origine).
    #[serde(rename = "taille_numeros")]
    pub taille_numeros: u16,
    /// Un fond clair derrière les numéros de plan (lisibles sur n'importe quel dessin qui déborde).
    #[serde(rename = "fond_numeros")]
    pub fond_numeros: bool,
    /// L'étiquette de version (« V2 ») se montre, à côté du numéro du plan, pour un plan qui a plusieurs versions.
    #[serde(rename = "pastille_version")]
    pub pastille_version: bool,
    /// Afficher la durée du plan (en secondes et images) à côté de son numéro.
    #[serde(rename = "afficher_duree")]
    pub afficher_duree: bool,
    /// Les champs de texte (action, dialogue, notes) montrés sous les cases d'un projet neuf : aucun d'origine.
    #[serde(rename = "textes_par_defaut")]
    pub textes_defaut: crate::boardsens::Textes,
    /// La couleur de la ligne de chaque champ : action, dialogue, notes.
    #[serde(rename = "couleurs_textes")]
    pub couleurs_textes: [[u8; 3]; 3],
    /// Le zoom d'ouverture : cases par rangée et rangées visibles quand on ouvre un projet (6 × 2 d'origine).
    #[serde(rename = "ouverture_colonnes")]
    pub ouverture_colonnes: u8,
    #[serde(rename = "ouverture_rangees")]
    pub ouverture_rangees: u8,
    /// L'épaisseur du filet des cases, en points (1 d'origine), et sa couleur (gris d'origine, plus foncé pour la case courante).
    #[serde(rename = "epaisseur_cadre")]
    pub epaisseur_cadre: f32,
    #[serde(rename = "couleur_filet")]
    pub couleur_filet: Option<[u8; 3]>,
    /// L'opacité (pour cent) du dessin hors des cases, sur la planche : 100, plein.
    #[serde(rename = "opacite_hors_champ")]
    pub opacite_hors_champ: u8,
    /// Le passe-partout des cases : allumé ou non, et son opacité (pour cent). Retenus d'une séance à l'autre.
    #[serde(rename = "passe_partout_cases")]
    pub passe_partout_cases: bool,
    #[serde(rename = "passe_partout_cases_opacite")]
    pub passe_partout_cases_opacite: u8,
    /// La case « Passe-partout » de la fenêtre d'export de la planche : décochée d'origine, retenue d'une séance à l'autre. Indépendante de
    /// l'interrupteur de l'écran (`passe_partout_cases`) ; seule l'opacité est celle de l'écran.
    #[serde(rename = "export_passe_partout")]
    pub export_passe_partout: bool,
}

/// Les couleurs d'origine des lignes de texte : action, dialogue, notes.
pub const COULEURS_TEXTES_DEFAUT: [[u8; 3]; 3] = [[48, 96, 160], [160, 88, 24], [92, 92, 92]];
pub const NUMEROS_MIN: u16 = 50;
pub const NUMEROS_MAX: u16 = 300;
pub const OUVERTURE_MAX: u8 = 12;

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
            taille_numeros: 100,
            fond_numeros: true,
            pastille_version: true,
            afficher_duree: false,
            textes_defaut: crate::boardsens::Textes { action: false, dialogue: false, notes: false },
            couleurs_textes: COULEURS_TEXTES_DEFAUT,
            ouverture_colonnes: 6,
            ouverture_rangees: 2,
            epaisseur_cadre: 1.0,
            couleur_filet: None,
            opacite_hors_champ: 100,
            passe_partout_cases: false,
            passe_partout_cases_opacite: 100,
            export_passe_partout: false,
        }
    }
}

impl ReglagesVignettes {
    /// Des valeurs ramenées dans leurs bornes (un fichier modifié à la main).
    pub fn nettoyes(mut self) -> Self {
        self.debord = self.debord.min(DEBORD_MAX);
        self.aimant = self.aimant.min(AIMANT_MAX);
        self.taille_numeros = self.taille_numeros.clamp(NUMEROS_MIN, NUMEROS_MAX);
        self.ouverture_colonnes = self.ouverture_colonnes.clamp(1, crate::board::MAX_COLUMNS as u8);
        self.ouverture_rangees = self.ouverture_rangees.clamp(1, OUVERTURE_MAX);
        self.epaisseur_cadre = if self.epaisseur_cadre.is_finite() { self.epaisseur_cadre.clamp(0.5, 6.0) } else { 1.0 };
        self.opacite_hors_champ = self.opacite_hors_champ.min(100);
        self.passe_partout_cases_opacite = self.passe_partout_cases_opacite.min(100);
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

    /// La taille des numéros de plan, en part de la taille d'origine.
    pub fn numeros_echelle(&self) -> f32 {
        f32::from(self.taille_numeros.clamp(NUMEROS_MIN, NUMEROS_MAX)) / 100.0
    }

    /// La couleur de la ligne du champ `c`.
    pub fn couleur_texte(&self, c: crate::boardsens::Champ) -> [u8; 3] {
        use crate::boardsens::Champ;
        self.couleurs_textes[match c {
            Champ::Action => 0,
            Champ::Dialogue => 1,
            Champ::Notes => 2,
        }]
    }

    /// L'aimant des cadres de caméra, en points d'écran ; rien s'il est éteint.
    pub fn aimant_points(&self) -> Option<f32> {
        (self.aimant > 0).then(|| f32::from(self.aimant.min(AIMANT_MAX)))
    }

}

/// Une couleur d'affichage.
pub fn en_couleur(c: [u8; 3]) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgb(c[0], c[1], c[2])
}
