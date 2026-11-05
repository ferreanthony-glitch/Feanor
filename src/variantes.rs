//! Versions de séquence (le code les appelle encore « variantes » ; à l'écran : « version de séquence », A, B, C…) :
//! essayer un autre découpage d'une SÉQUENCE (A plus serrée, B avec un plan de plus…) sans perdre le premier
//! (Anthony, 2026-10-21), **en partageant les plans** (Anthony, 2026-10-28 : « toute nouvelle version d'un plan est
//! disponible dans toutes les versions de séquence ; le choix des plans et leurs durées varient d'une version de séquence
//! à l'autre ; on peut mettre à jour un plan dans une autre version de séquence »).
//!
//! # Le modèle (format 39)
//!
//! Un **plan** existe une seule fois dans le projet, avec **ses versions** (V1, V2…, `planvers.rs`) : une seule réserve de
//! versions par plan, quel que soit le nombre de versions de séquence qui le montrent. Une **version de séquence** ne
//! possède plus de plans : c'est une liste d'**emplacements** (`Emplacement`) — quel plan, dans quelle scène, **quelle
//! version** du plan elle montre, et quelle **durée** elle lui donne — plus ses scènes, ses transitions et les liens des
//! références. L'ordre, le choix des plans, leurs durées et la version montrée de chacun sont donc propres à chaque version
//! de séquence ; ce qui est dans un plan (ses dessins, sa caméra, ses textes, ses commentaires, son contenu rogné, ses
//! versions) est commun.
//!
//! - La **version choisie** de la séquence est celle du film : ses plans sont dans `Document::panels` (montage, lecture,
//!   exports, numérotation, relecture, fiche technique la suivent), chacun montrant sa version active.
//! - Les **autres versions de séquence** sont rangées (`VarianteRangee`) dans `Document::variantes`, par numéro de
//!   séquence : seulement leurs emplacements. Les plans que le film ne montre pas pour l'instant, mais qu'une version
//!   rangée montre, sont au **garage** (`VariantesDeSequence::garage` : le plan tel qu'il serait dans le film, avec ses
//!   versions, son contenu rogné et ses commentaires).
//! - Passer d'une version de séquence à une autre : on range les emplacements de l'une, et on fait entrer les plans de
//!   l'autre (ceux que le film montre déjà y restent, les autres sortent du garage), chacun sur la version et à la durée de
//!   son emplacement. Aucun dessin n'est recopié ni relu.
//! - **Retoucher un plan retouche le plan** : si une autre version de séquence en montre la même version, elle la voit
//!   retouchée. Pour continuer sur une autre forme sans toucher à la première : « Nouvelle version du plan » (partie 15) ;
//!   la nouvelle version est tout de suite proposée par toutes les versions de séquence, qui ne basculent pas d'elles-mêmes.
//!   « Mettre à jour dans les autres versions de séquence » fait montrer la version courante du plan aux versions cochées.
//! - Un plan retiré du film (supprimé, ou réuni à un voisin) qu'une autre version de séquence montre est **gardé au
//!   garage** (`Document::ranger_plan_partage`) : on ne supprime un plan que de la version de séquence où l'on est.
//! - Une version d'un plan que montre une autre version de séquence **ne se supprime pas** (`version_montree_ailleurs`).
//!
//! # Trois façons d'en créer une
//!
//! - **Garder les plans** (`Nouvelle::Partager`, par défaut) : la nouvelle version montre les mêmes plans, aux mêmes versions
//!   et durées ; l'ancienne est rangée telle quelle. Rien n'est recopié.
//! - **Copier les plans** (`Nouvelle::Copier`) : comme avant le format 39, une copie profonde (numéros neufs, dessins
//!   dupliqués : les tuiles restent partagées jusqu'à la première retouche) que rien ne relie à l'original. La copie est
//!   rangée au garage, sous l'ancienne version de séquence ; le film ne bouge pas.
//! - **Partir d'un plan vide** (`Nouvelle::Vide`) : le jeu en cours est rangé, la séquence repart d'une scène et d'un plan vide.
//!
//! # Anciens projets
//!
//! Un projet d'avant (format 38 ou moins) a des variantes qui sont des copies profondes complètes : elles s'ouvrent
//! **sans rien perdre ni fusionner** (`installer`) — chacun de leurs plans devient un plan du garage, avec ses versions, son
//! contenu rogné et ses commentaires, montré par un emplacement. Ces plans n'ont pas de lien avec ceux du film (le plan
//! copié et son original sont deux plans) : les nouvelles versions de séquence, elles, partagent leurs plans.
//!
//! # Les envois
//!
//! Un plan copié (`Nouvelle::Copier`) se souvient du plan dont il vient (`VariantesDeSequence::origines`), et
//! `Document::plan_dans_envoi` remonte cette filiation jusqu'à un plan que l'envoi connaît ; les plans partagés gardent leur
//! numéro. Un envoi note aussi la version choisie de chaque séquence qui en a plusieurs (`envois::VarianteEnvoyee`).
//!
//! # Fichier
//!
//! `variantes.json` à côté du manifeste (format 2 ; le format 1, celui des copies complètes, se lit toujours) : pour chaque
//! séquence, la version choisie (son numéro et son nom), les plans du garage (l'entrée du manifeste,
//! `project::snapshot_value`, leurs versions, leur contenu rogné, leurs commentaires) et, pour chaque version rangée, ses
//! scènes, ses emplacements, ses transitions. À l'ouverture, les plans du garage passent par la lecture ordinaire du
//! manifeste (une séquence cachée qu'on retire ensuite du film, comme pour `plans_versions.json`) : leurs dessins se
//! chargent avec les autres, avec les mêmes protections. Un fichier illisible ou plus récent n'est jamais effacé
//! (`write_sidecar`), et tant qu'il est là l'enregistrement ne nettoie pas `dessins/`. Dans un projet partagé
//! (`docs/COOPERATION.md`), c'est une annexe du montage : seul le porteur du jeton l'écrit, et changer de version de
//! séquence demande ce jeton.

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
/// 2 : versions de séquence qui partagent leurs plans (`garage`, `emplacements`) ; 1 : copies complètes (toujours lu).
const VERSION: u32 = 2;
/// Le plus grand nombre de versions de séquence d'une séquence : une lettre chacune, de A à Z.
pub const MAX_VARIANTES: usize = 26;
/// Plus long nom de version de séquence gardé.
pub const MAX_NOM: usize = 40;
/// Le plus de plans que le garage d'une séquence peut avoir à la lecture (un fichier abîmé ne remplit pas la mémoire).
const MAX_PLANS: usize = 20_000;

/// Comment on crée une version de séquence (voir l'en-tête du module).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nouvelle {
    /// Les mêmes plans, aux mêmes versions et aux mêmes durées : rien n'est recopié.
    Partager,
    /// Une copie profonde des plans, que rien ne relie à l'original.
    Copier,
    /// Un plan vide.
    Vide,
}

/// Où une version de séquence montre un plan : le plan, sa scène dans cette version, la version du plan qu'elle montre et
/// la durée qu'elle lui donne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Emplacement {
    /// Le numéro du plan.
    pub plan: u64,
    pub scene: u64,
    /// Le numéro de la version du plan montrée (1 tant que le plan n'a qu'une version).
    pub version: u32,
    /// La durée voulue, en images (la durée du plan quand la version de séquence a été rangée).
    pub duree: usize,
}

/// Ce que possède une version de séquence rangée : de quoi refaire son découpage.
#[derive(Clone, Default)]
pub struct Contenu {
    pub scenes: Vec<Scene>,
    /// Les plans, dans l'ordre.
    pub slots: Vec<Emplacement>,
    /// Les transitions entre deux plans de la version.
    pub transitions: Vec<Transition>,
    /// Les références liées à des plans de la version : (numéro de la référence, plans).
    pub references: Vec<(u64, Vec<u64>)>,
}

impl Contenu {
    pub fn ids_plans(&self) -> HashSet<u64> {
        self.slots.iter().map(|s| s.plan).collect()
    }
}

/// Un plan hors du film : tout ce qui est à lui — le plan (sa version active), ses autres versions, son contenu rogné et
/// ses commentaires. C'est aussi ce qui sort du film et y entre quand une version de séquence en remplace une autre.
#[derive(Clone)]
pub struct Parque {
    pub panel: Panel,
    pub versions: Option<VersionsDuPlan>,
    pub reserve: Option<Reserve>,
    pub comments: Vec<Comment>,
}

impl Parque {
    /// Le numéro de la version active.
    pub fn numero(&self) -> u32 {
        self.versions.as_ref().map_or(1, |v| v.actuelle)
    }

    /// Le plan de la version `version` (l'active ou une rangée).
    pub fn montre(&self, version: u32) -> Option<&Panel> {
        if self.numero() == version {
            return Some(&self.panel);
        }
        self.versions.as_ref()?.rangee(version).map(|r| &r.panel)
    }

    /// Le numéro des versions du plan.
    pub fn numeros(&self) -> Vec<u32> {
        self.versions.as_ref().map_or_else(|| vec![1], VersionsDuPlan::numeros)
    }

    /// Fait de la version rangée `version` la version active (les deux échangent leur contenu).
    fn changer_de_version(&mut self, version: u32) -> bool {
        let Some(v) = self.versions.as_mut() else { return false };
        let Some(k) = v.autres.iter().position(|r| r.numero == version) else { return false };
        let rangee = &mut v.autres[k];
        crate::planvers::echanger(&mut self.panel, &mut rangee.panel);
        std::mem::swap(&mut rangee.numero, &mut v.actuelle);
        std::mem::swap(&mut rangee.date, &mut v.date_actuelle);
        v.autres.sort_by_key(|r| r.numero);
        true
    }

    /// Prépare le plan pour un emplacement : sa version, sa durée (jamais en coupant des images : une durée plus courte que
    /// le contenu ne change rien) et sa scène.
    fn montrer(&mut self, e: &Emplacement) {
        if e.version != self.numero() {
            self.changer_de_version(e.version);
        }
        appliquer_duree(&mut self.panel, e.duree);
        poser_scene(&mut self.panel, e.scene);
    }
}

/// La durée `duree` pour un plan : seulement quand elle est au moins celle de son contenu (on ne coupe jamais d'images).
fn appliquer_duree(p: &mut Panel, duree: usize) {
    let contenu = p.layers.iter().map(|l| l.frames.len()).max().unwrap_or(0).max(1);
    if duree >= contenu {
        p.length = duree.max(1);
    }
}

/// La scène réelle d'un plan : ses calques de toute la scène la citent.
fn poser_scene(panel: &mut Panel, scene: u64) {
    panel.scene = scene;
    for l in &mut panel.layers {
        if let Some(w) = l.scene_wide.as_mut() {
            w.scene = scene;
        }
    }
}

/// Une version de séquence rangée : son découpage, sous son numéro.
#[derive(Clone)]
pub struct VarianteRangee {
    pub numero: u32,
    /// Nom court choisi (vide : la lettre seule).
    pub nom: String,
    /// Quand elle a été créée (secondes depuis 1970 ; 0 : inconnue).
    pub date: u64,
    pub contenu: Contenu,
}

/// Les versions de séquence d'une séquence.
#[derive(Clone)]
pub struct VariantesDeSequence {
    /// Le numéro de la version choisie (celle du film).
    pub actuelle: u32,
    pub nom_actuelle: String,
    pub date_actuelle: u64,
    /// Les autres, par numéro.
    pub autres: Vec<VarianteRangee>,
    /// Plan copié → plan dont il vient (voir l'en-tête du module, « Les envois »).
    pub origines: BTreeMap<u64, u64>,
    /// Les plans que le film ne montre pas pour l'instant et que des versions rangées montrent.
    pub garage: BTreeMap<u64, Parque>,
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

    /// Le nom court de la version `numero` (vide : aucun).
    pub fn nom(&self, numero: u32) -> &str {
        if numero == self.actuelle {
            &self.nom_actuelle
        } else {
            self.rangee(numero).map_or("", |r| r.nom.as_str())
        }
    }

    /// Le plus petit numéro libre (la lettre d'une version supprimée est reprise).
    fn numero_libre(&self) -> u32 {
        (1..).find(|n| *n != self.actuelle && self.rangee(*n).is_none()).unwrap_or(1)
    }

    /// Les plans que montre au moins une version rangée.
    fn plans_montres(&self) -> HashSet<u64> {
        self.autres.iter().flat_map(|r| r.contenu.slots.iter().map(|s| s.plan)).collect()
    }

    /// Retire du garage les plans que plus aucune version rangée ne montre.
    fn nettoyer_garage(&mut self) {
        let utiles = self.plans_montres();
        self.garage.retain(|id, _| utiles.contains(id));
    }
}

/// Les versions de séquence de tout le projet, partagées (`Arc`) : une étape d'annulation ne les recopie pas.
pub type Variantes = Arc<BTreeMap<u64, VariantesDeSequence>>;

/// « A », « B »… « Z » : la lettre d'une version de séquence.
pub fn lettre(numero: u32) -> String {
    match numero {
        1..=26 => char::from(b'A' + (numero - 1) as u8).to_string(),
        n => format!("V{n}"),
    }
}

/// « B » ou « B · serré » : le nom d'une version de séquence, partout (sélecteur, montage, comparaison, messages).
pub fn etiquette(numero: u32, nom: &str) -> String {
    let nom = nom.trim();
    if nom.is_empty() {
        lettre(numero)
    } else {
        format!("{} · {nom}", lettre(numero))
    }
}

/// Un nom de version de séquence nettoyé : espaces retirés, une seule ligne, `MAX_NOM` lettres au plus.
pub fn nom_propre(texte: &str) -> String {
    texte.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NOM).collect()
}

thread_local! {
    /// Pourquoi la dernière opération de montage a été refusée pour ne pas perdre de versions (vide : rien à dire). Le document
    /// n'a pas de canal vers la ligne d'état : l'interface le lit une fois par image d'écran (`App::dire_le_refus_des_versions`).
    static REFUS: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Note la raison d'un refus (la dernière gagne).
pub(crate) fn noter_refus(message: String) {
    REFUS.with(|r| *r.borrow_mut() = Some(message));
}

/// Reprend la raison du dernier refus, s'il y en a une.
pub(crate) fn prendre_refus() -> Option<String> {
    REFUS.with(|r| r.borrow_mut().take())
}

/// Un jeu de plans qui sort du film ou y entre : tout ce qui tient à lui dans les tables du film.
#[derive(Default)]
struct Jeu {
    scenes: Vec<Scene>,
    plans: Vec<Parque>,
    transitions: Vec<Transition>,
    references: Vec<(u64, Vec<u64>)>,
}

/// L'ancien contenu d'une variante : une copie profonde complète, plans compris (format 1 du fichier ; `Nouvelle::Copier`).
#[derive(Clone, Default)]
struct Ancien {
    scenes: Vec<Scene>,
    panels: Vec<Panel>,
    versions: BTreeMap<u64, VersionsDuPlan>,
    reserves: BTreeMap<u64, Reserve>,
    comments: Vec<Comment>,
    transitions: Vec<Transition>,
    references: Vec<(u64, Vec<u64>)>,
}

impl Ancien {
    /// Les plans passent au garage, un emplacement chacun ; le reste est le découpage.
    fn en_parques(self) -> (Contenu, Vec<Parque>) {
        let Ancien { scenes, panels, mut versions, mut reserves, comments, transitions, references } = self;
        let mut par_plan: HashMap<u64, Vec<Comment>> = HashMap::new();
        for c in comments {
            par_plan.entry(c.panel).or_default().push(c);
        }
        let mut slots = Vec::with_capacity(panels.len());
        let mut parques = Vec::with_capacity(panels.len());
        for panel in panels {
            let id = panel.id;
            let versions = versions.remove(&id).filter(|v| !v.autres.is_empty());
            slots.push(Emplacement { plan: id, scene: panel.scene, version: versions.as_ref().map_or(1, |v| v.actuelle), duree: panel.frame_count() });
            let reserve = reserves.remove(&id).filter(|r| !r.is_empty());
            parques.push(Parque { panel, versions, reserve, comments: par_plan.remove(&id).unwrap_or_default() });
        }
        (Contenu { scenes, slots, transitions, references }, parques)
    }
}

impl Document {
    /// Les versions de la séquence `sequence`, si elle en a plusieurs.
    pub fn variantes_de(&self, sequence: u64) -> Option<&VariantesDeSequence> {
        self.variantes.get(&sequence)
    }

    /// Le numéro de la version de séquence choisie (1 tant qu'elle n'en a qu'une).
    pub fn numero_de_variante(&self, sequence: u64) -> u32 {
        self.variantes.get(&sequence).map_or(1, |v| v.actuelle)
    }

    pub fn nombre_de_variantes(&self, sequence: u64) -> usize {
        self.variantes.get(&sequence).map_or(1, VariantesDeSequence::nombre)
    }

    /// « B · serré » : le nom de la version `numero` de la séquence.
    pub fn etiquette_de_variante(&self, sequence: u64, numero: u32) -> String {
        etiquette(numero, self.variantes.get(&sequence).map_or("", |v| v.nom(numero)))
    }

    fn variantes_mut(&mut self) -> &mut BTreeMap<u64, VariantesDeSequence> {
        Arc::make_mut(&mut self.variantes)
    }

    /// Les plans du garage, et les plans de leurs versions : tout ce que les versions de séquence gardent hors du film.
    pub fn plans_de_variantes(&self) -> impl Iterator<Item = &Panel> {
        self.variantes.values().flat_map(|v| v.garage.values()).flat_map(|p| std::iter::once(&p.panel).chain(p.versions.iter().flat_map(|v| v.autres.iter().map(|x| &x.panel))))
    }

    /// Le contenu rogné des plans du garage.
    pub(crate) fn reserves_de_variantes(&self) -> impl Iterator<Item = &Reserve> {
        self.variantes.values().flat_map(|v| v.garage.values()).filter_map(|p| p.reserve.as_ref())
    }

    /// Les dessins du garage (plans, versions des plans, contenu rogné).
    pub fn cels_des_variantes(&self) -> BTreeSet<CelId> {
        let mut out: BTreeSet<CelId> = self.plans_de_variantes().flat_map(crate::empreinte::dessins_du_plan).collect();
        for res in self.reserves_de_variantes() {
            out.extend(res.cels());
        }
        out
    }

    /// Tous les numéros (de plans, de marques, de scènes, de commentaires, de transitions) que les versions rangées possèdent,
    /// pour ne pas les redonner.
    pub(crate) fn ids_des_variantes(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        for r in self.variantes.values().flat_map(|v| v.autres.iter()) {
            ids.extend(r.contenu.scenes.iter().map(|s| s.id));
            ids.extend(r.contenu.transitions.iter().map(|t| t.id));
            ids.extend(r.contenu.slots.iter().map(|s| s.plan));
        }
        for g in self.variantes.values().flat_map(|v| v.garage.values()) {
            ids.extend(g.comments.iter().map(|c| c.id));
            if let Some(r) = &g.reserve {
                for piece in [&r.head, &r.tail] {
                    ids.extend(piece.marks.iter().map(|m| m.id));
                    ids.extend(piece.comments.iter().map(|c| c.id));
                }
            }
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

    /// Ce que note un envoi : la version choisie de chaque séquence qui en a plusieurs.
    pub fn variantes_pour_envoi(&self) -> Vec<crate::envois::VarianteEnvoyee> {
        self.variantes
            .iter()
            .map(|(s, v)| crate::envois::VarianteEnvoyee { sequence: *s, numero: v.actuelle, nom: v.nom_actuelle.clone() })
            .collect()
    }

    /// Le plan `plan` sous sa version `version`, où qu'il soit : dans le film (la version active, ou une version rangée) ou
    /// au garage.
    pub fn plan_montre(&self, plan: u64, version: u32) -> Option<&Panel> {
        if let Some(p) = self.panels.iter().find(|p| p.id == plan) {
            if self.numero_de_version(plan) == version {
                return Some(p);
            }
            return self.versions.get(&plan)?.rangee(version).map(|r| &r.panel);
        }
        self.variantes.values().find_map(|v| v.garage.get(&plan)).and_then(|g| g.montre(version))
    }

    /// Les numéros des versions du plan `plan`, où qu'il soit (rien : le plan n'existe pas).
    pub fn versions_connues(&self, plan: u64) -> Vec<u32> {
        if self.panels.iter().any(|p| p.id == plan) {
            return self.versions.get(&plan).map_or_else(|| vec![1], VersionsDuPlan::numeros);
        }
        self.variantes.values().find_map(|v| v.garage.get(&plan)).map_or_else(Vec::new, Parque::numeros)
    }

    /// Le plan de la version rangée `numero` qui est au même rang que `rang` dans sa séquence (pour comparer), sous la version
    /// que cette version de séquence montre.
    pub fn plan_de_variante_au_rang(&self, sequence: u64, numero: u32, rang: usize) -> Option<&Panel> {
        let e = self.variantes.get(&sequence)?.rangee(numero)?.contenu.slots.get(rang)?;
        self.plan_montre(e.plan, e.version)
    }

    /// Les emplacements de la séquence `sequence` tels que le film la montre : c'est ce qu'on range quand on la quitte.
    pub(crate) fn emplacements_du_film(&self, sequence: u64) -> Vec<Emplacement> {
        let Some((a, b)) = self.course_de_sequence(sequence) else { return Vec::new() };
        self.panels[a..b].iter().map(|p| Emplacement { plan: p.id, scene: p.scene, version: self.numero_de_version(p.id), duree: p.frame_count() }).collect()
    }

    // ------------------------------------------------------------------ sortir un jeu de plans du film, le poser

    /// Retire du film tout le jeu de plans de la séquence `sequence` : plans, scènes, et ce qui tient à eux dans les tables du
    /// film. Rend la place où il était, et son contenu.
    fn sortir(&mut self, sequence: u64) -> Option<(usize, Jeu)> {
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
        let (mes_commentaires, gardes): (Vec<Comment>, Vec<Comment>) = std::mem::take(&mut self.comments).into_iter().partition(|c| ids.contains(&c.panel));
        self.comments = gardes;
        let mut par_plan: HashMap<u64, Vec<Comment>> = HashMap::new();
        for c in mes_commentaires {
            par_plan.entry(c.panel).or_default().push(c);
        }
        let plans: Vec<Parque> = panels
            .into_iter()
            .map(|panel| {
                let id = panel.id;
                Parque { versions: self.versions.remove(&id), reserve: self.reserves.remove(&id), comments: par_plan.remove(&id).unwrap_or_default(), panel }
            })
            .collect();
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
        Some((start, Jeu { scenes, plans, transitions, references }))
    }

    /// Pose un jeu de plans dans le film, à la place `start`.
    fn poser(&mut self, start: usize, mut c: Jeu) {
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
        let mut panels = Vec::with_capacity(c.plans.len());
        for p in c.plans {
            let id = p.panel.id;
            // (Une entrée sans autre version garde le numéro de la version active : une autre version de séquence le montre.)
            if let Some(v) = p.versions {
                self.versions.insert(id, v);
            }
            if let Some(r) = p.reserve.filter(|r| !r.is_empty()) {
                self.reserves.insert(id, r);
            }
            self.comments.extend(p.comments);
            panels.push(p.panel);
        }
        self.panels.splice(start..start, panels);
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

    /// Les transitions aux bords de la séquence (avec le plan d'avant ou le plan d'après) : (numéro, côté sortant).
    fn bords_de(&self, ids: &HashSet<u64>) -> Vec<(u64, bool)> {
        self.transitions
            .iter()
            .filter_map(|t| {
                let (o, i) = (t.out.is_some_and(|x| ids.contains(&x)), t.inn.is_some_and(|x| ids.contains(&x)));
                match (o, i) {
                    (true, false) => Some((t.id, true)),
                    (false, true) => Some((t.id, false)),
                    _ => None,
                }
            })
            .collect()
    }

    /// Les transitions de bord restent sur la coupe : elles passent au nouveau premier ou dernier plan.
    fn refixer_bords(&mut self, bords: Vec<(u64, bool)>, premier: Option<u64>, dernier: Option<u64>) {
        for (id, sortant) in bords {
            if let Some(t) = self.transitions.iter_mut().find(|t| t.id == id) {
                if sortant {
                    t.out = dernier;
                } else {
                    t.inn = premier;
                }
            }
        }
    }

    /// Les clones des scènes, des transitions internes et des liens de références de la séquence `sequence` (le film ne bouge pas).
    fn copie_du_decoupage(&self, sequence: u64) -> Option<(Vec<Scene>, Vec<Transition>, Vec<(u64, Vec<u64>)>)> {
        let (a, b) = self.course_de_sequence(sequence)?;
        let ids: HashSet<u64> = self.panels[a..b].iter().map(|p| p.id).collect();
        let mut scene_ids: Vec<u64> = Vec::new();
        for p in &self.panels[a..b] {
            if !scene_ids.contains(&p.scene) {
                scene_ids.push(p.scene);
            }
        }
        let scenes = scene_ids.iter().filter_map(|id| self.scene(*id).cloned()).collect();
        let transitions = self.transitions.iter().filter(|t| t.out.is_some_and(|o| ids.contains(&o)) && t.inn.is_some_and(|i| ids.contains(&i))).cloned().collect();
        let references = self
            .references
            .iter()
            .filter_map(|r| {
                let liens: Vec<u64> = r.plans.iter().copied().filter(|p| ids.contains(p)).collect();
                (!liens.is_empty()).then_some((r.id, liens))
            })
            .collect();
        Some((scenes, transitions, references))
    }

    /// Une copie profonde du jeu de plans de la séquence : numéros neufs de plans, de scènes, de marques et de calques,
    /// dessins dupliqués. Rend aussi les couples (copie, original) des plans. Les versions des plans, les commentaires,
    /// les transitions internes et les liens des références sont copiés ; le contenu rogné ne l'est pas.
    fn copier_contenu(&mut self, sequence: u64) -> Option<(Ancien, Vec<(u64, u64)>)> {
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
        Some((Ancien { scenes, panels, versions, reserves: BTreeMap::new(), comments, transitions, references }, couples))
    }

    // ------------------------------------------------------------------ les opérations

    /// « Nouvelle version de séquence » de la séquence `sequence` : le découpage en cours est rangé sous son numéro, et la
    /// séquence prend le numéro suivant (voir l'en-tête du module pour les trois façons). Rend le numéro de la version choisie.
    pub fn nouvelle_variante(&mut self, sequence: u64, mode: Nouvelle, now: u64) -> Option<u32> {
        self.course_de_sequence(sequence)?;
        let (actuelle, nom_actuelle, date_actuelle, libre) = match self.variantes.get(&sequence) {
            Some(v) if v.nombre() >= MAX_VARIANTES => return None,
            Some(v) => (v.actuelle, v.nom_actuelle.clone(), v.date_actuelle, v.numero_libre()),
            None => (1, String::new(), 0, 2),
        };
        let slots = self.emplacements_du_film(sequence);
        let (contenu, parques, couples) = match mode {
            Nouvelle::Partager => {
                let (scenes, transitions, references) = self.copie_du_decoupage(sequence)?;
                (Contenu { scenes, slots, transitions, references }, Vec::new(), Vec::new())
            }
            Nouvelle::Copier => {
                let (ancien, couples) = self.copier_contenu(sequence)?;
                let (contenu, parques) = ancien.en_parques();
                (contenu, parques, couples)
            }
            Nouvelle::Vide => {
                let scene = self.new_scene(sequence);
                let panel = self.make_panel(scene);
                let neuve = self.scenes.pop().expect("scène créée juste avant");
                let ids: HashSet<u64> = self.plans_de_sequence(sequence).into_iter().collect();
                let bords = self.bords_de(&ids);
                let (start, jeu) = self.sortir(sequence)?;
                let (premier, dernier) = (Some(panel.id), Some(panel.id));
                self.poser(start, Jeu { scenes: vec![neuve], plans: vec![Parque { panel, versions: None, reserve: None, comments: Vec::new() }], ..Jeu::default() });
                self.refixer_bords(bords, premier, dernier);
                (Contenu { scenes: jeu.scenes, slots, transitions: jeu.transitions, references: jeu.references }, jeu.plans, Vec::new())
            }
        };
        let v = self.variantes_mut().entry(sequence).or_insert_with(|| VariantesDeSequence {
            actuelle: 1,
            nom_actuelle: String::new(),
            date_actuelle: 0,
            autres: Vec::new(),
            origines: BTreeMap::new(),
            garage: BTreeMap::new(),
        });
        v.autres.push(VarianteRangee { numero: actuelle, nom: nom_actuelle, date: date_actuelle, contenu });
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = libre;
        v.nom_actuelle = String::new();
        v.date_actuelle = now;
        v.origines.extend(couples);
        for p in parques {
            v.garage.insert(p.panel.id, p);
        }
        self.normalize();
        Some(libre)
    }

    /// Fait de la version rangée `numero` la version choisie de la séquence : on range les emplacements de l'ancienne et on
    /// fait entrer les plans de l'autre, chacun sur sa version et à sa durée. Rend faux si ce numéro n'est pas rangé.
    pub fn changer_de_variante(&mut self, sequence: u64, numero: u32) -> bool {
        let Some(v) = self.variantes.get(&sequence) else { return false };
        if numero == v.actuelle {
            return false;
        }
        let Some(entrante) = v.rangee(numero).filter(|r| !r.contenu.slots.is_empty()) else { return false };
        let Some((a, b)) = self.course_de_sequence(sequence) else { return false };
        let dans_le_film: HashSet<u64> = self.panels[a..b].iter().map(|p| p.id).collect();
        // (Un plan que le film a déplacé dans une autre séquence existe encore : `accorder_au_film` lui en fait une copie.)
        if !entrante.contenu.slots.iter().any(|s| dans_le_film.contains(&s.plan) || v.garage.contains_key(&s.plan) || self.panels.iter().any(|p| p.id == s.plan)) {
            return false;
        }
        self.accorder_au_film(sequence, numero);
        let slots_sortants = self.emplacements_du_film(sequence);
        let bords = self.bords_de(&dans_le_film);
        let Some((start, mut sorti)) = self.sortir(sequence) else { return false };
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        let k = v.autres.iter().position(|r| r.numero == numero).expect("version lue juste avant");
        let entrante = v.autres.remove(k);
        // Tous les plans à portée : ceux qui sortent du film et ceux du garage ; les plans de la version entrante en sont pris.
        let mut pool: BTreeMap<u64, Parque> = std::mem::take(&mut v.garage);
        for p in std::mem::take(&mut sorti.plans) {
            pool.insert(p.panel.id, p);
        }
        let mut plans = Vec::with_capacity(entrante.contenu.slots.len());
        for e in &entrante.contenu.slots {
            if let Some(mut p) = pool.remove(&e.plan) {
                p.montrer(e);
                plans.push(p);
            }
        }
        v.garage = pool;
        let sortante = VarianteRangee {
            numero: v.actuelle,
            nom: std::mem::take(&mut v.nom_actuelle),
            date: v.date_actuelle,
            contenu: Contenu { scenes: sorti.scenes, slots: slots_sortants, transitions: sorti.transitions, references: sorti.references },
        };
        v.autres.push(sortante);
        v.autres.sort_by_key(|r| r.numero);
        v.actuelle = numero;
        v.nom_actuelle = entrante.nom;
        v.date_actuelle = entrante.date;
        let (premier, dernier) = (plans.first().map(|p| p.panel.id), plans.last().map(|p| p.panel.id));
        let c = entrante.contenu;
        self.poser(start, Jeu { scenes: c.scenes, plans, transitions: c.transitions, references: c.references });
        self.refixer_bords(bords, premier, dernier);
        self.normalize();
        true
    }

    /// Une copie du plan du film `index` pour le garage : numéros neufs, dessins dupliqués (les tuiles restent partagées jusqu'à la
    /// première retouche), mêmes versions sous les mêmes numéros, commentaires copiés ; le contenu rogné ne l'est pas (comme pour
    /// `Nouvelle::Copier`). `scene` : la scène où une version de séquence le montre.
    fn parque_de_copie(&mut self, index: usize, scene: u64) -> Parque {
        let plan = self.panels[index].id;
        let source = self.panels[index].clone();
        let mut panel = self.copie_de(&source);
        self.adopt_scene_layers(std::slice::from_mut(&mut panel), scene);
        panel.scene = scene;
        let versions = match self.versions.get(&plan).cloned() {
            Some(v) => {
                let mut autres = Vec::with_capacity(v.autres.len());
                for r in &v.autres {
                    let mut p = self.copie_de(&r.panel);
                    self.adopt_scene_layers(std::slice::from_mut(&mut p), scene);
                    p.scene = scene;
                    autres.push(Rangee { numero: r.numero, date: r.date, panel: p });
                }
                Some(VersionsDuPlan { actuelle: v.actuelle, date_actuelle: v.date_actuelle, prochaine: v.prochaine, autres })
            }
            None => None,
        };
        let mut comments: Vec<Comment> = self.comments.iter().filter(|c| c.panel == plan).cloned().collect();
        for c in &mut comments {
            c.id = self.alloc();
            c.panel = panel.id;
        }
        Parque { panel, versions, reserve: None, comments }
    }

    /// Avant que la version rangée `numero` ne devienne celle du film : ce qu'elle cite doit exister à l'endroit où elle va entrer.
    /// - Un plan qu'elle montre et que le film a déplacé dans une **autre séquence** (`move_plans`, « Nouvelle séquence avec les
    ///   plans choisis »…) reste là où il est : la version en garde **une copie** au garage (même contenu, mêmes versions du plan),
    ///   sous un numéro neuf que toutes les versions rangées de la séquence se mettent à citer. Rien n'est enlevé de la version, rien
    ///   n'est pris à l'autre séquence.
    /// - Une scène qu'elle cite et que le film utilise ailleurs (la scène a changé de séquence avec ses plans) reçoit un numéro neuf :
    ///   deux scènes de même numéro dans le film casseraient l'ordre des plans.
    /// Sans effet quand tout est à sa place (le cas ordinaire).
    fn accorder_au_film(&mut self, sequence: u64, numero: u32) {
        let Some((a, b)) = self.course_de_sequence(sequence) else { return };
        let Some(v) = self.variantes.get(&sequence) else { return };
        let Some(entrante) = v.rangee(numero) else { return };
        let film: HashSet<u64> = self.panels[a..b].iter().map(|p| p.id).collect();
        let scenes_du_film: HashSet<u64> = self.panels[a..b].iter().map(|p| p.scene).collect();
        let scenes_prises: HashSet<u64> = self.scenes.iter().map(|s| s.id).collect();
        let conflits: Vec<u64> = entrante.contenu.scenes.iter().map(|s| s.id).filter(|id| !scenes_du_film.contains(id) && scenes_prises.contains(id)).collect();
        let mut plans: Vec<(u64, u64)> = Vec::new();
        for e in &entrante.contenu.slots {
            if !film.contains(&e.plan) && !v.garage.contains_key(&e.plan) && self.panels.iter().any(|p| p.id == e.plan) && plans.iter().all(|(p, _)| *p != e.plan) {
                plans.push((e.plan, e.scene));
            }
        }
        if conflits.is_empty() && plans.is_empty() {
            return;
        }
        let scenes_neuves: HashMap<u64, u64> = conflits.into_iter().map(|id| (id, self.alloc())).collect();
        let mut copies: Vec<(u64, Parque)> = Vec::new();
        for (plan, scene) in plans {
            let scene = scenes_neuves.get(&scene).copied().unwrap_or(scene);
            if let Some(i) = self.panels.iter().position(|p| p.id == plan) {
                let parque = self.parque_de_copie(i, scene);
                copies.push((plan, parque));
            }
        }
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        if let Some(r) = v.autres.iter_mut().find(|r| r.numero == numero) {
            for s in &mut r.contenu.scenes {
                if let Some(n) = scenes_neuves.get(&s.id) {
                    s.id = *n;
                }
            }
            for e in &mut r.contenu.slots {
                if let Some(n) = scenes_neuves.get(&e.scene) {
                    e.scene = *n;
                }
            }
        }
        let neuf_de: HashMap<u64, u64> = copies.iter().map(|(ancien, p)| (*ancien, p.panel.id)).collect();
        for r in &mut v.autres {
            let c = &mut r.contenu;
            for e in &mut c.slots {
                if let Some(n) = neuf_de.get(&e.plan) {
                    e.plan = *n;
                }
            }
            for t in &mut c.transitions {
                t.out = t.out.map(|o| neuf_de.get(&o).copied().unwrap_or(o));
                t.inn = t.inn.map(|i| neuf_de.get(&i).copied().unwrap_or(i));
            }
            for (_, liens) in &mut c.references {
                for l in liens.iter_mut() {
                    if let Some(n) = neuf_de.get(l) {
                        *l = *n;
                    }
                }
            }
        }
        for (ancien, p) in copies {
            v.origines.insert(p.panel.id, ancien);
            v.garage.insert(p.panel.id, p);
        }
    }

    /// Les séquences qui ont d'autres versions de séquence et dont **tous** les plans du film `partent` : leur retirer ce dernier
    /// plan ferait disparaître la séquence, et avec elle ses versions rangées et son garage (`elaguer_variantes`).
    pub(crate) fn sequences_videes_avec_versions(&self, partent: impl Fn(u64) -> bool) -> Vec<u64> {
        let mut out = Vec::new();
        for (sequence, v) in self.variantes.iter() {
            if v.autres.is_empty() {
                continue;
            }
            if let Some((a, b)) = self.course_de_sequence(*sequence) {
                if self.panels[a..b].iter().all(|p| partent(p.id)) {
                    out.push(*sequence);
                }
            }
        }
        out
    }

    /// Refuse (et dit pourquoi, `noter_refus`) une opération qui retirerait du film tous les plans d'une séquence qui a d'autres
    /// versions de séquence. Vrai s'il faut refuser. Rien à calculer tant que le projet n'a aucune version de séquence.
    pub(crate) fn refuser_si_sequence_videe(&self, partent: impl Fn(u64) -> bool) -> bool {
        if self.variantes.is_empty() {
            return false;
        }
        let videes = self.sequences_videes_avec_versions(partent);
        let Some(&sequence) = videes.first() else { return false };
        let noms: Vec<String> = self.variantes[&sequence].autres.iter().map(|r| self.etiquette_de_variante(sequence, r.numero)).collect();
        noter_refus(format!(
            "La séquence {} a d'autres versions de séquence ({}) : lui retirer son dernier plan les ferait disparaître. Supprime-les d'abord (Versions de la séquence), ou garde un plan.",
            self.sequence_label(sequence),
            noms.join(", ")
        ));
        true
    }

    /// Refuse (et dit pourquoi) de couper ou de réunir un plan qui a plusieurs versions : l'opération ne toucherait que la version
    /// affichée ; les autres garderaient le plan entier (une version rangée de plus qu'un plan, ou sous un numéro de plan qui n'existe
    /// plus). Vrai s'il faut refuser.
    pub(crate) fn refuser_si_versions_de_plan(&self, plans: &[u64], geste: &str) -> bool {
        let Some(&plan) = plans.iter().find(|p| self.nombre_de_versions(**p) > 1) else { return false };
        let noms: Vec<String> = self.versions.get(&plan).map(|v| v.numeros().into_iter().map(crate::planvers::etiquette).collect()).unwrap_or_default();
        noter_refus(format!(
            "Ce plan a plusieurs versions ({}) : {geste} ne toucherait que la version affichée, les autres garderaient le plan d'avant. Supprime les versions dont tu n'as plus besoin, puis recommence.",
            noms.join(", ")
        ));
        true
    }

    /// Supprime la version de séquence `numero` (jamais la dernière). Si c'est la version choisie, la séquence passe d'abord à
    /// la plus petite des autres. Les plans que plus aucune version ne montre sont supprimés avec elle ; les autres restent.
    /// Rend le numéro de la version choisie ensuite.
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
        v.nettoyer_garage();
        let reste = v.actuelle;
        if v.autres.is_empty() {
            self.variantes_mut().remove(&sequence);
        }
        Some(reste)
    }

    /// Les plans que supprimerait « Supprimer la version de séquence `numero` » : ceux que ni une autre version de séquence ni
    /// le film (si l'on en change) ne montrent. Rend (plan, durée en images).
    pub fn plans_exclusifs_de_variante(&self, sequence: u64, numero: u32) -> Vec<(u64, usize)> {
        let Some(v) = self.variantes.get(&sequence) else { return Vec::new() };
        let ailleurs = |plan: u64| v.autres.iter().filter(|r| r.numero != numero).any(|r| r.contenu.slots.iter().any(|s| s.plan == plan));
        if numero == v.actuelle {
            return self.emplacements_du_film(sequence).into_iter().filter(|e| !ailleurs(e.plan)).map(|e| (e.plan, e.duree)).collect();
        }
        let dans_le_film = self.plans_de_sequence(sequence);
        v.rangee(numero).map_or_else(Vec::new, |r| r.contenu.slots.iter().filter(|e| !ailleurs(e.plan) && !dans_le_film.contains(&e.plan)).map(|e| (e.plan, e.duree)).collect())
    }

    /// Donne le nom court `nom` à la version `numero`. Faux si la séquence n'a pas cette version, ou si rien ne change.
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

    /// Les versions de séquence dont la séquence n'existe plus s'en vont avec elle (l'annulation les ramène, `Structure`).
    pub(crate) fn elaguer_variantes(&mut self) {
        if self.variantes.is_empty() {
            return;
        }
        let seqs: HashSet<u64> = self.sequences.iter().map(|s| s.id).collect();
        if self.variantes.keys().any(|s| !seqs.contains(s)) {
            self.variantes_mut().retain(|s, _| seqs.contains(s));
        }
    }

    // ------------------------------------------------------------------ plans et versions partagés

    /// Les autres versions de séquence de `sequence` qui montrent le plan `plan` : (numéro de la version de séquence, version du
    /// plan qu'elle y montre), dans l'ordre des lettres.
    pub fn variantes_montrant(&self, sequence: u64, plan: u64) -> Vec<(u32, u32)> {
        let Some(v) = self.variantes.get(&sequence) else { return Vec::new() };
        v.autres.iter().filter_map(|r| r.contenu.slots.iter().find(|s| s.plan == plan).map(|s| (r.numero, s.version))).collect()
    }

    /// Les versions de séquence (rangées) qui montrent la version `version` du plan `plan`.
    pub fn version_montree_ailleurs(&self, plan: u64, version: u32) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .variantes
            .values()
            .flat_map(|v| v.autres.iter())
            .filter(|r| r.contenu.slots.iter().any(|s| s.plan == plan && s.version == version))
            .map(|r| r.numero)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Les (plan, version) que montrent les versions de séquence rangées : on ne les supprime pas.
    pub(crate) fn versions_protegees(&self) -> HashSet<(u64, u32)> {
        self.variantes.values().flat_map(|v| v.autres.iter()).flat_map(|r| r.contenu.slots.iter().map(|s| (s.plan, s.version))).collect()
    }

    /// Dans la version de séquence rangée `numero`, le plan `plan` montre désormais sa version `version`. Faux si cette version
    /// de séquence ne montre pas ce plan, si le plan n'a pas cette version, ou si rien ne change.
    pub fn choisir_version_dans_variante(&mut self, sequence: u64, numero: u32, plan: u64, version: u32) -> bool {
        if !self.versions_connues(plan).contains(&version) {
            return false;
        }
        let Some(v) = self.variantes.get(&sequence) else { return false };
        if !v.rangee(numero).is_some_and(|r| r.contenu.slots.iter().any(|s| s.plan == plan && s.version != version)) {
            return false;
        }
        let v = self.variantes_mut().get_mut(&sequence).expect("variantes lues juste avant");
        let r = v.autres.iter_mut().find(|r| r.numero == numero).expect("version lue juste avant");
        for s in r.contenu.slots.iter_mut().filter(|s| s.plan == plan) {
            s.version = version;
        }
        true
    }

    /// « Mettre à jour dans les autres versions de séquence » : les versions de séquence rangées `cibles` montrent désormais la
    /// version courante du plan `plan` (le plan du film). Leur durée, leur ordre et leurs autres plans ne changent pas. Rend le
    /// nombre de versions de séquence changées.
    pub fn mettre_a_jour_variantes(&mut self, sequence: u64, plan: u64, cibles: &[u32]) -> usize {
        if !self.panels.iter().any(|p| p.id == plan) {
            return 0;
        }
        let version = self.numero_de_version(plan);
        let mut n = 0;
        for &numero in cibles {
            if self.choisir_version_dans_variante(sequence, numero, plan, version) {
                n += 1;
            }
        }
        n
    }

    /// Avant qu'un plan quitte le film (supprimé, ou réuni à un voisin) : si une autre version de sa séquence le montre, il est
    /// gardé au garage tel qu'il est (`copier` : une copie qui a ses propres dessins, quand le plan va servir de matière à un
    /// autre). Sans effet pour un plan que personne d'autre ne montre.
    pub(crate) fn ranger_plan_partage(&mut self, plan: u64, copier: bool) {
        if self.variantes.is_empty() {
            return;
        }
        let Some(sequence) = self.variantes.iter().find(|(_, v)| v.autres.iter().any(|r| r.contenu.slots.iter().any(|s| s.plan == plan))).map(|(s, _)| *s) else { return };
        let Some(index) = self.panels.iter().position(|p| p.id == plan) else { return };
        let source = self.panels[index].clone();
        let anciens: Vec<u64> = source.layers.iter().map(|l| l.id).collect();
        let panel = if copier {
            let mut c = self.copie_de(&source);
            c.id = plan;
            c
        } else {
            source
        };
        let versions = self.versions.remove(&plan);
        let reserve = if copier {
            // (La copie a des numéros de calques neufs : le contenu rogné, rangé par numéro de calque, les suit calque pour calque.)
            let rang: HashMap<u64, u64> = anciens.iter().copied().zip(panel.layers.iter().map(|l| l.id)).collect();
            self.reserves.get(&plan).cloned().map(|mut r| {
                for piece in [&mut r.head, &mut r.tail] {
                    // (Ce qu'un calque déjà supprimé avait laissé là ne revient plus : il n'est pas enregistré non plus.)
                    piece.layers.retain(|lp| rang.contains_key(&lp.layer));
                    for lp in &mut piece.layers {
                        lp.layer = rang[&lp.layer];
                    }
                }
                r
            })
        } else {
            self.reserves.remove(&plan)
        };
        let mut comments: Vec<Comment> = self.comments.iter().filter(|c| c.panel == plan).cloned().collect();
        if copier {
            for c in &mut comments {
                c.id = self.alloc();
            }
        } else {
            self.comments.retain(|c| c.panel != plan);
        }
        let parque = Parque { panel, versions, reserve, comments };
        self.variantes_mut().get_mut(&sequence).expect("séquence lue juste avant").garage.insert(plan, parque);
    }

    /// Comme `ranger_plan_partage`, pour plusieurs plans.
    pub(crate) fn ranger_plans_partages(&mut self, plans: &[u64], copier: bool) {
        if self.variantes.is_empty() {
            return;
        }
        for &p in plans {
            self.ranger_plan_partage(p, copier);
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
    /// Dessins bitmap des plans du garage qui ont quelque chose hors de la feuille (toile infinie).
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
    /// Les plans du garage (format 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub garage: Vec<PlanVarFichier>,
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
    /// Les emplacements (format 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emplacements: Vec<EmplacementFichier>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<Transition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<(u64, Vec<u64>)>,
    /// Format 1 seulement : les plans complets de la variante (jamais écrit).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plans: Vec<PlanVarFichier>,
    /// Format 1 seulement : les commentaires de ces plans (jamais écrit).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commentaires: Vec<Comment>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct EmplacementFichier {
    pub plan: u64,
    pub scene: u64,
    #[serde(default = "un")]
    pub version: u32,
    #[serde(default)]
    pub duree: usize,
}

fn un() -> u32 {
    1
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
    /// Les commentaires du plan (format 2 ; au format 1 ils sont dans la variante).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commentaires: Vec<Comment>,
}

/// Où est un plan lu dans le fichier : un plan d'une variante du format 1 (séquence, variante, plan) ou l'une de ses versions,
/// un plan du garage (séquence, plan) ou l'une de ses versions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Cle {
    Plan(usize, usize, usize),
    Version(usize, usize, usize, usize),
    Parque(usize, usize),
    ParqueVersion(usize, usize, usize),
}

/// Le contenu de `variantes.json` ; rien s'il n'y a aucune version rangée (le fichier part, ou, s'il est illisible, reste là).
/// `hors` : ceux des dessins du garage qui ont quelque chose hors de la feuille.
pub(crate) fn to_bytes(doc: &Document, hors: &BTreeSet<CelId>) -> Result<Option<Vec<u8>>, String> {
    let existe: HashSet<u64> = doc.panels.iter().map(|p| p.id).chain(doc.plans_de_variantes().map(|p| p.id)).collect();
    let protegees = doc.versions_protegees();
    let mut sequences = Vec::new();
    for (sequence, v) in doc.variantes.iter() {
        if v.autres.is_empty() {
            continue;
        }
        let mut garage = Vec::new();
        for (id, g) in &v.garage {
            garage.push(PlanVarFichier {
                scene: g.panel.scene,
                entree: crate::project::snapshot_value(&g.panel, &|c| c)?,
                // (Une entrée sans autre version est écrite quand elle porte un numéro de version qu'une autre version de séquence montre.)
                versions: match g.versions.as_ref().filter(|v| !v.autres.is_empty() || protegees.contains(&(*id, v.actuelle))) {
                    Some(v) => Some(crate::planvers::plan_fichier(*id, v)?),
                    None => None,
                },
                rognage: match g.reserve.as_ref().filter(|r| !r.is_empty()) {
                    Some(res) => Some(crate::rognage::plan_value(&g.panel, res)?),
                    None => None,
                },
                commentaires: g.comments.clone(),
            });
        }
        let mut autres = Vec::new();
        for r in &v.autres {
            let c = &r.contenu;
            autres.push(VarFichier {
                numero: r.numero,
                nom: r.nom.clone(),
                date: r.date,
                scenes: c.scenes.iter().map(|s| SceneFichier { id: s.id, nom: s.name.clone(), numero: s.number.clone() }).collect(),
                emplacements: c.slots.iter().map(|e| EmplacementFichier { plan: e.plan, scene: e.scene, version: e.version, duree: e.duree }).collect(),
                transitions: c.transitions.clone(),
                references: c.references.clone(),
                plans: Vec::new(),
                commentaires: Vec::new(),
            });
        }
        sequences.push(SeqFichier {
            sequence: *sequence,
            actuelle: v.actuelle,
            nom_actuelle: v.nom_actuelle.clone(),
            date_actuelle: v.date_actuelle,
            origines: v.origines.iter().filter(|(p, _)| existe.contains(p)).map(|(p, o)| [*p, *o]).collect(),
            garage,
            autres,
        });
    }
    if sequences.is_empty() {
        return Ok(None);
    }
    let f = Fichier { format: FORMAT.into(), version: VERSION, sequences, hors: hors.iter().copied().collect() };
    serde_json::to_vec_pretty(&f).map(Some).map_err(|e| e.to_string())
}

/// Les versions de séquence lues dans le texte de `variantes.json` : numéros uniques et non nuls, la version choisie n'est pas
/// aussi rangée, pas plus de `MAX_VARIANTES` par séquence, une version sans plan est oubliée.
pub(crate) fn parse(text: &str) -> Result<Fichier, String> {
    let mut f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de versions de séquences".into());
    }
    if f.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", f.version));
    }
    let mut vues = HashSet::new();
    f.sequences.retain(|s| vues.insert(s.sequence));
    for s in &mut f.sequences {
        s.actuelle = s.actuelle.max(1);
        s.nom_actuelle = nom_propre(&s.nom_actuelle);
        s.garage.truncate(MAX_PLANS);
        let mut numeros = HashSet::from([s.actuelle]);
        s.autres.retain(|v| v.numero >= 1 && (!v.plans.is_empty() || !v.emplacements.is_empty()) && numeros.insert(v.numero));
        s.autres.truncate(MAX_VARIANTES - 1);
        s.autres.sort_by_key(|v| v.numero);
        for v in &mut s.autres {
            v.nom = nom_propre(&v.nom);
            v.plans.truncate(MAX_PLANS);
            v.emplacements.truncate(MAX_PLANS);
        }
    }
    f.sequences.retain(|s| !s.autres.is_empty());
    Ok(f)
}

/// Le fichier du dossier `dir` : rien s'il n'existe pas, et un avertissement s'il est illisible (le projet s'ouvre sans
/// versions de séquence rangées, le fichier reste en place).
pub(crate) fn lire(dir: &Path, warnings: &mut Vec<String>) -> Option<Fichier> {
    let path = dir.join(FILE);
    match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes).map_err(|_| "le fichier n'est pas du texte (UTF-8)".to_string()).and_then(|t| parse(&t)) {
            Ok(f) => Some(f),
            Err(e) => {
                warnings.push(format!("{FILE} : {e} ; les versions de séquences ne sont pas chargées."));
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            warnings.push(format!("{} : {e} ; les versions de séquences ne sont pas chargées.", path.display()));
            None
        }
    }
}

/// Le fichier existe mais cette version ne sait pas le lire : l'enregistrement n'efface alors aucun dessin de `dessins/`
/// (ceux des versions qu'il décrit y sont encore).
pub(crate) fn illisible(dir: &Path) -> bool {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| ()).and_then(|t| parse(&t).map(|_| ()).map_err(|_| ())).is_err(),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Un plan du garage lu : son plan, ses versions rangées, son contenu rogné, ses commentaires.
fn lire_un_plan(
    pf: PlanVarFichier,
    cle: Cle,
    cle_version: impl Fn(usize) -> Cle,
    panels: &mut HashMap<Cle, Panel>,
    rognages: &mut HashMap<Cle, crate::rognage::Loaded>,
    missing: &BTreeSet<CelId>,
) -> Option<Parque> {
    let mut panel = panels.remove(&cle)?;
    poser_scene(&mut panel, pf.scene);
    let versions = pf.versions.and_then(|vf| {
        let mut rangees = Vec::new();
        for (ri, r) in vf.autres.iter().enumerate() {
            if let Some(mut p) = panels.remove(&cle_version(ri)) {
                poser_scene(&mut p, pf.scene);
                rangees.push(Rangee { numero: r.numero, date: r.date, panel: p });
            }
        }
        (!rangees.is_empty() || vf.autres.is_empty()).then_some(VersionsDuPlan { actuelle: vf.actuelle, date_actuelle: vf.date_actuelle, prochaine: vf.prochaine, autres: rangees })
    });
    let reserve = rognages.remove(&cle).and_then(|l| l.pour_plans(&[&panel], missing).into_iter().next().map(|(_, r)| r));
    let id = panel.id;
    Some(Parque { panel, versions, reserve, comments: pf.commentaires.into_iter().filter(|c| c.panel == id).collect() })
}

/// Met en place les versions de séquence lues : `panels` donne, pour chaque clé du fichier, le plan lu ; `rognages`, le contenu
/// rogné lu pour la clé d'un plan. Les versions d'une séquence qui n'existe plus sont oubliées. `missing` : dessins illisibles ;
/// si une version en cite, `Document::variantes_partielles` est levé. Un fichier du format 1 (copies complètes) est lu sans
/// rien perdre : chaque plan copié devient un plan du garage (voir l'en-tête du module).
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
        let mut garage: BTreeMap<u64, Parque> = BTreeMap::new();
        for (pi, pf) in s.garage.into_iter().enumerate() {
            match lire_un_plan(pf, Cle::Parque(si, pi), |ri| Cle::ParqueVersion(si, pi, ri), &mut panels, &mut rognages, missing) {
                Some(g) => {
                    garage.insert(g.panel.id, g);
                }
                None => warnings.push("version de séquence : un plan illisible, ignoré.".into()),
            }
        }
        let film: HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
        let mut autres = Vec::new();
        for (vi, v) in s.autres.into_iter().enumerate() {
            let scenes: Vec<Scene> = v.scenes.iter().map(|f| Scene { id: f.id, name: f.nom.clone(), number: f.numero.clone(), sequence: s.sequence }).collect();
            let mut contenu = if v.plans.is_empty() {
                let slots: Vec<Emplacement> = v.emplacements.into_iter().map(|e| Emplacement { plan: e.plan, scene: e.scene, version: e.version.max(1), duree: e.duree }).collect();
                Contenu { scenes, slots, transitions: v.transitions, references: v.references }
            } else {
                // Format 1 : une copie complète. Ses plans passent au garage, un emplacement chacun.
                let mut ancien = Ancien { scenes, ..Ancien::default() };
                let mut commentaires = v.commentaires;
                for (pi, pf) in v.plans.into_iter().enumerate() {
                    let scene = pf.scene;
                    let mut pf = pf;
                    pf.commentaires = Vec::new();
                    let Some(g) = lire_un_plan(pf, Cle::Plan(si, vi, pi), |ri| Cle::Version(si, vi, pi, ri), &mut panels, &mut rognages, missing) else { continue };
                    if !ancien.scenes.iter().any(|sc| sc.id == scene) {
                        warnings.push(format!("version {} d'une séquence : un plan sans scène connue, ignoré.", lettre(v.numero)));
                        continue;
                    }
                    let id = g.panel.id;
                    if let Some(vv) = g.versions {
                        ancien.versions.insert(id, vv);
                    }
                    if let Some(r) = g.reserve {
                        ancien.reserves.insert(id, r);
                    }
                    ancien.panels.push(g.panel);
                }
                let ids: HashSet<u64> = ancien.panels.iter().map(|p| p.id).collect();
                commentaires.retain(|k| ids.contains(&k.panel));
                ancien.comments = commentaires;
                ancien.transitions = v.transitions;
                ancien.references = v.references;
                let (contenu, parques) = ancien.en_parques();
                for g in parques {
                    garage.insert(g.panel.id, g);
                }
                contenu
            };
            // Un emplacement dont le plan n'est ni au film ni au garage est ignoré ; la scène de chaque plan est connue.
            let avant = contenu.slots.len();
            contenu.slots.retain(|e| film.contains(&e.plan) || garage.contains_key(&e.plan));
            if contenu.slots.len() < avant {
                warnings.push(format!("version {} d'une séquence : {} plan(s) illisible(s) ignoré(s).", lettre(v.numero), avant - contenu.slots.len()));
            }
            if contenu.slots.is_empty() {
                warnings.push(format!("version {} d'une séquence : illisible, ignorée.", lettre(v.numero)));
                continue;
            }
            let ids = contenu.ids_plans();
            contenu.scenes.retain(|sc| contenu.slots.iter().any(|e| e.scene == sc.id));
            contenu.transitions.retain(|t| t.out.is_some_and(|o| ids.contains(&o)) && t.inn.is_some_and(|i| ids.contains(&i)));
            contenu.references = contenu.references.into_iter().map(|(r, liens)| (r, liens.into_iter().filter(|p| ids.contains(p)).collect::<Vec<_>>())).filter(|(_, l)| !l.is_empty()).collect();
            for sc in &contenu.scenes {
                doc.reserve_id(sc.id);
            }
            for t in &contenu.transitions {
                doc.reserve_id(t.id);
            }
            autres.push(VarianteRangee { numero: v.numero, nom: v.nom, date: v.date, contenu });
        }
        if autres.is_empty() {
            continue;
        }
        let mut v = VariantesDeSequence {
            actuelle: s.actuelle,
            nom_actuelle: s.nom_actuelle,
            date_actuelle: s.date_actuelle,
            autres,
            origines: s.origines.into_iter().map(|[p, o]| (p, o)).collect(),
            garage,
        };
        v.nettoyer_garage();
        for g in v.garage.values() {
            for p in std::iter::once(&g.panel).chain(g.versions.iter().flat_map(|v| v.autres.iter().map(|r| &r.panel))) {
                doc.reserve_id(p.id);
                p.marks.iter().for_each(|m| doc.reserve_id(m.id));
            }
            g.comments.iter().for_each(|k| doc.reserve_id(k.id));
            if let Some(r) = &g.reserve {
                for piece in [&r.head, &r.tail] {
                    piece.marks.iter().for_each(|m| doc.reserve_id(m.id));
                    piece.comments.iter().for_each(|k| doc.reserve_id(k.id));
                }
            }
        }
        out.insert(s.sequence, v);
    }
    if partielle {
        warnings.push("Des dessins de versions de séquences manquent : le fichier des versions de séquences ne sera pas réécrit tant qu'ils ne sont pas là.".into());
    }
    doc.variantes_partielles = partielle;
    doc.variantes = Arc::new(out);
}
