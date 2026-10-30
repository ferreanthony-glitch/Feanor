//! Le paquet de livraison (pipeline, étape 1) : en un clic, dans un dossier choisi, une arborescence rangée selon un modèle de chemin, avec au choix une
//! vidéo par plan, les vignettes par plan, le PDF du storyboard, l'animatique complète, la liste des plans (CSV et/ou JSON), et toujours un
//! `manifest.json` (chaque fichier avec son empreinte SHA-256, la date, la version de SHOT, l'envoi courant).
//!
//! **Aucun code de rendu nouveau** : le paquet enchaîne les tâches d'export qui existent (`editorialjob` pour les vidéos par plan, `exportstills` pour les
//! vignettes, `ExportJob::pdf_layout`, `ExportJob::video_with_sound_alpha`), toutes en tâche de fond, sur **un seul cliché** du projet (pris par la
//! première, prêté aux suivantes).
//!
//! **Écriture atomique.** Tout s'écrit d'abord dans un dossier de travail caché (`.shot-paquet-…`) du dossier choisi. Quand tout est fini, un fil à part
//! calcule les empreintes puis **range** les fichiers à leur place (un renommage chacun) et écrit le manifeste en dernier ; si quelque chose échoue ou si
//! l'on arrête, ce qui était déjà rangé est retiré, les fichiers remplacés sont remis, et le dossier de travail est effacé : **rien ne reste**.
//!
//! **Modèle de chemin** : les jetons de `nommage.rs` (`{projet}`, `{episode}`, `{sequence}`, `{date}`), des `/` pour les dossiers. Un morceau du chemin qui
//! porte un jeton vide est laissé de côté (pas d'épisode : pas de dossier « EP »). Les vidéos et les vignettes vont dans le dossier de la séquence du plan ;
//! le PDF, l'animatique, la liste et le manifeste dans le dossier sans séquence. Le manifeste est écrit dans le dossier commun à tous les fichiers, ses chemins
//! lui sont relatifs.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::editorial::{self, Media};
use crate::editorialjob::EditorialJob;
use crate::export::{self, Encoders, ExportJob, Progress, SoundSpan};
use crate::exportstills::{self, Still, StillsPer, StillsSettings};
use crate::exportvideo::{self, VideoSize, VideoSpec};
use crate::listeplans;
use crate::nommage::{self, Jeton, Piece};

/// Le modèle de chemin d'origine.
pub const CHEMIN_DEFAUT: &str = "{projet}/EP{episode:02}/storyboard/SQ{sequence:02}";

/// Les modèles de chemin prêts.
pub const CHEMINS_PRETS: &[(&str, &str)] = &[
    ("Studio : projet, épisode, séquence", CHEMIN_DEFAUT),
    ("Projet et séquence", "{projet}/SQ{sequence:02}"),
    ("Tout dans le dossier choisi", "."),
];

pub const MANIFESTE: &str = "manifest.json";

/// Ce que le paquet contient, et où.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    pub videos: bool,
    pub vignettes: bool,
    pub pdf: bool,
    pub animatique: bool,
    pub liste_csv: bool,
    pub liste_json: bool,
    /// Les vidéos par plan en ProRes (sinon H.264, plus léger).
    pub prores: bool,
    pub modele_chemin: String,
    /// Remplacer les fichiers qui existent déjà (les anciens sont remis si le paquet échoue).
    pub remplacer: bool,
    /// Tout le film (sinon : la séquence courante).
    pub film_entier: bool,
    pub dossier: Option<PathBuf>,
}

impl Default for Reglages {
    fn default() -> Self {
        Reglages { videos: true, vignettes: true, pdf: true, animatique: true, liste_csv: true, liste_json: false, prores: false, modele_chemin: CHEMIN_DEFAUT.into(), remplacer: false, film_entier: true, dossier: None }
    }
}

impl Reglages {
    pub fn vide(&self) -> bool {
        !(self.videos || self.vignettes || self.pdf || self.animatique || self.liste_csv || self.liste_json)
    }

    pub fn a_besoin_de_ffmpeg(&self) -> bool {
        self.videos || self.animatique
    }

    /// Le modèle de chemin à utiliser (celui d'origine s'il est vide).
    pub fn chemin(&self) -> &str {
        if self.modele_chemin.trim().is_empty() {
            CHEMIN_DEFAUT
        } else {
            self.modele_chemin.trim()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sorte {
    Video,
    Vignette,
    Pdf,
    Animatique,
    Liste,
}

/// Un fichier produit dans le dossier de travail, et le nom qu'il aura.
#[derive(Clone, Debug)]
pub struct Prevu {
    pub sorte: Sorte,
    /// Le plan (rang du panneau) pour une vidéo ou une vignette ; rien pour les fichiers du film.
    pub panel: Option<usize>,
    pub nom: String,
    pub source: PathBuf,
}

// ----------------------------------------------------------------------------------------------- les chemins

/// Une partie d'un chemin sans caractère interdit ; rien si elle est vide ou ne désigne qu'un dossier de remontée.
fn segment_propre(s: &str) -> Option<String> {
    let t: String = s.trim().chars().map(|c| if nommage::caractere_interdit(c) { '_' } else { c }).collect();
    let t = t.trim_end_matches(['.', ' ']).trim().to_string();
    (!t.is_empty() && t != "." && t != "..").then_some(t)
}

/// Le dossier (relatif au dossier choisi) du modèle de chemin `modele` : pour le plan `panel`, ou, sans plan, sans séquence. Un morceau qui porte un jeton vide
/// (la séquence d'un fichier du film, un épisode non réglé) est laissé de côté.
pub fn dossier_de(doc: &Document, modele: &str, panel: Option<usize>) -> Result<PathBuf, String> {
    let mut cx = nommage::Contexte {
        projet: match doc.properties.serie.trim() {
            "" => doc.properties.title.trim().to_string(),
            s => s.to_string(),
        },
        episode: doc.properties.episode.clone(),
        date: nommage::date_du_jour(),
        ..Default::default()
    };
    if let Some(p) = panel.filter(|&p| p < doc.panels.len()) {
        cx.sequence_num = Some(doc.sequence_num(doc.sequence_of(p)));
        cx.plan_num = Some(doc.plan_num(doc.panels[p].scene));
        cx.version = doc.numero_de_version(doc.panels[p].id);
    }
    let mut out = PathBuf::new();
    for morceau in modele.split(['/', '\\']) {
        let pieces = nommage::analyser(morceau).map_err(|e| format!("modèle de chemin : {e}"))?;
        if morceau.trim().is_empty() || morceau.trim() == "." {
            continue;
        }
        // Les jetons permis dans un chemin : ceux du film, pas ceux d'un plan.
        for p in &pieces {
            if let Piece::Jeton { jeton: j @ (Jeton::Plan | Jeton::Version | Jeton::Case | Jeton::Calque), .. } = p {
                return Err(format!("modèle de chemin : « {{{}}} n'a pas de sens dans un dossier »", j.nom()));
            }
        }
        let vide = pieces.iter().any(|p| match p {
            Piece::Jeton { jeton: Jeton::Sequence, .. } => cx.sequence_num.is_none(),
            Piece::Jeton { jeton: Jeton::Projet, .. } => cx.projet.trim().is_empty(),
            Piece::Jeton { jeton: Jeton::Episode, .. } => cx.episode.trim().is_empty(),
            _ => false,
        });
        if vide {
            continue;
        }
        if let Some(s) = segment_propre(&nommage::remplir(&pieces, &cx)) {
            out.push(s);
        }
    }
    Ok(out)
}

/// Le nom des fichiers du film (PDF, animatique, liste) : `<projet>_EP02`.
pub fn base_du_film(doc: &Document) -> String {
    let projet = match doc.properties.serie.trim() {
        "" => doc.properties.title.trim(),
        s => s,
    };
    let projet = if projet.is_empty() { "SHOT" } else { projet };
    let ep = doc.properties.episode.trim();
    if ep.is_empty() {
        editorial::file_safe(projet)
    } else {
        format!("{}_EP{}", editorial::file_safe(projet), editorial::file_safe(ep))
    }
}

/// Où va chaque fichier : le chemin relatif au dossier choisi. Erreur si deux fichiers auraient le même chemin.
#[cfg_attr(not(test), allow(dead_code))]
pub fn rangement(doc: &Document, modele: &str, fichiers: &[Prevu]) -> Result<Vec<PathBuf>, String> {
    let global = dossier_de(doc, modele, None)?;
    let mut par_plan: HashMap<usize, PathBuf> = HashMap::new();
    for p in fichiers.iter().filter_map(|f| f.panel) {
        if !par_plan.contains_key(&p) {
            par_plan.insert(p, dossier_de(doc, modele, Some(p))?);
        }
    }
    rangement_avec(&global, &par_plan, fichiers)
}

/// Comme `rangement`, avec les dossiers déjà décidés.
pub fn rangement_avec(global: &Path, par_plan: &HashMap<usize, PathBuf>, fichiers: &[Prevu]) -> Result<Vec<PathBuf>, String> {
    let mut out: Vec<PathBuf> = Vec::with_capacity(fichiers.len());
    let mut vus: BTreeMap<String, usize> = BTreeMap::new();
    for f in fichiers {
        let dossier = match f.panel {
            Some(p) => par_plan.get(&p).cloned().unwrap_or_else(|| global.to_path_buf()),
            None => global.to_path_buf(),
        };
        let chemin = dossier.join(&f.nom);
        let cle = chemin.to_string_lossy().to_lowercase();
        *vus.entry(cle).or_insert(0) += 1;
        out.push(chemin);
    }
    if let Some((c, _)) = vus.iter().find(|(_, n)| **n > 1) {
        return Err(format!("Deux fichiers auraient le même chemin dans le paquet (« {c} ») : change le modèle de noms ou le modèle de chemin."));
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------ le manifeste

#[derive(Serialize)]
struct FichierManifeste {
    chemin: String,
    sorte: Sorte,
    octets: u64,
    sha256: String,
}

#[derive(Serialize)]
struct EnvoiManifeste {
    id: u64,
    nom: String,
    date: String,
}

#[derive(Serialize)]
struct Manifeste {
    format: &'static str,
    version_du_format: u32,
    shot: &'static str,
    date: String,
    projet: String,
    episode: String,
    cadence: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    envoi: Option<EnvoiManifeste>,
    fichiers: Vec<FichierManifeste>,
}

/// « 2026-10-30T14:05:09Z ».
pub fn horodatage(secs: u64) -> String {
    let (h, m, s) = ((secs % 86_400) / 3600, (secs % 3600) / 60, secs % 60);
    format!("{}T{h:02}:{m:02}:{s:02}Z", crate::incrustation::date_text(secs))
}

/// Ce que le manifeste sait du projet, avant d'avoir les fichiers.
#[derive(Clone)]
struct EnTete {
    projet: String,
    episode: String,
    cadence: u32,
    envoi: Option<(u64, String, u64)>,
}

fn en_tete(doc: &Document) -> EnTete {
    EnTete {
        projet: match doc.properties.serie.trim() {
            "" => doc.properties.title.trim().to_string(),
            s => s.to_string(),
        },
        episode: doc.properties.episode.trim().to_string(),
        cadence: doc.fps,
        envoi: doc.envois.dernier().map(|e| (e.id, e.nom.clone(), e.date)),
    }
}

/// Le dossier commun à tous les chemins (le manifeste y est écrit).
pub fn racine_commune(chemins: &[PathBuf]) -> PathBuf {
    let mut it = chemins.iter().map(|c| c.parent().map(Path::to_path_buf).unwrap_or_default());
    let Some(mut commun) = it.next() else { return PathBuf::new() };
    for d in it {
        while !d.starts_with(&commun) {
            if !commun.pop() {
                break;
            }
        }
    }
    commun
}

// ---------------------------------------------------------------------------------------------- le compte rendu

#[derive(Clone, Debug)]
pub struct Rapport {
    /// Le dossier choisi.
    pub dossier: PathBuf,
    /// Le dossier du manifeste.
    pub racine: PathBuf,
    pub manifeste: PathBuf,
    pub fichiers: usize,
    pub octets: u64,
}

// ------------------------------------------------------------------------------ le rangement, sur un fil à part

struct Rangement {
    /// (source dans le dossier de travail, chemin relatif final, sorte).
    items: Vec<(PathBuf, PathBuf, Sorte)>,
    dossier: PathBuf,
    staging: PathBuf,
    entete: EnTete,
    remplacer: bool,
}

/// Les empreintes, le rangement et le manifeste. S'arrête (et défait tout) si `annule` est levé ou si quelque chose échoue.
fn ranger(r: Rangement, annule: &AtomicBool, fait: &AtomicUsize) -> Result<Rapport, String> {
    // 1. Rien ne doit être écrasé sans l'avoir demandé.
    if !r.remplacer {
        let pris: Vec<String> = r.items.iter().filter(|(_, rel, _)| r.dossier.join(rel).exists()).map(|(_, rel, _)| rel.display().to_string()).collect();
        if !pris.is_empty() {
            return Err(format!("{} fichier(s) existent déjà dans le dossier choisi (par exemple « {} ») : coche « Remplacer les fichiers qui existent » ou choisis un autre dossier.", pris.len(), pris[0]));
        }
    }
    // 2. Les empreintes (lues dans le dossier de travail : rien n'est encore à sa place).
    let mut fichiers: Vec<FichierManifeste> = Vec::with_capacity(r.items.len());
    let mut octets = 0u64;
    for (src, rel, sorte) in &r.items {
        let Some(h) = crate::sha256::du_fichier(src, Some(annule))? else { return Err("Export annulé.".into()) };
        let n = fs::metadata(src).map_err(|e| format!("{} : {e}", src.display()))?.len();
        octets += n;
        fichiers.push(FichierManifeste { chemin: String::new(), sorte: *sorte, octets: n, sha256: h });
        let _ = rel;
        fait.fetch_add(1, Ordering::Relaxed);
    }
    // 3. Le rangement : un renommage par fichier, avec de quoi tout défaire.
    let avant = r.staging.join("avant");
    let mut deplaces: Vec<PathBuf> = Vec::new();
    let mut remplaces: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut dossiers_faits: Vec<PathBuf> = Vec::new();
    let racine_rel = racine_commune(&r.items.iter().map(|(_, rel, _)| rel.clone()).collect::<Vec<_>>());
    let manifeste = r.dossier.join(&racine_rel).join(MANIFESTE);
    let defaire = |deplaces: &[PathBuf], remplaces: &[(PathBuf, PathBuf)], dossiers: &[PathBuf]| {
        for d in deplaces.iter().rev() {
            let _ = fs::remove_file(d);
        }
        for (ancien, rangé) in remplaces.iter().rev() {
            let _ = fs::rename(rangé, ancien);
        }
        for d in dossiers.iter().rev() {
            let _ = fs::remove_dir(d);
        }
    };
    let travail = (|| -> Result<(), String> {
        if annule.load(Ordering::Relaxed) {
            return Err("Export annulé.".into());
        }
        for (k, (src, rel, _)) in r.items.iter().enumerate() {
            if annule.load(Ordering::Relaxed) {
                return Err("Export annulé.".into());
            }
            let dest = r.dossier.join(rel);
            if let Some(parent) = dest.parent() {
                let mut a_creer: Vec<PathBuf> = Vec::new();
                let mut p = parent.to_path_buf();
                while !p.exists() {
                    a_creer.push(p.clone());
                    if !p.pop() {
                        break;
                    }
                }
                fs::create_dir_all(parent).map_err(|e| format!("{} : {e}", parent.display()))?;
                dossiers_faits.extend(a_creer.into_iter().rev());
            }
            if dest.exists() {
                fs::create_dir_all(&avant).map_err(|e| format!("{} : {e}", avant.display()))?;
                let garde = avant.join(format!("{k}"));
                fs::rename(&dest, &garde).map_err(|e| format!("{} : {e}", dest.display()))?;
                remplaces.push((dest.clone(), garde));
            }
            fs::rename(src, &dest).map_err(|e| format!("{} : {e}", dest.display()))?;
            deplaces.push(dest);
        }
        let mut manif_fichiers = Vec::with_capacity(fichiers.len());
        for (f, (_, rel, _)) in fichiers.drain(..).zip(&r.items) {
            let relatif = rel.strip_prefix(&racine_rel).unwrap_or(rel);
            manif_fichiers.push(FichierManifeste { chemin: relatif.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"), ..f });
        }
        let secs = crate::incrustation::now_secs();
        let m = Manifeste {
            format: "shot-paquet",
            version_du_format: 1,
            shot: env!("CARGO_PKG_VERSION"),
            date: horodatage(secs),
            projet: r.entete.projet.clone(),
            episode: r.entete.episode.clone(),
            cadence: r.entete.cadence,
            envoi: r.entete.envoi.clone().map(|(id, nom, date)| EnvoiManifeste { id, nom, date: crate::incrustation::date_text(date) }),
            fichiers: manif_fichiers,
        };
        let texte = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())? + "\n";
        if manifeste.exists() {
            if !r.remplacer {
                return Err(format!("{} existe déjà.", manifeste.display()));
            }
            fs::create_dir_all(&avant).map_err(|e| format!("{} : {e}", avant.display()))?;
            let garde = avant.join("manifeste");
            fs::rename(&manifeste, &garde).map_err(|e| format!("{} : {e}", manifeste.display()))?;
            remplaces.push((manifeste.clone(), garde));
        }
        if let Some(p) = manifeste.parent() {
            fs::create_dir_all(p).map_err(|e| format!("{} : {e}", p.display()))?;
        }
        export::write_atomic(&manifeste, texte.as_bytes())?;
        deplaces.push(manifeste.clone());
        Ok(())
    })();
    if let Err(e) = travail {
        defaire(&deplaces, &remplaces, &dossiers_faits);
        return Err(e);
    }
    Ok(Rapport { dossier: r.dossier.clone(), racine: r.dossier.join(&racine_rel), manifeste, fichiers: r.items.len() + 1, octets })
}

// ------------------------------------------------------------------------------------------------- la tâche

/// Ce qui est décidé au départ, d'après le projet : de quoi lancer chaque étape sans relire le projet.
struct Preparation {
    plan_videos: Option<(editorial::Plan, editorial::Options)>,
    vignettes: Vec<Still>,
    frame: [usize; 2],
    pdf: Option<(String, Vec<(usize, usize)>, Vec<crate::pdf::Board>)>,
    pdf_layout: crate::pdfmise::PdfLayout,
    animatique: Option<(Vec<(usize, usize)>, SoundSpan, bool)>,
    fps: u32,
    listes: Vec<(String, String)>,
    base: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Etape {
    Videos,
    Vignettes,
    Pdf,
    Animatique,
    Listes,
}

impl Etape {
    fn nom(self) -> &'static str {
        match self {
            Etape::Videos => "vidéos par plan",
            Etape::Vignettes => "vignettes",
            Etape::Pdf => "PDF du storyboard",
            Etape::Animatique => "animatique",
            Etape::Listes => "liste des plans",
        }
    }
}

enum Courant {
    Montage(EditorialJob),
    Export(ExportJob, Etape),
}

enum Etat {
    Etapes,
    Rangement { handle: Option<JoinHandle<Result<Rapport, String>>>, fait: Arc<AtomicUsize>, total: usize, annule: Arc<AtomicBool> },
    Fini,
}

pub struct PaquetJob {
    dossier: PathBuf,
    staging: PathBuf,
    reglages: Reglages,
    prep: Preparation,
    etapes: VecDeque<Etape>,
    total_etapes: usize,
    courant: Option<(Courant, Etape)>,
    snapshot: Option<Arc<Document>>,
    prevus: Vec<Prevu>,
    ffmpeg: Option<(PathBuf, Encoders)>,
    exact: Vec<crate::document::CelId>,
    entete: EnTete,
    global: PathBuf,
    dossiers: HashMap<usize, PathBuf>,
    etat: Etat,
    pub started: Instant,
    pub outcome: Option<Result<Rapport, String>>,
}

static NUMERO: AtomicUsize = AtomicUsize::new(0);

impl PaquetJob {
    /// Prépare le paquet de `plans` (rangs de panneaux, d'un seul tenant : le film ou une séquence) dans `dossier`. `cur_panel` : le plan courant (pour les
    /// vidéos par plan). Rien ne s'écrit avant le premier `step`, sauf le dossier de travail.
    pub fn start(doc: &Document, r: &Reglages, cur_panel: usize, ffmpeg: Option<(PathBuf, Encoders)>, dossier: &Path, exact: Vec<crate::document::CelId>) -> Result<PaquetJob, String> {
        if r.vide() {
            return Err("Coche au moins un contenu.".into());
        }
        if doc.panels.is_empty() {
            return Err("Le projet n'a aucun plan.".into());
        }
        if r.a_besoin_de_ffmpeg() && ffmpeg.is_none() {
            return Err("ffmpeg est introuvable ou inutilisable : c'est lui qui fabrique les vidéos (décoche les vidéos et l'animatique, ou indique où il est).".into());
        }
        if !dossier.is_dir() {
            return Err(format!("{} n'est pas un dossier.", dossier.display()));
        }
        let n = doc.panels.len();
        let cur = cur_panel.min(n - 1);
        let (a, b) = if r.film_entier { (0, n) } else { doc.sequence_run(cur) };
        let plans: Vec<usize> = (a..b).collect();
        let base = base_du_film(doc);
        let title = match doc.properties.title.trim() {
            "" => base.clone(),
            t => t.to_string(),
        };
        let mut etapes = VecDeque::new();
        let mut prep = Preparation { plan_videos: None, vignettes: Vec::new(), frame: doc.frame, pdf: None, pdf_layout: Default::default(), animatique: None, fps: doc.fps, listes: Vec::new(), base: base.clone() };
        if r.videos {
            let (ff, enc) = ffmpeg.as_ref().ok_or("ffmpeg manquant")?;
            let _ = ff;
            let media = if r.prores && enc.prores { Media::ProRes } else { Media::H264 };
            if media == Media::H264 && enc.h264.is_none() {
                return Err("Ce ffmpeg ne sait pas encoder en H.264.".into());
            }
            let opts = editorial::Options { edl: false, xml: false, otio: false, fcpxml: false, markers: false, sound: false, media, whole_film: r.film_entier, ..editorial::Options::default() };
            let plan = editorial::build_plan(doc, &opts, cur, &title)?;
            prep.plan_videos = Some((plan, opts));
            etapes.push_back(Etape::Videos);
        }
        if r.vignettes {
            prep.vignettes = exportstills::items(doc, &plans, StillsPer::Plan);
            etapes.push_back(Etape::Vignettes);
        }
        if r.pdf {
            let (shots, boards) = export::storyboard_of_with(doc, &plans, false);
            prep.pdf = Some((base.clone(), shots, boards));
            etapes.push_back(Etape::Pdf);
        }
        if r.animatique {
            let frames: Vec<(usize, usize)> = plans.iter().flat_map(|&p| (0..doc.panels[p].frame_count()).map(move |f| (p, f))).collect();
            let (start, len) = export::sample_span(doc.fps, doc.panel_start(a), frames.len());
            let sound = doc.audio_tracks.iter().any(|t| !t.clips.is_empty());
            prep.animatique = Some((frames, SoundSpan { start, len }, sound));
            etapes.push_back(Etape::Animatique);
        }
        if r.liste_csv || r.liste_json {
            let lr = listeplans::Reglages::default();
            let lignes = listeplans::construire(doc, &plans, lr.depart, None, false);
            if r.liste_csv {
                prep.listes.push((format!("{base}_plans.csv"), listeplans::csv(&lignes, lr.profil, lr.separateur)));
            }
            if r.liste_json {
                prep.listes.push((format!("{base}_plans.json"), listeplans::json(doc, &lignes, lr.profil, lr.depart)));
            }
            etapes.push_back(Etape::Listes);
        }
        // Les dossiers de chaque plan et du film : décidés maintenant, d'après le projet tel qu'il est au départ.
        let global = dossier_de(doc, r.chemin(), None)?;
        let mut dossiers = HashMap::new();
        for &p in &plans {
            dossiers.insert(p, dossier_de(doc, r.chemin(), Some(p))?);
        }
        let staging = dossier.join(format!(".shot-paquet-{}-{}", std::process::id(), NUMERO.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&staging).map_err(|e| format!("{} : {e}", staging.display()))?;
        let total_etapes = etapes.len();
        Ok(PaquetJob {
            dossier: dossier.to_path_buf(),
            staging,
            reglages: r.clone(),
            prep,
            etapes,
            total_etapes,
            courant: None,
            snapshot: None,
            prevus: Vec::new(),
            ffmpeg,
            exact,
            entete: en_tete(doc),
            global,
            dossiers,
            etat: Etat::Etapes,
            started: Instant::now(),
            outcome: None,
        })
    }

    pub fn running(&self) -> bool {
        self.outcome.is_none()
    }

    /// Le paquet copie encore le projet (les premières images d'écran).
    pub fn needs_project(&self) -> bool {
        self.outcome.is_none()
            && self.snapshot.is_none()
            && match &self.courant {
                Some((Courant::Montage(j), _)) => j.needs_project(),
                Some((Courant::Export(j, _), _)) => j.needs_project(),
                None => matches!(self.etat, Etat::Etapes) && !self.etapes.is_empty(),
            }
    }

    /// Où en est le paquet : une phrase et une part de 0 à 1.
    pub fn status(&self) -> (String, f32) {
        match &self.etat {
            Etat::Rangement { fait, total, .. } => (format!("Empreintes et rangement : {} fichier(s) sur {total}", fait.load(Ordering::Relaxed)), 0.97),
            Etat::Fini => ("Terminé".into(), 1.0),
            Etat::Etapes => {
                let fait = self.total_etapes - self.etapes.len() - usize::from(self.courant.is_some());
                let (nom, part) = match &self.courant {
                    Some((Courant::Montage(j), e)) => {
                        let (d, t) = j.frames();
                        (e.nom(), d as f32 / t.max(1) as f32)
                    }
                    Some((Courant::Export(j, e), _)) => match j.progress() {
                        Progress::Running(d, t) => (e.nom(), d as f32 / t.max(1) as f32),
                        _ => (e.nom(), 0.99),
                    },
                    None => ("préparation", 0.0),
                };
                let tot = self.total_etapes.max(1) as f32;
                (format!("Étape {} sur {} : {nom}", fait + 1, self.total_etapes), (fait as f32 + part) / tot * 0.95)
            }
        }
    }

    fn fail(&mut self, e: String) {
        self.abandonner();
        self.etat = Etat::Fini;
        self.outcome = Some(Err(e));
    }

    /// Arrête, et efface tout ce que le paquet avait écrit.
    pub fn cancel(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.fail("Export annulé.".into());
    }

    fn abandonner(&mut self) {
        if let Some((c, _)) = self.courant.take() {
            match c {
                Courant::Montage(mut j) => j.cancel(),
                Courant::Export(mut j, _) => j.cancel(),
            }
        }
        if let Etat::Rangement { handle, annule, .. } = &mut self.etat {
            annule.store(true, Ordering::Relaxed);
            if let Some(h) = handle.take() {
                let _ = h.join();
            }
        }
        let _ = fs::remove_dir_all(&self.staging);
    }

    pub fn step(&mut self, doc: &Document, budget: Duration) {
        if self.outcome.is_some() {
            return;
        }
        let t0 = Instant::now();
        loop {
            match &mut self.etat {
                Etat::Fini => return,
                Etat::Rangement { handle, .. } => {
                    if handle.as_ref().is_some_and(|h| h.is_finished()) {
                        let res = handle.take().map(|h| h.join().unwrap_or_else(|_| Err("le rangement s'est arrêté".into()))).unwrap_or(Err("rangement perdu".into()));
                        let _ = fs::remove_dir_all(&self.staging);
                        self.etat = Etat::Fini;
                        self.outcome = Some(res);
                    }
                    return;
                }
                Etat::Etapes => {}
            }
            if self.courant.is_none() {
                match self.etapes.pop_front() {
                    Some(e) => match self.lancer(e, doc) {
                        Ok(Some(c)) => self.courant = Some((c, e)),
                        Ok(None) => continue,
                        Err(err) => return self.fail(err),
                    },
                    None => {
                        match self.commencer_le_rangement() {
                            Ok(()) => {}
                            Err(e) => self.fail(e),
                        }
                        return;
                    }
                }
            }
            let left = budget.saturating_sub(t0.elapsed());
            let mut fini: Option<Result<(), String>> = None;
            if let Some((c, _)) = &mut self.courant {
                match c {
                    Courant::Montage(j) => {
                        j.step(doc, left);
                        if self.snapshot.is_none() {
                            self.snapshot = j.snapshot();
                        }
                        if let Some(o) = &j.outcome {
                            fini = Some(o.clone().map(|_| ()));
                        }
                    }
                    Courant::Export(j, _) => {
                        j.step(doc, left);
                        if self.snapshot.is_none() {
                            self.snapshot = j.snapshot();
                        }
                        if let Some(o) = &j.outcome {
                            fini = Some(o.clone().map(|_| ()));
                        }
                    }
                }
            }
            match fini {
                None => return,
                Some(Err(e)) => return self.fail(e),
                Some(Ok(())) => {
                    let Some((c, e)) = self.courant.take() else { return };
                    self.recolter(&c, e);
                    drop(c);
                    if t0.elapsed() >= budget {
                        return;
                    }
                }
            }
        }
    }

    fn avec_cliche(&self, j: ExportJob) -> ExportJob {
        let mut j = j;
        j.set_exact(self.exact.clone());
        match &self.snapshot {
            Some(s) => j.with_snapshot(s.clone()),
            None => j,
        }
    }

    fn lancer(&mut self, e: Etape, doc: &Document) -> Result<Option<Courant>, String> {
        match e {
            Etape::Videos => {
                let (plan, opts) = self.prep.plan_videos.take().ok_or("vidéos déjà faites")?;
                let (ff, enc) = self.ffmpeg.clone().ok_or("ffmpeg manquant")?;
                let dir = self.staging.join("videos");
                fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let j = EditorialJob::start(plan, opts, ff, enc, &dir)?.with_exact(self.exact.clone());
                Ok(Some(Courant::Montage(match &self.snapshot {
                    Some(s) => j.with_snapshot(s.clone()),
                    None => j,
                })))
            }
            Etape::Vignettes => {
                let dir = self.staging.join("vignettes");
                fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let j = exportstills::job(&dir, &self.prep.vignettes, self.prep.frame, &StillsSettings::default(), false);
                Ok(Some(Courant::Export(self.avec_cliche(j), e)))
            }
            Etape::Pdf => {
                let (base, shots, boards) = self.prep.pdf.take().ok_or("PDF déjà fait")?;
                let dir = self.staging.join("pdf");
                fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let out = dir.join(format!("{base}_storyboard.pdf"));
                let j = ExportJob::pdf_layout(&out, &base, shots, boards, self.prep.frame, self.prep.pdf_layout.clone());
                Ok(Some(Courant::Export(self.avec_cliche(j), e)))
            }
            Etape::Animatique => {
                let (frames, span, sound) = self.prep.animatique.take().ok_or("animatique déjà faite")?;
                let (ff, enc) = self.ffmpeg.clone().ok_or("ffmpeg manquant")?;
                let dir = self.staging.join("animatique");
                fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let out = dir.join(format!("{}_animatique.mp4", self.prep.base));
                let frame = self.prep.frame;
                let size = VideoSize::Project.dimensions(frame);
                let rate = self.prep.fps.to_string();
                let spec = move |enc: &Encoders, wav: Option<&Path>, target: &Path| {
                    exportvideo::video_args(&VideoSpec { format: export::Format::Mp4, enc, frame, out_size: size, rate_in: rate.clone(), rate_out: None, audio: wav, target, mxf: false })
                };
                if let Some(p) = exportvideo::size_problem_for(export::Format::Mp4, size, &enc) {
                    return Err(format!("Animatique impossible : {p}."));
                }
                spec(&enc, None, &out)?;
                let target = out.clone();
                let j = ExportJob::video_with_sound_alpha(&ff, move |wav| spec(&enc, wav, &target), &out, frames, sound.then_some(span), false).with_transitions(doc);
                Ok(Some(Courant::Export(self.avec_cliche(j), e)))
            }
            Etape::Listes => {
                let dir = self.staging.join("liste");
                fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                for (nom, texte) in std::mem::take(&mut self.prep.listes) {
                    let p = dir.join(&nom);
                    export::write_atomic(&p, texte.as_bytes())?;
                    self.prevus.push(Prevu { sorte: Sorte::Liste, panel: None, nom, source: p });
                }
                Ok(None)
            }
        }
    }

    /// Note les fichiers que l'étape `e` vient d'écrire.
    fn recolter(&mut self, c: &Courant, e: Etape) {
        match (c, e) {
            (Courant::Montage(j), _) => {
                for clip in &j.plan.clips {
                    let nom = Path::new(&clip.file).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| clip.file.clone());
                    self.prevus.push(Prevu { sorte: Sorte::Video, panel: Some(clip.panel), nom, source: j.dir.join(&clip.file) });
                }
            }
            (Courant::Export(..), Etape::Vignettes) => {
                for s in &self.prep.vignettes {
                    self.prevus.push(Prevu { sorte: Sorte::Vignette, panel: Some(s.frame.0), nom: format!("{}.png", s.name), source: self.staging.join("vignettes").join(format!("{}.png", s.name)) });
                }
            }
            (Courant::Export(..), Etape::Pdf) => {
                let nom = format!("{}_storyboard.pdf", self.prep.base);
                self.prevus.push(Prevu { sorte: Sorte::Pdf, panel: None, source: self.staging.join("pdf").join(&nom), nom });
            }
            (Courant::Export(..), Etape::Animatique) => {
                let nom = format!("{}_animatique.mp4", self.prep.base);
                self.prevus.push(Prevu { sorte: Sorte::Animatique, panel: None, source: self.staging.join("animatique").join(&nom), nom });
            }
            _ => {}
        }
    }

    fn commencer_le_rangement(&mut self) -> Result<(), String> {
        let chemins = rangement_avec(&self.global, &self.dossiers, &self.prevus)?;
        let items: Vec<(PathBuf, PathBuf, Sorte)> = self.prevus.iter().zip(chemins).map(|(p, rel)| (p.source.clone(), rel, p.sorte)).collect();
        let total = items.len();
        let fait = Arc::new(AtomicUsize::new(0));
        let annule = Arc::new(AtomicBool::new(false));
        let r = Rangement { items, dossier: self.dossier.clone(), staging: self.staging.clone(), entete: self.entete.clone(), remplacer: self.reglages.remplacer };
        let (f2, a2) = (fait.clone(), annule.clone());
        let handle = std::thread::spawn(move || ranger(r, &a2, &f2));
        self.etat = Etat::Rangement { handle: Some(handle), fait, total, annule };
        Ok(())
    }
}

impl std::fmt::Debug for PaquetJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PaquetJob")
    }
}

impl Drop for PaquetJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant le paquet : rien ne reste.
        if self.outcome.is_none() {
            self.cancel();
        }
    }
}