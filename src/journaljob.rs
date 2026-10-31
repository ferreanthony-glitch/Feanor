//! La tâche du journal des différences (`journal.rs` décrit, ceci calcule et écrit).
//!
//! Règle « sans latence » : l'interface ne calcule aucune image. Elle prépare, d'un coup et en quelques millisecondes, une `Demande`
//! (la comparaison des listes de plans, les notes, et pour le côté « État actuel » une petite copie de chaque plan à montrer : les
//! tuiles des dessins sont partagées, rien n'est recopié). Un fil à part fait le reste :
//!
//! 1. il relit l'instantané de chaque envoi une fois (`envois::lire_instantane`) ;
//! 2. pour chaque plan à montrer, il compose quelques images de chaque côté (la première, celle du milieu, la dernière : le même calcul
//!    que les vignettes et la fenêtre « Comparer », effets compris) et garde les deux qui diffèrent le plus, avec la part des pixels qui
//!    diffèrent ; plusieurs plans en parallèle ;
//! 3. il écrit la page HTML et le PDF.
//!
//! Les fichiers sont d'abord écrits sous un nom en `.part` puis renommés : une tâche arrêtée ou ratée ne laisse **aucun fichier** ; un
//! journal déjà écrit sous le même nom n'est remplacé qu'à la toute fin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use eframe::egui::{ColorImage, Context};

use crate::comparer;
use crate::document::Document;
use crate::envois::{self, Envoi, InstantaneLu};
use crate::export;
use crate::journal::{self, Genre, Journal, Ligne, Rendu, Vignette};

/// Largeur de calcul des images avant / après (la page et le PDF les réduisent).
const LARGEUR: usize = 640;
/// Largeur et qualité des images de la page HTML.
const HTML_LARGEUR: u32 = 480;
const HTML_QUALITE: u8 = 72;
/// Largeur des images du PDF.
const PDF_LARGEUR: u32 = 640;

/// D'où viennent les plans d'un côté.
pub enum Cote {
    /// L'état d'un envoi, relu dans son instantané (`dossier` : celui du projet).
    Envoi { dossier: PathBuf, envoi: Box<Envoi> },
    /// L'état d'aujourd'hui : une petite copie de chaque plan à montrer (`comparer::petit_projet`).
    Docs(HashMap<u64, Document>),
}

/// Tout ce que la tâche a besoin de savoir.
pub struct Demande {
    pub journal: Journal,
    pub a: Cote,
    pub b: Cote,
    /// Où écrire la page et le PDF (au moins l'un des deux).
    pub html: Option<PathBuf>,
    pub pdf: Option<PathBuf>,
}

/// Ce qui est écrit.
#[derive(Clone, Debug)]
pub struct Rapport {
    pub html: Option<PathBuf>,
    pub pdf: Option<PathBuf>,
    pub modifies: usize,
    pub ajoutes: usize,
    pub retires: usize,
    pub deplaces: usize,
    pub notes: usize,
    pub octets: u64,
}

/// Un côté prêt à être lu.
enum Lu {
    Instantane(Box<InstantaneLu>),
    Docs(HashMap<u64, Document>),
    Rien,
}

enum Tenu<'a> {
    Possede(Box<Document>),
    Prete(&'a Document),
}

impl Tenu<'_> {
    fn doc(&self) -> &Document {
        match self {
            Tenu::Possede(d) => d,
            Tenu::Prete(d) => d,
        }
    }
}

impl Lu {
    fn ouvrir(&self, id: u64) -> Option<Tenu<'_>> {
        let t = match self {
            Lu::Instantane(i) => Tenu::Possede(Box::new(i.plan(id).ok()?.doc)),
            Lu::Docs(m) => Tenu::Prete(m.get(&id)?),
            Lu::Rien => return None,
        };
        (!t.doc().panels.is_empty()).then_some(t)
    }
}

fn lire(c: Cote, avertissements: &mut Vec<String>) -> Lu {
    match c {
        Cote::Docs(m) => Lu::Docs(m),
        Cote::Envoi { dossier, envoi } => match envois::lire_instantane(&dossier, &envoi) {
            Ok(i) => Lu::Instantane(Box::new(i)),
            Err(e) => {
                avertissements.push(format!("Pas d'images pour l'envoi « {} » : {e}", envoi.nom));
                Lu::Rien
            }
        },
    }
}

/// Les couples (image d'avant, image d'après) à comparer : le début, le milieu et la fin de chaque plan.
pub(crate) fn echantillons(na: usize, nb: usize) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = Vec::new();
    for p in [0.0_f32, 0.5, 1.0] {
        let f = |n: usize| (p * n.saturating_sub(1) as f32).round() as usize;
        let c = (f(na), f(nb));
        if !v.contains(&c) {
            v.push(c);
        }
    }
    v
}

fn vignette(img: &ColorImage, html: bool, pdf: bool) -> Vignette {
    Vignette {
        html: html.then(|| export::jpeg_on_white(&img.pixels, img.size, HTML_LARGEUR, HTML_QUALITE).ok().map(|(_, _, j)| j)).flatten(),
        pdf: pdf.then(|| export::board_image_at(&img.pixels, img.size, PDF_LARGEUR).ok()).flatten(),
    }
}

/// Les images d'un plan : les deux côtés, à l'image où ils diffèrent le plus.
fn rendre_ligne(l: &Ligne, a: &Lu, b: &Lu, html: bool, pdf: bool) -> Rendu {
    let da = (l.genre != Genre::Ajoute).then(|| a.ouvrir(l.id)).flatten();
    let db = (l.genre != Genre::Retire).then(|| b.ouvrir(l.id)).flatten();
    let frames = |t: &Option<Tenu>| t.as_ref().map(|t| t.doc().panels[0].frame_count().max(1));
    let paires = match (frames(&da), frames(&db)) {
        (Some(na), Some(nb)) => echantillons(na, nb),
        (None, None) => Vec::new(),
        _ => vec![(0, 0)],
    };
    let mut meilleur: Option<(Option<f32>, Option<ColorImage>, Option<ColorImage>)> = None;
    for (fa, fb) in paires {
        let ia = da.as_ref().map(|t| comparer::rendre(t.doc(), fa, LARGEUR));
        let ib = db.as_ref().map(|t| comparer::rendre(t.doc(), fb, LARGEUR));
        let diff = match (&ia, &ib) {
            (Some(x), Some(y)) => Some(comparer::difference(x, y, 8)),
            _ => None,
        };
        if meilleur.as_ref().is_none_or(|(d, _, _)| diff > *d) {
            meilleur = Some((diff, ia, ib));
        }
    }
    match meilleur {
        Some((pct, ia, ib)) => Rendu { avant: ia.map(|i| vignette(&i, html, pdf)), apres: ib.map(|i| vignette(&i, html, pdf)), pct },
        None => Rendu::default(),
    }
}

fn part_de(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// Le travail du fil.
pub(crate) fn travailler(demande: Demande, annule: &AtomicBool, fait: &AtomicUsize) -> Result<Rapport, String> {
    let Demande { mut journal, a, b, html, pdf } = demande;
    let arret = || -> Result<(), String> {
        if annule.load(Ordering::Relaxed) {
            Err("Journal des différences arrêté.".into())
        } else {
            Ok(())
        }
    };
    arret()?;
    let (lu_a, lu_b) = (lire(a, &mut journal.avertissements), lire(b, &mut journal.avertissements));
    let taches = journal.diff.a_rendre();
    let (veut_html, veut_pdf) = (html.is_some(), pdf.is_some());
    let faits: Vec<Option<Rendu>> = crate::parallele::par_map(&taches, 3, |_, &i| {
        if annule.load(Ordering::Relaxed) {
            return None;
        }
        let r = rendre_ligne(&journal.diff.lignes[i], &lu_a, &lu_b, veut_html, veut_pdf);
        fait.fetch_add(1, Ordering::Relaxed);
        Some(r)
    });
    arret()?;
    let mut rendus: Vec<Option<Rendu>> = (0..journal.diff.lignes.len()).map(|_| None).collect();
    for (&i, r) in taches.iter().zip(faits) {
        rendus[i] = r;
    }
    // Les fichiers : d'abord sous `.part`, puis renommés ensemble.
    let sorties: Vec<(PathBuf, Vec<u8>)> = html
        .iter()
        .map(|p| (p.clone(), journal::page(&journal, &rendus).into_bytes()))
        .chain(pdf.iter().map(|p| (p.clone(), journal::pdf(&journal, &rendus))))
        .collect();
    let octets = sorties.iter().map(|(_, b)| b.len() as u64).sum();
    let mut parts: Vec<PathBuf> = Vec::new();
    let nettoyer = |parts: &[PathBuf]| {
        for p in parts {
            let _ = std::fs::remove_file(p);
        }
    };
    for (p, bytes) in &sorties {
        let part = part_de(p);
        parts.push(part.clone());
        if let Err(e) = std::fs::write(&part, bytes) {
            nettoyer(&parts);
            return Err(format!("{} : {e}", part.display()));
        }
    }
    if let Err(e) = arret() {
        nettoyer(&parts);
        return Err(e);
    }
    let mut crees: Vec<PathBuf> = Vec::new();
    for (p, _) in &sorties {
        let existait = p.exists();
        if let Err(e) = std::fs::rename(part_de(p), p) {
            nettoyer(&parts);
            // Les fichiers neufs de ce journal ne restent pas sans leur pendant ; un ancien fichier remplacé, lui, ne revient pas.
            nettoyer(&crees);
            return Err(format!("{} : {e}", p.display()));
        }
        if !existait {
            crees.push(p.clone());
        }
    }
    let d = &journal.diff;
    Ok(Rapport { html, pdf, modifies: d.modifies(), ajoutes: d.ajoutes(), retires: d.retires(), deplaces: d.deplaces(), notes: journal.notes.len(), octets })
}

/// La tâche en cours.
pub struct JournalJob {
    handle: Option<JoinHandle<Result<Rapport, String>>>,
    annule: Arc<AtomicBool>,
    fait: Arc<AtomicUsize>,
    total: usize,
    /// Le compte rendu, quand la tâche est finie (réussie, ratée ou arrêtée).
    pub outcome: Option<Result<Rapport, String>>,
}

impl JournalJob {
    /// Lance le fil. `ctx` : de quoi réveiller l'interface à la fin.
    pub fn start(demande: Demande, ctx: Option<Context>) -> Result<JournalJob, String> {
        if demande.html.is_none() && demande.pdf.is_none() {
            return Err("Choisis au moins un format : PDF ou page HTML.".into());
        }
        let total = demande.journal.diff.a_rendre().len();
        let annule = Arc::new(AtomicBool::new(false));
        let fait = Arc::new(AtomicUsize::new(0));
        let (a2, f2) = (annule.clone(), fait.clone());
        let handle = std::thread::Builder::new()
            .name("journal".into())
            .spawn(move || {
                let r = travailler(demande, &a2, &f2);
                if let Some(c) = ctx {
                    c.request_repaint();
                }
                r
            })
            .map_err(|e| format!("Le journal n'a pas pu démarrer : {e}"))?;
        Ok(JournalJob { handle: Some(handle), annule, fait, total, outcome: None })
    }

    pub fn running(&self) -> bool {
        self.outcome.is_none()
    }

    /// Prend le résultat du fil s'il a fini (à appeler à chaque image d'écran).
    pub fn poll(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        if self.handle.as_ref().is_some_and(|h| h.is_finished()) {
            let r = self.handle.take().map(|h| h.join().unwrap_or_else(|_| Err("le journal s'est arrêté".into())));
            self.outcome = r;
        }
    }

    /// (plans faits, plans à faire).
    pub fn progress(&self) -> (usize, usize) {
        (self.fait.load(Ordering::Relaxed).min(self.total), self.total)
    }

    /// Demande l'arrêt : le fil s'arrête dès qu'il le voit et n'écrit rien (le compte rendu arrive au prochain `poll`).
    pub fn cancel(&mut self) {
        self.annule.store(true, Ordering::Relaxed);
    }

    /// Arrête et attend la fin du fil (le paquet de livraison efface son dossier de travail juste après).
    pub fn cancel_and_wait(&mut self) {
        self.cancel();
        if let Some(h) = self.handle.take() {
            self.outcome = Some(h.join().unwrap_or_else(|_| Err("le journal s'est arrêté".into())));
        }
    }
}

impl Drop for JournalJob {
    fn drop(&mut self) {
        // Fermeture de SHOT pendant le journal : pas de demi-fichier.
        if self.handle.is_some() {
            self.cancel_and_wait();
        }
    }
}
