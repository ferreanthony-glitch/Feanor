//! Comparer un plan avec lui-même : deux de ses versions (`planvers.rs`), ou le plan d'aujourd'hui avec
//! son état à un envoi (`envois.rs`). Une vue en lecture seule : elle ne garde que des copies, ne
//! touche jamais au plan, à ses dessins ni à l'historique.
//!
//! # Zéro latence
//!
//! Rien de lourd sur le fil de l'interface. Il fait seulement ce qui est petit : copier le plan à
//! comparer (`petit_projet` : le plan et ses dessins, dont les tuiles sont partagées) et envoyer à un fil
//! les demandes « cette image du plan, à cette largeur ». Le fil (`Moteur`) :
//!
//! 1. relit l'état d'un envoi dans le dossier du projet (`envois::charger_plan` : disque et rendu des
//!    traits), ou reçoit la copie d'une version ;
//! 2. compose l'image de chaque côté (`rendre`, le même calcul que les vignettes et l'export, effets
//!    compris) ; les deux côtés en parallèle ;
//! 3. calcule la superposition colorée (`pelure`) et la part de l'image qui diffère (`difference`) ;
//! 4. répond. Une demande qui arrive pendant le calcul d'une autre la remplace (la glissière des images ne
//!    fait pas la queue).
//!
//! L'interface (`comparerui.rs`) ne fait que poser les textures reçues, côte à côte, en volet, en fondu
//! ou en superposition.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

use eframe::egui::{Color32, ColorImage, Context};

use crate::document::{Cel, Document, Panel};
use crate::envois::Envoi;

/// Ce qu'on compare à un côté.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// Le plan tel qu'il est maintenant (sa version active).
    Actuelle,
    /// Une version du plan, par son numéro (l'active aussi, c'est alors le plan d'aujourd'hui).
    Version(u32),
    /// Le plan tel qu'il était à un envoi, par le numéro de l'envoi.
    Envoi(u64),
    /// Le plan de même rang (le 2e de la séquence, le 3e…) dans une autre variante de sa séquence (`variantes.rs`),
    /// par le numéro de la variante.
    Variante(u32),
}

/// Comment les deux côtés se montrent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Les deux images l'une à côté de l'autre.
    Cote,
    /// Une seule image : l'une à gauche d'un curseur, l'autre à droite.
    Volet,
    /// Une seule image : l'une fondue dans l'autre.
    Fondu,
    /// Une seule image : ce qui n'est que d'un côté en rouge, que de l'autre en bleu, des deux en noir.
    Pelure,
}

/// Un côté prêt à partir au fil : une copie du plan, ou de quoi relire un envoi sur le disque.
pub enum Entree {
    Doc(Box<Document>),
    Envoi { dossier: PathBuf, envoi: Box<Envoi>, plan: u64 },
}

/// Une copie légère de `panel` qui suffit à le composer : la taille de la feuille et du cadre, le plan, ses
/// dessins (les tuiles sont partagées, rien n'est recopié ; l'image d'un dessin vectoriel suffit).
pub fn petit_projet(doc: &Document, panel: &Panel) -> Document {
    let mut mini = Document::new(doc.width, doc.height, doc.fps);
    mini.frame = doc.frame;
    mini.paper = doc.paper;
    mini.background = doc.background;
    mini.paper_b = doc.paper_b;
    mini.panels = vec![panel.clone()];
    for id in crate::empreinte::dessins_du_plan(panel) {
        if let Some(c) = doc.cels.get_ou_absent(&id) {
            let vector = c.absent.as_ref().and_then(|_| c.vector.clone());
            mini.cels.insert(id, Cel { raster: c.raster.clone(), vector, version: c.version, fond: c.fond.clone(), absent: c.absent.clone() });
        }
    }
    mini
}

/// L'image `frame` du plan de `doc` (son seul plan), à `largeur` pixels de large, telle que la caméra la
/// filme. Les effets sont calculés tout de suite (on est sur un fil de fond).
pub fn rendre(doc: &Document, frame: usize, largeur: usize) -> ColorImage {
    let frame = frame.min(doc.panels[0].frame_count().saturating_sub(1));
    let plan = crate::render::panel_plan_at(doc, 0, frame, Some(doc.paper));
    let cam = doc.panel_camera(0, frame);
    let (tw, th) = crate::thumbs::thumb_size_at(doc, largeur, 100);
    crate::render::thumbnail_until(doc, &plan, &cam, tw, th, &|| false).expect("jamais interrompue")
}

fn luminance(c: Color32) -> i32 {
    (c.r() as i32 * 299 + c.g() as i32 * 587 + c.b() as i32 * 114) / 1000
}

/// `b` à la taille de `a` (au plus proche), pour pouvoir les superposer.
fn a_la_taille(b: &ColorImage, taille: [usize; 2]) -> ColorImage {
    if b.size == taille {
        return b.clone();
    }
    let mut pixels = Vec::with_capacity(taille[0] * taille[1]);
    for y in 0..taille[1] {
        for x in 0..taille[0] {
            let (sx, sy) = (x * b.size[0] / taille[0].max(1), y * b.size[1] / taille[1].max(1));
            pixels.push(b.pixels[sy.min(b.size[1] - 1) * b.size[0] + sx.min(b.size[0] - 1)]);
        }
    }
    ColorImage { size: taille, pixels }
}

/// La superposition colorée de `a` et `b` : le trait qui n'est que dans `a` est rouge, celui qui n'est que
/// dans `b` est bleu, celui qui est dans les deux est noir, le papier est blanc.
pub fn pelure(a: &ColorImage, b: &ColorImage) -> ColorImage {
    let b = a_la_taille(b, a.size);
    let pixels = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .map(|(pa, pb)| {
            let (ink_a, ink_b) = (255 - luminance(*pa), 255 - luminance(*pb));
            let clair = |v: i32| v.clamp(0, 255) as u8;
            Color32::from_rgb(clair(255 - ink_b), clair(255 - ink_a.max(ink_b)), clair(255 - ink_a))
        })
        .collect();
    ColorImage { size: a.size, pixels }
}

/// La part (de 0 à 1) des pixels qui diffèrent d'un côté à l'autre de plus de `seuil` (par couleur).
pub fn difference(a: &ColorImage, b: &ColorImage, seuil: u8) -> f32 {
    let b = a_la_taille(b, a.size);
    if a.pixels.is_empty() {
        return 0.0;
    }
    let diff = a
        .pixels
        .iter()
        .zip(&b.pixels)
        .filter(|(pa, pb)| {
            let d = |x: u8, y: u8| x.abs_diff(y);
            d(pa.r(), pb.r()) > seuil || d(pa.g(), pb.g()) > seuil || d(pa.b(), pb.b()) > seuil
        })
        .count();
    diff as f32 / a.pixels.len() as f32
}

// ---------------------------------------------------------------------------------- le fil

/// Une demande à faire.
pub struct Requete {
    pub numero: u64,
    pub frame: usize,
    pub largeur: usize,
    /// Calculer aussi la superposition colorée.
    pub avec_pelure: bool,
}

/// Ce que le fil répond.
pub enum Reponse {
    /// Les deux côtés sont lus : leur nombre d'images (`None` : ce côté n'a pas pu être lu), et les
    /// avertissements de lecture.
    Prete { images: [Option<usize>; 2], erreurs: [Option<String>; 2], notes: Vec<String> },
    /// Les images d'une demande.
    Rendu(Rendu),
}

pub struct Rendu {
    pub numero: u64,
    pub frame: usize,
    pub a: Option<ColorImage>,
    pub b: Option<ColorImage>,
    pub pelure: Option<ColorImage>,
    /// La part de l'image qui diffère.
    pub difference: Option<f32>,
}

/// Le fil qui relit, compose et compare.
pub struct Moteur {
    tx: Option<Sender<Requete>>,
    rx: Receiver<Reponse>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Moteur {
    pub fn demarrer(a: Entree, b: Entree, ctx: Context) -> Moteur {
        let (tx, rx_requetes) = channel::<Requete>();
        let (tx_reponses, rx) = channel::<Reponse>();
        let handle = std::thread::Builder::new()
            .name("comparer".into())
            .spawn(move || {
                // Chaque côté se lit à son tour (un envoi se relit sur le disque).
                let mut docs: [Option<Document>; 2] = [None, None];
                let (mut erreurs, mut notes) = ([None, None], Vec::new());
                for (k, entree) in [a, b].into_iter().enumerate() {
                    match ouvrir(entree) {
                        Ok((doc, n)) => {
                            docs[k] = Some(doc);
                            notes.extend(n);
                        }
                        Err(e) => erreurs[k] = Some(e),
                    }
                }
                let images = [0, 1].map(|k| docs[k].as_ref().map(|d| d.panels[0].frame_count()));
                let _ = tx_reponses.send(Reponse::Prete { images, erreurs, notes });
                ctx.request_repaint();
                while let Ok(mut req) = rx_requetes.recv() {
                    // Seule la dernière demande compte.
                    while let Ok(plus_recente) = rx_requetes.try_recv() {
                        req = plus_recente;
                    }
                    let rendus = crate::parallele::par_map(&[0usize, 1], 2, |_, &k| docs[k].as_ref().map(|d| rendre(d, req.frame, req.largeur)));
                    let [a, b] = <[Option<ColorImage>; 2]>::try_from(rendus).unwrap_or([None, None]);
                    let (pelure_, difference_) = match (&a, &b) {
                        (Some(a), Some(b)) => (req.avec_pelure.then(|| pelure(a, b)), Some(difference(a, b, 8))),
                        _ => (None, None),
                    };
                    if tx_reponses.send(Reponse::Rendu(Rendu { numero: req.numero, frame: req.frame, a, b, pelure: pelure_, difference: difference_ })).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .ok();
        Moteur { tx: Some(tx), rx, handle }
    }

    /// Demande une image.
    pub fn demander(&self, r: Requete) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(r);
        }
    }

    /// La réponse suivante, s'il y en a une.
    pub fn recevoir(&self) -> Option<Reponse> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

}

impl Drop for Moteur {
    fn drop(&mut self) {
        // Plus de demande possible : le fil sort de sa boucle ; on ne l'attend pas (il peut relire un envoi).
        self.tx = None;
        let _ = self.handle.take();
    }
}

/// Le document à un seul plan d'un côté, et les avertissements de sa lecture.
fn ouvrir(e: Entree) -> Result<(Document, Vec<String>), String> {
    match e {
        // (Ses dessins hors de la mémoire reviennent ici, sur le fil de la comparaison : `residence.rs`.)
        Entree::Doc(d) => {
            let mut d = *d;
            crate::residence::resoudre_copie(&mut d);
            Ok((d, Vec::new()))
        }
        Entree::Envoi { dossier, envoi, plan } => {
            let l = crate::envois::charger_plan(&dossier, &envoi, plan)?;
            if l.doc.panels.is_empty() {
                return Err("le plan n'est pas dans l'instantané".into());
            }
            Ok((l.doc, l.warnings))
        }
    }
}
