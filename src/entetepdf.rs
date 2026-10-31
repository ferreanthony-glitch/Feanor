//! Ce que les PDF de production écrivent en plus des images (Anthony, 2026-10-30, audit H4 et « planche PDF plate, sans titre ni légendes ») : le bloc de titre
//! (série, épisode, séquence, version, date), les plans **modifiés depuis le dernier envoi** (marqués), et la page finale « Notes de révision ».
//!
//! Tout se calcule ici, sans interface ni image, à partir du projet et de l'envoi de référence ; `pdfmise.rs` (storyboard PDF) et `planche.rs` (planche des
//! vignettes) le dessinent.
//!
//! # Le bloc de titre (`Entete`)
//!
//! - ligne 1 : la série (à défaut, le titre du projet) ;
//! - ligne 2 : « Épisode 3 · Séquence 2 · Version v2 au réal · 30/10/2026 » (chaque morceau seulement s'il existe : sans épisode, sans envoi, sans séquence — une
//!   page qui mêle plusieurs séquences n'en nomme aucune).
//! - « Version » est le nom du **dernier envoi** : c'est l'état que le réalisateur a vu pour la dernière fois.
//!
//! # Les notes de révision (`Revision`)
//!
//! Depuis l'envoi de référence : les plans modifiés ou ajoutés (comme la colonne « Modifié depuis l'envoi » de la fiche) et les notes du **fil de décision**
//! (`fildecision.rs`) traitées depuis — statut « Fait » ou « Refusé » posé après la date où l'envoi a été noté — avec, pour un refus, sa raison. Une note
//! résolue avant que le fil existe n'a pas de date de statut : elle n'y figure pas.

use std::collections::HashMap;

use crate::document::Document;
use crate::envois::{Envoi, Etat};
use crate::fildecision::Status;

/// Le bloc de titre.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entete {
    /// La série, sinon le titre du projet.
    pub projet: String,
    pub episode: String,
    /// La séquence, quand la page n'en montre qu'une.
    pub sequence: String,
    /// Le nom du dernier envoi.
    pub version: String,
    pub date: String,
}

impl Entete {
    /// Le bloc du projet `doc` : `envoi` est l'envoi de référence, `date` le jour du document (« 2026-10-30 »).
    pub fn de_projet(doc: &Document, envoi: Option<&Envoi>, date: &str) -> Entete {
        let projet = match doc.properties.serie.trim() {
            "" => doc.properties.title.trim().to_string(),
            s => s.to_string(),
        };
        Entete { projet, episode: doc.properties.episode.trim().to_string(), sequence: String::new(), version: envoi.map(|e| e.nom.trim().to_string()).unwrap_or_default(), date: date.to_string() }
    }

    /// La même, pour une page de la séquence `sequence`.
    pub fn pour_sequence(&self, sequence: &str) -> Entete {
        Entete { sequence: sequence.to_string(), ..self.clone() }
    }

    /// La première ligne : la série ou le titre.
    pub fn ligne1(&self) -> String {
        self.projet.clone()
    }

    /// La seconde ligne : épisode, séquence, version, date.
    pub fn ligne2(&self) -> String {
        let mut morceaux: Vec<String> = Vec::new();
        if !self.episode.is_empty() {
            morceaux.push(format!("Épisode {}", self.episode));
        }
        if !self.sequence.is_empty() {
            morceaux.push(format!("Séquence {}", self.sequence));
        }
        if !self.version.is_empty() {
            morceaux.push(format!("Version {}", self.version));
        }
        if !self.date.is_empty() {
            morceaux.push(self.date.clone());
        }
        morceaux.join(" · ")
    }

    /// Tout le texte du bloc (pour choisir une police qui a toutes les lettres).
    pub fn tout_le_texte(&self) -> String {
        format!("{} {} Page / ", self.ligne1(), self.ligne2())
    }
}

/// Une note du fil de décision traitée depuis l'envoi.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteTraitee {
    /// « Séquence 1 — plan 12A ».
    pub plan: String,
    /// « Fait » ou « Refusé ».
    pub statut: String,
    pub auteur: String,
    pub texte: String,
    /// Pourquoi une note est refusée.
    pub raison: String,
}

/// Les notes de révision : les plans qui ont changé et les notes traitées.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Revision {
    pub envoi: String,
    /// (« Séquence 1 — plan 12A », « Modifié » ou « Ajouté »), dans l'ordre du film.
    pub plans: Vec<(String, String)>,
    pub notes: Vec<NoteTraitee>,
}

impl Revision {
    /// Les lignes de la page : (en gras ?, retrait, texte), avant le découpage à la largeur de la page.
    pub fn lignes(&self) -> Vec<(bool, f32, String)> {
        let mut out: Vec<(bool, f32, String)> = Vec::new();
        out.push((false, 0.0, format!("Depuis l'envoi « {} ».", self.envoi)));
        out.push((false, 0.0, String::new()));
        out.push((true, 0.0, format!("Plans modifiés ou ajoutés ({})", self.plans.len())));
        if self.plans.is_empty() {
            out.push((false, 10.0, "Aucun.".into()));
        }
        for (plan, etat) in &self.plans {
            out.push((false, 10.0, format!("{plan} : {etat}")));
        }
        out.push((false, 0.0, String::new()));
        out.push((true, 0.0, format!("Notes traitées ({})", self.notes.len())));
        if self.notes.is_empty() {
            out.push((false, 10.0, "Aucune.".into()));
        }
        for n in &self.notes {
            let qui = if n.auteur.trim().is_empty() { String::new() } else { format!(" ({})", n.auteur.trim()) };
            let mut l = format!("{} — {} : {}{qui}", n.plan, n.statut, n.texte.trim().replace('\n', " "));
            if !n.raison.trim().is_empty() {
                l += &format!(" Raison : {}", n.raison.trim());
            }
            out.push((false, 10.0, l));
        }
        out
    }

    /// Tout le texte de la page (pour choisir une police).
    pub fn tout_le_texte(&self) -> String {
        let mut s = String::from("Notes de révision ");
        for (_, _, t) in self.lignes() {
            s.push_str(&t);
            s.push(' ');
        }
        s
    }
}

/// Tout ce que le PDF de studio ajoute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Studio {
    pub entete: Entete,
    /// Pour chaque case du PDF (dans l'ordre des `Board`) : le plan a changé depuis l'envoi.
    pub modifies: Vec<bool>,
    /// La page finale ; rien : pas de page.
    pub revision: Option<Revision>,
}

impl Studio {
    pub fn tout_le_texte(&self) -> String {
        format!("{} MODIFIÉ {}", self.entete.tout_le_texte(), self.revision.as_ref().map(Revision::tout_le_texte).unwrap_or_default())
    }
}

/// Les notes du fil de décision traitées (« Fait » ou « Refusé ») depuis l'instant `depuis` (secondes depuis 1970), dans l'ordre du film puis de la note.
pub fn notes_traitees(doc: &Document, depuis: u64) -> Vec<NoteTraitee> {
    let rang: HashMap<u64, usize> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut v: Vec<(usize, u64, NoteTraitee)> = doc
        .comments
        .iter()
        .filter(|c| matches!(c.status(), Status::Done | Status::Refused) && c.follow.status_at > depuis)
        .filter_map(|c| {
            let i = *rang.get(&c.panel)?;
            Some((
                i,
                c.id,
                NoteTraitee {
                    plan: crate::storyboard::panel_long_title(doc, i),
                    statut: c.status().label().to_string(),
                    auteur: c.author.clone(),
                    texte: c.text.clone(),
                    raison: c.follow.reason.clone(),
                },
            ))
        })
        .collect();
    v.sort_by_key(|(i, id, _)| (*i, *id));
    v.into_iter().map(|(_, _, n)| n).collect()
}

/// Les notes de révision depuis `envoi`. `etats` : l'état des plans à son égard (le relevé de `App::fiche_etats`) ; rien : le relevé n'est pas prêt, la liste
/// des plans est vide.
pub fn revision(doc: &Document, envoi: &Envoi, etats: Option<&HashMap<u64, Etat>>) -> Revision {
    let plans = doc
        .panels
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let etat = match etats?.get(&p.id)? {
                Etat::Modifie => "Modifié",
                Etat::Nouveau => "Ajouté",
                Etat::Inchange => return None,
            };
            Some((crate::storyboard::panel_long_title(doc, i), etat.to_string()))
        })
        .collect();
    let depuis = if envoi.note_le > 0 { envoi.note_le } else { envoi.date };
    Revision { envoi: envoi.libelle(), plans, notes: notes_traitees(doc, depuis) }
}

/// Le PDF de studio de `doc` : le bloc, les cases modifiées (`panels` : le plan de chaque case, dans l'ordre des `Board`), et la page finale si `avec_notes`.
pub fn studio(doc: &Document, panels: &[usize], etats: Option<&HashMap<u64, Etat>>, avec_notes: bool, date: &str) -> Studio {
    let envoi = doc.envois.dernier();
    let entete = Entete::de_projet(doc, envoi, date);
    let modifies = panels.iter().map(|&i| doc.panels.get(i).is_some_and(|p| etats.is_some_and(|m| m.contains_key(&p.id)))).collect();
    let revision = envoi.filter(|_| avec_notes).map(|e| revision(doc, e, etats));
    Studio { entete, modifies, revision }
}
