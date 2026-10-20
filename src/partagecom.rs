//! Les commentaires d'un film partagé : un fichier par auteur, fusionnés à la lecture (`docs/COOPERATION.md`, §3 et §10).
//!
//! `commentaires/<équipier>.json` ne contient que des **enregistrements** : « à tel instant, ce commentaire est dans cet
//! état » (écrit, modifié, résolu) ou « supprimé ». Chaque équipier n'écrit que son fichier, y compris pour résoudre ou
//! supprimer le commentaire d'un autre. La fusion garde, pour chaque commentaire, l'enregistrement le plus récent
//! (à égalité, celui du plus petit identifiant d'équipier) : deux postes qui fusionnent les mêmes fichiers obtiennent la même
//! liste, quel que soit l'ordre d'arrivée, et une copie en conflit d'un client de synchronisation n'est qu'un fichier de plus.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::commentaires::Comment;

pub(crate) const FORMAT: &str = "shot-commentaires-partage";
const VERSION: u32 = 1;

/// Un état d'un commentaire à un instant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Enregistrement {
    /// Millisecondes depuis 1970, horloge de l'auteur de l'enregistrement.
    pub at: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub supprime: bool,
    pub commentaire: Comment,
}

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    equipier: String,
    #[serde(default)]
    enregistrements: Vec<Enregistrement>,
}

/// Les enregistrements d'un fichier. Textes bornés comme dans `commentaires.json`.
pub(crate) fn lire(texte: &str) -> Result<Vec<Enregistrement>, String> {
    let f: Fichier = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de commentaires partagés".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let mut out = f.enregistrements;
    for e in &mut out {
        if e.commentaire.text.chars().count() > crate::commentaires::TEXT_MAX {
            e.commentaire.text = e.commentaire.text.chars().take(crate::commentaires::TEXT_MAX).collect();
        }
    }
    Ok(out)
}

pub(crate) fn octets(equipier: &str, enregistrements: &[Enregistrement]) -> Result<Vec<u8>, String> {
    let f = Fichier { format: FORMAT.into(), version: VERSION, equipier: equipier.into(), enregistrements: enregistrements.to_vec() };
    serde_json::to_vec_pretty(&f).map_err(|e| e.to_string())
}

/// La fusion : pour chaque commentaire, l'enregistrement le plus récent. `fichiers` : (équipier, enregistrements). Rend les
/// états vivants, dans l'ordre des numéros.
pub(crate) fn fusionner(fichiers: &[(String, Vec<Enregistrement>)]) -> BTreeMap<u64, Comment> {
    let mut meilleurs: BTreeMap<u64, (u64, &str, &Enregistrement)> = BTreeMap::new();
    for (qui, enr) in fichiers {
        for e in enr {
            let id = e.commentaire.id;
            let mieux = match meilleurs.get(&id) {
                None => true,
                Some((at, autre, _)) => e.at > *at || (e.at == *at && qui.as_str() < *autre),
            };
            if mieux {
                meilleurs.insert(id, (e.at, qui, e));
            }
        }
    }
    meilleurs.into_iter().filter(|(_, (_, _, e))| !e.supprime).map(|(id, (_, _, e))| (id, e.commentaire.clone())).collect()
}

/// Ce que ma séance sait des commentaires : l'état fusionné qu'elle a vu (`base`) et mes enregistrements (`miens`, ce que mon
/// fichier dira à l'enregistrement).
#[derive(Clone, Debug, Default)]
pub(crate) struct Suivi {
    /// Les commentaires de tout le monde tels qu'ils étaient à la dernière fusion.
    pub base: BTreeMap<u64, Comment>,
    /// Mes enregistrements, par numéro de commentaire.
    pub miens: BTreeMap<u64, Enregistrement>,
    /// Mes enregistrements ont changé depuis la dernière écriture de mon fichier.
    pub a_ecrire: bool,
}

impl Suivi {
    /// Ma séance a changé des commentaires depuis la dernière fusion : ils deviennent mes enregistrements (daté `now`).
    /// `locaux` : la liste de commentaires du document ; `plans` : les plans qui existent (les commentaires d'un plan qui
    /// n'existe plus ne sont pas affichés, donc pas considérés comme supprimés).
    pub fn relever(&mut self, locaux: &[Comment], plans: &dyn Fn(u64) -> bool, now: u64) -> bool {
        let mut change = false;
        let par_id: BTreeMap<u64, &Comment> = locaux.iter().map(|c| (c.id, c)).collect();
        for (id, c) in &par_id {
            let pareil = self.base.get(id).is_some_and(|b| b == *c);
            if pareil {
                continue;
            }
            // Un état que mon dernier enregistrement dit déjà : rien de neuf.
            if self.miens.get(id).is_some_and(|m| !m.supprime && &m.commentaire == *c) {
                continue;
            }
            self.miens.insert(*id, Enregistrement { at: now, supprime: false, commentaire: (*c).clone() });
            self.base.insert(*id, (*c).clone());
            change = true;
        }
        // Un commentaire que la fusion connaît, dont le plan existe, et que le document n'a plus : je l'ai supprimé.
        let disparus: Vec<u64> = self.base.iter().filter(|(id, c)| !par_id.contains_key(id) && plans(c.panel)).map(|(id, _)| *id).collect();
        for id in disparus {
            let ancien = self.base.remove(&id).expect("cité juste avant");
            self.miens.insert(id, Enregistrement { at: now, supprime: true, commentaire: ancien });
            change = true;
        }
        self.a_ecrire |= change;
        change
    }

    /// Fusionne les fichiers lus (les miens en mémoire comptent, avec leurs changements pas encore écrits) et rend la liste
    /// à mettre dans le document, limitée aux plans qui existent. `base` devient cette fusion.
    pub fn fusionner(&mut self, mon_id: &str, autres: &[(String, Vec<Enregistrement>)], plans: &dyn Fn(u64) -> bool) -> Vec<Comment> {
        let mut fichiers: Vec<(String, Vec<Enregistrement>)> = autres.iter().filter(|(q, _)| q != mon_id).cloned().collect();
        fichiers.push((mon_id.to_string(), self.miens.values().cloned().collect()));
        let fusion = fusionner(&fichiers);
        self.base = fusion.clone();
        fusion.into_values().filter(|c| plans(c.panel)).collect()
    }

    /// Mon fichier tel qu'il doit être écrit.
    pub fn fichier(&self, mon_id: &str) -> Result<Vec<u8>, String> {
        let v: Vec<Enregistrement> = self.miens.values().cloned().collect();
        octets(mon_id, &v)
    }
}
