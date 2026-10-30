use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2};
use serde::{Deserialize, Serialize};

use crate::brushtex::{grain_factor, BrushTextures};
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::{Raster, Tile, TILE, TILE_PIXELS, WORLD};
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
    /// Peinture humide (moteur de dessin, étape 11 ; modèle de Pelure, à comparer avec TVPaint) :
    /// la part de la couleur déjà posée que la brosse prend à chaque empreinte et entraîne
    /// (0 : aucune, le trait d'avant).
    #[serde(rename = "melange")]
    pub mix: f32,
    /// La peinture chargée dans la brosse : 1, elle ne s'épuise pas ; moins, elle s'épuise le long
    /// du trait, et la brosse vide ne fait plus qu'étaler.
    pub charge: f32,
    /// Comment la couleur se mélange (« La couleur se mélange », 2026-10-28) : pigment (gouache,
    /// acrylique : `mix` et `charge`), eau (aquarelle : `water`) ou doigt (pas de couleur posée, seulement
    /// l'entraînement : `smudge`). Sans effet tant que le mélange est éteint (`mixes`).
    #[serde(rename = "melange_type", default, skip_serializing_if = "MixKind::is_default")]
    pub mix_kind: MixKind,
    /// L'eau de l'aquarelle (0 : peu d'eau, couleur franche ; 1 : très diluée, transparente, qui coule dans
    /// les couleurs mouillées).
    #[serde(rename = "eau", default = "default_water", skip_serializing_if = "is_default_water")]
    pub water: f32,
    /// L'entraînement du doigt (`MixKind::Smudge`, outil Mélangeur) : haut, la brosse emporte longtemps ce
    /// qu'elle a pris.
    #[serde(rename = "entrainement", default = "default_smudge", skip_serializing_if = "is_default_smudge")]
    pub smudge: f32,
    /// Le mélange est coupé : la case « La couleur se mélange » est décochée. Les valeurs (`mix`, `charge`,
    /// `water`, `smudge`, type) sont gardées pour la rallumer telle qu'elle était.
    #[serde(rename = "melange_coupe", default, skip_serializing_if = "is_false")]
    pub mix_off: bool,
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
    /// Lissage selon le temps (sensation des brosses, §2.3) : le « Suivi » rattrape la pointe d'une
    /// part qui dépend du temps écoulé depuis le point d'avant (constante de temps, comme le
    /// `slow_tracking` de libmypaint), non du nombre d'échantillons : la même douceur à 60 ou 250
    /// échantillons par seconde. Sans instants (rejeu, Studio) : le calcul d'avant.
    #[serde(rename = "suivi_temps")]
    pub smooth_time: bool,
    /// Corde et distance mesurées en pixels de l'écran (le « Scalable Distance » de Krita) : la
    /// même corde sous le stylet quel que soit le zoom. Faux : en pixels du dessin, comme avant.
    #[serde(rename = "lissage_ecran")]
    pub smooth_screen: bool,
    /// Lissage après le trait (TVPaint : Line Smoothing avec Real Time décoché, le réglage
    /// d'Anthony) : pendant le geste, le trait brut suit la pointe sans retard ; au levé, il est
    /// rejoué lissé depuis les points reçus.
    #[serde(rename = "lissage_differe")]
    pub smooth_deferred: bool,
    /// Comment le trait rattrape la pointe au levé (le lissage le laisse en retard).
    #[serde(rename = "rattrapage")]
    pub catch_up: CatchUp,
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
    /// Force du grain du papier (0 = papier lisse ; la « Hardness » du papier de TVPaint).
    pub grain: f32,
    /// Papier (moteur de dessin, étape 7) : sa taille (1 = un texel par pixel), son angle en
    /// degrés, et son relief inversé (les creux deviennent des sommets).
    #[serde(rename = "papier_taille")]
    pub paper_scale: f32,
    #[serde(rename = "papier_angle")]
    pub paper_angle: f32,
    #[serde(rename = "papier_inverser")]
    pub paper_invert: bool,
    /// Douceur du grain (rendu des brosses, étape 2) : largeur de la transition entre un creux
    /// vide et un sommet plein, en part du relief. 0,12 : le seuil presque net d'avant l'audit (la
    /// valeur des anciens réglages) ; 0,3 : un grain qui répond en douceur, comme un vrai papier.
    #[serde(rename = "papier_douceur")]
    pub paper_softness: f32,
    /// Le papier et le grain du crayon suivent le trait (lus depuis son premier point) au lieu de
    /// rester collés à la feuille (TVPaint, le défaut : repasser retrouve les mêmes creux).
    #[serde(rename = "papier_suit_trait")]
    pub paper_follow: bool,
    /// Grain propre du crayon (rendu, étape 3 ; le « Grain » Silk / Soft / Medium / Hard du crayon
    /// de TVPaint) : un bruit fin collé à la feuille, même sans papier.
    #[serde(rename = "grain_crayon")]
    pub pencil_grain: PencilGrain,
    /// Papier et pointe en image (numéros dans `BrushLibrary`) ; pas de pointe : pointe ronde.
    #[serde(rename = "papier")]
    pub paper: Option<u32>,
    #[serde(rename = "pointe")]
    pub tip: Option<u32>,
    /// Pointe en plusieurs cases (moteur de dessin, étape 8 ; les brosses animées de TVPaint, les
    /// `.gih` de GIMP) : l'image est faite de `tip_cells` dessins côte à côte, et chaque empreinte
    /// en prend un, dans l'ordre `tip_order`.
    #[serde(rename = "pointe_cases")]
    pub tip_cells: u32,
    #[serde(rename = "pointe_ordre")]
    pub tip_order: TipOrder,
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

fn default_water() -> f32 {
    0.5
}

fn is_default_water(w: &f32) -> bool {
    (*w - 0.5).abs() < 1e-6
}

fn default_smudge() -> f32 {
    0.6
}

fn is_default_smudge(w: &f32) -> bool {
    (*w - 0.6).abs() < 1e-6
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Le type de mélange de la couleur (case « La couleur se mélange » des réglages de la brosse, sur un
/// calque bitmap) : de quoi retrouver la gouache, l'acrylique, l'aquarelle ou le doigt.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum MixKind {
    /// Gouache, acrylique : la brosse se charge de la couleur qu'elle traverse (mélange soustractif, bleu
    /// plus jaune donnent un vert) et se vide le long du trait (`mix`, `charge`).
    #[default]
    #[serde(rename = "pigment")]
    Pigment,
    /// Aquarelle : la couleur est diluée et transparente, elle se mêle aux couleurs mouillées dessous, avec
    /// un léger bord plus foncé (`water`).
    #[serde(rename = "eau")]
    Water,
    /// Le doigt : aucune couleur posée, la brosse entraîne et étale ce qu'elle traverse (`smudge`).
    #[serde(rename = "doigt")]
    Smudge,
}

impl MixKind {
    pub const ALL: [MixKind; 3] = [MixKind::Pigment, MixKind::Water, MixKind::Smudge];

    pub fn is_default(&self) -> bool {
        *self == MixKind::Pigment
    }

    pub fn label(self) -> &'static str {
        match self {
            MixKind::Pigment => "Pigment",
            MixKind::Water => "Eau",
            MixKind::Smudge => "Lisser",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            MixKind::Pigment => "Gouache, acrylique : la brosse se charge de la couleur qu'elle traverse et la mêle à la sienne comme de la vraie peinture (bleu plus jaune donnent un vert) ; elle se vide le long du trait.",
            MixKind::Water => "Aquarelle : la couleur est diluée et transparente, elle se mêle aux couleurs mouillées dessous et laisse un léger bord plus foncé.",
            MixKind::Smudge => "Le doigt : aucune couleur n'est posée, la brosse entraîne et étale ce qui est déjà peint.",
        }
    }
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

/// Au levé, comment le trait lissé (en retard sur la pointe) la rattrape : pas du tout, tout droit
/// (comme avant : un petit crochet quand le geste tournait), ou en prolongeant sa courbe (une
/// Bézier qui part dans la dernière direction du trait et finit à la pointe). Avec le Suivi et la
/// corde tirée, dont chaque pas vise déjà la pointe, la courbe se confond avec le segment droit ;
/// la différence se voit avec les moyennes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum CatchUp {
    #[serde(rename = "aucun")]
    None,
    #[serde(rename = "droit")]
    Straight,
    #[default]
    #[serde(rename = "courbe")]
    Curve,
}

impl CatchUp {
    pub const ALL: [CatchUp; 3] = [CatchUp::None, CatchUp::Straight, CatchUp::Curve];

    pub fn label(self) -> &'static str {
        match self {
            CatchUp::None => "Sans rattrapage",
            CatchUp::Straight => "Rattrapage droit",
            CatchUp::Curve => "Rattrapage en courbe",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            CatchUp::None => "Au levé, le trait s'arrête où le lissage l'a mené, en retrait de la pointe.",
            CatchUp::Straight => "Au levé, un segment droit jusqu'à la pointe (comme avant).",
            CatchUp::Curve => "Au levé, le trait prolonge sa courbe jusqu'à la pointe.",
        }
    }
}

/// Cadence de référence du « Suivi » selon le temps : à 200 échantillons par seconde (un stylet
/// Windows Ink courant), la part rattrapée par échantillon est celle d'avant ; à cadence plus
/// basse (grosse brosse, image lente) on rattrape plus, à cadence plus haute moins. À vérifier à la
/// tablette (la cadence réelle d'Anthony se lit dans le banc de traits).
pub const FOLLOW_REF_HZ: f32 = 200.0;

/// Part de l'écart à la pointe rattrapée par le « Suivi » pour un lissage `smoothing` (0 à 0,95) :
/// `1 − lissage` par échantillon, ou, avec le temps `dt` écoulé depuis le point d'avant,
/// `1 − lissage^(dt × cadence de référence)` (constante de temps). Sans `dt`, ou nul : par
/// échantillon. Après une pause, on rattrape tout.
pub fn follow_alpha(smoothing: f32, dt: Option<f32>) -> f32 {
    let s = smoothing.clamp(0.0, 0.95);
    match dt {
        Some(dt) if dt > 0.0 && s > 0.0 => 1.0 - s.powf(dt * FOLLOW_REF_HZ),
        _ => 1.0 - s,
    }
}

/// Quelle case d'une pointe en plusieurs cases prend chaque empreinte (les modes « Anim » de
/// TVPaint : boucle, hasard, pression, direction).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum TipOrder {
    #[default]
    #[serde(rename = "boucle")]
    Loop,
    #[serde(rename = "hasard")]
    Random,
    #[serde(rename = "pression")]
    Pressure,
    #[serde(rename = "direction")]
    Direction,
}

impl TipOrder {
    pub const ALL: [TipOrder; 4] = [TipOrder::Loop, TipOrder::Random, TipOrder::Pressure, TipOrder::Direction];

    pub fn label(self) -> &'static str {
        match self {
            TipOrder::Loop => "À la suite",
            TipOrder::Random => "Au hasard",
            TipOrder::Pressure => "Selon l'appui",
            TipOrder::Direction => "Selon la direction",
        }
    }

    /// La case (de 0 à `cells` − 1) de la `n`-ième empreinte, pour ces entrées ; `random` de 0 à 1.
    pub fn cell(self, cells: u32, n: u32, inp: &Inputs, random: f32) -> u32 {
        let cells = cells.max(1);
        let pick = |x: f32| ((x.clamp(0.0, 1.0) * cells as f32) as u32).min(cells - 1);
        match self {
            TipOrder::Loop => n % cells,
            TipOrder::Random => pick(random),
            TipOrder::Pressure => pick(inp.pressure),
            TipOrder::Direction => pick(inp.direction),
        }
    }
}

/// Grain propre du crayon (rendu des brosses, étape 3) : les quatre forces du menu « Grain » du
/// crayon de TVPaint (Silk, Soft, Medium, Hard — leur calcul n'est pas publié, celui-ci est de
/// Pelure : un papier fin collé à la feuille, `brushtex::pencil_grain_paper`, à une force et une
/// douceur par niveau).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum PencilGrain {
    #[default]
    #[serde(rename = "aucun")]
    None,
    #[serde(rename = "soie")]
    Silk,
    #[serde(rename = "doux")]
    Soft,
    #[serde(rename = "moyen")]
    Medium,
    #[serde(rename = "dur")]
    Hard,
}

impl PencilGrain {
    pub const ALL: [PencilGrain; 5] = [PencilGrain::None, PencilGrain::Silk, PencilGrain::Soft, PencilGrain::Medium, PencilGrain::Hard];

    pub fn label(self) -> &'static str {
        match self {
            PencilGrain::None => "Aucun",
            PencilGrain::Silk => "Soie",
            PencilGrain::Soft => "Doux",
            PencilGrain::Medium => "Moyen",
            PencilGrain::Hard => "Dur",
        }
    }

    /// Force du grain (la part de couleur retenue dans les creux).
    pub fn strength(self) -> f32 {
        match self {
            PencilGrain::None => 0.0,
            PencilGrain::Silk => 0.35,
            PencilGrain::Soft => 0.55,
            PencilGrain::Medium => 0.75,
            PencilGrain::Hard => 0.92,
        }
    }

    /// Douceur de la transition (voir `grain_factor`).
    pub fn softness(self) -> f32 {
        match self {
            PencilGrain::None | PencilGrain::Silk => 0.45,
            PencilGrain::Soft => 0.35,
            PencilGrain::Medium => 0.28,
            PencilGrain::Hard => 0.2,
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
    /// Couleur (demande d'Anthony, 2026-10-17) : la part de la couleur d'arrière-plan (B) mêlée à
    /// celle d'avant-plan (A) ; aucune source : la couleur A seule.
    #[serde(rename = "couleur")]
    pub color: Connection,
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
            color: Connection::default(),
            speed_max: 2000.0,
            fade_length: 300.0,
        }
    }
}

impl Dynamics {
    pub fn uses(&self, s: Source) -> bool {
        [self.size, self.opacity, self.angle, self.roundness, self.color].iter().any(|c| c.source == s)
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

/// Toucher du stylet (sensation des brosses, étude du 2026-10-18) : la réponse à la pression
/// commune à tous les pinceaux, réglée une fois pour toutes (préférence) et appliquée par le moteur avant les connexions de chaque brosse.
/// `Toucher::ancien()` est le comportement d'avant, à portée de main pour comparer à la tablette.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Toucher {
    /// Courbe de pression globale : l'appui lu sur la tablette passe par elle avant tout le reste
    /// (droite : tel quel). La souris, qui appuie toujours à fond, n'y passe pas.
    #[serde(rename = "courbe_pression")]
    pub pressure_curve: Curve,
    /// Plancher de la part de taille quand on effleure, si la taille suit l'appui (ancien : 0,03,
    /// soit un fil de 0,18 px pour un pinceau de 6).
    #[serde(rename = "plancher_taille")]
    pub size_floor: f32,
    /// Plus fin qu'un pixel : garder un demi-pixel de rayon et un pixel plein (vrai), ou éclaircir
    /// le débit en gardant 0,75 px de rayon (ancien : des traits fins gris et mous).
    #[serde(rename = "fin_plein")]
    pub thin_full: bool,
    /// Bord des empreintes rondes en une seule chute douce, large d'au moins un pixel (vrai ; le
    /// fondu minimal du « pixel feather » de libmypaint) ; ancien : rampe linéaire d'un pixel
    /// coupée par la chute de la dureté (un léger cran, des petites tailles grises).
    #[serde(rename = "bord_doux")]
    pub soft_edge: bool,
    /// Appui au-dessous duquel le stylet ne pose rien, au posé comme au levé (0 : dès le premier
    /// échantillon). La souris n'est pas concernée.
    #[serde(rename = "seuil_appui")]
    pub threshold: f32,
}

impl Default for Toucher {
    fn default() -> Self {
        Toucher { pressure_curve: Curve::linear(), size_floor: 0.1, thin_full: true, soft_edge: true, threshold: 0.03 }
    }
}

impl Toucher {
    /// Le toucher d'avant l'étude (pour comparer à la tablette).
    pub fn ancien() -> Self {
        Toucher { pressure_curve: Curve::linear(), size_floor: 0.03, thin_full: false, soft_edge: false, threshold: 0.0 }
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
            mix: 0.0,
            charge: 1.0,
            mix_kind: MixKind::Pigment,
            water: 0.5,
            smudge: 0.6,
            mix_off: false,
            hardness: 0.85,
            profile: None,
            spacing: 0.08,
            smoothing: 0.3,
            smooth_kind: SmoothKind::Follow,
            smooth_px: 8.0,
            smooth_points: 6,
            smooth_time: true,
            smooth_screen: true,
            smooth_deferred: false,
            catch_up: CatchUp::Curve,
            pressure_size: true,
            pressure_opacity: false,
            aliased: false,
            // 0,18 (étude du toucher, 2026-10-18) : un trait qui garde du corps quand on effleure,
            // comme le minimum de 10 % des outils TVPaint d'Anthony ; avant : 0, des bouts en fil.
            min_size: 0.18,
            min_opacity: 0.0,
            grain: 0.0,
            paper_scale: 1.0,
            paper_angle: 0.0,
            paper_invert: false,
            paper_softness: crate::brushtex::GRAIN_SOFTNESS_OLD,
            paper_follow: false,
            pencil_grain: PencilGrain::None,
            paper: None,
            tip: None,
            tip_cells: 1,
            tip_order: TipOrder::Loop,
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

    /// La couleur se mélange-t-elle, avec ces réglages ? Faux si la case est décochée (`mix_off`), ou si le
    /// type choisi n'a rien à mêler (pigment sans mélange ni décharge, eau sans eau, doigt sans entraînement).
    pub fn mixes(&self) -> bool {
        !self.mix_off
            && match self.mix_kind {
                MixKind::Pigment => self.mix > 0.0 || self.charge < 1.0,
                MixKind::Water => self.water > 0.0,
                MixKind::Smudge => self.smudge > 0.0,
            }
    }

    /// Allume le mélange (la case « La couleur se mélange » est cochée) : les valeurs gardées reviennent ;
    /// si le type choisi n'a rien à mêler avec elles, il prend une valeur de départ.
    pub fn turn_mix_on(&mut self) {
        self.mix_off = false;
        if !self.mixes() {
            match self.mix_kind {
                MixKind::Pigment => self.mix = 0.4,
                MixKind::Water => self.water = 0.5,
                MixKind::Smudge => self.smudge = 0.6,
            }
        }
    }

    /// Le mélange agit-il, de ce type ? (Le moteur choisit son chemin d'après ceci.)
    fn mixes_as(&self, kind: MixKind) -> bool {
        self.mix_kind == kind && self.mixes()
    }

    /// Cette brosse en « Mélangeur » (l'outil) : le doigt, qui entraîne et étale ce qui est peint sans
    /// poser de couleur. Un doigt réglé pour ne rien entraîner (`mix` à 0) prend un entraînement moyen.
    pub fn as_mixer(&self) -> Self {
        let mut s = *self;
        s.mix_kind = MixKind::Smudge;
        s.mix_off = false;
        if s.smudge <= 0.0 {
            s.smudge = 0.6;
        }
        s
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

    /// Part de la taille à une empreinte, jamais moins de `floor` (le plancher du toucher ;
    /// `Toucher::ancien()` : 3 %).
    pub fn size_factor(&self, inp: &Inputs, floor: f32) -> f32 {
        self.size_connection().map_or(1.0, |c| c.at(inp).max(floor.clamp(0.0, 1.0)))
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

    /// Le « Crayon net » : le crayon de la barre d'outils d'avant l'audit du rendu (anticrénelé
    /// depuis l'étude du toucher du 2026-10-18, qui mord plus ou moins selon l'appui, lissé un
    /// rien, sans matière). L'outil Crayon part maintenant du crayon graphite (`graphite`) ; celui-ci
    /// reste dans la liste des réglages.
    pub fn pencil() -> Self {
        Self {
            size: 2.5,
            hardness: 1.0,
            spacing: 0.1,
            smoothing: 0.15,
            aliased: false,
            pressure_opacity: true,
            min_opacity: 0.3,
            ..Self::brush()
        }
    }

    // Brosses traditionnelles (rendu des brosses, étapes 1 à 3, `docs/AUDIT-RENDU-BROSSES.md`
    // §3) : pointes et papiers fabriqués par Pelure (`brushtex`), un débit qui s'accumule sous un
    // plafond d'opacité (on fonce en repassant), l'appui qui joue sur la taille et le débit, un
    // grain collé à la feuille. Tailles pour un projet 1920×1080 vu à 50 %.

    /// Crayon graphite : le réglage de départ de l'outil Crayon. Amas de grains tourné au hasard,
    /// papier à dessin fin (le sien, `PAPER_GRAPHITE`), débit qui s'accumule jusqu'à 90 % d'opacité (jamais tout à fait noir),
    /// grain propre du crayon « Doux ». (Le débit vaut pour une empreinte pleine ; une pointe en
    /// amas de grains n'en dépose qu'une part, d'où un débit réglé haut : un passage appuyé fait un
    /// gris foncé, repasser fonce.)
    pub fn graphite() -> Self {
        let mut s = Self {
            size: 6.0,
            hardness: 0.8,
            spacing: 0.12,
            smoothing: 0.15,
            opacity: 0.9,
            power: 0.85,
            build_up: true,
            pressure_size: true,
            min_size: 0.25,
            tip: Some(crate::brushtex::TIP_GRAPHITE),
            tip_cells: 8,
            tip_order: TipOrder::Random,
            angle_jitter: 360.0,
            flip_x: true,
            flip_y: true,
            size_jitter: 0.1,
            scatter: 0.03,
            paper: Some(crate::brushtex::PAPER_GRAPHITE),
            grain: 0.85,
            paper_scale: 2.0,
            paper_softness: crate::brushtex::GRAIN_SOFTNESS_SOFT,
            pencil_grain: PencilGrain::Soft,
            ..Self::brush()
        };
        // L'appui fait le débit : le crayon mord dès qu'on effleure (Anthony, 2026-10-18 : les
        // hachures légères étaient trop pâles à 50 %), et appuyé il est presque noir.
        s.dynamics.opacity = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.35], [0.4, 0.8], [1.0, 1.0]]), ..Default::default() };
        s
    }

    /// Crayon de couleur : comme le graphite, plus gros et plus clair, qui garde la trace du grain.
    pub fn colour_pencil() -> Self {
        Self {
            size: 7.0,
            opacity: 0.8,
            power: 0.75,
            grain: 0.9,
            paper_scale: 2.0,
            pencil_grain: PencilGrain::Medium,
            ..Self::graphite()
        }
    }

    /// Encre au pinceau : le ruban plein, mais des bouts effilés (courbe de taille ferme) et un
    /// bord à peine vivant.
    pub fn ink() -> Self {
        let mut s = Self {
            size: 7.0,
            hardness: 1.0,
            spacing: 0.05,
            smoothing: 0.4,
            pressure_size: true,
            min_size: 0.04,
            size_jitter: 0.03,
            ..Self::brush()
        };
        s.dynamics.size = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.0], [0.4, 0.15], [0.8, 0.7], [1.0, 1.0]]), min: 0.04, ..Default::default() };
        s
    }

    /// Feutre : un aplat à 70 %, un biseau (pointe aplatie à angle fixe), largeur constante ; deux
    /// traits croisés foncent par superposition.
    pub fn felt() -> Self {
        Self {
            size: 10.0,
            hardness: 0.9,
            spacing: 0.05,
            smoothing: 0.25,
            opacity: 0.7,
            pressure_size: false,
            roundness: 0.55,
            angle: 40.0,
            ..Self::brush()
        }
    }

    /// Fusain : un bâton cotonneux qui s'écrase avec l'appui, débit 60 % qui s'accumule, grain du
    /// papier, un peu de dispersion.
    pub fn charcoal() -> Self {
        let mut s = Self {
            size: 14.0,
            spacing: 0.1,
            smoothing: 0.2,
            opacity: 0.95,
            power: 0.8,
            build_up: true,
            pressure_size: true,
            min_size: 0.5,
            tip: Some(crate::brushtex::TIP_CHARCOAL),
            tip_cells: 6,
            tip_order: TipOrder::Random,
            angle_jitter: 40.0,
            flip_x: true,
            flip_y: true,
            scatter: 0.05,
            size_jitter: 0.1,
            paper: Some(crate::brushtex::PAPER_DRAWING),
            grain: 0.6,
            paper_scale: 2.5,
            paper_softness: crate::brushtex::GRAIN_SOFTNESS_SOFT,
            ..Self::brush()
        };
        s.dynamics.opacity = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.2], [1.0, 1.0]]), ..Default::default() };
        s.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
        s
    }

    /// Craie : un bâton net et plat, débit 70 % qui s'accumule, sur une toile.
    pub fn chalk() -> Self {
        let mut s = Self {
            size: 16.0,
            spacing: 0.08,
            smoothing: 0.2,
            opacity: 1.0,
            power: 0.85,
            build_up: true,
            pressure_size: true,
            min_size: 0.5,
            tip: Some(crate::brushtex::TIP_CHALK),
            tip_cells: 6,
            tip_order: TipOrder::Random,
            angle_jitter: 30.0,
            flip_x: true,
            flip_y: true,
            paper: Some(crate::brushtex::PAPER_CANVAS),
            grain: 0.8,
            paper_scale: 1.5,
            paper_softness: 0.25,
            ..Self::brush()
        };
        s.dynamics.opacity = Connection { source: Source::Pressure, curve: Curve::from_points(&[[0.0, 0.3], [1.0, 1.0]]), ..Default::default() };
        s.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
        s
    }

    /// Gouache : des poils plats qui suivent le trait, une peinture opaque qui se mêle un peu à ce
    /// qui est dessous et dont la brosse se vide doucement.
    pub fn gouache() -> Self {
        let mut s = Self {
            size: 18.0,
            hardness: 0.6,
            spacing: 0.06,
            smoothing: 0.3,
            opacity: 1.0,
            power: 0.9,
            mix: 0.25,
            charge: 0.95,
            pressure_size: true,
            min_size: 0.6,
            tip: Some(crate::brushtex::TIP_BRISTLES_FLAT),
            tip_cells: 4,
            tip_order: TipOrder::Loop,
            angle_jitter: 8.0,
            ..Self::brush()
        };
        s.dynamics.angle = Connection { source: Source::Direction, ..Default::default() };
        s
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
    s.smooth_deferred = false;
    // (Le doigt ne pose rien sur une feuille vide : son aperçu montre la pointe, comme un pinceau.)
    if s.mix_kind == MixKind::Smudge {
        s.mix_off = true;
    }
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
    /// Appui (après la courbe de pression) et instant (−1 : inconnu) de chaque point de `path` :
    /// la trace qui permet au trait vectoriel de rejouer ses empreintes exactement (étape 5).
    pub path_pressure: Vec<f32>,
    pub path_time: Vec<f64>,
    /// Instant du point en cours (−1 : inconnu).
    cur_time: f64,
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
    /// Empreintes posées jusqu'ici (case d'une pointe en plusieurs cases, « à la suite »).
    dabs: u32,
    /// Peinture humide : la couleur dans la brosse (0 à 255, sans prémultiplication) et ce qui
    /// reste de sa charge (0 à 1).
    wet_rgb: [f32; 3],
    wet_load: f32,
    /// Le doigt (`MixKind::Smudge`) : la couverture (0 à 1) de ce que la brosse emporte ; elle ne prend
    /// rien avant son premier contact.
    wet_cover: f32,
    wet_started: bool,
    /// Les points reçus tels quels (x, y, appui, instant ; −1 : instant inconnu), pour le banc
    /// de traits (moteur de dessin, étape 1).
    pub raw_log: Vec<[f64; 4]>,
    next_time: f64,
    /// Couleur d'arrière-plan (B), mêlée à la couleur du trait par la connexion « couleur ».
    secondary: Option<[u8; 3]>,
    /// Peint sur l'aplat (`brushfill`, l'ancien « peint derrière ») : la peinture passe sous ce qui est déjà dessiné.
    behind: bool,
    /// Remplir la forme (`brushfill` ; plus proposé par l'interface depuis le 2026-10-23, l'aplat sous le trait le
    /// remplace) : au lâcher, un trait fermé (à `close_tol` près) remplit
    /// son intérieur de sa couleur.
    fill_closed: bool,
    close_tol: f32,
    /// Toucher du stylet (courbe de pression globale, plancher, traits fins, bord, seuil).
    feel: Toucher,
    /// Dernier point où le stylet touchait (appui au-dessus du seuil), déjà passé par la courbe :
    /// c'est lui que le trait rattrape au levé, pas un échantillon où la pression s'effondre.
    contact: Option<(Pos2, f32)>,
    /// Pixels d'écran par pixel du dessin (corde et distance en pixels d'écran).
    zoom: f32,
    /// Instant du point d'avant, et temps écoulé depuis lui pour le point en cours (lissage selon
    /// le temps) ; aucun : point sans instant.
    last_time: Option<f64>,
    dt: Option<f32>,
    /// Lissage après le trait : on rejoue les points reçus, lissage allumé.
    replaying: bool,
    /// Première empreinte du trait : l'origine du papier quand il suit le trait.
    origin: Option<Pos2>,
    /// Le trait est peint sur une image décalée de (x, y) pixels par rapport à la feuille (une
    /// marge à gauche et en haut, pour garder ce qui dépasse de la feuille) : le papier se lit
    /// comme sur la feuille, pas sur l'image décalée.
    grain_off: (i64, i64),
    /// Le trait ne sort pas de l'image qu'il peint (comportement d'origine : les images temporaires
    /// du rendu des traits vectoriels, les cases de la planche…). Faux : toile infinie, le trait
    /// peint où il va, hors de la feuille aussi (`Raster::outer`).
    bounded: bool,
}

/// Graine du hasard d'un trait (la même à chaque fois : un trait rejoué est le même).
const RNG_SEED: u32 = 0x9E37_79B9;

/// La couleur dans la brosse au départ d'un trait (peinture humide).
fn wet_start(mode: StrokeMode) -> [f32; 3] {
    match mode {
        StrokeMode::Paint(rgb) => [rgb[0] as f32, rgb[1] as f32, rgb[2] as f32],
        StrokeMode::Erase => [0.0; 3],
    }
}

/// Point d'une Bézier quadratique de `a` à `b`, de contrôle `c`, en `t` (0 à 1).
fn bezier2(a: Pos2, c: Pos2, b: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    Pos2::new(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y)
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
            path_pressure: Vec::new(),
            path_time: Vec::new(),
            cur_time: -1.0,
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
            rng: RNG_SEED,
            keep_alpha: false,
            stencil: None,
            clock: None,
            since_clock: 0.0,
            speed: 0.0,
            travel: 0.0,
            direction: 0.0,
            history: std::collections::VecDeque::new(),
            dabs: 0,
            wet_rgb: wet_start(mode),
            wet_load: 1.0,
            wet_cover: 0.0,
            wet_started: false,
            raw_log: Vec::new(),
            next_time: -1.0,
            secondary: None,
            behind: false,
            fill_closed: false,
            close_tol: 0.0,
            feel: Toucher::default(),
            contact: None,
            zoom: 1.0,
            last_time: None,
            dt: None,
            replaying: false,
            origin: None,
            grain_off: (0, 0),
            bounded: true,
        }
    }

    /// Toile infinie : le trait peint partout où il passe, hors de la feuille comprise (les tuiles
    /// hors de la fenêtre du dessin sont gardées à part, `Raster::outer`).
    pub fn on_infinite_canvas(mut self) -> Self {
        self.bounded = false;
        self
    }

    /// Le trait est peint sur une image décalée de `off` pixels (marge à gauche et en haut) :
    /// le grain du papier reste celui de la feuille. Les positions du chemin sont celles de l'image
    /// décalée (l'appelant les a décalées aussi).
    pub fn with_grain_offset(mut self, off: (i64, i64)) -> Self {
        self.grain_off = off;
        self
    }

    /// Le zoom de la vue (pixels d'écran par pixel du dessin), pour la corde et la distance du
    /// lissage mesurées à l'écran ; sans lui, 1.
    pub fn with_zoom(mut self, zoom: f32) -> Self {
        self.zoom = if zoom.is_finite() && zoom > 0.0 { zoom } else { 1.0 };
        self
    }

    /// Le toucher du stylet (réglage commun à tous les pinceaux ; sans lui, celui par défaut).
    pub fn with_feel(mut self, feel: Toucher) -> Self {
        self.feel = feel;
        self
    }

    pub fn feel(&self) -> Toucher {
        self.feel
    }

    /// Part de la taille à une empreinte, avec le plancher du toucher.
    fn size_factor(&self, inp: &Inputs) -> f32 {
        self.settings.size_factor(inp, self.feel.size_floor)
    }

    /// Options du pinceau (`brushfill`) : peint derrière ; remplir la forme, un trait étant
    /// fermé quand son bout revient à moins de `close_tol` pixels de son début.
    pub fn with_options(mut self, behind: bool, fill_closed: bool, close_tol: f32) -> Self {
        self.behind = behind;
        self.fill_closed = fill_closed;
        self.close_tol = close_tol;
        self
    }

    /// La couleur d'arrière-plan (B), pour une brosse qui mêle deux couleurs.
    pub fn with_secondary(mut self, rgb: [u8; 3]) -> Self {
        self.secondary = Some(rgb);
        self
    }

    /// La couleur posée à une empreinte : A, ou A mêlée à B par la connexion « couleur ».
    fn paint_color(&self, a: [u8; 3], inp: &Inputs) -> [u8; 3] {
        let c = self.settings.dynamics.color;
        match self.secondary {
            Some(b) if c.active() => {
                let t = c.at(inp);
                std::array::from_fn(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * t).round().clamp(0.0, 255.0) as u8)
            }
            _ => a,
        }
    }

    /// Le réglage et le mode du trait (banc de traits).
    pub fn settings(&self) -> BrushSettings {
        self.settings
    }

    pub fn mode(&self) -> StrokeMode {
        self.mode
    }

    /// Le point posé pour une nouvelle position `pos` de la pointe, selon le type de lissage ;
    /// `dt` : le temps écoulé depuis le point d'avant, s'il est connu.
    fn smooth(&mut self, pos: Pos2, dt: Option<f32>) -> Pos2 {
        let s = self.settings;
        let Some(prev) = self.smoothed else {
            self.history.push_back(pos);
            return pos;
        };
        // Lissage après le trait : brut pendant le geste, lissé au rejeu du levé.
        if s.smooth_deferred && !self.replaying {
            self.history.push_back(pos);
            return pos;
        }
        // Corde et distance : en pixels de l'écran si demandé.
        let px = if s.smooth_screen { s.smooth_px / self.zoom } else { s.smooth_px }.max(0.0);
        match s.smooth_kind {
            SmoothKind::Follow => prev + (pos - prev) * follow_alpha(s.smoothing, if s.smooth_time { dt } else { None }),
            SmoothKind::PulledString => {
                let len = px;
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
                // On garde les positions des derniers `px` pixels parcourus.
                let span = px;
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
        Some((last, raw, self.settings.size * 0.5 * self.size_factor(&self.inputs(pressure, 0.0))))
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
        // Temps écoulé depuis le point d'avant (lissage selon le temps) ; nul ou négatif : inconnu.
        self.dt = self.last_time.map(|t0| (time - t0) as f32).filter(|d| *d > 0.0);
        self.last_time = Some(time);
        self.next_time = time;
        self.add_point(raster, pos, pressure, dirty);
    }

    pub fn add_point(&mut self, raster: &mut Raster, pos: Pos2, pressure: f32, dirty: &mut Vec<usize>) {
        self.cur_time = std::mem::replace(&mut self.next_time, -1.0);
        self.raw_log.push([pos.x as f64, pos.y as f64, pressure as f64, self.cur_time]);
        // Toucher du stylet : au-dessous du seuil, le stylet ne touche pas (rien n'est posé ; le
        // trait reprendra, ou finira, au dernier point qui touchait). L'appui passe ensuite par la
        // courbe de pression globale. La souris appuie toujours à fond.
        let touching = !self.pen || self.feel.threshold <= 0.0 || pressure >= self.feel.threshold;
        let pressure = if self.pen { self.feel.pressure_curve.eval(pressure) } else { pressure };
        self.raw = Some((pos, pressure));
        let dt = self.dt.take();
        if !touching {
            return;
        }
        self.contact = Some((pos, pressure));
        let target = self.smooth(pos, dt);
        self.smoothed = Some(target);
        self.record(target, pressure);
        self.move_to(raster, target, pressure, dirty);
    }

    fn record(&mut self, p: Pos2, pressure: f32) {
        let r = self.settings.size * 0.5 * self.size_factor(&self.inputs(pressure, 0.0));
        self.path.push(VPoint { x: p.x, y: p.y, r });
        // La trace du trait vectoriel (étape 5) : de quoi rejouer les empreintes exactement.
        self.path_pressure.push(pressure);
        self.path_time.push(self.cur_time);
    }

    /// Le lissage a-t-il un effet, avec ces réglages ?
    fn smoothing_active(&self) -> bool {
        let s = self.settings;
        match s.smooth_kind {
            SmoothKind::Follow => s.smoothing > 0.0,
            SmoothKind::PulledString | SmoothKind::AverageDistance => s.smooth_px > 0.0,
            SmoothKind::AveragePoints => s.smooth_points > 1,
        }
    }

    /// Lissage après le trait : le trait brut est effacé (les tuiles d'avant reprennent leur place)
    /// et les points reçus sont rejoués avec le lissage, comme s'ils arrivaient. Borne : un trait
    /// qui a touché plus de 2000 tuiles (8 millions de pixels) reste brut, le rejeu coûterait une
    /// image entière.
    fn replay_smoothed(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) {
        if self.before.len() > 2000 {
            return;
        }
        for (&idx, tile) in &self.before {
            raster.replace_slot(idx, tile.clone());
            dirty.push(idx);
        }
        let log = std::mem::take(&mut self.raw_log);
        self.reset_gesture();
        self.replaying = true;
        for p in &log {
            let pos = Pos2::new(p[0] as f32, p[1] as f32);
            if p[3] >= 0.0 {
                self.add_point_at(raster, pos, p[2] as f32, p[3], dirty);
            } else {
                self.add_point(raster, pos, p[2] as f32, dirty);
            }
        }
    }

    /// Remet le geste à son départ, tout sauf les tuiles d'avant et le journal : pour le rejouer.
    fn reset_gesture(&mut self) {
        self.path.clear();
        self.path_pressure.clear();
        self.path_time.clear();
        self.cur_time = -1.0;
        self.mask.clear();
        self.last = None;
        self.smoothed = None;
        self.raw = None;
        self.contact = None;
        self.since_dab = 0.0;
        self.rng = RNG_SEED;
        self.clock = None;
        self.since_clock = 0.0;
        self.speed = 0.0;
        self.travel = 0.0;
        self.direction = 0.0;
        self.history.clear();
        self.dabs = 0;
        self.wet_rgb = wet_start(self.mode);
        self.wet_load = 1.0;
        self.wet_cover = 0.0;
        self.wet_started = false;
        self.next_time = -1.0;
        self.last_time = None;
        self.dt = None;
        self.origin = None;
    }

    /// Au levé, le trait rattrape la pointe (le lissage le laisse en retard), selon le réglage :
    /// pas du tout, tout droit, ou en prolongeant sa courbe — une Bézier qui part dans la dernière
    /// direction du trait lissé et finit à la pointe, en autant de points qu'il faut (un tous les
    /// deux pixels), l'appui glissant vers celui de la pointe.
    fn catch_up(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) {
        let (Some((p, pr)), Some(s)) = (self.contact, self.smoothed) else { return };
        if p == s {
            return;
        }
        let n = self.path.len();
        let dir = (n >= 2)
            .then(|| self.path[n - 1].pos() - self.path[n - 2].pos())
            .filter(|d| d.length() > 1e-3)
            .map(|d| d / d.length());
        match (self.settings.catch_up, dir) {
            (CatchUp::None, _) => {}
            (CatchUp::Straight, _) | (CatchUp::Curve, None) => {
                self.move_to(raster, p, pr, dirty);
                self.record(p, pr);
            }
            (CatchUp::Curve, Some(dir)) => {
                let dist = s.distance(p);
                let ctrl = s + dir * (dist * 0.5);
                let p0 = self.last.map_or(pr, |l| l.1);
                let steps = ((dist / 2.0).ceil() as usize).clamp(2, 64);
                for k in 1..=steps {
                    let t = k as f32 / steps as f32;
                    let q = if k == steps { p } else { bezier2(s, ctrl, p, t) };
                    let pk = p0 + (pr - p0) * t;
                    self.move_to(raster, q, pk, dirty);
                    self.record(q, pk);
                }
            }
        }
    }

    /// Termine le trait (lissage après le trait s'il est demandé, puis rattrapage de la pointe) et
    /// renvoie de quoi l'annuler.
    pub fn finish(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) -> Option<Edit> {
        if self.settings.smooth_deferred && !self.replaying && self.smoothing_active() {
            self.replay_smoothed(raster, dirty);
        }
        self.catch_up(raster, dirty);
        self.fill_shape(raster, dirty);
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

    /// Remplir la forme (`brushfill`) : un trait fermé remplit son intérieur de sa couleur, sous
    /// le trait et par-dessus le dessin d'avant (sous tout s'il peint derrière, ou en peinture
    /// humide, dont la couverture n'est pas connue). Les pixels remplis entrent dans
    /// l'annulation du trait. Rien pour un calque qui préserve sa transparence.
    fn fill_shape(&mut self, raster: &mut Raster, dirty: &mut Vec<usize>) {
        let StrokeMode::Paint(rgb) = self.mode else { return };
        if !self.fill_closed || self.keep_alpha {
            return;
        }
        let pts: Vec<Pos2> = self.path.iter().map(|p| p.pos()).collect();
        let Some(outline) = crate::brushfill::closed_outline(&pts, self.close_tol) else { return };
        let Some(area) = crate::pixsel::PixelMask::from_polygons(&[outline], raster.width, raster.height) else { return };
        let b = area.bounds();
        let (x0, y0) = (b.min.x.max(0.0) as usize, b.min.y.max(0.0) as usize);
        let (x1, y1) = ((b.max.x.max(0.0).ceil() as usize).min(raster.width), (b.max.y.max(0.0).ceil() as usize).min(raster.height));
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let opacity = self.settings.opacity;
        let under_all = self.behind || self.mask.is_empty();
        for ty in y0 / TILE..=(y1 - 1) / TILE {
            for tx in x0 / TILE..=(x1 - 1) / TILE {
                let idx = ty * raster.tiles_x + tx;
                let stencil = self.stencil.as_ref();
                if stencil.is_some_and(|st| st.blocks(idx)) {
                    continue;
                }
                let (ox, oy) = (tx * TILE, ty * TILE);
                let mut cells: Vec<(usize, f32)> = Vec::new();
                for py in y0.max(oy)..y1.min(oy + TILE) {
                    for px in x0.max(ox)..x1.min(ox + TILE) {
                        let i = (py - oy) * TILE + (px - ox);
                        let cov = area.at(px, py) * stencil.map_or(1.0, |st| st.at(idx, i));
                        if cov > 0.0 {
                            cells.push((i, cov));
                        }
                    }
                }
                if cells.is_empty() {
                    continue;
                }
                if !self.before.contains_key(&idx) {
                    self.before.insert(idx, raster.tiles[idx].clone());
                }
                let before = self.before[&idx].clone();
                let stroke = self.mask.get(&idx);
                let tile = raster.tile_mut(idx);
                for (i, cov) in cells {
                    let fill = crate::brushfill::solid(rgb, opacity * cov);
                    tile[i] = if under_all {
                        crate::brushfill::over_premul(tile[i], fill)
                    } else {
                        let a = stroke.map_or(0.0, |m| m[i] * opacity);
                        let b = before.as_deref().map_or(Color32::TRANSPARENT, |t| t[i]);
                        crate::brushfill::fill_under_stroke(tile[i], b, a, fill)
                    };
                }
                dirty.push(idx);
            }
        }
    }

    fn step(&self, inp: &Inputs) -> f32 {
        let diameter = self.settings.size * self.size_factor(inp);
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
        let diameter = s.size * self.size_factor(&inp);
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
        let mut radius = s.size * 0.5 * self.size_factor(&inp);
        if s.size_jitter > 0.0 {
            radius *= (1.0 + s.size_jitter * 2.0 * self.jitter()).max(0.05);
        }
        let mut flow = s.flow(&inp);
        if s.aliased {
            radius = radius.max(0.71);
        } else if radius < 0.75 {
            if self.feel.thin_full {
                // Plus fin qu'un pixel : un trait d'un pixel plein (un demi-pixel de rayon, le bord
                // doux remplit le pixel du centre), au lieu d'un fil gris.
                radius = radius.max(0.5);
            } else {
                // (Ancien toucher.) Plus fin qu'un pixel : on éclaircit au lieu d'amincir.
                flow *= radius / 0.75;
                radius = 0.75;
            }
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
            let dabs = (diameter / (diameter * s.spacing).max(0.3)).max(1.0);
            // Une pointe en image clairsemée (amas de grains, poils) ne dépose qu'une part de son
            // carré à chaque empreinte : le débit vaut pour ce que le trait atteint, pas pour une
            // empreinte pleine (rendu des brosses, étape 2).
            let n = dabs * self.textures.tip.as_ref().map_or(1.0, |t| t.mean_coverage().clamp(0.05, 1.0));
            1.0 - (1.0 - s.power.clamp(0.0, 1.0)).powf(1.0 / n)
        } else {
            s.power.clamp(0.0, 1.0)
        };
        let inner = (radius * s.hardness.clamp(0.0, 1.0)).min(radius - 0.5).max(0.0);
        // Bord doux (toucher) : une seule chute, de rayon × dureté au bord, large d'un pixel au moins.
        let soft = self.feel.soft_edge && !s.aliased;
        let inner_soft = radius * s.hardness.clamp(0.0, 1.0);
        // Pointe aplatie ou tournée : on mesure chaque pixel dans le repère de la pointe.
        let angle = (s.angle_at(&inp) + s.angle_jitter * self.jitter()).to_radians();
        let (sin, cos) = angle.sin_cos();
        let squash = s.roundness_at(&inp).clamp(0.05, 1.0);
        let shaped = squash < 1.0 || self.textures.tip.is_some();
        let tip = self.textures.tip.clone();
        // Pointe en plusieurs cases : la case de cette empreinte (le hasard n'est tiré que pour
        // l'ordre « au hasard »).
        let cells = if tip.is_some() { s.tip_cells.clamp(1, 64) } else { 1 };
        let cell = if cells > 1 {
            let r = if s.tip_order == TipOrder::Random { self.jitter() + 0.5 } else { 0.5 };
            s.tip_order.cell(cells, self.dabs, &inp, r)
        } else {
            0
        };
        self.dabs = self.dabs.wrapping_add(1);
        let tip = tip.as_ref().map(|t| t.level(radius * 2.0 * cells as f32));
        // Retournements au hasard d'une pointe en image (tirés seulement s'ils sont demandés).
        let flip_u = tip.is_some() && s.flip_x && self.jitter() < 0.0;
        let flip_v = tip.is_some() && s.flip_y && self.jitter() < 0.0;
        let paper = self.textures.paper.clone().filter(|_| s.grain > 0.0);
        // Le grain propre du crayon (rendu, étape 3), même sans papier.
        let pencil = (s.pencil_grain != PencilGrain::None).then(crate::brushtex::pencil_grain_paper);
        let profile = s.profile.map(|c| profile_lut(&c));
        // Le papier suit le trait : le relief est lu depuis le premier point du trait ; sinon il
        // est collé à la feuille (origine en haut à gauche, comme avant).
        if self.origin.is_none() {
            self.origin = Some(c);
        }
        // (Image décalée : le papier fixe sur la feuille se lit à l'origine de la feuille.)
        let (gox, goy) = self.grain_off;
        let origin = if s.paper_follow { self.origin.unwrap_or(c) } else { Pos2::new(gox as f32, goy as f32) };
        // Papier tel quel (comme avant), ou mis à l'échelle et tourné.
        let paper_plain = (s.paper_scale - 1.0).abs() < 1e-4 && s.paper_angle == 0.0 && !s.paper_follow;
        let pscale = s.paper_scale.clamp(0.05, 20.0);
        let (psin, pcos) = s.paper_angle.to_radians().sin_cos();
        // Grain du papier en un pixel, fixe sur le dessin (à sa taille, son angle, inversé ou non),
        // fois le grain du crayon.
        let grain_at = |px: i64, py: i64| -> f32 {
            let (x, y) = (px as f32 + 0.5 - origin.x, py as f32 + 0.5 - origin.y);
            let (sx, sy) = (px - gox, py - goy);
            let mut f = paper.as_ref().map_or(1.0, |p| {
                let h = if paper_plain {
                    p.tiled_at(sx, sy)
                } else {
                    p.sample_wrapped((x * pcos + y * psin) / pscale, (-x * psin + y * pcos) / pscale)
                };
                grain_factor(if s.paper_invert { 1.0 - h } else { h }, pressure, s.grain, s.paper_softness)
            });
            if let Some(g) = &pencil {
                let h = if s.paper_follow { g.sample_wrapped(x, y) } else { g.tiled_at(sx, sy) };
                f *= grain_factor(h, pressure, s.pencil_grain.strength(), s.pencil_grain.softness());
            }
            f
        };
        // Une pointe en image tournée déborde du cercle : on couvre le carré qui la contient.
        let reach = if tip.is_some() { radius * std::f32::consts::SQRT_2 } else { radius };

        // Les bornes en pixels du monde : la feuille du dessin, ou (toile infinie) tout l'espace.
        let (lo_x, lo_y, hi_x, hi_y) = if self.bounded { (0, 0, raster.width as i64, raster.height as i64) } else { (-WORLD, -WORLD, WORLD, WORLD) };
        let x0 = ((c.x - reach - 1.0).floor() as i64).clamp(lo_x, hi_x);
        let y0 = ((c.y - reach - 1.0).floor() as i64).clamp(lo_y, hi_y);
        let x1 = ((c.x + reach + 1.0).ceil() as i64).clamp(lo_x, hi_x);
        let y1 = ((c.y + reach + 1.0).ceil() as i64).clamp(lo_y, hi_y);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let tile_px = TILE as i64;
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
                    let fu = lx / (2.0 * radius) + 0.5;
                    if cells > 1 && !(0.0..1.0).contains(&fu) {
                        return 0.0;
                    }
                    let u = (fu + cell as f32) / cells as f32 * img.width as f32;
                    let v = (ly / (2.0 * radius) + 0.5) * img.height as f32;
                    img.sample(u, v)
                }
                None => match &profile {
                    Some(lut) => profile_coverage((lx * lx + ly * ly).sqrt(), radius, lut, s.aliased),
                    None if soft => coverage_soft((lx * lx + ly * ly).sqrt(), radius, inner_soft),
                    None => coverage((lx * lx + ly * ly).sqrt(), radius, inner, s.aliased),
                },
            }
        };

        // Préserver la transparence : on ne peint que sur ce qui est déjà dessiné (13b).
        let keep_alpha = self.keep_alpha && matches!(mode, StrokeMode::Paint(_));
        // Couleur de cette empreinte (A, ou A mêlée à B).
        let mut mode = match mode {
            StrokeMode::Paint(a) => StrokeMode::Paint(self.paint_color(a, &inp)),
            m => m,
        };
        // « La couleur se mélange » (réglages de la brosse, `mixes`) : pigment (gouache, acrylique), doigt
        // (rien de posé, seulement l'entraînement) ou eau (aquarelle). Seul le trait de peinture se mêle, jamais
        // la gomme. Sans mélange : le chemin d'avant, inchangé.
        let painting = matches!(mode, StrokeMode::Paint(_));
        let pigment = painting && s.mixes_as(MixKind::Pigment);
        let finger = painting && s.mixes_as(MixKind::Smudge);
        let water = painting && s.mixes_as(MixKind::Water);
        // La couleur moyenne sous l'empreinte (sans prémultiplication) et sa couverture, lues dans le calque
        // même, pendant le trait : (couleur, couverture, il y a de la peinture dessous).
        let under = |raster: &Raster| -> ([f32; 3], f32, bool) {
            let (mut acc, mut wsum) = ([0f32; 4], 0f32);
            for py in y0..y1 {
                for px in x0..x1 {
                    let cov = footprint(px as f32 + 0.5 - c.x, py as f32 + 0.5 - c.y);
                    if cov <= 0.0 {
                        continue;
                    }
                    let p = raster.get_i(px, py);
                    acc[0] += p.r() as f32 * cov;
                    acc[1] += p.g() as f32 * cov;
                    acc[2] += p.b() as f32 * cov;
                    acc[3] += p.a() as f32 * cov;
                    wsum += cov;
                }
            }
            let cover = if wsum > 0.0 { acc[3] / wsum / 255.0 } else { 0.0 };
            if acc[3] > 0.0 {
                ([acc[0] / acc[3] * 255.0, acc[1] / acc[3] * 255.0, acc[2] / acc[3] * 255.0], cover, true)
            } else {
                ([0.0; 3], cover, false)
            }
        };
        // Peinture humide (étape 11) : la brosse prend la couleur sous elle, la mêle à la sienne
        // et pose le mélange directement (pas de masque : repasser mêle encore). Le doigt fait de même
        // sans couleur à lui : il emporte ce qu'il a pris.
        if pigment || finger {
            // Une brosse à deux couleurs se recharge de la couleur de l'empreinte.
            if let (StrokeMode::Paint(fresh), true, true) = (mode, s.dynamics.color.active(), pigment) {
                for k in 0..3 {
                    self.wet_rgb[k] += (fresh[k] as f32 - self.wet_rgb[k]) * self.wet_load;
                }
            }
            let (under_rgb, under_a, any) = under(raster);
            let (rgb, target_a, strength);
            if finger {
                // Entraînement e : élevé, la brosse garde longtemps ce qu'elle a pris (un long traînage) ;
                // bas, elle reprend vite la couleur du lieu. Au premier contact, elle prend tout.
                let e = s.smudge.clamp(0.0, 1.0);
                let pick = if self.wet_started { (1.0 - e) * (1.0 - e) * 0.9 + 0.03 } else { 1.0 };
                self.wet_started = true;
                let carried = self.wet_cover;
                if any && under_a > 0.0 {
                    // La part de la couleur dessous dans ce que la brosse emporte, pesée par les couvertures.
                    let t = pick * under_a / (carried * (1.0 - pick) + pick * under_a);
                    self.wet_rgb = crate::melange::melanger(self.wet_rgb, under_rgb, t);
                }
                self.wet_cover = (carried + (under_a - carried) * pick).clamp(0.0, 1.0);
                (rgb, target_a, strength) = (self.wet_rgb, self.wet_cover, 0.35 + 0.65 * e);
            } else {
                if any {
                    // Mélange soustractif : bleu plus jaune donnent un vert, pas un gris.
                    let pick = s.mix.clamp(0.0, 1.0) * under_a;
                    if pick > 0.002 {
                        self.wet_rgb = crate::melange::melanger(self.wet_rgb, under_rgb, pick);
                    }
                }
                let load = self.wet_load;
                // La charge s'épuise avec la distance (en diamètres, d'une empreinte à la suivante) :
                // sans charge, il reste un vingtième de la peinture au bout de dix diamètres.
                self.wet_load *= (-(1.0 - s.charge.clamp(0.0, 1.0)) * 0.3 * s.spacing.max(0.02)).exp();
                // Ce que pose la brosse : sa charge, plus ce qu'elle entraîne de la peinture prise.
                let target = (load + (1.0 - load) * under_a * s.mix.clamp(0.0, 1.0)).clamp(0.0, 1.0);
                (rgb, target_a, strength) = (self.wet_rgb, target, 1.0);
            }
            let behind = self.behind;
            let stencil = self.stencil.as_ref();
            for ty in y0.div_euclid(tile_px)..=(y1 - 1).div_euclid(tile_px) {
                for tx in x0.div_euclid(tile_px)..=(x1 - 1).div_euclid(tile_px) {
                    let idx = raster.id_of(tx as i32, ty as i32);
                    // (Le doigt qui ne porte rien n'a rien à poser sur une tuile vide : on ne l'alloue pas.)
                    if (keep_alpha && raster.slot(idx).is_none()) || stencil.is_some_and(|st| st.blocks(idx)) || (finger && target_a <= 0.0 && raster.slot(idx).is_none()) {
                        continue;
                    }
                    let (ox, oy) = (tx * tile_px, ty * tile_px);
                    if !self.before.contains_key(&idx) {
                        self.before.insert(idx, raster.slot(idx).cloned());
                    }
                    let tile = raster.tile_mut(idx);
                    let mut changed = false;
                    for py in y0.max(oy)..y1.min(oy + tile_px) {
                        for px in x0.max(ox)..x1.min(ox + tile_px) {
                            let cov = footprint(px as f32 + 0.5 - c.x, py as f32 + 0.5 - c.y);
                            if cov <= 0.0 {
                                continue;
                            }
                            let grain = grain_at(px, py);
                            let i = (py - oy) as usize * TILE + (px - ox) as usize;
                            let through = stencil.map_or(1.0, |st| st.at(idx, i));
                            let amt = cov * flow * grain * through * s.opacity * s.power.clamp(0.0, 1.0) * strength;
                            if amt <= 0.0 {
                                continue;
                            }
                            let wet = wet_blend(tile[i], rgb, target_a, amt, keep_alpha);
                            // Peint derrière : la peinture humide ne vient que sous ce qui est là.
                            tile[i] = if behind { crate::brushfill::over_premul(tile[i], wet) } else { wet };
                            changed = true;
                        }
                    }
                    if changed {
                        dirty.push(idx);
                    }
                }
            }
            return;
        }
        // Aquarelle : la couleur emportée se mêle (soustractivement) à celle des couleurs mouillées dessous,
        // d'autant plus qu'il y a d'eau ; elle est posée diluée (transparente, d'autant plus qu'il y a d'eau)
        // par le chemin ordinaire du trait (un masque : repasser dans le même trait ne fonce pas), avec un bord
        // un peu plus dense, comme le pigment que l'eau pousse vers le bord de la flaque.
        let (mut wash, mut edge) = (1.0f32, 0.0f32);
        if water {
            let w = s.water.clamp(0.0, 1.0);
            let (under_rgb, under_a, any) = under(raster);
            if any {
                let pick = (0.1 + 0.5 * w) * under_a;
                if pick > 0.002 {
                    self.wet_rgb = crate::melange::melanger(self.wet_rgb, under_rgb, pick);
                }
            }
            mode = StrokeMode::Paint(std::array::from_fn(|k| (self.wet_rgb[k] + 0.5).clamp(0.0, 255.0) as u8));
            (wash, edge) = (1.0 - 0.75 * w, 0.5);
        }
        let stencil = self.stencil.as_ref();
        let behind = self.behind;
        let diluted = s.opacity * wash;
        for ty in y0.div_euclid(tile_px)..=(y1 - 1).div_euclid(tile_px) {
            for tx in x0.div_euclid(tile_px)..=(x1 - 1).div_euclid(tile_px) {
                let idx = raster.id_of(tx as i32, ty as i32);
                if (matches!(mode, StrokeMode::Erase) || keep_alpha) && raster.slot(idx).is_none() {
                    continue;
                }
                if stencil.is_some_and(|st| st.blocks(idx)) {
                    continue;
                }
                let (ox, oy) = (tx * tile_px, ty * tile_px);
                if !self.before.contains_key(&idx) {
                    self.before.insert(idx, raster.slot(idx).cloned());
                }
                let before = self.before[&idx].as_deref();
                let mask = self
                    .mask
                    .entry(idx)
                    .or_insert_with(|| Box::new([0.0; TILE_PIXELS]));
                let tile = raster.tile_mut(idx);
                let mut changed = false;
                for py in y0.max(oy)..y1.min(oy + tile_px) {
                    let dy = py as f32 + 0.5 - c.y;
                    for px in x0.max(ox)..x1.min(ox + tile_px) {
                        let dx = px as f32 + 0.5 - c.x;
                        let cov = footprint(dx, dy);
                        if cov <= 0.0 {
                            continue;
                        }
                        let grain = grain_at(px, py);
                        let i = (py - oy) as usize * TILE + (px - ox) as usize;
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
                        // Aquarelle : plus dense où le trait finit (m entre 0 et 1), jamais au-delà du plein.
                        let a = if edge > 0.0 { (m * (1.0 + edge * 4.0 * m * (1.0 - m))).min(1.0) * diluted } else { m * s.opacity };
                        tile[i] = match mode {
                            StrokeMode::Paint(rgb) if keep_alpha => paint_over(b, rgb, a),
                            StrokeMode::Paint(rgb) if behind => crate::brushfill::paint_under(b, rgb, a),
                            StrokeMode::Paint(rgb) => paint(b, rgb, a),
                            StrokeMode::Erase => erase(b, a),
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

/// Peinture humide : le pixel `cur` mêlé de la quantité `amt` (0 à 1) à la peinture de couleur
/// `rgb` (0 à 255) et de couverture `a` ; `keep_alpha` : sa couverture ne change pas.
pub(crate) fn wet_blend(cur: Color32, rgb: [f32; 3], a: f32, amt: f32, keep_alpha: bool) -> Color32 {
    let amt = amt.clamp(0.0, 1.0);
    let alpha = if keep_alpha { cur.a() as f32 } else { cur.a() as f32 * (1.0 - amt) + 255.0 * a * amt };
    let mix = |c: u8, t: f32| {
        // Composantes prémultipliées : la cible est la couleur fois sa couverture.
        let v = c as f32 * (1.0 - amt) + t * a * amt;
        if keep_alpha {
            // Seule la couleur change, ramenée à la couverture du pixel.
            let k = cur.a() as f32 / 255.0;
            (c as f32 * (1.0 - amt) + t * k * amt).min(cur.a() as f32)
        } else {
            v.min(alpha)
        }
    };
    let (r, g, b) = (mix(cur.r(), rgb[0]), mix(cur.g(), rgb[1]), mix(cur.b(), rgb[2]));
    Color32::from_rgba_premultiplied((r + 0.5) as u8, (g + 0.5) as u8, (b + 0.5) as u8, (alpha + 0.5).min(255.0) as u8)
}

/// Instant du `k`-ième (de 1 à `n`) événement du pointeur d'une image d'écran dont egui ne
/// connaît que l'heure `now` (sensation des brosses, §2.5) : les échantillons du stylet sont
/// étalés régulièrement sur l'intervalle depuis l'image d'avant (`prev`), le dernier à `now`.
/// Sans image d'avant, ou hors bornes : `now`. (Approximation : winit lit tout l'historique
/// WM_POINTER d'un coup ; l'heure vraie de chaque échantillon, `PerformanceCount`, viendra avec
/// la lecture directe de WM_POINTER, à faire avec Anthony à la tablette.)
pub fn spread_time(prev: f64, now: f64, k: usize, n: usize) -> f64 {
    if n == 0 || k == 0 || k >= n || !(prev < now) {
        return now;
    }
    prev + (now - prev) * (k as f64 / n as f64)
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

/// Couverture d'un pixel à la distance `d` du centre d'une empreinte ronde de rayon `radius`,
/// pleine jusqu'à `inner` (rayon × dureté), par le bord doux du toucher : une seule chute
/// (smoothstep) de `inner` au bord extérieur `radius + 0,5`, large d'au moins un pixel — le fondu
/// minimal du « pixel feather » de libmypaint. Le rayon optique est `radius` (une pointe dure passe
/// à moitié à `d = radius`), les bords demi-durs n'ont plus de cran, et une empreinte d'un demi-pixel
/// de rayon remplit le pixel de son centre.
pub(crate) fn coverage_soft(d: f32, radius: f32, inner: f32) -> f32 {
    let outer = radius + 0.5;
    if d >= outer {
        return 0.0;
    }
    let width = (outer - inner.clamp(0.0, radius)).max(1.0);
    let t = ((d - (outer - width)) / width).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

/// Couverture d'avant le toucher (`Toucher::ancien()`, et les traits vectoriels) : une rampe
/// linéaire d'un pixel au bord, coupée par la chute de la dureté entre `inner` et `radius`.
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
