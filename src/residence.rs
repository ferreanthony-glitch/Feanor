//! Les dessins en mémoire (`docs/PERFORMANCE.md`, section 27) : un film de 300 plans tenait tous ses pixels, 2,7 Go de
//! dessins bitmap et 2,2 Go d'images de dessins vectoriels, du début à la fin de la séance.
//!
//! Un dessin peut maintenant être **absent** (`Cel::absent`) : il est toujours dans le document (son numéro, sa version,
//! ses traits s'il est vectoriel, sa strate Fond), mais ses pixels ne sont pas en mémoire. `Cel::raster` est alors une
//! image vide de la taille de la feuille, que personne ne doit lire. Deux sortes :
//! - un dessin **bitmap** déjà écrit à sa version dans le dossier du projet : ses pixels se relisent de son fichier ;
//! - l'**image** d'un dessin vectoriel : ses traits restent, l'image se refait par le rejeu (`rejeu.rs`).
//!
//! `Dessins` (la table `Document::cels`) ne montre aux lecteurs ordinaires que les dessins présents (`get`, `get_mut`,
//! `values`, `iter`) : un dessin absent y est comme un dessin manquant, qu'on ne peut ni lire ni modifier. Les rares
//! lecteurs qui doivent aussi voir les absents (enregistrement, numéros, versions, traits) passent par `get_ou_absent` /
//! `tous`. C'est la règle qui protège les pixels : rien ne peut peindre dans une image vide qui remplacerait le dessin, et
//! aucun cache (vignettes, effets, carte graphique) ne peut garder une image vide sous la version du dessin.
//!
//! Plusieurs dessins peuvent tenir la même absence (`Arc`, un dessin dupliqué pendant qu'il était absent) : ils reviennent
//! ensemble (`Dessins::resoudre`). Le fichier d'un dessin absent n'est jamais effacé par un enregistrement
//! (`Dessins::fichiers_a_garder`) ; il ne peut pas être réécrit tant que le dessin est absent (le réécrire demande de le
//! modifier, donc qu'il soit revenu).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};

use crate::document::{Cel, CelId};
use crate::raster::Raster;

/// L'identité d'un fichier (taille et date de modification) au moment où l'on s'y est fié : relu plus tard, un fichier
/// qui n'a plus la même identité a été changé à côté de SHOT.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Identite {
    pub taille: u64,
    pub modifie: Option<std::time::SystemTime>,
}

impl Identite {
    pub(crate) fn de(path: &std::path::Path) -> Option<Identite> {
        let m = std::fs::metadata(path).ok()?;
        Some(Identite { taille: m.len(), modifie: m.modified().ok() })
    }
}

/// D'où reviennent les pixels d'un dessin absent.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Source {
    /// Un dessin bitmap : son fichier `dossier/dessins/<id>.png` (et son hors-feuille, `hors`), écrit à la version du
    /// dessin. `id` est celui du dessin dont c'est le fichier (une copie absente tient la même source). `identite` : celle
    /// du fichier quand on l'a laissé (rien : pas encore vérifiée, un projet ouvert sans lire ses dessins).
    Fichier { dossier: PathBuf, id: CelId, hors: bool, identite: Option<Identite> },
    /// L'image d'un dessin vectoriel : elle se refait d'après ses traits.
    Rendu,
}

/// Un dessin dont les pixels ne sont pas en mémoire.
///
/// Une absence peut être tenue hors du document (une copie dans le presse-papiers, un cliché d'export, de vignette ou
/// d'enregistrement, un sous-projet) : quand le document la résout (`Dessins::resoudre`), les pixels lus y sont aussi posés
/// (`charge`), et ces copies les y trouvent. C'est ce qui rend sûre une copie d'un dessin absent : son fichier ne peut
/// changer qu'après que le dessin est revenu (le modifier demande qu'il soit présent), donc après que `charge` a reçu les
/// pixels d'avant.
pub(crate) struct Absent {
    pub source: Source,
    /// Tuiles qu'il avait (fenêtre et hors-feuille) ; 0 : on ne sait pas (projet ouvert sans le lire).
    pub tuiles: usize,
    /// Les pixels, une fois que le document les a fait revenir (pour les copies qui tiennent encore l'absence).
    charge: std::sync::Mutex<Option<Raster>>,
    /// Lecture en cours par le chargeur (0 : libre, 1 : en lecture) : deux demandes ne la lisent pas deux fois.
    lecture: std::sync::atomic::AtomicU8,
}

impl std::fmt::Debug for Absent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Absent({:?}, {} tuiles)", self.source, self.tuiles)
    }
}

impl Absent {
    pub(crate) fn fichier(dossier: PathBuf, id: CelId, hors: bool, identite: Option<Identite>, tuiles: usize) -> Arc<Absent> {
        Arc::new(Absent { source: Source::Fichier { dossier, id, hors, identite }, tuiles, charge: Default::default(), lecture: Default::default() })
    }

    pub(crate) fn rendu(tuiles: usize) -> Arc<Absent> {
        Arc::new(Absent { source: Source::Rendu, tuiles, charge: Default::default(), lecture: Default::default() })
    }

    /// Les pixels déjà revenus (résolus par le document), s'il y en a.
    pub(crate) fn deja_charge(&self) -> Option<Raster> {
        self.charge.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn poser_charge(&self, r: &Raster) {
        *self.charge.lock().unwrap_or_else(|e| e.into_inner()) = Some(r.clone());
    }

    /// Les pixels d'un dessin bitmap absent, de la feuille `w` × `h` : ceux déjà revenus, sinon lus de son fichier. Lit le
    /// disque : jamais sur le fil de l'interface (fils de fond : vignettes, exports, enregistrement, préchargement).
    pub(crate) fn lire(&self, w: usize, h: usize) -> Result<Raster, String> {
        if let Some(r) = self.deja_charge() {
            return Ok(r);
        }
        match &self.source {
            Source::Fichier { .. } => {
                let png = self.png().expect("fichier");
                crate::project::relire_bitmap(&png, self.hors().as_ref().map(|(a, b)| (a.as_path(), b.as_path())), w, h)
            }
            Source::Rendu => Err("l'image d'un dessin vectoriel se refait d'après ses traits".into()),
        }
    }

    /// Le fichier PNG du dessin (bitmap).
    pub(crate) fn png(&self) -> Option<PathBuf> {
        match &self.source {
            Source::Fichier { dossier, id, .. } => Some(dossier.join("dessins").join(format!("{id}.png"))),
            Source::Rendu => None,
        }
    }

    /// Les fichiers du hors-feuille (atlas PNG, index JSON), s'il en a un.
    pub(crate) fn hors(&self) -> Option<(PathBuf, PathBuf)> {
        match &self.source {
            Source::Fichier { dossier, id, hors: true, .. } => Some((dossier.join("dessins").join(format!("{id}_hors.png")), dossier.join("dessins").join(format!("{id}_hors.json")))),
            _ => None,
        }
    }

    pub(crate) fn a_du_hors(&self) -> bool {
        matches!(self.source, Source::Fichier { hors: true, .. })
    }
}

/// Les dessins d'un document (`Document::cels`). Voir le haut du module : les lecteurs ordinaires ne voient que les
/// dessins présents.
#[derive(Default, Clone)]
pub struct Dessins {
    map: HashMap<CelId, Cel>,
}

impl std::fmt::Debug for Dessins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dessins({} dont {} absents)", self.map.len(), self.absents())
    }
}

impl Dessins {
    pub fn new() -> Dessins {
        Dessins::default()
    }

    /// Nombre de dessins (absents compris).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Le dessin existe (présent ou absent).
    pub fn contains_key(&self, id: &CelId) -> bool {
        self.map.contains_key(id)
    }

    /// Les numéros de tous les dessins (absents compris).
    pub fn keys(&self) -> std::collections::hash_map::Keys<'_, CelId, Cel> {
        self.map.keys()
    }

    /// Le dessin, s'il est présent (un dessin absent n'est ni lisible ni modifiable).
    pub fn get(&self, id: &CelId) -> Option<&Cel> {
        self.map.get(id).filter(|c| c.absent.is_none())
    }

    /// Le dessin à modifier, s'il est présent.
    pub fn get_mut(&mut self, id: &CelId) -> Option<&mut Cel> {
        self.map.get_mut(id).filter(|c| c.absent.is_none())
    }

    /// Le dessin, présent ou absent : pour ce qui ne lit pas ses pixels (version, traits, strate Fond, enregistrement).
    pub fn get_ou_absent(&self, id: &CelId) -> Option<&Cel> {
        self.map.get(id)
    }

    /// Le dessin à modifier, présent ou absent : seulement pour ce qui ne touche pas ses pixels (adopter un rendu,
    /// résoudre une absence).
    pub(crate) fn get_ou_absent_mut(&mut self, id: &CelId) -> Option<&mut Cel> {
        self.map.get_mut(id)
    }

    /// Le dessin existe et ses pixels ne sont pas en mémoire.
    pub fn est_absent(&self, id: &CelId) -> bool {
        self.map.get(id).is_some_and(|c| c.absent.is_some())
    }

    /// Nombre de dessins absents.
    pub fn absents(&self) -> usize {
        self.map.values().filter(|c| c.absent.is_some()).count()
    }

    pub fn insert(&mut self, id: CelId, cel: Cel) -> Option<Cel> {
        self.map.insert(id, cel)
    }

    pub fn remove(&mut self, id: &CelId) -> Option<Cel> {
        self.map.remove(id)
    }

    /// Les dessins présents.
    pub fn values(&self) -> impl Iterator<Item = &Cel> {
        self.map.values().filter(|c| c.absent.is_none())
    }

    /// Les dessins présents, à modifier.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Cel> {
        self.map.values_mut().filter(|c| c.absent.is_none())
    }

    /// Les dessins présents, avec leur numéro.
    pub fn iter(&self) -> impl Iterator<Item = (&CelId, &Cel)> {
        self.map.iter().filter(|(_, c)| c.absent.is_none())
    }

    /// Les dessins présents, avec leur numéro, à modifier.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&CelId, &mut Cel)> {
        self.map.iter_mut().filter(|(_, c)| c.absent.is_none())
    }

    /// Tous les dessins, absents compris (pour ce qui ne lit pas leurs pixels).
    pub fn tous(&self) -> impl Iterator<Item = (&CelId, &Cel)> {
        self.map.iter()
    }

    /// Tous les dessins (absents compris), rendus en table : ce qui s'en va du document en entier (projet partagé).
    pub(crate) fn into_map(self) -> HashMap<CelId, Cel> {
        self.map
    }

    /// Les dessins tenant l'absence `a` (un dessin et ses copies faites pendant qu'il était absent) reprennent les pixels
    /// `raster`. Rend leurs numéros.
    pub(crate) fn resoudre(&mut self, a: &Arc<Absent>, raster: &Raster) -> Vec<CelId> {
        let mut out = Vec::new();
        for (id, c) in self.map.iter_mut() {
            if c.absent.as_ref().is_some_and(|x| Arc::ptr_eq(x, a)) {
                c.raster = raster.clone();
                c.absent = None;
                out.push(*id);
            }
        }
        // (Les copies tenues hors du document — presse-papiers, clichés — trouveront les pixels là : voir `Absent`. Il ne
        // reste plus que la référence de l'appelant si personne d'autre ne la tient.)
        if Arc::strong_count(a) > 1 {
            a.poser_charge(raster);
        }
        out
    }

    /// Les fichiers (dans le dossier `dossier`) des dessins bitmap absents : un enregistrement ne doit jamais les effacer,
    /// ils tiennent les seuls pixels de ces dessins. Les numéros dont c'est le fichier.
    pub(crate) fn fichiers_a_garder(&self, dossier: &std::path::Path) -> std::collections::BTreeSet<CelId> {
        let mut out = std::collections::BTreeSet::new();
        for c in self.map.values() {
            if let Some(Source::Fichier { dossier: d, id, .. }) = c.absent.as_ref().map(|a| &a.source) {
                if d == dossier {
                    out.insert(*id);
                }
            }
        }
        out
    }
}

impl std::ops::Index<&CelId> for Dessins {
    type Output = Cel;
    /// Le dessin, présent ou absent (essais ; le code de l'application passe par `get`).
    fn index(&self, id: &CelId) -> &Cel {
        &self.map[id]
    }
}

impl From<HashMap<CelId, Cel>> for Dessins {
    fn from(map: HashMap<CelId, Cel>) -> Dessins {
        Dessins { map }
    }
}

impl FromIterator<(CelId, Cel)> for Dessins {
    fn from_iter<I: IntoIterator<Item = (CelId, Cel)>>(iter: I) -> Dessins {
        Dessins { map: iter.into_iter().collect() }
    }
}

impl Extend<(CelId, Cel)> for Dessins {
    fn extend<I: IntoIterator<Item = (CelId, Cel)>>(&mut self, iter: I) {
        self.map.extend(iter)
    }
}

impl<'a> IntoIterator for &'a Dessins {
    type Item = (&'a CelId, &'a Cel);
    type IntoIter = Box<dyn Iterator<Item = (&'a CelId, &'a Cel)> + 'a>;
    /// Les dessins présents.
    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

// ---------------------------------------------------------------------------------- copies résolues sur un fil de fond

impl Cel {
    /// Les pixels de ce dessin, sur un fil de fond : les siens s'il est présent ; lus de son fichier (bitmap absent) ;
    /// rendus d'après ses traits (image absente d'un dessin vectoriel). `w` × `h` : la feuille. Jamais sur le fil de
    /// l'interface (lecture du disque, rendu des traits).
    pub(crate) fn pixels_hors_interface(&self, w: usize, h: usize) -> Result<std::borrow::Cow<'_, Raster>, String> {
        let Some(a) = &self.absent else { return Ok(std::borrow::Cow::Borrowed(&self.raster)) };
        match (&a.source, &self.vector) {
            (Source::Rendu, Some(v)) => Ok(std::borrow::Cow::Owned(v.render(w, h))),
            (Source::Rendu, None) => Err("image d'un dessin vectoriel sans traits".into()),
            (Source::Fichier { .. }, _) => a.lire(w, h).map(std::borrow::Cow::Owned),
        }
    }
}

/// Fait revenir les dessins absents d'une **copie** du document (cliché d'une vignette, d'un export, d'une comparaison) :
/// lus de leur fichier, ou rendus d'après leurs traits. Sur un fil de fond. Rend le nombre de dessins qui n'ont pas pu
/// revenir (ils restent vides dans la copie).
pub(crate) fn resoudre_copie(doc: &mut crate::document::Document) -> usize {
    let (w, h) = (doc.width, doc.height);
    let ids: Vec<CelId> = doc.cels.tous().filter(|(_, c)| c.absent.is_some()).map(|(id, _)| *id).collect();
    let mut perdus = 0;
    for id in ids {
        let Some(c) = doc.cels.get_ou_absent(&id) else { continue };
        match c.pixels_hors_interface(w, h).map(std::borrow::Cow::into_owned) {
            Ok(r) => {
                if let Some(c) = doc.cels.get_ou_absent_mut(&id) {
                    c.raster = r;
                    c.absent = None;
                }
            }
            Err(_) => perdus += 1,
        }
    }
    perdus
}

/// Essais : une copie du document où les dessins bitmap absents sont revenus (lus de leur fichier, sur ce fil) ; les images
/// vectorielles absentes restent vides (leurs traits se comparent, pas leurs pixels). Rend aussi les dessins qui n'ont pas pu
/// revenir.
#[cfg(test)]
pub(crate) fn copie_resolue(doc: &crate::document::Document) -> (crate::document::Document, Vec<CelId>) {
    let mut copie = doc.copie_sans_dessins();
    let (w, h) = (doc.width, doc.height);
    let mut perdus = Vec::new();
    for (id, c) in doc.cels.tous() {
        let mut c = c.clone();
        if let Some(a) = c.absent.take() {
            if c.vector.is_none() {
                match a.lire(w, h) {
                    Ok(r) => c.raster = r,
                    Err(_) => {
                        perdus.push(*id);
                        c.absent = Some(a);
                    }
                }
            }
        }
        copie.cels.insert(*id, c);
    }
    (copie, perdus)
}

// ---------------------------------------------------------------------------------- le budget

/// Le budget des pixels des dessins présents, en octets (0 : sans limite). Préférences > Performance.
static BUDGET: AtomicU64 = AtomicU64::new(0);

/// Pose le budget (`diagnostic::Cibles::appliquer_globales`).
pub(crate) fn set_budget(octets: u64) {
    BUDGET.store(octets, Ordering::Relaxed);
}

pub(crate) fn budget() -> u64 {
    // (Essais : sans limite, comme avant, sauf sur le fil d'un essai qui en pose une : `budget_essai`.)
    #[cfg(test)]
    return BUDGET_ESSAI.with(std::cell::Cell::get).unwrap_or(0);
    #[cfg(not(test))]
    BUDGET.load(Ordering::Relaxed)
}

#[cfg(test)]
thread_local! {
    /// Essais : le budget de ce fil (les essais tournent en parallèle, chacun avec son application).
    static BUDGET_ESSAI: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Essais : le budget des dessins sur ce fil (rien : celui de l'application).
#[cfg(test)]
pub(crate) fn budget_essai(octets: Option<u64>) {
    BUDGET_ESSAI.with(|b| b.set(octets));
}

/// Le budget d'office, d'après la mémoire de la machine : un seizième de la mémoire vive, entre 512 Mo et 2 Go (un film de
/// 300 plans tenait 4,9 Go de pixels ; ce qu'on regarde, ses voisins et la lecture qui vient en demandent bien moins).
pub(crate) fn budget_auto_mo() -> u32 {
    let ram = crate::materiel::memoire().map_or(16 << 30, |m| m.totale);
    ((ram / 16) >> 20).clamp(512, 2048) as u32
}

/// Octets d'une tuile.
pub(crate) const OCTETS_TUILE: u64 = (crate::raster::TILE_PIXELS * 4) as u64;

/// Tuiles d'une image (fenêtre et hors-feuille).
pub(crate) fn tuiles_de(r: &Raster) -> usize {
    r.tiles.iter().filter(|t| t.is_some()).count() + r.outer.len()
}

// ---------------------------------------------------------------------------------- le chargeur

/// Ce que le chargeur rapporte à un document.
pub(crate) enum Arrivee {
    /// Les pixels d'un dessin absent, lus de son fichier.
    Lu { absent: Arc<Absent>, pixels: Result<Raster, String> },
    /// L'identité du fichier d'un dessin à laisser (rien : un de ses fichiers n'est pas là).
    Verifie { id: CelId, version: u64, identite: Option<Identite> },
}

pub(crate) type Boite = Arc<Mutex<Vec<Arrivee>>>;

pub(crate) enum Travail {
    Lire { absent: Arc<Absent>, w: usize, h: usize },
    Verifier { id: CelId, version: u64, fichiers: Vec<PathBuf> },
}

pub(crate) struct Tache {
    /// Le tour (`TOUR`) de la demande : les plus récentes passent d'abord ; au-delà de `PEREMPTION` tours sans être
    /// redemandée, elle est oubliée (on regarde ailleurs).
    pub tour: u64,
    /// Le rang dans ce que le document veut (0 : l'image à l'écran d'abord).
    pub rang: u32,
    pub travail: Travail,
    pub boite: Boite,
    pub reveil: Option<eframe::egui::Context>,
}

#[derive(Default)]
struct File {
    taches: Vec<Tache>,
    fils: usize,
}

static FILE: LazyLock<(Mutex<File>, Condvar)> = LazyLock::new(Default::default);

/// Le tour du document affiché (chaque image d'écran où il suit ses dessins).
pub(crate) static TOUR: AtomicU64 = AtomicU64::new(0);

/// Une demande qui n'a pas été renouvelée depuis ce nombre de tours est oubliée.
/// (Essais : le tour est partagé par tous les essais qui tournent ensemble ; aucune demande n'y est périmée.)
#[cfg(not(test))]
const PEREMPTION: u64 = 240;
#[cfg(test)]
const PEREMPTION: u64 = u64::MAX / 2;

/// Fils du chargeur : décoder un PNG de feuille prend 20 à 30 ms ; trois fils suivent un changement de plan sans prendre
/// les cœurs de l'interface et du rejeu.
const FILS: usize = 3;

/// Lectures faites par le chargeur (essais, diagnostic).
static LECTURES: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn lectures() -> usize {
    LECTURES.load(Ordering::Relaxed)
}

/// Lectures et vérifications demandées et pas encore rapportées (essais : attendre le chargeur).
static EN_COURS: AtomicUsize = AtomicUsize::new(0);

#[cfg(test)]
pub(crate) fn en_cours() -> usize {
    EN_COURS.load(Ordering::Relaxed)
}

pub(crate) fn demander(t: Tache) {
    let (m, cv) = &*FILE;
    let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
    EN_COURS.fetch_add(1, Ordering::Relaxed);
    f.taches.push(t);
    if f.fils < FILS {
        f.fils += 1;
        if std::thread::Builder::new().name("dessins".into()).spawn(travailler).is_err() {
            f.fils -= 1;
        }
    }
    drop(f);
    cv.notify_one();
}

fn prendre() -> Tache {
    let (m, cv) = &*FILE;
    let mut f = m.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        let tour = TOUR.load(Ordering::Relaxed);
        // (Les demandes périmées partent sans être faites.)
        let avant = f.taches.len();
        f.taches.retain(|t| t.tour + PEREMPTION >= tour);
        EN_COURS.fetch_sub(avant - f.taches.len(), Ordering::Relaxed);
        let meilleure = f.taches.iter().enumerate().max_by_key(|(_, t)| (t.tour, std::cmp::Reverse(t.rang))).map(|(i, _)| i);
        if let Some(i) = meilleure {
            return f.taches.swap_remove(i);
        }
        f = cv.wait(f).unwrap_or_else(|e| e.into_inner());
    }
}

fn travailler() {
    crate::parallele::fil_sous_la_normale();
    loop {
        let t = prendre();
        let arrivee = match t.travail {
            Travail::Lire { absent, w, h } => {
                // Revenue entre-temps (plus personne d'autre ne la tient), ou un autre fil la lit : rien à faire.
                let libre = absent.lecture.compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed).is_ok();
                if !libre {
                    None
                } else if Arc::strong_count(&absent) <= 1 {
                    absent.lecture.store(0, Ordering::Release);
                    None
                } else {
                    let pixels = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| absent.lire(w, h))).unwrap_or_else(|_| Err("la lecture a planté".into()));
                    LECTURES.fetch_add(1, Ordering::Relaxed);
                    absent.lecture.store(0, Ordering::Release);
                    Some(Arrivee::Lu { absent, pixels })
                }
            }
            Travail::Verifier { id, version, fichiers } => {
                let identite = fichiers.first().and_then(|p| Identite::de(p)).filter(|_| fichiers.iter().all(|p| p.is_file()));
                Some(Arrivee::Verifie { id, version, identite })
            }
        };
        if let Some(a) = arrivee {
            t.boite.lock().unwrap_or_else(|e| e.into_inner()).push(a);
            if let Some(ctx) = &t.reveil {
                ctx.request_repaint();
            }
        }
        EN_COURS.fetch_sub(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------------- le suivi d'un document

/// Ce qu'un document suit de ses dessins (rangé avec son onglet, `etat_projet!`). Le travail est dans `residenceapp.rs`.
#[derive(Default)]
pub(crate) struct Suivi {
    /// Images d'écran où ce document a suivi ses dessins.
    pub tour: u64,
    pub boite: Boite,
    /// Pour chaque dessin, le dernier tour où il était voulu (les moins récents partent d'abord).
    pub vu: HashMap<CelId, u64>,
    /// Tuiles de chaque dessin présent, à sa version (comptées une fois par version).
    pub poids: HashMap<CelId, (u64, usize)>,
    /// Le dernier tour où le retour d'un dessin a été demandé.
    pub demandes: HashMap<CelId, u64>,
    /// Les dessins qu'on n'a pas pu relire : le tour de l'échec (on réessaie plus tard) et pourquoi.
    pub echecs: HashMap<CelId, (u64, String)>,
    /// Identité vérifiée du fichier d'un dessin à laisser : sa version, l'identité, le tour de la vérification.
    pub verifies: HashMap<CelId, (u64, Identite, u64)>,
    /// Vérifications en route.
    pub verifs: std::collections::HashSet<CelId>,
    /// Le dernier tour où le budget a été tenu.
    pub controle: u64,
    /// Ce que le diagnostic montre.
    pub stats: Stats,
    /// Une annulation (ou un rétablissement) qui attend le retour de ses dessins : rétablir ?, combien de fois, et le
    /// numéro de l'historique quand elle a été demandée (une autre étape entre-temps l'oublie).
    pub attente_historique: Option<(bool, u32, u64)>,
    /// Des dessins qu'une tâche de l'interface attend (vignettes d'une lecture du storyboard…) : voulus à la prochaine
    /// image d'écran, puis oubliés (la tâche les redemande tant qu'elle attend).
    pub en_plus: Vec<CelId>,
    /// Une commande qui attend les dessins du plan courant : le plan (son numéro) et la commande.
    pub action_en_attente: Option<(u64, String)>,
    /// Tous les dessins sont voulus (une opération sur tout le projet attend, comme changer sa taille).
    pub tout_garder: bool,
}

/// Ce que le diagnostic des performances montre des dessins en mémoire.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Stats {
    /// Octets des pixels des dessins présents (tuiles des images, des strates Fond).
    pub octets_presents: u64,
    pub presents: usize,
    pub absents: usize,
    /// Dessins revenus et laissés depuis l'ouverture.
    pub revenus: usize,
    pub laisses: usize,
    pub budget: u64,
}

impl Suivi {
    /// Le poids d'un dessin présent, en tuiles (compté une fois par version).
    pub(crate) fn tuiles(&mut self, id: CelId, c: &Cel) -> usize {
        match self.poids.get(&id) {
            Some(&(v, n)) if v == c.version => n,
            _ => {
                let n = tuiles_de(&c.raster) + c.fond.as_ref().map_or(0, |s| tuiles_de(&s.peinture.raster) + tuiles_de(&s.coupe));
                self.poids.insert(id, (c.version, n));
                n
            }
        }
    }
}
