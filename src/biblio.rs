//! Le panneau Bibliothèque (2026-10-30) : explorer les fichiers de l'ordinateur et en voir, lire ou
//! écouter les images, vidéos et sons dans un aperçu intégré, sans rien en faire d'autre que les
//! poser dans le projet quand on le demande. Lecture seule : rien n'est jamais écrit, déplacé ou
//! supprimé sur le disque.
//!
//! Ce fichier est la partie sans écran :
//!
//! - les formats que SHOT sait lire (`Genre`) : les mêmes que ceux de l'import (images PNG et JPEG,
//!   vidéos par ffmpeg, sons WAV, MP3, FLAC et OGG) ;
//! - la lecture d'un dossier, **sur un fil à part** (`Parcours`) : la liste arrive par paquets, la
//!   première page tout de suite, le tri complet à la fin ; l'interface ne lit jamais un dossier ;
//! - le filtre par type, la recherche dans le dossier, le tri ;
//! - le fil d'Ariane, les lecteurs de Windows, les favoris ;
//! - les réglages retenus d'une séance à l'autre (`BiblioReglages`, `reglages.toml`).
//!
//! Les vignettes sont dans `bibliovignettes.rs`, l'aperçu dans `biblioapercu.rs`, l'écran dans
//! `biblioui.rs`.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as Atomique};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::project::SOUND_EXTS;

/// Le plus de fichiers gardés dans un dossier (un dossier de plusieurs centaines de milliers de
/// fichiers ne fait pas grossir la mémoire sans fin).
pub const MAX_ENTREES: usize = 200_000;
/// Taille d'un paquet envoyé à l'interface pendant la lecture d'un gros dossier.
const PAQUET: usize = 1000;

// ------------------------------------------------------------------------------------ formats

/// Ce que SHOT sait lire d'un fichier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Genre {
    Image,
    Video,
    Son,
}

impl Genre {
    /// Le genre d'un fichier, d'après son extension ; rien s'il n'est pas lisible par SHOT.
    pub fn de(chemin: &Path) -> Option<Genre> {
        let ext = chemin.extension()?.to_str()?.to_ascii_lowercase();
        if crate::references::IMAGE_EXTS.contains(&ext.as_str()) {
            Some(Genre::Image)
        } else if crate::videoui::VIDEO_EXTS.contains(&ext.as_str()) {
            Some(Genre::Video)
        } else if SOUND_EXTS.contains(&ext.as_str()) {
            Some(Genre::Son)
        } else {
            None
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Genre::Image => "Image",
            Genre::Video => "Vidéo",
            Genre::Son => "Son",
        }
    }

    pub fn icone(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Genre::Image => ph::IMAGE,
            Genre::Video => ph::FILM_STRIP,
            Genre::Son => ph::WAVEFORM,
        }
    }
}

// ------------------------------------------------------------------------------------ réglages

/// Grille de vignettes, ou liste (nom, type, durée, taille, date).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vue {
    #[default]
    Grille,
    Liste,
}

/// Le filtre par type de fichier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Filtre {
    #[default]
    Tout,
    Images,
    Videos,
    Sons,
}

impl Filtre {
    pub const TOUS: [Filtre; 4] = [Filtre::Tout, Filtre::Images, Filtre::Videos, Filtre::Sons];

    pub fn nom(self) -> &'static str {
        match self {
            Filtre::Tout => "Tout",
            Filtre::Images => "Images",
            Filtre::Videos => "Vidéos",
            Filtre::Sons => "Sons",
        }
    }

    /// Un fichier de ce genre passe-t-il le filtre ? (Un fichier que SHOT ne sait pas lire ne passe que « Tout ».)
    pub fn laisse_passer(self, genre: Option<Genre>) -> bool {
        match self {
            Filtre::Tout => true,
            Filtre::Images => genre == Some(Genre::Image),
            Filtre::Videos => genre == Some(Genre::Video),
            Filtre::Sons => genre == Some(Genre::Son),
        }
    }
}

/// Le tri de la liste.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tri {
    #[default]
    Nom,
    Type,
    Taille,
    Date,
}

/// Les réglages de la bibliothèque, dans `reglages.toml` (section `[bibliotheque]`) : ils suivent
/// l'utilisateur d'un projet à l'autre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BiblioReglages {
    /// Les dossiers favoris ajoutés à la main (le dossier du projet, Documents et Bureau sont toujours là).
    #[serde(rename = "favoris", skip_serializing_if = "Vec::is_empty")]
    pub favoris: Vec<PathBuf>,
    /// Le dernier dossier ouvert.
    #[serde(rename = "dernier_dossier", skip_serializing_if = "Option::is_none")]
    pub dernier: Option<PathBuf>,
    pub vue: Vue,
    /// Côté d'une vignette de la grille, en points.
    #[serde(rename = "taille_vignettes")]
    pub taille: u16,
    /// Montrer aussi les fichiers que SHOT ne sait pas lire.
    #[serde(rename = "tout_montrer")]
    pub tout_montrer: bool,
    pub filtre: Filtre,
    pub tri: Tri,
    /// Tri du plus grand au plus petit.
    #[serde(rename = "tri_inverse")]
    pub tri_inverse: bool,
    /// L'aperçu rejoue en boucle.
    pub boucle: bool,
    /// Volume de l'aperçu, de 0 à 1.
    pub volume: f32,
    /// Part de la hauteur (ou de la largeur) prise par l'aperçu, de 0,15 à 0,85.
    #[serde(rename = "part_apercu")]
    pub part_apercu: f32,
}

pub const TAILLE_MIN: u16 = 56;
pub const TAILLE_MAX: u16 = 240;

impl Default for BiblioReglages {
    fn default() -> Self {
        BiblioReglages {
            favoris: Vec::new(),
            dernier: None,
            vue: Vue::Grille,
            taille: 104,
            tout_montrer: false,
            filtre: Filtre::Tout,
            tri: Tri::Nom,
            tri_inverse: false,
            boucle: false,
            volume: 0.8,
            part_apercu: 0.5,
        }
    }
}

impl BiblioReglages {
    /// Des valeurs saines (un fichier de réglages édité à la main ne casse rien).
    pub fn nettoye(mut self) -> Self {
        self.taille = self.taille.clamp(TAILLE_MIN, TAILLE_MAX);
        self.volume = if self.volume.is_finite() { self.volume.clamp(0.0, 1.0) } else { 0.8 };
        self.part_apercu = if self.part_apercu.is_finite() { self.part_apercu.clamp(0.15, 0.85) } else { 0.5 };
        self
    }

    /// Le dossier est-il dans les favoris ajoutés à la main ?
    pub fn est_favori(&self, dossier: &Path) -> bool {
        self.favoris.iter().any(|f| meme_chemin(f, dossier))
    }

    /// Ajoute (ou retire, s'il y est déjà) un dossier des favoris ; renvoie vrai s'il y est maintenant.
    pub fn basculer_favori(&mut self, dossier: &Path) -> bool {
        if let Some(i) = self.favoris.iter().position(|f| meme_chemin(f, dossier)) {
            self.favoris.remove(i);
            false
        } else {
            self.favoris.push(dossier.to_path_buf());
            true
        }
    }
}

/// Deux chemins désignent-ils le même dossier ? (Windows ne tient pas compte de la casse.)
pub fn meme_chemin(a: &Path, b: &Path) -> bool {
    a == b || a.to_string_lossy().trim_end_matches(['\\', '/']).eq_ignore_ascii_case(b.to_string_lossy().trim_end_matches(['\\', '/']))
}

// ------------------------------------------------------------------------------------ fichiers

/// Un fichier ou un dossier de la liste.
#[derive(Clone, Debug, PartialEq)]
pub struct Entree {
    pub nom: String,
    /// Le nom en minuscules (recherche et tri).
    pub cle: String,
    pub chemin: PathBuf,
    pub dossier: bool,
    pub genre: Option<Genre>,
    pub octets: u64,
    pub modifie: Option<SystemTime>,
}

impl Entree {
    /// Le texte d'un type : « Dossier », « Image PNG »…
    pub fn type_texte(&self) -> String {
        if self.dossier {
            return "Dossier".into();
        }
        let ext = self.chemin.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_uppercase();
        match self.genre {
            Some(g) if !ext.is_empty() => format!("{} {ext}", g.nom()),
            Some(g) => g.nom().into(),
            None if ext.is_empty() => "Fichier".into(),
            None => format!("Fichier {ext}"),
        }
    }

    pub fn secondes_modifie(&self) -> u64 {
        self.modifie.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
    }
}

/// Un nom lu pour l'ordre naturel : « plan2 » avant « plan10 ».
pub fn ordre_naturel(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = a.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    a.next();
                }
                let mut nb = String::new();
                while let Some(c) = b.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(c);
                    b.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a.next();
                b.next();
            }
        }
    }
}

/// Une entrée lue dans un dossier ; rien pour un fichier caché ou système.
fn entree_de(e: &std::fs::DirEntry) -> Option<Entree> {
    let nom = e.file_name().to_string_lossy().into_owned();
    if nom.starts_with('.') {
        return None;
    }
    let meta = e.metadata().ok()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Caché (0x2) ou système (0x4) : « System Volume Information », « $RECYCLE.BIN »…
        if meta.file_attributes() & 0x6 != 0 {
            return None;
        }
    }
    let chemin = e.path();
    let meta = if meta.file_type().is_symlink() { std::fs::metadata(&chemin).ok()? } else { meta };
    let dossier = meta.is_dir();
    let genre = if dossier { None } else { Genre::de(&chemin) };
    Some(Entree { cle: nom.to_lowercase(), nom, chemin, dossier, genre, octets: if dossier { 0 } else { meta.len() }, modifie: meta.modified().ok() })
}

/// Les dossiers d'abord, puis par ordre naturel des noms.
pub fn trier_par_nom(entrees: &mut [Entree]) {
    entrees.sort_by(|a, b| b.dossier.cmp(&a.dossier).then_with(|| ordre_naturel(&a.cle, &b.cle)));
}

/// Ce que dit une erreur de lecture de dossier, en français.
pub fn erreur_texte(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "Ce dossier n'existe plus (ou le lecteur n'est pas branché).".into(),
        std::io::ErrorKind::PermissionDenied => "Windows n'autorise pas la lecture de ce dossier.".into(),
        _ => format!("Impossible de lire ce dossier ({e})."),
    }
}

// ------------------------------------------------------------------------------------ parcours

/// Ce que le fil de lecture d'un dossier envoie à l'interface.
pub enum Message {
    /// Les premiers fichiers, pas encore triés : la liste s'affiche tout de suite.
    Partiel(Vec<Entree>),
    /// Tout le dossier, trié ; `tronque` si `MAX_ENTREES` a coupé la liste.
    Fin { entrees: Vec<Entree>, tronque: bool },
    Erreur(String),
}

/// La lecture d'un dossier, sur un fil à part. L'abandonner (la laisser tomber) l'arrête.
pub struct Parcours {
    rx: Receiver<Message>,
    annule: Arc<AtomicBool>,
}

impl Drop for Parcours {
    fn drop(&mut self) {
        self.annule.store(true, Atomique::Relaxed);
    }
}

/// Lit `dossier` : `envoyer` reçoit les messages. Appelée sur le fil de lecture (et par les essais).
pub fn lire_dossier(dossier: &Path, annule: &AtomicBool, mut envoyer: impl FnMut(Message)) {
    let lecture = match std::fs::read_dir(dossier) {
        Ok(l) => l,
        Err(e) => return envoyer(Message::Erreur(erreur_texte(&e))),
    };
    let mut tout: Vec<Entree> = Vec::new();
    let mut envoye = 0;
    let mut tronque = false;
    for e in lecture.flatten() {
        if annule.load(Atomique::Relaxed) {
            return;
        }
        let Some(entree) = entree_de(&e) else { continue };
        if tout.len() >= MAX_ENTREES {
            tronque = true;
            break;
        }
        tout.push(entree);
        if tout.len() - envoye >= PAQUET && envoye == 0 {
            envoye = tout.len();
            envoyer(Message::Partiel(tout.clone()));
        }
    }
    trier_par_nom(&mut tout);
    envoyer(Message::Fin { entrees: tout, tronque });
}

impl Parcours {
    pub fn lancer(dossier: PathBuf) -> Parcours {
        let (tx, rx) = channel();
        let annule = Arc::new(AtomicBool::new(false));
        let (a, d) = (annule.clone(), dossier.clone());
        let fil = std::thread::Builder::new().name("bibliotheque-dossier".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            lire_dossier(&d, &a, |m| {
                let _ = tx.send(m);
            });
        });
        // (Un fil qui ne part pas : la liste dira qu'elle n'a rien lu, rien ne plante.)
        if fil.is_err() {
            annule.store(true, Atomique::Relaxed);
        }
        Parcours { rx, annule }
    }

    /// Les messages arrivés depuis la dernière fois.
    pub fn relever(&self) -> Vec<Message> {
        self.rx.try_iter().collect()
    }
}

// ------------------------------------------------------------------------------------ navigateur

/// Le dossier courant, sa liste, la recherche et le fichier choisi.
pub struct Navigateur {
    pub dossier: PathBuf,
    pub entrees: Vec<Entree>,
    parcours: Option<Parcours>,
    /// La liste est en cours de lecture.
    pub charge: bool,
    pub erreur: Option<String>,
    pub tronque: bool,
    pub recherche: String,
    /// Le fichier (ou dossier) choisi.
    pub selection: Option<PathBuf>,
    /// Les rangs (dans `entrees`) de ce qui passe le filtre, dans l'ordre du tri.
    visibles: Vec<usize>,
    cle: Option<CleFiltre>,
    /// Une lecture de dossier a eu lieu au moins une fois (la première ouverture du panneau la lance).
    pub demarre: bool,
    /// Les dossiers visités, pour « retour ».
    historique: Vec<PathBuf>,
}

#[derive(Clone, PartialEq)]
struct CleFiltre {
    filtre: Filtre,
    tout: bool,
    recherche: String,
    tri: Tri,
    inverse: bool,
    nombre: usize,
    fin: bool,
}

impl Default for Navigateur {
    fn default() -> Self {
        Navigateur {
            dossier: PathBuf::new(),
            entrees: Vec::new(),
            parcours: None,
            charge: false,
            erreur: None,
            tronque: false,
            recherche: String::new(),
            selection: None,
            visibles: Vec::new(),
            cle: None,
            demarre: false,
            historique: Vec::new(),
        }
    }
}

impl Navigateur {
    /// Ouvre `dossier` : la lecture part sur un fil à part, la liste se remplit au fil des images.
    pub fn aller(&mut self, dossier: PathBuf) {
        if self.demarre && meme_chemin(&self.dossier, &dossier) && self.erreur.is_none() {
            return self.actualiser();
        }
        if self.demarre && !self.dossier.as_os_str().is_empty() && self.historique.last() != Some(&self.dossier) {
            self.historique.push(self.dossier.clone());
            if self.historique.len() > 64 {
                self.historique.remove(0);
            }
        }
        self.dossier = dossier;
        self.recherche.clear();
        self.selection = None;
        self.actualiser();
    }

    /// Relit le dossier courant.
    pub fn actualiser(&mut self) {
        self.demarre = true;
        self.entrees.clear();
        self.erreur = None;
        self.tronque = false;
        self.cle = None;
        self.charge = true;
        self.parcours = Some(Parcours::lancer(self.dossier.clone()));
    }

    /// Le dossier parent, s'il y en a un.
    pub fn parent(&self) -> Option<PathBuf> {
        self.dossier.parent().filter(|p| !p.as_os_str().is_empty()).map(Path::to_path_buf)
    }

    #[cfg(test)]
    pub fn remonter(&mut self) -> bool {
        match self.parent() {
            Some(p) => {
                self.aller(p);
                true
            }
            None => false,
        }
    }

    /// Revient au dossier visité avant celui-ci.
    pub fn retour(&mut self) -> bool {
        match self.historique.pop() {
            Some(d) => {
                self.dossier = d;
                self.recherche.clear();
                self.selection = None;
                self.actualiser();
                true
            }
            None => false,
        }
    }

    pub fn peut_revenir(&self) -> bool {
        !self.historique.is_empty()
    }

    /// Reprend ce que la lecture a trouvé ; vrai si quelque chose a changé.
    pub fn poll(&mut self) -> bool {
        let Some(p) = &self.parcours else { return false };
        let messages = p.relever();
        let mut change = false;
        for m in messages {
            change = true;
            match m {
                Message::Partiel(e) => self.entrees = e,
                Message::Fin { entrees, tronque } => {
                    self.entrees = entrees;
                    self.tronque = tronque;
                    self.charge = false;
                    self.parcours = None;
                }
                Message::Erreur(e) => {
                    self.erreur = Some(e);
                    self.charge = false;
                    self.parcours = None;
                }
            }
            if self.parcours.is_none() {
                break;
            }
        }
        change
    }

    /// La lecture du dossier est-elle en route (un fil tourne) ?
    pub fn lit(&self) -> bool {
        self.parcours.is_some()
    }

    /// Ce qui passe le filtre et la recherche, dans l'ordre du tri ; refait seulement quand quelque chose a changé.
    #[cfg(test)]
    pub fn visibles(&mut self, r: &BiblioReglages) -> &[usize] {
        self.rafraichir(r);
        &self.visibles
    }

    /// Refait la liste visible si le filtre, la recherche, le tri ou la liste ont changé.
    pub fn rafraichir(&mut self, r: &BiblioReglages) {
        let cle = CleFiltre { filtre: r.filtre, tout: r.tout_montrer, recherche: self.recherche.clone(), tri: r.tri, inverse: r.tri_inverse, nombre: self.entrees.len(), fin: !self.charge };
        if self.cle.as_ref() != Some(&cle) {
            self.visibles = filtrer(&self.entrees, r.filtre, r.tout_montrer, &self.recherche, r.tri, r.tri_inverse);
            self.cle = Some(cle);
        }
    }

    /// La liste visible telle que `rafraichir` l'a faite.
    pub fn rangs(&self) -> &[usize] {
        &self.visibles
    }

    /// L'entrée choisie.
    pub fn choisie(&self) -> Option<&Entree> {
        let s = self.selection.as_ref()?;
        self.entrees.iter().find(|e| &e.chemin == s)
    }

    /// Le fichier lisible suivant (ou précédent) dans la liste visible, pour passer de l'un à l'autre aux flèches.
    pub fn voisin(&self, sens: i32) -> Option<PathBuf> {
        let courant = self.selection.as_ref()?;
        let i = self.visibles.iter().position(|&k| &self.entrees[k].chemin == courant)?;
        let j = i as i64 + sens as i64;
        let k = *self.visibles.get(usize::try_from(j).ok()?)?;
        Some(self.entrees[k].chemin.clone())
    }
}

/// Les rangs de `entrees` qui passent `filtre`, `tout` (montrer aussi les fichiers illisibles) et `recherche`, triés.
/// Les dossiers restent toujours devant et ne sont masqués que par la recherche.
pub fn filtrer(entrees: &[Entree], filtre: Filtre, tout: bool, recherche: &str, tri: Tri, inverse: bool) -> Vec<usize> {
    let mots: Vec<String> = recherche.split_whitespace().map(str::to_lowercase).collect();
    let mut v: Vec<usize> = (0..entrees.len())
        .filter(|&i| {
            let e = &entrees[i];
            if !mots.iter().all(|m| e.cle.contains(m.as_str())) {
                return false;
            }
            e.dossier || ((tout || e.genre.is_some()) && filtre.laisse_passer(e.genre))
        })
        .collect();
    // Le tri par nom est celui de la lecture (déjà fait) ; un autre se refait ici.
    if tri != Tri::Nom || inverse {
        v.sort_by(|a, b| {
            let (x, y) = (&entrees[*a], &entrees[*b]);
            // Les dossiers restent devant, quel que soit le tri.
            let devant = y.dossier.cmp(&x.dossier);
            if devant != Ordering::Equal {
                return devant;
            }
            let nom = ordre_naturel(&x.cle, &y.cle);
            let o = if x.dossier {
                // (Les dossiers n'ont ni taille ni type : leur ordre reste celui des noms.)
                nom
            } else {
                match tri {
                    Tri::Nom => nom,
                    Tri::Type => x.type_texte().to_lowercase().cmp(&y.type_texte().to_lowercase()).then(nom),
                    Tri::Taille => x.octets.cmp(&y.octets).then(nom),
                    Tri::Date => x.modifie.cmp(&y.modifie).then(nom),
                }
            };
            if inverse {
                o.reverse()
            } else {
                o
            }
        });
    }
    v
}

// ------------------------------------------------------------------------------------ chemins

/// Le fil d'Ariane d'un dossier : (nom, chemin) du lecteur jusqu'au dossier.
pub fn fil_d_ariane(dossier: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<(String, PathBuf)> = dossier
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| {
            let nom = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string().trim_end_matches(['\\', '/']).to_string());
            (if nom.is_empty() { "/".to_string() } else { nom }, p.to_path_buf())
        })
        .collect();
    v.reverse();
    v
}

/// Les lecteurs de l'ordinateur (lettres de Windows). Ne touche à aucun disque : Windows donne la liste d'un coup.
pub fn lecteurs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLogicalDrives() -> u32;
        }
        // SAFETY: `GetLogicalDrives` ne prend rien et rend un masque de bits.
        let masque = unsafe { GetLogicalDrives() };
        (0..26u8).filter(|i| masque >> i & 1 == 1).map(|i| PathBuf::from(format!("{}:\\", (b'A' + i) as char))).collect()
    }
    #[cfg(not(windows))]
    {
        vec![PathBuf::from("/")]
    }
}

/// Un favori d'office : son nom et son dossier.
#[derive(Clone, Debug, PartialEq)]
pub struct FavoriFixe {
    pub nom: &'static str,
    pub icone: &'static str,
    pub chemin: PathBuf,
}

/// Les favoris qui sont toujours là : le dossier du projet (s'il est enregistré), Documents et Bureau.
/// `profil` : le dossier de l'utilisateur. Aucun disque n'est interrogé ici : un dossier qui n'existe pas est dit
/// par la lecture du dossier, quand on l'ouvre.
pub fn favoris_fixes(projet: Option<&Path>, profil: Option<&Path>) -> Vec<FavoriFixe> {
    use egui_phosphor::regular as ph;
    let mut v = Vec::new();
    if let Some(p) = projet {
        v.push(FavoriFixe { nom: "Projet", icone: ph::FOLDER_OPEN, chemin: p.to_path_buf() });
    }
    if let Some(profil) = profil {
        v.push(FavoriFixe { nom: "Documents", icone: ph::FILE_TEXT, chemin: profil.join("Documents") });
        v.push(FavoriFixe { nom: "Bureau", icone: ph::DESKTOP, chemin: profil.join("Desktop") });
    }
    v
}

/// Le dossier de l'utilisateur de Windows (lu dans l'environnement, sans toucher au disque).
pub fn dossier_profil() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)
}

/// Les dossiers où peut s'ouvrir la bibliothèque, du préféré au dernier recours : le dernier visité, celui du projet,
/// Documents, le dossier de l'utilisateur, le premier lecteur. Si l'un d'eux ne se lit pas, on prend le suivant
/// (l'ouverture ne se bloque jamais sur un dossier disparu).
pub fn candidats_de_depart(reglages: &BiblioReglages, projet: Option<&Path>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    v.extend(reglages.dernier.clone());
    v.extend(projet.map(Path::to_path_buf));
    if let Some(p) = dossier_profil() {
        v.push(p.join("Documents"));
        v.push(p);
    }
    v.extend(lecteurs().into_iter().next());
    v.dedup_by(|a, b| meme_chemin(a, b));
    v
}

// ------------------------------------------------------------------------------------ textes

/// « 12 o », « 3,4 Ko », « 12,5 Mo », « 1,2 Go ».
pub fn taille_texte(octets: u64) -> String {
    const U: [&str; 4] = ["Ko", "Mo", "Go", "To"];
    if octets < 1024 {
        return format!("{octets} o");
    }
    let mut v = octets as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < U.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let t = if v >= 100.0 { format!("{v:.0}") } else { format!("{v:.1}") };
    format!("{} {}", t.replace('.', ","), U[u])
}

/// « 0:07 », « 1:05 », « 1:02:03 » ; « 0,4 s » sous la seconde.
pub fn duree_texte(secondes: f64) -> String {
    if !secondes.is_finite() || secondes < 0.0 {
        return "—".into();
    }
    if secondes < 1.0 {
        return format!("{secondes:.1} s").replace('.', ",");
    }
    let s = secondes.round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// La date d'un fichier, en français (`30/10/2026`).
pub fn date_texte(secondes: u64) -> String {
    if secondes == 0 {
        String::new()
    } else {
        crate::planchref::date_fr(secondes)
    }
}
