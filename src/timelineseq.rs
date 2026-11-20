//! Partie 201 (`docs/INTERFACE.md` partie 201) : **le zoom et le défilement de la timeline de la séquence** de l'onglet 3D (partie
//! 198), comme la bande du Storyboard (`zoombande.rs`) :
//!
//! - **les mêmes gestes** : Ctrl + molette zoome autour de la tête de lecture ; la molette (ou Maj + molette) fait défiler ; les
//!   raccourcis du zoom de la bande (`bande.zoom_avant`, `bande.zoom_arriere`) et ceux de l'image quand le pointeur est sur la bande
//!   (`vue.zoom_*`, la règle de la partie 192) ; `bande.tout_voir` : toute la séquence ;
//! - **une réglette de zoom** et **« Ajuster »** en tête de la bande ;
//! - **la règle** gradue selon le zoom (au moins `ECART_GRADUATION` points entre deux chiffres : des images, des secondes, des minutes) ;
//! - **la tête de lecture reste visible** pendant la lecture (la bande tourne la page quand elle en sort).
//!
//! L'état (`Onglet3d::zoom_seq`, `Onglet3d::debut_seq`) est de la séance, pas dans le projet.

use eframe::egui::{self, pos2, Rect};

use crate::app::App;

/// Le zoom le plus fort : une image de la séquence sur la largeur divisée par 1, … ; 1 : toute la séquence.
pub const ZOOM_MAX: f32 = 200.0;
/// Le pas d'un cran de zoom (raccourci, bouton).
pub const CRAN: f32 = 1.25;
/// Points au moins entre deux chiffres de la règle.
pub const ECART_GRADUATION: f32 = 56.0;

/// Ce que montre la piste : la séquence (`debut`, image globale de sa première image ; `n` images), de son image `depart` (relative,
/// fractionnaire) sur `visibles` images, dans le rectangle `piste`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fenetre {
    pub piste: Rect,
    pub debut: usize,
    pub n: usize,
    pub depart: f32,
    pub visibles: f32,
}

impl Fenetre {
    /// La fenêtre d'une séquence de `n` images, au zoom `zoom` et partant de `depart` (ramené dans la séquence).
    pub fn de(piste: Rect, debut: usize, n: usize, zoom: f32, depart: f32) -> Fenetre {
        let n = n.max(1);
        let visibles = n as f32 / zoom.clamp(1.0, ZOOM_MAX);
        let depart = depart.clamp(0.0, (n as f32 - visibles).max(0.0));
        Fenetre { piste, debut, n, depart, visibles }
    }

    /// L'abscisse de l'image globale `f` (son bord gauche).
    pub fn x_de(&self, f: usize) -> f32 {
        self.piste.left() + ((f as f32 - self.debut as f32) - self.depart) / self.visibles.max(1e-3) * self.piste.width()
    }

    /// L'image globale sous l'abscisse `x` (dans la séquence).
    pub fn f_de(&self, x: f32) -> usize {
        let rel = self.depart + (x - self.piste.left()) / self.piste.width().max(1.0) * self.visibles;
        self.debut + rel.floor().clamp(0.0, (self.n - 1) as f32) as usize
    }

    /// Points par image.
    pub fn points_par_image(&self) -> f32 {
        self.piste.width() / self.visibles.max(1e-3)
    }
}

/// Le pas de la règle (en images), pour `ppi` points par image à `fps` images par seconde : le plus petit qui laisse
/// `ECART_GRADUATION` points entre deux chiffres.
pub fn pas_de_la_regle(ppi: f32, fps: usize) -> usize {
    let fps = fps.max(1);
    let mut pas: Vec<usize> = vec![1, 2, 5, 10];
    for s in [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1800, 3600] {
        pas.push(s * fps);
    }
    pas.sort_unstable();
    pas.dedup();
    pas.iter().copied().find(|&p| p as f32 * ppi >= ECART_GRADUATION).unwrap_or(3600 * fps)
}

/// Le chiffre d'une graduation à l'image `f` (relative) : des secondes (« 12 s »), avec l'image quand le pas est sous la seconde
/// (« 3 s 06 »), des minutes au-delà (« 2 min 30 »).
pub fn chiffre(f: usize, fps: usize, pas: usize) -> String {
    let fps = fps.max(1);
    let (s, i) = (f / fps, f % fps);
    if pas < fps {
        return if i == 0 { format!("{s} s") } else { format!("{s} s {i:02}") };
    }
    if s >= 60 {
        return format!("{} min {:02}", s / 60, s % 60);
    }
    format!("{s} s")
}

impl App {
    /// Le zoom de la timeline de la séquence par le facteur `facteur`, autour de la tête de lecture (au milieu si elle n'est pas
    /// dans la vue).
    pub(crate) fn o3_zoomer_la_timeline(&mut self, facteur: f32) {
        let Some(f) = self.o3.fenetre_seq else {
            self.o3.zoom_seq = (self.o3.zoom_seq * facteur).clamp(1.0, ZOOM_MAX);
            return;
        };
        let tete = (self.global_frame().saturating_sub(f.debut)) as f32 + 0.5;
        let part = ((tete - f.depart) / f.visibles).clamp(-0.01, 1.01);
        let part = if (0.0..=1.0).contains(&part) { part } else { 0.5 };
        let zoom = (self.o3.zoom_seq * facteur).clamp(1.0, ZOOM_MAX);
        let visibles = f.n as f32 / zoom;
        self.o3.zoom_seq = zoom;
        self.o3.debut_seq = (tete - part * visibles).clamp(0.0, (f.n as f32 - visibles).max(0.0));
    }

    /// « Ajuster » : toute la séquence dans la largeur.
    pub(crate) fn o3_ajuster_la_timeline(&mut self) {
        self.o3.zoom_seq = 1.0;
        self.o3.debut_seq = 0.0;
    }

    /// La réglette de zoom et « Ajuster », en tête de la bande.
    pub(crate) fn o3_reglette_du_zoom(&mut self, ui: &mut egui::Ui) {
        let r = ui.button("Ajuster");
        if crate::bulles::bulle(r, "Ajuster la timeline", "Toute la séquence dans la largeur de la bande.", "bande.tout_voir").clicked() {
            self.o3_ajuster_la_timeline();
        }
        let mut z = self.o3.zoom_seq;
        let r = ui.add(egui::Slider::new(&mut z, 1.0..=ZOOM_MAX).logarithmic(true).show_value(false));
        let r = crate::bulles::bulle(r, "Zoom de la timeline", "Rapproche ou éloigne la timeline de la séquence, autour de la tête de lecture (aussi : Ctrl + molette sur la bande, et les raccourcis du zoom de la bande).", "");
        if r.changed() && z > 0.0 {
            let facteur = z / self.o3.zoom_seq.max(1e-3);
            self.o3_zoomer_la_timeline(facteur);
        }
    }

    /// Les gestes de la molette sur la timeline de la séquence (`dessus` : le pointeur y est) : Ctrl, zoom autour de la tête de
    /// lecture ; sinon, défilement. Et, pendant la lecture, la tête gardée dans la vue.
    pub(crate) fn o3_gestes_de_la_timeline(&mut self, ui: &egui::Ui, dessus: bool, f: Fenetre) {
        self.o3.fenetre_seq = Some(f);
        if dessus {
            // (Ctrl + molette : egui en fait un zoom, `zoom_delta`, comme sur la bande du Storyboard.)
            let (delta, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            if zoom != 1.0 {
                self.o3_zoomer_la_timeline(zoom);
            } else {
                let d = if delta.x.abs() > delta.y.abs() { delta.x } else { delta.y };
                if d.abs() > 0.0 && f.visibles < f.n as f32 {
                    self.o3.debut_seq = (f.depart - d / f.points_par_image()).clamp(0.0, (f.n as f32 - f.visibles).max(0.0));
                }
            }
        }
        if self.playing && f.visibles < f.n as f32 {
            let tete = (self.global_frame().saturating_sub(f.debut)) as f32;
            if tete < f.depart || tete >= f.depart + f.visibles {
                // (La page tourne : la tête revient au dixième de la largeur.)
                self.o3.debut_seq = (tete - 0.1 * f.visibles).clamp(0.0, (f.n as f32 - f.visibles).max(0.0));
                ui.ctx().request_repaint();
            }
        }
    }
}

/// (Pour les essais et la règle : le rectangle d'une image globale `f` dans la piste.)
#[allow(dead_code)]
pub fn rect_de(f: &Fenetre, image: usize, haut: f32, bas: f32) -> Rect {
    Rect::from_min_max(pos2(f.x_de(image), haut), pos2(f.x_de(image + 1), bas))
}
