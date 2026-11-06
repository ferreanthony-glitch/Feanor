//! Le format d'un projet partagé (`docs/COOPERATION.md`, §3) : lecture, enregistrement, conversion.
//!
//! Sous-module de `project` : il lit les mêmes types privés (`Manifest`, `PanelEntry`…) que l'ouverture et l'enregistrement
//! d'un projet classique, dont il réutilise tout le travail — c'est `load_manifest_avec` qui lit les dessins, les sons, les
//! effets ; ici on ne fait que **rassembler** les morceaux (la structure, les fichiers de plans, les commentaires de chacun)
//! avant, et les **séparer** après.

use super::*;

use std::sync::Arc;

use crate::commentaires::Comment;
use crate::depot::{self, Depot};
use crate::empreinte;
use crate::partagecom::{self, Enregistrement};
use crate::planvers::VersionsDuPlan;
use crate::rognage::Reserve;

/// Format du fichier d'un plan (`plans/<n°>.json`).
pub(crate) const FORMAT_PLAN: &str = "shot-plan";
pub(crate) const VERSION_PLAN: u32 = 1;
/// Un fichier de plan plus gros qu'ici n'est pas lu (un fichier abîmé ne remplit pas la mémoire).
const PLAN_MAX: usize = 400 * 1024 * 1024;

/// Qui a écrit un fichier, et quand (millisecondes depuis 1970).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub id: String,
    pub nom: String,
    pub le: u64,
}

/// Le plan de chaque scène (un plan = une scène à un panneau).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanListe {
    pub scene: u64,
    pub plan: u64,
}

/// Le bloc `partage` du fichier de structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlocPartage {
    /// Un nom tiré au hasard, qui dit « ce film-ci » (un dossier copié ne se confond pas avec l'original).
    pub projet: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enregistre_par: Option<Signature>,
    #[serde(default)]
    pub plans: Vec<PlanListe>,
}

/// Le contenu d'un plan : tout ce que son porteur écrit (sans la révision ni la signature, qui changent à chaque écriture).
#[derive(Serialize, Deserialize)]
pub(crate) struct ContenuPlan {
    pub panneau: PanelEntry,
    /// Dessins bitmap qui ont quelque chose hors de la feuille (toile infinie).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hors: Vec<u64>,
    /// Les autres versions du plan (`planvers.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<crate::planvers::PlanFichier>,
    /// Le contenu rogné (au format de `rognages.json`, pour ce plan seul).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rognage: Option<serde_json::Value>,
    /// L'empreinte du fichier de chacun de ses dessins, telle que son porteur l'a écrite : un lecteur ne prend le plan que
    /// si les fichiers qu'il a sous les yeux ont ces empreintes (un dossier synchronisé peut livrer le plan avant ses
    /// dessins, ou garder un ancien dessin sous le même nom). Vide : pas de vérification (brouillons).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dessins: BTreeMap<u64, u64>,
}

/// `plans/<n°>.json`.
#[derive(Serialize, Deserialize)]
pub(crate) struct PlanFichier {
    pub format: String,
    pub version: u32,
    pub plan: u64,
    pub scene: u64,
    /// Augmente à chaque écriture : un autre poste sait quel fichier attendre (§5.4).
    #[serde(default)]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enregistre_par: Option<Signature>,
    /// (Un champ à part et non `#[serde(flatten)]` : le « flatten » de serde perd les clés numériques des tables du plan.)
    pub contenu: ContenuPlan,
}

impl PlanFichier {
    /// Lit les octets d'un fichier de plan.
    pub(crate) fn lire(octets: &[u8]) -> Result<PlanFichier, String> {
        if octets.len() > PLAN_MAX {
            return Err("fichier de plan démesuré".into());
        }
        let texte = std::str::from_utf8(octets).map_err(|_| "le fichier du plan n'est pas du texte (UTF-8)".to_string())?;
        let f: PlanFichier = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("plan illisible ({e})"))?;
        if f.format != FORMAT_PLAN {
            return Err("ce n'est pas un fichier de plan".into());
        }
        if f.version > VERSION_PLAN {
            return Err(format!("plan écrit par une version plus récente (format {})", f.version));
        }
        Ok(f)
    }

    pub(crate) fn octets(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(self).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------------- les données d'un plan

/// Tout ce qui appartient à un plan, copié du document (peu coûteux : les dessins ne sont pas dedans).
#[derive(Clone)]
pub(crate) struct DonneesPlan {
    pub panel: Panel,
    pub versions: Option<VersionsDuPlan>,
    pub reserve: Option<Reserve>,
    pub hors: Vec<CelId>,
    /// L'empreinte du fichier de chaque dessin (voir `ContenuPlan::dessins`).
    pub dessins: BTreeMap<u64, u64>,
}

/// L'empreinte du ou des fichiers d'un dessin : le dessin lui-même, sa strate Fond (peinture et coupe) et son hors-feuille.
/// Rien si le fichier du dessin n'est pas là.
pub(crate) fn hash_dessin(dir: &Path, id: CelId) -> Option<u64> {
    let principal = if drawing_path(dir, id, true).exists() { drawing_path(dir, id, true) } else { drawing_path(dir, id, false) };
    let mut octets = fs::read(principal).ok()?;
    for extra in [fond_path(dir, id), coupe_path(dir, id), hors_png_path(dir, id), hors_json_path(dir, id)] {
        if let Ok(b) = fs::read(&extra) {
            octets.extend(b);
        }
    }
    Some(empreinte::de_octets(&octets))
}

impl DonneesPlan {
    pub(crate) fn de(doc: &Document, index: usize, dessins: &BTreeMap<u64, u64>) -> DonneesPlan {
        let panel = doc.panels[index].clone();
        let versions = doc.versions.get(&panel.id).filter(|v| !v.autres.is_empty()).cloned();
        let reserve = doc.reserves.get(&panel.id).filter(|r| !r.is_empty()).cloned();
        let mut d = DonneesPlan { panel, versions, reserve, hors: Vec::new(), dessins: BTreeMap::new() };
        let cels = d.cels();
        d.hors = hors_of(doc, &cels).into_iter().collect();
        d.dessins = dessins.iter().filter(|(id, _)| cels.contains(id)).map(|(k, v)| (*k, *v)).collect();
        d
    }

    /// Les dessins du plan : ceux de ses calques, de ses versions rangées et de son contenu rogné.
    pub(crate) fn cels(&self) -> BTreeSet<CelId> {
        let mut c: BTreeSet<CelId> = empreinte::dessins_du_plan(&self.panel).into_iter().collect();
        if let Some(v) = &self.versions {
            for r in &v.autres {
                c.extend(empreinte::dessins_du_plan(&r.panel));
            }
        }
        if let Some(r) = &self.reserve {
            c.extend(r.cels());
        }
        c
    }

    pub(crate) fn contenu(&self) -> Result<ContenuPlan, String> {
        let cels = self.cels();
        let panneau = panel_entry(&self.panel, &|c| c, &|c| cels.contains(&c), false);
        let versions = match &self.versions {
            Some(v) => Some(crate::planvers::plan_fichier(self.panel.id, v)?),
            None => None,
        };
        let rognage = match &self.reserve {
            Some(r) => Some(crate::rognage::plan_value(&self.panel, r)?),
            None => None,
        };
        Ok(ContenuPlan { panneau, hors: self.hors.clone(), versions, rognage, dessins: self.dessins.clone() })
    }

    /// L'empreinte du contenu : la même à l'enregistrement et à la lecture d'un plan, donc un plan qu'on n'a pas touché ne
    /// paraît jamais modifié.
    pub(crate) fn empreinte(&self) -> Result<u64, String> {
        let octets = serde_json::to_vec(&self.contenu()?).map_err(|e| e.to_string())?;
        Ok(empreinte::de_octets(&octets))
    }

    pub(crate) fn fichier(&self, scene: u64, revision: u64, signature: Option<Signature>) -> Result<PlanFichier, String> {
        Ok(PlanFichier { format: FORMAT_PLAN.into(), version: VERSION_PLAN, plan: self.panel.id, scene, revision, enregistre_par: signature, contenu: self.contenu()? })
    }
}

/// Remplace dans `panel` les numéros de dessins de `map` (images des calques, noms et marques de couleur des dessins, couples
/// trait / couleur des calques de couleur) : un brouillon cite des dessins neufs là où le plan en avait de modifiés.
pub(crate) fn remapper_dessins(panel: &mut Panel, map: &HashMap<CelId, CelId>) {
    let nouveau = |id: &CelId| map.get(id).copied().unwrap_or(*id);
    for l in &mut panel.layers {
        for f in l.frames.iter_mut().flatten() {
            *f = nouveau(f);
        }
        l.names = l.names.iter().map(|(k, v)| (nouveau(k), v.clone())).collect();
        l.color_marks = l.color_marks.iter().map(|(k, v)| (nouveau(k), *v)).collect();
        if let Some(c) = l.couleur.as_mut() {
            c.dessins = c.dessins.iter().map(|(t, k)| (nouveau(t), nouveau(k))).collect();
            c.griffonnages = c.griffonnages.iter().map(|(p, g)| (nouveau(p), nouveau(g))).collect();
        }
    }
}

// ---------------------------------------------------------------------------------- lire un plan

/// Un plan lu et décodé, prêt à être posé dans le document (sur l'interface : rien à décoder).
pub(crate) struct PlanCharge {
    pub plan: u64,
    pub scene: u64,
    pub revision: u64,
    pub signature: Option<Signature>,
    pub panel: Panel,
    pub cels: HashMap<CelId, Cel>,
    pub versions: Option<VersionsDuPlan>,
    pub reserve: Option<Reserve>,
    /// Empreinte des octets du fichier lu (voir que le fichier a changé), et du contenu (voir que le plan a changé).
    pub hash_fichier: u64,
    pub hash_contenu: u64,
    /// L'empreinte de chaque dessin telle que le fichier de plan la donne.
    pub dessins: BTreeMap<u64, u64>,
    /// Dessins cités que le dossier n'a pas donnés : le plan est « en attente ».
    pub manquants: BTreeSet<CelId>,
    /// Dessins vectoriels en rendu rapide : leur rendu exact se demande au rejeu.
    pub approx: Vec<CelId>,
}

impl PlanCharge {
    pub(crate) fn incomplet(&self) -> bool {
        !self.manquants.is_empty()
    }
}

/// Un plan relu à lui seul : c'est l'ouverture ordinaire (`load_manifest_avec`) d'un petit projet fait de ce plan, avec ses
/// protections (un dessin manquant ou abîmé est un avertissement). Sur un fil de fond.
pub(crate) fn charger_plan(dir: &Path, tete: &Tete, plan: u64, octets: &[u8], quick: bool) -> Result<PlanCharge, String> {
    let hash_fichier = empreinte::de_octets(octets);
    let fichier = PlanFichier::lire(octets)?;
    if fichier.plan != plan {
        return Err(format!("le fichier du plan {plan} dit être le plan {}", fichier.plan));
    }
    let scene = fichier.scene.max(1);
    // Les fichiers de dessins sont-ils ceux que le porteur a écrits ? Sinon le plan n'est pas encore complet ici.
    let pas_encore = fichier.contenu.dessins.iter().filter(|(id, h)| hash_dessin(dir, **id) != Some(**h)).count();
    if pas_encore > 0 {
        return Err(format!("{pas_encore} dessin(s) du plan ne sont pas encore arrivés"));
    }
    let dessins = fichier.contenu.dessins.clone();
    let mut panneau = fichier.contenu.panneau;
    panneau.id = plan;
    let versions = match fichier.contenu.versions {
        Some(v) => Some(crate::planvers::fichier_de(vec![v])?),
        None => None,
    };
    let reserves = match &fichier.contenu.rognage {
        Some(v) => Some(crate::rognage::parse(&v.to_string())?),
        None => None,
    };
    let manifest = manifest_de_tete(tete, vec![SequenceEntry { id: 1, nom: String::new(), numero: String::new(), scenes: vec![SceneEntry { id: scene, nom: String::new(), numero: String::new(), panneaux: vec![panneau] }] }], fichier.contenu.hors);
    let annexes = Annexes { fournies: true, plans_seuls: true, versions, reserves, ..Default::default() };
    let loaded = load_manifest_avec(manifest, dir, quick, None, annexes)?;
    let Loaded { mut doc, approx, manquants, .. } = loaded;
    let index = doc.panels.iter().position(|p| p.id == plan).ok_or("le plan n'a pas pu être lu")?;
    let data = DonneesPlan::de(&doc, index, &dessins);
    let hash_contenu = data.empreinte()?;
    let panel = doc.panels.swap_remove(index);
    Ok(PlanCharge {
        plan,
        scene,
        revision: fichier.revision,
        signature: fichier.enregistre_par,
        panel,
        versions: doc.versions.remove(&plan),
        reserve: doc.reserves.remove(&plan),
        cels: std::mem::take(&mut doc.cels),
        hash_fichier,
        hash_contenu,
        dessins,
        manquants,
        approx,
    })
}

fn manifest_de_tete(tete: &Tete, sequences: Vec<SequenceEntry>, hors: Vec<u64>) -> Manifest {
    Manifest {
        format: FORMAT.into(),
        version: VERSION,
        largeur: tete.largeur,
        hauteur: tete.hauteur,
        cadre: Some(tete.cadre),
        cadence: tete.cadence.max(1),
        papier: tete.papier,
        sequences,
        sons: Vec::new(),
        pistes_son: Vec::new(),
        guides: Vec::new(),
        planche: Vec::new(),
        planche_textes: Vec::new(),
        groupes: Vec::new(),
        image_de_depart: 1,
        fond: BackgroundEntry::default(),
        volume_general: 1.0,
        storyboard_verrouille: false,
        numerotation_verrouillee: false,
        proprietes: PropertiesEntry::default(),
        dessins_hors: hors,
        partage: None,
    }
}

impl Document {
    /// Pose un plan lu dans le document, à la place de celui qui a le même numéro (même rang, même scène). Les dessins
    /// reçoivent une version au-dessus de celle qu'ils avaient : les vignettes et les caches se refont. Rend le plan s'il
    /// n'est pas dans le document (la structure n'est pas encore arrivée).
    pub(crate) fn installer_plan(&mut self, pc: PlanCharge) -> Result<usize, Box<PlanCharge>> {
        let Some(i) = self.panels.iter().position(|p| p.id == pc.plan) else { return Err(Box::new(pc)) };
        let PlanCharge { plan, mut panel, cels, versions, reserve, .. } = pc;
        panel.scene = self.panels[i].scene;
        for (id, mut cel) in cels {
            if let Some(old) = self.cels.get(&id) {
                cel.version = old.version + 1;
            }
            self.cels.insert(id, cel);
        }
        for m in &panel.marks {
            self.reserve_id(m.id);
        }
        self.reserve_id(panel.id);
        self.panels[i] = panel;
        match versions {
            Some(v) => {
                for r in &v.autres {
                    self.reserve_id(r.panel.id);
                }
                self.versions.insert(plan, v);
            }
            None => {
                self.versions.remove(&plan);
            }
        }
        match reserve {
            Some(r) => {
                self.reserves.insert(plan, r);
            }
            None => {
                self.reserves.remove(&plan);
            }
        }
        Ok(i)
    }
}

/// La structure relue : un petit document dont les plans ne sont que des coquilles, à confronter au vrai.
pub(crate) struct StructureLue {
    pub doc: Document,
    pub bloc: BlocPartage,
}

/// Relit la structure d'un projet partagé (`octets` : le fichier de structure) sans toucher aux plans : chaque scène reçoit
/// une coquille. Les sons déjà connus ne sont pas décodés de nouveau. Sur un fil de fond.
pub(crate) fn charger_structure(dir: &Path, octets: &[u8], sons: &HashMap<u64, Sound>) -> Result<StructureLue, String> {
    let texte = std::str::from_utf8(octets).map_err(|_| "la structure n'est pas du texte (UTF-8)".to_string())?;
    let mut m: Manifest = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("structure illisible ({e})"))?;
    if m.format != FORMAT || m.version > crate::project::version_partage_ecrite() {
        return Err("ce n'est pas la structure d'un projet partagé de cette version".into());
    }
    let bloc = m.partage.take().ok_or("ce n'est pas un projet partagé")?;
    for seq in &mut m.sequences {
        for sc in &mut seq.scenes {
            let plan = bloc.plans.iter().find(|p| p.scene == sc.id).map(|p| p.plan).ok_or_else(|| format!("la scène {} n'a pas de plan", sc.id))?;
            sc.panneaux = vec![coquille(plan, m.cadence)];
        }
    }
    m.version = VERSION;
    m.dessins_hors = Vec::new();
    let annexes = Annexes { fournies: true, plans_seuls: false, sons_connus: sons.clone(), ..Default::default() };
    let loaded = load_manifest_avec(m, dir, true, None, annexes)?;
    Ok(StructureLue { doc: loaded.doc, bloc })
}

impl Document {
    /// Pose la structure d'un autre poste : séquences, ordre et noms des plans, pistes son, propriétés, annexes du montage.
    /// Le contenu des plans que je connais reste le mien (ils sont posés un par un, `installer_plan`) ; un plan que la
    /// structure cite et que je ne connais pas reste une coquille jusqu'à l'arrivée de son fichier. Rend les plans que la
    /// structure ne cite plus.
    pub(crate) fn adopter_structure(&mut self, stub: Document, en_attente: &mut HashMap<u64, PlanCharge>) -> Vec<u64> {
        let mut connus: HashMap<u64, Panel> = std::mem::take(&mut self.panels).into_iter().map(|p| (p.id, p)).collect();
        let mut panels = Vec::with_capacity(stub.panels.len());
        for sp in &stub.panels {
            match connus.remove(&sp.id) {
                Some(mut p) => {
                    p.scene = sp.scene;
                    panels.push(p);
                }
                None => panels.push(sp.clone()),
            }
        }
        let retires: Vec<u64> = connus.keys().copied().collect();
        for p in &retires {
            en_attente.remove(p);
        }
        let existe: std::collections::HashSet<u64> = panels.iter().map(|p| p.id).collect();
        let mut s = stub.structure();
        s.panels = panels;
        s.versions = std::mem::take(&mut self.versions).into_iter().filter(|(p, _)| existe.contains(p)).collect();
        s.reserves = std::mem::take(&mut self.reserves).into_iter().filter(|(p, _)| existe.contains(p)).collect();
        s.comments = std::mem::take(&mut self.comments).into_iter().filter(|c| existe.contains(&c.panel)).collect();
        let _ = self.replace_structure(s);
        self.variantes_partielles = stub.variantes_partielles;
        self.sons_proposes = stub.sons_proposes;
        self.envois = stub.envois;
        for (k, v) in stub.sounds {
            self.sounds.entry(k).or_insert(v);
        }
        retires
    }

    /// Range `panel` (le travail d'un autre, un brouillon, une copie en conflit) comme une version du plan `index`, avec une
    /// ligne en tête de ses notes qui dit d'où elle vient. Rend son numéro de version.
    pub(crate) fn ranger_comme_version(&mut self, index: usize, panel: Panel, cels: HashMap<CelId, Cel>, de: &str, now: u64) -> Option<u32> {
        let host = self.panels.get(index)?.id;
        if self.nombre_de_versions(host) >= crate::planvers::MAX_VERSIONS {
            return None;
        }
        // Les dessins de la version sont des copies à elle (modifier la version ne touche jamais le plan, ni l'inverse).
        for (id, cel) in cels {
            self.cels.entry(id).or_insert(cel);
        }
        let mut panel = self.copie_de(&panel);
        panel.scene = self.panels[index].scene;
        panel.notes = if panel.notes.is_empty() { format!("[{de}]") } else { format!("[{de}]\n{}", panel.notes) };
        let v = self.versions.entry(host).or_insert(VersionsDuPlan { actuelle: 1, date_actuelle: 0, prochaine: 2, autres: Vec::new() });
        let numero = v.prochaine;
        v.prochaine += 1;
        v.autres.push(crate::planvers::Rangee { numero, date: now, panel });
        v.autres.sort_by_key(|r| r.numero);
        Some(numero)
    }
}

// ---------------------------------------------------------------------------------- la structure

/// Ce que l'ouverture d'un projet partagé apprend, pour la session.
pub(crate) struct Ouverture {
    pub bloc: BlocPartage,
    pub tete: Tete,
    /// Nom du fichier de structure (`MonFilm.pelure`).
    pub nom_structure: String,
    /// L'état de chaque plan du film.
    pub plans: BTreeMap<u64, EtatPlan>,
    /// Empreinte de la structure telle que le document la ré-écrirait (la base pour voir qu'on l'a changée), et des octets lus.
    pub hash_structure: u64,
    pub hash_fichier_structure: u64,
    pub marque_structure: Option<depot::Marque>,
    /// Les enregistrements de commentaires de chaque auteur (`partagecom.rs`).
    pub commentaires: Vec<(String, Vec<Enregistrement>)>,
}

/// L'état d'un plan du film, tel que la dernière lecture l'a vu.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct EtatPlan {
    pub scene: u64,
    pub revision: u64,
    pub hash_contenu: u64,
    pub hash_fichier: u64,
    /// Le fichier du plan n'est pas (encore) lisible : le plan n'est qu'une coquille, il ne se prend ni ne s'enregistre.
    pub attente: Option<String>,
    /// Des dessins manquent : même effet.
    pub incomplet: bool,
    pub signature: Option<Signature>,
    /// La date et la taille du fichier quand l'ouverture l'a lu (le premier balayage ne signale que ce qui a changé depuis).
    pub marque: Option<depot::Marque>,
    /// L'empreinte du fichier de chaque dessin du plan, telle que son porteur l'a écrite (`ContenuPlan::dessins`).
    pub dessins: BTreeMap<u64, u64>,
}

/// La structure du document, sans plans et sans révision : sa forme « canonique », celle dont on prend l'empreinte.
pub(crate) fn structure_manifest(doc: &Document, projet: &str) -> Manifest {
    let mut m = manifest_de(doc, false, &BTreeSet::new(), &BTreeSet::new());
    m.version = crate::project::version_partage_ecrite();
    let mut plans = Vec::new();
    for s in &m.sequences {
        for sc in &s.scenes {
            if let Some(p) = doc.panels.iter().find(|p| p.scene == sc.id) {
                plans.push(PlanListe { scene: sc.id, plan: p.id });
            }
        }
    }
    m.partage = Some(BlocPartage { projet: projet.into(), revision: 0, enregistre_par: None, plans });
    m
}

/// Les octets du fichier de structure, avec sa révision et sa signature.
pub(crate) fn structure_octets(m: &mut Manifest, revision: u64, signature: Option<Signature>) -> Result<Vec<u8>, String> {
    if let Some(b) = m.partage.as_mut() {
        b.revision = revision;
        b.enregistre_par = signature;
    }
    serde_json::to_vec_pretty(&*m).map_err(|e| e.to_string())
}

/// L'empreinte de la structure : le manifeste sans plans et sans révision, plus les annexes du montage dont le document
/// répond (transitions, effets des sons, personnages). Ce qui changerait sans le jeton.
pub(crate) fn empreinte_structure(doc: &Document, projet: &str) -> Result<u64, String> {
    let mut octets = serde_json::to_vec(&structure_manifest(doc, projet)).map_err(|e| e.to_string())?;
    for annexe in [crate::transitions::to_bytes(doc)?, crate::audiofx::to_bytes(doc)?, crate::personnages::to_bytes(doc)?, crate::annotations::to_bytes(doc)?, crate::scenariogarde::to_bytes(doc)?] {
        octets.extend(annexe.unwrap_or_default());
    }
    Ok(empreinte::de_octets(&octets))
}

// ---------------------------------------------------------------------------------- ouvrir

/// Un plan lu à l'ouverture : le fichier, son empreinte, sa marque, et le nombre de ses dessins qui ne sont pas (encore) ceux
/// que le fichier annonce.
type LectureDePlan = Result<(PlanFichier, u64, Option<depot::Marque>, usize), String>;

/// Ouvre un projet partagé : lit la structure (déjà lue par l'appelant) et les fichiers de plans, les rassemble en un
/// manifeste ordinaire et le fait lire par `load_manifest_avec`. Un fichier de plan manquant ou illisible donne un plan
/// « en attente » (une coquille), jamais un plan vide qu'on pourrait écraser.
pub(super) fn ouvrir(mut structure: Manifest, dir: &Path, quick: bool, progress: Option<&Progress>) -> Result<Loaded, String> {
    let nom_structure = String::new();
    let mut bloc = structure.partage.take().ok_or("ce n'est pas un projet partagé")?;
    let tete = Tete { largeur: structure.largeur, hauteur: structure.hauteur, cadre: structure.cadre.unwrap_or([structure.largeur, structure.hauteur]), cadence: structure.cadence.max(1), papier: structure.papier };
    let mut avertissements: Vec<String> = Vec::new();
    // Les fichiers de plans, lus en parallèle.
    let a_lire: Vec<PlanListe> = bloc.plans.clone();
    let lus: Vec<LectureDePlan> = crate::parallele::par_map(&a_lire, LOAD_THREADS, |_, pl| {
        let chemin = dir.join(depot::PLANS).join(format!("{}.json", pl.plan));
        let marque = fs::metadata(&chemin).ok().map(|m| depot::marque_de(&m));
        let octets = fs::read(&chemin).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "le fichier du plan n'est pas encore arrivé".to_string() } else { format!("{} : {e}", chemin.display()) })?;
        let hash = empreinte::de_octets(&octets);
        let f = PlanFichier::lire(&octets)?;
        if f.plan != pl.plan {
            return Err(format!("le fichier dit être le plan {}", f.plan));
        }
        let pas_encore = f.contenu.dessins.iter().filter(|(id, h)| hash_dessin(dir, **id) != Some(**h)).count();
        Ok((f, hash, marque, pas_encore))
    });
    let mut par_scene: HashMap<u64, (u64, LectureDePlan)> = HashMap::new();
    for (pl, lu) in a_lire.iter().zip(lus) {
        if par_scene.contains_key(&pl.scene) {
            continue;
        }
        par_scene.insert(pl.scene, (pl.plan, lu));
    }
    let mut hors_total: BTreeSet<u64> = BTreeSet::new();
    let mut versions: Vec<crate::planvers::PlanFichier> = Vec::new();
    let mut reserves: Vec<crate::rognage::Loaded> = Vec::new();
    let mut etats: BTreeMap<u64, EtatPlan> = BTreeMap::new();
    let mut vus_plans: BTreeSet<u64> = BTreeSet::new();
    for seq in &mut structure.sequences {
        for sc in &mut seq.scenes {
            let Some((plan, lu)) = par_scene.remove(&sc.id) else {
                return Err(format!("la scène {} n'a pas de plan dans la structure du projet partagé", sc.id));
            };
            if !vus_plans.insert(plan) {
                return Err(format!("le plan {plan} est cité deux fois par la structure du projet partagé"));
            }
            match lu {
                Ok((f, hash, marque, pas_encore)) => {
                    let mut p = f.contenu.panneau;
                    p.id = plan;
                    hors_total.extend(f.contenu.hors.iter().copied());
                    if let Some(v) = f.contenu.versions {
                        versions.push(v);
                    }
                    if let Some(r) = &f.contenu.rognage {
                        match crate::rognage::parse(&r.to_string()) {
                            Ok(l) => reserves.push(l),
                            Err(e) => avertissements.push(format!("plan {plan} : contenu rogné illisible ({e})")),
                        }
                    }
                    if pas_encore > 0 {
                        avertissements.push(format!("plan {plan} : {pas_encore} dessin(s) pas encore arrivés (le plan n'est pas prêt)"));
                    }
                    etats.insert(plan, EtatPlan { scene: sc.id, revision: f.revision, hash_fichier: hash, signature: f.enregistre_par, marque, dessins: f.contenu.dessins, incomplet: pas_encore > 0, ..Default::default() });
                    sc.panneaux = vec![p];
                }
                Err(raison) => {
                    avertissements.push(format!("plan {plan} : {raison} (en attente)"));
                    sc.panneaux = vec![coquille(plan, tete.cadence)];
                    etats.insert(plan, EtatPlan { scene: sc.id, attente: Some(raison), ..Default::default() });
                }
            }
        }
    }
    // Les commentaires de chacun, fusionnés (les plans qui n'existent pas sont laissés de côté).
    let mut par_auteur: Vec<(String, Vec<Enregistrement>)> = Vec::new();
    if let Ok(entrees) = fs::read_dir(dir.join(depot::COMMENTAIRES)) {
        for e in entrees.flatten() {
            let nom = e.file_name().to_string_lossy().into_owned();
            if !nom.to_lowercase().ends_with(".json") || nom.ends_with(".tmp") {
                continue;
            }
            match fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|t| partagecom::lire(&t)) {
                Ok(v) => par_auteur.push((nom[..nom.len() - 5].to_string(), v)),
                Err(m) => avertissements.push(format!("commentaires/{nom} : {m} ; ses commentaires ne sont pas chargés.")),
            }
        }
    }
    par_auteur.sort_by(|a, b| a.0.cmp(&b.0));
    let commentaires: Vec<Comment> = partagecom::fusionner(&par_auteur).into_values().filter(|c| vus_plans.contains(&c.panel)).collect();
    let annexes = Annexes {
        fournies: true,
        plans_seuls: false,
        versions: if versions.is_empty() { None } else { Some(crate::planvers::fichier_de(versions)?) },
        reserves: if reserves.is_empty() { None } else { Some(crate::rognage::Loaded::fusion(reserves)) },
        commentaires,
        ..Default::default()
    };
    structure.version = VERSION;
    structure.dessins_hors = hors_total.into_iter().collect();
    let mut loaded = load_manifest_avec(structure, dir, quick, progress, annexes)?;
    loaded.warnings.extend(avertissements.iter().cloned());
    // L'état de chaque plan, tel que le document l'a lu : les empreintes se prennent sur le document, jamais sur le fichier.
    for (i, p) in loaded.doc.panels.iter().enumerate() {
        if let Some(e) = etats.get_mut(&p.id) {
            if e.attente.is_none() {
                e.hash_contenu = DonneesPlan::de(&loaded.doc, i, &e.dessins).empreinte()?;
                let cels = empreinte::dessins_du_plan(p);
                e.incomplet |= cels.iter().any(|c| loaded.manquants.contains(c));
            }
        }
    }
    let hash_structure = empreinte_structure(&loaded.doc, &bloc.projet)?;
    bloc.plans = a_lire;
    let nom = nom_structure;
    loaded.partage = Some(Ouverture { bloc, tete, nom_structure: nom, plans: etats, hash_structure, hash_fichier_structure: 0, marque_structure: None, commentaires: par_auteur });
    Ok(loaded)
}

/// Un plan dont le fichier n'est pas arrivé : une coquille d'une seconde, un calque.
fn coquille(plan: u64, cadence: u32) -> PanelEntry {
    serde_json::from_value(serde_json::json!({ "id": plan, "duree": cadence.max(1), "calques": [] })).expect("coquille de plan valide")
}

// ---------------------------------------------------------------------------------- enregistrer

/// Un plan que je tiens, à écrire s'il a changé.
#[derive(Clone)]
pub(crate) struct PlanTenu {
    pub plan: u64,
    pub scene: u64,
    /// La révision du fichier que j'ai lu (ou écrit) : l'écriture porte la suivante.
    pub revision: u64,
    /// L'empreinte du contenu à ce moment : un plan qui n'a pas changé n'est pas réécrit.
    pub hash: u64,
    /// L'empreinte du fichier de chaque dessin à ce moment (celles des dessins qu'on ne réécrit pas ne sont pas relues).
    pub dessins: BTreeMap<u64, u64>,
}

/// Du travail mis de côté (§6) : le fichier de plan, et les dessins neufs qu'il cite (déjà dans le document, sous des
/// numéros à moi : ils s'écrivent à côté des autres sans rien écraser).
pub(crate) struct Brouillon {
    pub nom: String,
    pub octets: Vec<u8>,
    pub cels: BTreeSet<CelId>,
}

/// Ce que la session dit à l'enregistrement.
pub(crate) struct ContexteEcriture {
    pub moi: Signature,
    pub projet: String,
    pub depot: Arc<dyn Depot>,
    pub tenus: Vec<PlanTenu>,
    /// Je tiens le jeton du montage ; la révision de la structure que j'ai lue.
    pub jeton: Option<u64>,
    /// Mon fichier de commentaires, s'il a changé.
    pub commentaires: Option<Vec<u8>>,
    /// Du travail à mettre de côté.
    pub brouillons: Vec<Brouillon>,
}

/// Le travail propre au partage dans un `SaveJob`.
pub(crate) struct TravailPartage {
    pub ctx: ContexteEcriture,
    pub plans: Vec<(PlanTenu, DonneesPlan)>,
    /// Le manifeste de structure (sans révision) si je tiens le jeton.
    pub structure: Option<Manifest>,
    /// Les dessins dont le fichier va être (ré)écrit par ce travail.
    pub reecrits: BTreeSet<CelId>,
}

/// Un plan tel que l'enregistrement l'a laissé (écrit, ou trouvé inchangé).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlanEcrit {
    pub plan: u64,
    pub revision: u64,
    pub hash_contenu: u64,
    pub hash_fichier: u64,
    pub dessins: BTreeMap<u64, u64>,
}

/// Ce que l'écriture a fait.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultatPartage {
    pub plans_ecrits: Vec<PlanEcrit>,
    /// Les plans tenus qui n'avaient pas changé.
    pub plans_inchanges: Vec<PlanEcrit>,
    /// La structure a été écrite : (révision, empreinte du fichier).
    pub structure: Option<(u64, u64)>,
    pub commentaires_ecrits: bool,
    pub brouillons: Vec<String>,
}

/// L'enregistrement d'un projet partagé : seulement ce que je tiens (§10).
pub(crate) fn preparer(doc: &Document, manifest_path: &Path, written: &mut HashMap<CelId, u64>, ctx: ContexteEcriture) -> Result<SaveJob, String> {
    let dir = manifest_path.parent().ok_or("chemin de projet invalide")?;
    let mut plans: Vec<(PlanTenu, DonneesPlan)> = Vec::new();
    for t in &ctx.tenus {
        if let Some(i) = doc.panels.iter().position(|p| p.id == t.plan) {
            plans.push((t.clone(), DonneesPlan::de(doc, i, &t.dessins)));
        }
    }
    let mut used: BTreeSet<CelId> = BTreeSet::new();
    for (_, d) in &plans {
        used.extend(d.cels());
    }
    // Les dessins neufs des brouillons s'écrivent avec les autres.
    for b in &ctx.brouillons {
        used.extend(b.cels.iter().copied());
    }
    // Les variantes rangées des séquences sont une annexe du montage (`variantes.rs`) : celui qui tient le jeton écrit aussi
    // leurs dessins (sauf si des dessins manquaient à la lecture : on n'y touche pas).
    let variantes = ctx.jeton.is_some() && !doc.variantes_partielles;
    if variantes {
        used.extend(doc.cels_des_variantes());
    }
    let (drawings, fonds, hors, reecrits) = dessins_a_ecrire(doc, dir, &used, written, None)?;
    let mut used_textures = used_textures(doc, &used);
    // (Et celles des pinceaux du dessin hors des cases : `horschamptex.rs`.)
    used_textures.extend(doc.annotations.textures_citees());
    let textures = used_textures
        .iter()
        .map(|&(kind, key)| (texture_file(dir, kind, key), kind, key))
        .filter(|(path, _, _)| !path.exists())
        .filter_map(|(path, kind, key)| crate::brushtex::shared_image(kind, key).map(|img| (path, img)))
        .collect();
    // Les images collées : écrites si elles manquent (jamais effacées ici : un autre équipier peut en citer, `effacer_inutiles`
    // ne passe pas par un enregistrement partagé).
    let used_images = used_images(doc, &used);
    let images = images_a_ecrire(dir, &used_images);
    let mut videos = Vec::new();
    for v in plans.iter().flat_map(|(_, d)| d.panel.layers.iter()).filter_map(|l| l.video.as_ref()) {
        let to = dir.join(VIDEOS).join(v.id.to_string());
        if let Some(from) = crate::video::dir_of(v.id).filter(|from| *from != to) {
            if !videos.iter().any(|(w, _, _): &(crate::video::VideoClip, PathBuf, PathBuf)| w.id == v.id) {
                videos.push((v.clone(), from, to));
            }
        }
    }
    let mut job = SaveJob::vide(manifest_path);
    job.drawings = drawings;
    job.textures = textures;
    job.images = images;
    job.videos = videos;
    job.fonds = fonds;
    job.hors = hors;
    job.used = used;
    job.used_textures = used_textures;
    let mut structure = None;
    if ctx.jeton.is_some() {
        // Je tiens le jeton : le montage s'écrit aussi (structure, sons, modèles, annexes du montage).
        structure = Some(structure_manifest(doc, &ctx.projet));
        let used_sounds = used_sounds(doc);
        job.sounds = used_sounds.iter().map(|id| &doc.sounds[id]).map(|s| (dir.join(SOUNDS).join(sound_file(s)), s.bytes.clone())).filter(|(path, _)| !path.exists()).collect();
        job.used_sounds = used_sounds;
        job.transitions = crate::transitions::to_bytes(doc)?;
        job.sound_fx = crate::audiofx::to_bytes(doc)?;
        job.personnages = crate::personnages::to_bytes(doc)?;
        job.annotations = crate::annotations::to_bytes(doc)?;
        job.scenario = crate::scenariogarde::to_bytes(doc)?;
        job.sons_proposes = crate::indications::to_bytes(doc)?;
        job.variantes = if variantes { variantes_octets(doc)? } else { None };
        job.variantes_gardees = doc.variantes_partielles;
        job.references = crate::references::plan_save(doc, dir)?;
        job.envois = crate::envois::to_bytes(doc)?;
        job.snapshots = doc.envois.pending.clone();
        job.snapshot_reuse = crate::envois::dessins_reutilisables(&job.snapshots, written, &|id| Some(drawing_path(dir, id, false)));
    }
    job.partage = Some(TravailPartage { ctx, plans, structure, reecrits });
    Ok(job)
}

impl SaveJob {
    /// Un travail sans rien dedans : le point de départ de l'enregistrement d'un projet partagé.
    pub(super) fn vide(manifest_path: &Path) -> SaveJob {
        SaveJob {
            manifest_path: manifest_path.to_path_buf(),
            drawings: Vec::new(),
            sounds: Vec::new(),
            textures: Vec::new(),
            images: Vec::new(),
            used_images: BTreeSet::new(),
            videos: Vec::new(),
            manifest: Vec::new(),
            has_unreadable: false,
            kept: BTreeSet::new(),
            used: BTreeSet::new(),
            fonds: BTreeSet::new(),
            hors: BTreeSet::new(),
            used_sounds: BTreeSet::new(),
            used_textures: BTreeSet::new(),
            comments: None,
            reserves: None,
            transitions: None,
            sound_fx: None,
            personnages: None,
            annotations: None,
            scenario: None,
            sons_proposes: None,
            references: Default::default(),
            envois: None,
            plans_versions: None,
            variantes: None,
            variantes_gardees: false,
            corbeille: None,
            froids_a_copier: None,
            keep_drawings: false,
            snapshots: Vec::new(),
            snapshot_reuse: HashMap::new(),
            versions_from: None,
            partage: None,
            differe: None,
            manquants: Vec::new(),
            ancien_manifeste: None,
        }
    }

    /// Écrit ce que je tiens, dans l'ordre : brouillons (le travail mis de côté d'abord), dessins, ressources, plans, montage,
    /// commentaires. Un fichier ne cite jamais ce qui n'est pas encore écrit. Aucun effacement.
    pub(super) fn run_partage(mut self, progress: Option<&Progress>, threads: usize) -> Result<SaveReport, String> {
        let dir = self.manifest_path.parent().ok_or("chemin de projet invalide")?.to_path_buf();
        let mut travail = self.partage.take().ok_or("pas de travail de partage")?;
        let depot = travail.ctx.depot.clone();
        let mut resultat = ResultatPartage::default();
        let mut count = 0;
        fs::create_dir_all(dir.join(DRAWINGS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        if let Some(p) = progress {
            p.total.store(self.drawings.len(), std::sync::atomic::Ordering::Relaxed);
        }
        let ecrits = crate::parallele::par_map(&self.drawings, threads, |_, (path, data)| -> Result<(), String> {
            let bytes: std::borrow::Cow<[u8]> = match data {
                DrawingData::Bytes(b) => std::borrow::Cow::Borrowed(b),
                DrawingData::Raster(r) => std::borrow::Cow::Owned(encode_png(&r.to_rgba(), r.width, r.height)?),
                DrawingData::Atlas(tiles) => {
                    let (atlas, _) = hors_atlas(tiles);
                    std::borrow::Cow::Owned(encode_png(&atlas.to_rgba(), atlas.width, atlas.height)?)
                }
                DrawingData::CopyFrom(from) => std::borrow::Cow::Owned(fs::read(from).map_err(|e| format!("{} : {e}", from.display()))?),
            };
            let r = write_atomic(path, &bytes);
            if let Some(p) = progress {
                p.done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            r
        });
        for r in ecrits {
            r?;
            count += 1;
        }
        // Le travail mis de côté (les dessins neufs qu'il cite viennent d'être écrits).
        for b in &travail.ctx.brouillons {
            depot.ecrire_brouillon(&b.nom, &b.octets)?;
            resultat.brouillons.push(b.nom.clone());
        }
        if !self.textures.is_empty() {
            fs::create_dir_all(dir.join(TEXTURES)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, img) in &self.textures {
            write_atomic(path, &texture_png(img)?)?;
            count += 1;
        }
        if !self.images.is_empty() {
            fs::create_dir_all(dir.join(crate::imagevec::DOSSIER)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, px) in &self.images {
            write_atomic(path, &crate::imagevec::png(px)?)?;
            count += 1;
        }
        for (clip, from, to) in &self.videos {
            count += crate::video::copy_frames(clip, from, to)?;
        }
        if !self.sounds.is_empty() {
            fs::create_dir_all(dir.join(SOUNDS)).map_err(|e| format!("{} : {e}", dir.display()))?;
        }
        for (path, bytes) in &self.sounds {
            write_atomic(path, bytes)?;
            count += 1;
        }
        // Les plans que je tiens : seulement ceux dont le contenu a changé.
        let reecrits = std::mem::take(&mut travail.reecrits);
        for (tenu, data) in &mut travail.plans {
            // L'empreinte de chaque dessin : celle qu'on connaissait pour ceux qu'on n'a pas réécrits, celle des fichiers
            // qu'on vient d'écrire pour les autres.
            let mut dessins = BTreeMap::new();
            for id in data.cels() {
                let connue = if reecrits.contains(&id) { None } else { tenu.dessins.get(&id).copied() };
                if let Some(h) = connue.or_else(|| hash_dessin(&dir, id)) {
                    dessins.insert(id, h);
                }
            }
            data.dessins = dessins;
            let hash = data.empreinte()?;
            if hash == tenu.hash {
                resultat.plans_inchanges.push(PlanEcrit { plan: tenu.plan, revision: tenu.revision, hash_contenu: hash, hash_fichier: 0, dessins: data.dessins.clone() });
                continue;
            }
            let revision = tenu.revision + 1;
            let fichier = data.fichier(tenu.scene, revision, Some(travail.ctx.moi.clone()))?;
            let octets = fichier.octets()?;
            depot.ecrire_plan(tenu.plan, &octets)?;
            resultat.plans_ecrits.push(PlanEcrit { plan: tenu.plan, revision, hash_contenu: hash, hash_fichier: empreinte::de_octets(&octets), dessins: data.dessins.clone() });
            count += 1;
        }
        // Le montage, si je tiens le jeton.
        if let (Some(manifest), Some(revision_lue)) = (travail.structure.as_mut(), travail.ctx.jeton) {
            let mut change = false;
            let mut ecrire = |path: PathBuf, bytes: Option<&[u8]>, lisible: &dyn Fn(&str) -> bool| -> Result<(), String> {
                let identique = match (bytes, fs::read(&path)) {
                    (Some(b), Ok(actuel)) => actuel == b,
                    (None, Err(e)) => e.kind() == std::io::ErrorKind::NotFound,
                    _ => false,
                };
                if !identique {
                    write_sidecar(&path, bytes, lisible)?;
                    change = true;
                }
                Ok(())
            };
            ecrire(dir.join(crate::transitions::FILE), self.transitions.as_deref(), &|t| crate::transitions::parse(t).is_ok())?;
            ecrire(dir.join(crate::audiofx::FILE), self.sound_fx.as_deref(), &|t| crate::audiofx::parse(t).is_ok())?;
            ecrire(dir.join(crate::personnages::FILE), self.personnages.as_deref(), &|t| crate::personnages::parse(t).is_ok())?;
            ecrire(dir.join(crate::annotations::FILE), self.annotations.as_deref(), &|t| crate::annotations::parse(t).is_ok())?;
            {
                // (Le scénario d'avant, s'il est d'un autre, passe à côté avant d'être remplacé.)
                let chemin = dir.join(crate::scenariogarde::FILE);
                let identique = match (self.scenario.as_deref(), fs::read(&chemin)) {
                    (Some(b), Ok(actuel)) => actuel == b,
                    (None, Err(e)) => e.kind() == std::io::ErrorKind::NotFound,
                    _ => false,
                };
                if !identique {
                    crate::scenariogarde::mettre_de_cote(&chemin, self.scenario.as_deref());
                }
            }
            ecrire(dir.join(crate::scenariogarde::FILE), self.scenario.as_deref(), &|t| crate::scenariogarde::parse(t).is_ok())?;
            ecrire(dir.join(crate::indications::FILE), self.sons_proposes.as_deref(), &|t| crate::indications::parse(t).is_ok())?;
            ecrire(dir.join(crate::envois::FILE), self.envois.as_deref(), &|t| crate::envois::parse(t).is_ok())?;
            // Les variantes rangées (leurs dessins viennent d'être écrits) : le fichier n'est touché que si elles ont été lues en entier.
            if !self.variantes_gardees {
                ecrire(dir.join(crate::variantes::FILE), self.variantes.as_deref(), &|t| crate::variantes::parse(t).is_ok())?;
            }
            count += self.references.write(&dir)?;
            count += crate::envois::ecrire(&dir, &self.snapshots, &self.snapshot_reuse, None)?;
            // La structure : ré-écrite si ses octets ont changé, ou si une annexe a changé (les autres postes ne surveillent que
            // ce fichier-là pour savoir que le montage a bougé).
            let courant = structure_octets(manifest, revision_lue, None)?;
            let ancien = depot.lire_structure()?.map(|f| f.octets);
            let pareil = ancien.as_deref().is_some_and(|a| structure_sans_signature(a) == structure_sans_signature(&courant));
            if !pareil || change {
                let revision = revision_lue + 1;
                let octets = structure_octets(manifest, revision, Some(travail.ctx.moi.clone()))?;
                depot.ecrire_structure(&octets)?;
                resultat.structure = Some((revision, empreinte::de_octets(&octets)));
                count += 1;
            }
        }
        if let Some(octets) = &travail.ctx.commentaires {
            depot.ecrire_commentaires(&travail.ctx.moi.id, octets)?;
            resultat.commentaires_ecrits = true;
        }
        Ok(SaveReport { written: count, removed: 0, snapshots: self.snapshots.iter().map(|s| s.id).collect(), partage: Some(resultat), manquants: Vec::new() })
    }
}

/// Les octets d'une structure sans sa révision ni sa signature (pour savoir si elle a vraiment changé).
fn structure_sans_signature(octets: &[u8]) -> Vec<u8> {
    let Ok(mut m) = serde_json::from_slice::<Manifest>(octets) else { return octets.to_vec() };
    if let Some(b) = m.partage.as_mut() {
        b.revision = 0;
        b.enregistre_par = None;
    }
    serde_json::to_vec(&m).unwrap_or_default()
}

// ---------------------------------------------------------------------------------- partager un projet

/// Ce qu'il faut pour écrire un projet partagé à partir d'un projet classique : préparé sur le fil de l'interface (copies
/// légères), joué sur un fil de fond.
pub(crate) struct PartageAEcrire {
    job: SaveJob,
    manifeste: PathBuf,
    plans: Vec<(u64, u64, DonneesPlan)>,
    commentaires: Option<Vec<u8>>,
    structure: Manifest,
    moi: Signature,
}

/// « Partager ce projet… », première moitié : le cliché de `doc`. Le projet de départ n'est pas touché ; dans le nouveau, tous les
/// plans et le jeton sont libres ; `moi` signe les fichiers et les commentaires.
pub(crate) fn preparer_partage(doc: &Document, manifeste: &Path, moi: &Signature, projet: &str) -> Result<PartageAEcrire, String> {
    let dir = manifeste.parent().ok_or("chemin de projet invalide")?;
    if manifeste.exists() || dir.join(depot::PLANS).exists() {
        return Err(format!("un projet existe déjà à cet endroit ({}). Choisis un autre nom.", dir.display()));
    }
    // Tout le projet à plat, avec ses dessins, sons, modèles, références, envois (un enregistrement ordinaire).
    let mut written = HashMap::new();
    let job = prepare_save(doc, manifeste, &mut written, None)?;
    let plans = doc.panels.iter().enumerate().map(|(i, p)| (p.id, p.scene, DonneesPlan::de(doc, i, &BTreeMap::new()))).collect();
    let commentaires = if doc.comments.is_empty() {
        None
    } else {
        let mut suivi = partagecom::Suivi::default();
        suivi.relever(&doc.comments, &|_| true, moi.le);
        Some(suivi.fichier(&moi.id)?)
    };
    Ok(PartageAEcrire { job, manifeste: manifeste.to_path_buf(), plans, commentaires, structure: structure_manifest(doc, projet), moi: moi.clone() })
}

impl PartageAEcrire {
    /// Seconde moitié : écrit le dossier du projet partagé. Sur un fil de fond.
    pub(crate) fn ecrire(self, progress: Option<&Progress>, threads: usize) -> Result<(), String> {
        let PartageAEcrire { job, manifeste, plans, commentaires, mut structure, moi } = self;
        let dir = manifeste.parent().ok_or("chemin de projet invalide")?.to_path_buf();
        job.run_with(progress, threads)?;
        // Un fichier par plan.
        for (id, scene, mut data) in plans {
            for c in data.cels() {
                if let Some(h) = hash_dessin(&dir, c) {
                    data.dessins.insert(c, h);
                }
            }
            let fichier = data.fichier(scene, 1, Some(moi.clone()))?;
            let chemin = dir.join(depot::PLANS).join(format!("{id}.json"));
            if let Some(d) = chemin.parent() {
                fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
            }
            write_atomic_synced(&chemin, &fichier.octets()?)?;
        }
        // Ce qui est maintenant dans les plans quitte les fichiers du montage de l'ancienne disposition (dossier neuf, écrit à l'instant).
        for nom in [crate::rognage::FILE, crate::planvers::FILE, crate::commentaires::FILE] {
            let _ = fs::remove_file(dir.join(nom));
        }
        // Les commentaires, au nom de celui qui partage.
        if let Some(octets) = commentaires {
            let chemin = dir.join(depot::COMMENTAIRES).join(format!("{}.json", moi.id));
            if let Some(d) = chemin.parent() {
                fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
            }
            write_atomic_synced(&chemin, &octets)?;
        }
        // La structure, en dernier : elle cite des fichiers de plans qui sont tous là.
        let octets = structure_octets(&mut structure, 1, Some(moi))?;
        write_atomic_synced(&manifeste, &octets)?;
        Ok(())
    }
}

/// « Partager ce projet… » en un temps (essais).
#[cfg(test)]
pub(crate) fn partager(doc: &Document, manifeste: &Path, moi: &Signature, projet: &str, progress: Option<&Progress>, threads: usize) -> Result<(), String> {
    preparer_partage(doc, manifeste, moi, projet)?.ecrire(progress, threads)
}
/// Un identifiant de film neuf.
pub(crate) fn nouveau_projet_id() -> String {
    format!("p-{}", crate::equipe::nouvel_id())
}
