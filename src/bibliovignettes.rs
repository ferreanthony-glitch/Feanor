//! Les vignettes du panneau Bibliothèque : une petite image par fichier visible, faite **sur des fils à part**
//! (deux), jamais par l'interface.
//!
//! - L'interface dit chaque image d'écran quels fichiers elle montre (`souhaiter`) : la file des demandes
//!   est remplacée par celle-là, dans l'ordre où on les voit. Ce qui a défilé hors de l'écran avant d'être fait
//!   n'est jamais fait ; au plus `FILE_MAX` demandes attendent.
//! - Un fil lit l'image (PNG, JPEG), ou demande à ffmpeg une image de la vidéo, ou lit la durée du son, puis
//!   rend une petite image de `COTE` points au plus, avec la durée et la taille quand elles viennent avec.
//! - L'interface n'envoie à la carte graphique que `PAR_IMAGE` vignettes à la fois, et garde en mémoire
//!   graphique `BUDGET` octets au plus (les moins récemment vues partent : `references::Lru`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};

use crate::biblio::Genre;
use crate::references::Lru;

/// Côté le plus long d'une vignette faite, en pixels.
pub const COTE: u32 = 256;
/// Demandes qui attendent au plus.
pub const FILE_MAX: usize = 96;
/// Mémoire graphique gardée pour les vignettes, en octets (une vignette de 256 points fait 256 Ko).
pub const BUDGET: usize = 64 << 20;
/// Vignettes envoyées à la carte graphique par image d'écran au plus.
const PAR_IMAGE: usize = 4;
/// Les images plus lourdes que ça (en octets) n'ont pas de vignette : les lire serait trop long.
const IMAGE_MAX: u64 = 256 << 20;
/// Les durées et tailles gardées au plus.
const META_MAX: usize = 50_000;

/// Ce qu'on demande aux fils.
#[derive(Clone, Debug, PartialEq)]
pub struct Demande {
    pub chemin: PathBuf,
    pub genre: Genre,
}

/// Ce que les fils rendent.
pub struct Resultat {
    pub chemin: PathBuf,
    /// Largeur, hauteur, pixels RVBA ; rien pour un son, ou un fichier illisible.
    pub image: Option<(usize, usize, Vec<u8>)>,
    pub meta: Meta,
}

/// Ce qu'on a appris d'un fichier en passant.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Meta {
    pub duree: Option<f64>,
    pub dims: Option<(u32, u32)>,
    /// La lecture a échoué (fichier abîmé, vidéo sans ffmpeg…).
    pub illisible: bool,
}

/// Où trouver ffmpeg : cherché une seule fois, la première fois qu'une vidéo en a besoin (sur un fil à part).
#[derive(Clone)]
pub struct Ffmpeg {
    reglage: Option<PathBuf>,
    trouve: Arc<OnceLock<Option<PathBuf>>>,
}

impl Ffmpeg {
    pub fn new(reglage: Option<PathBuf>) -> Ffmpeg {
        Ffmpeg { reglage, trouve: Arc::new(OnceLock::new()) }
    }

    /// Un ffmpeg connu d'avance (les essais).
    #[cfg(test)]
    pub fn connu(chemin: Option<PathBuf>) -> Ffmpeg {
        let f = Ffmpeg::new(chemin.clone());
        let _ = f.trouve.set(chemin);
        f
    }

    /// Le chemin de ffmpeg ; la recherche (qui lit des dossiers) se fait une fois, ici : à ne pas appeler depuis l'interface.
    pub fn chemin(&self) -> Option<PathBuf> {
        self.trouve.get_or_init(|| crate::export::find_ffmpeg(self.reglage.as_deref())).clone()
    }
}

/// Fait la vignette d'un fichier (sur un fil de fond).
pub fn produire(d: &Demande, ffmpeg: &Ffmpeg, annule: &AtomicBool) -> Resultat {
    let mut meta = Meta::default();
    let mut image = None;
    match d.genre {
        Genre::Image => {
            meta.dims = image::image_dimensions(&d.chemin).ok();
            let lourde = std::fs::metadata(&d.chemin).map_or(true, |m| m.len() > IMAGE_MAX);
            if !lourde {
                match image::open(&d.chemin) {
                    Ok(img) => {
                        meta.dims = Some((img.width(), img.height()));
                        let petite = if img.width().max(img.height()) > COTE { img.thumbnail(COTE, COTE) } else { img };
                        let rgba = petite.to_rgba8();
                        image = Some((rgba.width() as usize, rgba.height() as usize, rgba.into_raw()));
                    }
                    Err(_) => meta.illisible = true,
                }
            }
            meta.illisible |= meta.dims.is_none();
        }
        Genre::Son => {
            meta.duree = crate::bruitages::probe_seconds(&d.chemin);
            meta.illisible = meta.duree.is_none();
        }
        Genre::Video => match ffmpeg.chemin() {
            None => meta.illisible = true,
            Some(f) => match crate::bibliovideo::lire_infos(&f, &d.chemin, annule) {
                Ok(infos) => {
                    meta.dims = Some((infos.largeur, infos.hauteur));
                    meta.duree = (infos.duree > 0.0).then_some(infos.duree);
                    // Une image au dixième de la durée (au plus à 2 s) : rarement le noir d'ouverture.
                    let t = (infos.duree * 0.1).min(2.0);
                    match crate::bibliovideo::une_image(&f, &d.chemin, t, COTE, annule).or_else(|_| crate::bibliovideo::une_image(&f, &d.chemin, 0.0, COTE, annule)) {
                        Ok(i) => image = Some(i),
                        Err(_) => meta.illisible = true,
                    }
                }
                Err(_) => meta.illisible = true,
            },
        },
    }
    Resultat { chemin: d.chemin.clone(), image, meta }
}

/// La file partagée avec les fils.
struct File {
    demandes: Mutex<VecDeque<Demande>>,
    reveil: Condvar,
}

/// Les vignettes : la file, les fils, et ce qui est revenu.
pub struct Vignettes {
    file: Arc<File>,
    arret: Arc<AtomicBool>,
    rx: Receiver<Resultat>,
    /// Les chemins demandés dont le résultat n'est pas encore revenu.
    en_attente: HashSet<PathBuf>,
    /// Revenus des fils, pas encore envoyés à la carte graphique.
    prets: VecDeque<Resultat>,
    textures: Lru<PathBuf, TextureHandle>,
    /// Durées et tailles lues en passant.
    pub meta: HashMap<PathBuf, Meta>,
    /// Les fichiers sans vignette possible (on ne les redemande pas).
    sans: HashSet<PathBuf>,
    /// Les vignettes faites depuis le début (les essais).
    pub faites: usize,
}

impl Vignettes {
    pub fn new(ffmpeg: Ffmpeg) -> Vignettes {
        Self::avec(ffmpeg, 2)
    }

    /// Avec `fils` fils de lecture (les essais).
    pub fn avec(ffmpeg: Ffmpeg, fils: usize) -> Vignettes {
        let file = Arc::new(File { demandes: Mutex::new(VecDeque::new()), reveil: Condvar::new() });
        let arret = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        for i in 0..fils.max(1) {
            let (file, arret, tx, ffmpeg) = (file.clone(), arret.clone(), tx.clone(), ffmpeg.clone());
            let _ = std::thread::Builder::new().name(format!("bibliotheque-vignettes-{i}")).spawn(move || {
                crate::parallele::baisser_la_priorite_ici();
                loop {
                    let demande = {
                        let mut q = file.demandes.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if arret.load(Ordering::Relaxed) {
                                return;
                            }
                            if let Some(d) = q.pop_front() {
                                break d;
                            }
                            q = file.reveil.wait(q).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let r = produire(&demande, &ffmpeg, &arret);
                    if tx.send(r).is_err() {
                        return;
                    }
                }
            });
        }
        Vignettes { file, arret, rx, en_attente: HashSet::new(), prets: VecDeque::new(), textures: Lru::default(), meta: HashMap::new(), sans: HashSet::new(), faites: 0 }
    }

    /// Les fichiers montrés à l'écran, dans l'ordre où on les voit : la file devient celle-là (sans ce qui est déjà fait,
    /// déjà demandé ou sans vignette possible).
    pub fn souhaiter(&mut self, vus: &[Demande]) {
        let mut q = self.file.demandes.lock().unwrap_or_else(|e| e.into_inner());
        let vus_set: HashSet<&Path> = vus.iter().map(|d| d.chemin.as_path()).collect();
        // Ce qui a défilé hors de l'écran avant d'être pris n'est plus demandé.
        let avant: Vec<PathBuf> = q.iter().map(|d| d.chemin.clone()).collect();
        q.retain(|d| vus_set.contains(d.chemin.as_path()));
        let reste: HashSet<PathBuf> = q.iter().map(|d| d.chemin.clone()).collect();
        for p in avant {
            if !reste.contains(&p) {
                self.en_attente.remove(&p);
            }
        }
        let mut ajout = false;
        for d in vus {
            if q.len() >= FILE_MAX {
                break;
            }
            let fait = self.textures_contient(&d.chemin) || self.sans.contains(&d.chemin);
            if fait || self.en_attente.contains(&d.chemin) {
                continue;
            }
            self.en_attente.insert(d.chemin.clone());
            q.push_back(d.clone());
            ajout = true;
        }
        drop(q);
        if ajout {
            self.file.reveil.notify_all();
        }
    }

    fn textures_contient(&self, chemin: &Path) -> bool {
        self.textures.keys().any(|k| k.as_path() == chemin)
    }

    /// Reprend ce que les fils ont fait ; envoie quelques vignettes à la carte graphique. Vrai si quelque chose est arrivé.
    pub fn relever(&mut self, ctx: &egui::Context) -> bool {
        let mut change = false;
        while let Ok(r) = self.rx.try_recv() {
            self.en_attente.remove(&r.chemin);
            self.faites += 1;
            change = true;
            if self.meta.len() >= META_MAX {
                self.meta.clear();
            }
            self.meta.insert(r.chemin.clone(), r.meta);
            if r.image.is_some() {
                self.prets.push_back(r);
            } else {
                self.sans.insert(r.chemin);
            }
        }
        for _ in 0..PAR_IMAGE {
            let Some(r) = self.prets.pop_front() else { break };
            if let Some((w, h, rgba)) = r.image {
                let octets = rgba.len();
                let tex = ctx.load_texture(format!("biblio {}", r.chemin.display()), ColorImage::from_rgba_unmultiplied([w, h], &rgba), TextureOptions::LINEAR);
                self.textures.insert(r.chemin, tex, octets);
                change = true;
            }
        }
        change
    }

    /// Des vignettes attendent encore (faites ou à faire) : l'interface se redessine.
    pub fn occupe(&self) -> bool {
        !self.en_attente.is_empty() || !self.prets.is_empty()
    }

    /// La vignette de `chemin`, si elle est à la carte graphique (la marque comme vue pendant cette image).
    pub fn texture(&mut self, chemin: &PathBuf) -> Option<&TextureHandle> {
        self.textures.get(chemin)
    }

    /// Une nouvelle image d'écran : ce qui sert pendant elle ne part pas du cache.
    pub fn debut_image(&mut self) {
        self.textures.tick();
    }

    /// Fin d'image d'écran : les vignettes les moins récemment vues partent au-delà du budget.
    pub fn nettoyer(&mut self) {
        self.nettoyer_avec(BUDGET);
    }

    /// `nettoyer` avec un autre budget (les essais).
    pub fn nettoyer_avec(&mut self, budget: usize) {
        self.textures.evict(budget);
    }

    /// Octets de vignettes gardés à la carte graphique.
    #[cfg(test)]
    pub fn octets(&self) -> usize {
        self.textures.total()
    }

    /// Les vignettes demandées et pas encore faites sont abandonnées (changement de dossier) ; ce qui est fait reste.
    pub fn abandonner_la_file(&mut self) {
        let mut q = self.file.demandes.lock().unwrap_or_else(|e| e.into_inner());
        for d in q.drain(..) {
            self.en_attente.remove(&d.chemin);
        }
    }

    /// Tout est oublié : le dossier a changé sur le disque (bouton Actualiser).
    pub fn tout_oublier(&mut self) {
        self.abandonner_la_file();
        self.textures = Lru::default();
        self.meta.clear();
        self.sans.clear();
        self.prets.clear();
    }

    /// Le nombre de vignettes à la carte graphique (les essais).
    #[cfg(test)]
    pub fn nombre(&self) -> usize {
        self.textures.len()
    }

    #[cfg(test)]
    pub fn attend(&self) -> usize {
        self.en_attente.len()
    }
}

impl Drop for Vignettes {
    fn drop(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
        self.file.reveil.notify_all();
    }
}
