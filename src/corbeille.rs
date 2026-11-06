//! La corbeille du projet (partie 104 de `docs/INTERFACE.md`, format 47).
//!
//! Règle du réalisateur : « ce qui sort du film reste récupérable ». Tout ce qu'un geste du montage retire du film va ici, et
//! reste récupérable après l'enregistrement et la réouverture :
//!
//! - un **plan** supprimé (`Document::jeter_plan`), avec son contenu rogné et ses commentaires ;
//! - des **calques** (`Document::jeter_calques`) : un calque supprimé, ou les dessins qu'un geste de clés a recouverts ou
//!   poussés hors du plan (rangés dans un calque « Dessins remplacés », à leurs images d'origine) ;
//! - des **images rognées** qu'une jonction de plans ou « Oublier le contenu rogné » ne peut plus garder
//!   (`Document::jeter_images`) : un petit plan à part, qui reprend les dessins aux mêmes images.
//!
//! **Pour un geste à venir** (lift, extract, écrasement…) : il appelle `jeter_calques(plan, calques, raison)` avec le calque
//! (ou les calques) qui quitte le film, ou `jeter_plan(index, exclus, raison)` avant de retirer un plan ; l'entrée, la
//! restauration, l'annulation, le fichier et la fenêtre sont les mêmes. Rien d'autre à écrire.
//!
//! **Dans le document.** `Document::corbeille` fait partie de `Structure` : jeter est un changement comme les autres, une
//! seule étape d'annulation qui le défait, sans double dans la corbeille (annuler retire l'entrée). Une entrée n'est pas un
//! plan du film : rien ne la compte (exports, numéros, Kitsu, versions rangées, envois).
//!
//! **Les dessins** restent dans `dessins/` sous leur numéro, comme tous les autres ; l'enregistrement ne les efface pas tant
//! que la corbeille les cite. À l'ouverture ils ne sont **pas lus** (règle de la mémoire d'un long épisode, partie 95) : la
//! corbeille ne garde que ce qu'il faut pour les montrer (`vignette`) et les relire à la demande. Une entrée jetée dans la
//! séance garde son plan en mémoire (`Contenu::Vif`), avec ses dessins que `elagage.rs` retient.
//!
//! **Le fichier** : `corbeille.json` à côté du manifeste (rien dans la corbeille : il n'y en a pas). Format 47 du projet : un
//! projet 47 s'ouvre avec une corbeille vide quand il n'a pas ce fichier ; un projet d'avant (46 ou moins) s'ouvre sans rien
//! perdre, corbeille vide ; une version plus ancienne que celle-ci refuse un projet 47 comme les autres montées de format.
//! Un projet partagé (plusieurs personnes) ne garde pas la corbeille dans son dossier : celle de la séance seulement.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::commentaires::Comment;
use crate::document::{Cel, CelId, Document, Layer, Panel};
use crate::project::Tete;
use crate::rognage::{Piece, Reserve};

pub(crate) const FILE: &str = "corbeille.json";
const FORMAT: &str = "pelure-corbeille";
const VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorte {
    /// Un plan (ou un petit plan fait d'images rognées) : il revient dans le film.
    Plan,
    /// Des calques : ils reviennent dans leur plan.
    Calques,
}

/// Ce qu'une entrée garde du plan (ou du plan fait des calques) : le plan lui-même, pour une entrée de la séance ; son texte
/// de fichier, pour une entrée relue (ses dessins sont dans `dessins/`, relus à la restauration).
#[derive(Clone)]
pub enum Contenu {
    Vif { panel: Panel, reserve: Option<Reserve>, commentaires: Vec<Comment> },
    Fichier { plan: serde_json::Value, reserve: Option<serde_json::Value>, commentaires: Vec<Comment> },
}

#[derive(Clone)]
pub struct Entree {
    /// Numéro de l'entrée (unique dans le projet, comme tous les numéros).
    pub id: u64,
    /// Secondes depuis 1970.
    pub date: u64,
    pub sorte: Sorte,
    /// Ce que la fenêtre dit : « Plan 3 », « Calque « Dessin » (Plan 2) »…
    pub titre: String,
    /// Pourquoi c'est là : « Plan supprimé », « Calque supprimé », « Dessins remplacés (Alt + glisser) »…
    pub raison: String,
    /// La séquence d'origine (numéro et nom).
    pub sequence: u64,
    pub sequence_nom: String,
    /// Le plan d'origine (celui qui est jeté, ou qui portait les calques).
    pub plan: u64,
    /// Le plan qui précédait, dans la même séquence : le plan revient derrière lui s'il existe encore.
    pub apres: Option<u64>,
    /// Rang d'origine de chaque calque (pour `Calques`).
    pub rangs: Vec<usize>,
    /// Les dessins de l'entrée, pour les garder dans `dessins/`.
    pub cels: Vec<CelId>,
    /// Le dessin que la fenêtre montre en vignette.
    pub vignette: Option<CelId>,
    /// Le plus grand numéro cité (plan, marques, commentaires, entrée) : les numéros neufs restent au-dessus.
    pub id_max: u64,
    /// La feuille au moment où l'entrée a été jetée (les dessins ont cette taille).
    pub(crate) tete: Tete,
    pub contenu: Contenu,
}

/// La corbeille du projet. Les entrées sont partagées (`Arc`) : une étape d'annulation ne les recopie pas.
#[derive(Clone, Default)]
pub struct Corbeille {
    pub entrees: Vec<Arc<Entree>>,
}

impl Corbeille {
    pub fn is_empty(&self) -> bool {
        self.entrees.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    /// Tous les dessins que la corbeille garde.
    pub fn cels(&self) -> impl Iterator<Item = CelId> + '_ {
        self.entrees.iter().flat_map(|e| e.cels.iter().copied())
    }

    pub fn trouver(&self, id: u64) -> Option<&Arc<Entree>> {
        self.entrees.iter().find(|e| e.id == id)
    }
}

fn maintenant() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Le premier dessin du plan, en partant du calque du haut : la vignette de l'entrée.
fn premier_dessin(p: &Panel) -> Option<CelId> {
    p.layers.iter().rev().find_map(|l| l.frames.iter().flatten().next().copied())
}

fn dessins_de(p: &Panel, reserve: Option<&Reserve>) -> Vec<CelId> {
    let mut out = std::collections::HashSet::new();
    crate::elagage::dessins_du_plan_retenus(p, &mut out);
    if let Some(r) = reserve {
        out.extend(r.cels());
    }
    let mut v: Vec<CelId> = out.into_iter().collect();
    v.sort_unstable();
    v
}

impl Document {
    fn nom_de_sequence(&self, sequence: u64) -> String {
        self.sequences.iter().position(|s| s.id == sequence).map_or_else(String::new, |i| {
            let s = &self.sequences[i];
            if s.name.trim().is_empty() { format!("Séquence {}", i + 1) } else { s.name.clone() }
        })
    }

    /// Range une entrée : numéro, date, plus grand numéro cité.
    fn ranger_dans_la_corbeille(&mut self, mut e: Entree) {
        e.id = self.alloc();
        e.date = maintenant();
        let mut ids = vec![e.id];
        match &e.contenu {
            Contenu::Vif { panel, reserve, commentaires } => {
                ids.push(panel.id);
                ids.extend(panel.marks.iter().map(|m| m.id));
                ids.extend(commentaires.iter().map(|c| c.id));
                if let Some(r) = reserve {
                    for piece in [&r.head, &r.tail] {
                        ids.extend(piece.marks.iter().map(|m| m.id));
                        ids.extend(piece.comments.iter().map(|c| c.id));
                    }
                }
            }
            Contenu::Fichier { .. } => {}
        }
        ids.extend(e.cels.iter().copied());
        e.id_max = ids.into_iter().max().unwrap_or(0);
        self.corbeille.entrees.push(Arc::new(e));
    }

    /// Le plan `index` va à la corbeille, **avant** d'être retiré du film. `exclus` : les plans retirés avec lui (le plan « d'avant »
    /// qui sert à retrouver sa place n'en fait pas partie). Sans effet pour un plan qu'une autre version de sa séquence montre : il
    /// est déjà gardé au garage (`variantes.rs`).
    pub(crate) fn jeter_plan(&mut self, index: usize, exclus: &[u64], raison: &str) -> bool {
        let Some(p) = self.panels.get(index) else { return false };
        let id = p.id;
        if self.variantes.iter().any(|(_, v)| v.garage.contains_key(&id)) {
            return false;
        }
        let sequence = self.sequence_of(index);
        let apres = (0..index).rev().find(|&i| !exclus.contains(&self.panels[i].id)).filter(|&i| self.sequence_of(i) == sequence).map(|i| self.panels[i].id);
        let reserve = self.reserves.get(&id).cloned();
        let commentaires: Vec<Comment> = self.comments.iter().filter(|c| c.panel == id).cloned().collect();
        let panel = p.clone();
        let e = Entree {
            id: 0,
            date: 0,
            sorte: Sorte::Plan,
            titre: crate::storyboard::panel_title(self, index),
            raison: raison.into(),
            sequence,
            sequence_nom: self.nom_de_sequence(sequence),
            plan: id,
            apres,
            rangs: Vec::new(),
            cels: dessins_de(&panel, reserve.as_ref()),
            vignette: premier_dessin(&panel),
            id_max: 0,
            tete: Tete::de(self),
            contenu: Contenu::Vif { panel, reserve, commentaires },
        };
        self.ranger_dans_la_corbeille(e);
        true
    }

    /// Des calques du plan `plan` (avec leur rang d'origine) quittent le film : ils vont à la corbeille. Les calques sont ceux
    /// d'*avant* leur retrait ; rien n'est retiré ici.
    pub(crate) fn jeter_calques(&mut self, plan: u64, calques: Vec<(usize, Layer)>, raison: &str) -> bool {
        let Some(index) = self.panels.iter().position(|p| p.id == plan) else { return false };
        if calques.is_empty() {
            return false;
        }
        let mut panel = self.panels[index].clone();
        let rangs: Vec<usize> = calques.iter().map(|(r, _)| *r).collect();
        panel.layers = calques.into_iter().map(|(_, l)| l).collect();
        let sequence = self.sequence_of(index);
        let n = panel.layers.len();
        let nom = if n == 1 { format!("Calque « {} »", panel.layers[0].name) } else { format!("{n} calques") };
        let e = Entree {
            id: 0,
            date: 0,
            sorte: Sorte::Calques,
            titre: format!("{nom} ({})", crate::storyboard::panel_title(self, index)),
            raison: raison.into(),
            sequence,
            sequence_nom: self.nom_de_sequence(sequence),
            plan,
            apres: None,
            rangs,
            cels: dessins_de(&panel, None),
            vignette: premier_dessin(&panel),
            id_max: 0,
            tete: Tete::de(self),
            contenu: Contenu::Vif { panel, reserve: None, commentaires: Vec::new() },
        };
        self.ranger_dans_la_corbeille(e);
        true
    }

    /// Des images rognées (`piece`) du plan `modele` ne peuvent plus être gardées (jonction de plans, « Oublier le contenu
    /// rogné ») : un petit plan à part, qui reprend leurs dessins aux mêmes images, va à la corbeille ; il reviendra derrière
    /// le plan `apres` (le plan réuni, ou le plan lui-même). Faux s'il n'y avait aucun dessin.
    pub(crate) fn jeter_images(&mut self, modele: &Panel, apres: u64, piece: &Piece, raison: &str) -> bool {
        if piece.len == 0 {
            return false;
        }
        let mut layers = Vec::new();
        for lp in &piece.layers {
            let Some(l) = modele.layers.iter().find(|l| l.id == lp.layer) else { continue };
            if lp.frames.iter().all(Option::is_none) {
                continue;
            }
            let mut c = l.clone();
            c.frames = lp.frames.clone();
            c.frames.resize(piece.len, None);
            c.roles = lp.roles.clone();
            c.fx.clear();
            c.pos_keys.clear();
            c.opacity_keys.clear();
            layers.push(c);
        }
        if layers.is_empty() {
            return false;
        }
        let mut panel = modele.clone();
        panel.id = self.alloc();
        panel.layers = layers;
        panel.length = piece.len;
        panel.camera.clear();
        panel.marks.clear();
        panel.cues = Default::default();
        panel.action.clear();
        panel.dialogue.clear();
        panel.notes.clear();
        let index = self.panels.iter().position(|p| p.id == apres);
        let sequence = index.map_or(0, |i| self.sequence_of(i));
        let titre = match index {
            Some(i) => format!("Images rognées ({})", crate::storyboard::panel_title(self, i)),
            None => "Images rognées".to_string(),
        };
        let e = Entree {
            id: 0,
            date: 0,
            sorte: Sorte::Plan,
            titre,
            raison: raison.into(),
            sequence,
            sequence_nom: self.nom_de_sequence(sequence),
            plan: panel.id,
            apres: Some(apres),
            rangs: Vec::new(),
            cels: dessins_de(&panel, None),
            vignette: premier_dessin(&panel),
            id_max: 0,
            tete: Tete::de(self),
            contenu: Contenu::Vif { panel, reserve: None, commentaires: Vec::new() },
        };
        self.ranger_dans_la_corbeille(e);
        true
    }

    /// La corbeille ne garde plus l'entrée `id` (restaurée, ou supprimée pour de bon).
    pub(crate) fn retirer_de_la_corbeille(&mut self, id: u64) -> bool {
        let n = self.corbeille.entrees.len();
        self.corbeille.entrees.retain(|e| e.id != id);
        self.corbeille.entrees.len() != n
    }

    /// Remet l'entrée `id` dans le film, d'après `charge` (son contenu, relu au besoin : `charger`). Un plan revient derrière le plan qui
    /// le précédait s'il existe encore, sinon à la fin de sa séquence, sinon à la fin de `sequence_courante` ; des calques
    /// reviennent dans leur plan, qui doit exister. Rend la phrase à dire et le plan à rendre courant. Une seule étape
    /// d'annulation (l'appelant la prend), qui rend aussi l'entrée à la corbeille.
    pub(crate) fn restaurer_entree(&mut self, id: u64, charge: Charge, sequence_courante: u64) -> Result<(String, usize), String> {
        let Some(e) = self.corbeille.trouver(id).cloned() else { return Err("Cette entrée n'est plus dans la corbeille.".into()) };
        let Charge { mut panel, cels, reserve, commentaires } = charge;
        match e.sorte {
            Sorte::Calques => {
                let Some(index) = self.panels.iter().position(|p| p.id == e.plan) else {
                    return Err("Le plan de ce calque n'existe plus : restaure d'abord le plan (s'il est dans la corbeille), ou ce calque ne peut pas revenir.".into());
                };
                for (id, c) in cels {
                    self.cels.entry(id).or_insert(c);
                }
                let n = panel.layers.len();
                for (k, mut l) in panel.layers.drain(..).enumerate() {
                    if self.panels[index].layers.iter().any(|x| x.id == l.id) {
                        l.id = crate::document::fresh_layer_id();
                    }
                    let at = e.rangs.get(k).copied().unwrap_or(0).min(self.panels[index].layers.len());
                    self.panels[index].layers.insert(at, l);
                }
                self.retirer_de_la_corbeille(id);
                Ok((format!("{} revenu{} dans {}.", if n == 1 { "Calque" } else { "Calques" }, if n == 1 { "" } else { "s" }, crate::storyboard::panel_title(self, index)), index))
            }
            Sorte::Plan => {
                for (id, c) in cels {
                    self.cels.entry(id).or_insert(c);
                }
                let ancien = panel.id;
                if self.panels.iter().any(|p| p.id == ancien) {
                    panel.id = self.alloc();
                }
                // Où : derrière le plan d'avant, sinon à la fin de la séquence, sinon à la fin de la séquence courante.
                let (at, place) = match e.apres.and_then(|a| self.panels.iter().position(|p| p.id == a)) {
                    Some(i) => (i + 1, "à sa place"),
                    None => match (0..self.panels.len()).rev().find(|&i| self.sequence_of(i) == e.sequence).filter(|_| e.sequence != 0) {
                        Some(i) => (i + 1, "à la fin de sa séquence"),
                        None => {
                            let n = self.panels.len();
                            ((0..n).rev().find(|&i| self.sequence_of(i) == sequence_courante).map_or(n, |i| i + 1), "à la fin de la séquence courante")
                        }
                    },
                };
                let sequence = if at > 0 { self.sequence_of(at - 1) } else { self.sequence_of(0) };
                panel.scene = self.new_scene(sequence);
                let nouveau = panel.id;
                if let Some(r) = reserve {
                    self.reserves.insert(nouveau, r);
                }
                for mut c in commentaires {
                    c.panel = nouveau;
                    self.reserve_id(c.id);
                    self.comments.push(c);
                }
                let index = self.insert_and_normalize(at, vec![panel]);
                self.retirer_de_la_corbeille(id);
                Ok((format!("« {} » restauré {place}.", e.titre), index))
            }
        }
    }
}

/// Le contenu d'une entrée, prêt à remettre : le plan, ses dessins à ajouter au document (ceux que la séance n'a plus en mémoire,
/// relus de `dessins/`), son contenu rogné, ses commentaires.
pub(crate) struct Charge {
    pub panel: Panel,
    pub cels: HashMap<CelId, Cel>,
    pub reserve: Option<Reserve>,
    pub commentaires: Vec<Comment>,
}

/// Lit le contenu de `e`. Une entrée de la séance l'a déjà ; une entrée relue demande le dossier du projet (`dir`) : ses dessins s'y
/// relisent, avec les protections de l'ouverture (un dessin manquant est dit, le plan revient quand même). Rend aussi ce qu'il
/// faut dire des dessins qui manquaient.
pub(crate) fn charger(e: &Entree, dir: Option<&Path>, deja: &dyn Fn(CelId) -> bool) -> Result<(Charge, Option<String>), String> {
    match &e.contenu {
        Contenu::Vif { panel, reserve, commentaires } => {
            let mut cels = HashMap::new();
            // (Un dessin de la séance que l'élagage aurait lâché ne peut pas arriver : la corbeille le retient. Par prudence, relu.)
            if let Some(dir) = dir {
                for &id in e.cels.iter().filter(|id| !deja(**id)) {
                    if let Ok(c) = crate::project::charger_un_dessin(dir, id, e.tete.largeur, e.tete.hauteur) {
                        cels.insert(id, c);
                    }
                }
            }
            Ok((Charge { panel: panel.clone(), cels, reserve: reserve.clone(), commentaires: commentaires.clone() }, None))
        }
        Contenu::Fichier { plan, reserve, commentaires } => {
            let dir = dir.ok_or("Le projet n'est pas enregistré : ses dessins de la corbeille ne peuvent pas être relus.")?;
            let loaded = crate::project::charger_plans(dir, &e.tete, 1, vec![plan.clone()], false)?;
            let Some(panel) = loaded.doc.panels.first().cloned() else { return Err("Plan illisible dans la corbeille.".into()) };
            let reserve = match reserve {
                Some(v) => {
                    let texte = serde_json::to_string(v).map_err(|er| er.to_string())?;
                    crate::rognage::parse(&texte)?.pour_plans(&[&panel], &loaded.manquants).into_iter().next().map(|(_, r)| r)
                }
                None => None,
            };
            let avertissement = (!loaded.manquants.is_empty()).then(|| format!("{} dessin(s) manquaient dans le dossier du projet : ils reviennent vides.", loaded.manquants.len()));
            let cels: HashMap<CelId, Cel> = loaded.doc.cels.into_iter().filter(|(id, _)| !deja(*id)).collect();
            Ok((Charge { panel, cels, reserve, commentaires: commentaires.clone() }, avertissement))
        }
    }
}

// ----------------------------------------------------------------------------------------------- le fichier

#[derive(Serialize, Deserialize)]
struct EntreeFichier {
    id: u64,
    #[serde(default)]
    date: u64,
    sorte: String,
    #[serde(default)]
    titre: String,
    #[serde(default)]
    raison: String,
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    sequence_nom: String,
    #[serde(default)]
    plan: u64,
    #[serde(default)]
    apres: Option<u64>,
    #[serde(default)]
    rangs: Vec<usize>,
    #[serde(default)]
    dessins: Vec<CelId>,
    #[serde(default)]
    vignette: Option<CelId>,
    #[serde(default)]
    id_max: u64,
    tete: Tete,
    contenu: serde_json::Value,
    #[serde(default)]
    rognage: Option<serde_json::Value>,
    #[serde(default)]
    commentaires: Vec<Comment>,
}

#[derive(Serialize, Deserialize)]
struct Fichier {
    format: String,
    version: u32,
    #[serde(default)]
    entrees: Vec<EntreeFichier>,
}

/// Le contenu de `corbeille.json` ; rien : la corbeille est vide (le fichier part). Un projet partagé n'écrit pas la sienne.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.corbeille.is_empty() || doc.est_partage() {
        return Ok(None);
    }
    let mut entrees = Vec::with_capacity(doc.corbeille.len());
    for e in &doc.corbeille.entrees {
        let (contenu, rognage, commentaires) = match &e.contenu {
            Contenu::Vif { panel, reserve, commentaires } => (
                crate::project::snapshot_value(panel, &|c| c)?,
                match reserve {
                    Some(r) if !r.is_empty() => Some(crate::rognage::plan_value(panel, r)?),
                    _ => None,
                },
                commentaires.clone(),
            ),
            Contenu::Fichier { plan, reserve, commentaires } => (plan.clone(), reserve.clone(), commentaires.clone()),
        };
        entrees.push(EntreeFichier {
            id: e.id,
            date: e.date,
            sorte: match e.sorte {
                Sorte::Plan => "plan".into(),
                Sorte::Calques => "calques".into(),
            },
            titre: e.titre.clone(),
            raison: e.raison.clone(),
            sequence: e.sequence,
            sequence_nom: e.sequence_nom.clone(),
            plan: e.plan,
            apres: e.apres,
            rangs: e.rangs.clone(),
            dessins: e.cels.clone(),
            vignette: e.vignette,
            id_max: e.id_max,
            tete: e.tete.clone(),
            contenu,
            rognage,
            commentaires,
        });
    }
    serde_json::to_vec_pretty(&Fichier { format: FORMAT.into(), version: VERSION, entrees }).map(Some).map_err(|e| e.to_string())
}

/// Lit le texte de `corbeille.json`.
pub(crate) fn parse(text: &str) -> Result<Corbeille, String> {
    let f: Fichier = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if f.format != FORMAT {
        return Err("ce n'est pas un fichier de corbeille".into());
    }
    if f.version > VERSION {
        return Err(format!("écrite par une version plus récente (format {})", f.version));
    }
    let mut vus = BTreeSet::new();
    let entrees = f
        .entrees
        .into_iter()
        .filter(|e| vus.insert(e.id))
        .map(|e| {
            Arc::new(Entree {
                id: e.id,
                date: e.date,
                sorte: if e.sorte == "calques" { Sorte::Calques } else { Sorte::Plan },
                titre: e.titre,
                raison: e.raison,
                sequence: e.sequence,
                sequence_nom: e.sequence_nom,
                plan: e.plan,
                apres: e.apres,
                rangs: e.rangs,
                cels: e.dessins,
                vignette: e.vignette,
                id_max: e.id_max.max(e.id),
                tete: e.tete,
                contenu: Contenu::Fichier { plan: e.contenu, reserve: e.rognage, commentaires: e.commentaires },
            })
        })
        .collect();
    Ok(Corbeille { entrees })
}

/// L'ouverture : la corbeille du dossier (absente : vide). Un fichier illisible ou d'une version plus récente n'empêche pas d'ouvrir
/// le projet : on le dit, la corbeille est vide, et le fichier reste en place (`write_sidecar`).
pub(crate) fn lire(dir: &Path, warnings: &mut Vec<String>) -> Corbeille {
    let path = dir.join(FILE);
    match std::fs::read_to_string(&path) {
        Ok(text) => match parse(&text) {
            Ok(c) => c,
            Err(e) => {
                warnings.push(format!("corbeille ({}) : {e} ; ignorée, le fichier reste là.", FILE));
                Corbeille::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Corbeille::default(),
        Err(e) => {
            warnings.push(format!("corbeille ({}) : {e}", FILE));
            Corbeille::default()
        }
    }
}
