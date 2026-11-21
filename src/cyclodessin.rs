//! Partie 203 (`docs/INTERFACE.md` partie 203 ; Anthony : « une option qui permet de créer son propre cyclo dans un espace de
//! dessin ») : **le cyclo dessiné**.
//!
//! - **La feuille** : une bande panoramique de 360° (`LARGEUR` × `HAUTEUR`, 4:1), projetée en cylindre autour du décor : de gauche
//!   à droite on tourne sur soi-même, de `E_HAUT` (60°) au-dessus de l'horizon à `E_BAS` (−30°) au-dessous ; l'horizon est au deux
//!   tiers de la hauteur. Au milieu, 0° : **devant**, ce que regarde la caméra de référence (vers −z) ; au trois quarts, 90° : à
//!   droite (+x) ; aux bords, 180° : derrière (+z) ; au quart, 270° : à gauche. Les deux bords se rejoignent.
//! - **C'est un petit projet FEANOR** (`Document`, un plan, ses calques), dessiné **dans un onglet à lui** : le dessin de FEANOR, tous
//!   ses outils, brosses, couleurs, calques, son annulation et la pression, sans rien réécrire. L'onglet du cyclo n'est pas un plan
//!   du film : rien ne change aux plans, à leurs numéros, au montage, aux vignettes ni à l'export. « Terminer » le referme et pose le
//!   dessin dans le décor de la séquence (`Decor3d::dessin_cyclo`), une étape d'annulation du projet.
//! - **Son image** : la bande, aplatie (`aplatir`, l'image du plan par le chemin de l'export), se pose dans l'image équirectangulaire
//!   du cyclo (`composer`, appelé par `cyclos::generer_avec`) : la bande à l'horizon, le ciel du choix au-dessus, le sol au-dessous,
//!   un fondu aux bords de la bande. L'image se fait par le cache de la partie 202, à la version du dessin (`Cyclo::dessin`), sur un
//!   autre fil ; une fois par « Terminer » qui change le dessin.
//! - **Son enregistrement** : à côté du projet, `cyclos/<séquence>/cyclo.shot`, un projet FEANOR écrit et relu par
//!   `project::save` et `project::load`, comme les dessins des plans ; seulement quand sa version change. Un FEANOR d'avant ignore
//!   le dossier et la clé `dessin` du cyclo (il voit le cyclo du choix) et ne perd rien d'autre.

use std::collections::{BTreeSet, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Stroke};

use crate::app::App;
use crate::cyclos::{Cyclo, Equirect, Preset};
use crate::document::{Document, Layer, LayerKind};

/// La feuille du cyclo : 4096 × 1024 pixels (4:1), 360° de large. Le même pas que l'image d'un cyclo dessiné (4096 pixels pour 360°,
/// `cyclos::taille`) : un pixel de la feuille, un pixel de l'image. Un calque bitmap peint partout : 16 Mo (RGBA), en tuiles (un
/// calque vide ne coûte rien) ; la bande aplatie : 16 Mo ; l'image du cyclo : 32 Mo.
pub const LARGEUR: usize = 4096;
pub const HAUTEUR: usize = 1024;
/// Le haut et le bas de la bande (degrés d'élévation) : 90° de haut, pour 360° de large (le même pas que la largeur).
pub const E_HAUT: f32 = 60.0;
pub const E_BAS: f32 = -30.0;
/// La part de la hauteur de la bande qui se fond dans le ciel (en haut) et le sol (en bas) du choix.
const FONDU: f32 = 0.06;
/// Le dossier des cyclos dessinés, dans le dossier du projet.
pub const DOSSIER: &str = "cyclos";
/// La largeur des fantômes de part et d'autre de la feuille (pixels de la feuille) : le bord opposé, pour continuer un trait.
pub const FANTOME: usize = 512;

/// La ligne d'horizon, en pixels de la feuille de `LARGEUR` (aux deux tiers de la hauteur).
pub fn horizon_y() -> f32 {
    horizon_y_sur(HAUTEUR)
}

/// La ligne d'horizon d'une feuille de `hauteur` pixels.
pub fn horizon_y_sur(hauteur: usize) -> f32 {
    E_HAUT / (E_HAUT - E_BAS) * hauteur as f32
}

/// Où est l'azimut `deg` (0° devant, 90° à droite, 180° derrière, 270° à gauche) sur la largeur de la feuille de `LARGEUR` (pixels).
pub fn x_de_l_azimut(deg: f32) -> f32 {
    x_de_l_azimut_sur(deg, LARGEUR)
}

/// `x_de_l_azimut`, sur une feuille de `largeur` pixels.
pub fn x_de_l_azimut_sur(deg: f32, largeur: usize) -> f32 {
    ((0.5 + deg / 360.0).rem_euclid(1.0)) * largeur as f32
}

/// La largeur des fantômes d'une feuille de `largeur` pixels (`FANTOME` pour une feuille de `LARGEUR`, la même part de la feuille
/// pour une plus petite).
pub fn fantome_de(largeur: usize) -> usize {
    (FANTOME * largeur / LARGEUR).clamp(1, largeur.max(1))
}

/// La largeur de la feuille qu'une carte graphique dont la plus grande texture a `cote_max` pixels de côté peut montrer : `LARGEUR`,
/// ou, en deçà, la plus grande puissance de deux qui y tient (au moins 512) ; la hauteur est le quart.
pub fn largeur_pour(cote_max: usize) -> usize {
    if cote_max >= LARGEUR {
        LARGEUR
    } else {
        let mut w = 512;
        while w * 2 <= cote_max {
            w *= 2;
        }
        w
    }
}

/// La colonne de la bande (0 à 1) qui regarde dans la direction de la colonne `u_e` (0 à 1) de l'image équirectangulaire
/// (`cyclos::uv` : l'image se lit de l'extérieur, la bande de l'intérieur, comme on la voit autour de soi).
pub fn u_bande(u_e: f32) -> f32 {
    (1.5 - u_e).rem_euclid(1.0)
}

// ------------------------------------------------------------------------------------------------ le dessin

/// Le dessin d'un cyclo : son projet (une feuille, ses calques), sa version, sa bande aplatie (faite une fois, à la demande).
pub struct DessinCyclo {
    pub doc: Document,
    pub version: u64,
    bande: OnceLock<Arc<Equirect>>,
}

impl DessinCyclo {
    pub fn new(doc: Document, version: u64) -> DessinCyclo {
        DessinCyclo { doc, version, bande: OnceLock::new() }
    }

    /// Avec sa bande déjà aplatie (celle que l'onglet du cyclo a faite au dernier trait).
    pub fn avec_bande(doc: Document, version: u64, bande: Option<Arc<Equirect>>) -> DessinCyclo {
        let d = DessinCyclo::new(doc, version);
        if let Some(b) = bande {
            let _ = d.bande.set(b);
        }
        d
    }

    /// La bande aplatie (sRGB, non prémultipliée) : faite la première fois qu'on la demande (sur le fil du cyclo, `cyclos`).
    pub fn bande(&self) -> Arc<Equirect> {
        self.bande.get_or_init(|| Arc::new(aplatir(&self.doc))).clone()
    }

    /// Une copie du projet du dessin, à dessiner (« Redessiner ») : les tuiles sont partagées, rien n'est recopié en pixels.
    pub fn copie_du_doc(&self) -> Document {
        copie(&self.doc)
    }
}

/// Une copie d'un projet et de ses dessins (les tuiles des dessins sont partagées).
pub fn copie(doc: &Document) -> Document {
    let mut c = doc.copie_sans_dessins();
    c.cels = doc.cels.clone();
    c
}

/// L'empreinte du contenu de la feuille : ses calques (ordre, visibilité, opacité, fusion, dessins) et la version de chacun de leurs
/// dessins. Elle change à chaque trait posé (la version du dessin touché), à chaque calque ajouté, caché, enlevé.
pub fn signature(doc: &Document) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in &doc.panels {
        for l in &p.layers {
            (l.id, l.visible, l.opacity.to_bits()).hash(&mut h);
            format!("{:?}", l.blend).hash(&mut h);
            for c in l.frames.iter().flatten() {
                c.hash(&mut h);
                doc.cels.get(c).map(|c| c.version).hash(&mut h);
            }
        }
    }
    h.finish()
}

/// Une version neuve (jamais 0, jamais deux fois la même).
pub fn nouvelle_version() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    (t ^ N.fetch_add(1, std::sync::atomic::Ordering::Relaxed).rotate_left(48)) | 1
}

/// L'image de la feuille (son premier plan, sa première image), par le chemin de l'export, sans papier : la bande, en sRGB non
/// prémultiplié.
pub fn aplatir(doc: &Document) -> Equirect {
    use crate::exportrender::{Papier, Rendu, SnapshotBuilder};
    let (w, h) = (doc.frame[0], doc.frame[1]);
    let mut cliche = SnapshotBuilder::new(doc, &[0]);
    cliche.step(doc, std::time::Duration::MAX);
    let rendu = Rendu::new(cliche.finish(), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let mut rgba = vec![0u8; w * h * 4];
    if let Some(img) = rendu.image(0, 0) {
        for (o, c) in rgba.chunks_exact_mut(4).zip(img.iter()) {
            o.copy_from_slice(&c.to_srgba_unmultiplied());
        }
    }
    Equirect { largeur: w, hauteur: h, rgba }
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// La couleur (linéaire) de l'image d'un cyclo dessiné à la colonne `u_e` (0 à 1) et à l'élévation `e` (degrés) : `col`, le ciel
/// ou le sol du choix, sous la bande `b` (lue en bilinéaire, ses bords gauche et droit joints, prémultipliée pour que ses bords
/// transparents ne noircissent rien), fondue en haut et en bas.
pub fn composer(b: &Equirect, col: [f32; 3], u_e: f32, e: f32) -> [f32; 3] {
    let v = (E_HAUT - e) / (E_HAUT - E_BAS);
    if !(0.0..=1.0).contains(&v) || b.largeur == 0 || b.hauteur == 0 {
        return col;
    }
    let fondu = smooth(0.0, FONDU, v) * smooth(0.0, FONDU, 1.0 - v);
    let (w, h) = (b.largeur, b.hauteur);
    let x = u_bande(u_e) * w as f32 - 0.5;
    let y = (v * h as f32 - 0.5).clamp(0.0, h as f32 - 1.0);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let px = |xi: i64, yi: i64| {
        let xi = xi.rem_euclid(w as i64) as usize;
        let yi = yi.clamp(0, h as i64 - 1) as usize;
        let o = (yi * w + xi) * 4;
        let a = b.rgba[o + 3] as f32 / 255.0;
        let c = crate::couleurs3d::lineaire([b.rgba[o], b.rgba[o + 1], b.rgba[o + 2]]);
        [c[0] * a, c[1] * a, c[2] * a, a]
    };
    let (xi, yi) = (x0 as i64, y0 as i64);
    let (p, q, r, s) = (px(xi, yi), px(xi + 1, yi), px(xi, yi + 1), px(xi + 1, yi + 1));
    let m: [f32; 4] = std::array::from_fn(|k| (p[k] * (1.0 - fx) + q[k] * fx) * (1.0 - fy) + (r[k] * (1.0 - fx) + s[k] * fx) * fy);
    let a = m[3] * fondu;
    [0, 1, 2].map(|k| col[k] * (1.0 - a) + m[k] * fondu)
}

/// La bande du cyclo `c` telle qu'il la montre (RGBA sRGB, `LARGEUR` × `HAUTEUR`) : celle du choix (ses nuages, ses silhouettes),
/// ou, `vide`, son ciel de fond seul (le dégradé, le soleil). Le départ d'un dessin.
pub fn bande_du_choix(c: &Cyclo, vide: bool) -> Vec<u8> {
    bande_du_choix_sur(c, vide, LARGEUR)
}

/// `bande_du_choix`, pour une feuille de `largeur` × `largeur / 4`.
pub fn bande_du_choix_sur(c: &Cyclo, vide: bool, largeur: usize) -> Vec<u8> {
    let (w, h) = (largeur, largeur / 4);
    let base = Cyclo { dessin: vide.then_some(1), ..*c };
    let (we, he) = (w, w / 2);
    let img = crate::cyclos::generer_avec(&base, we, he, None);
    let mut out = vec![0u8; w * h * 4];
    for y in 0..h {
        let e = E_HAUT - (y as f32 + 0.5) / h as f32 * (E_HAUT - E_BAS);
        let ye = (((90.0 - e) / 180.0 * he as f32) as usize).min(he - 1);
        for x in 0..w {
            // (La colonne de l'image qui regarde comme cette colonne de la bande : `u_bande` est sa propre inverse.)
            let ue = u_bande((x as f32 + 0.5) / w as f32);
            let xe = ((ue * we as f32) as usize).min(we - 1);
            let (o, i) = ((y * w + x) * 4, (ye * we + xe) * 4);
            out[o..o + 4].copy_from_slice(&img.rgba[i..i + 4]);
        }
    }
    out
}

/// Comment commence un dessin de cyclo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depart {
    /// Le ciel de fond du choix seul (son dégradé) : on dessine tout l'horizon.
    Vide,
    /// La bande d'un choix (ses nuages, ses silhouettes), à repeindre.
    Choix(Preset),
}

/// La feuille d'un dessin neuf : un projet de `LARGEUR` × `HAUTEUR`, un plan d'une image, deux calques bitmap : « Fond du cyclo »
/// (la bande de départ, `bande_du_choix`) et, devant, « Mon horizon », vide, où l'on dessine.
pub fn feuille(base: &Cyclo, vide: bool, titre: &str) -> Document {
    feuille_sur(base, vide, titre, LARGEUR)
}

/// `feuille`, de `largeur` × `largeur / 4` (une carte graphique qui ne montre pas une texture de `LARGEUR` de côté, `largeur_pour`).
pub fn feuille_sur(base: &Cyclo, vide: bool, titre: &str, largeur: usize) -> Document {
    let (w, h) = (largeur, largeur / 4);
    let mut doc = Document::new(w, h, 24);
    doc.properties.title = titre.to_string();
    doc.panels[0].set_length(1);
    let n = doc.panels[0].frame_count();
    let fond = doc.new_cel(LayerKind::Bitmap);
    if let Some(c) = doc.cels.get_mut(&fond) {
        c.raster = crate::raster::Raster::from_rgba(w, h, &bande_du_choix_sur(base, vide, w));
    }
    let dessin = doc.new_cel(LayerKind::Bitmap);
    let mut l_fond = Layer::new("Fond du cyclo", LayerKind::Bitmap);
    l_fond.frames = vec![Some(fond); n];
    let mut l_dessin = Layer::new("Mon horizon", LayerKind::Bitmap);
    l_dessin.frames = vec![Some(dessin); n];
    doc.panels[0].layers = vec![l_fond, l_dessin];
    doc
}

/// Les fantômes de la bande `b` : à gauche de la feuille, sa bande de droite (`FANTOME` pixels) ; à droite, sa bande de gauche.
/// RGBA, `FANTOME` × `HAUTEUR` chacun.
pub fn fantomes(b: &Equirect) -> (Vec<u8>, Vec<u8>) {
    let l = fantome_de(b.largeur);
    let mut gauche = vec![0u8; l * b.hauteur * 4];
    let mut droite = vec![0u8; l * b.hauteur * 4];
    for y in 0..b.hauteur {
        let ligne = &b.rgba[y * b.largeur * 4..(y + 1) * b.largeur * 4];
        gauche[y * l * 4..(y + 1) * l * 4].copy_from_slice(&ligne[(b.largeur - l) * 4..]);
        droite[y * l * 4..(y + 1) * l * 4].copy_from_slice(&ligne[..l * 4]);
    }
    (gauche, droite)
}

/// Le raccord des bords : l'encre qu'un trait a posée au-delà d'un bord de la feuille (les tuiles hors de la feuille d'un dessin
/// bitmap, la toile infinie de FEANOR) revient par le bord opposé, posée par-dessus ce qui y est (comme un trait). Rend vrai si
/// quelque chose a bougé. Les tuiles au-dessus ou au-dessous de la feuille restent (hors de la bande, elles ne se voient pas).
pub fn replier_les_bords(r: &mut crate::raster::Raster) -> bool {
    use crate::raster::TILE;
    let (tx_n, ty_n) = (r.width.div_ceil(TILE) as i32, r.height.div_ceil(TILE) as i32);
    if r.width % TILE != 0 || tx_n == 0 {
        return false;
    }
    let cles: Vec<(i32, i32)> = r.outer.keys().copied().filter(|&(tx, ty)| (0..ty_n).contains(&ty) && !(0..tx_n).contains(&tx)).collect();
    for (tx, ty) in &cles {
        let Some(t) = r.outer.remove(&(*tx, *ty)) else { continue };
        let vers = r.id_of(tx.rem_euclid(tx_n), *ty);
        let dst = r.tile_mut(vers);
        for (d, s) in dst.iter_mut().zip(t.iter()) {
            if s.a() == 0 {
                continue;
            }
            // (Prémultipliés : la source par-dessus.)
            let k = 1.0 - s.a() as f32 / 255.0;
            let m = |a: u8, b: u8| (a as f32 + b as f32 * k).round().min(255.0) as u8;
            *d = Color32::from_rgba_premultiplied(m(s.r(), d.r()), m(s.g(), d.g()), m(s.b(), d.b()), m(s.a(), d.a()));
        }
    }
    !cles.is_empty()
}

/// `replier_les_bords` sur les dessins bitmap de la feuille `doc` ; leur version change. Rend vrai si quelque chose a bougé.
pub fn replier_la_feuille(doc: &mut Document) -> bool {
    let cels: Vec<crate::document::CelId> = doc.panels.iter().flat_map(|p| p.layers.iter().flat_map(|l| l.frames.iter().flatten().copied())).collect();
    let mut bouge = false;
    for id in cels {
        if let Some(c) = doc.cels.get_mut(&id) {
            if c.vector.is_none() && !c.raster.outer.is_empty() && replier_les_bords(&mut c.raster) {
                c.version += 1;
                bouge = true;
            }
        }
    }
    bouge
}

// ------------------------------------------------------------------------------------------------ l'enregistrement

/// Le manifeste du dessin du cyclo de la séquence `seq`, dans le projet du dossier `dir`.
pub fn chemin(dir: &Path, seq: u64) -> PathBuf {
    dir.join(DOSSIER).join(seq.to_string()).join(format!("cyclo.{}", crate::project::EXTENSION))
}

/// Quelle version de dessin est écrite à quel endroit (pendant la séance) : un dessin qui n'a pas changé ne se réécrit pas.
fn ecrits() -> &'static Mutex<HashMap<PathBuf, u64>> {
    static E: OnceLock<Mutex<HashMap<PathBuf, u64>>> = OnceLock::new();
    E.get_or_init(Default::default)
}

/// Écrit le dessin `d` du cyclo de la séquence `seq` (sur le fil de l'enregistrement du projet), s'il n'y est pas déjà à cette
/// version. Rend vrai s'il a été écrit.
pub(crate) fn ecrire(dir: &Path, seq: u64, d: &DessinCyclo) -> Result<bool, String> {
    let ch = chemin(dir, seq);
    if ecrits().lock().unwrap_or_else(|e| e.into_inner()).get(&ch) == Some(&d.version) && ch.exists() {
        return Ok(false);
    }
    if let Some(p) = ch.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{} : {e}", p.display()))?;
    }
    crate::project::save(&d.doc, &ch, &mut HashMap::new()).map_err(|e| format!("le dessin du cyclo de la séquence : {e}"))?;
    ecrits().lock().unwrap_or_else(|e| e.into_inner()).insert(ch, d.version);
    Ok(true)
}

/// Efface les dessins des séquences qui n'ont plus de cyclo dessiné (`gardes` : celles qui en ont un). Seulement les dossiers
/// nommés d'un numéro, que FEANOR écrit.
pub(crate) fn nettoyer(dir: &Path, gardes: &BTreeSet<u64>) -> usize {
    let Ok(entries) = std::fs::read_dir(dir.join(DOSSIER)) else { return 0 };
    let mut n = 0;
    for e in entries.flatten() {
        let nom = e.file_name().to_string_lossy().into_owned();
        if let Ok(seq) = nom.parse::<u64>() {
            if !gardes.contains(&seq) && e.path().is_dir() && std::fs::remove_dir_all(e.path()).is_ok() {
                ecrits().lock().unwrap_or_else(|e| e.into_inner()).retain(|p, _| !p.starts_with(e.path()));
                n += 1;
            }
        }
    }
    n
}

/// À l'ouverture d'un projet : relit le dessin de chaque cyclo dessiné. Un dessin qui manque laisse le cyclo à son ciel de fond
/// (un message le dit) ; la clé reste, le dossier n'est pas effacé.
pub(crate) fn lire_les_dessins(dir: &Path, decors: &mut crate::decor3d::Decors3d, warnings: &mut Vec<String>) {
    for (&seq, d) in decors.par_sequence.iter_mut() {
        let Some(version) = d.cyclo.and_then(|c| c.dessin) else { continue };
        let ch = chemin(dir, seq);
        match crate::project::load(&ch) {
            Ok(l) => {
                d.dessin_cyclo = Some(Arc::new(DessinCyclo::new(l.doc, version)));
                ecrits().lock().unwrap_or_else(|e| e.into_inner()).insert(ch, version);
            }
            Err(e) => warnings.push(format!("Le dessin du cyclo d'une séquence ne se lit pas ({e}) : la séquence montre le ciel de son choix.")),
        }
    }
}

// ------------------------------------------------------------------------------------------------ l'onglet du dessin

/// L'onglet du dessin d'un cyclo : d'où l'on vient, pour y revenir ; la bande aplatie au dernier trait et ses fantômes.
pub(crate) struct Edition {
    /// L'onglet du projet (`Onglet::id`) et la séquence (« la séquence 3 », ou « la séquence « La cuisine » »).
    pub origine: u64,
    pub seq: u64,
    pub nom_sequence: String,
    /// Le cyclo de départ (le choix, ses réglages) ; un dessin neuf (`neuf`) se pose même sans trait.
    pub base: Cyclo,
    pub neuf: bool,
    /// L'empreinte de la feuille à l'ouverture (`signature`) : sans changement, « Terminer » ne pose rien.
    pub depart: u64,
    /// Où l'on était : l'espace de travail, l'outil, la page des vignettes.
    pub espace: crate::workspace::Espace,
    pub outil: crate::app::Tool,
    pub board_view: crate::app::BoardView,
    /// La bande aplatie au dernier trait (l'empreinte qu'elle montre), celle en cours sur un autre fil, et ses fantômes.
    pub vue: Option<(u64, Arc<Equirect>)>,
    travail: Option<(u64, std::thread::JoinHandle<Equirect>)>,
    fantomes: Option<[egui::TextureHandle; 2]>,
    /// Combien de fois la bande a été aplatie (essais : une fois par trait au plus).
    pub aplaties: usize,
}

impl App {
    /// « Dessiner mon cyclo… » (`depart`) ou « Redessiner » (rien : le dessin du cyclo de la séquence) : la feuille s'ouvre dans un
    /// onglet à elle, la vue de dessin de FEANOR. Faux si rien ne s'est ouvert.
    pub(crate) fn dessiner_mon_cyclo(&mut self, depart: Option<Depart>) -> bool {
        if self.doc.panels.is_empty() || self.cyclo_edition.is_some() || !self.onglet_libre() {
            return false;
        }
        self.decor3d_retouche_finir();
        self.end_stroke();
        let seq = self.doc.env_cle(self.cur_panel);
        // (« la séquence 3 », ou son nom : « la séquence « La cuisine » ».)
        let (code, nom) = (self.doc.sequence_code(seq), self.doc.sequence_label(seq));
        // (Partie 204 : l'environnement propre d'un plan : « l'environnement du plan 3 ».)
        let nom_sequence = if crate::decor3d::plan_de_cle(seq).is_some() {
            format!("l'environnement du {}", crate::storyboard::panel_title(&self.doc, self.cur_panel).to_lowercase())
        } else if nom == code {
            format!("la séquence {code}")
        } else {
            format!("la séquence « {nom} »")
        };
        let actuel = self.cyclo_courant();
        let titre = format!("Cyclo de {nom_sequence}");
        // (La plus grande texture que la carte graphique accepte : une feuille plus petite si elle ne montre pas 4096 pixels de côté.
        // FEANOR ne s'arrête jamais sur une texture trop grande.)
        let cote_max = self.ctx.input(|i| i.max_texture_side);
        let largeur = largeur_pour(cote_max);
        let (doc, base, neuf) = match depart {
            None => {
                let Some(d) = self.doc.decors3d.par_sequence.get(&seq).and_then(|d| d.dessin_cyclo.clone()) else {
                    self.status = "Cette séquence n'a pas de cyclo dessiné : « Dessiner mon cyclo… ».".into();
                    return false;
                };
                if d.doc.width.max(d.doc.height) > cote_max {
                    self.status = format!(
                        "Ce cyclo a été dessiné sur une feuille de {} pixels de large ; la carte graphique de cet ordinateur ne montre pas plus de {cote_max} pixels : il se redessine sur un autre ordinateur (ou « Dessiner mon cyclo… » en fait un neuf).",
                        d.doc.width
                    );
                    return false;
                }
                (d.copie_du_doc(), actuel.unwrap_or_default_cyclo(), false)
            }
            Some(Depart::Choix(p)) => {
                let base = actuel.filter(|c| c.preset == p).map_or_else(|| p.cyclo(), |c| Cyclo { dessin: None, ..c });
                (feuille_sur(&base, false, &titre, largeur), base, true)
            }
            Some(Depart::Vide) => {
                let base = actuel.map_or_else(|| Preset::Jour.cyclo(), |c| Cyclo { dessin: None, ..c });
                (feuille_sur(&base, true, &titre, largeur), base, true)
            }
        };
        let ed = Edition {
            origine: self.onglets[self.actif].id,
            seq,
            nom_sequence: nom_sequence.clone(),
            base,
            neuf,
            depart: signature(&doc),
            espace: self.espace,
            outil: self.tool,
            board_view: self.board_view,
            vue: None,
            travail: None,
            fantomes: None,
            aplaties: 0,
        };
        if !self.ouvrir_onglet_du_cyclo(doc) {
            return false;
        }
        self.cyclo_edition = Some(ed);
        self.welcome = false;
        self.modified = false;
        self.aller_a_l_espace(crate::workspace::Espace::Storyboard);
        self.board_view = crate::app::BoardView::Scene;
        self.camera_view = false;
        self.cur_layer = self.layers().len().saturating_sub(1);
        if !self.tool_dessine() {
            self.set_tool(crate::app::Tool::Brush);
        }
        self.fit_pending = true;
        self.status = if largeur < LARGEUR {
            format!("Cyclo de {nom_sequence} : dessine l'horizon tout autour (360°), puis « Terminer ». (Feuille de {largeur} pixels : la carte graphique de cet ordinateur n'en montre pas plus.)")
        } else {
            format!("Cyclo de {nom_sequence} : dessine l'horizon tout autour (360°), puis « Terminer ».")
        };
        true
    }

    /// L'outil courant dessine sur la feuille.
    fn tool_dessine(&self) -> bool {
        matches!(self.tool, crate::app::Tool::Brush | crate::app::Tool::Eraser)
    }

    /// « Terminer » : l'onglet du cyclo se ferme, le dessin se pose dans le décor de la séquence (une étape d'annulation, « Dessiner
    /// le cyclo » ; rien s'il n'a pas changé), et l'on revient où l'on était (le projet, son plan, l'espace, l'outil). Faux si l'on
    /// n'a pas pu revenir (le projet du cyclo est fermé, ou occupé).
    pub(crate) fn terminer_cyclo(&mut self) -> bool {
        let Some(origine) = self.cyclo_edition.as_ref().map(|e| e.origine) else { return true };
        self.end_stroke();
        self.finish_text_edit();
        let Some(rang) = self.rang_onglet(origine) else {
            // (Le projet a été fermé entre-temps : l'onglet du cyclo se ferme, son dessin n'a plus où aller.)
            self.cyclo_edition = None;
            self.fermer_l_onglet_affiche();
            self.status = "Le projet de ce cyclo était fermé : son dessin n'a pas pu y aller.".into();
            return true;
        };
        if !self.onglet_libre() {
            return false;
        }
        let sig = signature(&self.doc);
        let (doc, ed) = self.fermer_l_onglet_du_cyclo(rang);
        let Some(ed) = ed else { return true };
        if ed.neuf || sig != ed.depart {
            let version = nouvelle_version();
            let bande = ed.vue.as_ref().filter(|(s, _)| *s == sig).map(|(_, b)| b.clone());
            let dessin = Arc::new(DessinCyclo::avec_bande(doc, version, bande));
            let snap = self.snapshot();
            let mut d = self.doc.decors3d.par_sequence.get(&ed.seq).cloned().unwrap_or_else(|| crate::decor3d::Decor3d::new(Default::default()));
            // (Les réglages du cyclo de la séquence, s'il en a un ; sinon ceux du départ.)
            let base = d.cyclo.filter(|_| !ed.neuf).unwrap_or(ed.base);
            crate::cyclosui::poser(&mut d, Some(Cyclo { dessin: Some(version), ..base }));
            d.dessin_cyclo = Some(dessin);
            self.decor3d_appliquer(ed.seq, d);
            self.commit_named(snap, "Dessiner le cyclo");
            self.renderer.invalidate_all();
            self.status = format!("Le cyclo dessiné de {} est posé (Ctrl+Z le retire).", ed.nom_sequence);
        } else {
            self.status = format!("Le cyclo de {} n'a pas changé.", ed.nom_sequence);
        }
        if self.espace != ed.espace {
            self.aller_a_l_espace(ed.espace);
        }
        self.board_view = ed.board_view;
        self.set_tool(ed.outil);
        true
    }

    /// Un onglet de cyclo est ouvert (affiché ou non).
    pub(crate) fn cyclos_en_cours(&self) -> bool {
        self.cyclo_edition.is_some() || self.onglets.iter().any(|t| t.etat.as_ref().is_some_and(|e| e.cyclo_edition.is_some()))
    }

    /// Tous les onglets de cyclo se terminent (avant de quitter).
    pub(crate) fn terminer_tous_les_cyclos(&mut self) {
        for _ in 0..self.onglets.len() {
            let Some(i) = (0..self.onglets.len()).find(|&i| if i == self.actif { self.cyclo_edition.is_some() } else { self.onglets[i].etat.as_ref().is_some_and(|e| e.cyclo_edition.is_some()) }) else { return };
            if i != self.actif && !self.activer_onglet(i) {
                return;
            }
            if !self.terminer_cyclo() {
                return;
            }
        }
    }

    /// À chaque image d'écran, dans l'onglet d'un cyclo : un trait posé (l'empreinte de la feuille change) fait aplatir la bande sur
    /// un autre fil (une à la fois, la plus récente gagne) ; elle arrive : ses fantômes se refont.
    pub(crate) fn cyclo_edition_tick(&mut self, ctx: &egui::Context) {
        if self.cyclo_edition.is_none() {
            return;
        }
        let geste = self.stroke.is_some() || self.vstroke.is_some();
        // (Un trait lâché qui a passé un bord : son encre revient par l'autre bord.)
        if !geste && replier_la_feuille(&mut self.doc) {
            self.renderer.invalidate_all();
        }
        let sig = signature(&self.doc);
        let copie_a_faire = {
            let ed = self.cyclo_edition.as_ref().unwrap();
            !geste && ed.travail.is_none() && ed.vue.as_ref().is_none_or(|(s, _)| *s != sig)
        };
        let copie = copie_a_faire.then(|| copie(&self.doc));
        let ed = self.cyclo_edition.as_mut().unwrap();
        if let Some(c) = copie {
            if let Ok(h) = std::thread::Builder::new().name("cyclo-bande".into()).spawn(move || aplatir(&c)) {
                ed.travail = Some((sig, h));
            }
        }
        if ed.travail.as_ref().is_some_and(|(_, h)| h.is_finished()) {
            let (s, h) = ed.travail.take().unwrap();
            if let Ok(b) = h.join() {
                let (g, d) = fantomes(&b);
                let l = fantome_de(b.largeur);
                let image = |px: &[u8]| egui::ColorImage::from_rgba_unmultiplied([l, b.hauteur], px);
                ed.fantomes = Some([
                    ctx.load_texture("cyclo-fantome-gauche", image(&g), egui::TextureOptions::LINEAR),
                    ctx.load_texture("cyclo-fantome-droite", image(&d), egui::TextureOptions::LINEAR),
                ]);
                ed.vue = Some((s, Arc::new(b)));
                ed.aplaties += 1;
                ctx.request_repaint();
            }
        }
        if ed.travail.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
    }

    /// Sur la feuille d'un cyclo : la ligne d'horizon, les repères 0° (devant), 90° (droite), 180° (derrière), 270° (gauche), et de
    /// part et d'autre, pâles, les fantômes du bord opposé (pour continuer un trait d'un bord à l'autre).
    pub(crate) fn peindre_guides_du_cyclo(&self, painter: &egui::Painter) {
        let Some(ed) = self.cyclo_edition.as_ref() else { return };
        let (w, h) = (self.doc.width as f32, self.doc.height as f32);
        let l = fantome_de(self.doc.width) as f32;
        if let Some([g, d]) = ed.fantomes.as_ref() {
            let teinte = Color32::from_white_alpha(150);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let r = |x0: f32, x1: f32| Rect::from_two_pos(self.layer_to_screen(pos2(x0, 0.0)), self.layer_to_screen(pos2(x1, h)));
            painter.image(g.id(), r(-l, 0.0), uv, teinte);
            painter.image(d.id(), r(w, w + l), uv, teinte);
        }
        // Les bords de la feuille (où les fantômes commencent).
        let bord = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 120));
        for x in [0.0, w] {
            painter.line_segment([self.layer_to_screen(pos2(x, 0.0)), self.layer_to_screen(pos2(x, h))], bord);
        }
        // L'horizon.
        let y = horizon_y_sur(self.doc.height);
        let horizon = Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 220, 90, 200));
        painter.extend(egui::Shape::dashed_line(&[self.layer_to_screen(pos2(-l, y)), self.layer_to_screen(pos2(w + l, y))], horizon, 10.0, 6.0));
        painter.text(self.layer_to_screen(pos2(0.0, y)) + vec2(6.0, -4.0), Align2::LEFT_BOTTOM, "Horizon", FontId::proportional(12.0), Color32::from_rgb(255, 220, 90));
        // Les quatre directions.
        let repere = Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 150));
        for (deg, nom) in [(0.0, "0° devant"), (90.0, "90° droite"), (180.0, "180° derrière"), (270.0, "270° gauche")] {
            let xs: Vec<f32> = if deg == 180.0 { vec![0.0, w] } else { vec![x_de_l_azimut_sur(deg, self.doc.width)] };
            for x in xs {
                painter.extend(egui::Shape::dashed_line(&[self.layer_to_screen(pos2(x, 0.0)), self.layer_to_screen(pos2(x, h))], repere, 4.0, 6.0));
                painter.text(self.layer_to_screen(pos2(x, 0.0)) + vec2(4.0, 4.0), Align2::LEFT_TOP, nom, FontId::proportional(12.0), Color32::WHITE);
            }
        }
    }

    /// Le bandeau de l'onglet d'un cyclo, au-dessus du dessin : ce qu'on dessine, et « Terminer ».
    pub(crate) fn bandeau_du_cyclo(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(nom) = self.cyclo_edition.as_ref().map(|e| e.nom_sequence.clone()) else { return false };
        ui.label(RichText::new(format!("Cyclo de {nom} : dessin à 360°")).strong());
        let r = ui.button(RichText::new("Terminer").strong());
        if crate::bulles::bulle(r, "Terminer le cyclo", "Pose ce dessin autour du décor de la séquence (Ctrl+Z le retire) et revient où tu étais.", "").clicked() {
            self.terminer_cyclo();
        }
        true
    }
}

/// Un cyclo de repli pour « Redessiner » (la séquence a un dessin mais plus de cyclo : impossible d'ordinaire).
trait ParDefaut {
    fn unwrap_or_default_cyclo(self) -> Cyclo;
}

impl ParDefaut for Option<Cyclo> {
    fn unwrap_or_default_cyclo(self) -> Cyclo {
        self.map(|c| Cyclo { dessin: None, ..c }).unwrap_or_else(|| Preset::Jour.cyclo())
    }
}
