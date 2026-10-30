//! Ce qui reste de l'ancienne « Planche de crayonnés » (supprimée le 2026-10-21 : les Vignettes font le
//! même travail, on y dessine directement dans une case vide).
//!
//! Les projets qui ont un `crayonnes.json` s'ouvrent normalement : SHOT ne lit plus ce fichier, ne
//! l'écrit plus et ne l'efface JAMAIS (il reste dans le dossier du projet, octet pour octet). Seule
//! chose faite à l'ouverture : si des cases dessinées n'ont jamais été transformées en plans, un
//! avertissement le dit une fois, avec le chemin du fichier. Aucune conversion automatique : un croquis
//! qui revient tout seul dans le film surprendrait plus qu'il ne rendrait service ; rouvrir le projet
//! avec la version d'avant, ou redessiner la case dans les Vignettes, suffit.

use std::path::Path;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "crayonnes.json";

/// Combien de cases du fichier ont un dessin (traits ou peinture) et n'ont jamais servi à faire des
/// plans. `None` si le fichier est absent ou illisible (ni l'un ni l'autre n'est une erreur).
pub(crate) fn cases_oubliees(text: &str) -> Option<usize> {
    let racine: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let cases = racine.get("cases")?.as_array()?;
    let dessinee = |c: &serde_json::Value| {
        let traits = c.get("traits").and_then(|t| t.as_array()).is_some_and(|t| !t.is_empty());
        let peinture = c.get("peinture").and_then(|p| p.as_str()).is_some_and(|p| !p.is_empty());
        traits || peinture
    };
    let utilisee = |c: &serde_json::Value| c.get("utilisee").and_then(|u| u.as_bool()).unwrap_or(false);
    Some(cases.iter().filter(|c| dessinee(c) && !utilisee(c)).count())
}

/// Ajoute un avertissement à `warnings` si le dossier `dir` garde un `crayonnes.json` avec des croquis
/// jamais transformés en plans. Ne touche à aucun fichier.
pub(crate) fn signaler(dir: &Path, warnings: &mut Vec<String>) {
    let path = dir.join(FILE);
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    match cases_oubliees(&text) {
        Some(n) if n > 0 => warnings.push(format!(
            "{FILE} : la planche de crayonnés n'existe plus ; {n} case{} dessinée{} n'avai{} pas été transformée{} en plans et ne s'affiche{} plus. Le fichier reste en place (rien n'est effacé) ; dessine ces croquis dans les Vignettes (une case vide après le dernier plan). « Mettre de côté » renomme le fichier en crayonnes.json.ancien pour que cet avis ne revienne plus.",
            if n > 1 { "s" } else { "" },
            if n > 1 { "s" } else { "" },
            if n > 1 { "ent" } else { "t" },
            if n > 1 { "s" } else { "" },
            if n > 1 { "nt" } else { "" },
        )),
        _ => {}
    }
}

/// Le nom du fichier mis de côté.
pub(crate) const MIS_DE_COTE: &str = "crayonnes.json.ancien";

impl crate::app::App {
    /// « Mettre de côté » : `crayonnes.json` devient `crayonnes.json.ancien` dans le dossier du projet. Rien n'est effacé ni lu ; le
    /// bouton « Annuler » du message qui suit le remet. Sans effet sur le projet lui-même (il ne l'utilise pas).
    pub(crate) fn crayonnes_de_cote(&mut self, dir: &Path) {
        let (de, vers) = (dir.join(FILE), dir.join(MIS_DE_COTE));
        if !de.is_file() {
            self.status = format!("{FILE} n'est plus dans le dossier du projet : rien à mettre de côté.");
        } else if vers.exists() {
            self.status = format!("{MIS_DE_COTE} existe déjà dans le dossier du projet : {FILE} reste en place (rien n'est changé).");
        } else {
            match std::fs::rename(&de, &vers) {
                Ok(()) => self.status_avec_action(format!("{FILE} mis de côté : il s'appelle maintenant {MIS_DE_COTE} (rien n'est effacé)."), crate::messages::ActionMessage::CrayonnesRemettre(dir.to_path_buf())),
                Err(e) => self.status = format!("Impossible de mettre {FILE} de côté : {e}"),
            }
        }
    }

    /// Annule `crayonnes_de_cote`.
    pub(crate) fn crayonnes_remettre(&mut self, dir: &Path) {
        let (de, vers) = (dir.join(MIS_DE_COTE), dir.join(FILE));
        if !de.is_file() || vers.exists() {
            self.status = format!("{FILE} ne peut pas être remis : {MIS_DE_COTE} a disparu, ou {FILE} existe de nouveau.");
        } else {
            match std::fs::rename(&de, &vers) {
                Ok(()) => self.status = format!("{FILE} est remis à son nom."),
                Err(e) => self.status = format!("Impossible de remettre {FILE} : {e}"),
            }
        }
    }
}
