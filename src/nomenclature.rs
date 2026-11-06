//! La nomenclature des exports (partie 105) : comment se nomment les fichiers des exports par plan (images par plan, par case, à la clé, par
//! calque à la clé, son par plan), sous plusieurs formes, **avec le même modèle à jetons que « Noms et production »** (`nommage.rs`, partie 87) :
//! rien n'est réécrit ici, on assemble des modèles de `nommage` (le jeton `{cle}` s'y ajoute pour ces exports, `Jeton::TOUS_EXPORT`).
//!
//! - **Formes prêtes** : `SQ01_PL012A_K03` (le modèle du projet suivi de `_K03` : les noms d'aujourd'hui, d'origine), `sq01_pl012a_k03`,
//!   `S01-P012A-K03`, `EP01_SQ01_PL012A_K03`, `012A_03`. **Personnalisée** : un modèle à jetons écrit à la main (projet, épisode, séquence, plan,
//!   clé, calque, version, date, séparateurs libres).
//! - **Réglages** : chiffres de la séquence, du plan et de la clé (0 : ceux du modèle ; la clé : le nombre de chiffres commun à tout l'export,
//!   au moins deux), majuscules ou minuscules, place du nom de calque quand le modèle n'a pas `{calque}` (à la fin ou au début).
//! - **Sûreté** : un modèle qui ne se lit pas, ou sans `{cle}` pour des clés, est refusé ; deux fichiers de même nom (sans tenir compte des
//!   majuscules : Windows ne les distingue pas) sont refusés avec les deux noms ; les formes du projet gardent le suffixe `_2` des plans de même
//!   nom, comme avant. Les caractères que Windows refuse sont remplacés par « _ » ; accents, espaces et points comme dans tous les
//!   noms de fichiers des exports (`editorial::file_safe` : accent ramené à sa lettre, espace et point en « _ »).
//! - **Rien ne change par défaut** : la forme d'origine sans réglage (`est_defaut`) passe par les fonctions d'avant (`exportstills::items`,
//!   `exportcles::items`…), donc les mêmes noms qu'avant.
//!
//! Le choix est retenu par poste de travail avec les autres choix d'export (`ExportPrefs`), pas dans le projet : le format de projet ne change pas.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::nommage::{self, Contexte, Jeton, Piece};

/// Les formes de nom proposées.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Forme {
    /// Le modèle du projet, puis `_K03` : les noms d'origine.
    #[default]
    Defaut,
    DefautMinuscules,
    Tirets,
    Episode,
    Court,
    Perso,
}

impl Forme {
    pub const TOUS: [Forme; 6] = [Forme::Defaut, Forme::DefautMinuscules, Forme::Tirets, Forme::Episode, Forme::Court, Forme::Perso];

    pub fn exemple(self) -> &'static str {
        match self {
            Forme::Defaut => "SQ01_PL012A_K03",
            Forme::DefautMinuscules => "sq01_pl012a_k03",
            Forme::Tirets => "S01-P012A-K03",
            Forme::Episode => "EP01_SQ01_PL012A_K03",
            Forme::Court => "012A_03",
            Forme::Perso => "Personnalisée",
        }
    }

    /// Le modèle du plan (rien : celui du projet) et ce qui s'y ajoute pour une clé.
    fn modeles(self) -> (Option<&'static str>, &'static str) {
        match self {
            Forme::Defaut | Forme::DefautMinuscules => (None, "_K{cle}"),
            Forme::Tirets => (Some("S{sequence:02}-P{plan:03}"), "-K{cle}"),
            Forme::Episode => (Some("EP{episode:02}_SQ{sequence:02}_PL{plan:03}"), "_K{cle}"),
            Forme::Court => (Some("{plan:03}"), "_{cle}"),
            Forme::Perso => (Some(""), ""),
        }
    }

    /// Le modèle du projet fait le nom du plan (ses plans de même nom prennent « _2 », comme avant).
    fn du_projet(self) -> bool {
        matches!(self, Forme::Defaut | Forme::DefautMinuscules)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Casse {
    #[default]
    Inchangee,
    Majuscules,
    Minuscules,
}

/// Un nom à fabriquer : le plan, et selon l'export la clé, la case, le calque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entree {
    pub plan: usize,
    pub cle: Option<usize>,
    pub case: Option<usize>,
    pub calque: Option<String>,
}

/// Les choix de nomenclature d'une fenêtre d'export.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Nomenclature {
    pub forme: Forme,
    /// Le modèle de la forme personnalisée (jetons de `nommage.rs` et `{cle}`).
    pub modele: String,
    /// Chiffres de la séquence, du plan, de la clé ; 0 : ceux du modèle (la clé : automatique).
    pub chiffres_sequence: u8,
    pub chiffres_plan: u8,
    pub chiffres_cle: u8,
    pub casse: Casse,
    /// Le nom du calque en tête du nom plutôt qu'à la fin (quand le modèle n'a pas `{calque}`).
    pub calque_au_debut: bool,
}

impl Default for Nomenclature {
    fn default() -> Self {
        Nomenclature { forme: Forme::Defaut, modele: "{projet}_EP{episode:02}_SQ{sequence:02}_PL{plan:03}_K{cle:02}".into(), chiffres_sequence: 0, chiffres_plan: 0, chiffres_cle: 0, casse: Casse::Inchangee, calque_au_debut: false }
    }
}

struct Pieces {
    base: Vec<Piece>,
    suffixe: Vec<Piece>,
}

impl Nomenclature {
    /// La forme d'origine, sans aucun réglage : les noms d'avant.
    pub fn est_defaut(&self) -> bool {
        self.forme == Forme::Defaut && self.casse == Casse::Inchangee && self.chiffres_sequence == 0 && self.chiffres_plan == 0 && self.chiffres_cle == 0 && !self.calque_au_debut
    }

    fn casse_effective(&self) -> Casse {
        if self.forme == Forme::DefautMinuscules && self.casse == Casse::Inchangee {
            Casse::Minuscules
        } else {
            self.casse
        }
    }

    fn pieces(&self, doc: &Document, cle_auto: usize) -> Result<Pieces, String> {
        let (base, suffixe) = self.forme.modeles();
        let mut p = match self.forme {
            Forme::Perso => Pieces { base: nommage::analyser_avec(self.modele.trim(), &Jeton::TOUS_EXPORT)?, suffixe: Vec::new() },
            _ => Pieces {
                base: match base {
                    None => doc.modele_de_noms(),
                    Some(t) => nommage::analyser_avec(t, &Jeton::TOUS_EXPORT)?,
                },
                suffixe: nommage::analyser_avec(suffixe, &Jeton::TOUS_EXPORT)?,
            },
        };
        for piece in p.base.iter_mut().chain(p.suffixe.iter_mut()) {
            if let Piece::Jeton { jeton, largeur } = piece {
                match jeton {
                    Jeton::Sequence if self.chiffres_sequence > 0 => *largeur = self.chiffres_sequence as usize,
                    Jeton::Plan if self.chiffres_plan > 0 => *largeur = self.chiffres_plan as usize,
                    Jeton::Cle if self.chiffres_cle > 0 => *largeur = self.chiffres_cle as usize,
                    Jeton::Cle if *largeur == 0 => *largeur = cle_auto,
                    _ => {}
                }
            }
        }
        Ok(p)
    }

    /// Ce que la fenêtre dit du modèle personnalisé avant l'export (rien : il est bon).
    pub fn avertissements(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.forme == Forme::Perso {
            if let Ok(p) = nommage::analyser_avec(self.modele.trim(), &Jeton::TOUS_EXPORT) {
                let mut interdits: Vec<char> = Vec::new();
                for piece in &p {
                    if let Piece::Texte(t) = piece {
                        for c in t.chars().filter(|c| nommage::caractere_interdit(*c)) {
                            if !interdits.contains(&c) {
                                interdits.push(c);
                            }
                        }
                    }
                }
                if !interdits.is_empty() {
                    v.push(format!("Windows refuse « {} » dans un nom de fichier : il sera remplacé par « _ ».", interdits.iter().map(char::to_string).collect::<Vec<_>>().join(" ")));
                }
            }
        }
        v
    }

    /// Les noms (sans extension) des `entrees`, dans le même ordre. `cle_auto` : le nombre de chiffres de la clé quand rien ne le fixe.
    /// Erreur : le modèle ne se lit pas, n'a pas `{cle}` pour des clés, ou deux fichiers auraient le même nom.
    pub fn noms(&self, doc: &Document, entrees: &[Entree], cle_auto: usize) -> Result<Vec<String>, String> {
        let p = self.pieces(doc, cle_auto)?;
        let avec_cle = entrees.iter().any(|e| e.cle.is_some());
        if avec_cle && self.forme == Forme::Perso && !nommage::contient(&p.base, Jeton::Cle) {
            return Err("le modèle n'a pas de champ {cle} : deux clés d'un même plan porteraient le même nom".into());
        }
        let calque_dans_modele = nommage::contient(&p.base, Jeton::Calque) || nommage::contient(&p.suffixe, Jeton::Calque);
        let mut contextes: HashMap<usize, Contexte> = HashMap::new();
        // Les plans de même nom (formes du projet) : le deuxième prend « _2 », comme les images par plan d'avant.
        let mut doubles: HashMap<usize, String> = HashMap::new();
        if self.forme.du_projet() {
            let mut vus: HashMap<String, usize> = HashMap::new();
            for e in entrees {
                if doubles.contains_key(&e.plan) || contextes.contains_key(&e.plan) {
                    continue;
                }
                let cx = doc.contexte_de_nom(e.plan);
                let base = crate::editorial::file_safe(&doc.nom_avec_contexte(&p.base, e.plan, Contexte { calque: None, ..cx.clone() })).to_lowercase();
                contextes.insert(e.plan, cx);
                let n = vus.entry(base).and_modify(|n| *n += 1).or_insert(1);
                doubles.insert(e.plan, if *n > 1 { format!("_{n}") } else { String::new() });
            }
        }
        let casse = self.casse_effective();
        let mut out: Vec<String> = Vec::with_capacity(entrees.len());
        for e in entrees {
            let mut cx = contextes.entry(e.plan).or_insert_with(|| doc.contexte_de_nom(e.plan)).clone();
            cx.case = e.case;
            cx.calque = e.calque.clone();
            let mut nom = if self.forme == Forme::Perso {
                cx.cle = e.cle;
                doc.nom_avec_contexte(&p.base, e.plan, cx)
            } else {
                let mut b = crate::editorial::file_safe(&doc.nom_avec_contexte(&p.base, e.plan, cx.clone()));
                b.push_str(doubles.get(&e.plan).map_or("", String::as_str));
                if e.cle.is_some() {
                    cx.cle = e.cle;
                    b.push_str(&nommage::remplir(&p.suffixe, &cx));
                }
                b
            };
            if let (Some(c), false) = (&e.calque, calque_dans_modele) {
                let c = crate::exportstems::file_name_of(c, "Calque");
                let sep = if nom.contains('-') && !nom.contains('_') { '-' } else { '_' };
                nom = if self.calque_au_debut { format!("{c}{sep}{nom}") } else { format!("{nom}{sep}{c}") };
            }
            let mut nom = crate::editorial::file_safe(&nom);
            match casse {
                Casse::Inchangee => {}
                Casse::Majuscules => nom = nom.to_uppercase(),
                Casse::Minuscules => nom = nom.to_lowercase(),
            }
            out.push(nom);
        }
        let mut vus: HashMap<String, usize> = HashMap::new();
        for (i, n) in out.iter().enumerate() {
            if let Some(j) = vus.insert(n.to_lowercase(), i) {
                return Err(format!("deux fichiers porteraient le même nom : « {} » et « {n} » (Windows ne distingue pas les majuscules). Ajoute au modèle ce qui les distingue : plan, clé, calque…", out[j]));
            }
        }
        Ok(out)
    }
}
