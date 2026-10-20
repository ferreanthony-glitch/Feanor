//! Le dépôt : tout ce qui touche au stockage d'un film partagé (`docs/COOPERATION.md`, §2).
//!
//! La session (`cooperation.rs`) ne connaît ni les chemins ni `std::fs` : elle parle à un [`Depot`]. Aujourd'hui il n'y a
//! qu'une réalisation, [`DossierPartage`] (un dossier local, ou synchronisé par Dropbox / Drive, ou un partage réseau) ;
//! un `ServeurShot` ferait les mêmes appels par le réseau, avec une prise atomique côté serveur.
//!
//! Les règles d'un dossier synchronisé sont celles du §1 : rien n'est supposé présent, les fichiers à moitié arrivés sont
//! « illisibles pour l'instant », les copies en conflit sont repérées, jamais effacées.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::write_atomic_synced;
use crate::verrous::{Equipier, Fiche};

/// Dossier des plans, des fiches d'équipier, des commentaires, des membres et des brouillons (relatifs à la racine).
pub(crate) const PLANS: &str = "plans";
pub(crate) const FICHES: &str = "partage/equipe";
pub(crate) const MEMBRES: &str = "partage/membres";
pub(crate) const BROUILLONS: &str = "partage/brouillons";
pub(crate) const IMPORTES: &str = "partage/importes";
pub(crate) const COMMENTAIRES: &str = "commentaires";

/// La date et la taille d'un fichier : de quoi voir qu'il a changé sans le lire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marque {
    pub modifie_ms: u64,
    pub taille: u64,
}

/// Un fichier lu.
#[derive(Clone, Debug)]
pub struct Fichier {
    pub octets: Vec<u8>,
}

/// Ce que le balayage a vu changer depuis le dernier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Releve {
    /// Premier balayage : tout est nouveau.
    pub premier: bool,
    pub structure: bool,
    /// Les plans dont le fichier a changé ou apparu.
    pub plans: Vec<u64>,
    pub fiches: bool,
    pub commentaires: bool,
    pub membres: bool,
    pub brouillons: bool,
    /// Copies en conflit vues pour la première fois.
    pub copies: Vec<CopieEnConflit>,
}

/// D'où vient une copie en conflit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrigineCopie {
    Plan(u64),
    Structure,
    Fiche,
    Commentaires,
}

/// Un fichier que le client de synchronisation a fabriqué parce que deux postes ont écrit le même fichier
/// (« … (Copie en conflit de Léa 2026-10-20).json », « … (1).json », « …-POSTE.json »).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopieEnConflit {
    /// Relatif à la racine du projet.
    pub chemin: PathBuf,
    pub origine: OrigineCopie,
}

impl CopieEnConflit {
    pub fn nom(&self) -> String {
        self.chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// Le curseur d'un balayage : ce qu'on a vu la dernière fois.
#[derive(Clone, Debug, Default)]
pub struct Curseur {
    vus: HashMap<String, Marque>,
}

impl Curseur {
    /// Note un fichier que l'ouverture vient de lire (`plan:12`, `structure`) : le premier balayage ne le signale que s'il a
    /// changé depuis.
    pub fn poser(&mut self, cle: &str, marque: Marque) {
        self.vus.insert(cle.to_string(), marque);
    }
}

/// Un membre du film : qui a rejoint, sa couleur, son préfixe de numéros (§4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Membre {
    pub format: String,
    pub version: u32,
    pub equipier: Equipier,
    /// Quand il a rejoint (millisecondes depuis 1970).
    pub depuis: u64,
}

pub(crate) const FORMAT_MEMBRE: &str = "shot-membre";

impl Membre {
    pub fn neuf(equipier: Equipier, depuis: u64) -> Membre {
        Membre { format: FORMAT_MEMBRE.into(), version: 1, equipier, depuis }
    }

    pub fn lire(texte: &str) -> Result<Membre, String> {
        let m: Membre = serde_json::from_str(texte.trim_start_matches('\u{feff}')).map_err(|e| format!("membre illisible ({e})"))?;
        if m.format != FORMAT_MEMBRE {
            return Err("ce n'est pas un fichier de membre".into());
        }
        if m.version > 1 {
            return Err(format!("membre écrit par une version plus récente (format {})", m.version));
        }
        Ok(m)
    }
}

/// Le stockage d'un film partagé. Appelé **sur des fils de fond** (jamais sur celui de l'interface) ; bloquant.
pub trait Depot: Send + Sync {
    /// Pour les messages (« dossier partagé D:\Dropbox\MonFilm »).
    fn nom(&self) -> String;
    /// Le dossier local où sont les fichiers du projet (dessins compris) : celui que l'ouverture lit. Un dépôt réseau le
    /// remplirait (copie locale de travail).
    fn racine(&self) -> &Path;
    /// Un balayage peu coûteux de ce qui peut changer ; met le curseur à jour.
    fn surveiller(&self, curseur: &mut Curseur) -> Result<Releve, String>;

    fn lire_structure(&self) -> Result<Option<Fichier>, String>;
    fn ecrire_structure(&self, octets: &[u8]) -> Result<(), String>;
    fn lire_plan(&self, plan: u64) -> Result<Option<Fichier>, String>;
    fn ecrire_plan(&self, plan: u64, octets: &[u8]) -> Result<(), String>;

    /// Les fiches d'équipier lisibles (une fiche abîmée ou à moitié arrivée est laissée de côté) ; avec les copies en conflit
    /// des fiches ignorées.
    fn lire_fiches(&self) -> Result<Vec<Fiche>, String>;
    fn ecrire_fiche(&self, fiche: &Fiche) -> Result<(), String>;

    fn lire_membres(&self) -> Result<Vec<Membre>, String>;
    fn ecrire_membre(&self, membre: &Membre) -> Result<(), String>;

    /// Les fichiers de commentaires, un par auteur (copies en conflit comprises : leurs enregistrements se fusionnent).
    fn lire_commentaires(&self) -> Result<Vec<(String, Vec<u8>)>, String>;
    fn ecrire_commentaires(&self, equipier: &str, octets: &[u8]) -> Result<(), String>;

    /// Un brouillon : du travail mis de côté plutôt que perdu (§6). `nom` : un nom de fichier simple.
    fn ecrire_brouillon(&self, nom: &str, octets: &[u8]) -> Result<(), String>;
    fn lister_brouillons(&self) -> Result<Vec<String>, String>;
    fn lire_brouillon(&self, nom: &str) -> Result<Vec<u8>, String>;
    /// Déplace un brouillon importé dans `importes/` (jamais effacé).
    fn ranger_brouillon(&self, nom: &str) -> Result<(), String>;

    /// Lit une copie en conflit, et la range dans `importes/` une fois importée.
    fn lire_copie(&self, copie: &CopieEnConflit) -> Result<Vec<u8>, String>;
    fn ranger_copie(&self, copie: &CopieEnConflit) -> Result<(), String>;
}

// ---------------------------------------------------------------------------------- le dossier partagé

/// Un film partagé dans un dossier ordinaire (local, Dropbox, Drive, partage réseau).
pub struct DossierPartage {
    racine: PathBuf,
    /// Nom du fichier de structure (`MonFilm.pelure`).
    structure: String,
}

impl DossierPartage {
    /// `manifeste` : le fichier de structure (`…/MonFilm/MonFilm.pelure`).
    pub fn nouveau(manifeste: &Path) -> DossierPartage {
        DossierPartage {
            racine: manifeste.parent().map(Path::to_path_buf).unwrap_or_default(),
            structure: manifeste.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "projet.pelure".into()),
        }
    }

    fn plan_path(&self, plan: u64) -> PathBuf {
        self.racine.join(PLANS).join(format!("{plan}.json"))
    }

    fn lire(path: &Path) -> Result<Option<Fichier>, String> {
        match fs::read(path) {
            Ok(octets) => Ok(Some(Fichier { octets })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{} : {e}", path.display())),
        }
    }

    fn ecrire(path: &Path, octets: &[u8]) -> Result<(), String> {
        if let Some(d) = path.parent() {
            fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
        }
        // (Un client de synchronisation ou un antivirus tient parfois le fichier une fraction de seconde : le renommage final
        // échoue alors avec « accès refusé ». On réessaie quelques fois avant de dire que l'écriture a échoué.)
        let mut essai = 0;
        loop {
            match write_atomic_synced(path, octets) {
                Ok(()) => return Ok(()),
                Err(e) if essai >= 4 => return Err(e),
                Err(_) => {
                    essai += 1;
                    std::thread::sleep(std::time::Duration::from_millis(80 * essai));
                }
            }
        }
    }

    /// Les fichiers `*.json` d'un sous-dossier (sans les temporaires), avec leur marque.
    fn lister(&self, sous: &str) -> Vec<(String, Marque)> {
        let Ok(entrees) = fs::read_dir(self.racine.join(sous)) else { return Vec::new() };
        entrees
            .flatten()
            .filter_map(|e| {
                let nom = e.file_name().to_string_lossy().into_owned();
                if temporaire(&nom) || !nom.to_lowercase().ends_with(".json") {
                    return None;
                }
                let meta = e.metadata().ok()?;
                meta.is_file().then(|| (nom, marque_de(&meta)))
            })
            .collect()
    }
}

pub(crate) fn marque_de(m: &fs::Metadata) -> Marque {
    let modifie_ms = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
    Marque { modifie_ms, taille: m.len() }
}

/// Un fichier que SHOT (ou un client de synchronisation) est en train d'écrire.
fn temporaire(nom: &str) -> bool {
    let n = nom.to_lowercase();
    n.ends_with(".tmp") || n.ends_with(".part") || n.starts_with('~') || n.starts_with(".dropbox") || n.ends_with(".crdownload") || n == "desktop.ini"
}

/// Ce que le nom d'un fichier dit de lui.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Nom {
    /// Le nom que SHOT donne (un numéro de plan, l'identifiant d'un équipier…).
    Canonique(String),
    /// Un nom fabriqué par un client de synchronisation à partir de celui-ci.
    Copie(String),
    Autre,
}

/// Le nom d'un fichier de `plans/` : « 12.json » est le plan 12, « 12 (Copie en conflit de Léa).json » en est une copie.
pub(crate) fn nom_de_plan(fichier: &str) -> Nom {
    let Some(souche) = souche_json(fichier) else { return Nom::Autre };
    let chiffres: String = souche.chars().take_while(char::is_ascii_digit).collect();
    if chiffres.is_empty() || chiffres.len() > 19 {
        return Nom::Autre;
    }
    if chiffres.len() == souche.len() {
        Nom::Canonique(chiffres)
    } else {
        Nom::Copie(chiffres)
    }
}

/// Le nom d'un fichier d'un dossier d'équipiers (`<id>.json`, `<id>.<séance>.json`) : alphanumérique et points ; le reste
/// (espace, parenthèse) est une copie.
pub(crate) fn nom_d_equipier(fichier: &str) -> Nom {
    let Some(souche) = souche_json(fichier) else { return Nom::Autre };
    let base: String = souche.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.').collect();
    if base.is_empty() {
        return Nom::Autre;
    }
    if base.len() == souche.len() {
        Nom::Canonique(base)
    } else {
        Nom::Copie(base)
    }
}

fn souche_json(fichier: &str) -> Option<&str> {
    let bas = fichier.to_lowercase();
    bas.ends_with(".json").then(|| &fichier[..fichier.len() - 5])
}

/// Le fichier de structure fabriqué en double : « MonFilm (Copie en conflit …).pelure ».
pub(crate) fn est_copie_de_structure(fichier: &str, structure: &str) -> bool {
    let (Some(souche), Some(attendue)) = (fichier.strip_suffix(".pelure"), structure.strip_suffix(".pelure")) else { return false };
    souche != attendue && souche.starts_with(attendue) && souche[attendue.len()..].starts_with([' ', '(', '-', '_'])
}

impl Depot for DossierPartage {
    fn nom(&self) -> String {
        format!("dossier partagé {}", self.racine.display())
    }

    fn racine(&self) -> &Path {
        &self.racine
    }

    fn surveiller(&self, curseur: &mut Curseur) -> Result<Releve, String> {
        let mut releve = Releve { premier: curseur.vus.is_empty(), ..Default::default() };
        let mut vus: HashMap<String, Marque> = HashMap::new();
        let changer = |cle: String, marque: Marque, vus: &mut HashMap<String, Marque>, curseur: &Curseur| -> bool {
            let neuf = curseur.vus.get(&cle) != Some(&marque);
            vus.insert(cle, marque);
            neuf
        };
        // La structure.
        if let Ok(m) = fs::metadata(self.racine.join(&self.structure)) {
            if changer("structure".into(), marque_de(&m), &mut vus, curseur) {
                releve.structure = true;
            }
        }
        // Les copies de la structure, à côté d'elle.
        if let Ok(entrees) = fs::read_dir(&self.racine) {
            for e in entrees.flatten() {
                let nom = e.file_name().to_string_lossy().into_owned();
                if est_copie_de_structure(&nom, &self.structure) {
                    if let Ok(m) = e.metadata() {
                        if changer(format!("copie:{nom}"), marque_de(&m), &mut vus, curseur) {
                            releve.copies.push(CopieEnConflit { chemin: PathBuf::from(&nom), origine: OrigineCopie::Structure });
                        }
                    }
                }
            }
        }
        for (nom, marque) in self.lister(PLANS) {
            match nom_de_plan(&nom) {
                Nom::Canonique(n) => {
                    if changer(format!("plan:{n}"), marque, &mut vus, curseur) {
                        if let Ok(id) = n.parse() {
                            releve.plans.push(id);
                        }
                    }
                }
                Nom::Copie(n) => {
                    if changer(format!("copie:{PLANS}/{nom}"), marque, &mut vus, curseur) {
                        if let Ok(id) = n.parse() {
                            releve.copies.push(CopieEnConflit { chemin: Path::new(PLANS).join(&nom), origine: OrigineCopie::Plan(id) });
                        }
                    }
                }
                Nom::Autre => {}
            }
        }
        for (nom, marque) in self.lister(FICHES) {
            match nom_d_equipier(&nom) {
                Nom::Autre => {}
                Nom::Canonique(_) => releve.fiches |= changer(format!("fiche:{nom}"), marque, &mut vus, curseur),
                Nom::Copie(_) => {
                    if changer(format!("copie:{FICHES}/{nom}"), marque, &mut vus, curseur) {
                        releve.copies.push(CopieEnConflit { chemin: Path::new(FICHES).join(&nom), origine: OrigineCopie::Fiche });
                    }
                }
            }
        }
        for (nom, marque) in self.lister(COMMENTAIRES) {
            // (Les copies en conflit des commentaires se fusionnent comme les autres fichiers : rien à signaler.)
            releve.commentaires |= changer(format!("com:{nom}"), marque, &mut vus, curseur);
        }
        for (nom, marque) in self.lister(MEMBRES) {
            releve.membres |= changer(format!("membre:{nom}"), marque, &mut vus, curseur);
        }
        for (nom, marque) in self.lister(BROUILLONS) {
            releve.brouillons |= changer(format!("brouillon:{nom}"), marque, &mut vus, curseur);
        }
        // Un fichier disparu est un changement aussi (un plan retiré à la main, une fiche nettoyée).
        for cle in curseur.vus.keys() {
            if !vus.contains_key(cle) {
                match cle.split_once(':') {
                    Some(("fiche", _)) => releve.fiches = true,
                    Some(("com", _)) => releve.commentaires = true,
                    Some(("membre", _)) => releve.membres = true,
                    Some(("brouillon", _)) => releve.brouillons = true,
                    _ => {}
                }
            }
        }
        curseur.vus = vus;
        Ok(releve)
    }

    fn lire_structure(&self) -> Result<Option<Fichier>, String> {
        Self::lire(&self.racine.join(&self.structure))
    }

    fn ecrire_structure(&self, octets: &[u8]) -> Result<(), String> {
        Self::ecrire(&self.racine.join(&self.structure), octets)
    }

    fn lire_plan(&self, plan: u64) -> Result<Option<Fichier>, String> {
        Self::lire(&self.plan_path(plan))
    }

    fn ecrire_plan(&self, plan: u64, octets: &[u8]) -> Result<(), String> {
        Self::ecrire(&self.plan_path(plan), octets)
    }

    fn lire_fiches(&self) -> Result<Vec<Fiche>, String> {
        let mut fiches = Vec::new();
        for (nom, _) in self.lister(FICHES) {
            if !matches!(nom_d_equipier(&nom), Nom::Canonique(_)) {
                continue;
            }
            // (Une fiche à moitié arrivée ou abîmée est laissée de côté : on la relira au prochain balayage.)
            if let Ok(Some(f)) = Self::lire(&self.racine.join(FICHES).join(&nom)) {
                if let Some(fiche) = String::from_utf8(f.octets).ok().and_then(|t| Fiche::lire(&t).ok()) {
                    fiches.push(fiche);
                }
            }
        }
        Ok(fiches)
    }

    fn ecrire_fiche(&self, fiche: &Fiche) -> Result<(), String> {
        let nom = format!("{}.{}.json", fiche.equipier.id, fiche.seance);
        Self::ecrire(&self.racine.join(FICHES).join(nom), &fiche.octets()?)
    }

    fn lire_membres(&self) -> Result<Vec<Membre>, String> {
        let mut membres = Vec::new();
        for (nom, _) in self.lister(MEMBRES) {
            if !matches!(nom_d_equipier(&nom), Nom::Canonique(_)) {
                continue;
            }
            if let Ok(Some(f)) = Self::lire(&self.racine.join(MEMBRES).join(&nom)) {
                if let Some(m) = String::from_utf8(f.octets).ok().and_then(|t| Membre::lire(&t).ok()) {
                    membres.push(m);
                }
            }
        }
        Ok(membres)
    }

    fn ecrire_membre(&self, membre: &Membre) -> Result<(), String> {
        let octets = serde_json::to_vec_pretty(membre).map_err(|e| e.to_string())?;
        Self::ecrire(&self.racine.join(MEMBRES).join(format!("{}.json", membre.equipier.id)), &octets)
    }

    fn lire_commentaires(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut out = Vec::new();
        for (nom, _) in self.lister(COMMENTAIRES) {
            if let Ok(Some(f)) = Self::lire(&self.racine.join(COMMENTAIRES).join(&nom)) {
                out.push((nom[..nom.len() - 5].to_string(), f.octets));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    fn ecrire_commentaires(&self, equipier: &str, octets: &[u8]) -> Result<(), String> {
        Self::ecrire(&self.racine.join(COMMENTAIRES).join(format!("{equipier}.json")), octets)
    }

    fn ecrire_brouillon(&self, nom: &str, octets: &[u8]) -> Result<(), String> {
        let nom = nom_sur(nom)?;
        Self::ecrire(&self.racine.join(BROUILLONS).join(nom), octets)
    }

    fn lister_brouillons(&self) -> Result<Vec<String>, String> {
        let mut noms: Vec<String> = self.lister(BROUILLONS).into_iter().map(|(n, _)| n).collect();
        noms.sort();
        Ok(noms)
    }

    fn lire_brouillon(&self, nom: &str) -> Result<Vec<u8>, String> {
        let nom = nom_sur(nom)?;
        fs::read(self.racine.join(BROUILLONS).join(nom)).map_err(|e| format!("brouillon {nom} : {e}"))
    }

    fn ranger_brouillon(&self, nom: &str) -> Result<(), String> {
        let nom = nom_sur(nom)?;
        deplacer(&self.racine.join(BROUILLONS).join(nom), &self.racine.join(IMPORTES).join(nom))
    }

    fn lire_copie(&self, copie: &CopieEnConflit) -> Result<Vec<u8>, String> {
        fs::read(self.racine.join(sure(&copie.chemin)?)).map_err(|e| format!("{} : {e}", copie.chemin.display()))
    }

    fn ranger_copie(&self, copie: &CopieEnConflit) -> Result<(), String> {
        let nom = copie.chemin.file_name().ok_or("copie sans nom")?;
        deplacer(&self.racine.join(sure(&copie.chemin)?), &self.racine.join(IMPORTES).join(nom))
    }
}

/// Un nom de fichier simple : ni dossier, ni chemin qui sorte du projet.
fn nom_sur(nom: &str) -> Result<&str, String> {
    let ok = !nom.is_empty() && !nom.contains(['/', '\\', ':']) && nom != "." && nom != ".." && !nom.starts_with('.');
    ok.then_some(nom).ok_or_else(|| format!("nom de fichier refusé : {nom}"))
}

/// Un chemin relatif au projet qui n'en sort pas.
fn sure(chemin: &Path) -> Result<&Path, String> {
    let ok = chemin.is_relative() && chemin.components().all(|c| matches!(c, std::path::Component::Normal(_)));
    ok.then_some(chemin).ok_or_else(|| format!("chemin refusé : {}", chemin.display()))
}

/// Déplace un fichier sans jamais en écraser un autre (« nom (2).ext » si le nom est pris).
fn deplacer(de: &Path, vers: &Path) -> Result<(), String> {
    if let Some(d) = vers.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
    }
    let mut cible = vers.to_path_buf();
    let mut k = 1;
    while cible.exists() {
        k += 1;
        let souche = vers.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = vers.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
        cible = vers.with_file_name(format!("{souche} ({k}){ext}"));
    }
    fs::rename(de, &cible).map_err(|e| format!("{} : {e}", de.display()))
}

/// Le nombre de fichiers de plans qu'un projet partagé a dans `plans/` (pour les mesures et les essais).
#[cfg(test)]
pub(crate) fn nombre_de_plans(racine: &Path) -> usize {
    fs::read_dir(racine.join(PLANS)).map(|e| e.flatten().filter(|e| matches!(nom_de_plan(&e.file_name().to_string_lossy()), Nom::Canonique(_))).count()).unwrap_or(0)
}
