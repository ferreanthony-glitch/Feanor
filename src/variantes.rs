//! Variantes de séquence : essayer un autre découpage d'une SÉQUENCE (A plus serrée, B avec un plan de plus…) sans
//! perdre le premier (Anthony, 2026-10-21). Ce ne sont pas des versions du film entier (trop lourd), ni des
//! versions d'un plan (`planvers.rs`) : une variante est **un jeu de plans** pour la séquence.
//!
//! # Le modèle
//!
//! Une séquence a toujours **une variante choisie** : ce sont ses scènes et ses plans dans `Document::scenes` et
//! `Document::panels`, donc ce que montrent le montage, la lecture, les exports, la numérotation, l'export pour le
//! montage, la relecture et la fiche technique. Les **autres variantes** sont rangées (`VarianteRangee`) dans
//! `Document::variantes`, par numéro de séquence, avec **tout ce qui tient à leurs plans** : leurs scènes, leurs
//! plans (dessins, marques d'image, clés de caméra, textes), les versions de chaque plan (`planvers.rs`), le contenu
//! rogné, les commentaires, les transitions d'un plan à l'autre de la variante et les liens des références. Une
//! variante rangée est donc complète et indépendante : rien d'elle ne traîne dans les tables du film.
//!
//! Une séquence qui n'a jamais eu de variante n'a pas d'entrée : c'est la variante A.
//!
//! - **Nouvelle variante, en dupliquant** : le film ne bouge pas (mêmes plans, mêmes numéros : l'historique, la
//!   sélection, les notes de la planche accrochées à ses plans, les envois suivent) ; ce sont ses plans qui prennent
//!   le numéro suivant, et c'est une **copie profonde** (numéros neufs de plans, de scènes, de marques et de calques,
//!   dessins dupliqués — les tuiles restent partagées jusqu'à la première retouche) qui est rangée sous le numéro
//!   qu'ils portaient.
//! - **Nouvelle variante, vide** : le jeu de plans en cours est rangé tel quel ; la séquence repart d'une scène et d'un
//!   plan vide.
//! - **Passer à une variante** : le jeu de plans de la séquence et celui de la variante choisie s'échangent. Les
//!   transitions aux bords de la séquence (avec le plan d'avant et le plan d'après) restent sur la coupe : elles
//!   passent au nouveau premier ou dernier plan.
//! - **Supprimer** une variante (jamais la dernière) ; **renommer** (A, B, C par défaut ; un nom court en plus).
//!
//! Tout cela est dans `Structure` (`Arc` : la copier pour l'annulation ne recopie rien) : une opération = une étape
//! d'annulation nommée. Les dessins des variantes rangées sont de vrais dessins du projet (`dessins/`), comptés par
//! `Document::used_cels`.
//!
//! # Les envois
//!
//! Une copie profonde a des numéros de plans neufs : pour que « modifié depuis l'envoi » reste juste, chaque plan copié
//! se souvient du plan dont il vient (`VariantesDeSequence::origines`), et `Document::plan_dans_envoi` remonte cette
//! filiation jusqu'à un plan que l'envoi connaît. Un envoi note aussi la variante choisie de chaque séquence qui en a
//! plusieurs (`envois::VarianteEnvoyee`).
//!
//! # Fichier
//!
//! `variantes.json` à côté du manifeste, sans changer `project::VERSION` : pour chaque séquence, la variante choisie
//! (son numéro et son nom) et, pour chaque variante rangée, ses scènes, ses plans (l'entrée du manifeste,
//! `project::snapshot_value`), les versions de chaque plan, son contenu rogné, ses commentaires et ses transitions. À
//! l'ouverture, les plans passent par la lecture ordinaire du manifeste (une séquence cachée qu'on retire ensuite du
//! film, comme pour `plans_versions.json`) : leurs dessins se chargent avec les autres, avec les mêmes protections.
//! Un fichier illisible ou plus récent n'est jamais effacé (`write_sidecar`), et tant qu'il est là l'enregistrement ne
//! nettoie pas `dessins/`. Dans un projet partagé (`docs/COOPERATION.md`), c'est une annexe du montage : seul le
//! porteur du jeton l'écrit, et changer de variante demande ce jeton.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::commentaires::Comment;
use crate::document::{CelId, Document, Panel, Scene};
use crate::planvers::{Rangee, VersionsDuPlan};
use crate::rognage::Reserve;
use crate::transitions::Transition;

pub(crate) const FILE: &str = "variantes.json";
const FORMAT: &str = "shot-variantes-sequences";
const VERSION: u32 = 1;
/// Le plus grand nombre de variantes d'une séquence : une lettre chacune, de A à Z.
pub const MAX_VARIANTES: usize = 26;
/// Plus long nom de variante gardé.
pub const MAX_NOM: usize = 40;
/// Le plus de plans qu'une variante rangée peut avoir à la lecture (un fichier abîmé ne remplit pas la mémoire).
const MAX_PLANS: usize = 20_000;

/// Tout ce que possède un jeu de plans : ce qui s'échange quand on passe d'une variante à l'autre.
#[derive(Clone, Default)]
pub struct Contenu {
    pub scenes: Vec<Scene>,
    pub panels: Vec<Panel>,
    /// Les versions de chaque plan (`planvers.rs`), par numéro de plan.
    pub versions: BTreeMap<u64, VersionsDuPlan>,
    /// Le contenu rogné de chaque plan (`rognage.rs`).
    pub reserves: BTreeMap<u64, Reserve>,
    pub comments: Vec<Comment>,
    /// Les transitions entre deux plans de la variante.
    pub transitions: Vec<Transition>,
    /// Les références liées à des plans de la variante : (numéro de la référence, plans).
    pub references: Vec<(u64, Vec<u64>)>,
}

impl Contenu {
    pub fn ids_plans(&self) -> HashSet<u64> {
        self.panels.iter().map(|p| p.id).collect()
    }
}

/// Une variante rangée : le jeu de plans, sous son numéro.
#[derive(Clone)]
pub struct VarianteRangee {
    pub numero: u32,
    /// Nom court choisi (vide : la lettre seule).
    pub nom: String,
    /// Quand elle a été créée (secondes depuis 1970 ; 0 : inconnue).
    pub date: u64,
    pub contenu: Contenu,
}

/// Les variantes d'une séquence.
#[derive(Clone)]
pub struct VariantesDeSequence {
    /// Le numéro de la variante choisie (celle du film).
    pub actuelle: u32,
    pub nom_actuelle: String,
    pub date_actuelle: u64,
    /// Les autres, par numéro.
    pub autres: Vec<VarianteRangee>,
    /// Plan copié → plan dont il vient (voir l'en-tête du module, « Les envois »).
    pub origines: BTreeMap<u64, u64>,
}

impl VariantesDeSequence {
    pub fn nombre(&self) -> usize {
        1 + self.autres.len()
    }

    /// Tous les numéros, dans l'ordre.
    pub fn numeros(&self) -> Vec<u32> {
        let mut n: Vec<u32> = std::iter::once(self.actuelle).chain(self.autres.iter().map(|r| r.numero)).collect();
        n.sort_unstable();
        n
    }

    pub fn rangee(&self, numero: u32) -> Option<&VarianteRangee> {
        self.autres.iter().find(|r| r.numero == numero)
    }

    /// Le nom court de la variante `numero` (vide : aucun).
    pub fn nom(&self, numero: u32) -> &str {
        if numero == self.actuelle {
            &self.nom_actuelle
        } else {
            self.rangee(numero).map_or("", |r| r.nom.as_str())
        }
    }

    /// Le plus petit numéro libre (la lettre d'une variante supprimée est reprise).
    fn numero_libre(&self) -> u32 {
        (1..).find(|n| *n != self.actuelle && self.rangee(*n).is_none()).unwrap_or(1)
    }
}

/// Les variantes de tout le projet, partagées (`Arc`) : une étape d'annulation ne les recopie pas.
pub type Variantes = Arc<BTreeMap<u64, VariantesDeSequence>>;

/// « A », « B »… « Z » : la lettre d'une variante.
pub fn lettre(numero: u32) -> String {
    match numero {
        1..=26 => char::from(b'A' + (numero - 1) as u8).to_string(),
        n => format!("V{n}"),
    }
}

/// « B » ou « B · serré » : le nom d'une variante, partout (sélecteur, montage, comparaison, messages).
pub fn etiquette(numero: u32, nom: &str) -> String {
    let nom = nom.trim();
    if nom.is_empty() {
        lettre(numero)
    } else {
        format!("{} · {nom}", lettre(numero))
    }
}

/// Un nom de variante nettoyé : espaces retirés, une seule ligne, `MAX_NOM` lettres au plus.
pub fn nom_propre(texte: &str) -> String {
    texte.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NOM).collect()
}

impl Document {
    /// Les variantes de la séquence `sequence`, si elle en a plusieurs.
    pub fn variantes_de(&self, sequence: u64) -> Option<&VariantesDeSequence> {
        self.variantes.get(&sequence)
    }

    /// Le numéro de la variante choisie de la séquence (1 tant qu'elle n'a qu'une variante).
    pub fn numero_de_variante(&self, sequence: u64) -> u32 {
        self.variantes.get(&sequence).map_or(1, |v| v.actuelle)
    }

    pub fn nombre_de_variantes(&self, sequence: u64) -> usize {
        self.variantes.get(&sequence).map_or(1, VariantesDeSequence::nombre)
    }

    /// « B · serré » : le nom de la variante `numero` de la séquence.
    pub fn etiquette_de_variante(&self, sequence: u64, numero: u32) -> String {
        etiquette(numero, self.variantes.get(&sequence).map_or("", |v| v.nom(numero)))
    }

    fn variantes_mut(&mut self) -> &mut BTreeMap<u64, VariantesDeSequence> {
        Arc::make_mut(&mut self.variantes)
    }

    /// Les plans des variantes rangées, et les plans de leurs versions.
    pub fn plans_de_variantes(&self) -> impl Iterator<Item = &Panel> {
        self.variantes
            .values()
            .flat_map(|v| v.autres.iter())
            .flat_map(|r| r.contenu.panels.iter().chain(r.contenu.versions.values().flat_map(|v| v.autres.iter().map(|x| &x.panel))))
    }

    /// Les dessins des variantes rangées (plans, versions des plans, contenu rogné).
    pub fn cels_des_variantes(&self) -> BTreeSet<CelId> {
        let mut out: BTreeSet<CelId> = self.plans_de_variantes().flat_map(crate::empreinte::dessins_du_plan).collect();
        for r in self.variantes.values().flat_map(|v| v.autres.iter()) {
            for res in r.contenu.reserves.values() {
                out.extend(res.cels());
            }
        }
        out
    }

    /// Tous les numéros de plans (et de marques) que les variantes rangées possèdent, pour ne pas les redonner.
    pub(crate) fn ids_des_variantes(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        for r in self.variantes.values().flat_map(|v| v.autres.iter()) {
            ids.extend(r.contenu.scenes.iter().map(|s| s.id));
            ids.extend(r.contenu.comments.iter().map(|c| c.id));
            ids.extend(r.contenu.transitions.iter().map(|t| t.id));
        }
        for p in self.plans_de_variantes() {
            ids.push(p.id);
            ids.extend(p.marks.iter().map(|m| m.id));
        }
        ids
    }

    /// Les bornes [début, fin) des plans de la séquence dans le film.
    pub fn course_de_sequence(&self, sequence: u64) -> Option<(usize, usize)> {
        let ids = self.sequence_ids();
        let first = ids.iter().position(|&s| s == sequence)?;
        let mut end = first + 1;
        while end < ids.len() && ids[end] == sequence {
            end += 1;
        }
        Some((first, end))
    }

    /// Les numéros des plans de la séquence, dans l'ordre du film.
    pub fn plans_de_sequence(&self, sequence: u64) -> Vec<u64> {
        self.course_de_sequence(sequence).map_or_else(Vec::new, |(a, b)| self.panels[a..b].iter().map(|p| p.id).collect())
    }

    /// Le plan dont `plan` est la copie (le plus proche), s'il en vient d'un.
    pub fn origine_du_plan(&self, plan: u64) -> Option<u64> {
        self.variantes.values().find_map(|v| v.origines.get(&plan).copied())
    }

    /// Le plan, parmi `plan` et ses ancêtres de copie, que l'envoi connaît : c'est lui qu'on compare au plan d'aujourd'hui.
    pub fn plan_dans_envoi(&self, envoi: &crate::envois::Envoi, plan: u64) -> Option<u64> {
        let mut courant = plan;
        for _ in 0..16 {
            if envoi.plan(courant).is_some() {
                return Some(courant);
            }
            courant = self.origine_du_plan(courant)?;
        }
        None
    }

    /// Ce que note un envoi : la variante choisie de chaque séquence qui en a plusieurs.
    pub fn variantes_pour_envoi(&self) -> Vec<crate::envois::VarianteEnvoyee> {
        self.variantes
            .iter()
            .map(|(s, v)| crate::envois::VarianteEnvoyee { sequence: *s, numero: v.actuelle, nom: v.nom_actuelle.clone() })
            .collect()
    }

    /// Le plan de la variante rangée `numero` qui est au même rang que `rang` dans sa séquence (pour comparer).
    pub fn plan_de_variante_au_rang(&self, sequence: u64, numero: u32, rang: usize) -> Option<&Panel> {
        self.variantes.get(&sequence)?.rangee(numero)?.contenu.panels.get(rang)
    }

    // ------------------------------------------------------------------ échanger un jeu de plans

    /// Retire de la séquence `sequence` tout son jeu de plans : plans, scènes, et ce qui tient à eux dans les tables du
    /// film. Rend la place où il était, et son contenu.
    fn extraire(&mut self, sequence: u64) -> Option<(usize, Contenu)> {
        let (start, end) = self.course_de_sequence(sequence)?;
        let panels: Vec<Panel> = self.panels.drain(start..end).collect();
        let ids: HashSet<u64> = panels.iter().map(|p| p.id).collect();
        let mut scene_ids: Vec<u64> = Vec::new();
        for p in &panels {
            if !scene_ids.contains(&p.scene) {
                scene_ids.push(p.scene);
            }
        }
        let mut scenes = Vec::new();
        for id in &scene_ids {
            if let Some(k) = self.scenes.iter().position(|s| s.id == *id) {
                scenes.push(self.scenes.remove(k));
            }
        }
        let versions: BTreeMap<u64, VersionsDuPlan> = ids.iter().filter_map(|id| self.versions.remove(id).map(|v| (*id, v))).collect();
        let reserves: BTreeMap<u64, Reserve> = ids.iter().filter_map(|id| self.reserves.remove(id).map(|r| (*id, r))).collect();
        let (comments, gardes): (Vec<Comment>, Vec<Comment>) = std::mem::take(&mut self.comments).into_iter().partition(|c| ids.contains(&c.panel));
        self.comments = gardes;
        let (transitions, gardees): (Vec<Transition>, Vec<Transition>) =
            std::mem::take(&mut self.transitions).into_iter().partition(|t| t.out.is_some_and(|o| ids.contains(&o)) && t.inn.is_some_and(|i| ids.contains(&i)));
        self.transitions = gardees;
        let mut references = Vec::new();
        for r in &mut self.references {
            let liens: Vec<u64> = r.plans.iter().copied().filter(|p| ids.contains(p)).collect();
            if !liens.is_empty() {
                r.plans.retain(|p| !ids.contains(p));
                references.push((r.id, liens));
            }
        }
        Some((start, Contenu { scenes, panels, versions, reserves, comments, transitions, references }))
    }

    /// Remplace le jeu de plans de la séquence par `nouveau` et rend l'ancien. Les transitions aux bords de la séquence
    /// (avec le plan d'avant ou le plan d'après) restent sur la coupe : elles passent au nouveau premier ou dernier plan.
    fn echanger_contenu(&mut self, sequence: u64, nouveau: Contenu) -> Option<Contenu> {
        let (a, b) = self.course_de_sequence(sequence)?;
        let ids: HashSet<u64> = self.panels[a..b].iter().map(|p| p.id).collect();
        // Les transitions de bord : (numéro, côté sortant).
        let bords: Vec<(u64, bool)> = self
            .transitions
            .iter()
            .filter_map(|t| {
                let (o, i) = (t.out.is_some_and(|x| ids.contains(&x)), t.inn.is_some_and(|x| ids.contains(&x)));
                match (o, i) {
                    (true, false) => Some((t.id, true)),
                    (false, true) => Some((t.id, false)),
                    _ => None,
                }
            })
            .collect();
        let (premier, dernier) = (nouveau.panels.first().map(|p| p.id), nouveau.panels.last().map(|p| p.id));
        let (start, ancien) = self.extraire(sequence)?;
        self.inserer(start, nouveau);
        for (id, sortant) in bords {
            if let Some(t) = self.transitions.iter_mut().find(|t| t.id == id) {
                if sortant {
                    t.out = dernier;
                } else {
                    t.inn = premier;
                }
            }
        }
        Some(ancien)
    }

    /// Pose un jeu de plans dans le film, à la place `start`.
    fn inserer(&mut self, start: usize, mut c: Contenu) {
        // Numérotation verrouillée : un numéro de plan déjà pris ailleurs repart de rien, `normalize` en donne un autre.
        if self.numbering_locked {
            let pris: HashSet<String> = self.scenes.iter().filter(|s| !s.number.is_empty()).map(|s| s.number.clone()).collect();
            for s in &mut c.scenes {
                if pris.contains(&s.number) {
                    s.number.clear();
                }
            }
        }
        self.scenes.extend(c.scenes);
        let start = start.min(self.panels.len());
        self.panels.splice(start..start, c.panels);
        self.versions.extend(c.versions);
        self.reserves.extend(c.reserves);
        self.comments.extend(c.comments);
        self.transitions.extend(c.transitions);
        for (id, liens) in c.references {
            if let Some(r) = self.references.iter_mut().find(|r| r.id == id) {
                for p in liens {
                    if !r.plans.contains(&p) {
                        r.plans.push(p);
                    }
                }
            }
        }
    }

    /// Une copie profonde du jeu de plans de la séquence : numéros neufs de plans, de scènes, de marques et de calques,
    /// dessins dupliqués. Rend aussi les couples (copie, original) des plans. Les versions des plans, les commentaires,
    /// les transitions internes et les liens des références sont copiés ; le contenu rogné ne l'est pas.
    fn copier_contenu(&mut self, sequence: u64) -> Option<(Contenu, Vec<(u64, u64)>)> {
        let (start, end) = self.course_de_sequence(sequence)?;
        let sources: Vec<Panel> = self.panels[start..end].to_vec();
        let mut scenes: Vec<Scene> = Vec::new();
        let mut scene_map: Vec<(u64, u64)> = Vec::new();
        for p in &sources {
            if scene_map.iter().all(|(ancienne, _)| *ancienne != p.scene) {
                let neuve = self.alloc();
                let (name, number) = self.scene(p.scene).map_or((String::new(), String::new()), |s| (s.name.clone(), s.number.clone()));
                scenes.push(Scene { id: neuve, name, number, sequence });
                scene_map.push((p.scene, neuve));
            }
        }
        let scene_de = |ancienne: u64| scene_map.iter().find(|(a, _)| *a == ancienne).map_or(ancienne, |(_, n)| *n);
        let mut panels: Vec<Panel> = Vec::with_capacity(sources.len());
        let mut plan_map: HashMap<u64, u64> = HashMap::new();
        let mut couples: Vec<(u64, u64)> = Vec::new();
        for src in &sources {
            let mut copie = self.copie_de(src);
            copie.scene = scene_de(src.scene);
            plan_map.insert(src.id, copie.id);
            couples.push((copie.id, src.id));
            panels.push(copie);
        }
        // Les calques de toute la scène : la copie a les siens (dessins dupliqués, clé neuve).
        let mut a = 0;
        while a < panels.len() {
            let mut b = a + 1;
            while b < panels.len() && panels[b].scene == panels[a].scene {
                b += 1;
            }
            let scene = panels[a].scene;
            self.adopt_scene_layers(&mut panels[a..b], scene);
            a = b;
        }
        let mut versions = BTreeMap::new();
        for src in &sources {
            let Some(v) = self.versions.get(&src.id).cloned() else { continue };
            let host = plan_map[&src.id];
            let scene = scene_de(src.scene);
            let mut autres = Vec::new();
            for r in &v.autres {
                let mut p = self.copie_de(&r.panel);
                p.scene = scene;
                self.adopt_scene_layers(std::slice::from_mut(&mut p), scene);
                autres.push(Rangee { numero: r.numero, date: r.date, panel: p });
            }
            versions.insert(host, VersionsDuPlan { actuelle: v.actuelle, date_actuelle: v.date_actuelle, prochaine: v.prochaine, autres });
        }
        let mut comments: Vec<Comment> = self.comments.iter().filter(|c| plan_map.contains_key(&c.panel)).cloned().collect();
        for c in &mut comments {
            c.id = self.alloc();
            c.panel = plan_map[&c.panel];
        }
        let mut transitions: Vec<Transition> = self
            .transitions
            .iter()
            .filter(|t| t.out.is_some_and(|o| plan_map.contains_key(&o)) && t.inn.is_some_and(|i| plan_map.contains_key(&i)))
            .cloned()
            .collect();
        for t in &mut transitions {
            t.id = self.alloc();
            t.out = t.out.map(|o| plan_map[&o]);
            t.inn = t.inn.map(|i| plan_map[&i]);
        }
        let references: Vec<(u64, Vec<u64>)> = self
            .references
            .iter()
            .filter_map(|r| {
                let liens: Vec<u64> = r.plans.iter().filter_map(|p| plan_map.get(p).copied()).collect();
                (!liens.is_empty()).then_some((r.id, liens))
            })
            .collect();
        Some((Contenu { scenes, panels, versions, reserves: BTreeMap::new(), comments, transitions, references }, couples))
    }

    // ------------------------------------------------------------------ les opérations

    /// « Nouvelle variante » de la séquence `sequence` : le jeu de plans en cours est rangé sous son numéro, et la
    /// séquence prend le numéro suivant. `vide` : elle repart d'un plan vide ; sinon ses plans restent ceux du film
    /// et c'est une copie profonde qui est rangée (voir l'en-tête du module). Rend le numéro de la variante choisie.
    pub fn nouvelle_variante(&mut self, sequence: u64, vide: bool, now: u64) -> Option<u32> {
        self.course_de_sequence(sequence)?;
        let (actuelle, nom_actuelle, date_actuelle, libre) = match self.variantes.get(&sequence) {
            Some(v) if v.nombre() >= MAX_VARIANTES => return None,
            Some(v) => (v.actuelle, v.nom_actuelle.clone(), v.date_actuelle, v.numero_libre()),
            None => (1, String::new(), 0, 2),
        };
        let (rangee, couples) = if vide {
            let scene = self.new_scene(sequence);
            let panel = self.make_panel(scene);
            let neuve = self.scenes.pop().expect("scène créée juste avant");
            let ancien = self.echanger_contenu(sequence, Contenu { scenes: vec![neuve], panels: vec![panel], ..Contenu::default() })?;
            (ancien, Vec::new())
        } else {
            self.copier_contenu(sequence)?
        };
        let v = self.variantes_mut().entry(sequence).or_insert_with(|| VariantesDeSequence { actuelle: 1, nom_actuelle: String::new(), date_actuelle: 0, autres: Vec::new(), origines: BTreeMap::new() });
        v.autres.push(VarianteRangee { numero: actuelle, nom: nom_actuelle, date: date_actuelle, contenu: rangee });
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = libre;
        v.nom_actuelle = String::new();
        v.date_actuelle = now;
        v.origines.extend(couples);
        self.normalize();
        Some(libre)
    }

    /// Fait de la variante rangée `numero` la variante choisie de la séquence : les deux jeux de plans s'échangent.
    /// Rend faux si ce numéro n'est pas rangé.
    pub fn changer_de_variante(&mut self, sequence: u64, numero: u32) -> bool {
        let Some(v) = self.variantes.get(&sequence) else { return false };
        if numero == v.actuelle || !v.rangee(numero).is_some_and(|r| !r.contenu.panels.is_empty()) {
            return false;
        }
        if self.course_de_sequence(sequence).is_none() {
            return false;
        }
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        let k = v.autres.iter().position(|r| r.numero == numero).expect("variante lue juste avant");
        let entrante = v.autres.remove(k);
        let Some(ancien) = self.echanger_contenu(sequence, entrante.contenu) else { return false };
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        let sortante = VarianteRangee { numero: v.actuelle, nom: std::mem::take(&mut v.nom_actuelle), date: v.date_actuelle, contenu: ancien };
        v.autres.push(sortante);
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = numero;
        v.nom_actuelle = entrante.nom;
        v.date_actuelle = entrante.date;
        self.normalize();
        true
    }

    /// Supprime la variante `numero` (jamais la dernière). Si c'est la variante choisie, la séquence passe d'abord à la
    /// plus petite des autres. Rend le numéro de la variante choisie ensuite.
    pub fn supprimer_variante(&mut self, sequence: u64, numero: u32) -> Option<u32> {
        let v = self.variantes.get(&sequence)?;
        if v.nombre() < 2 || !v.numeros().contains(&numero) {
            return None;
        }
        if numero == v.actuelle {
            let autre = v.autres.iter().map(|r| r.numero).find(|n| *n != numero)?;
            if !self.changer_de_variante(sequence, autre) {
                return None;
            }
        }
        let v = self.variantes_mut().get_mut(&sequence)?;
        v.autres.retain(|r| r.numero != numero);
        let reste = v.actuelle;
        if v.autres.is_empty() {
            self.variantes_mut().remove(&sequence);
        }
        Some(reste)
    }

    /// Donne le nom court `nom` à la variante `numero`. Faux si la séquence n'a pas cette variante, ou si rien ne change.
    pub fn renommer_variante(&mut self, sequence: u64, numero: u32, nom: &str) -> bool {
        let nom = nom_propre(nom);
        let Some(v) = self.variantes.get(&sequence) else { return false };
        if v.nom(numero) == nom || !v.numeros().contains(&numero) {
            return false;
        }
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        if numero == v.actuelle {
            v.nom_actuelle = nom;
        } else if let Some(r) = v.autres.iter_mut().find(|r| r.numero == numero) {
            r.nom = nom;
        }
        true
    }

    /// Les variantes dont la séquence n'existe plus s'en vont avec elle (l'annulation les ramène, `Structure`).
    pub(crate) fn elaguer_variantes(&mut self) {
        if self.variantes.is_empty() {
            return;
        }
        let seqs: HashSet<u64> = self.sequences.iter().map(|s| s.id).collect();
        if self.variantes.keys().any(|s| !seqs.contains(s)) {
            self.variantes_mut().retain(|s, _| seqs.contains(s));
        }
    }
}

// ---------------------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
pub(crate) struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    pub sequences: Vec<SeqFichier>,
    /// Dessins bitmap des variantes qui ont quelque chose hors de la feuille (toile infinie).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hors: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SeqFichier {
    pub sequence: u64,
    pub actuelle: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nom_actuelle: String,
    #[serde(default)]
    pub date_actuelle: u64,
    /// Couples (plan copié, plan d'origine).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub origines: Vec<[u64; 2]>,
    #[serde(default)]
    pub autres: Vec<VarFichier>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct VarFichier {
    pub numero: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nom: String,
    #[serde(default)]
    pub date: u64,
    #[serde(default)]
    pub scenes: Vec<SceneFichier>,
    #[serde(default)]
    pub plans: Vec<PlanVarFichier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commentaires: Vec<Comment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<(u64, Vec<u64>)>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SceneFichier {
    pub id: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub nom: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub numero: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PlanVarFichier {
    pub scene: u64,
    /// L'entrée du plan dans le format du manifeste (`project::snapshot_value`), dessins cités par leur numéro.
    #[serde(default)]
    pub entree: serde_json::Value,
    /// Les autres versions du plan (`planvers.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<crate::planvers::PlanFichier>,
    /// Le contenu rogné du plan, au format de `rognages.json` pour ce plan seul.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rognage: Option<serde_json::Value>,
}

/// Où est un plan lu dans le fichier : (séquence, variante, plan) ou (séquence, variante, plan, version).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Cle {
    Plan(usize, usize, usize),
    Version(usize, usize, usize, usize),
}

/// Le contenu de `variantes.json` ; rien s'il n'y a aucune variante rangée (le fichier part, ou, s'il est illisible,
/// reste là). `hors` : ceux des dessins des variantes qui ont quelque chose hors de la feuille.
pub(crate) fn to_bytes(doc: &Document, hors: &BTreeSet<CelId>) -> Result<Option<Vec<u8>>, String> {
    let existe: HashSet<u64> = doc.panels.iter().map(|p| p.id).chain(doc.plans_de_variantes().map(|p| p.id)).collect();
    let mut sequences = Vec::new();
    for (sequence, v) in doc.variantes.iter() {
        if v.autres.is_empty() {
            continue;
        }
        let mut autres = Vec::new();
        for r in &v.autres {
            let c = &r.contenu;
            let mut plans = Vec::new();
            for p in &c.panels {
                plans.push(PlanVarFichier {
                    scene: p.scene,
                    entree: crate::project::snapshot_value(p, &|c| c)?,
                    versions: match c.versions.get(&p.id).filter(|v| !v.autres.is_empty()) {
                        Some(v) => Some(crate::planvers::plan_fichier(p.id, v)?),
                        None => None,
                    },
                    rognage: match c.reserves.get(&p.id).filter(|r| !r.is_empty()) {
                        Some(res) => Some(crate::rognage::plan_value(p, res)?),
                        None => None,
                    },
                });
            }
            autres.push(VarFichier {
                numero: r.numero,
                nom: r.nom.clone(),
                date: r.date,
                scenes: c.scenes.iter().map(|s| SceneFichier { id: s.id, nom: s.name.clone(), numero: s.number.clone() }).collect(),
                plans,
                commentaires: c.comments.clone(),
                transitions: c.transitions.clone(),
                references: c.references.clone(),
            });
        }
        sequences.push(SeqFichier {
            sequence: *sequence,
            actuelle: v.actuelle,
            nom_actuelle: v.nom_actuelle.clone(),
            date_actuelle: v.date_actuelle,
            origines: v.origines.iter().filter(|(p, _)| existe.contains(p)).map(|(p, o)| [*p, *o]).collect(),
            autres,
        });
    }
    if sequences.is_empty() {
        return Ok(None);
    }
    let f = Fichier { format: FORMAT.into(), version: VERSION, sequences, hors: hors.iter().copied().collect() };
    serde_json::to_vec_pretty(&f).map(Some).map_err(|e| e.to_string())
}

/// Les variantes lues dans le texte de `variantes.json` : numéros uniques et non nuls, la variante choisie n'est pas
/// aussi rangée, pas plus de `MAX_VARIANTES` par séquence, une variante sans plan est oubliée.
pub(crate) fn parse(text: &str) -> Result<Fichier, String> {
    let mut f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de variantes de séquences".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let mut vues = HashSet::new();
    f.sequences.retain(|s| vues.insert(s.sequence));
    for s in &mut f.sequences {
        s.actuelle = s.actuelle.max(1);
        s.nom_actuelle = nom_propre(&s.nom_actuelle);
        let mut numeros = HashSet::from([s.actuelle]);
        s.autres.retain(|v| v.numero >= 1 && !v.plans.is_empty() && numeros.insert(v.numero));
        s.autres.truncate(MAX_VARIANTES - 1);
        s.autres.sort_by_key(|v| v.numero);
        for v in &mut s.autres {
            v.nom = nom_propre(&v.nom);
            v.plans.truncate(MAX_PLANS);
        }
    }
    f.sequences.retain(|s| !s.autres.is_empty());
    Ok(f)
}

/// Le fichier du dossier `dir` : rien s'il n'existe pas, et un avertissement s'il est illisible (le projet s'ouvre sans
/// variantes rangées, le fichier reste en place).
pub(crate) fn lire(dir: &Path, warnings: &mut Vec<String>) -> Option<Fichier> {
    let path = dir.join(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()).and_then(|t| parse(&t)) {
            Ok(f) => Some(f),
            Err(e) => {
                warnings.push(format!("{FILE} : {e} ; les variantes de séquences ne sont pas chargées."));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; les variantes de séquences ne sont pas chargées.", path.display()));
            None
        }
    }
}

/// Le fichier existe mais cette version ne sait pas le lire : l'enregistrement n'efface alors aucun dessin de `dessins/`
/// (ceux des variantes qu'il décrit y sont encore).
pub(crate) fn illisible(dir: &Path) -> bool {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()).and_then(|t| parse(&t).map(|_| ()).map_err(|_| ())).is_err(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Le scène réelle d'un plan lu : ses calques de toute la scène la citent (la lecture les avait posés sur une scène cachée).
fn poser_scene(panel: &mut Panel, scene: u64) {
    panel.scene = scene;
    for l in &mut panel.layers {
        if let Some(w) = l.scene_wide.as_mut() {
            w.scene = scene;
        }
    }
}

/// Met en place les variantes lues : `panels` donne, pour chaque clé du fichier, le plan lu ; `rognages`, le contenu rogné lu
/// pour la clé d'un plan. Les variantes d'une séquence qui n'existe plus sont oubliées. `missing` : dessins illisibles ; si
/// une variante en cite, `Document::variantes_partielles` est levé.
pub(crate) fn installer(
    doc: &mut Document,
    lu: Fichier,
    mut panels: HashMap<Cle, Panel>,
    mut rognages: HashMap<Cle, crate::rognage::Loaded>,
    missing: &BTreeSet<CelId>,
    partielle: bool,
    warnings: &mut Vec<String>,
) {
    let mut out: BTreeMap<u64, VariantesDeSequence> = BTreeMap::new();
    for (si, s) in lu.sequences.into_iter().enumerate() {
        if !doc.sequences.iter().any(|q| q.id == s.sequence) {
            continue;
        }
        let mut autres = Vec::new();
        for (vi, v) in s.autres.into_iter().enumerate() {
            let scenes: Vec<Scene> = v.scenes.iter().map(|f| Scene { id: f.id, name: f.nom.clone(), number: f.numero.clone(), sequence: s.sequence }).collect();
            let mut c = Contenu::default();
            for (pi, pf) in v.plans.into_iter().enumerate() {
                let Some(mut panel) = panels.remove(&Cle::Plan(si, vi, pi)) else { continue };
                if !scenes.iter().any(|sc| sc.id == pf.scene) {
                    warnings.push(format!("variante {} d'une séquence : un plan sans scène connue, ignoré.", lettre(v.numero)));
                    continue;
                }
                poser_scene(&mut panel, pf.scene);
                if let Some(vf) = pf.versions {
                    let mut rangees = Vec::new();
                    for (ri, r) in vf.autres.iter().enumerate() {
                        if let Some(mut p) = panels.remove(&Cle::Version(si, vi, pi, ri)) {
                            poser_scene(&mut p, pf.scene);
                            rangees.push(Rangee { numero: r.numero, date: r.date, panel: p });
                        }
                    }
                    if !rangees.is_empty() {
                        c.versions.insert(panel.id, VersionsDuPlan { actuelle: vf.actuelle, date_actuelle: vf.date_actuelle, prochaine: vf.prochaine, autres: rangees });
                    }
                }
                if let Some(l) = rognages.remove(&Cle::Plan(si, vi, pi)) {
                    c.reserves.extend(l.pour_plans(&[&panel], missing));
                }
                c.panels.push(panel);
            }
            if c.panels.is_empty() {
                warnings.push(format!("variante {} d'une séquence : illisible, ignorée.", lettre(v.numero)));
                continue;
            }
            let ids = c.ids_plans();
            c.scenes = scenes.into_iter().filter(|sc| c.panels.iter().any(|p| p.scene == sc.id)).collect();
            c.comments = v.commentaires.into_iter().filter(|k| ids.contains(&k.panel)).collect();
            c.transitions = v.transitions.into_iter().filter(|t| t.out.is_some_and(|o| ids.contains(&o)) && t.inn.is_some_and(|i| ids.contains(&i))).collect();
            c.references = v.references.into_iter().map(|(r, liens)| (r, liens.into_iter().filter(|p| ids.contains(p)).collect::<Vec<_>>())).filter(|(_, l)| !l.is_empty()).collect();
            for sc in &c.scenes {
                doc.reserve_id(sc.id);
            }
            for k in &c.comments {
                doc.reserve_id(k.id);
            }
            for t in &c.transitions {
                doc.reserve_id(t.id);
            }
            for r in c.reserves.values() {
                for piece in [&r.head, &r.tail] {
                    piece.marks.iter().for_each(|m| doc.reserve_id(m.id));
                    piece.comments.iter().for_each(|k| doc.reserve_id(k.id));
                }
            }
            for p in c.panels.iter().chain(c.versions.values().flat_map(|v| v.autres.iter().map(|r| &r.panel))) {
                doc.reserve_id(p.id);
                p.marks.iter().for_each(|m| doc.reserve_id(m.id));
            }
            autres.push(VarianteRangee { numero: v.numero, nom: v.nom, date: v.date, contenu: c });
        }
        if autres.is_empty() {
            continue;
        }
        out.insert(
            s.sequence,
            VariantesDeSequence {
                actuelle: s.actuelle,
                nom_actuelle: s.nom_actuelle,
                date_actuelle: s.date_actuelle,
                autres,
                origines: s.origines.into_iter().map(|[p, o]| (p, o)).collect(),
            },
        );
    }
    if partielle {
        warnings.push("Des dessins de variantes de séquences manquent : le fichier des variantes ne sera pas réécrit tant qu'ils ne sont pas là.".into());
    }
    doc.variantes_partielles = partielle;
    doc.variantes = Arc::new(out);
}
