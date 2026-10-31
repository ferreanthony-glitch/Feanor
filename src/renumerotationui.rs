//! Le rapport de numérotation côté écran : la commande du hub Exporter (`fichier.exporter_renumerotation`), qui écrit le CSV et la page lisible là où l'on
//! choisit, et la case de la fenêtre « Pour le montage » (`editorialui.rs`). Le calcul : `renumerotation.rs`.
//!
//! Règle « sans latence » : le rapport ne touche ni image ni dessin (des numéros, des noms et des rangs) ; deux petits fichiers de texte s'écrivent
//! tout de suite, comme la liste des plans sans vignettes.

use std::path::Path;

use crate::app::App;
use crate::renumerotation;

impl App {
    /// Fichier > Exporter > Rapport de numérotation : on choisit le fichier CSV, la page lisible s'écrit à côté (même nom, `.txt`).
    pub(crate) fn exporter_renumerotation(&mut self) {
        self.end_stroke();
        let rapport = match renumerotation::depuis_le_dernier_envoi(&self.doc) {
            Ok(r) => r,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        let nom = format!("{}_renumerotation.csv", crate::editorial::file_safe(&self.base_name()));
        let Some(mut csv) = self.dialogue_enregistrer("renumerotation", "Rapport de numérotation (le CSV ; la page lisible est écrite à côté)", ("CSV (.csv)", &["csv"]), &nom) else { return };
        if csv.extension().is_none() {
            csv.set_extension("csv");
        }
        self.status = match self.renumerotation_ecrire(&rapport, &csv) {
            Ok(m) | Err(m) => m,
        };
    }

    /// Écrit le rapport (CSV à `csv`, page lisible au même nom en `.txt`). Rend ce qu'il faut dire.
    pub(crate) fn renumerotation_ecrire(&self, rapport: &renumerotation::Rapport, csv: &Path) -> Result<String, String> {
        let (c, t) = renumerotation::ecrire_vers(rapport, &self.doc, csv)?;
        Ok(resume(rapport, &c, &t))
    }
}

/// La phrase de fin : ce qui a changé, et où sont les fichiers.
pub(crate) fn resume(r: &renumerotation::Rapport, csv: &Path, txt: &Path) -> String {
    if r.est_vide() {
        return format!("Rapport de numérotation écrit : rien n'a changé depuis l'envoi « {} » ({}).", r.envoi_nom, csv.display());
    }
    format!(
        "Rapport de numérotation depuis « {} » : {} ajouté(s), {} supprimé(s), {} déplacé(s), {} renuméroté(s). {} et {}",
        r.envoi_nom,
        r.ajoutes(),
        r.supprimes(),
        r.deplaces(),
        r.renumerotes(),
        csv.display(),
        txt.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
    )
}
