use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2};
use serde::{Deserialize, Serialize};

use crate::brushtex::{grain_factor, BrushTextures};
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS};
use crate::vector::VPoint;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct BrushSettings {
    /// Diamètre en pixels.
    #[serde(rename = "taille")]
    pub size: f32,
    #[serde(rename = "opacite")]
    pub opacity: f32,
    /// Débit (moteur de dessin, étape 4) : la peinture déposée à chaque empreinte, le « Power »
    /// de TVPaint ; l'opacité en reste le plafond.
    #[serde(rename = "debit")]
    pub power: f32,
    /// Le débit s'accumule dans le trait : repasser fonce, jusqu'à l'opacité. Sinon (comme
    /// avant), chaque pixel garde la plus forte empreinte reçue : le trait reste uniforme.
    #[serde(rename = "accumuler")]
    pub build_up: bool,
    /// 0 = bord très flou, 1 = bord net.
    #[serde(rename = "durete")]
    pub hardness: f32,
    /// Profil de la pointe (moteur de dessin, étape 3 ; le « Profile » de TVPaint, « vue de côté
    /// de la pointe ») : la couverture (y) du centre (x = 0) au bord (x = 1). Il remplace la
    /// dureté ; aucun : la dureté.
    #[serde(rename = "profil", skip_serializing_if = "Option::is_none")]
    pub profile: Option<Curve>,
    /// Distance entre deux empreintes, en fraction du diamètre.
    #[serde(rename = "espacement")]
    pub spacing: f32,
    /// Stabilisation du trait (0 = aucune), pour le lissage « Suivi ».
    #[serde(rename = "lissage")]
    pub smoothing: f32,
    /// Type de lissage (moteur de dessin, étape 5 ; les types de « Line Smoothing » de TVPaint).
    #[serde(rename = "lissage_type")]
    pub smooth_kind: SmoothKind,
    /// Longueur de la corde, ou distance de la moyenne, en pixels du dessin.
    #[serde(rename = "lissage_px")]
    pub smooth_px: f32,
    /// Nombre de points de la moyenne sur des points.
    #[serde(rename = "lissage_points")]
    pub smooth_points: u32,
    #[serde(rename = "pression_taille")]
    pub pressure_size: bool,
    #[serde(rename = "pression_opacite")]
    pub pressure_opacity: bool,
    /// Pixels francs, sans anticrénelage (crayon).
    #[serde(rename = "pixels_nets")]
    pub aliased: bool,
    /// Taille (en part de la taille) et opacité quand on effleure, si elles suivent la pression.
    #[serde(rename = "taille_min")]
    pub min_size: f32,
    #[serde(rename = "opacite_min")]
    pub min_opacity: f32,
    /// Force du grain du papier (0 = papier lisse).
    pub grain: f32,
    /// Papier et pointe en image (numéros dans `BrushLibrary`) ; pas de pointe : pointe ronde.
    #[serde(rename = "papier")]
    pub paper: Option<u32>,
    #[serde(rename = "pointe")]
    pub tip: Option<u32>,
    /// Pointe aplatie (1 = ronde) et tournée de `angle` degrés, plus un hasard de ± `angle_jitter`/2.
    #[serde(rename = "rondeur")]
    pub roundness: f32,
    pub angle: f32,
    #[serde(rename = "angle_hasard")]
    pub angle_jitter: f32,
    /// Variation de taille au hasard, en part de la taille.
    #[serde(rename = "taille_hasard")]
    pub size_jitter: f32,
    /// Dispersion (moteur de dessin, étape 6 ; le « Jitter » de TVPaint, les « Jitter Lateral /
    /// Linear » de Procreate) : chaque empreinte s'écarte au hasard du trait, en travers et le
    /// long, jusqu'à cette part du diamètre.
    #[serde(rename = "dispersion")]
    pub scatter: f32,
    #[serde(rename = "dispersion_long")]
    pub scatter_along: f32,
    /// Empreintes posées à chaque pas (le « Count » de Procreate), chacune dispersée à part.
    #[serde(rename = "nombre")]
    pub count: u32,
    /// Pointe en image retournée au hasard, horizontalement et verticalement.
    #[serde(rename = "retourner_x")]
    pub flip_x: bool,
    #[serde(rename = "retourner_y")]
    pub flip_y: bool,
    /// Connexions (moteur de dessin, étape 2) : taille, opacité, angle et rondeur qui suivent la
    /// pression, la vitesse, la direction du trait, un fondu ou le hasard, par une courbe.
    #[serde(rename = "dynamique")]
    pub dynamics: Dynamics,
}

/// Comment le trait suit la pointe du stylet. « Suivi » : le lissage d'avant (le point posé
/// rattrape la pointe d'une part fixe à chaque mouvement). Les trois autres reprennent les noms
/// du « Line Smoothing » de TVPaint, leur calcul est celui de Pelure :
/// - corde tirée : le point posé ne bouge que lorsque la pointe s'en éloigne de plus que la
///   longueur de la corde, et il la suit alors à cette distance ;
/// - moyenne sur des points : la moyenne des N dernières positions de la pointe ;
/// - moyenne sur une distance : la moyenne des positions de la pointe sur ses derniers pixels.
///
/// À la fin du trait, le point posé rattrape toujours la pointe.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SmoothKind {
    #[default]
    #[serde(rename = "suivi")]
    Follow,
    #[serde(rename = "corde")]
    PulledString,
    #[serde(rename = "moyenne_points")]
    AveragePoints,
    #[serde(rename = "moyenne_distance")]
    AverageDistance,
}

impl SmoothKind {
    pub const ALL: [SmoothKind; 4] = [SmoothKind::Follow, SmoothKind::PulledString, SmoothKind::AveragePoints, SmoothKind::AverageDistance];

    pub fn label(self) -> &'static str {
        match self {
            SmoothKind::Follow => "Suivi",
            SmoothKind::PulledString => "Corde tirée",
            SmoothKind::AveragePoints => "Moyenne (points)",
            SmoothKind::AverageDistance => "Moyenne (distance)",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            SmoothKind::Follow => "Le trait rattrape la pointe d'une part à chaque mouvement.",
            SmoothKind::PulledString => "Le trait suit la pointe au bout d'une corde : les petits tremblements ne le bougent pas (encrage).",
            SmoothKind::AveragePoints => "Le trait passe par la moyenne des dernières positions de la pointe.",
            SmoothKind::AverageDistance => "Le trait passe par la moyenne des positions sur les derniers pixels parcourus.",
        }
    }
}

/// Ce qu'une connexion suit : les « connexions » de TVPaint (pression, vitesse, direction,
/// fondu, hasard… relevées dans ses fichiers) et la « dynamique » de Procreate. L'inclinaison
/// du stylet viendra quand Pelure saura la lire.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Source {
    #[default]
    #[serde(rename = "aucune")]
    None,
    #[serde(rename = "pression")]
    Pressure,
    #[serde(rename = "vitesse")]
    Speed,
    #[serde(rename = "direction")]
    Direction,
    #[serde(rename = "fondu")]
    Fade,
    #[serde(rename = "hasard")]
    Random,
}

impl Source {
    pub const ALL: [Source; 6] = [Source::None, Source::Pressure, Source::Speed, Source::Direction, Source::Fade, Source::Random];

    pub fn label(self) -> &'static str {
        match self {
            Source::None => "Aucune",
            Source::Pressure => "Pression",
            Source::Speed => "Vitesse",
            Source::Direction => "Direction",
            Source::Fade => "Fondu",
            Source::Random => "Hasard",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Source::None => "Le réglage ne bouge pas.",
            Source::Pressure => "L'appui du stylet.",
            Source::Speed => "La vitesse du trait (jusqu'à la vitesse max).",
            Source::Direction => "La direction du trait : un tour complet va de 0 à 1.",
            Source::Fade => "La longueur déjà tracée (jusqu'à la longueur du fondu).",
            Source::Random => "Une valeur au hasard à chaque empreinte.",
        }
    }
}

/// Courbe de réponse d'une connexion : de 2 à `Curve::MAX` points, x croissant de 0 à 1, y de 0
/// à 1, en ligne droite entre eux. Taille fixe : les réglages restent copiables tels quels.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(from = "Vec<[f32; 2]>", into = "Vec<[f32; 2]>")]
pub struct Curve {
    n: u8,
    pts: [[f32; 2]; Curve::MAX],
}

impl Curve {
    pub const MAX: usize = 8;

    /// La droite de (0, 0) à (1, 1) : la réponse suit l'entrée.
    pub fn linear() -> Self {
        Self::from_points(&[[0.0, 0.0], [1.0, 1.0]])
    }

    /// Une courbe faite de `pts`, remis en ordre : x et y bornés à 0..1, x croissant, le premier à
    /// x = 0 et le dernier à x = 1, au plus `MAX` points.
    pub fn from_points(pts: &[[f32; 2]]) -> Self {
        let mut v: Vec<[f32; 2]> = pts.iter().map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)]).filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
        v.sort_by(|a, b| a[0].total_cmp(&b[0]));
        v.truncate(Self::MAX);
        match v.len() {
            0 => v = vec![[0.0, 0.0], [1.0, 1.0]],
            1 => v = vec![[0.0, v[0][1]], [1.0, v[0][1]]],
            _ => {}
        }
        let last = v.len() - 1;
        v[0][0] = 0.0;
        v[last][0] = 1.0;
        let mut c = Curve { n: v.len() as u8, pts: [[0.0; 2]; Self::MAX] };
        c.pts[..v.len()].copy_from_slice(&v);
        c
    }

    pub fn points(&self) -> &[[f32; 2]] {
        &self.pts[..self.n as usize]
    }

    /// La réponse à l'entrée `x` (0 à 1).
    pub fn eval(&self, x: f32) -> f32 {
        let p = self.points();
        let x = x.clamp(0.0, 1.0);
        for w in p.windows(2) {
            let ([x0, y0], [x1, y1]) = (w[0], w[1]);
            if x <= x1 {
                let t = if x1 - x0 > 1e-6 { (x - x0) / (x1 - x0) } else { 1.0 };
                return y0 + (y1 - y0) * t;
            }
        }
        p.last().map_or(x, |q| q[1])
    }
}

impl Default for Curve {
    fn default() -> Self {
        Self::linear()
    }
}

impl From<Vec<[f32; 2]>> for Curve {
    fn from(v: Vec<[f32; 2]>) -> Self {
        Curve::from_points(&v)
    }
}

impl From<Curve> for Vec<[f32; 2]> {
    fn from(c: Curve) -> Self {
        c.points().to_vec()
    }
}

/// Un réglage relié à une source : sa réponse va de `min` (la courbe donne 0) à 1 (la courbe
/// donne 1), en part de la valeur réglée ; `invert` retourne l'entrée.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub source: Source,
    #[serde(rename = "inverser")]
    pub invert: bool,
    #[serde(rename = "courbe")]
    pub curve: Curve,
    pub min: f32,
}

impl Default for Connection {
    fn default() -> Self {
        Connection { source: Source::None, invert: false, curve: Curve::linear(), min: 0.0 }
    }
}

impl Connection {
    pub fn active(&self) -> bool {
        self.source != Source::None
    }

    /// Réponse (de `min` à 1) à l'entrée `x` (0 à 1).
    pub fn response(&self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let x = if self.invert { 1.0 - x } else { x };
        let min = self.min.clamp(0.0, 1.0);
        min + (1.0 - min) * self.curve.eval(x)
    }

    /// Réponse aux entrées d'une empreinte.
    pub fn at(&self, inp: &Inputs) -> f32 {
        self.response(inp.get(self.source))
    }
}

/// La dynamique d'une brosse : une connexion par réglage, et les échelles de la vitesse et du
/// fondu (TVPaint : « Limit » de la vitesse, longueur du fondu en pixels).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Dynamics {
    #[serde(rename = "taille")]
    pub size: Connection,
    #[serde(rename = "opacite")]
    pub opacity: Connection,
    pub angle: Connection,
    #[serde(rename = "rondeur")]
    pub roundness: Connection,
    /// Vitesse du trait (pixels du dessin par seconde) à laquelle l'entrée « vitesse » vaut 1.
    #[serde(rename = "vitesse_max")]
    pub speed_max: f32,
    /// Longueur tracée (pixels du dessin) au bout de laquelle l'entrée « fondu » vaut 1.
    #[serde(rename = "fondu_longueur")]
    pub fade_length: f32,
}

impl Default for Dynamics {
    fn default() -> Self {
        Dynamics {
            size: Connection::default(),
            opacity: Connection::default(),
            angle: Connection::default(),
            roundness: Connection::default(),
            speed_max: 2000.0,
            fade_length: 300.0,
        }
    }
}

impl Dynamics {
    pub fn uses(&self, s: Source) -> bool {
        [self.size, self.opacity, self.angle, self.roundness].iter().any(|c| c.source == s)
    }
}

/// Ce que mesure le trait à une empreinte : les entrées des connexions, de 0 à 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inputs {
    pub pressure: f32,
    pub speed: f32,
    pub direction: f32,
    pub fade: f32,
    pub random: f32,
}

impl Inputs {
    /// Seulement l'appui (les autres entrées au repos).
    pub fn pressure(p: f32) -> Self {
        Inputs { pressure: p, random: 0.5, ..Default::default() }
    }

    pub fn get(&self, s: Source) -> f32 {
        match s {
            Source::None => 1.0,
            Source::Pressure => self.pressure,
            Source::Speed => self.speed,
            Source::Direction => self.direction,
            Source::Fade => self.fade,
            Source::Random => self.random,
        }
        .clamp(0.0, 1.0)
    }
}

impl Default for BrushSettings {
    fn default() -> Self {
        Self::brush()
    }
}

impl BrushSettings {
    pub fn brush() -> Self {
        Self {
            size: 6.0,
            opacity: 1.0,
            power: 1.0,
            build_up: false,
            hardness: 0.85,
            profile: None,
            spacing: 0.08,
            smoothing: 0.3,
            smooth_kind: SmoothKind::Follow,
            smooth_px: 8.0,
            smooth_points: 6,
            pressure_size: true,
            pressure_opacity: false,
            aliased: false,
            min_size: 0.0,
            min_opacity: 0.0,
            grain: 0.0,
            paper: None,
            tip: None,
            roundness: 1.0,
            angle: 0.0,
            angle_jitter: 0.0,
            size_jitter: 0.0,
            scatter: 0.0,
            scatter_along: 0.0,
            count: 1,
            flip_x: false,
            flip_y: false,
            dynamics: Dynamics::default(),
        }
    }

    /// La connexion de la taille : celle de la dynamique si elle est choisie, sinon « taille
    /// selon la pression » (droite de la taille mini à la taille).
    pub fn size_connection(&self) -> Option<Connection> {
        if self.dynamics.size.active() {
            Some(self.dynamics.size)
        } else {
            self.pressure_size.then(|| Connection { source: Source::Pressure, min: self.min_size, ..Default::default() })
        }
    }

    /// La connexion de l'opacité, comme celle de la taille.
    pub fn opacity_connection(&self) -> Option<Connection> {
        if self.dynamics.opacity.active() {
            Some(self.dynamics.opacity)
        } else {
            self.pressure_opacity.then(|| Connection { source: Source::Pressure, min: self.min_opacity, ..Default::default() })
        }
    }

    /// Part de la taille à une empreinte (jamais moins de 3 %).
    pub fn size_factor(&self, inp: &Inputs) -> f32 {
        self.size_connection().map_or(1.0, |c| c.at(inp).max(0.03))
    }

    /// Part de l'opacité à une empreinte.
    pub fn flow(&self, inp: &Inputs) -> f32 {
        self.opacity_connection().map_or(1.0, |c| c.at(inp))
    }

    /// Angle de la pointe à une empreinte, en degrés, sans le hasard : l'angle réglé, plus un
    /// tour complet fois la réponse de sa connexion (la direction, telle quelle, fait suivre le
    /// trait à la pointe).
    pub fn angle_at(&self, inp: &Inputs) -> f32 {
        let c = self.dynamics.angle;
        self.angle + if c.active() { 360.0 * c.at(inp) } else { 0.0 }
    }

    /// Le profil de pointe qui reproduit la dureté `hardness` (plein jusqu'à `hardness` du rayon,
    /// puis une chute douce) : le point de départ quand on choisit d'éditer le profil.
    pub fn profile_from_hardness(hardness: f32) -> Curve {
        let h = hardness.clamp(0.0, 0.98);
        let mut pts = vec![[0.0, 1.0]];
        for k in 0..=5 {
            let x = h + (1.0 - h) * k as f32 / 5.0;
            let t = (x - h) / (1.0 - h);
            pts.push([x, 1.0 - t * t * (3.0 - 2.0 * t)]);
        }
        Curve::from_points(&pts)
    }

    /// Rondeur de la pointe à une empreinte.
    pub fn roundness_at(&self, inp: &Inputs) -> f32 {
        let c = self.dynamics.roundness;
        self.roundness * if c.active() { c.at(inp) } else { 1.0 }
    }

    pub fn pencil() -> Self {
        Self {
            size: 2.0,
            hardness: 1.0,
            spacing: 0.1,
            smoothing: 0.0,
            aliased: true,
            ..Self::brush()
        }
    }

    pub fn eraser() -> Self {
        Self {
            size: 24.0,
            hardness: 0.7,
            smoothing: 0.2,
            ..Self::brush()
        }
    }
}

/// Aperçu d'un réglage (grille des pinceaux) : un trait en vague tracé par le vrai moteur dans
/// une image `w`×`h`, effleuré et lent aux bouts, appuyé et rapide au milieu (une connexion à
/// la vitesse se voit aussi). Renvoie la couverture (0 à 255) de chaque pixel, ligne par ligne.
/// La taille est ramenée à ce qui tient dans l'image ; une gomme est montrée comme le trait
/// qu'elle enlève.
pub fn stroke_preview(settings: &BrushSettings, textures: BrushTextures, w: usize, h: usize) -> Vec<u8> {
    let mut s = *settings;
    s.size = s.size.clamp(1.0, h as f32 * 0.5);
    s.smoothing = 0.0;
    s.smooth_kind = SmoothKind::Follow;
    let mut raster = Raster::new(w, h);
    let mut stroke = StrokeState::new(0, StrokeMode::Paint([0, 0, 0]), s, None, true).with_textures(textures);
    let mut dirty = Vec::new();
    let margin = s.size * 0.5 + 2.0;
    let swing = (h as f32 * 0.5 - margin).max(0.0) * 0.7;
    let n = 64;
    let mut time = 0.0f64;
    let mut prev: Option<Pos2> = None;
    for k in 0..=n {
        let t = k as f32 / n as f32;
        let bell = (t * std::f32::consts::PI).sin();
        let p = Pos2::new(
            margin + t * (w as f32 - 2.0 * margin),
            h as f32 * 0.5 - (t * std::f32::consts::TAU).sin() * swing,
        );
        // De 10 % à 100 % de la vitesse max de la brosse, au plus vite au milieu.
        let v = s.dynamics.speed_max.max(1.0) * (0.1 + 0.9 * bell);
        time += prev.map_or(0.0, |q| (q.distance(p) / v) as f64);
        prev = Some(p);
        stroke.add_point_at(&mut raster, p, bell.max(0.15), time, &mut dirty);
    }
    stroke.finish(&mut raster, &mut dirty);
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| raster.get(x, y).a()).collect()
}

#[derive(Clone, Copy)]
pub enum StrokeMode {
    Paint([u8; 3]),
    Erase,
}

/// Un trait en cours.
///
/// Chaque empreinte écrit dans un masque propre au trait (on garde le maximum), puis le pixel
/// est recalculé depuis son état d'avant le trait. Le chevauchement des empreintes ne
/// fonce donc pas le trait : une opacité de 50 % donne un trait uniforme à 50 %.
pub struct StrokeState {
    pub cel: CelId,
    /// Le trait vient d'un stylet (pression lue sur la tablette) plutôt que de la souris.
    pub pen: bool,
    /// Ligne médiane lissée et rayon en chaque point : ce que garde un trait vectoriel.
    pub path: Vec<VPoint>,
    mode: StrokeMode,
    settings: BrushSettings,
    before: HashMap<usize, Option<Tile>>,
    mask: HashMap<usize, Box<[f32; TILE_PIXELS]>>,
    prefix: Option<Edit>,
    last: Option<(Pos2, f32)>,
    smoothed: Option<Pos2>,
    raw: Option<(Pos2, f32)>,
    since_dab: f32,
    /// Pointe et papier en image (aucun : pointe ronde, papier lisse).
    textures: BrushTextures,
    /// Hasard (angle, taille des empreintes), le même à chaque fois pour un même trait.
    rng: u32,
    /// Préserver la transparence (13b) : le pinceau ne colore que ce qui est déjà dessiné.
    keep_alpha: bool,
    /// Pochoirs (13b) : où le trait peut peindre ; aucun : partout.
    stencil: Option<crate::stencil::Stencil>,
    /// Vitesse : instant de la dernière mesure, distance faite depuis (plusieurs points peuvent
    /// arriver au même instant), vitesse lissée en pixels par seconde.
    clock: Option<f64>,
    since_clock: f32,
    speed: f32,
    /// Longueur déjà tracée (pour le fondu) et direction du dernier segment (0 à 1, un tour).
    travel: f32,
    direction: f32,
    /// Dernières positions de la pointe (lissages par moyenne).
    history: std::collections::VecDeque<Pos2>,
}

impl StrokeState {
    pub fn new(
        cel: CelId,
        mode: StrokeMode,
        settings: BrushSettings,
        prefix: Option<Edit>,
        pen: bool,
    ) -> Self {
        Self {
            cel,
            pen,
            path: Vec::new(),
            mode,
            settings,
            before: HashMap::new(),
            mask: HashMap::new(),
            prefix,
            last: None,
            smoothed: None,
            raw: None,
            since_dab: 0.0,
            textures: BrushTextures::default(),
            rng: 0x9E37_79B9,
            keep_alpha: false,
            stencil: None,
            clock: None,
            since_clock: 0.0,
            speed: 0.0,
            travel: 0.0,
            direction: 0.0,
            history: std::collections::VecDeque::new(),
        }
    }

    /// Le point posé pour une nouvelle position `pos` de la pointe, selon le type de lissage.
    fn smooth(&mut self, pos: Pos2) -> Pos2 {
        let s = self.settings;
        let Some(prev) = self.smoothed else {
            self.history.push_back(pos);
            return pos;
        };
        match s.smooth_kind {
            SmoothKind::Follow => prev + (pos - prev) * (1.0 - s.smoothing.clamp(0.0, 0.95)),
            SmoothKind::PulledString => {
                let len = s.smooth_px.max(0.0);
                let d = pos - prev;
                let dist = d.length();
                if dist > len {
                    prev + d * ((dist - len) / dist)
                } else {
                    prev
                }
            }
            SmoothKind::AveragePoints => {
                self.history.push_back(pos);
                let n = (s.smooth_points.max(1)) as usize;
                while self.history.len() > n {
                    self.history.pop_front();
                }
                mean(self.history.iter())
            }
            SmoothKind::AverageDistance => {
                self.history.push_back(pos);
                // On garde les positions des derniers `smooth_px` pixels parcourus.
                let span = s.smooth_px.max(0.0);
                let mut keep = 1;
                let mut run = 0.0;
                let pts: Vec<Pos2> = self.history.iter().rev().copied().collect();
                for w in pts.windows(2) {
                    run += w[0].distance(w[1]);
                    if run > span {
                        break;
                    }
                    keep += 1;
                }
                while self.history.len() > keep {
                    self.history.pop_front();
                }
                mean(self.history.iter())
            }
        }
    }

    /// Les entrées des connexions pour l'appui `pressure`, à `ahead` pixels après le dernier point
    /// posé (le hasard au repos : il est tiré à l'empreinte).
    fn inputs(&self, pressure: f32, ahead: f32) -> Inputs {
        let d = &self.settings.dynamics;
        Inputs {
            pressure,
            speed: self.speed / d.speed_max.max(1.0),
            direction: self.direction,
            fade: (self.travel + ahead) / d.fade_length.max(1.0),
            random: 0.5,
        }
    }

    /// Réglages du calque peint (13b) : préserver la transparence, pochoirs des autres calques.
    pub fn on_layer(mut self, keep_alpha: bool, stencil: Option<crate::stencil::Stencil>) -> Self {
        self.keep_alpha = keep_alpha;
        self.stencil = stencil;
        self
    }

    /// Bout du trait : le dernier point posé, la pointe réelle du stylet et son rayon. Le
    /// lissage (et l'espacement des empreintes) laissent un petit écart entre les deux, que
    /// l'affichage comble pour que le trait reste attaché à la pointe.
    pub fn tip(&self) -> Option<(Pos2, Pos2, f32)> {
        let (last, _) = self.last?;
        let (raw, pressure) = self.raw?;
        Some((last, raw, self.settings.size * 0.5 * self.settings.size_factor(&self.inputs(pressure, 0.0))))
    }

    /// Couleur et opacité du trait à la pointe (aucune pour la gomme).
    pub fn tip_paint(&self) -> Option<([u8; 3], f32)> {
        let StrokeMode::Paint(rgb) = self.mode else { return None };
        let s = self.settings;
        let pressure = self.raw.map_or(1.0, |r| r.1);
        let flow = s.flow(&self.inputs(pressure, 0.0));
        Some((rgb, (s.opacity * flow * s.power).clamp(0.0, 1.0)))
    }

    /// Pointe et papier du trait (pinceaux texturés).
    pub fn with_textures(mut self, textures: BrushTextures) -> Self {
        self.textures = textures;
        self
    }

    /// Nombre au hasard entre -0,5 et 0,5.
    fn jitter(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f32 / u32::MAX as f32) - 0.5
    }

    /// Comme `add_point`, avec l'instant (en secondes) où le stylet y était : la vitesse du trait
    /// en vient. Plusieurs points au même instant (une image d'écran) comptent ensemble.
    pub fn add_point_at(&mut self, raster: &mut Raster, pos: Pos2, pressure: f32, time: f64, dirty: &mut Vec<usize>) {
        if let Some((prev, _)) = self.raw {
            self.since_clock += prev.distance(pos);
        }
        match self.clock {
            Some(t0) if time > t0 => {
                let v = self.since_clock / (time - t0) as f32;
                // Lissée : un à-coup du système ne fait pas sauter le trait.
                self.speed = if self.speed > 0.0 { self.speed * 0.5 + v * 0.5 } else { v };
                self.clock = Some(time);
                self.since_clock = 0.0;
            }
            Some(_) => {}
            None => self.clock = Some(time),
        }
        self.add_point(raster, pos, pressure, dirty);
    }

    pub fn add_point(&mut self, raster: &mut Raster, pos: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        self.raw = Some((pos, pressure));
        let target = self.smooth(pos);
        self.smoothed = Some(target);
        self.record(target, pressure);
        self.move_to(raster, target, pressure, dirty);
    }

    fn record(&mut self, p: Pos2, pressure: f32) {
        let r = self.settings.size * 0.5 * self.settings.size_factor(&self.inputs(pressure, 0.0));
        self.path.push(VPoint { x: p.x, y: p.y, r });
    }

    /// Termine le trait (en rattrapant le retard du lissage) et renvoie de quoi l'annuler.
    pub fn finish(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) -> Option<Edit> {
        if let (Some((p, pr)), Some(s)) = (self.raw, self.smoothed) {
            if p != s {
                self.move_to(raster, p, pr, dirty);
                self.record(p, pr);
            }
        }
        let tiles: Vec<_> = std::mem::take(&mut self.before).into_iter().collect();
        let pixels = (!tiles.is_empty()).then_some(Edit::Tiles {
            cel: self.cel,
            tiles,
        });
        match (self.prefix.take(), pixels) {
            (Some(pre), Some(px)) => Some(Edit::Group(vec![pre, px])),
            (Some(pre), None) => Some(pre),
            (None, px) => px,
        }
    }

    fn step(&self, inp: &Inputs) -> f32 {
        let diameter = self.settings.size * self.settings.size_factor(inp);
        (diameter * self.settings.spacing).max(0.3)
    }

    /// Pose des empreintes régulièrement espacées du dernier point jusqu'à `to`.
    fn move_to(&mut self, raster: &mut Raster, to: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        let Some((from, p0)) = self.last else {
            let inp = self.inputs(pressure, 0.0);
            self.dab(raster, to, inp, dirty);
            self.last = Some((to, pressure));
            self.since_dab = 0.0;
            return;
        };
        let dist = from.distance(to);
        if dist < 1e-3 {
            self.last = Some((from, pressure));
            return;
        }
        let d = to - from;
        self.direction = (d.y.atan2(d.x) / std::f32::consts::TAU).rem_euclid(1.0);
        let mut travelled = 0.0;
        loop {
            let pr = p0 + (pressure - p0) * (travelled / dist);
            let need = (self.step(&self.inputs(pr, travelled)) - self.since_dab).max(0.0);
            if travelled + need > dist {
                self.since_dab += dist - travelled;
                break;
            }
            travelled += need;
            let t = travelled / dist;
            let inp = self.inputs(p0 + (pressure - p0) * t, travelled);
            self.scattered_dabs(raster, from + (to - from) * t, d / dist, inp, dirty);
            self.since_dab = 0.0;
        }
        self.travel += dist;
        self.last = Some((to, pressure));
    }

    /// Les empreintes d'un pas en `c` (trait dans la direction unitaire `dir`) : `count`
    /// empreintes, chacune écartée au hasard en travers et le long du trait (rien n'est tiré si la
    /// brosse ne disperse pas : le même trait qu'avant).
    fn scattered_dabs(&mut self, raster: &mut Raster, c: Pos2, dir: egui::Vec2, inp: Inputs, dirty: &mut Vec<usize>) {
        let s = self.settings;
        let n = s.count.clamp(1, 16);
        if n == 1 && s.scatter <= 0.0 && s.scatter_along <= 0.0 {
            self.dab(raster, c, inp, dirty);
            return;
        }
        let diameter = s.size * s.size_factor(&inp);
        let across = egui::vec2(-dir.y, dir.x);
        for _ in 0..n {
            let mut p = c;
            if s.scatter > 0.0 {
                p += across * (s.scatter * diameter * 2.0 * self.jitter());
            }
            if s.scatter_along > 0.0 {
                p += dir * (s.scatter_along * diameter * 2.0 * self.jitter());
            }
            self.dab(raster, p, inp, dirty);
        }
    }

    fn dab(&mut self, raster: &mut Raster, c: Pos2, mut inp: Inputs, dirty: &mut Vec<usize>) {
        let s = self.settings;
        let mode = self.mode;
        if s.dynamics.uses(Source::Random) {
            inp.random = self.jitter() + 0.5;
        }
        let pressure = inp.pressure;
        let mut radius = s.size * 0.5 * s.size_factor(&inp);
        if s.size_jitter > 0.0 {
            radius *= (1.0 + s.size_jitter * 2.0 * self.jitter()).max(0.05);
        }
        let mut flow = s.flow(&inp);
        if s.aliased {
            radius = radius.max(0.71);
        } else if radius < 0.75 {
            // Plus fin qu'un pixel : on éclaircit au lieu d'amincir.
            flow *= radius / 0.75;
            radius = 0.75;
        }
        if flow <= 0.0 {
            return;
        }
        // En accumulant, une empreinte dépose une part du débit : les empreintes qui se
        // recouvrent au même point (environ diamètre ÷ pas) en déposent ensemble le débit, et
        // chaque nouveau passage fonce. (Choix de Pelure : la formule de TVPaint n'est pas
        // publiée.)
        let deposit = if s.build_up {
            let diameter = radius * 2.0;
            let n = (diameter / (diameter * s.spacing).max(0.3)).max(1.0);
            1.0 - (1.0 - s.power.clamp(0.0, 1.0)).powf(1.0 / n)
        } else {
            s.power.clamp(0.0, 1.0)
        };
        let inner = (radius * s.hardness.clamp(0.0, 1.0)).min(radius - 0.5).max(0.0);
        // Pointe aplatie ou tournée : on mesure chaque pixel dans le repère de la pointe.
        let angle = (s.angle_at(&inp) + s.angle_jitter * self.jitter()).to_radians();
        let (sin, cos) = angle.sin_cos();
        let squash = s.roundness_at(&inp).clamp(0.05, 1.0);
        let shaped = squash < 1.0 || self.textures.tip.is_some();
        let tip = self.textures.tip.clone();
        let tip = tip.as_ref().map(|t| t.level(radius * 2.0));
        // Retournements au hasard d'une pointe en image (tirés seulement s'ils sont demandés).
        let flip_u = tip.is_some() && s.flip_x && self.jitter() < 0.0;
        let flip_v = tip.is_some() && s.flip_y && self.jitter() < 0.0;
        let paper = self.textures.paper.clone().filter(|_| s.grain > 0.0);
        let profile = s.profile.map(|c| profile_lut(&c));
        // Une pointe en image tournée déborde du cercle : on couvre le carré qui la contient.
        let reach = if tip.is_some() { radius * std::f32::consts::SQRT_2 } else { radius };

        let x0 = (c.x - reach - 1.0).floor().max(0.0) as usize;
        let y0 = (c.y - reach - 1.0).floor().max(0.0) as usize;
        let x1 = (c.x + reach + 1.0).ceil().clamp(0.0, raster.width as f32) as usize;
        let y1 = (c.y + reach + 1.0).ceil().clamp(0.0, raster.height as f32) as usize;
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        // Couverture d'un pixel par l'empreinte, à (dx, dy) de son centre.
        let footprint = |dx: f32, dy: f32| -> f32 {
            let (lx, ly) = if shaped {
                (dx * cos + dy * sin, (-dx * sin + dy * cos) / squash)
            } else {
                (dx, dy)
            };
            match tip {
                Some(img) => {
                    let (lx, ly) = (if flip_u { -lx } else { lx }, if flip_v { -ly } else { ly });
                    let u = (lx / (2.0 * radius) + 0.5) * img.width as f32;
                    let v = (ly / (2.0 * radius) + 0.5) * img.height as f32;
                    img.sample(u, v)
                }
                None => match &profile {
                    Some(lut) => profile_coverage((lx * lx + ly * ly).sqrt(), radius, lut, s.aliased),
                    None => coverage((lx * lx + ly * ly).sqrt(), radius, inner, s.aliased),
                },
            }
        };

        // Préserver la transparence : on ne peint que sur ce qui est déjà dessiné (13b).
        let keep_alpha = self.keep_alpha && matches!(mode, StrokeMode::Paint(_));
        let stencil = self.stencil.as_ref();
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let idx = ty * raster.tiles_x + tx;
                if (matches!(mode, StrokeMode::Erase) || keep_alpha) && raster.tiles[idx].is_none() {
                    continue;
                }
                if stencil.is_some_and(|st| st.blocks(idx)) {
                    continue;
                }
                let (ox, oy) = (tx * TILE, ty * TILE);
                if !self.before.contains_key(&idx) {
                    self.before.insert(idx, raster.tiles[idx].clone());
                }
                let before = self.before[&idx].as_deref();
                let mask = self
                    .mask
                    .entry(idx)
                    .or_insert_with(|| Box::new([0.0; TILE_PIXELS]));
                let tile = raster.tile_mut(idx);
                let mut changed = false;
                for py in y0.max(oy)..y1.min(oy + TILE) {
                    let dy = py as f32 + 0.5 - c.y;
                    for px in x0.max(ox)..x1.min(ox + TILE) {
                        let dx = px as f32 + 0.5 - c.x;
                        let cov = footprint(dx, dy);
                        if cov <= 0.0 {
                            continue;
                        }
                        // Grain du papier, fixe sur le dessin.
                        let grain = paper.as_ref().map_or(1.0, |p| grain_factor(p.tiled(px, py), pressure, s.grain));
                        let i = (py - oy) * TILE + (px - ox);
                        let through = stencil.map_or(1.0, |st| st.at(idx, i));
                        let v = cov * flow * grain * through * deposit;
                        let m = if s.build_up {
                            // Le débit s'ajoute à ce qui est déjà déposé dans le trait.
                            mask[i] + (1.0 - mask[i]) * v
                        } else {
                            v
                        };
                        // (En accumulant, un ajout infime ne repeint pas le pixel.)
                        if m <= mask[i] + if s.build_up { 1e-5 } else { 0.0 } {
                            continue;
                        }
                        mask[i] = m;
                        let b = before.map_or(Color32::TRANSPARENT, |t| t[i]);
                        tile[i] = match mode {
                            StrokeMode::Paint(rgb) if keep_alpha => paint_over(b, rgb, m * s.opacity),
                            StrokeMode::Paint(rgb) => paint(b, rgb, m * s.opacity),
                            StrokeMode::Erase => erase(b, m * s.opacity),
                        };
                        changed = true;
                    }
                }
                if changed {
                    dirty.push(idx);
                }
            }
        }
    }
}

/// Moyenne de positions (au moins une).
fn mean<'a>(pts: impl Iterator<Item = &'a Pos2>) -> Pos2 {
    let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
    for p in pts {
        x += p.x;
        y += p.y;
        n += 1.0;
    }
    if n == 0.0 {
        Pos2::ZERO
    } else {
        Pos2::new(x / n, y / n)
    }
}

/// Points de la table d'un profil (du centre au bord).
const PROFILE_STEPS: usize = 64;

/// Le profil lu d'avance en table (une empreinte en lit beaucoup de pixels).
pub(crate) fn profile_lut(c: &Curve) -> [f32; PROFILE_STEPS + 1] {
    std::array::from_fn(|i| c.eval(i as f32 / PROFILE_STEPS as f32).clamp(0.0, 1.0))
}

/// Couverture d'un pixel à la distance `d` du centre d'une empreinte de rayon `radius`, d'après un
/// profil : la table entre ses points, et le bord anticrénelé sur un pixel (franc en pixels nets).
pub(crate) fn profile_coverage(d: f32, radius: f32, lut: &[f32; PROFILE_STEPS + 1], aliased: bool) -> f32 {
    if d > radius + 0.5 {
        return 0.0;
    }
    let x = (d / radius.max(1e-3)).clamp(0.0, 1.0) * PROFILE_STEPS as f32;
    let i = (x as usize).min(PROFILE_STEPS - 1);
    let v = lut[i] + (lut[i + 1] - lut[i]) * (x - i as f32);
    if aliased {
        return if d <= radius && v >= 0.5 { 1.0 } else { 0.0 };
    }
    v.min((radius - d + 0.5).clamp(0.0, 1.0))
}

pub(crate) fn coverage(d: f32, radius: f32, inner: f32, aliased: bool) -> f32 {
    if aliased {
        return if d <= radius { 1.0 } else { 0.0 };
    }
    let edge = (radius - d + 0.5).clamp(0.0, 1.0);
    if edge <= 0.0 || d <= inner {
        return edge;
    }
    let t = ((d - inner) / (radius - inner).max(1e-3)).clamp(0.0, 1.0);
    edge.min(1.0 - t * t * (3.0 - 2.0 * t))
}

pub(crate) fn paint(b: Color32, rgb: [u8; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ia = 1.0 - a;
    let ch = |c: u8, d: u8| (c as f32 * a + d as f32 * ia + 0.5) as u8;
    Color32::from_rgba_premultiplied(
        ch(rgb[0], b.r()),
        ch(rgb[1], b.g()),
        ch(rgb[2], b.b()),
        (255.0 * a + b.a() as f32 * ia + 0.5) as u8,
    )
}

/// Comme `paint`, mais l'opacité du pixel ne change pas (préserver la transparence, 13b) : seule
/// sa couleur va vers `rgb`. Un pixel vide reste vide.
pub(crate) fn paint_over(b: Color32, rgb: [u8; 3], a: f32) -> Color32 {
    let a = a.clamp(0.0, 1.0);
    let ia = 1.0 - a;
    let k = b.a() as f32 / 255.0;
    let ch = |c: u8, d: u8| ((c as f32 * k * a + d as f32 * ia + 0.5) as u8).min(b.a());
    Color32::from_rgba_premultiplied(ch(rgb[0], b.r()), ch(rgb[1], b.g()), ch(rgb[2], b.b()), b.a())
}

fn erase(b: Color32, a: f32) -> Color32 {
    let k = 1.0 - a.clamp(0.0, 1.0);
    let ch = |d: u8| (d as f32 * k + 0.5) as u8;
    Color32::from_rgba_premultiplied(ch(b.r()), ch(b.g()), ch(b.b()), ch(b.a()))
}
