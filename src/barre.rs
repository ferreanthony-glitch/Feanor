//! Les barres de chargement et d'avancement de FEANOR : une seule, partagée (`Barre`), qui prend la place de
//! `egui::ProgressBar` partout où quelque chose charge ou avance (ouverture et enregistrement d'un projet, exports, Kitsu,
//! journal, relecture, collage, scénario, propagation, diagnostic…). Les curseurs, la tête de lecture, les vumètres et les
//! positions de la timeline ne sont PAS des barres d'avancement : ils ne passent pas par ici.
//!
//! **Le dessin** (exception approuvée à la règle « pas d'iris dans l'interface », pour ces barres seulement) : une barre fine,
//! aux bouts tout ronds, sur une piste `canevas` ; le remplissage est un dégradé horizontal jade (0), lilas (0,38), rose
//! (0,66), jaune pâle (1) — les couleurs de l'iridescence du logo. Le dégradé est **ancré sur toute la largeur** de la piste :
//! l'avancement le révèle de gauche à droite. Le remplissage est un maillage à beaucoup de colonnes (pas de bandes visibles).
//!
//! **Le mouvement** : tant que ça charge, les couleurs glissent doucement d'avant en arrière (décalage réfléchi, période de
//! 5,2 s, comme l'iridescence du logo). Une barre à avancement inconnu (`animate(true)`) remplit toute la largeur et glisse de
//! même. Quand egui n'anime plus rien (`animation_time` à 0 : mouvement réduit), le dégradé reste immobile. L'écran n'est
//! repeint (toutes les 50 ms au plus) que pendant que la barre est montrée *et* qu'il reste du travail ; à 100 %, plus rien.
//! Le dessin ne touche jamais le travail qu'il décrit : il lit un nombre que d'autres fils écrivent.
//!
//! **Le texte** reste lisible à côté de la barre, en encre ordinaire : au-dessus d'elle dans une colonne, à sa droite dans une
//! ligne ; avec `show_percentage`, « 42 % ».

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Color32, CornerRadius, Mesh, Rect, Response, Sense, Stroke, StrokeKind, Ui, Widget, WidgetText};

use crate::logo::{IRIS_LILAS, IRIS_PALE, IRIS_ROSE, JADE};
use crate::theme;

/// Les arrêts du dégradé : position, couleur.
pub const ARRETS: [(f32, [u8; 3]); 4] = [(0.0, JADE), (0.38, IRIS_LILAS), (0.66, IRIS_ROSE), (1.0, IRIS_PALE)];
/// Période du va-et-vient des couleurs, en secondes (celle de l'iridescence du logo).
pub const PERIODE: f32 = 5.2;
/// Décalage maximal du dégradé (part de la largeur) au bout d'une demi-période.
pub const DERIVE_MAX: f32 = 0.3;
/// Délai entre deux images pendant que ça charge : un mouvement aussi lent n'en demande pas davantage.
pub const IMAGE_TOUTES_LES: Duration = Duration::from_millis(50);
/// Épaisseur de la barre, en points.
pub const EPAISSEUR: f32 = 8.0;

/// La couleur du dégradé en `t` (0 à 1) : mélange linéaire entre les arrêts.
pub fn couleur_degrade(t: f32) -> Color32 {
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
    for k in 0..ARRETS.len() - 1 {
        let ((p0, c0), (p1, c1)) = (ARRETS[k], ARRETS[k + 1]);
        if t <= p1 {
            let u = if p1 > p0 { (t - p0) / (p1 - p0) } else { 1.0 };
            let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * u).round().clamp(0.0, 255.0) as u8;
            return Color32::from_rgb(m(c0[0], c1[0]), m(c0[1], c1[1]), m(c0[2], c1[2]));
        }
    }
    let c = ARRETS[ARRETS.len() - 1].1;
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Réfléchit `u` dans [0, 1] (le dégradé est « réfléchi » au-delà de ses bouts : 1,2 donne 0,8).
pub fn reflechir(u: f32) -> f32 {
    let m = u.rem_euclid(2.0);
    1.0 - (1.0 - m).abs()
}

/// Le décalage du dégradé à l'instant `temps` (secondes) : de 0 à `DERIVE_MAX` et retour, en `PERIODE` secondes.
pub fn derive(temps: f64) -> f32 {
    let phase = (temps / PERIODE as f64).rem_euclid(1.0) as f32;
    DERIVE_MAX * 0.5 * (1.0 - (std::f32::consts::TAU * phase).cos())
}

/// La couleur à la position `t` (0 à 1 sur toute la largeur de la piste) avec le décalage `derive`.
pub fn couleur_a(t: f32, derive: f32) -> Color32 {
    couleur_degrade(reflechir(t + derive))
}

/// Le maillage du remplissage : `piste` est toute la largeur de la barre, `fraction` la part remplie (0 à 1). Les deux bouts
/// sont arrondis (colonnes serrées dans les arrondis) ; la couleur d'une colonne ne dépend que de sa place sur la piste entière.
pub fn maille(piste: Rect, fraction: f32, derive: f32) -> Mesh {
    let mut m = Mesh::default();
    let fraction = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 };
    let largeur = piste.width() * fraction;
    let r = piste.height() / 2.0;
    if largeur <= 0.0 || r <= 0.0 {
        return m;
    }
    // Abscisses (depuis le bord gauche du remplissage) : serrées dans l'arrondi de chaque bout, régulières entre les deux.
    let mut xs: Vec<f32> = Vec::new();
    let pas_angle = 10;
    if largeur <= 2.0 * r {
        for k in 0..=24 {
            xs.push(largeur * k as f32 / 24.0);
        }
    } else {
        for j in 0..=pas_angle {
            xs.push(r * (1.0 - (std::f32::consts::FRAC_PI_2 * j as f32 / pas_angle as f32).cos()));
        }
        let milieu = ((largeur - 2.0 * r) / 1.5).ceil().clamp(1.0, 400.0) as usize;
        for k in 1..milieu {
            xs.push(r + (largeur - 2.0 * r) * k as f32 / milieu as f32);
        }
        for j in (0..=pas_angle).rev() {
            xs.push(largeur - r * (1.0 - (std::f32::consts::FRAC_PI_2 * j as f32 / pas_angle as f32).cos()));
        }
    }
    let cy = piste.center().y;
    for (k, &x) in xs.iter().enumerate() {
        let d = x.min(largeur - x).max(0.0);
        let demi = if d >= r { r } else { (r * r - (r - d) * (r - d)).max(0.0).sqrt() };
        let couleur = couleur_a(x / piste.width(), derive);
        let cx = piste.left() + x;
        m.colored_vertex(pos2(cx, cy - demi), couleur);
        m.colored_vertex(pos2(cx, cy + demi), couleur);
        if k > 0 {
            let i = (2 * k) as u32;
            m.add_triangle(i - 2, i - 1, i);
            m.add_triangle(i - 1, i + 1, i);
        }
    }
    m
}

/// La barre de chargement : mêmes appels qu'`egui::ProgressBar` (`new`, `desired_width`, `desired_height`, `text`,
/// `show_percentage`, `animate`), pour qu'on puisse la mettre à sa place sans rien changer d'autre.
#[must_use = "ajoute la barre avec `ui.add`"]
pub struct Barre {
    fraction: f32,
    largeur: Option<f32>,
    hauteur: Option<f32>,
    texte: Option<WidgetText>,
    pourcentage: bool,
    inconnue: bool,
}

impl Barre {
    /// Une barre à `fraction` (0 à 1) de remplissage.
    pub fn new(fraction: f32) -> Self {
        Barre { fraction, largeur: None, hauteur: None, texte: None, pourcentage: false, inconnue: false }
    }

    pub fn desired_width(mut self, largeur: f32) -> Self {
        self.largeur = Some(largeur);
        self
    }

    /// (La barre est fine : la hauteur demandée est ramenée entre 6 et 10 points.)
    pub fn desired_height(mut self, hauteur: f32) -> Self {
        self.hauteur = Some(hauteur);
        self
    }

    pub fn text(mut self, texte: impl Into<WidgetText>) -> Self {
        self.texte = Some(texte.into());
        self
    }

    pub fn show_percentage(mut self) -> Self {
        self.pourcentage = true;
        self
    }

    /// Avancement inconnu : toute la largeur est remplie et les couleurs glissent.
    pub fn animate(mut self, inconnue: bool) -> Self {
        self.inconnue = inconnue;
        self
    }
}

impl Widget for Barre {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = theme::palette();
        let fraction = if self.fraction.is_finite() { self.fraction.clamp(0.0, 1.0) } else { 0.0 };
        let en_cours = self.inconnue || fraction < 1.0;
        let texte = self.texte.or_else(|| self.pourcentage.then(|| WidgetText::from(format!("{} %", (fraction * 100.0).round() as i32))));
        let epaisseur = self.hauteur.map_or(EPAISSEUR, |h| h.clamp(6.0, 10.0));
        let horizontale = ui.layout().main_dir().is_horizontal();
        let largeur = self.largeur.unwrap_or_else(|| if horizontale { 120.0 } else { ui.available_width().max(96.0) });
        let mouvement = en_cours && ui.ctx().style().animation_time > 0.0;
        let decalage = if mouvement { derive(ui.input(|i| i.time)) } else { 0.0 };
        if mouvement {
            // Seulement tant qu'il reste du travail, et pas plus vite que ce mouvement lent ne le demande.
            ui.ctx().request_repaint_after(IMAGE_TOUTES_LES);
        }
        let dessiner = |ui: &mut Ui| -> Response {
            let (rect, resp) = ui.allocate_exact_size(vec2(largeur, epaisseur), Sense::hover());
            if ui.is_rect_visible(rect) {
                let rayon = CornerRadius::same((epaisseur / 2.0).round() as u8);
                ui.painter().rect(rect, rayon, p.fond, Stroke::new(1.0_f32, p.bord), StrokeKind::Inside);
                let remplie = if self.inconnue { 1.0 } else { fraction };
                let piste = rect.shrink(1.0);
                if remplie > 0.0 {
                    ui.painter().add(egui::Shape::mesh(maille(piste, remplie, decalage)));
                }
            }
            resp
        };
        if horizontale {
            ui.horizontal(|ui| {
                let r = dessiner(ui);
                if let Some(t) = texte {
                    ui.label(t);
                }
                r
            })
            .inner
        } else {
            ui.vertical(|ui| {
                if let Some(t) = texte {
                    ui.label(t);
                }
                dessiner(ui)
            })
            .inner
        }
    }
}
