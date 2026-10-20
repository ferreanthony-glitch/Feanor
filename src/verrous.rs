//! Les prises : qui tient quel plan, et le jeton du montage (`docs/COOPERATION.md`, §5 et §6).
//!
//! Tout ce fichier est **pur** : il calcule, il ne lit ni n'écrit rien. Les fiches d'équipier sont lues par le dépôt
//! (`depot.rs`) ; ici on décide, à partir des fiches lues et de l'heure, qui est le titulaire de chaque cible. Les mêmes
//! fiches donnent le même titulaire sur tous les postes : c'est ce qui tranche la course de deux prises simultanées, sans
//! que personne n'ait à se parler.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::equipe::Cle;

/// Format de la fiche d'équipier (`partage/equipe/<équipier>.<séance>.json`).
pub(crate) const FORMAT_FICHE: &str = "shot-equipier";
pub(crate) const VERSION_FICHE: u32 = 1;

/// Ce qu'on prend : un plan (son numéro), ou le jeton du montage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cible {
    Plan(u64),
    Montage,
}

impl Cible {
    /// « plan:5 » ou « montage » : la forme du fichier.
    pub fn texte(&self) -> String {
        match self {
            Cible::Plan(p) => format!("plan:{p}"),
            Cible::Montage => "montage".into(),
        }
    }

    pub fn depuis_texte(t: &str) -> Option<Cible> {
        if t == "montage" {
            return Some(Cible::Montage);
        }
        t.strip_prefix("plan:").and_then(|n| n.parse().ok()).map(Cible::Plan)
    }
}

impl Serialize for Cible {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.texte())
    }
}

impl<'de> Deserialize<'de> for Cible {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = String::deserialize(d)?;
        Cible::depuis_texte(&t).ok_or_else(|| serde::de::Error::custom(format!("cible inconnue : {t}")))
    }
}

/// Une prise reprise à un autre (§5.3) : la prise écartée reste écartée pour toujours.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remplace {
    pub equipier: String,
    pub seance: String,
    pub pris_le: u64,
}

/// Une prise, dans la fiche de son auteur.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prise {
    pub cible: Cible,
    /// Millisecondes depuis 1970, sur l'horloge de celui qui prend.
    pub pris_le: u64,
    /// Le numéro du dernier fichier écrit par ce porteur pour cette cible (§5.4).
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remplace: Vec<Remplace>,
}

/// Une prise rendue : sa dernière révision, pour que le suivant sache quel fichier attendre.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendue {
    pub cible: Cible,
    pub le: u64,
    #[serde(default)]
    pub revision: u64,
}

/// Où est l'équipier (§9).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<u64>,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub vu_le: u64,
}

/// Qui est derrière une fiche.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Equipier {
    pub id: String,
    pub nom: String,
    pub couleur: [u8; 3],
    #[serde(default)]
    pub machine: String,
    #[serde(default)]
    pub prefixe: u64,
}

/// La fiche d'un équipier dans une séance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fiche {
    pub format: String,
    pub version: u32,
    pub equipier: Equipier,
    pub seance: String,
    /// Le dernier battement (millisecondes depuis 1970, horloge de l'auteur).
    pub renouvele_le: u64,
    /// Augmente à chaque écriture de la fiche : un lecteur voit qu'elle a changé même si l'horloge de l'auteur est fausse.
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub prises: Vec<Prise>,
    #[serde(default)]
    pub rendues: Vec<Rendue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presence: Option<Presence>,
}

impl Fiche {
    pub fn cle(&self) -> Cle {
        Cle { equipier: self.equipier.id.clone(), seance: self.seance.clone() }
    }

    /// Lit le texte d'une fiche. Refus clair d'un autre format ou d'une version plus récente.
    pub fn lire(texte: &str) -> Result<Fiche, String> {
        let f: Fiche = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("fiche illisible ({e})"))?;
        if f.format != FORMAT_FICHE {
            return Err("ce n'est pas une fiche d'équipier".into());
        }
        if f.version > VERSION_FICHE {
            return Err(format!("fiche écrite par une version plus récente (format {})", f.version));
        }
        // (Une cible n'est prise qu'une fois par fiche : la plus récente.)
        let mut f = f;
        f.prises.sort_by_key(|p| std::cmp::Reverse(p.pris_le));
        let mut vues = std::collections::HashSet::new();
        f.prises.retain(|p| vues.insert(p.cible));
        f.rendues.sort_by_key(|r| std::cmp::Reverse(r.le));
        let mut vues = std::collections::HashSet::new();
        f.rendues.retain(|r| vues.insert(r.cible));
        Ok(f)
    }

    pub fn octets(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|e| e.to_string())
    }

    /// La prise `p` a été rendue (par une rendue plus récente ou égale).
    pub fn rendue_apres(&self, p: &Prise) -> bool {
        self.rendues.iter().any(|r| r.cible == p.cible && r.le >= p.pris_le)
    }
}

/// Les changements de fiches que j'ai vus, avec mon heure à moi : une fiche qui change est vivante, même si l'horloge de
/// son auteur est fausse (§5.2). Le premier regard sur une fiche ne compte pas : une fiche d'hier vue pour la première
/// fois est périmée.
#[derive(Clone, Debug, Default)]
pub struct Observations {
    vus: HashMap<Cle, (u64, Option<u64>)>,
}

impl Observations {
    /// J'ai lu la fiche `cle`, de séquence `sequence`, à `maintenant`.
    pub fn voir(&mut self, cle: &Cle, sequence: u64, maintenant: u64) {
        match self.vus.get_mut(cle) {
            None => {
                self.vus.insert(cle.clone(), (sequence, None));
            }
            Some((seq, dernier)) => {
                if *seq != sequence {
                    *seq = sequence;
                    *dernier = Some(maintenant);
                }
            }
        }
    }

    /// Le dernier changement que j'ai vu de cette fiche (mon heure).
    pub fn dernier_changement(&self, cle: &Cle) -> Option<u64> {
        self.vus.get(cle).and_then(|v| v.1)
    }
}

/// Une prise vue dans une fiche, avec de quoi la juger.
#[derive(Clone, Debug, PartialEq)]
pub struct Revendication {
    pub cle: Cle,
    pub equipier: Equipier,
    pub prise: Prise,
    /// Le dernier battement de la fiche.
    pub renouvele_le: u64,
}

impl Revendication {
    /// L'ordre de la course : la plus ancienne, puis le plus petit équipier, puis la plus petite séance.
    fn rang(&self) -> (u64, &str, &str) {
        (self.prise.pris_le, &self.cle.equipier, &self.cle.seance)
    }
}

/// Le bail : une prise est vivante si son battement est récent (horloge de son auteur) ou si j'ai vu sa fiche changer
/// récemment (mon horloge).
pub fn vivante(r: &Revendication, maintenant: u64, bail_ms: u64, obs: &Observations) -> bool {
    let age = maintenant.saturating_sub(r.renouvele_le);
    if age <= bail_ms {
        return true;
    }
    obs.dernier_changement(&r.cle).is_some_and(|t| maintenant.saturating_sub(t) <= bail_ms)
}

/// Le jugement sur une cible.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Evaluation {
    /// Le titulaire : la plus ancienne des prises vivantes que personne n'a reprise.
    pub titulaire: Option<Revendication>,
    /// Les prises dont le bail est expiré et que personne n'a reprise : à reprendre après confirmation.
    pub perimees: Vec<Revendication>,
    /// Les prises vivantes qui ont perdu la course (les autres que le titulaire).
    pub perdantes: Vec<Revendication>,
    /// Le plus grand numéro de révision annoncé par un porteur (prise ou rendue) : le fichier à attendre (§5.4).
    pub revision_annoncee: u64,
}

/// Qui tient `cible`, d'après toutes les fiches lues. `maintenant` et `bail_ms` en millisecondes.
pub fn evaluer(fiches: &[Fiche], cible: Cible, maintenant: u64, bail_ms: u64, obs: &Observations) -> Evaluation {
    let mut toutes: Vec<Revendication> = Vec::new();
    let mut remplacees: Vec<(&str, &str, u64)> = Vec::new();
    let mut revision = 0;
    for f in fiches {
        for p in f.prises.iter().filter(|p| p.cible == cible) {
            revision = revision.max(p.revision);
            for r in &p.remplace {
                remplacees.push((&r.equipier, &r.seance, r.pris_le));
            }
            if !f.rendue_apres(p) {
                toutes.push(Revendication { cle: f.cle(), equipier: f.equipier.clone(), prise: p.clone(), renouvele_le: f.renouvele_le });
            }
        }
        for r in f.rendues.iter().filter(|r| r.cible == cible) {
            revision = revision.max(r.revision);
        }
    }
    // Les prises reprises ne comptent plus.
    toutes.retain(|r| !remplacees.iter().any(|(e, s, t)| *e == r.cle.equipier && *s == r.cle.seance && *t == r.prise.pris_le));
    let (mut vivantes, perimees): (Vec<Revendication>, Vec<Revendication>) = toutes.into_iter().partition(|r| vivante(r, maintenant, bail_ms, obs));
    vivantes.sort_by(|a, b| a.rang().cmp(&b.rang()));
    let mut it = vivantes.into_iter();
    let titulaire = it.next();
    Evaluation { titulaire, perimees, perdantes: it.collect(), revision_annoncee: revision }
}

/// Ce que ma séance voit d'une cible.
#[derive(Clone, Debug, PartialEq)]
pub enum EtatPrise {
    /// Personne ne la tient.
    Libre,
    /// Je la tiens. `provisoire` : ma prise est toute récente, une prise plus ancienne chez un autre peut encore arriver.
    Moi { provisoire: bool },
    /// Quelqu'un d'autre la tient.
    Autre(Revendication),
    /// Personne ne la tient vraiment : la prise de quelqu'un est périmée (à reprendre après confirmation).
    Perimee(Vec<Revendication>),
    /// J'avais pris : un autre est arrivé avant moi, ou m'a repris. `Some` : le titulaire qui m'a devancé.
    Perdu(Option<Revendication>),
}

/// Mon état pour une cible. L'évaluation compte **ma fiche telle que je l'ai en mémoire** (pas celle que le disque a
/// peut-être pas encore reçue) ; `ma_prise` dit si ma séance croit tenir la cible.
pub fn etat_pour(moi: &Cle, ma_prise: bool, eval: &Evaluation, maintenant: u64, confirmation_ms: u64) -> EtatPrise {
    match (&eval.titulaire, ma_prise) {
        (Some(t), true) if &t.cle == moi => EtatPrise::Moi { provisoire: maintenant.saturating_sub(t.prise.pris_le) < confirmation_ms },
        (Some(t), true) => EtatPrise::Perdu(Some(t.clone())),
        (Some(t), false) => EtatPrise::Autre(t.clone()),
        // Personne n'est titulaire alors que je croyais tenir : on m'a repris la place.
        (None, true) => EtatPrise::Perdu(None),
        (None, false) if !eval.perimees.is_empty() => EtatPrise::Perimee(eval.perimees.clone()),
        (None, false) => EtatPrise::Libre,
    }
}
