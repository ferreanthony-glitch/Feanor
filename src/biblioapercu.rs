//! L'aperçu intégré du panneau Bibliothèque : une image qu'on regarde (zoom à la molette), une vidéo qu'on lit
//! (lecture, pause, barre de temps, image par image, son, boucle), un son qu'on écoute (forme d'onde, barre de temps,
//! volume, boucle). **Un seul aperçu à la fois** : en ouvrir un autre ferme le précédent (`Apercu::fermer`).
//!
//! Rien ne se décode sur le fil de l'interface :
//!
//! - l'image est lue et réduite sur un fil à part (`references::decode`) ;
//! - le son est décodé sur un fil à part (`bruitages::decode_file`), puis joué par la sortie audio de SHOT ;
//! - la vidéo est confiée à un fil qui lance ffmpeg (`bibliovideo.rs`) : une image à la demande (barre de temps,
//!   image par image), ou un flux d'images à la cadence de la vidéo pendant la lecture ; son son est extrait par un
//!   autre fil et joué par la même sortie audio. La lecture a une horloge (`Lecture`) : celle de la carte son quand
//!   elle joue, celle de l'ordinateur sinon.
//!
//! La sortie audio est cachée derrière `SortieAudio` : les essais en glissent une fausse et ne produisent aucun son.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Vec2};

use crate::audio::{AudioEngine, MixClip, Pcm, RATE};
use crate::bibliovideo::{self, Infos};
use crate::bibliovignettes::Ffmpeg;

/// Les sons plus lourds que ça (en octets) n'ont pas d'aperçu : les décoder en mémoire serait trop long.
pub const SON_MAX: u64 = 400 << 20;

// ------------------------------------------------------------------------------------ sortie audio

/// Où part le son de l'aperçu : la sortie audio de SHOT, ou un faux dans les essais.
pub trait SortieAudio {
    /// Joue le son depuis `depuis` (échantillons) ; faux si rien ne peut sonner (pas de sortie audio).
    fn jouer(&mut self, clips: Vec<MixClip>, depuis: u64, fin: u64, boucle: bool) -> bool;
    fn arreter(&mut self);
    /// Position de lecture, en échantillons à `RATE`.
    fn position(&self) -> f64;
    /// Le son joué sonne-t-il encore ?
    fn sonne(&self) -> bool;
    /// Suit un changement fait pendant la lecture (volume, boucle).
    fn regler(&mut self, clips: Vec<MixClip>, boucle: bool);
}

/// La sortie audio de SHOT.
pub struct Moteur<'a>(pub &'a AudioEngine);

impl SortieAudio for Moteur<'_> {
    fn jouer(&mut self, clips: Vec<MixClip>, depuis: u64, fin: u64, boucle: bool) -> bool {
        self.0.play(clips, depuis, (0, fin), boucle);
        true
    }
    fn arreter(&mut self) {
        self.0.stop();
    }
    fn position(&self) -> f64 {
        self.0.position()
    }
    fn sonne(&self) -> bool {
        self.0.is_playing()
    }
    fn regler(&mut self, clips: Vec<MixClip>, boucle: bool) {
        self.0.update(clips, boucle);
    }
}

/// Pas de sortie audio : rien ne sonne, l'horloge de l'ordinateur compte le temps.
pub struct Muet;

impl SortieAudio for Muet {
    fn jouer(&mut self, _: Vec<MixClip>, _: u64, _: u64, _: bool) -> bool {
        false
    }
    fn arreter(&mut self) {}
    fn position(&self) -> f64 {
        0.0
    }
    fn sonne(&self) -> bool {
        false
    }
    fn regler(&mut self, _: Vec<MixClip>, _: bool) {}
}

/// Ce que joue la sortie : le son entier, au volume demandé (au carré : l'oreille entend en puissance).
fn extraits(pcm: &Arc<Pcm>, volume: f32) -> (Vec<MixClip>, u64) {
    let len = pcm.frames() as u64;
    (vec![MixClip::plain(pcm.clone(), 0, 0, len, volume.clamp(0.0, 1.0).powi(2))], len)
}

// ------------------------------------------------------------------------------------ la lecture

/// La lecture d'un son : où elle en est, et l'horloge qui la compte.
pub struct Lecture {
    pub pcm: Option<Arc<Pcm>>,
    /// L'utilisateur veut que ça joue.
    pub joue: bool,
    /// La position (secondes) quand ça ne joue pas, ou au départ de la lecture en cours.
    pos: f64,
    depart: Option<Instant>,
    /// Le moteur audio a été lancé pour cette lecture.
    moteur: bool,
    pub volume: f32,
    /// Le moteur rejoue le son en boucle de lui-même (pour un son ; pour une vidéo, c'est l'aperçu qui reprend au début).
    pub boucle_moteur: bool,
}

impl Lecture {
    pub fn new(volume: f32, boucle_moteur: bool) -> Lecture {
        Lecture { pcm: None, joue: false, pos: 0.0, depart: None, moteur: false, volume, boucle_moteur }
    }

    /// L'instant de la lecture, en secondes.
    pub fn temps(&self, sortie: &dyn SortieAudio) -> f64 {
        if !self.joue {
            return self.pos;
        }
        if self.moteur && sortie.sonne() {
            return sortie.position() / RATE as f64;
        }
        self.pos + self.depart.map_or(0.0, |d| d.elapsed().as_secs_f64())
    }

    /// Joue depuis `t` secondes.
    pub fn demarrer(&mut self, t: f64, sortie: &mut dyn SortieAudio) {
        self.pos = t.max(0.0);
        self.joue = true;
        self.depart = Some(Instant::now());
        self.moteur = false;
        if let Some(pcm) = &self.pcm {
            let (clips, len) = extraits(pcm, self.volume);
            let depuis = ((self.pos * RATE as f64) as u64).min(len);
            if depuis < len {
                self.moteur = sortie.jouer(clips, depuis, len, self.boucle_moteur);
            }
        }
    }

    pub fn pause(&mut self, sortie: &mut dyn SortieAudio) {
        if self.joue {
            self.pos = self.temps(sortie);
        }
        if self.moteur {
            sortie.arreter();
        }
        self.joue = false;
        self.moteur = false;
        self.depart = None;
    }

    /// La sortie audio a été prise par autre chose (le projet joue) : on s'arrête sans y toucher.
    pub fn pause_sans_moteur(&mut self, sortie: &dyn SortieAudio) {
        if self.joue {
            self.pos = self.temps(sortie);
        }
        self.joue = false;
        self.moteur = false;
        self.depart = None;
    }

    /// Va à `t` secondes ; ça continue de jouer si ça jouait.
    pub fn aller(&mut self, t: f64, sortie: &mut dyn SortieAudio) {
        if self.joue {
            self.demarrer(t, sortie);
        } else {
            self.pos = t.max(0.0);
        }
    }

    /// Le volume ou la boucle ont changé.
    pub fn regler(&mut self, volume: f32, boucle_moteur: bool, sortie: &mut dyn SortieAudio) {
        self.volume = volume;
        self.boucle_moteur = boucle_moteur;
        if self.moteur {
            if let Some(pcm) = &self.pcm {
                sortie.regler(extraits(pcm, volume).0, boucle_moteur);
            }
        }
    }

    /// Le son arrive alors que ça joue déjà (la vidéo se lisait sans lui) : il se joint à l'instant courant.
    pub fn joindre_le_son(&mut self, pcm: Arc<Pcm>, sortie: &mut dyn SortieAudio) {
        self.pcm = Some(pcm);
        if self.joue {
            let t = self.temps(sortie);
            self.demarrer(t, sortie);
        }
    }

    /// À chaque image d'écran. Vrai quand la lecture vient d'arriver au bout de `duree` secondes.
    pub fn suivre(&mut self, sortie: &dyn SortieAudio, duree: f64) -> bool {
        if !self.joue {
            return false;
        }
        if self.moteur && !sortie.sonne() {
            // Le son a fini (il peut être plus court que la vidéo) : l'horloge de l'ordinateur prend la suite.
            self.pos = self.pcm.as_ref().map_or(self.pos, |p| p.seconds());
            self.depart = Some(Instant::now());
            self.moteur = false;
        }
        let t = self.temps(sortie);
        if duree > 0.0 && t >= duree - 0.005 {
            self.joue = false;
            self.pos = duree;
            self.moteur = false;
            self.depart = None;
            return true;
        }
        false
    }
}

// ------------------------------------------------------------------------------------ l'image

type ImageLue = Result<(usize, usize, Vec<u8>, (u32, u32)), String>;

pub struct ApercuImage {
    pub chemin: PathBuf,
    rx: Option<Receiver<ImageLue>>,
    pub texture: Option<TextureHandle>,
    /// Taille de l'image d'origine.
    pub dims: Option<(u32, u32)>,
    pub erreur: Option<String>,
    /// 1 : l'image tient dans la zone ; plus : on a zoomé.
    pub zoom: f32,
    /// Décalage de l'image, en points de la zone.
    pub decalage: Vec2,
}

impl ApercuImage {
    pub fn ouvrir(chemin: PathBuf, cote_max: u32) -> ApercuImage {
        let (tx, rx) = channel();
        let p = chemin.clone();
        let lourd = std::fs::metadata(&p).map_or(true, |m| m.len() > 256 << 20);
        let fil = std::thread::Builder::new().name("bibliotheque-image".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            let r: ImageLue = if lourd {
                Err("Cette image est trop lourde pour l'aperçu.".into())
            } else {
                let dims = image::image_dimensions(&p).unwrap_or((0, 0));
                crate::references::decode(&p, cote_max, false).map(|(w, h, px)| (w, h, px, if dims.0 > 0 { dims } else { (w as u32, h as u32) })).ok_or_else(|| "Impossible de lire cette image.".to_string())
            };
            let _ = tx.send(r);
        });
        let (rx, erreur) = if fil.is_ok() { (Some(rx), None) } else { (None, Some("Impossible de lire cette image.".to_string())) };
        ApercuImage { chemin, rx, texture: None, dims: None, erreur, zoom: 1.0, decalage: Vec2::ZERO }
    }

    pub fn charge(&self) -> bool {
        self.rx.is_some()
    }

    /// Reprend l'image lue ; vrai si elle vient d'arriver.
    pub fn suivre(&mut self, ctx: &egui::Context) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(Ok((w, h, px, dims))) => {
                self.texture = Some(ctx.load_texture(format!("apercu {}", self.chemin.display()), ColorImage::from_rgba_unmultiplied([w, h], &px), TextureOptions::LINEAR));
                self.dims = Some(dims);
                self.rx = None;
                true
            }
            Ok(Err(e)) => {
                self.erreur = Some(e);
                self.rx = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.erreur = Some("La lecture de l'image s'est interrompue.".into());
                self.rx = None;
                true
            }
        }
    }

    /// Zoom autour d'un point (`ancre`, en points depuis le centre de la zone) : le point sous la souris reste en place.
    pub fn zoomer(&mut self, facteur: f32, ancre: Vec2) {
        let nouveau = (self.zoom * facteur).clamp(1.0, 32.0);
        let k = nouveau / self.zoom;
        self.decalage = ancre - (ancre - self.decalage) * k;
        self.zoom = nouveau;
        if nouveau <= 1.0 {
            self.decalage = Vec2::ZERO;
        }
    }

    pub fn recentrer(&mut self) {
        self.zoom = 1.0;
        self.decalage = Vec2::ZERO;
    }
}

// ------------------------------------------------------------------------------------ le son

type SonLu = Result<Arc<Pcm>, String>;

pub struct ApercuSon {
    pub chemin: PathBuf,
    rx: Option<Receiver<SonLu>>,
    pub erreur: Option<String>,
    pub lecture: Lecture,
    pub duree: f64,
}

impl ApercuSon {
    pub fn ouvrir(chemin: PathBuf, volume: f32, boucle: bool) -> ApercuSon {
        let (tx, rx) = channel();
        let p = chemin.clone();
        let lourd = std::fs::metadata(&p).map_or(false, |m| m.len() > SON_MAX);
        let mut erreur = None;
        let mut rx = Some(rx);
        if lourd {
            erreur = Some("Ce son est trop lourd pour l'aperçu.".to_string());
            rx = None;
        } else if std::thread::Builder::new()
            .name("bibliotheque-son".into())
            .spawn(move || {
                crate::parallele::baisser_la_priorite_ici();
                let _ = tx.send(crate::bruitages::decode_file(&p));
            })
            .is_err()
        {
            erreur = Some("Impossible de lire ce son.".to_string());
            rx = None;
        }
        ApercuSon { chemin, rx, erreur, lecture: Lecture::new(volume, boucle), duree: 0.0 }
    }

    pub fn charge(&self) -> bool {
        self.rx.is_some()
    }

    pub fn pret(&self) -> bool {
        self.lecture.pcm.is_some()
    }

    /// Reprend le son décodé ; vrai s'il vient d'arriver.
    pub fn suivre(&mut self) -> bool {
        let Some(rx) = &self.rx else { return false };
        match rx.try_recv() {
            Ok(Ok(pcm)) => {
                self.duree = pcm.seconds();
                self.lecture.pcm = Some(pcm);
                self.rx = None;
                true
            }
            Ok(Err(e)) => {
                self.erreur = Some(format!("Impossible de lire ce son ({e})."));
                self.rx = None;
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.erreur = Some("La lecture du son s'est interrompue.".into());
                self.rx = None;
                true
            }
        }
    }

    pub fn jouer_ou_pause(&mut self, sortie: &mut dyn SortieAudio) {
        if !self.pret() {
            return;
        }
        if self.lecture.joue {
            self.lecture.pause(sortie);
        } else {
            // Au bout du son, rejouer reprend au début.
            let t = self.lecture.temps(sortie);
            self.lecture.demarrer(if t >= self.duree - 0.01 { 0.0 } else { t }, sortie);
        }
    }
}

// ------------------------------------------------------------------------------------ la vidéo

/// Les ordres de l'interface au fil de la vidéo.
enum Ordre {
    /// Une image à cet instant (secondes) ; `u64` : la génération de la demande.
    Image(f64, u64),
    /// Les images qui suivent depuis cet instant.
    Lire(f64, u64),
    /// Arrête le flux en cours.
    Stop,
}

/// Ce que le fil de la vidéo rend.
enum Rendu {
    Infos(Result<Infos, String>),
    Image { gen: u64, t: f64, flux: bool, w: usize, h: usize, rgba: Vec<u8> },
    Fin { gen: u64 },
    Erreur(String),
}

type Flux = Arc<Mutex<Option<Arc<Mutex<std::process::Child>>>>>;

pub struct ApercuVideo {
    pub chemin: PathBuf,
    pub infos: Option<Infos>,
    pub erreur: Option<String>,
    ordres: Sender<Ordre>,
    rx: Receiver<Rendu>,
    arret: Arc<AtomicBool>,
    flux_courant: Flux,
    gen: u64,
    /// Les images du flux qui attendent leur instant.
    file: VecDeque<(f64, usize, usize, Vec<u8>)>,
    fin_flux: bool,
    pub texture: Option<TextureHandle>,
    /// L'instant de l'image montrée (secondes).
    pub t_image: f64,
    pub lecture: Lecture,
    pub boucle: bool,
    /// Le son de la vidéo est en cours d'extraction.
    son_rx: Option<Receiver<Result<Pcm, String>>>,
    pub son_erreur: Option<String>,
    ffmpeg: Ffmpeg,
    /// Une image demandée n'est pas encore revenue.
    pub attend_image: bool,
    /// L'instant montré quand l'image demandée arrive.
    t_apres: f64,
}

impl ApercuVideo {
    pub fn ouvrir(chemin: PathBuf, ffmpeg: Ffmpeg, volume: f32, boucle: bool) -> ApercuVideo {
        let (ordres, rx_ordres) = channel::<Ordre>();
        let (tx, rx) = sync_channel::<Rendu>(6);
        let arret = Arc::new(AtomicBool::new(false));
        let flux_courant: Flux = Arc::new(Mutex::new(None));
        let (f, p, a, fc) = (ffmpeg.clone(), chemin.clone(), arret.clone(), flux_courant.clone());
        let fil = std::thread::Builder::new().name("bibliotheque-video".into()).spawn(move || travail_video(f, p, rx_ordres, tx, a, fc));
        let erreur = fil.is_err().then(|| "Impossible de lire cette vidéo.".to_string());
        let lecture = Lecture::new(volume, false);
        ApercuVideo {
            chemin,
            infos: None,
            erreur,
            ordres,
            rx,
            arret,
            flux_courant,
            gen: 0,
            file: VecDeque::new(),
            fin_flux: false,
            texture: None,
            t_image: 0.0,
            lecture,
            boucle,
            son_rx: None,
            son_erreur: None,
            ffmpeg,
            attend_image: true,
            t_apres: 0.0,
        }
    }

    pub fn duree(&self) -> f64 {
        self.infos.as_ref().map_or(0.0, |i| i.duree)
    }

    pub fn fps(&self) -> f64 {
        self.infos.as_ref().map_or(25.0, |i| i.fps)
    }

    /// L'instant montré par la barre de temps.
    pub fn temps(&self, sortie: &dyn SortieAudio) -> f64 {
        if self.lecture.joue {
            self.lecture.temps(sortie).min(self.duree().max(0.0))
        } else {
            self.t_image
        }
    }

    fn envoyer(&self, o: Ordre) {
        let _ = self.ordres.send(o);
    }

    /// Demande l'image à `t` (la barre de temps) : la demande précédente est périmée.
    pub fn voir(&mut self, t: f64) {
        self.voir_a(t, t);
    }

    /// Demande l'image à `t`, et montre `t_affiche` comme son instant quand elle arrive (image par image : le début de
    /// l'image, alors que la demande tombe en son milieu).
    fn voir_a(&mut self, t: f64, t_affiche: f64) {
        self.gen += 1;
        self.file.clear();
        self.fin_flux = false;
        let max = (self.duree() - 0.5 / self.fps()).max(0.0);
        let t = t.clamp(0.0, max);
        self.t_image = t_affiche.clamp(0.0, max);
        self.t_apres = self.t_image;
        self.attend_image = true;
        self.envoyer(Ordre::Image(t, self.gen));
    }

    /// Joue depuis `t`.
    pub fn jouer(&mut self, t: f64, sortie: &mut dyn SortieAudio) {
        let d = self.duree();
        let t = if d > 0.0 && t >= d - 0.05 { 0.0 } else { t.max(0.0) };
        self.gen += 1;
        self.file.clear();
        self.fin_flux = false;
        self.lecture.demarrer(t, sortie);
        self.attend_image = true;
        self.envoyer(Ordre::Lire(t, self.gen));
    }

    pub fn pause(&mut self, sortie: &mut dyn SortieAudio) {
        if !self.lecture.joue {
            return;
        }
        let t = self.temps(sortie);
        self.lecture.pause(sortie);
        // L'image montrée reste ; la suivante (image par image) part d'ici.
        self.gen += 1;
        self.file.clear();
        self.envoyer(Ordre::Stop);
        self.t_image = t;
    }

    pub fn jouer_ou_pause(&mut self, sortie: &mut dyn SortieAudio) {
        if self.infos.is_none() {
            return;
        }
        if self.lecture.joue {
            self.pause(sortie);
        } else {
            let t = self.t_image;
            self.jouer(t, sortie);
        }
    }

    /// Va à `t` : si ça joue, ça continue de là ; sinon l'image s'affiche.
    pub fn aller(&mut self, t: f64, sortie: &mut dyn SortieAudio) {
        if self.lecture.joue {
            self.jouer(t, sortie);
        } else {
            self.voir(t);
        }
    }

    /// Image par image : `n` images plus loin (négatif : avant). Arrête la lecture.
    pub fn pas(&mut self, n: i64, sortie: &mut dyn SortieAudio) {
        let t = self.temps(sortie);
        self.pause(sortie);
        let fps = self.fps();
        let k = (t * fps).floor() as i64 + n;
        let derniere = (self.duree() * fps).round() as i64 - 1;
        let k = k.clamp(0, derniere.max(0));
        // (Au milieu de l'image : `-ss` de ffmpeg prend la première image qui n'est pas avant l'instant.)
        self.voir_a((k as f64 + 0.3) / fps, k as f64 / fps);
    }

    pub fn regler(&mut self, volume: f32, boucle: bool, sortie: &mut dyn SortieAudio) {
        self.boucle = boucle;
        self.lecture.regler(volume, false, sortie);
    }

    /// À chaque image d'écran : reprend ce que les fils ont fait, montre l'image de l'instant, boucle ou s'arrête à la fin.
    /// Vrai si l'image montrée a changé.
    pub fn suivre(&mut self, ctx: &egui::Context, sortie: &mut dyn SortieAudio) -> bool {
        let mut change = false;
        while let Ok(r) = self.rx.try_recv() {
            match r {
                Rendu::Infos(Ok(i)) => {
                    let (son, duree) = (i.son, i.duree);
                    self.infos = Some(i);
                    self.voir(0.0);
                    if son && duree <= bibliovideo::DUREE_SON_MAX {
                        self.extraire_le_son();
                    }
                }
                Rendu::Infos(Err(e)) | Rendu::Erreur(e) => {
                    self.erreur = Some(e);
                    self.attend_image = false;
                }
                Rendu::Image { gen, t, flux, w, h, rgba } if gen == self.gen => {
                    if flux {
                        self.file.push_back((t, w, h, rgba));
                    } else {
                        self.montrer(ctx, w, h, &rgba);
                        let _ = t;
                        self.t_image = self.t_apres;
                        self.attend_image = false;
                        change = true;
                    }
                }
                Rendu::Fin { gen } if gen == self.gen => self.fin_flux = true,
                _ => {}
            }
        }
        // Le son de la vidéo arrive.
        if let Some(rx) = &self.son_rx {
            match rx.try_recv() {
                Ok(Ok(pcm)) => {
                    self.son_rx = None;
                    self.lecture.joindre_le_son(Arc::new(pcm), sortie);
                }
                Ok(Err(e)) => {
                    self.son_rx = None;
                    self.son_erreur = Some(e);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.son_rx = None,
            }
        }
        if self.lecture.joue {
            let t = self.lecture.temps(sortie);
            // L'image dont l'instant est le plus proche, sans dépasser celui de la lecture.
            let marge = 0.5 / self.fps();
            let mut a_montrer = None;
            while self.file.front().is_some_and(|f| f.0 <= t + marge) {
                a_montrer = self.file.pop_front();
            }
            if let Some((ti, w, h, rgba)) = a_montrer {
                self.montrer(ctx, w, h, &rgba);
                self.t_image = ti;
                self.attend_image = false;
                change = true;
            }
            if self.lecture.suivre(sortie, self.duree()) {
                if self.boucle {
                    self.jouer(0.0, sortie);
                } else {
                    self.gen += 1;
                    self.file.clear();
                    self.envoyer(Ordre::Stop);
                    self.t_image = self.duree();
                }
            } else if self.fin_flux && self.file.is_empty() && self.duree() <= 0.0 {
                // (Une durée inconnue : la fin du flux est la fin de la vidéo.)
                self.lecture.pause(sortie);
                if self.boucle {
                    self.jouer(0.0, sortie);
                }
            }
        }
        change
    }

    fn montrer(&mut self, ctx: &egui::Context, w: usize, h: usize, rgba: &[u8]) {
        if rgba.len() != w * h * 4 {
            return;
        }
        let image = ColorImage::from_rgba_unmultiplied([w, h], rgba);
        match &mut self.texture {
            Some(t) if t.size() == [w, h] => t.set(image, TextureOptions::LINEAR),
            _ => self.texture = Some(ctx.load_texture(format!("apercu {}", self.chemin.display()), image, TextureOptions::LINEAR)),
        }
    }

    fn extraire_le_son(&mut self) {
        let (tx, rx) = channel();
        let (ffmpeg, chemin, arret) = (self.ffmpeg.clone(), self.chemin.clone(), self.arret.clone());
        let fil = std::thread::Builder::new().name("bibliotheque-video-son".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            let r = match ffmpeg.chemin() {
                Some(f) => bibliovideo::extraire_son(&f, &chemin, &arret),
                None => Err("ffmpeg introuvable".to_string()),
            };
            let _ = tx.send(r);
        });
        if fil.is_ok() {
            self.son_rx = Some(rx);
        }
    }

    /// Du travail est en cours (un fil lit, ou la lecture avance) : l'interface se redessine.
    pub fn actif(&self) -> bool {
        self.lecture.joue || self.attend_image || self.infos.is_none() && self.erreur.is_none() || self.son_rx.is_some()
    }

    /// Arrête ffmpeg et le fil (aussi à la fermeture, par `Drop`).
    fn arreter_les_fils(&mut self) {
        self.arret.store(true, Ordering::Relaxed);
        if let Some(c) = self.flux_courant.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = c.lock().unwrap_or_else(|e| e.into_inner()).kill();
        }
        let _ = self.ordres.send(Ordre::Stop);
    }
}

impl Drop for ApercuVideo {
    fn drop(&mut self) {
        self.arreter_les_fils();
    }
}

/// Envoie `r` sans jamais bloquer pour de bon : le fil regarde les ordres et l'arrêt pendant qu'il attend la place.
fn envoyer_rendu(tx: &SyncSender<Rendu>, mut r: Rendu, ordres: &Receiver<Ordre>, arret: &AtomicBool, en_attente: &mut Option<Ordre>) -> bool {
    loop {
        if arret.load(Ordering::Relaxed) {
            return false;
        }
        match tx.try_send(r) {
            Ok(()) => return true,
            Err(TrySendError::Disconnected(_)) => return false,
            Err(TrySendError::Full(back)) => {
                r = back;
                match ordres.try_recv() {
                    Ok(o) => {
                        *en_attente = Some(o);
                        return false;
                    }
                    Err(TryRecvError::Disconnected) => return false,
                    Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(4)),
                }
            }
        }
    }
}

/// Le fil de la vidéo : lit les informations, puis exécute les ordres (le dernier seul compte).
fn travail_video(ffmpeg: Ffmpeg, fichier: PathBuf, ordres: Receiver<Ordre>, tx: SyncSender<Rendu>, arret: Arc<AtomicBool>, flux_courant: Flux) {
    crate::parallele::baisser_la_priorite_ici();
    let Some(f) = ffmpeg.chemin() else {
        let _ = tx.send(Rendu::Infos(Err("ffmpeg est introuvable : choisis-le dans Fichier > Exporter… > Le film.".into())));
        return;
    };
    let infos = match bibliovideo::lire_infos(&f, &fichier, &arret) {
        Ok(i) => i,
        Err(e) => {
            let _ = tx.send(Rendu::Infos(Err(e)));
            return;
        }
    };
    let (fps, taille) = (infos.fps, infos.taille_apercu());
    if tx.send(Rendu::Infos(Ok(infos))).is_err() {
        return;
    }
    let mut suivant: Option<Ordre> = None;
    loop {
        let mut ordre = match suivant.take() {
            Some(o) => o,
            None => match ordres.recv() {
                Ok(o) => o,
                Err(_) => return,
            },
        };
        // Seul le dernier ordre compte (on tire la barre de temps : les images périmées ne sont pas faites).
        while let Ok(o) = ordres.try_recv() {
            ordre = o;
        }
        if arret.load(Ordering::Relaxed) {
            return;
        }
        match ordre {
            Ordre::Stop => {}
            Ordre::Image(t, gen) => match bibliovideo::une_image(&f, &fichier, t, bibliovideo::LARGEUR_APERCU, &arret) {
                Ok((w, h, rgba)) => {
                    if !envoyer_rendu(&tx, Rendu::Image { gen, t, flux: false, w, h, rgba }, &ordres, &arret, &mut suivant) && arret.load(Ordering::Relaxed) {
                        return;
                    }
                }
                Err(e) => {
                    if arret.load(Ordering::Relaxed) {
                        return;
                    }
                    let _ = envoyer_rendu(&tx, Rendu::Erreur(format!("Pas d'image à cet instant ({e}).")), &ordres, &arret, &mut suivant);
                }
            },
            Ordre::Lire(t, gen) => {
                let mut flux = match bibliovideo::Flux::ouvrir(&f, &fichier, t, taille, fps) {
                    Ok(fl) => fl,
                    Err(e) => {
                        let _ = tx.send(Rendu::Erreur(e));
                        continue;
                    }
                };
                *flux_courant.lock().unwrap_or_else(|e| e.into_inner()) = Some(flux.tueur());
                let mut k = 0usize;
                loop {
                    if arret.load(Ordering::Relaxed) {
                        return;
                    }
                    match ordres.try_recv() {
                        Ok(o) => {
                            suivant = Some(o);
                            break;
                        }
                        Err(TryRecvError::Disconnected) => return,
                        Err(TryRecvError::Empty) => {}
                    }
                    match flux.suivante() {
                        Some(rgba) => {
                            let ti = t + k as f64 / fps;
                            k += 1;
                            let (w, h) = (flux.largeur, flux.hauteur);
                            if !envoyer_rendu(&tx, Rendu::Image { gen, t: ti, flux: true, w, h, rgba }, &ordres, &arret, &mut suivant) {
                                break;
                            }
                        }
                        None => {
                            let _ = envoyer_rendu(&tx, Rendu::Fin { gen }, &ordres, &arret, &mut suivant);
                            break;
                        }
                    }
                }
                *flux_courant.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
        }
    }
}

// ------------------------------------------------------------------------------------ un seul aperçu

/// L'aperçu ouvert.
pub enum Apercu {
    Image(ApercuImage),
    Video(Box<ApercuVideo>),
    Son(ApercuSon),
}

impl Apercu {
    pub fn chemin(&self) -> &Path {
        match self {
            Apercu::Image(a) => &a.chemin,
            Apercu::Video(a) => &a.chemin,
            Apercu::Son(a) => &a.chemin,
        }
    }

    /// Joue-t-il ?
    pub fn joue(&self) -> bool {
        match self {
            Apercu::Image(_) => false,
            Apercu::Video(v) => v.lecture.joue,
            Apercu::Son(s) => s.lecture.joue,
        }
    }

    /// Le ferme : la lecture s'arrête, la sortie audio est rendue, les fils et ffmpeg s'arrêtent (par `Drop`).
    pub fn fermer(mut self, sortie: &mut dyn SortieAudio) {
        match &mut self {
            Apercu::Image(_) => {}
            Apercu::Video(v) => v.lecture.pause(sortie),
            Apercu::Son(s) => s.lecture.pause(sortie),
        }
    }

    /// La sortie audio est prise par le projet qui joue : l'aperçu s'arrête sans y toucher.
    pub fn rendre_la_sortie(&mut self, sortie: &dyn SortieAudio) {
        match self {
            Apercu::Image(_) => {}
            Apercu::Video(v) => {
                let (gen, joue) = (v.gen, v.lecture.joue);
                if joue {
                    let t = v.temps(sortie);
                    v.lecture.pause_sans_moteur(sortie);
                    v.gen = gen + 1;
                    v.file.clear();
                    v.envoyer(Ordre::Stop);
                    v.t_image = t;
                }
            }
            Apercu::Son(s) => s.lecture.pause_sans_moteur(sortie),
        }
    }

    /// Du travail est en cours : l'interface se redessine.
    pub fn actif(&self) -> bool {
        match self {
            Apercu::Image(a) => a.charge(),
            Apercu::Video(v) => v.actif(),
            Apercu::Son(s) => s.charge() || s.lecture.joue,
        }
    }
}
