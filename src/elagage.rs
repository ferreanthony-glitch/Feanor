//! Élaguer les dessins que plus rien ne retient (chantier « mémoire d'une séance », `docs/ENDURANCE.md` §7).
//!
//! `Document::cels` ne se vide jamais : un plan supprimé, une version rangée puis effacée, un dessin d'une étape défaite dont le
//! rétablissement a été jeté, tout y reste jusqu'à la fermeture. Après une longue séance sur un épisode de 300 plans, 40 % des dessins
//! du projet n'étaient plus montrés par rien : 514 Mo de tuiles rangées et 6 millions de points de traits. Ils ne s'enregistrent pas
//! (`Document::used_cels`), mais ils restent en mémoire.
//!
//! Ici, on retire d'`App::doc.cels` ceux dont **rien** ne dépend, et seulement ceux-là :
//! - le document vivant : plans, calques de couleur (griffonnages), contenu rogné, versions des plans, variantes rangées
//!   (`Document::used_cels`, que l'enregistrement utilise déjà) ;
//! - tout l'historique d'annulation et de rétablissement : chaque instantané de structure (les mêmes sources, pour ce qu'il remettrait),
//!   et les dessins que citent les étapes de tuiles, de fond ou de dessin vectoriel ;
//! - rien qui n'ait déjà été sans attache au passage précédent (quinze secondes plus tôt) ; les dessins temporaires (feuille flottante d'une
//!   sélection : numéros réservés) ne sont jamais touchés ;
//! - rien dans un projet partagé (autres équipiers) ni dans un autre onglet.
//!
//! Un dessin retiré est lâché sur un fil de fond (`lacher::en_fond`) : libérer 6 000 dessins sur le fil de l'interface coûterait des
//! dizaines de millisecondes.

use std::collections::{BTreeSet, HashSet};

use crate::document::{CelId, Document, Structure};
use crate::history::{Edit, History};

impl Structure {
    /// Tous les dessins que cet état (un instantané d'annulation) remettrait dans le projet : ceux des plans (rangés ou non), leurs
    /// griffonnages, le contenu rogné, les versions des plans et les variantes rangées.
    pub fn dessins_retenus(&self) -> BTreeSet<CelId> {
        let mut out: BTreeSet<CelId> = self.dessins_des_plans().into_iter().collect();
        for p in &self.panels {
            for l in &p.layers {
                out.extend(l.couleur.iter().flat_map(|c| c.griffonnages.values().copied()));
            }
        }
        for r in self.reserves.values() {
            out.extend(r.cels());
        }
        for v in self.versions.values() {
            for r in &v.autres {
                out.extend(crate::empreinte::dessins_du_plan(&r.panel));
            }
        }
        for v in self.variantes.values() {
            for p in v.garage.values() {
                out.extend(crate::empreinte::dessins_du_plan(&p.panel));
                if let Some(vers) = &p.versions {
                    for r in &vers.autres {
                        out.extend(crate::empreinte::dessins_du_plan(&r.panel));
                    }
                }
                if let Some(r) = &p.reserve {
                    out.extend(r.cels());
                }
            }
        }
        out
    }
}

fn de_l_etape(e: &Edit, out: &mut BTreeSet<CelId>) {
    match e {
        Edit::Tiles { cel, .. } | Edit::FondTiles { cel, .. } | Edit::Vector { cel, .. } => {
            out.insert(*cel);
        }
        Edit::Structure { structure, .. } => out.extend(structure.dessins_retenus()),
        Edit::Group(g) => g.iter().for_each(|e| de_l_etape(e, out)),
    }
}

/// Les dessins que l'historique (étapes à annuler et à rétablir) peut encore demander.
pub fn retenus_par_l_historique(h: &History) -> BTreeSet<CelId> {
    let mut out = BTreeSet::new();
    for e in h.etapes() {
        de_l_etape(e, &mut out);
    }
    out
}

/// Les dessins du document vivant que rien ne cite.
fn retenus_du_document(doc: &Document) -> BTreeSet<CelId> {
    let mut out = doc.used_cels();
    for p in doc.panels.iter().chain(doc.plans_ranges()) {
        for l in &p.layers {
            out.extend(crate::calquecouleur::dessins_du_calque(l));
        }
    }
    out
}

/// Retire de `doc.cels` les dessins que ni le document ni l'historique ne retiennent, et qui ne sont pas retenus non plus depuis le passage
/// précédent (`precedents` : ceux du dernier passage ; un dessin tout juste créé et pas encore posé dans un plan a ainsi au moins un passage,
/// quinze secondes, pour l'être). Les numéros réservés aux dessins temporaires (`flottante::BASE_CEL` et au-dessus : la feuille flottante
/// d'une sélection) ne sont jamais touchés. Rend le nombre de dessins retirés, lâchés sur un fil de fond.
pub fn elaguer(doc: &mut Document, h: &History, precedents: &mut HashSet<CelId>) -> usize {
    let mut retenus = retenus_du_document(doc);
    if doc.cels.len() <= retenus.len() {
        precedents.clear();
        return 0;
    }
    retenus.extend(retenus_par_l_historique(h));
    let orphelins: HashSet<CelId> = doc.cels.keys().copied().filter(|id| *id < crate::flottante::BASE_CEL && !retenus.contains(id)).collect();
    let a_retirer: Vec<CelId> = orphelins.iter().copied().filter(|id| precedents.contains(id)).collect();
    *precedents = orphelins;
    if a_retirer.is_empty() {
        return 0;
    }
    let mut sortis = Vec::with_capacity(a_retirer.len());
    for id in &a_retirer {
        if let Some(c) = doc.cels.remove(id) {
            sortis.push(c);
        }
        precedents.remove(id);
    }
    let n = sortis.len();
    crate::lacher::en_fond(sortis);
    n
}