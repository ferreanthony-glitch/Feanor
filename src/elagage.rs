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
use std::time::{Duration, Instant};

use crate::document::{CelId, Document, Panel, Structure};
use crate::history::{Edit, History};

/// Les dessins de couleur d'un plan (`CalqueCouleur::dessins`, trait → couleur) et ses griffonnages : ils sont retenus même si les images du
/// calque de couleur ne les montrent pas pour l'instant (`calquecouleur::refleter` remplace en silence une couleur absente par un dessin vide).
fn de_la_couleur(p: &Panel, out: &mut HashSet<CelId>) {
    for l in &p.layers {
        if let Some(c) = &l.couleur {
            out.extend(c.dessins.values().copied());
            out.extend(c.griffonnages.values().copied());
        }
    }
}

/// Tous les dessins qu'un plan cite (images, griffonnages, dessins de couleur).
pub(crate) fn dessins_du_plan_retenus(p: &Panel, out: &mut HashSet<CelId>) {
    out.extend(crate::empreinte::dessins_du_plan(p));
    de_la_couleur(p, out);
}

impl Structure {
    /// Ajoute à `out` tous les dessins que cet état (un instantané d'annulation) remettrait dans le projet : ceux des plans (rangés ou non),
    /// leurs griffonnages, le contenu rogné, les versions des plans et les variantes rangées.
    pub fn dessins_retenus_dans(&self, out: &mut HashSet<CelId>) {
        match &self.frames_rangees {
            Some(r) => out.extend(r.dessins()),
            None => {
                for p in &self.panels {
                    for l in &p.layers {
                        out.extend(l.frames.iter().flatten().copied());
                    }
                }
            }
        }
        for p in &self.panels {
            de_la_couleur(p, out);
        }
        for r in self.reserves.values() {
            out.extend(r.cels());
        }
        // La corbeille (`corbeille.rs`) garde ses dessins : annuler une suppression, ou la restaurer, les remet.
        out.extend(self.corbeille.cels());
        // Les plans annulés (`planannule.rs`) gardent les leurs : rétablir, ou annuler une annulation, les remet.
        for g in &self.annules.plans {
            dessins_du_plan_retenus(&g.panel, out);
            for r in g.versions.iter().flat_map(|v| &v.autres) {
                dessins_du_plan_retenus(&r.panel, out);
            }
            out.extend(g.reserve.iter().flat_map(|r| r.cels()));
        }
        for v in self.versions.values() {
            for r in &v.autres {
                dessins_du_plan_retenus(&r.panel, out);
            }
        }
        for v in self.variantes.values() {
            for p in v.garage.values() {
                dessins_du_plan_retenus(&p.panel, out);
                if let Some(vers) = &p.versions {
                    for r in &vers.autres {
                        dessins_du_plan_retenus(&r.panel, out);
                    }
                }
                if let Some(r) = &p.reserve {
                    out.extend(r.cels());
                }
            }
        }
    }

    /// Les dessins que cet état remettrait (essais).
    #[cfg(test)]
    pub fn dessins_retenus(&self) -> BTreeSet<CelId> {
        let mut out = HashSet::new();
        self.dessins_retenus_dans(&mut out);
        out.into_iter().collect()
    }
}

fn de_l_etape(e: &Edit, out: &mut HashSet<CelId>) {
    match e {
        Edit::Tiles { cel, .. } | Edit::FondTiles { cel, .. } | Edit::Vector { cel, .. } => {
            out.insert(*cel);
        }
        Edit::Structure { structure, .. } => structure.dessins_retenus_dans(out),
        Edit::Group(g) => g.iter().for_each(|e| de_l_etape(e, out)),
    }
}

/// Les dessins du document vivant que rien ne cite.
fn retenus_du_document(doc: &Document) -> BTreeSet<CelId> {
    let mut out = doc.used_cels();
    out.extend(doc.corbeille.cels());
    let mut en_plus = HashSet::new();
    for p in doc.panels.iter().chain(doc.plans_ranges()) {
        for l in &p.layers {
            out.extend(crate::calquecouleur::dessins_du_calque(l));
        }
        de_la_couleur(p, &mut en_plus);
    }
    out.extend(en_plus);
    out
}

/// Un parcours de l'historique en cours, par tranches : les dessins que ses étapes peuvent encore demander.
pub struct Balayage {
    /// Le numéro de l'historique au départ : s'il change, les étapes ne sont plus celles qu'on a lues.
    serial: u64,
    prochaine: usize,
    retenus: HashSet<CelId>,
}

impl Balayage {
    pub fn commencer(h: &History) -> Balayage {
        Balayage { serial: h.serial(), prochaine: 0, retenus: HashSet::new() }
    }

    /// Lit des étapes pendant au plus `budget` ; vrai quand toutes sont lues. Faux aussi (et `perime`) si l'historique a changé.
    pub fn avancer(&mut self, h: &History, budget: Duration) -> bool {
        let debut = Instant::now();
        loop {
            let Some(e) = h.etapes().nth(self.prochaine) else { return true };
            de_l_etape(e, &mut self.retenus);
            self.prochaine += 1;
            if debut.elapsed() >= budget {
                return h.etapes().nth(self.prochaine).is_none();
            }
        }
    }

    /// L'historique n'est plus celui qu'on parcourait.
    pub fn est_perime(&self, h: &History) -> bool {
        h.serial() != self.serial
    }
}

/// La fin d'un parcours (`b` entièrement lu, l'historique inchangé) : retire de `doc.cels` les dessins que ni le document ni l'historique ne
/// retiennent, et qui ne l'étaient pas non plus au passage précédent (`precedents` : ceux du dernier passage ; un dessin tout juste créé et
/// pas encore posé dans un plan a ainsi au moins un passage, quinze secondes, pour l'être). Le document vivant est relu ici, d'un coup, au
/// moment de retirer. Les numéros réservés aux dessins temporaires (`flottante::BASE_CEL` et au-dessus : la feuille flottante d'une
/// sélection) ne sont jamais touchés. Rend le nombre de dessins retirés, lâchés sur un fil de fond.
pub fn conclure(doc: &mut Document, b: Balayage, precedents: &mut HashSet<CelId>) -> usize {
    let mut retenus = b.retenus;
    retenus.extend(retenus_du_document(doc));
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
