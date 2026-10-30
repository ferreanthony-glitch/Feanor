//! Le dessin hors des cases, avec la texture du pinceau (Anthony, 2026-10-30) : un trait tracé hors du champ d'une case a le même rendu
//! que dans une case, même brosse, même texture, même opacité, même taille, même pression. Ce n'est plus une approximation vectorielle.
//!
//! # Ce que garde un trait de la planche
//!
//! Un trait du calque « Hors cadres » (`annotations::Trace`) garde, **en plus** de sa ligne et de sa couleur, la **recette** du trait
//! (`Brosse`) : le réglage complet du pinceau (pointe et papier désignés par leur clé de contenu, comme un trait vectoriel d'un plan),
//! l'opacité, la dureté, le toucher du stylet, la couleur B, l'appui et l'instant de chaque point, et la largeur d'une case en pixels du
//! dessin (`largeur_px`). Les points du trait restent relatifs à leur case, en largeurs de case, comme avant : multipliés par `largeur_px`,
//! ils redonnent les **pixels du dessin où le trait a été tracé**, que le moteur rejoue (`vector::render_isole`) avec le grain du papier lu
//! aux mêmes coordonnées que dans la case. La partie du trait dans une case et celle qui est hors cadres sont donc rejouées par le même
//! moteur, avec le même réglage.
//!
//! # Sans latence
//!
//! - **Pendant le geste** : rien de nouveau (l'aperçu pâle de `debord.rs`).
//! - **Au lâcher** : le trait est réparti (`horschamp.rs`) ; la partie hors cadres est peinte tout de suite comme avant (la ligne vectorielle,
//!   un fallback), pendant que **un fil de fond** rejoue le moteur (`calculer`). Rien n'est rejoué sur le fil de l'interface.
//! - **Le rendu fini** est une pyramide de pixels (`Pyramide` : la taille d'origine, puis moitié, quart…, en blocs de 256 pixels, creux),
//!   gardée par le trait, partagée avec ses copies (annulation, cliché d'export) : calculée une seule fois, jamais refaite tant que le
//!   trait ne change pas (déplacé, mis à l'échelle ou rattaché ailleurs : un rendu neuf, à la demande).
//! - **À l'écran** (`Textures`) : seuls les blocs des traits visibles sont envoyés à la carte graphique, au niveau de la pyramide qui convient
//!   au zoom (pas de scintillement en dézoomant, pas de flou en zoomant), dans une limite d'octets par image d'écran ; ce qui n'est pas prêt
//!   se peint en vectoriel. Les textures qu'on ne voit plus sont rendues. Le coût d'une image d'écran ne dépend pas du nombre de traits de la
//!   planche, seulement de ceux qu'on voit.
//! - **À l'export** : la planche s'écrit sur un fil de fond ; elle prend la pyramide du trait (ou la calcule, sur ce fil), au niveau qui
//!   convient à la résolution (`peindre_pixmap`).
//!
//! # Fichier
//!
//! Dans `annotations.json`, un trait du calque « Hors cadres » porte en plus la clé `pinceau` (la recette). Un fichier d'avant ne l'a pas :
//! le trait reste une ligne vectorielle ; une version d'avant, qui ne connaît pas la clé, lit le trait comme une ligne. `VERSION` du fichier
//! et `project::VERSION` ne changent pas. Les textures citées par la recette sont écrites avec le projet comme celles des traits des plans
//! (`Annotations::textures_citees`).

use std::collections::{BTreeSet, HashMap};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, LazyLock, Mutex};

use eframe::egui::{self, pos2, Color32, ColorImage, Pos2, Rect, TextureHandle, TextureOptions};
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

use crate::brush::{BrushSettings, Toucher};
use crate::brushtex::TextureKind;
use crate::raster::{Raster, TILE};
use crate::vector::{self, Trace as VTrace, VPoint};

/// Côté d'un bloc de la pyramide, en pixels de son niveau.
pub const BLOC: usize = 256;
/// Niveaux de la pyramide : 1, 1/2, 1/4… jusqu'à 1/32.
pub const NIVEAUX: usize = 6;

/// Ce qu'il faut, avec les appuis et les instants, pour rejouer un trait comme dans une case (`vector::Element::Stroke`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recette {
    /// Le réglage complet du pinceau (pointe et papier par clé de contenu) ; aucun : trait lisse (une forme).
    pub reglage: Option<BrushSettings>,
    pub opacite: f32,
    pub durete: f32,
    pub toucher: Option<Toucher>,
    pub couleur_b: Option<[u8; 3]>,
}

/// Le rendu d'un trait, calculé une fois (sur un fil de fond) et gardé.
#[derive(Default)]
pub struct Rendu {
    etat: Mutex<EtatRendu>,
}

#[derive(Default)]
enum EtatRendu {
    #[default]
    Vide,
    EnCours,
    Pret(Arc<Pyramide>),
    /// Trait vide ou démesuré, ou calcul perdu : le trait reste une ligne vectorielle.
    Perdu,
}

impl std::fmt::Debug for Rendu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let etat = match self.etat.lock().map(|e| match &*e {
            EtatRendu::Vide => "vide",
            EtatRendu::EnCours => "en cours",
            EtatRendu::Pret(_) => "prêt",
            EtatRendu::Perdu => "perdu",
        }) {
            Ok(s) => s,
            Err(_) => "?",
        };
        write!(f, "rendu {etat}")
    }
}

/// Un bloc de pixels (prémultipliés) de la pyramide : le bloc `x`, `y` de son niveau, **rogné à ce qu'il a d'encre** (plus un pixel de
/// marge) : le rectangle `ox`, `oy` (pixels, depuis le coin du bloc), `w` × `h`. Un trait fin ne coûte que son encre.
#[derive(Clone)]
pub struct Bloc {
    pub x: usize,
    pub y: usize,
    pub ox: usize,
    pub oy: usize,
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<Color32>,
}

impl Bloc {
    pub fn octets(&self) -> usize {
        self.w * self.h * 4
    }
}

#[derive(Clone)]
pub struct Niveau {
    /// 1, 2, 4… : pixels du dessin par pixel de ce niveau.
    pub facteur: usize,
    pub blocs: Vec<Bloc>,
}

/// Les pixels d'un trait rendu : `origine` est la place, en pixels du dessin où le trait a été tracé, du pixel (0, 0) du niveau 0.
pub struct Pyramide {
    pub origine: (i64, i64),
    pub niveaux: Vec<Niveau>,
    /// La signature des points d'où elle vient (`cle_points`) : un trait dont les points ont changé n'en veut plus.
    pub cle: u64,
}

impl Pyramide {
    pub fn octets(&self) -> usize {
        self.niveaux.iter().flat_map(|n| &n.blocs).map(Bloc::octets).sum()
    }

    /// Le niveau qui convient quand un pixel du dessin couvre `par_pixel` pixels de la destination : le plus réduit dont les pixels couvrent
    /// encore au moins la moitié d'un pixel de destination.
    pub fn niveau_pour(&self, par_pixel: f32) -> usize {
        if !(par_pixel > 0.0) || par_pixel >= 1.0 {
            return 0;
        }
        let k = (1.0 / par_pixel).log2().floor().max(0.0) as usize;
        k.min(self.niveaux.len().saturating_sub(1))
    }

    /// La somme des opacités du niveau d'origine (essais : le trait a de l'encre).
    pub fn alpha_total(&self) -> u64 {
        self.niveaux.first().map_or(0, |n| n.blocs.iter().flat_map(|b| &b.pixels).map(|p| u64::from(p.a())).sum())
    }

    /// La boîte du trait rendu, en pixels du dessin.
    pub fn boite(&self) -> Option<Rect> {
        let n = self.niveaux.first()?;
        let mut r = Rect::NOTHING;
        for b in &n.blocs {
            r = r.union(Rect::from_min_size(pos2((self.origine.0 + (b.x * BLOC + b.ox) as i64) as f32, (self.origine.1 + (b.y * BLOC + b.oy) as i64) as f32), egui::vec2(b.w as f32, b.h as f32)));
        }
        r.is_finite().then_some(r)
    }

    fn depuis(sheet: &Raster, origine: (i64, i64), cle: u64) -> Pyramide {
        let t = TILE;
        let dessinees: Vec<(usize, usize)> = sheet
            .tiles
            .iter()
            .enumerate()
            .filter(|(_, tile)| tile.as_ref().is_some_and(|tile| tile.iter().any(|c| c.a() != 0)))
            .map(|(i, _)| (i % sheet.tiles_x, i / sheet.tiles_x))
            .collect();
        let mut niveaux = Vec::new();
        for k in 0..NIVEAUX {
            let f = 1usize << k;
            let mut set: BTreeSet<(usize, usize)> = BTreeSet::new();
            for &(tx, ty) in &dessinees {
                let (x0, x1) = (tx * t / f, ((tx + 1) * t - 1) / f);
                let (y0, y1) = (ty * t / f, ((ty + 1) * t - 1) / f);
                for by in y0 / BLOC..=y1 / BLOC {
                    for bx in x0 / BLOC..=x1 / BLOC {
                        set.insert((bx, by));
                    }
                }
            }
            let mut blocs = Vec::new();
            for (bx, by) in set {
                let mut pixels = vec![Color32::TRANSPARENT; BLOC * BLOC];
                let mut any = false;
                for j in 0..BLOC {
                    for i in 0..BLOC {
                        let (sx, sy) = ((bx * BLOC + i) * f, (by * BLOC + j) * f);
                        if sx >= sheet.width || sy >= sheet.height {
                            continue;
                        }
                        let c = if f == 1 { sheet.get(sx, sy) } else { moyenne(sheet, sx, sy, f) };
                        any |= c.a() != 0;
                        pixels[j * BLOC + i] = c;
                    }
                }
                if any {
                    // Rogné à l'encre, avec un pixel transparent autour (l'échantillonnage lisse ne mord pas sur le bord).
                    let (mut x0, mut y0, mut x1, mut y1) = (BLOC, BLOC, 0, 0);
                    for j in 0..BLOC {
                        for i in 0..BLOC {
                            if pixels[j * BLOC + i].a() != 0 {
                                (x0, y0, x1, y1) = (x0.min(i), y0.min(j), x1.max(i + 1), y1.max(j + 1));
                            }
                        }
                    }
                    let (x0, y0, x1, y1) = (x0.saturating_sub(1), y0.saturating_sub(1), (x1 + 1).min(BLOC), (y1 + 1).min(BLOC));
                    let (w, h) = (x1 - x0, y1 - y0);
                    let mut rogne = Vec::with_capacity(w * h);
                    for j in y0..y1 {
                        rogne.extend_from_slice(&pixels[j * BLOC + x0..j * BLOC + x1]);
                    }
                    blocs.push(Bloc { x: bx, y: by, ox: x0, oy: y0, w, h, pixels: rogne });
                }
            }
            niveaux.push(Niveau { facteur: f, blocs });
        }
        Pyramide { origine, niveaux, cle }
    }
}

/// La moyenne (prémultipliée) du carré de `f` × `f` pixels de `sheet` qui commence en (`x`, `y`).
fn moyenne(sheet: &Raster, x: usize, y: usize, f: usize) -> Color32 {
    let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
    for v in 0..f {
        for u in 0..f {
            let c = sheet.get(x + u, y + v);
            r += u32::from(c.r());
            g += u32::from(c.g());
            b += u32::from(c.b());
            a += u32::from(c.a());
        }
    }
    let n = (f * f) as u32;
    Color32::from_rgba_premultiplied(((r + n / 2) / n) as u8, ((g + n / 2) / n) as u8, ((b + n / 2) / n) as u8, ((a + n / 2) / n) as u8)
}

/// La recette d'un trait de la planche, avec ses appuis et ses instants (alignés sur ses points, ou vides).
#[derive(Clone, Debug)]
pub struct Brosse {
    pub recette: Recette,
    pub appui: Vec<f32>,
    pub instants: Vec<f64>,
    /// La largeur de la case du trait, en pixels du dessin où il a été tracé : les points relatifs multipliés par elle sont ces pixels.
    pub largeur_px: f32,
    /// Combien la taille du pinceau a été multipliée depuis (le trait mis à l'échelle au lasso) : 1 d'origine.
    pub taille: f32,
    rendu: Arc<Rendu>,
}

impl PartialEq for Brosse {
    fn eq(&self, o: &Brosse) -> bool {
        self.recette == o.recette && self.appui == o.appui && self.instants == o.instants && self.largeur_px == o.largeur_px && self.taille == o.taille
    }
}

/// La signature des points d'un trait : un trait dont les points ont bougé ne reprend pas un rendu fait pour d'autres.
pub fn cle_points(points: &[VPoint]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: f32| {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(points.len() as f32);
    for p in points.iter().step_by(points.len().div_ceil(8).max(1)).chain(points.last()) {
        eat(p.x);
        eat(p.y);
        eat(p.r);
    }
    h
}

impl Brosse {
    pub fn nouvelle(recette: Recette, appui: Vec<f32>, instants: Vec<f64>, largeur_px: f32) -> Brosse {
        Brosse { recette, appui, instants, largeur_px, taille: 1.0, rendu: Arc::new(Rendu::default()) }
    }

    /// La même recette, rendu à refaire (les points ont changé).
    pub fn refaite(&self) -> Brosse {
        Brosse { rendu: Arc::new(Rendu::default()), ..self.clone() }
    }

    /// La recette d'un trait dont l'épaisseur a été multipliée par `k` : la taille du pinceau suit, le rendu est à refaire.
    pub fn mise_a_l_echelle(&self, k: f32) -> Brosse {
        let k = if k.is_finite() && k > 0.0 { k } else { 1.0 };
        Brosse { taille: self.taille * k, rendu: Arc::new(Rendu::default()), ..self.clone() }
    }

    /// Les textures (pointe, papier) que cite le réglage.
    pub fn textures(&self) -> Vec<(TextureKind, u32)> {
        let mut out = Vec::new();
        if let Some(b) = &self.recette.reglage {
            out.extend(b.tip.map(|k| (TextureKind::Tip, k)));
            out.extend(b.paper.map(|k| (TextureKind::Paper, k)));
        }
        out
    }

    /// Les points relatifs `rel` du trait, en pixels du dessin où il a été tracé.
    pub fn points_px(&self, rel: &[VPoint]) -> Vec<VPoint> {
        let l = self.largeur_px;
        rel.iter().map(|p| VPoint { x: p.x * l, y: p.y * l, r: p.r * l }).collect()
    }

    /// Le rendu fini du trait de points `rel`, s'il est prêt.
    pub fn pyramide(&self, rel: &[VPoint]) -> Option<Arc<Pyramide>> {
        match &*self.rendu.etat.lock().ok()? {
            EtatRendu::Pret(p) if p.cle == cle_points(rel) => Some(p.clone()),
            _ => None,
        }
    }

    /// Le rendu est perdu (démesuré, ou le calcul a échoué) : le trait reste une ligne vectorielle.
    pub fn perdu(&self) -> bool {
        self.rendu.etat.lock().is_ok_and(|e| matches!(&*e, EtatRendu::Perdu))
    }

    /// Un rendu est-il prêt ou en cours (essais) ?
    pub fn etat(&self) -> &'static str {
        match self.rendu.etat.lock().map(|e| match &*e {
            EtatRendu::Vide => "vide",
            EtatRendu::EnCours => "en cours",
            EtatRendu::Pret(_) => "prêt",
            EtatRendu::Perdu => "perdu",
        }) {
            Ok(s) => s,
            Err(_) => "?",
        }
    }

    fn travail(&self, rel: &[VPoint], couleur: [u8; 3], ctx: Option<egui::Context>) -> Travail {
        Travail { rendu: self.rendu.clone(), pts: self.points_px(rel), couleur, brosse: self.clone(), cle: cle_points(rel), ctx }
    }

    /// Demande le rendu du trait à un fil de fond (une seule fois : tant qu'il est en cours ou prêt, rien). Jamais sur le fil de l'interface.
    pub fn demander(&self, rel: &[VPoint], couleur: [u8; 3], ctx: &egui::Context) {
        let Ok(mut e) = self.rendu.etat.lock() else { return };
        match &*e {
            EtatRendu::Vide => {}
            // (Prêt pour d'autres points : le trait a changé sans refaire sa recette, il redemande.)
            EtatRendu::Pret(p) if p.cle != cle_points(rel) => {}
            _ => return,
        }
        *e = EtatRendu::EnCours;
        drop(e);
        envoyer(self.travail(rel, couleur, Some(ctx.clone())));
    }

    /// Le rendu du trait, prêt ou calculé tout de suite **sur le fil appelant** (l'export, sur son fil de fond ; les essais). Rien : trait vide
    /// ou démesuré.
    pub fn pyramide_ou_calcul(&self, rel: &[VPoint], couleur: [u8; 3]) -> Option<Arc<Pyramide>> {
        if let Some(p) = self.pyramide(rel) {
            return Some(p);
        }
        let t = self.travail(rel, couleur, None);
        let p = calculer(&t).map(Arc::new);
        if let Ok(mut e) = self.rendu.etat.lock() {
            *e = match &p {
                Some(p) => EtatRendu::Pret(p.clone()),
                None => EtatRendu::Perdu,
            };
        }
        p
    }

    /// Peint le trait dans `pix` (l'export de la planche) : `case` est la case du trait sur la planche, `cadre` ce que l'image montre, `s`
    /// l'échelle (pixels par unité de la planche). Vrai si le trait est peint.
    pub fn peindre_pixmap(&self, pix: &mut Pixmap, rel: &[VPoint], couleur: [u8; 3], case: Rect, cadre: Rect, s: f32) -> bool {
        let Some(py) = self.pyramide_ou_calcul(rel, couleur) else { return false };
        // Une unité de la planche = `k / L` ; un pixel du dessin = `k / L` unités = `s k / L` pixels de l'image.
        let par_px = s * case.width() / self.largeur_px.max(1.0);
        let niveau = &py.niveaux[py.niveau_pour(par_px)];
        let sc = par_px * niveau.facteur as f32;
        let k = case.width() / self.largeur_px.max(1.0);
        let mut paint = PixmapPaint::default();
        paint.quality = if sc >= 0.999 && sc <= 1.001 { FilterQuality::Nearest } else { FilterQuality::Bilinear };
        for b in &niveau.blocs {
            let mut octets = Vec::with_capacity(b.w * b.h * 4);
            for c in &b.pixels {
                octets.extend_from_slice(&[c.r(), c.g(), c.b(), c.a()]);
            }
            let Some(bloc) = IntSize::from_wh(b.w as u32, b.h as u32).and_then(|sz| Pixmap::from_vec(octets, sz)) else { continue };
            let (px, py_) = (py.origine.0 as f32 + ((b.x * BLOC + b.ox) * niveau.facteur) as f32, py.origine.1 as f32 + ((b.y * BLOC + b.oy) * niveau.facteur) as f32);
            let (x, y) = ((case.left() + px * k - cadre.left()) * s, (case.top() + py_ * k - cadre.top()) * s);
            pix.draw_pixmap(0, 0, bloc.as_ref(), &paint, Transform::from_row(sc, 0.0, 0.0, sc, x, y), None);
        }
        true
    }
}

// ------------------------------------------------------------------------------------ le fil de fond

struct Travail {
    rendu: Arc<Rendu>,
    pts: Vec<VPoint>,
    couleur: [u8; 3],
    brosse: Brosse,
    cle: u64,
    ctx: Option<egui::Context>,
}

/// Le calcul : le trait rejoué par le moteur sur une feuille à sa mesure, puis la pyramide.
fn calculer(t: &Travail) -> Option<Pyramide> {
    let b = &t.brosse;
    let reglage = b.recette.reglage.map(|mut r| {
        r.size *= b.taille;
        r
    });
    let trace = VTrace { pressure: b.appui.clone(), time: b.instants.clone(), feel: b.recette.toucher, secondary: b.recette.couleur_b, cache: Default::default() };
    let (sheet, origine) = vector::render_isole(&t.pts, t.couleur, b.recette.opacite, b.recette.durete, reglage.as_ref(), &trace)?;
    Some(Pyramide::depuis(&sheet, origine, t.cle))
}

static FIL: LazyLock<Mutex<Option<Sender<Travail>>>> = LazyLock::new(|| Mutex::new(None));

fn envoyer(mut t: Travail) {
    let Ok(mut fil) = FIL.lock() else { return };
    for _ in 0..2 {
        if let Some(tx) = fil.as_ref() {
            match tx.send(t) {
                Ok(()) => return,
                // (Le fil est mort : on en refait un, avec le même travail.)
                Err(e) => t = e.0,
            }
        }
        let (tx, rx) = channel::<Travail>();
        let ok = std::thread::Builder::new()
            .name("hors-cadres".into())
            .spawn(move || {
                for t in rx {
                    // Plus personne ne veut ce rendu (le trait a été supprimé) : rien à faire.
                    if Arc::strong_count(&t.rendu) <= 1 {
                        continue;
                    }
                    let fini = catch_unwind(AssertUnwindSafe(|| calculer(&t)));
                    if let Ok(mut e) = t.rendu.etat.lock() {
                        *e = match fini {
                            Ok(Some(p)) => EtatRendu::Pret(Arc::new(p)),
                            _ => EtatRendu::Perdu,
                        };
                    }
                    if let Some(ctx) = &t.ctx {
                        ctx.request_repaint();
                    }
                }
            })
            .is_ok();
        if !ok {
            return;
        }
        *fil = Some(tx);
    }
}

// ------------------------------------------------------------------------------------ à l'écran

/// Les octets de textures qu'une image d'écran peut envoyer à la carte graphique, et ce que garde le tout (au-delà, les moins récemment vues
/// sont rendues).
const BUDGET_IMAGE: usize = 4 << 20;
const BUDGET_TOTAL: usize = 192 << 20;
/// Une texture qu'on n'a pas vue depuis autant d'images d'écran est rendue.
const OUBLI: u64 = 240;

struct Entree {
    pyramide: Arc<Pyramide>,
    niveau: usize,
    blocs: Vec<(usize, TextureHandle)>,
    vu: u64,
}

impl Entree {
    fn octets(&self) -> usize {
        self.blocs.iter().map(|(i, _)| self.pyramide.niveaux[self.niveau].blocs[*i].octets()).sum()
    }
}

/// Les textures des traits de la planche, à la carte graphique (sur le fil de l'interface : un envoi par bloc, borné par image d'écran).
#[derive(Default)]
pub struct Textures {
    entrees: HashMap<usize, Entree>,
    image: u64,
    envoyes: usize,
    /// Octets envoyés depuis le début (essais : un zoom ou un défilement ne renvoie pas ce qui est déjà là).
    pub total_envoye: usize,
}

impl Textures {
    /// Début d'une image d'écran : le budget d'envoi est remis, les textures oubliées sont rendues.
    pub fn debut(&mut self) {
        self.image += 1;
        self.envoyes = 0;
        let image = self.image;
        self.entrees.retain(|_, e| e.vu + OUBLI >= image);
        let mut total: usize = self.entrees.values().map(Entree::octets).sum();
        if total > BUDGET_TOTAL {
            let mut ages: Vec<(u64, usize)> = self.entrees.iter().map(|(k, e)| (e.vu, *k)).collect();
            ages.sort_unstable();
            for (_, k) in ages {
                if total <= BUDGET_TOTAL / 10 * 9 {
                    break;
                }
                if let Some(e) = self.entrees.remove(&k) {
                    total -= e.octets();
                }
            }
        }
    }

    /// Les octets de textures qui sont à la carte graphique (essais).
    #[cfg(test)]
    pub fn octets(&self) -> usize {
        self.entrees.values().map(Entree::octets).sum()
    }

    pub fn nombre(&self) -> usize {
        self.entrees.len()
    }

    /// Peint le trait de points `rel` posé dans `case` (la case de la planche) : ses blocs visibles, au niveau qui convient au zoom
    /// `xf.zoom`, à l'opacité `teinte` (blanc prémultiplié d'alpha 255 : plein). Faux si le rendu n'est pas prêt, ou si le budget d'envoi de
    /// l'image est épuisé : l'appelant peint la ligne vectorielle.
    #[allow(clippy::too_many_arguments)]
    pub fn peindre(&mut self, painter: &egui::Painter, view: Rect, xf: crate::boardui::Xf, brosse: &Brosse, rel: &[VPoint], case: Rect, teinte: Color32) -> bool {
        let Some(py) = brosse.pyramide(rel) else { return false };
        let k = case.width() / brosse.largeur_px.max(1.0);
        let niveau = py.niveau_pour(k * xf.zoom);
        let cle = Arc::as_ptr(&brosse.rendu) as usize;
        let f = py.niveaux[niveau].facteur;
        // Le rectangle d'un bloc (rogné) sur la planche, puis à l'écran.
        let place = |b: &Bloc| {
            let min = pos2(case.left() + (py.origine.0 as f32 + ((b.x * BLOC + b.ox) * f) as f32) * k, case.top() + (py.origine.1 as f32 + ((b.y * BLOC + b.oy) * f) as f32) * k);
            xf.rect(Rect::from_min_size(min, egui::vec2((b.w * f) as f32 * k, (b.h * f) as f32 * k)))
        };
        let visibles: Vec<usize> = py.niveaux[niveau].blocs.iter().enumerate().filter(|(_, b)| place(b).intersects(view)).map(|(i, _)| i).collect();
        if visibles.is_empty() {
            return true;
        }
        let bonne = self.entrees.get(&cle).is_some_and(|e| Arc::ptr_eq(&e.pyramide, &py) && e.niveau == niveau);
        if !bonne {
            self.entrees.insert(cle, Entree { pyramide: py.clone(), niveau, blocs: Vec::new(), vu: self.image });
        }
        let Some(entree) = self.entrees.get_mut(&cle) else { return false };
        entree.vu = self.image;
        let mut complet = true;
        for i in visibles {
            if !entree.blocs.iter().any(|(j, _)| *j == i) {
                if self.envoyes >= BUDGET_IMAGE {
                    complet = false;
                    continue;
                }
                let bloc = &py.niveaux[niveau].blocs[i];
                let image = ColorImage { size: [bloc.w, bloc.h], pixels: bloc.pixels.clone() };
                let tex = painter.ctx().load_texture(format!("hors-cadres {cle:x} {niveau} {i}"), image, TextureOptions::LINEAR);
                entree.blocs.push((i, tex));
                self.envoyes += bloc.octets();
                self.total_envoye += bloc.octets();
            }
        }
        if !complet {
            return false;
        }
        for (i, tex) in &entree.blocs {
            let bloc = &py.niveaux[niveau].blocs[*i];
            let r = place(bloc);
            if !r.intersects(view) {
                continue;
            }
            let mut mesh = egui::Mesh::with_texture(tex.id());
            mesh.add_rect_with_uv(r, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), teinte);
            painter.add(egui::Shape::mesh(mesh));
        }
        true
    }
}

/// Les indices gardés quand un trait de `n` points est réduit à `max` points (`annotations::reduire` : réguliers, le premier et le dernier
/// restent).
pub fn indices_reduits(n: usize, max: usize) -> Vec<usize> {
    let max = max.max(2);
    if n <= max {
        return (0..n).collect();
    }
    (0..max).map(|k| (k * (n - 1)) / (max - 1)).collect()
}
