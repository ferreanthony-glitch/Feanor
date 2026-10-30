//! La vidéo du panneau Bibliothèque : tout passe par ffmpeg (celui de l'export, `export::find_ffmpeg`), lancé
//! **sur d'autres fils**, jamais par l'interface.
//!
//! - `lire_infos` : durée, cadence et taille d'une vidéo, lues sur ce que ffmpeg écrit d'elle ;
//! - `une_image` : une seule image à un instant donné (vignette, image par image, barre de temps) ;
//! - `Flux` : les images qui suivent, en continu, à la cadence de la vidéo et à la taille de l'aperçu ;
//! - `extraire_son` : le son de la vidéo, en WAV temporaire, pour l'écouter pendant la lecture.
//!
//! Aucun fichier de l'utilisateur n'est écrit : les seuls fichiers faits sont les WAV temporaires de
//! l'aperçu, dans le dossier temporaire de Windows, effacés dès qu'ils sont lus.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Largeur la plus grande d'une image d'aperçu (au-delà, ffmpeg la réduit : l'aperçu reste léger).
pub const LARGEUR_APERCU: u32 = 800;
/// Temps accordé à ffmpeg pour une seule image ou pour lire les informations.
const DELAI_IMAGE: Duration = Duration::from_secs(25);
/// Les vidéos plus longues n'ont pas de son d'aperçu (le WAV temporaire serait énorme).
pub const DUREE_SON_MAX: f64 = 3600.0;

/// Ce qu'on sait d'une vidéo.
#[derive(Clone, Debug, PartialEq)]
pub struct Infos {
    /// En secondes ; 0 si ffmpeg ne l'annonce pas.
    pub duree: f64,
    pub fps: f64,
    /// Taille des images telles qu'on les voit (la rotation des vidéos de téléphone est comptée).
    pub largeur: u32,
    pub hauteur: u32,
    pub son: bool,
}

impl Infos {
    /// La taille des images de l'aperçu : au plus `LARGEUR_APERCU` de large, dimensions paires.
    pub fn taille_apercu(&self) -> (usize, usize) {
        let (w, h) = (self.largeur.max(2) as f64, self.hauteur.max(2) as f64);
        let k = (LARGEUR_APERCU as f64 / w).min(1.0);
        let pair = |v: f64| ((v.round() as usize) & !1).max(2);
        (pair(w * k), pair(h * k))
    }
}

/// « 00:01:05.20 » → secondes.
fn duree_de(s: &str) -> Option<f64> {
    let mut p = s.trim().split(':');
    let (h, m, sec) = (p.next()?.parse::<f64>().ok()?, p.next()?.parse::<f64>().ok()?, p.next()?.parse::<f64>().ok()?);
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// Les informations d'une vidéo, lues dans ce que `ffmpeg -i` écrit sur sa sortie d'erreur.
pub fn parse_infos(stderr: &str) -> Option<Infos> {
    let ligne_video = stderr.lines().find(|l| l.contains("Video:") && !l.contains("attached pic"))?;
    let (mut largeur, mut hauteur) = (0u32, 0u32);
    for mot in ligne_video.split(|c: char| c == ',' || c.is_whitespace() || c == '[' || c == '(') {
        if let Some((w, h)) = mot.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                if w >= 16 && h >= 16 {
                    (largeur, hauteur) = (w, h);
                    break;
                }
            }
        }
    }
    if largeur == 0 || hauteur == 0 {
        return None;
    }
    // Les vidéos de téléphone tournées : ffmpeg les redresse, la taille vue est celle d'après.
    let tourne = stderr.lines().filter(|l| l.contains("rotation of")).any(|l| {
        l.split("rotation of").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok()).is_some_and(|d| (d.abs() - 90.0).abs() < 1.0 || (d.abs() - 270.0).abs() < 1.0)
    });
    if tourne {
        std::mem::swap(&mut largeur, &mut hauteur);
    }
    let duree = stderr
        .lines()
        .find_map(|l| l.trim().strip_prefix("Duration:"))
        .and_then(|l| l.split(',').next())
        .and_then(duree_de)
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(0.0);
    let fps = crate::video::parse_fps(stderr).filter(|f| *f >= 1.0 && *f <= 240.0).unwrap_or(25.0);
    Some(Infos { duree, fps, largeur, hauteur, son: stderr.lines().any(|l| l.contains("Audio:")) })
}

/// Lance un programme sans ouvrir de console.
fn commande(programme: &Path) -> Command {
    let mut c = Command::new(programme);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c.stdin(Stdio::null());
    c
}

/// Lance `cmd`, en lit les deux sorties, et le tue s'il dépasse `delai` ou si `annule` est levé.
fn sortie_avec_delai(mut cmd: Command, delai: Duration, annule: &AtomicBool) -> Result<(Vec<u8>, String), String> {
    let mut enfant = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("ffmpeg ne démarre pas ({e})"))?;
    let (mut out, mut err) = (enfant.stdout.take(), enfant.stderr.take());
    let lecteur_out = std::thread::spawn(move || {
        let mut v = Vec::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_end(&mut v);
        }
        v
    });
    let lecteur_err = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let mut v = Vec::new();
            let _ = e.read_to_end(&mut v);
            s = String::from_utf8_lossy(&v).into_owned();
        }
        s
    });
    let debut = Instant::now();
    loop {
        match enfant.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if annule.load(Ordering::Relaxed) || debut.elapsed() > delai {
            let _ = enfant.kill();
            let _ = enfant.wait();
            let _ = lecteur_out.join();
            let _ = lecteur_err.join();
            return Err(if annule.load(Ordering::Relaxed) { "annulé".into() } else { "ffmpeg met trop de temps à lire cette vidéo".into() });
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    Ok((lecteur_out.join().unwrap_or_default(), lecteur_err.join().unwrap_or_default()))
}

/// Durée, cadence et taille de la vidéo `fichier`.
pub fn lire_infos(ffmpeg: &Path, fichier: &Path, annule: &AtomicBool) -> Result<Infos, String> {
    let mut c = commande(ffmpeg);
    c.args(["-hide_banner", "-i"]).arg(fichier);
    // (Sans fichier de sortie, ffmpeg s'arrête en erreur après avoir décrit le fichier : c'est ce qu'on veut.)
    let (_, texte) = sortie_avec_delai(c, DELAI_IMAGE, annule)?;
    parse_infos(&texte).ok_or_else(|| {
        let derniere = texte.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        format!("ffmpeg ne trouve pas d'image dans ce fichier ({derniere})")
    })
}

/// L'image de `fichier` à `t` secondes, réduite à `largeur` points au plus : largeur, hauteur, pixels RVBA.
pub fn une_image(ffmpeg: &Path, fichier: &Path, t: f64, largeur: u32, annule: &AtomicBool) -> Result<(usize, usize, Vec<u8>), String> {
    let mut c = commande(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{:.3}", t.max(0.0)), "-i"])
        .arg(fichier)
        .args(["-an", "-frames:v", "1", "-vf", &format!("scale='min(iw,{largeur})':-2"), "-f", "image2pipe", "-c:v", "mjpeg", "-q:v", "3", "pipe:1"]);
    let (octets, texte) = sortie_avec_delai(c, DELAI_IMAGE, annule)?;
    if octets.is_empty() {
        let derniere = texte.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
        return Err(if derniere.is_empty() { "pas d'image à cet instant".into() } else { derniere });
    }
    let img = image::load_from_memory(&octets).map_err(|e| format!("image illisible ({e})"))?.to_rgba8();
    Ok((img.width() as usize, img.height() as usize, img.into_raw()))
}

/// Les images d'une vidéo qui défilent, depuis l'instant demandé, à la taille de l'aperçu : ffmpeg les écrit en continu
/// et se bloque tout seul quand on ne les lit pas. Fermer le flux arrête ffmpeg.
pub struct Flux {
    enfant: Arc<Mutex<Child>>,
    sortie: ChildStdout,
    pub largeur: usize,
    pub hauteur: usize,
}

impl Flux {
    pub fn ouvrir(ffmpeg: &Path, fichier: &Path, depart: f64, taille: (usize, usize), fps: f64) -> Result<Flux, String> {
        let (w, h) = taille;
        let mut c = commande(ffmpeg);
        c.args(["-hide_banner", "-loglevel", "error", "-ss", &format!("{:.3}", depart.max(0.0)), "-i"])
            .arg(fichier)
            .args(["-an", "-vf", &format!("scale={w}:{h}:flags=fast_bilinear,fps={fps:.4}"), "-f", "rawvideo", "-pix_fmt", "rgba", "pipe:1"]);
        let mut enfant = c.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("ffmpeg ne démarre pas ({e})"))?;
        let sortie = enfant.stdout.take().ok_or("ffmpeg n'a pas de sortie")?;
        Ok(Flux { enfant: Arc::new(Mutex::new(enfant)), sortie, largeur: w, hauteur: h })
    }

    /// L'image suivante (pixels RVBA) ; rien à la fin de la vidéo (ou si ffmpeg s'est arrêté).
    pub fn suivante(&mut self) -> Option<Vec<u8>> {
        let mut tampon = vec![0u8; self.largeur * self.hauteur * 4];
        self.sortie.read_exact(&mut tampon).ok()?;
        Some(tampon)
    }
}

impl Flux {
    /// De quoi arrêter ffmpeg depuis un autre fil (fermer l'aperçu pendant que le fil lit une image).
    pub fn tueur(&self) -> Arc<Mutex<Child>> {
        self.enfant.clone()
    }
}

impl Drop for Flux {
    fn drop(&mut self) {
        let mut e = self.enfant.lock().unwrap_or_else(|e| e.into_inner());
        let _ = e.kill();
        let _ = e.wait();
    }
}

/// Un nom de fichier temporaire pour le son d'un aperçu.
pub fn wav_temporaire() -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join("shot-bibliotheque").join(format!("son-{}-{}.wav", std::process::id(), N.fetch_add(1, Ordering::Relaxed)))
}

/// Le son de `fichier`, décodé en mémoire : ffmpeg l'écrit en WAV temporaire, lu puis effacé. Erreur si la vidéo n'a pas de son.
pub fn extraire_son(ffmpeg: &Path, fichier: &Path, annule: &AtomicBool) -> Result<crate::audio::Pcm, String> {
    let wav = wav_temporaire();
    if let Some(d) = wav.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{} : {e}", d.display()))?;
    }
    let mut c = commande(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(fichier).args(["-vn", "-ac", "2", "-ar", "48000", "-c:a", "pcm_s16le"]).arg(&wav);
    let resultat = sortie_avec_delai(c, Duration::from_secs(600), annule).and_then(|_| {
        let octets = std::fs::read(&wav).map_err(|_| "pas de son dans cette vidéo".to_string())?;
        if octets.len() < 1024 {
            return Err("pas de son dans cette vidéo".into());
        }
        crate::audio::decode(&octets, "wav")
    });
    let _ = std::fs::remove_file(&wav);
    resultat
}
