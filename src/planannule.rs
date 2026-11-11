//! Le plan annulé (partie 115 de `docs/INTERFACE.md`, format 48) ; à l'écran, le plan **écarté** : « Écarter », « Reprendre » (le mot « annuler » est à Ctrl+Z). Demande du réalisateur : « Un plan annulé disparait partout, y compris
//! dans storyboard » ; clic sur le numéro du plan dans les Vignettes = croix rouge dans le cadre.
//!
//! **Le seul point de filtrage : `Document::panels` est le film.** Un plan annulé **sort** de `panels` et va dans `Document::annules`, avec tout
//! ce qui tient à lui (contenu rogné, versions, commentaires, transitions, liens de références) : Storyboard, timeline, montage, lecture,
//! durée, tous les exports et Kitsu ne lisent que `panels`, ils ne le voient donc pas, sans aucune retouche de leur côté. Seule la planche à
//! l'écran le remet à sa place (`board::layout_ecran`, `BoardLayout::annulees`), et les numéros le comptent (`scene_order_planche`).
//!
//! **Sa place** : `apres` (ce qui le précédait sur la planche, plan du film ou plan annulé) ; s'il n'existe plus, `suivant` ; sinon la fin
//! du film (`ordre_planche`). Couper ou réunir le plan d'avant le garde au même endroit (`annules_suivre_coupe`, `annules_suivre_jonction`).
//!
//! **Tout le film annulé** : `panels` ne peut jamais être vide (tout le logiciel le suppose). Un plan vide, le **support**
//! (`Annules::support`), le tient : aucun numéro de case, pas de case sur la planche, les exports refusent (`film_vide`), le dessin du Storyboard
//! montre un état vide clair. Il part de lui-même dès qu'un autre plan entre dans le film (`normalize`, `annules_menage`) ; s'il a reçu quelque
//! chose entre-temps (une clé collée, un texte), il reste, comme un plan ordinaire : rien ne se perd.
//!
//! **Versions de séquence** : comme la suppression, l'annulation ne vaut que pour la version de séquence où l'on est (`variante`). Un plan qu'une
//! autre version montre va aussi au garage (`ranger_plan_partage`) : c'est lui qui revient au rétablissement, avec ce qu'on y a retouché.
//!
//! **Le son ne bouge pas** : les pistes sont en temps du montage et ne bougent avec aucun plan (`Document::audio_tracks`). Annuler raccourcit
//! le film d'images, le son reste où il est, et rétablir retrouve l'accord d'avant. La ligne d'état le dit.
//!
//! **Le fichier** : `plans_annules.json` à côté du manifeste (aucun plan annulé : pas de fichier). Les plans passent par la lecture ordinaire du
//! manifeste, dans une séquence cachée (`project::cacher_annules`), comme ceux du garage : leurs dessins se chargent avec les autres. Un fichier
//! illisible ou plus récent n'est jamais écrasé, et l'enregistrement ne nettoie alors pas `dessins/` (`illisible`). **Un film partagé** ne
//! connaît pas l'annulation : le geste y est refusé, et partager un film qui a des plans annulés aussi (`App::partage_refuse_annules`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::commentaires::Comment;
use crate::document::{CelId, Document, Panel, Scene, Sequence};
use crate::planvers::VersionsDuPlan;
use crate::rognage::Reserve;
use crate::transitions::Transition;

pub(crate) const FILE: &str = "plans_annules.json";
const FORMAT: &str = "feanor-plans-annules";
const VERSION: u32 = 1;

/// Un plan sorti du film par une annulation, et tout ce qui tient à lui.
#[derive(Clone)]
pub struct PlanAnnule {
    pub panel: Panel,
    pub scene: Scene,
    pub sequence: Sequence,
    /// La version de séquence où il est annulé (`Document::numero_de_variante`) : les autres ne le montrent pas.
    pub variante: u32,
    /// Ce qui le précédait sur la planche, et ce qui le suivait (des numéros de plan).
    pub apres: Option<u64>,
    pub suivant: Option<u64>,
    pub reserve: Option<Reserve>,
    pub commentaires: Vec<Comment>,
    pub versions: Option<VersionsDuPlan>,
    /// Les références (`references.rs`) liées à ce plan.
    pub references: Vec<u64>,
    /// Secondes depuis 1970.
    pub date: u64,
}

/// Les plans annulés du projet. Partagés (`Arc`) : une étape d'annulation ne les recopie pas.
#[derive(Clone, Default)]
pub struct Annules {
    pub plans: Vec<Arc<PlanAnnule>>,
    /// Le plan vide qui tient `panels` quand tout le film est annulé (voir le début du fichier).
    pub support: Option<u64>,
    /// Les transitions qui attendent que leurs deux plans soient de nouveau voisins dans le film : jamais jetées.
    pub transitions: Vec<Transition>,
    /// Les entrées abîmées du fichier, telles quelles : réécrites à chaque enregistrement, et tant qu'il y en a, `dessins/` n'est pas nettoyé.
    pub illisibles: Vec<serde_json::Value>,
}

impl Annules {
    pub fn is_empty(&self) -> bool {
        self.plans.is_empty() && self.support.is_none()
    }

    /// Rien à écrire dans `plans_annules.json` : le fichier part.
    pub fn rien_a_ecrire(&self) -> bool {
        self.is_empty() && self.transitions.is_empty() && self.illisibles.is_empty()
    }
}

/// Une place de la planche : un plan du film (son rang dans `panels`) ou un plan annulé (son rang dans `annules.plans`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rang {
    Film(usize),
    Annule(usize),
}

fn maintenant() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Document {
    /// Ce plan annulé se montre-t-il ? Oui dans la version de séquence où il l'a été (ou si cette version n'existe plus).
    pub(crate) fn annule_visible(&self, g: &PlanAnnule) -> bool {
        // (Un plan qui est dans le film n'est pas aussi écarté : jamais de double.)
        if self.panels.iter().any(|p| p.id == g.panel.id) {
            return false;
        }
        match self.variantes.get(&g.sequence.id) {
            None => true,
            Some(v) => g.variante == v.actuelle || !v.autres.iter().any(|r| r.numero == g.variante),
        }
    }

    /// Les transitions qui attendent reviennent dans le film dès que leurs deux plans y sont voisins (`normalize`).
    pub(crate) fn annules_reposer_transitions(&mut self) {
        if self.annules.transitions.is_empty() {
            return;
        }
        let rang: HashMap<u64, usize> = self.panels.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        let n = self.panels.len();
        for t in std::mem::take(&mut self.annules.transitions) {
            let tient = match (t.out, t.inn) {
                (Some(a), Some(b)) => matches!((rang.get(&a), rang.get(&b)), (Some(x), Some(y)) if x + 1 == *y),
                (Some(a), None) => rang.get(&a).is_some_and(|x| x + 1 == n),
                (None, Some(b)) => rang.get(&b) == Some(&0),
                (None, None) => false,
            };
            if tient && !self.transitions.iter().any(|x| x.out == t.out && x.inn == t.inn) {
                self.transitions.push(t);
            } else {
                self.annules.transitions.push(t);
            }
        }
    }

    /// La version `numero` de la séquence `sequence` vient de disparaître (`supprimer_variante`), le garage n'est pas encore nettoyé. Un plan écarté dans
    /// cette version qu'une autre montre encore n'est plus écarté nulle part : il n'a plus de case barrée. Un plan écarté que plus aucune version ne
    /// montre reprend au garage tout ce qui tient à lui (ses versions, son contenu rogné, ses commentaires), qui sinon y disparaîtrait.
    pub(crate) fn annules_apres_une_version_supprimee(&mut self, sequence: u64, numero: u32) {
        let Some(v) = self.variantes.get(&sequence) else { return };
        let actuelle = v.actuelle;
        let montres: HashSet<u64> = v.autres.iter().flat_map(|r| r.contenu.slots.iter().map(|s| s.plan)).chain(self.panels.iter().map(|p| p.id)).collect();
        for k in (0..self.annules.plans.len()).rev() {
            let g = &self.annules.plans[k];
            if g.sequence.id != sequence {
                continue;
            }
            let (id, de_cette_version) = (g.panel.id, g.variante == numero);
            if montres.contains(&id) {
                if de_cette_version {
                    self.annules.plans.remove(k);
                }
                continue;
            }
            self.annules_rapatrier(k);
            if de_cette_version {
                Arc::make_mut(&mut self.annules.plans[k]).variante = actuelle;
            }
        }
    }

    /// Le plan écarté `k` reprend au garage ce qui tient à lui, s'il y est.
    fn annules_rapatrier(&mut self, k: usize) {
        let id = self.annules.plans[k].panel.id;
        let Some((mut panel, versions, reserve, commentaires)) = self.reprendre_du_garage(id) else { return };
        let g = Arc::make_mut(&mut self.annules.plans[k]);
        crate::variantes::poser_scene(&mut panel, g.panel.scene);
        g.panel = panel;
        g.versions = versions;
        g.reserve = reserve;
        g.commentaires = commentaires;
    }

    /// Les versions de la séquence `sequence` vont disparaître avec elle (`elaguer_variantes`) : ses plans écartés reprennent au garage ce qui tient à eux.
    pub(crate) fn annules_rapatrier_la_sequence(&mut self, sequence: u64) {
        for k in 0..self.annules.plans.len() {
            if self.annules.plans[k].sequence.id == sequence {
                self.annules_rapatrier(k);
            }
        }
    }

    /// Les rangs (dans `annules.plans`) des plans annulés qui se montrent.
    pub(crate) fn annules_visibles(&self) -> Vec<usize> {
        (0..self.annules.plans.len()).filter(|&k| self.annule_visible(&self.annules.plans[k])).collect()
    }

    /// L'ordre de lecture de la planche : le film, et les plans annulés qui se montrent, chacun à sa place ; sans le support. Une séquence reste
    /// d'un bloc. Toujours fini : un lien qui tourne en rond ou qui ne mène à rien passe à la fin du film.
    pub fn ordre_planche(&self) -> Vec<Rang> {
        let visibles = self.annules_visibles();
        let support = self.annules.support;
        if visibles.is_empty() {
            return self.panels.iter().enumerate().filter(|(_, p)| Some(p.id) != support).map(|(i, _)| Rang::Film(i)).collect();
        }
        let ids_film: HashSet<u64> = self.panels.iter().map(|p| p.id).collect();
        let ids_annules: HashSet<u64> = visibles.iter().map(|&k| self.annules.plans[k].panel.id).collect();
        let existe = |id: u64| ids_film.contains(&id) || ids_annules.contains(&id);
        let (mut derriere, mut devant): (HashMap<u64, Vec<usize>>, HashMap<u64, Vec<usize>>) = (HashMap::new(), HashMap::new());
        let (mut tete, mut queue): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for &k in &visibles {
            let g = &self.annules.plans[k];
            // Rien devant lui quand il a été annulé : il reste en tête. Ce qui le précédait est là : derrière lui.
            let Some(apres) = g.apres else {
                tete.push(k);
                continue;
            };
            if existe(apres) && apres != g.panel.id {
                derriere.entry(apres).or_default().push(k);
                continue;
            }
            // Ce qui le précédait n'est plus là : devant ce qui le suivait, en remontant les plans annulés attachés derrière lui (leur chaîne suit).
            let mut cible = g.suivant;
            let mut pas = 0;
            while let Some(s) = cible.filter(|s| !ids_film.contains(s)) {
                pas += 1;
                cible = if pas > visibles.len() { None } else { visibles.iter().map(|&j| &self.annules.plans[j]).find(|h| h.panel.id == s).and_then(|h| h.suivant) };
            }
            match cible {
                Some(s) => devant.entry(s).or_default().push(k),
                None => queue.push(k),
            }
        }
        let mut sortie: Vec<Rang> = Vec::with_capacity(self.panels.len() + visibles.len());
        let mut vus: HashSet<usize> = HashSet::new();
        // (Une pile, pas une récursion : ce qui suit un plan annulé se pose juste derrière lui.)
        fn poser(k: usize, plans: &[Arc<PlanAnnule>], derriere: &HashMap<u64, Vec<usize>>, devant: &HashMap<u64, Vec<usize>>, vus: &mut HashSet<usize>, sortie: &mut Vec<Rang>) {
            let mut pile = vec![(k, false)];
            while let Some((k, fait)) = pile.pop() {
                let id = plans[k].panel.id;
                if fait {
                    sortie.push(Rang::Annule(k));
                    for &d in derriere.get(&id).map_or(&[][..], |v| &v[..]).iter().rev() {
                        if vus.insert(d) {
                            pile.push((d, false));
                        }
                    }
                } else {
                    pile.push((k, true));
                    for &d in devant.get(&id).map_or(&[][..], |v| &v[..]).iter().rev() {
                        if vus.insert(d) {
                            pile.push((d, false));
                        }
                    }
                }
            }
        }
        let plans = &self.annules.plans;
        for &k in &tete {
            if vus.insert(k) {
                poser(k, plans, &derriere, &devant, &mut vus, &mut sortie);
            }
        }
        for (i, p) in self.panels.iter().enumerate() {
            for &k in devant.get(&p.id).map_or(&[][..], |v| &v[..]) {
                if vus.insert(k) {
                    poser(k, plans, &derriere, &devant, &mut vus, &mut sortie);
                }
            }
            if Some(p.id) != support {
                sortie.push(Rang::Film(i));
            }
            for &k in derriere.get(&p.id).map_or(&[][..], |v| &v[..]) {
                if vus.insert(k) {
                    poser(k, plans, &derriere, &devant, &mut vus, &mut sortie);
                }
            }
        }
        // Ce qui reste : un lien vers rien, ou une boucle (un fichier abîmé).
        for k in queue.into_iter().chain(visibles.iter().copied()) {
            if vus.insert(k) {
                poser(k, plans, &derriere, &devant, &mut vus, &mut sortie);
            }
        }
        // Une séquence reste d'un bloc, dans l'ordre de première apparition.
        let de_scene: HashMap<u64, u64> = self.scenes.iter().map(|s| (s.id, s.sequence)).collect();
        let mut rang_seq: HashMap<u64, usize> = HashMap::new();
        let mut cles = Vec::with_capacity(sortie.len());
        for r in &sortie {
            let q = match *r {
                Rang::Film(i) => de_scene.get(&self.panels[i].scene).copied().unwrap_or(0),
                Rang::Annule(k) => plans[k].sequence.id,
            };
            let n = rang_seq.len();
            cles.push(*rang_seq.entry(q).or_insert(n));
        }
        let mut indices: Vec<usize> = (0..sortie.len()).collect();
        indices.sort_by_key(|&i| cles[i]);
        indices.into_iter().map(|i| sortie[i]).collect()
    }

    /// Les scènes dans l'ordre de la planche, annulées comprises : c'est ce qui donne les numéros (rien n'est renuméroté par une annulation).
    pub fn scene_order_planche(&self) -> Vec<u64> {
        if self.annules.is_empty() {
            return self.scene_order();
        }
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for r in self.ordre_planche() {
            let s = match r {
                Rang::Film(i) => self.panels[i].scene,
                Rang::Annule(k) => self.annules.plans[k].panel.scene,
            };
            if seen.insert(s) {
                out.push(s);
            }
        }
        // (Le support a le numéro d'après.)
        for p in &self.panels {
            if seen.insert(p.scene) {
                out.push(p.scene);
            }
        }
        out
    }

    /// Les séquences dans l'ordre de la planche, celles dont tous les plans sont annulés comprises.
    pub fn sequence_order_planche(&self) -> Vec<u64> {
        if self.annules.is_empty() {
            return self.sequence_order();
        }
        let de_scene: HashMap<u64, u64> = self.scenes.iter().map(|s| (s.id, s.sequence)).collect();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for r in self.ordre_planche() {
            let q = match r {
                Rang::Film(i) => de_scene.get(&self.panels[i].scene).copied().unwrap_or(0),
                Rang::Annule(k) => self.annules.plans[k].sequence.id,
            };
            if seen.insert(q) {
                out.push(q);
            }
        }
        for q in self.sequence_ids() {
            if seen.insert(q) {
                out.push(q);
            }
        }
        out
    }

    /// La scène `id`, du film ou d'un plan annulé.
    pub(crate) fn scene_ou_annulee(&self, id: u64) -> Option<&Scene> {
        self.scene(id).or_else(|| self.annules.plans.iter().find(|g| g.panel.scene == id).map(|g| &g.scene))
    }

    /// La séquence `id`, du film ou d'un plan annulé.
    pub(crate) fn sequence_ou_annulee(&self, id: u64) -> Option<&Sequence> {
        self.sequences.iter().find(|s| s.id == id).or_else(|| self.annules.plans.iter().find(|g| g.sequence.id == id).map(|g| &g.sequence))
    }

    /// Le plan du film a-t-il autre chose que du vide (un trait, un texte, une caméra, une réserve) ?
    fn plan_a_du_contenu(&self, p: &Panel) -> bool {
        !p.action.is_empty()
            || !p.dialogue.is_empty()
            || !p.notes.is_empty()
            || !p.marks.is_empty()
            || !p.camera.is_empty()
            || p.effet_camera.is_some()
            || p.layers.len() > 1
            || p.layers.iter().any(|l| l.frames.iter().flatten().any(|c| self.cel_a_du_trait(*c)))
    }

    /// Tout le film est-il annulé (il ne reste que le support, vide) ? Les exports refusent alors, et le Storyboard montre un état vide.
    pub fn film_vide(&self) -> bool {
        match (self.annules.support, self.panels.as_slice()) {
            (Some(s), [p]) => p.id == s && !self.plan_a_du_contenu(p),
            _ => false,
        }
    }

    /// Le support n'a plus lieu d'être : un autre plan est entré dans le film. Appelé par `normalize`.
    pub(crate) fn annules_menage(&mut self) {
        let Some(s) = self.annules.support else { return };
        if self.panels.len() > 1 {
            if let Some(i) = self.panels.iter().position(|p| p.id == s) {
                if !self.plan_a_du_contenu(&self.panels[i]) {
                    self.panels.remove(i);
                }
            }
            self.annules.support = None;
        } else if self.annules.plans.is_empty() || self.panels.first().is_none_or(|p| p.id != s) {
            self.annules.support = None;
        }
    }

    /// Le plan `first` vient d'être coupé, `second` est sa suite : ce qui était annulé derrière lui reste derrière la suite.
    pub(crate) fn annules_suivre_coupe(&mut self, first: u64, second: u64) {
        for g in &mut self.annules.plans {
            if g.apres == Some(first) {
                Arc::make_mut(g).apres = Some(second);
            }
        }
    }

    /// Le plan `absorbe` vient d'être réuni à `hote` : ce qui s'attachait à lui s'attache à l'hôte.
    pub(crate) fn annules_suivre_jonction(&mut self, absorbe: u64, hote: u64) {
        for g in &mut self.annules.plans {
            if g.apres == Some(absorbe) {
                Arc::make_mut(g).apres = Some(hote);
            }
            if g.suivant == Some(absorbe) {
                Arc::make_mut(g).suivant = Some(hote);
            }
        }
    }

    /// Annule le plan `id` : voir le début du fichier. Rend le rang du plan à rendre courant (le voisin), ou rien (et rien n'a changé, le refus
    /// est noté : `variantes::noter_refus`).
    pub fn annuler_plan(&mut self, id: u64) -> Option<usize> {
        let index = self.panels.iter().position(|p| p.id == id)?;
        if self.annules.support == Some(id) {
            return None;
        }
        if self.annules.plans.iter().any(|g| g.panel.id == id) {
            crate::variantes::noter_refus("Ce plan est déjà écarté dans une autre version de la séquence : reprends-le d'abord là-bas.".into());
            return None;
        }
        // (Le dernier plan d'une séquence qui a d'autres versions : elles disparaîtraient avec elle.)
        if self.refuser_si_sequence_videe(|p| p == id) {
            return None;
        }
        let ordre = self.ordre_planche();
        let pos = ordre.iter().position(|r| *r == crate::planannule::Rang::Film(index));
        let voisin = |r: Option<&Rang>| {
            r.map(|r| match *r {
                Rang::Film(i) => self.panels[i].id,
                Rang::Annule(k) => self.annules.plans[k].panel.id,
            })
        };
        let apres = pos.and_then(|p| p.checked_sub(1)).and_then(|p| voisin(ordre.get(p)));
        let suivant = pos.and_then(|p| voisin(ordre.get(p + 1)));
        let scene_id = self.panels[index].scene;
        let scene = self.scene(scene_id).cloned().unwrap_or(Scene { id: scene_id, name: String::new(), number: String::new(), sequence: self.sequence_of(index) });
        let seq_id = scene.sequence;
        let sequence = self.sequences.iter().find(|s| s.id == seq_id).cloned().unwrap_or(Sequence { id: seq_id, name: String::new(), number: String::new() });
        let variante = self.numero_de_variante(seq_id);
        // (Un plan qu'une autre version de sa séquence montre est gardé au garage, avec ce qui tient à lui.)
        self.ranger_plan_partage(id, false);
        let panel = self.panels[index].clone();
        let versions = self.versions.remove(&id);
        let reserve = self.reserves.remove(&id);
        let (commentaires, gardes): (Vec<Comment>, Vec<Comment>) = std::mem::take(&mut self.comments).into_iter().partition(|c| c.panel == id);
        self.comments = gardes;
        let (transitions, gardees): (Vec<Transition>, Vec<Transition>) = std::mem::take(&mut self.transitions).into_iter().partition(|t| t.out == Some(id) || t.inn == Some(id));
        self.transitions = gardees;
        self.annules.transitions.extend(transitions);
        let references: Vec<u64> = self.references.iter().filter(|r| r.plans.contains(&id)).map(|r| r.id).collect();
        // Le dernier plan du film : un plan vide tient la place, dans sa séquence.
        if self.panels.len() == 1 {
            let s = self.new_scene(seq_id);
            let p = self.make_panel(s);
            self.annules.support = Some(p.id);
            self.panels.push(p);
        }
        self.annules.plans.push(Arc::new(PlanAnnule { panel, scene, sequence, variante, apres, suivant, reserve, commentaires, versions, references, date: maintenant() }));
        self.panels.remove(index);
        let next = self.panels[index.min(self.panels.len() - 1)].id;
        Some(self.normalize_keeping(next))
    }

    /// Rétablit le plan annulé `id` à sa place, tel qu'il était. Rend son rang, ou rien (il n'est pas annulé ici).
    pub fn retablir_plan(&mut self, id: u64) -> Option<usize> {
        let k = self.annules.plans.iter().position(|g| g.panel.id == id)?;
        if !self.annule_visible(&self.annules.plans[k]) {
            return None;
        }
        // Sa place : juste après ce qui le précède dans le film, sur la planche.
        let ordre = self.ordre_planche();
        let pos = ordre.iter().position(|r| *r == Rang::Annule(k))?;
        let support = self.annules.support;
        let at = ordre[..pos]
            .iter()
            .rev()
            .find_map(|r| match *r {
                Rang::Film(i) if Some(self.panels[i].id) != support => Some(i + 1),
                _ => None,
            })
            .unwrap_or(0);
        let g = self.annules.plans.remove(k);
        let g = Arc::try_unwrap(g).unwrap_or_else(|a| (*a).clone());
        let PlanAnnule { mut panel, mut scene, sequence, references, reserve, commentaires, versions, .. } = g;
        // Une autre version de la séquence le montrait : il est au garage, peut-être retouché depuis. C'est lui qui revient.
        let (reserve, commentaires, versions) = match self.reprendre_du_garage(id) {
            Some((p, v, r, c)) => {
                panel = p;
                (r, c, v)
            }
            None => (reserve, commentaires, versions),
        };
        // (Un plan qui a pris ce numéro entre-temps, venu d'une autre version de séquence : un numéro neuf, comme la corbeille.)
        if self.panels.iter().any(|p| p.id == panel.id) {
            panel.id = self.alloc();
        }
        let nouveau = panel.id;
        // Sa scène, si elle a encore d'autres panneaux : il y revient, rien ne bouge. Sinon sa séquence et sa scène reviennent avec lui, avec leurs noms et
        // leurs numéros.
        if !self.scenes.iter().any(|s| s.id == scene.id) {
            if !self.sequences.iter().any(|s| s.id == sequence.id) {
                let mut s = sequence.clone();
                if self.numbering_locked && self.sequences.iter().any(|q| q.number == s.number) {
                    s.number.clear();
                }
                self.sequences.push(s);
            }
            if self.numbering_locked && self.scenes.iter().any(|s| !s.number.is_empty() && s.number == scene.number) {
                scene.number.clear();
            }
            scene.sequence = sequence.id;
            self.scenes.push(scene.clone());
        }
        crate::variantes::poser_scene(&mut panel, scene.id);
        if let Some(r) = reserve {
            self.reserves.insert(nouveau, r);
        }
        if let Some(v) = versions {
            self.versions.insert(nouveau, v);
        }
        for mut c in commentaires {
            c.panel = nouveau;
            self.comments.push(c);
        }
        // (Ses transitions attendent dans `annules.transitions` : `normalize` les repose si leur coupe existe de nouveau.)
        for t in &mut self.annules.transitions {
            t.out = t.out.map(|o| if o == id { nouveau } else { o });
            t.inn = t.inn.map(|i| if i == id { nouveau } else { i });
        }
        let index = self.insert_and_normalize(at, vec![panel]);
        for r in &mut self.references {
            if references.contains(&r.id) && !r.plans.contains(&nouveau) {
                r.plans.push(nouveau);
            }
        }
        Some(self.panels.iter().position(|p| p.id == nouveau).unwrap_or(index))
    }

    /// Les plans annulés portent leurs réserves : `Fn` appliquée aux caméras, à chaque changement de feuille (`enlarge_stage`, `scale_stage`).
    pub(crate) fn map_annules(&mut self, mut f: impl FnMut(&mut Panel, Option<&mut Reserve>)) {
        for g in &mut self.annules.plans {
            let g = Arc::make_mut(g);
            f(&mut g.panel, g.reserve.as_mut());
            if let Some(v) = g.versions.as_mut() {
                for r in &mut v.autres {
                    f(&mut r.panel, None);
                }
            }
        }
    }

    /// Les plans des plans annulés et de leurs versions rangées : ils sont dans le projet sans être dans le film (`plans_ranges`).
    pub(crate) fn plans_des_annules(&self) -> impl Iterator<Item = &Panel> {
        self.annules.plans.iter().flat_map(|g| std::iter::once(&g.panel).chain(g.versions.iter().flat_map(|v| v.autres.iter().map(|r| &r.panel))))
    }

    /// Le contenu rogné des plans annulés.
    pub(crate) fn reserves_des_annules(&self) -> impl Iterator<Item = &Reserve> {
        self.annules.plans.iter().filter_map(|g| g.reserve.as_ref())
    }
}

/// La planche à l'écran, sans toucher à sa mise en page : un petit document où les plans annulés sont à leur place dans `panels` (sans calques : la
/// planche ne lit que les durées, les marques et les caméras), mis en page comme tous les autres (lignes, japonaise, par séquences) ; `board.rs`
/// en retire ensuite les cases annulées. Rend ce document et le rang d'origine de chacun de ses plans.
pub(crate) fn document_de_la_planche(doc: &Document) -> (Document, Vec<Rang>) {
    let ordre = doc.ordre_planche();
    let mut mini = Document::new(doc.width, doc.height, doc.fps);
    mini.frame = doc.frame;
    mini.sequences = doc.sequences.clone();
    mini.scenes = doc.scenes.clone();
    mini.panels = ordre
        .iter()
        .map(|r| {
            let p = match *r {
                Rang::Film(i) => &doc.panels[i],
                Rang::Annule(k) => &doc.annules.plans[k].panel,
            };
            Panel {
                id: p.id,
                scene: p.scene,
                length: p.frame_count(),
                layers: Vec::new(),
                action: String::new(),
                dialogue: String::new(),
                notes: String::new(),
                camera: p.camera.clone(),
                saccade: p.saccade,
                effet_camera: p.effet_camera,
                marks: p.marks.clone(),
                cues: Default::default(),
                color: None,
                espace: p.espace,
            }
        })
        .collect();
    for r in &ordre {
        if let Rang::Annule(k) = *r {
            let g = &doc.annules.plans[k];
            if !mini.sequences.iter().any(|s| s.id == g.sequence.id) {
                mini.sequences.push(g.sequence.clone());
            }
            if !mini.scenes.iter().any(|s| s.id == g.panel.scene) {
                let mut s = g.scene.clone();
                s.id = g.panel.scene;
                s.sequence = g.sequence.id;
                mini.scenes.push(s);
            }
        }
    }
    (mini, ordre)
}

// ----------------------------------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
pub(crate) struct NomFichier {
    id: u64,
    #[serde(default)]
    nom: String,
    #[serde(default)]
    numero: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PlanAnnuleFichier {
    /// L'entrée du manifeste (`project::snapshot_value`).
    pub entree: serde_json::Value,
    scene: NomFichier,
    sequence: NomFichier,
    #[serde(default = "un")]
    variante: u32,
    #[serde(default)]
    apres: Option<u64>,
    #[serde(default)]
    suivant: Option<u64>,
    #[serde(default)]
    pub versions: Option<crate::planvers::PlanFichier>,
    #[serde(default)]
    pub rognage: Option<serde_json::Value>,
    #[serde(default)]
    commentaires: Vec<Comment>,
    #[serde(default)]
    references: Vec<u64>,
    #[serde(default)]
    date: u64,
}

fn un() -> u32 {
    1
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    pub plans: Vec<PlanAnnuleFichier>,
    #[serde(default)]
    support: Option<u64>,
    #[serde(default)]
    transitions: Vec<Transition>,
}

/// Ce qui s'écrit : les plans lus et, telles quelles, les entrées abîmées.
#[derive(Serialize)]
struct FichierEcrit {
    format: String,
    version: u32,
    plans: Vec<serde_json::Value>,
    support: Option<u64>,
    transitions: Vec<Transition>,
}

/// Le contenu de `plans_annules.json` ; rien : aucun plan annulé, le fichier part.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.annules.rien_a_ecrire() {
        return Ok(None);
    }
    let mut plans = Vec::with_capacity(doc.annules.plans.len());
    for g in &doc.annules.plans {
        let plan = PlanAnnuleFichier {
            entree: crate::project::snapshot_value(&g.panel, &|c| c)?,
            scene: NomFichier { id: g.scene.id, nom: g.scene.name.clone(), numero: g.scene.number.clone() },
            sequence: NomFichier { id: g.sequence.id, nom: g.sequence.name.clone(), numero: g.sequence.number.clone() },
            variante: g.variante,
            apres: g.apres,
            suivant: g.suivant,
            versions: match &g.versions {
                Some(v) => Some(crate::planvers::plan_fichier(g.panel.id, v)?),
                None => None,
            },
            rognage: match g.reserve.as_ref().filter(|r| !r.is_empty()) {
                Some(r) => Some(crate::rognage::plan_value(&g.panel, r)?),
                None => None,
            },
            commentaires: g.commentaires.clone(),
            references: g.references.clone(),
            date: g.date,
        };
        plans.push(serde_json::to_value(&plan).map_err(|e| e.to_string())?);
    }
    plans.extend(doc.annules.illisibles.iter().cloned());
    serde_json::to_vec_pretty(&FichierEcrit { format: FORMAT.into(), version: VERSION, plans, support: doc.annules.support, transitions: doc.annules.transitions.clone() }).map(Some).map_err(|e| e.to_string())
}

pub(crate) fn parse(text: &str) -> Result<Fichier, String> {
    let f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de plans écartés".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    Ok(f)
}

/// Le fichier existe mais cette version ne sait pas le lire : l'enregistrement n'efface alors aucun dessin de `dessins/`.
pub(crate) fn illisible(dir: &Path) -> bool {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()).and_then(|t| parse(&t).map(|_| ()).map_err(|_| ())).is_err(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// L'ouverture : le fichier du dossier (absent : rien). Illisible : on le dit, le projet s'ouvre, le fichier reste là (`write_sidecar`).
pub(crate) fn lire(dir: &Path, warnings: &mut Vec<String>) -> Option<Fichier> {
    let path = dir.join(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()).and_then(|t| parse(&t)) {
            Ok(f) => Some(f),
            Err(e) => {
                warnings.push(format!("{FILE} : {e} ; les plans écartés ne sont pas chargés, le fichier reste là."));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{FILE} : {e}"));
            None
        }
    }
}

/// Met en place les plans annulés lus : `panels[i]` est le plan `i` du fichier (rien : son entrée est abîmée, elle est dans `illisibles`), `rangees[i]`
/// ses versions rangées, toutes, dans l'ordre du fichier, `rognages[i]` son contenu rogné lu. `missing` : dessins illisibles.
#[allow(clippy::too_many_arguments)]
pub(crate) fn installer(doc: &mut Document, f: Fichier, mut panels: Vec<Option<Panel>>, mut rangees: Vec<Vec<Panel>>, mut rognages: Vec<Option<crate::rognage::Loaded>>, illisibles: Vec<serde_json::Value>, missing: &BTreeSet<CelId>) {
    let mut plans = Vec::with_capacity(f.plans.len());
    // (Les numéros d'une entrée abîmée restent pris, autant qu'on les lit.)
    for v in &illisibles {
        for id in [v["entree"]["id"].as_u64(), v["scene"]["id"].as_u64(), v["sequence"]["id"].as_u64()].into_iter().flatten() {
            doc.reserve_id(id);
        }
    }
    for t in &f.transitions {
        doc.reserve_id(t.id);
    }
    for (i, pf) in f.plans.into_iter().enumerate() {
        let Some(mut panel) = panels.get_mut(i).and_then(Option::take) else { continue };
        crate::variantes::poser_scene(&mut panel, pf.scene.id);
        let reserve = rognages.get_mut(i).and_then(Option::take).and_then(|l| l.pour_plans(&[&panel], missing).into_iter().next().map(|(_, r)| r));
        let versions = pf.versions.map(|vf| {
            let lues = rangees.get_mut(i).map(std::mem::take).unwrap_or_default();
            let autres = vf
                .autres
                .iter()
                .zip(lues)
                .map(|(a, mut p)| {
                    crate::variantes::poser_scene(&mut p, pf.scene.id);
                    crate::planvers::Rangee { numero: a.numero, date: a.date, panel: p }
                })
                .collect();
            VersionsDuPlan { actuelle: vf.actuelle, date_actuelle: vf.date_actuelle, prochaine: vf.prochaine, autres }
        });
        let mut ids: Vec<u64> = vec![panel.id, pf.scene.id, pf.sequence.id];
        ids.extend(panel.marks.iter().map(|m| m.id));
        ids.extend(pf.commentaires.iter().map(|c| c.id));
        for v in versions.iter().flat_map(|v| &v.autres) {
            ids.push(v.panel.id);
            ids.extend(v.panel.marks.iter().map(|m| m.id));
        }
        if let Some(r) = &reserve {
            for piece in [&r.head, &r.tail] {
                ids.extend(piece.marks.iter().map(|m| m.id));
                ids.extend(piece.comments.iter().map(|c| c.id));
            }
        }
        for id in ids {
            doc.reserve_id(id);
        }
        let scene = Scene { id: pf.scene.id, name: pf.scene.nom, number: pf.scene.numero, sequence: pf.sequence.id };
        let sequence = Sequence { id: pf.sequence.id, name: pf.sequence.nom, number: pf.sequence.numero };
        let (id, _) = (panel.id, ());
        let commentaires: Vec<Comment> = pf.commentaires.into_iter().filter(|c| c.panel == id).collect();
        plans.push(Arc::new(PlanAnnule { panel, scene, sequence, variante: pf.variante.max(1), apres: pf.apres, suivant: pf.suivant, reserve, commentaires, versions, references: pf.references, date: pf.date }));
    }
    doc.annules = Annules { plans, support: f.support.filter(|s| doc.panels.iter().any(|p| p.id == *s)), transitions: f.transitions, illisibles };
    doc.annules_reposer_transitions();
}
