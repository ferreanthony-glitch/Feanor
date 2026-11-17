//! Les jalons : le projet tel qu'il était à un moment, pour y revenir (2026-11-10, proposition 25 d'Anthony, « On n'enregistre
//! plus »). FEANOR écrit le projet en continu (`continu.rs`) : fermer sans enregistrer n'existe plus, et ce qui le remplace
//! pour abandonner un travail raté, c'est le jalon.
//!
//! Un jalon est un dossier de projet complet, rangé dans le projet : `<projet>/jalons/<date>/`, avec sa fiche (`jalon.json` :
//! son nom, quand il a été posé, s'il est automatique). Il s'ouvre comme un projet, même par une version d'avant.
//!
//! **Il ne coûte presque pas de place.** Les dessins, les sons, les textures, les images collées et les images des vidéos ne
//! sont jamais modifiés sur place (`project::dossiers_jamais_modifies_sur_place`) : le jalon en garde un second nom sur le
//! disque (lien physique), pas une copie. Quand FEANOR réécrit un dessin, le fichier neuf prend sa place d'un renommage et le
//! jalon garde l'ancien. Le manifeste et les petits fichiers à côté de lui sont copiés. Sur un disque qui ne connaît pas les
//! liens (une clé en FAT), tout est copié.
//!
//! **Revenir à un jalon** (`restaurer`) remet les fichiers du projet comme ils étaient : chaque fichier prend sa place d'un
//! renommage, le manifeste en dernier, ce que le jalon n'a pas part à la fin. Une marque (`jalons/.retour`) dit qu'un retour est
//! en cours : s'il est coupé au milieu, l'ouverture suivante le finit (`finir_un_retour`). L'application pose d'abord un jalon
//! de l'état d'aujourd'hui (`jalonsui.rs`) : un retour se défait.
//!
//! Ce module ne connaît que des fichiers : aucun document, aucune interface.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project;

/// Le dossier des jalons, dans le dossier du projet.
pub(crate) const DOSSIER: &str = "jalons";
/// La fiche d'un jalon, dans son dossier.
const FICHE: &str = "jalon.json";
/// Le début du nom d'un jalon qui s'écrit encore (il prend son vrai nom d'un renommage, une fois complet).
const EN_COURS: &str = ".en-cours-";
/// La marque d'un retour en cours : le nom du dossier du jalon auquel on revient.
const RETOUR: &str = ".retour";
/// Combien de jalons automatiques un projet garde (les plus récents). Un jalon nommé à la main reste jusqu'à ce qu'on le supprime.
pub(crate) const GARDES_AUTO: usize = 10;
/// Combien de jalons posés à la main et laissés sans nom un projet garde (les plus récents) : Ctrl+S tous les quarts d'heure
/// pendant des mois ne remplit pas le disque.
pub(crate) const GARDES_SANS_NOM: usize = 30;
/// Le plus long nom qu'un jalon garde.
const NOM_MAX: usize = 80;

/// Ce qu'on sait d'un jalon.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Fiche {
    /// Son nom (vide : il se dit par son heure).
    #[serde(default)]
    pub nom: String,
    /// Quand il a été posé (secondes depuis 1970).
    #[serde(rename = "pose_le")]
    pub quand: u64,
    /// Posé par FEANOR (à l'ouverture, avant un retour) : seuls les plus récents sont gardés.
    #[serde(default, rename = "automatique")]
    pub auto: bool,
    /// L'instant où sa copie a fini (secondes et nanosecondes depuis 1970) : ce que le projet écrit après n'est pas dans ce jalon
    /// (`a_change_depuis_le_dernier`). À la précision des dates de fichiers, pas à la seconde.
    #[serde(default, rename = "fini_le", skip_serializing_if = "Option::is_none")]
    pub fini: Option<(u64, u32)>,
}

/// Un jalon sur le disque.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Jalon {
    pub dossier: PathBuf,
    pub fiche: Fiche,
}

impl Jalon {
    /// Essais : le manifeste du projet gardé dans ce jalon (le `.shot`, sinon l'ancien `.pelure`).
    #[cfg(test)]
    pub(crate) fn manifeste(&self) -> Option<PathBuf> {
        let mut trouves: Vec<PathBuf> = fs::read_dir(&self.dossier).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_file() && project::est_un_projet(p)).collect();
        trouves.sort_by_key(|p| project::est_ancien_manifeste(p));
        trouves.into_iter().next()
    }

    /// « Avant les retakes », ou « Jalon de 19:42 » pour un jalon sans nom.
    pub(crate) fn titre(&self) -> String {
        let nom = self.fiche.nom.trim();
        if nom.is_empty() {
            let (_, _, _, h, mi) = crate::remplacer::heure_locale(self.fiche.quand);
            format!("Jalon de {h:02}:{mi:02}")
        } else {
            nom.to_string()
        }
    }
}

/// Ce qu'un jalon a écrit : des liens (aucune place prise) et des copies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Compte {
    pub liens: usize,
    pub copies: usize,
}

/// `manifeste` est-il celui d'un jalon (`…/jalons/<date>/Film.shot`) ? Un jalon s'ouvre pour le regarder ; FEANOR n'y écrit jamais.
pub(crate) fn est_dans_un_jalon(manifeste: &Path) -> bool {
    manifeste.parent().and_then(Path::parent).and_then(Path::file_name).is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(DOSSIER))
}

/// « 2026-11-10_19-42-07 », à l'heure d'ici : le nom du dossier d'un jalon (ils se rangent tout seuls dans l'ordre).
fn identifiant(racine: &Path, maintenant: u64) -> String {
    let (a, m, j, h, mi) = crate::remplacer::heure_locale(maintenant);
    let base = format!("{a:04}-{m:02}-{j:02}_{h:02}-{mi:02}-{:02}", maintenant % 60);
    let mut nom = base.clone();
    let mut k = 1;
    while racine.join(&nom).exists() || racine.join(format!("{EN_COURS}{nom}")).exists() {
        k += 1;
        nom = format!("{base}_{k}");
    }
    nom
}

/// Donne à `vers` le contenu de `de` sans prendre de place (un second nom du même fichier) ; une copie si le disque ne sait pas.
fn lier_ou_copier(de: &Path, vers: &Path, compte: &mut Compte) -> Result<(), String> {
    if fs::hard_link(de, vers).is_ok() {
        compte.liens += 1;
        return Ok(());
    }
    fs::copy(de, vers).map_err(|e| format!("{} : {e}", vers.display()))?;
    compte.copies += 1;
    Ok(())
}

/// Un fichier qu'une écriture a laissé à moitié (`<nom>.tmp`, `project::write_atomic`) : jamais gardé, jamais remis.
fn est_un_reste(nom: &std::ffi::OsStr) -> bool {
    nom.to_string_lossy().ends_with(".tmp")
}

/// Recopie le dossier `de` dans `vers` (créé), sous-dossiers compris. `lier` : par des liens.
fn copier_dossier(de: &Path, vers: &Path, lier: bool, compte: &mut Compte) -> Result<(), String> {
    fs::create_dir_all(vers).map_err(|e| format!("{} : {e}", vers.display()))?;
    for e in fs::read_dir(de).map_err(|e| format!("{} : {e}", de.display()))?.flatten() {
        let (src, nom) = (e.path(), e.file_name());
        if est_un_reste(&nom) {
            continue;
        }
        let dst = vers.join(&nom);
        if src.is_dir() {
            copier_dossier(&src, &dst, lier, compte)?;
        } else if lier {
            lier_ou_copier(&src, &dst, compte)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{} : {e}", dst.display()))?;
            compte.copies += 1;
        }
    }
    Ok(())
}

/// Tout le projet de `projet` dans `vers`, sauf ses jalons.
fn copier_le_projet(projet: &Path, vers: &Path) -> Result<Compte, String> {
    let lies = project::dossiers_jamais_modifies_sur_place();
    let mut compte = Compte::default();
    for e in fs::read_dir(projet).map_err(|e| format!("{} : {e}", projet.display()))?.flatten() {
        let (src, nom) = (e.path(), e.file_name());
        let texte = nom.to_string_lossy();
        if texte.eq_ignore_ascii_case(DOSSIER) || est_un_reste(&nom) {
            continue;
        }
        let dst = vers.join(&nom);
        if src.is_dir() {
            copier_dossier(&src, &dst, lies.iter().any(|d| texte.eq_ignore_ascii_case(d)), &mut compte)?;
        } else {
            fs::copy(&src, &dst).map_err(|e| format!("{} : {e}", dst.display()))?;
            compte.copies += 1;
        }
    }
    Ok(compte)
}

/// Pose un jalon du projet `manifeste` tel qu'il est sur le disque. Aucune écriture du projet ne doit tourner pendant ce
/// temps (`jalonsui.rs` y veille). Le jalon n'apparaît qu'une fois complet.
pub(crate) fn poser(manifeste: &Path, nom: &str, auto: bool, maintenant: u64) -> Result<(Jalon, Compte), String> {
    let projet = manifeste.parent().ok_or("chemin de projet invalide")?;
    if !manifeste.is_file() {
        return Err(format!("{} n'est pas sur le disque", manifeste.display()));
    }
    let racine = projet.join(DOSSIER);
    fs::create_dir_all(&racine).map_err(|e| format!("{} : {e}", racine.display()))?;
    let id = identifiant(&racine, maintenant);
    let chantier = racine.join(format!("{EN_COURS}{id}"));
    let mut fiche = Fiche { nom: nom_propre(nom), quand: maintenant, auto, fini: None };
    let fait = (|| -> Result<Compte, String> {
        fs::create_dir_all(&chantier).map_err(|e| format!("{} : {e}", chantier.display()))?;
        let compte = copier_le_projet(projet, &chantier)?;
        fiche.fini = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| (d.as_secs(), d.subsec_nanos()));
        let texte = serde_json::to_vec_pretty(&fiche).map_err(|e| e.to_string())?;
        project::write_atomic_synced(&chantier.join(FICHE), &texte)?;
        Ok(compte)
    })();
    let compte = match fait {
        Ok(c) => c,
        Err(e) => {
            let _ = fs::remove_dir_all(&chantier);
            return Err(e);
        }
    };
    let dossier = racine.join(&id);
    fs::rename(&chantier, &dossier).map_err(|e| {
        let _ = fs::remove_dir_all(&chantier);
        format!("{} : {e}", dossier.display())
    })?;
    Ok((Jalon { dossier, fiche }, compte))
}

/// Un nom sur une ligne, sans blancs autour, de longueur raisonnable.
fn nom_propre(nom: &str) -> String {
    nom.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(NOM_MAX).collect()
}

fn lire_fiche(dossier: &Path) -> Option<Fiche> {
    serde_json::from_str(fs::read_to_string(dossier.join(FICHE)).ok()?.trim_start_matches('\u{feff}')).ok()
}

/// Les jalons du projet rangé dans `projet`, du plus récent au plus ancien. Un dossier sans fiche lisible (un jalon coupé au
/// milieu, un dossier mis là à la main) n'en est pas un.
pub(crate) fn lister(projet: &Path) -> Vec<Jalon> {
    let mut v: Vec<Jalon> = fs::read_dir(projet.join(DOSSIER))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .filter_map(|dossier| lire_fiche(&dossier).map(|fiche| Jalon { dossier, fiche }))
        .collect();
    v.sort_by(|a, b| (b.fiche.quand, &b.dossier).cmp(&(a.fiche.quand, &a.dossier)));
    v
}

/// Le projet a-t-il été écrit depuis son dernier jalon ? (Sans jalon, ou dans le doute : oui.) Toute écriture du projet remplace
/// un fichier d'un renommage : la date du manifeste, celle du dossier du projet ou celle d'un de ses dossiers de dessins, de
/// sons… a bougé. Ne sert qu'à ne pas empiler des jalons d'ouverture identiques : un retour à un jalon, lui, garde toujours
/// l'état d'aujourd'hui sans rien demander à cette fonction.
pub(crate) fn a_change_depuis_le_dernier(manifeste: &Path) -> bool {
    let Some(projet) = manifeste.parent() else { return false };
    let Some(dernier) = lister(projet).into_iter().next() else { return true };
    let Some((s, ns)) = dernier.fiche.fini else { return true };
    let fini = std::time::UNIX_EPOCH + std::time::Duration::new(s, ns);
    let date = |p: &Path| fs::metadata(p).ok().and_then(|m| m.modified().ok());
    let mut regardes = vec![manifeste.to_path_buf(), projet.to_path_buf()];
    regardes.extend(project::dossiers_jamais_modifies_sur_place().iter().map(|d| projet.join(d)));
    regardes.iter().filter_map(|p| date(p)).any(|t| t >= fini)
}

/// `dossier` est-il bien celui d'un jalon (dans un dossier `jalons`, avec sa fiche) ? Rien d'autre n'est jamais renommé ni effacé.
fn est_un_jalon(dossier: &Path) -> bool {
    dossier.parent().and_then(Path::file_name).is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(DOSSIER)) && dossier.join(FICHE).is_file()
}

/// Donne un nom au jalon. Un jalon nommé n'est plus automatique : il reste jusqu'à ce qu'on le supprime.
pub(crate) fn renommer(dossier: &Path, nom: &str) -> Result<Fiche, String> {
    if !est_un_jalon(dossier) {
        return Err(format!("{} n'est pas un jalon", dossier.display()));
    }
    let mut fiche = lire_fiche(dossier).ok_or_else(|| format!("{} : fiche illisible", dossier.display()))?;
    fiche.nom = nom_propre(nom);
    if !fiche.nom.is_empty() {
        fiche.auto = false;
    }
    let texte = serde_json::to_vec_pretty(&fiche).map_err(|e| e.to_string())?;
    project::write_atomic_synced(&dossier.join(FICHE), &texte)?;
    Ok(fiche)
}

/// Supprime le jalon (ce qu'il était seul à garder est rendu au disque).
pub(crate) fn supprimer(dossier: &Path) -> Result<(), String> {
    if !est_un_jalon(dossier) {
        return Err(format!("{} n'est pas un jalon", dossier.display()));
    }
    fs::remove_dir_all(dossier).map_err(|e| format!("{} : {e}", dossier.display()))
}

/// Ne garde que les `GARDES_AUTO` jalons automatiques et les `GARDES_SANS_NOM` jalons sans nom les plus récents (un jalon nommé
/// n'est jamais retiré), et retire les jalons coupés au milieu (`.en-cours-…`). Rend le nombre de jalons retirés.
pub(crate) fn elaguer(projet: &Path) -> usize {
    let mut retires = 0;
    let tous = lister(projet);
    let autos = tous.iter().filter(|j| j.fiche.auto).skip(GARDES_AUTO);
    let sans_nom = tous.iter().filter(|j| !j.fiche.auto && j.fiche.nom.trim().is_empty()).skip(GARDES_SANS_NOM);
    for j in autos.chain(sans_nom) {
        if supprimer(&j.dossier).is_ok() {
            retires += 1;
        }
    }
    for e in fs::read_dir(projet.join(DOSSIER)).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().starts_with(EN_COURS) {
            let _ = fs::remove_dir_all(e.path());
        }
    }
    retires
}

/// La taille et la date d'un fichier : un fichier du projet qui a celles du jalon est le même (un lien, ou une copie jamais
/// retouchée), il n'a pas à être remis.
fn empreinte(p: &Path) -> Option<(u64, std::time::SystemTime)> {
    let m = fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?))
}

/// Met `de` à la place de `vers` d'un seul geste (à côté, puis un renommage). `lier` : par un lien.
fn remettre_fichier(de: &Path, vers: &Path, lier: bool) -> Result<(), String> {
    if empreinte(de).is_some() && empreinte(de) == empreinte(vers) {
        return Ok(());
    }
    let mut tmp = vers.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let _ = fs::remove_file(&tmp);
    if !(lier && fs::hard_link(de, &tmp).is_ok()) {
        fs::copy(de, &tmp).map_err(|e| format!("{} : {e}", tmp.display()))?;
    }
    fs::rename(&tmp, vers).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("{} : {e}", vers.display())
    })
}

fn remettre_dossier(de: &Path, vers: &Path, lier: bool) -> Result<(), String> {
    fs::create_dir_all(vers).map_err(|e| format!("{} : {e}", vers.display()))?;
    for e in fs::read_dir(de).map_err(|e| format!("{} : {e}", de.display()))?.flatten() {
        let (src, dst) = (e.path(), vers.join(e.file_name()));
        if src.is_dir() {
            remettre_dossier(&src, &dst, lier)?;
        } else {
            remettre_fichier(&src, &dst, lier)?;
        }
    }
    Ok(())
}

/// Retire de `projet` ce que `jalon` n'a pas (au même endroit). Les dossiers vides restent.
fn retirer_le_surplus(projet: &Path, jalon: &Path, racine: bool) {
    for e in fs::read_dir(projet).into_iter().flatten().flatten() {
        let (p, nom) = (e.path(), e.file_name());
        if racine && nom.to_string_lossy().eq_ignore_ascii_case(DOSSIER) {
            continue;
        }
        let jumeau = jalon.join(&nom);
        if p.is_dir() {
            retirer_le_surplus(&p, &jumeau, false);
        } else if !jumeau.is_file() {
            let _ = fs::remove_file(&p);
        }
    }
}

fn remettre(projet: &Path, jalon: &Path) -> Result<(), String> {
    let lies = project::dossiers_jamais_modifies_sur_place();
    let entrees: Vec<PathBuf> = fs::read_dir(jalon).map_err(|e| format!("{} : {e}", jalon.display()))?.flatten().map(|e| e.path()).collect();
    let nom_de = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    // Les dossiers d'abord (les dessins, les sons…), puis les fichiers à côté du manifeste, puis le manifeste : à aucun moment
    // un manifeste ne cite un dessin qui n'est pas encore là.
    for dossier in entrees.iter().filter(|p| p.is_dir()) {
        let nom = nom_de(dossier);
        remettre_dossier(dossier, &projet.join(&nom), lies.iter().any(|d| nom.eq_ignore_ascii_case(d)))?;
    }
    let fichiers = || entrees.iter().filter(|p| p.is_file() && nom_de(p) != FICHE);
    for f in fichiers().filter(|p| !project::est_un_projet(p)) {
        remettre_fichier(f, &projet.join(nom_de(f)), false)?;
    }
    for f in fichiers().filter(|p| project::est_un_projet(p)) {
        remettre_fichier(f, &projet.join(nom_de(f)), false)?;
    }
    retirer_le_surplus(projet, jalon, true);
    Ok(())
}

/// Remet le projet rangé dans `projet` comme il était au jalon `jalon`. Le projet ne doit être ouvert nulle part pendant ce
/// temps. Coupé au milieu, le retour se finit à l'ouverture suivante (`finir_un_retour`).
pub(crate) fn restaurer(projet: &Path, jalon: &Path) -> Result<(), String> {
    if !est_un_jalon(jalon) || jalon.parent() != Some(projet.join(DOSSIER).as_path()) {
        return Err(format!("{} n'est pas un jalon de ce projet", jalon.display()));
    }
    let marque = projet.join(DOSSIER).join(RETOUR);
    let nom = jalon.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    project::write_atomic_synced(&marque, nom.as_bytes())?;
    remettre(projet, jalon)?;
    let _ = fs::remove_file(&marque);
    Ok(())
}

/// Un retour à un jalon a été coupé au milieu (plantage, courant) : il se finit maintenant. Rend le nom du jalon, ou rien s'il n'y
/// avait rien à finir. À appeler avant de lire le projet.
pub(crate) fn finir_un_retour(projet: &Path) -> Result<Option<String>, String> {
    let marque = projet.join(DOSSIER).join(RETOUR);
    let Ok(nom) = fs::read_to_string(&marque) else { return Ok(None) };
    let jalon = projet.join(DOSSIER).join(nom.trim());
    if !est_un_jalon(&jalon) {
        // (Le jalon n'est plus là : rien à finir, et rien à casser de plus.)
        let _ = fs::remove_file(&marque);
        return Ok(None);
    }
    let titre = lire_fiche(&jalon).map(|fiche| Jalon { dossier: jalon.clone(), fiche }.titre()).unwrap_or_default();
    remettre(projet, &jalon)?;
    let _ = fs::remove_file(&marque);
    Ok(Some(titre))
}
