//! Transitions du montage (Anthony, 2026-10-19) : fondu enchaîné, fondu au noir, fondu au blanc,
//! volet et noir, sur une coupe entre deux plans ou au début / à la fin du film.
//!
//! # Modèle de temps (choisi, à confirmer à l'écran)
//!
//! **La durée du film ne change pas** : une transition n'avance ni ne recule aucun plan (le son, les
//! commentaires et les repères restent calés). Elle se pose **autour de la coupe** entre le plan
//! sortant A et le plan entrant B, à l'intérieur de leurs images :
//!
//! - « Centrée sur la coupe » (par défaut) : `n / 2` images avant la coupe, `n - n / 2` après ;
//! - « Finit à la coupe » : les `n` images d'avant la coupe ;
//! - « Commence à la coupe » : les `n` images d'après.
//!
//! Dans la zone, A et B sont tous deux montrés. Un plan n'a pas d'images au-delà de ses bords : la
//! partie manquante est **tenue** (A garde sa dernière image après la coupe, B sa première avant
//! elle). Les poignées du rognage (la réserve de `rognage.rs`) ne sont pas lues : à faire (une
//! transition y gagnerait des images vivantes au lieu d'images tenues). La zone ne sort jamais du
//! plan sortant ni du plan entrant (une transition plus longue qu'un plan est raccourcie).
//!
//! Au début du film, « Fondu au noir / au blanc » est un fondu **depuis** la couleur (les `n`
//! premières images, la première est toute noire) ; à la fin, un fondu **vers** la couleur (les `n`
//! dernières, la dernière est toute noire). Sur une coupe, il passe par la couleur : A s'éteint dans
//! la première moitié, B s'allume dans la seconde.
//!
//! # Rendu
//!
//! Une image du film dans une zone est un `Mix` : quelles images de quels plans (`a`, `b`), le poids
//! de B (`m`), un volet éventuel et une couche de couleur opaque (`color`, `k`). La carte graphique
//! (`gpu.rs`, lecture à l'écran) et le calcul sur processeur (`blend_images`, export) lisent le même
//! `Mix`, donc le même résultat à l'arrondi près.
//!
//! # Fichier
//!
//! `transitions.json` à côté du manifeste, sans changer `VERSION` (même principe que
//! `commentaires.json` et `rognages.json`) : absent, il n'y a pas de transition ; un fichier illisible
//! ou d'une version plus récente n'empêche pas d'ouvrir le projet et n'est jamais effacé
//! (`project::write_sidecar`, copie `.illisible` quand un fichier neuf doit le remplacer). Les
//! transitions sont dans `Structure` : une étape d'annulation les rétablit avec le reste.
//!
//! # Lot 4 des préréglages (partie 175)
//!
//! Cinq sortes de plus, de la liste de la partie 170 : la **poussée** (T3 : le plan entrant pousse le
//! sortant hors du cadre), le **raccord filé** (T4 : la même poussée, lente puis très vite puis lente,
//! avec le flou d'un panoramique filé), le **flash de coupe** (T6 : un blanc d'une ou deux images sur
//! la coupe), le **fondu par une couleur** (T7 : le fondu au noir, par la couleur choisie) et le
//! **volet par un objet** (T8 : un tronc passe tout près devant la caméra et sert de rideau). Elles se
//! calculent comme les autres, dans le cadre filmé : un `Mix` dit de combien chaque plan glisse, son
//! flou, où passe l'objet. Le fichier passe à la version 2 seulement quand une de ces sortes ou une
//! couleur y est écrite : un projet sans elles reste lisible par une version précédente.
//!
//! # Ses réponses (partie 177)
//!
//! Trois sortes de plus, restées sans réponse puis « oui » : l'**iris** (T1 : le rond de cartoon se ferme sur un point choisi,
//! puis se rouvre sur le plan suivant ; au bord du film, il s'ouvre ou se ferme seulement), le **volet en horloge** (T2 : le
//! plan entrant balaie le cadre comme l'aiguille d'une montre) et le **zoom traversant** (T5 : on plonge dans le plan, on ressort
//! du suivant). Le volet, la poussée et le raccord filé ont un **sens** (`depuis` : le plan entrant arrive de la gauche, de la
//! droite, du haut ou du bas) ; l'iris et le zoom traversant un **centre** (le point du cadre où il se ferme, où l'on plonge).
//! Le fichier passe à la version 3 seulement quand l'une de ces sortes, un sens ou un centre y est.

use std::collections::HashSet;

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "transitions.json";
const FORMAT: &str = "shot-transitions";
/// La version du fichier : 3 dès qu'une sorte de la partie 177, un sens ou un centre y est écrit ; 2 dès qu'une sorte du lot 4
/// ou une couleur y est écrite (partie 175) ; 1 sinon.
const VERSION: u32 = 3;
const VERSION_LOT4: u32 = 2;
const VERSION_SANS_LOT4: u32 = 1;
/// Durée d'une transition neuve, en images.
pub const DEFAULT_FRAMES: usize = 12;
/// La plus longue (un fichier abîmé ne fait pas calculer des milliers d'images doubles).
pub const MAX_FRAMES: usize = 240;
/// La couleur d'un fondu par une couleur neuf : un bleu nuit (celle de la page de choix).
pub const COULEUR_DEFAUT: [u8; 3] = [27, 36, 72];
/// La couleur de l'objet du volet par un objet : un tronc dans l'ombre, presque noir.
pub const OBJET_COULEUR: [u8; 3] = [30, 25, 22];
/// Le flou le plus fort du raccord filé, au milieu de la transition : la moitié de la traînée, en largeurs du cadre.
pub const FLOU_FILE: f32 = 0.12;

/// Sorte de transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// Fondu enchaîné : A se fond dans B.
    #[serde(rename = "enchaine")]
    Dissolve,
    /// Fondu au noir (par le noir sur une coupe ; depuis / vers le noir au bord du film).
    #[serde(rename = "noir")]
    Black,
    /// Fondu au blanc.
    #[serde(rename = "blanc")]
    White,
    /// Volet : B recouvre A de gauche à droite.
    #[serde(rename = "volet")]
    Wipe,
    /// Noir : les images de la zone sont toutes noires (coupe sèche vers le noir et retour).
    #[serde(rename = "coupe_noir")]
    CutBlack,
    /// Poussée (T3) : le plan entrant arrive de la droite et pousse le sortant hors du cadre.
    #[serde(rename = "poussee")]
    Push,
    /// Raccord filé (T4) : la poussée d'un panoramique filé, lente puis très vite puis lente, floue au milieu.
    #[serde(rename = "raccord_file")]
    Whip,
    /// Flash de coupe (T6) : un blanc d'une ou deux images sur la coupe.
    #[serde(rename = "flash")]
    Flash,
    /// Fondu par une couleur (T7) : le fondu au noir, par la couleur de la transition.
    #[serde(rename = "couleur")]
    Colour,
    /// Volet par un objet (T8) : un tronc passe tout près devant la caméra, de droite à gauche ; derrière lui, le plan entrant.
    #[serde(rename = "objet")]
    Objet,
    /// Iris (T1, partie 177) : le rond se ferme sur le centre de la transition, puis se rouvre sur le plan entrant.
    #[serde(rename = "iris")]
    Iris,
    /// Volet en horloge (T2) : le plan entrant balaie le cadre comme l'aiguille d'une montre, depuis midi.
    #[serde(rename = "horloge")]
    Horloge,
    /// Zoom traversant (T5) : on plonge dans le plan sortant, on ressort du plan entrant.
    #[serde(rename = "zoom_traversant")]
    ZoomTraversant,
}

/// D'où le plan entrant arrive (volet, poussée, raccord filé ; partie 177).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cote {
    #[default]
    #[serde(rename = "gauche")]
    Gauche,
    #[serde(rename = "droite")]
    Droite,
    #[serde(rename = "haut")]
    Haut,
    #[serde(rename = "bas")]
    Bas,
}

impl Cote {
    pub const ALL: [Cote; 4] = [Cote::Gauche, Cote::Droite, Cote::Haut, Cote::Bas];

    pub fn name(self) -> &'static str {
        match self {
            Cote::Gauche => "De la gauche",
            Cote::Droite => "De la droite",
            Cote::Haut => "Du haut",
            Cote::Bas => "Du bas",
        }
    }
}

impl Kind {
    pub const ALL: [Kind; 13] = [
        Kind::Dissolve,
        Kind::Black,
        Kind::White,
        Kind::Colour,
        Kind::Wipe,
        Kind::Horloge,
        Kind::Iris,
        Kind::Objet,
        Kind::Push,
        Kind::Whip,
        Kind::ZoomTraversant,
        Kind::Flash,
        Kind::CutBlack,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Dissolve => "Fondu enchaîné",
            Kind::Black => "Fondu au noir",
            Kind::White => "Fondu au blanc",
            Kind::Wipe => "Volet",
            Kind::CutBlack => "Noir",
            Kind::Push => "Poussée",
            Kind::Whip => "Raccord filé",
            Kind::Flash => "Flash de coupe",
            Kind::Colour => "Fondu par une couleur",
            Kind::Objet => "Volet par un objet",
            Kind::Iris => "Iris",
            Kind::Horloge => "Volet en horloge",
            Kind::ZoomTraversant => "Zoom traversant",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Kind::Dissolve => "Le plan sortant se fond dans le plan entrant.",
            Kind::Black => "Le plan sortant s'éteint dans le noir, puis le plan entrant s'allume (au bord du film : depuis ou vers le noir).",
            Kind::White => "Comme le fondu au noir, par le blanc.",
            Kind::Wipe => "Le plan entrant recouvre le plan sortant de gauche à droite.",
            Kind::CutBlack => "La coupe passe par des images toutes noires, sans fondu.",
            Kind::Push => "Le plan entrant arrive de la droite et pousse le plan sortant hors du cadre.",
            Kind::Whip => "Un panoramique filé sort du plan, un autre entre dans le suivant : la poussée part doucement, file très vite, floue, et se pose.",
            Kind::Flash => "Un blanc d'une ou deux images sur la coupe, qui s'éteint dans le plan entrant.",
            Kind::Colour => "Comme le fondu au noir, par la couleur choisie (clic droit sur la transition pour la changer).",
            Kind::Objet => "Un tronc passe tout près devant la caméra et sert de rideau : derrière lui, le plan entrant.",
            Kind::Iris => "Le rond de cartoon se ferme sur un point choisi (clic droit sur la transition), puis se rouvre sur le plan entrant ; au début du film il s'ouvre, à la fin il se ferme.",
            Kind::Horloge => "Le plan entrant balaie le cadre comme l'aiguille d'une montre, depuis midi.",
            Kind::ZoomTraversant => "On plonge dans le plan sortant, on ressort du plan entrant (vers le point choisi au clic droit).",
        }
    }

    /// Sorte permise sur une coupe (entre deux plans) ou au bord du film.
    pub fn allowed(self, between_plans: bool) -> bool {
        between_plans || matches!(self, Kind::Black | Kind::White | Kind::CutBlack | Kind::Colour | Kind::Iris)
    }

    /// Le côté d'où le plan entrant arrive, d'origine : la gauche pour le volet (comme toujours), la droite pour la poussée et le
    /// raccord filé (comme au lot 4) ; rien pour les sortes qui n'ont pas de sens.
    pub fn cote_d_origine(self) -> Option<Cote> {
        match self {
            Kind::Wipe => Some(Cote::Gauche),
            Kind::Push | Kind::Whip => Some(Cote::Droite),
            _ => None,
        }
    }

    /// La sorte a un centre à choisir (l'iris, le zoom traversant).
    pub fn a_un_centre(self) -> bool {
        matches!(self, Kind::Iris | Kind::ZoomTraversant)
    }

    /// Une sorte de la partie 177 : le fichier qui en contient une est écrit à la version 3.
    pub fn de_ses_reponses(self) -> bool {
        matches!(self, Kind::Iris | Kind::Horloge | Kind::ZoomTraversant)
    }

    /// La couleur de la couche : noir, blanc (aussi celui du flash), ou celle de la transition (`couleur`, sinon le bleu nuit).
    pub fn color(self, couleur: Option<[u8; 3]>) -> [u8; 3] {
        match self {
            Kind::White | Kind::Flash => [255, 255, 255],
            Kind::Colour => couleur.unwrap_or(COULEUR_DEFAUT),
            _ => [0, 0, 0],
        }
    }

    /// Durée d'une transition neuve de cette sorte, en images : courte pour le flash et le raccord filé.
    pub fn default_frames(self) -> usize {
        match self {
            Kind::Flash => 4,
            Kind::Whip => 8,
            Kind::Iris => 16,
            _ => DEFAULT_FRAMES,
        }
    }

    /// Une sorte du lot 4 (partie 175) : le fichier qui en contient une est écrit à la version 2.
    pub fn du_lot_4(self) -> bool {
        matches!(self, Kind::Push | Kind::Whip | Kind::Flash | Kind::Colour | Kind::Objet)
    }
}

/// L'objet du volet par un objet (le tronc), son milieu à l'abscisse `c` du cadre : ses quatre coins (haut-gauche, haut-droite,
/// bas-droite, bas-gauche), en parts du cadre (x vers la droite, y vers le bas). Un peu plus large en bas, comme un tronc vu de
/// près, un peu en contre-plongée.
pub fn objet_coins(c: f32) -> [[f32; 2]; 4] {
    [[c - 0.11, 0.0], [c + 0.11, 0.0], [c + 0.15, 1.0], [c - 0.15, 1.0]]
}

/// Le milieu de l'objet à l'image `i` (sur `n`) de la zone : il entre à droite, sort à gauche, à vitesse constante (un objet
/// tout près de la caméra file sans ralentir).
pub fn objet_milieu(i: usize, n: usize) -> f32 {
    1.2 - 1.4 * (i as f32 + 0.5) / n.max(1) as f32
}

/// L'avancée d'une poussée (de 0 à 1) au temps `t` : douce aux deux bouts.
fn lisse(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// L'avancée d'un raccord filé au temps `t`, et sa vitesse (3 au plus, au milieu) : très lente aux bouts, très vite au milieu.
fn file(t: f32) -> (f32, f32) {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        (4.0 * t * t * t, 12.0 * t * t)
    } else {
        let u = 2.0 - 2.0 * t;
        (1.0 - u * u * u / 2.0, 3.0 * u * u)
    }
}

/// Où la zone se pose par rapport à la coupe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    #[default]
    #[serde(rename = "centree")]
    Centre,
    /// Les images d'avant la coupe.
    #[serde(rename = "avant")]
    Before,
    /// Les images d'après la coupe.
    #[serde(rename = "apres")]
    After,
}

impl Align {
    pub const ALL: [Align; 3] = [Align::Centre, Align::Before, Align::After];

    pub fn name(self) -> &'static str {
        match self {
            Align::Centre => "Centrée sur la coupe",
            Align::Before => "Finit à la coupe",
            Align::After => "Commence à la coupe",
        }
    }

    /// (images avant la coupe, images après) pour une durée de `n` images.
    pub fn split(self, n: usize) -> (usize, usize) {
        match self {
            Align::Centre => (n / 2, n - n / 2),
            Align::Before => (n, 0),
            Align::After => (0, n),
        }
    }

    /// Durée qui amène le bord de la zone (`left` : le bord gauche, sinon le droit) à l'image `edge`
    /// du film, la coupe étant à l'image `cut`. En centré, la zone reste centrée : la durée est le
    /// double de la distance de ce bord à la coupe.
    pub fn frames_for_edge(self, cut: usize, edge: usize, left: bool) -> usize {
        let d = if left { cut.saturating_sub(edge) } else { edge.saturating_sub(cut) };
        match self {
            Align::Centre => (d * 2).clamp(1, MAX_FRAMES),
            Align::Before => if left { d } else { 1 }.clamp(1, MAX_FRAMES),
            Align::After => if left { 1 } else { d }.clamp(1, MAX_FRAMES),
        }
    }
}

/// Une transition : la coupe qu'elle habille (les plans de part et d'autre, par leur numéro), sa
/// sorte, sa durée et son calage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    pub id: u64,
    /// Le plan sortant (avant la coupe) ; rien : début du film.
    #[serde(rename = "sortant", default)]
    pub out: Option<u64>,
    /// Le plan entrant (après la coupe) ; rien : fin du film.
    #[serde(rename = "entrant", default)]
    pub inn: Option<u64>,
    #[serde(rename = "sorte")]
    pub kind: Kind,
    #[serde(rename = "images")]
    pub frames: usize,
    #[serde(rename = "calage", default, skip_serializing_if = "is_centre")]
    pub align: Align,
    /// La couleur d'un fondu par une couleur (partie 175) ; rien : le bleu nuit d'origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub couleur: Option<[u8; 3]>,
    /// D'où arrive le plan entrant d'un volet, d'une poussée, d'un raccord filé (partie 177) ; rien : le côté d'origine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depuis: Option<Cote>,
    /// Le centre d'un iris ou d'un zoom traversant, en parts du cadre (partie 177) ; rien : le milieu du cadre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre: Option<[f32; 2]>,
}

fn is_centre(a: &Align) -> bool {
    *a == Align::Centre
}

impl Transition {
    pub fn between_plans(&self) -> bool {
        self.out.is_some() && self.inn.is_some()
    }
}

// ------------------------------------------------------------------------------------- zones

/// Un plan de part ou d'autre de la zone : son rang, sa première image dans le film, sa durée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Side {
    pub panel: usize,
    pub start: usize,
    pub len: usize,
}

/// Les images du film qu'une transition recouvre, [start, end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub id: u64,
    pub kind: Kind,
    /// L'image de la coupe (première image du plan entrant ; 0 au début du film, le total à la fin).
    pub cut: usize,
    pub start: usize,
    pub end: usize,
    pub out: Option<Side>,
    pub inn: Option<Side>,
    pub align: Align,
    /// La couleur de la transition (celle d'un fondu par une couleur ; noir pour les autres sortes, qui ne la lisent pas).
    pub couleur: [u8; 3],
    /// D'où arrive le plan entrant (partie 177), et le centre de la transition, en parts du cadre.
    pub depuis: Cote,
    pub centre: [f32; 2],
    /// La hauteur du cadre sur sa largeur (le rond d'un iris reste rond).
    pub aspect: f32,
}

/// Ce que montre une image du film dans une zone (voir l'en-tête du module).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Mix {
    /// Plan et image du plan sortant, s'il se voit.
    pub a: Option<(usize, usize)>,
    /// Plan et image du plan entrant, s'il se voit.
    pub b: Option<(usize, usize)>,
    /// Volet (B recouvre A jusqu'à `m` de la largeur) au lieu d'un fondu.
    pub wipe: bool,
    /// Poids de B, de 0 (A seul) à 1 (B seul).
    pub m: f32,
    /// Couche de couleur opaque posée par-dessus, d'opacité `k`.
    pub color: [u8; 3],
    pub k: f32,
    /// Poussée, raccord filé (partie 175) : les deux plans glissent dans le cadre, A décalé de `shift_a`, B de `shift_b` (en
    /// largeurs du cadre, négatif vers la gauche), chacun coupé à son propre cadre ; B passe devant A.
    pub glisse: bool,
    pub shift_a: f32,
    pub shift_b: f32,
    /// Flou de mouvement horizontal des deux plans (raccord filé) : la moitié de la traînée, en largeurs du cadre.
    pub blur: f32,
    /// Volet par un objet : le milieu de l'objet, en parts du cadre ; A à sa gauche, B à sa droite, l'objet par-dessus.
    pub objet: Option<f32>,
    /// (Partie 177.) Le glissement va de haut en bas (sinon de gauche à droite) ; `shift_a`, `shift_b` en hauteurs du cadre.
    pub vertical: bool,
    /// Le côté d'où le volet amène B.
    pub wipe_depuis: Cote,
    /// L'iris : son centre (en parts du cadre) et son rayon (en largeurs du cadre) ; hors du rond, la couleur `color`.
    pub iris: Option<[f32; 3]>,
    /// Le volet en horloge : l'angle balayé (en radians, depuis midi, dans le sens des aiguilles) ; B dedans, A dehors.
    pub horloge: Option<f32>,
    /// Le zoom traversant : le grossissement de A et de B autour de `zoom_centre` (en parts du cadre) ; B pèse `m`.
    pub zoom: Option<[f32; 2]>,
    pub zoom_centre: [f32; 2],
}

impl Zone {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn contains(&self, g: usize) -> bool {
        (self.start..self.end).contains(&g)
    }

    /// L'image `g` du film dans cette zone ; rien si elle n'y est pas.
    pub fn mix(&self, g: usize) -> Option<Mix> {
        if !self.contains(g) {
            return None;
        }
        let n = self.len();
        let i = g - self.start;
        // Une image du plan, tenue au-delà de ses bords.
        let held = |s: &Side| (s.panel, g.saturating_sub(s.start).min(s.len.saturating_sub(1)));
        let (a, b) = (self.out.as_ref().map(held), self.inn.as_ref().map(held));
        let color = self.kind.color(Some(self.couleur));
        let mut mix = Mix { a, b, color, ..Mix::default() };
        match (self.kind, a, b) {
            // (Partie 175.) La poussée : B arrive de son côté (la droite d'origine), A sort par l'autre.
            (Kind::Push, Some(_), Some(_)) => {
                let p = lisse((i + 1) as f32 / (n + 1) as f32);
                self.pousser(&mut mix, p);
            }
            // Le raccord filé : la même poussée, très vite au milieu, floue d'autant.
            (Kind::Whip, Some(_), Some(_)) => {
                let (p, vitesse) = file((i + 1) as f32 / (n + 1) as f32);
                self.pousser(&mut mix, p);
                mix.blur = FLOU_FILE * vitesse / 3.0;
            }
            // (Partie 177.) L'iris : il se ferme sur A jusqu'au milieu, se rouvre sur B ensuite.
            (Kind::Iris, Some(_), Some(_)) => {
                let t = (i + 1) as f32 / (n + 1) as f32;
                mix.color = [0, 0, 0];
                if t < 0.5 {
                    mix.b = None;
                    mix.iris = Some(self.iris_rond(1.0 - lisse(t / 0.5)));
                } else {
                    mix.a = None;
                    mix.m = 1.0;
                    mix.iris = Some(self.iris_rond(lisse((t - 0.5) / 0.5)));
                }
            }
            (Kind::Iris, None, Some(_)) => {
                mix.color = [0, 0, 0];
                mix.m = 1.0;
                mix.iris = Some(self.iris_rond(lisse(i as f32 / n as f32)));
            }
            (Kind::Iris, Some(_), None) => {
                mix.color = [0, 0, 0];
                mix.iris = Some(self.iris_rond(1.0 - lisse((i + 1) as f32 / n as f32)));
            }
            // Le volet en horloge : B dans l'angle balayé depuis midi.
            (Kind::Horloge, Some(_), Some(_)) => {
                mix.m = 1.0;
                mix.horloge = Some(std::f32::consts::TAU * (i + 1) as f32 / (n + 1) as f32);
            }
            // Le zoom traversant : A grossit jusqu'à 4 fois, B arrive de 4 fois et se pose ; ils se fondent au milieu.
            (Kind::ZoomTraversant, Some(_), Some(_)) => {
                let t = (i + 1) as f32 / (n + 1) as f32;
                let za = 1.0 + 3.0 * (t / 0.6).min(1.0).powi(2);
                let zb = 1.0 + 3.0 * ((1.0 - t) / 0.6).min(1.0).powi(2);
                mix.zoom = Some([za, zb]);
                mix.zoom_centre = self.centre;
                mix.m = lisse((t - 0.4) / 0.2);
            }
            // Le flash : blanc aux images qui touchent la coupe, A jusqu'à elle, B après, le blanc s'éteignant de part et d'autre.
            (Kind::Flash, Some(_), Some(_)) => {
                if g < self.cut {
                    mix.b = None;
                    mix.k = (g - self.start + 1) as f32 / (self.cut - self.start) as f32;
                } else {
                    mix.a = None;
                    mix.m = 1.0;
                    mix.k = 1.0 - (g - self.cut) as f32 / (self.end - self.cut).max(1) as f32;
                }
            }
            // Le volet par un objet : le tronc passe de droite à gauche, B se découvre derrière lui.
            (Kind::Objet, Some(_), Some(_)) => {
                mix.m = 1.0;
                mix.objet = Some(objet_milieu(i, n));
            }
            // Le volet, depuis son côté (partie 177).
            (Kind::Wipe, Some(_), Some(_)) => {
                mix.wipe = true;
                mix.wipe_depuis = self.depuis;
                mix.m = (i + 1) as f32 / (n + 1) as f32;
            }
            (Kind::CutBlack, _, _) => {
                mix.k = 1.0;
                mix.color = [0, 0, 0];
                if a.is_none() {
                    mix.m = 1.0;
                } else {
                    mix.b = None;
                }
            }
            (Kind::Dissolve, Some(_), Some(_)) => mix.m = (i + 1) as f32 / (n + 1) as f32,
            // Par la couleur, sur une coupe : A s'éteint, puis B s'allume.
            (_, Some(_), Some(_)) => {
                let t = (i + 1) as f32 / (n + 1) as f32;
                if t < 0.5 {
                    mix.b = None;
                    mix.k = 2.0 * t;
                } else {
                    mix.a = None;
                    mix.m = 1.0;
                    mix.k = 2.0 * (1.0 - t);
                }
            }
            // Début du film : le plan entrant s'allume, la première image est toute de couleur.
            (_, None, Some(_)) => {
                mix.m = 1.0;
                mix.k = 1.0 - i as f32 / n as f32;
            }
            // Fin du film : le plan sortant s'éteint, la dernière image est toute de couleur.
            (_, Some(_), None) => mix.k = (i + 1) as f32 / n as f32,
            (_, None, None) => return None,
        }
        Some(mix)
    }
}

impl Zone {
    /// Le glissement d'une poussée à l'avancée `p` : B arrive de son côté, A sort par l'autre.
    fn pousser(&self, mix: &mut Mix, p: f32) {
        let (signe, vertical) = match self.depuis {
            Cote::Droite => (1.0, false),
            Cote::Gauche => (-1.0, false),
            Cote::Bas => (1.0, true),
            Cote::Haut => (-1.0, true),
        };
        (mix.glisse, mix.m, mix.vertical) = (true, 1.0, vertical);
        (mix.shift_a, mix.shift_b) = (-p * signe, (1.0 - p) * signe);
    }

    /// Le rond de l'iris ouvert à `ouvert` (0 : fermé, 1 : tout le cadre se voit) : son centre et son rayon, en largeurs du cadre.
    fn iris_rond(&self, ouvert: f32) -> [f32; 3] {
        let [cx, cy] = self.centre;
        let loin = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]].iter().map(|c: &[f32; 2]| (c[0] - cx).hypot((c[1] - cy) * self.aspect)).fold(0.0_f32, f32::max);
        [cx, cy, loin * ouvert.clamp(0.0, 1.0)]
    }
}

/// Les zones des transitions du document, par ordre de début. Une transition dont les plans ne se
/// suivent plus (ils ont été déplacés) n'a pas de zone : elle est gardée mais ne joue pas.
pub fn zones(doc: &Document) -> Vec<Zone> {
    if doc.transitions.is_empty() {
        return Vec::new();
    }
    let mut starts = Vec::with_capacity(doc.panels.len());
    let mut at = 0;
    for p in &doc.panels {
        starts.push(at);
        at += p.frame_count();
    }
    let total = at;
    let index = |id: u64| doc.panels.iter().position(|p| p.id == id);
    let side = |i: usize| Side { panel: i, start: starts[i], len: doc.panels[i].frame_count() };
    let mut out = Vec::new();
    for t in &doc.transitions {
        let n = t.frames.clamp(1, MAX_FRAMES);
        let (o, i) = match (t.out, t.inn) {
            (Some(o), Some(i)) => match (index(o), index(i)) {
                (Some(o), Some(i)) if i == o + 1 => (Some(o), Some(i)),
                _ => continue,
            },
            (None, Some(i)) => match index(i) {
                Some(0) => (None, Some(0)),
                _ => continue,
            },
            (Some(o), None) => match index(o) {
                Some(o) if o + 1 == doc.panels.len() => (Some(o), None),
                _ => continue,
            },
            (None, None) => continue,
        };
        let (cut, start, end) = match (o, i) {
            (Some(o), Some(i)) => {
                let cut = starts[i];
                let (before, after) = t.align.split(n);
                let lo = starts[o];
                let hi = starts[i] + doc.panels[i].frame_count();
                (cut, cut.saturating_sub(before).max(lo), (cut + after).min(hi))
            }
            (None, Some(i)) => (0, 0, n.min(doc.panels[i].frame_count())),
            (Some(o), None) => (total, total.saturating_sub(n).max(starts[o]), total),
            (None, None) => continue,
        };
        if end <= start {
            continue;
        }
        let couleur = t.couleur.unwrap_or(COULEUR_DEFAUT);
        let depuis = t.depuis.or(t.kind.cote_d_origine()).unwrap_or_default();
        let centre = t.centre.unwrap_or([0.5, 0.5]);
        let aspect = doc.frame[1].max(1) as f32 / doc.frame[0].max(1) as f32;
        out.push(Zone { id: t.id, kind: t.kind, cut, start, end, out: o.map(side), inn: i.map(side), align: t.align, couleur, depuis, centre, aspect });
    }
    out.sort_by_key(|z| (z.start, z.id));
    out
}

/// Ce que montre l'image `g` du film, si une transition la recouvre (la première, si deux se
/// recouvrent).
pub fn mix_in(zones: &[Zone], g: usize) -> Option<(&Zone, Mix)> {
    zones.iter().find_map(|z| z.mix(g).map(|m| (z, m)))
}

// --------------------------------------------------------------------------- rendu processeur

#[inline]
fn lerp8(x: u8, y: u8, t: u32) -> u8 {
    ((x as u32 * (256 - t) + y as u32 * t + 128) >> 8) as u8
}

#[inline]
fn lerp_px(x: Color32, y: Color32, t: u32) -> Color32 {
    let (x, y) = (x.to_array(), y.to_array());
    Color32::from_rgba_premultiplied(lerp8(x[0], y[0], t), lerp8(x[1], y[1], t), lerp8(x[2], y[2], t), lerp8(x[3], y[3], t))
}

/// Le poids `m` (0 à 1) en 256es.
fn weight(m: f32) -> u32 {
    (m.clamp(0.0, 1.0) * 256.0).round() as u32
}

/// Le flou horizontal d'une image (moyenne des pixels de `x - r` à `x + r` sur sa ligne, ceux de l'image seulement) : la
/// traînée d'un raccord filé. Les couleurs sont prémultipliées.
fn flou_horizontal(img: &[Color32], w: usize, h: usize, r: usize) -> Vec<Color32> {
    let mut out = Vec::with_capacity(img.len());
    let mut somme = vec![[0u32; 4]; w + 1];
    for y in 0..h {
        let ligne = &img[y * w..(y + 1) * w];
        for (x, p) in ligne.iter().enumerate() {
            let c = p.to_array();
            somme[x + 1] = std::array::from_fn(|k| somme[x][k] + c[k] as u32);
        }
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r + 1).min(w));
            let n = (hi - lo) as u32;
            let c: [u8; 4] = std::array::from_fn(|k| ((somme[hi][k] - somme[lo][k] + n / 2) / n) as u8);
            out.push(Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]));
        }
    }
    out
}

/// La part du pixel de la ligne `y` (de `x` à `x + 1`, en pixels) que couvre l'objet du volet par un objet (`objet_coins`)
/// posé au milieu `c`, dans une image de `w` × `h` : son bord est lissé, comme celui d'un trait.
fn objet_couvre(c: f32, x: usize, y: usize, w: usize, h: usize) -> f32 {
    let [hg, hd, bd, bg] = objet_coins(c);
    let t = (y as f32 + 0.5) / h.max(1) as f32;
    let (gauche, droite) = ((hg[0] + (bg[0] - hg[0]) * t) * w as f32, (hd[0] + (bd[0] - hd[0]) * t) * w as f32);
    (droite.min(x as f32 + 1.0) - gauche.max(x as f32)).clamp(0.0, 1.0)
}

/// Mélange les images `a` et `b` (celles du `Mix`, calculées par l'appelant, à la taille `size`) :
/// le même calcul que la carte graphique, sur le processeur. Les couleurs sont prémultipliées.
pub fn blend_images(mix: &Mix, a: Option<&[Color32]>, b: Option<&[Color32]>, size: [usize; 2]) -> Vec<Color32> {
    let [w, h] = size;
    let n = w * h;
    // (Le raccord filé : chaque plan flou d'abord, puis glissé.)
    let r = (mix.blur.max(0.0) * w as f32).round() as usize;
    let (fa, fb) = if r > 0 && w > 0 {
        (a.map(|p| flou_horizontal(p, w, h, r)), b.map(|p| flou_horizontal(p, w, h, r)))
    } else {
        (None, None)
    };
    let (a, b) = (fa.as_deref().or(a), fb.as_deref().or(b));
    let pick = |img: Option<&[Color32]>, i: usize| img.map_or(Color32::TRANSPARENT, |p| p[i]);
    let m = weight(mix.m);
    let k = weight(mix.k);
    let layer = Color32::from_rgba_premultiplied(mix.color[0], mix.color[1], mix.color[2], 255);
    let objet = Color32::from_rgba_premultiplied(OBJET_COULEUR[0], OBJET_COULEUR[1], OBJET_COULEUR[2], 255);
    // Colonne où le volet s'arrête : à gauche, B ; à droite, A.
    let edge = (mix.m.clamp(0.0, 1.0) * w as f32).round() as usize;
    // Le pixel de `img` que montre la colonne `x` quand l'image a glissé de `s` largeurs (de `s` hauteurs, en vertical) : rien
    // hors de son cadre.
    let glisse = |img: Option<&[Color32]>, s: f32, x: usize, y: usize| -> Option<Color32> {
        let p = img?;
        if mix.vertical {
            let src = y as f32 + 0.5 - s * h as f32;
            (src >= 0.0 && src < h as f32).then(|| p[src as usize * w + x])
        } else {
            let src = x as f32 + 0.5 - s * w as f32;
            (src >= 0.0 && src < w as f32).then(|| p[y * w + src as usize])
        }
    };
    // (Partie 177.) Le pixel de `img` grossi `z` fois autour du centre du zoom : rien au-delà de l'image.
    let (zcx, zcy) = (mix.zoom_centre[0] * w as f32, mix.zoom_centre[1] * h as f32);
    let grossi = |img: Option<&[Color32]>, z: f32, x: usize, y: usize| -> Color32 {
        let Some(p) = img else { return Color32::TRANSPARENT };
        let (sx, sy) = (zcx + (x as f32 + 0.5 - zcx) / z.max(1e-3), zcy + (y as f32 + 0.5 - zcy) / z.max(1e-3));
        if sx < 0.0 || sy < 0.0 || sx >= w as f32 || sy >= h as f32 {
            return Color32::TRANSPARENT;
        }
        p[sy as usize * w + sx as usize]
    };
    // Le volet depuis son côté : B se voit-il au pixel (`x`, `y`) ?
    let (bord_x, bord_y) = (mix.m.clamp(0.0, 1.0) * w as f32, mix.m.clamp(0.0, 1.0) * h as f32);
    let volet_b = |x: usize, y: usize| -> bool {
        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
        match mix.wipe_depuis {
            Cote::Gauche => x < edge,
            Cote::Droite => fx >= w as f32 - bord_x,
            Cote::Haut => fy < bord_y,
            Cote::Bas => fy >= h as f32 - bord_y,
        }
    };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let x = i % w.max(1);
        let y = i / w.max(1);
        let mut base = if mix.glisse {
            glisse(b, mix.shift_b, x, y).or_else(|| glisse(a, mix.shift_a, x, y)).unwrap_or(Color32::TRANSPARENT)
        } else if let Some([za, zb]) = mix.zoom {
            lerp_px(grossi(a, za, x, y), grossi(b, zb, x, y), m)
        } else if let Some(balaye) = mix.horloge {
            let (dx, dy) = (x as f32 + 0.5 - w as f32 / 2.0, y as f32 + 0.5 - h as f32 / 2.0);
            let mut angle = dx.atan2(-dy);
            if angle < 0.0 {
                angle += std::f32::consts::TAU;
            }
            if angle < balaye {
                pick(b, i)
            } else {
                pick(a, i)
            }
        } else if let Some(c) = mix.objet {
            if (x as f32 + 0.5) / (w as f32) < c {
                pick(a, i)
            } else {
                pick(b, i)
            }
        } else {
            match (mix.wipe, a.is_some(), b.is_some()) {
                (true, true, true) => {
                    if volet_b(x, y) {
                        pick(b, i)
                    } else {
                        pick(a, i)
                    }
                }
                (_, true, true) => lerp_px(pick(a, i), pick(b, i), m),
                (_, true, false) => pick(a, i),
                (_, false, true) => pick(b, i),
                (_, false, false) => Color32::TRANSPARENT,
            }
        };
        if let Some(c) = mix.objet {
            let part = objet_couvre(c, x, y, w, h);
            if part > 0.0 {
                base = lerp_px(base, objet, weight(part));
            }
        }
        // L'iris : hors du rond (au bord lissé sur un pixel), la couleur.
        if let Some([cx, cy, r]) = mix.iris {
            let d = (x as f32 + 0.5 - cx * w as f32).hypot(y as f32 + 0.5 - cy * h as f32);
            let dehors = (d - r * w as f32 + 0.5).clamp(0.0, 1.0);
            if dehors > 0.0 {
                base = lerp_px(base, layer, weight(dehors));
            }
        }
        out.push(if k == 0 { base } else { lerp_px(base, layer, k) });
    }
    out
}

// ------------------------------------------------------------------------------- opérations

/// Les deux plans de part et d'autre de la coupe `cut` (0 : début du film, `panels.len()` : fin).
pub fn cut_plans(doc: &Document, cut: usize) -> (Option<u64>, Option<u64>) {
    let out = cut.checked_sub(1).and_then(|i| doc.panels.get(i)).map(|p| p.id);
    let inn = doc.panels.get(cut).map(|p| p.id);
    (out, inn)
}

impl Document {
    /// La transition posée sur cette coupe, s'il y en a une.
    pub fn transition_on(&self, out: Option<u64>, inn: Option<u64>) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.out == out && t.inn == inn)
    }

    pub fn transition(&self, id: u64) -> Option<&Transition> {
        self.transitions.iter().find(|t| t.id == id)
    }

    /// La coupe (0 à `panels.len()`) la plus proche de l'image `g` du film.
    pub fn nearest_cut(&self, g: usize) -> usize {
        let mut best = (0, usize::MAX);
        let mut at = 0usize;
        for c in 0..=self.panels.len() {
            let d = at.abs_diff(g);
            if d < best.1 {
                best = (c, d);
            }
            if let Some(p) = self.panels.get(c) {
                at += p.frame_count();
            }
        }
        best.0
    }

    /// Pose une transition de sorte `kind` sur la coupe `cut`, ou change celle qui y est (sa durée
    /// et son calage restent). Renvoie son numéro ; rien si la sorte n'y est pas permise ou si la
    /// coupe n'existe pas.
    pub fn set_transition(&mut self, cut: usize, kind: Kind) -> Option<u64> {
        if cut > self.panels.len() {
            return None;
        }
        let (out, inn) = cut_plans(self, cut);
        if !kind.allowed(out.is_some() && inn.is_some()) {
            return None;
        }
        if let Some(t) = self.transitions.iter_mut().find(|t| t.out == out && t.inn == inn) {
            t.kind = kind;
            return Some(t.id);
        }
        let id = self.alloc();
        self.transitions.push(Transition { id, out, inn, kind, frames: kind.default_frames(), align: Align::Centre, couleur: None, depuis: None, centre: None });
        Some(id)
    }

    pub fn remove_transition(&mut self, id: u64) -> bool {
        let n = self.transitions.len();
        self.transitions.retain(|t| t.id != id);
        self.transitions.len() != n
    }

    /// Les transitions dont un plan n'existe plus disparaissent (avec leur plan ; l'annulation les
    /// ramène avec lui, `Structure`).
    pub(crate) fn prune_transitions(&mut self) {
        if self.transitions.is_empty() {
            return;
        }
        let ids: HashSet<u64> = self.panels.iter().map(|p| p.id).collect();
        self.transitions.retain(|t| t.out.is_none_or(|o| ids.contains(&o)) && t.inn.is_none_or(|i| ids.contains(&i)));
    }

    /// Le plan `neuf` vient d'être inséré (plan vide, plan type glissé sur une coupe). La transition de la coupe où il arrive reste à
    /// la sortie du plan d'avant : elle passe sur la coupe d'entrée du plan neuf, au lieu de rester dans le projet sans plus se montrer
    /// nulle part (audit du 2026-11-21). Le fondu d'ouverture reste en tête du film, le fondu de fermeture à sa fin. Si la coupe visée
    /// a déjà sa transition, elle attend, comme avant. Un plan déplacé, dupliqué ou collé là ne passe pas par ici (`Document::inserer_un_plan_neuf`).
    /// Renvoie le nombre de transitions rattachées.
    pub(crate) fn rattacher_transitions(&mut self, neuf: u64) -> usize {
        let Some(x) = self.panels.iter().position(|p| p.id == neuf) else { return 0 };
        let avant = x.checked_sub(1).map(|i| self.panels[i].id);
        let apres = self.panels.get(x + 1).map(|p| p.id);
        // (La coupe d'avant le plan neuf, la coupe d'après : ce qu'elles deviennent si la transition y va.)
        let libre = |ts: &[Transition], out: Option<u64>, inn: Option<u64>| !ts.iter().any(|t| t.out == out && t.inn == inn);
        let cible = match (avant, apres) {
            // Entre deux plans : la transition de leur coupe passe à l'entrée du plan neuf.
            (Some(_), Some(_)) => Some(((avant, apres), (avant, Some(neuf)))),
            // En tête du film : le fondu d'ouverture reste au début du film.
            (None, Some(_)) => Some(((None, apres), (None, Some(neuf)))),
            // En fin de film : le fondu de fermeture reste à la fin du film.
            (Some(_), None) => Some(((avant, None), (Some(neuf), None))),
            (None, None) => None,
        };
        let Some((de, vers)) = cible else { return 0 };
        if !libre(&self.transitions, vers.0, vers.1) {
            return 0;
        }
        match self.transitions.iter_mut().find(|t| t.out == de.0 && t.inn == de.1) {
            Some(t) => {
                (t.out, t.inn) = vers;
                1
            }
            None => 0,
        }
    }

    /// Ce que montre l'image `g` du film (voir `mix_in`) ; rien sans transition.
    pub fn mix_at(&self, g: usize) -> Option<(Zone, Mix)> {
        if self.transitions.is_empty() {
            return None;
        }
        let zones = zones(self);
        mix_in(&zones, g).map(|(z, m)| (*z, m))
    }
}

// --------------------------------------------------------------------------------- fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default)]
    transitions: Vec<Transition>,
}

/// Le contenu de `transitions.json` pour `doc` ; rien s'il n'y a aucune transition (le fichier est
/// alors retiré). Celles d'un plan qui n'existe plus ne sont pas écrites.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let ids: HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
    let order = |t: &Transition| t.inn.or(t.out).and_then(|id| doc.panels.iter().position(|p| p.id == id)).unwrap_or(usize::MAX);
    let mut list: Vec<Transition> = doc
        .transitions
        .iter()
        .filter(|t| t.out.is_none_or(|o| ids.contains(&o)) && t.inn.is_none_or(|i| ids.contains(&i)))
        .cloned()
        .collect();
    if list.is_empty() {
        return Ok(None);
    }
    // Dans l'ordre du film : le fichier se lit, et deux enregistrements se ressemblent.
    list.sort_by_key(|t| (order(t), t.id));
    // (Sans sorte du lot 4 ni couleur, la version 1 : une version précédente de FEANOR le lit encore.)
    let version = if list.iter().any(|t| t.kind.de_ses_reponses() || t.depuis.is_some() || t.centre.is_some()) {
        VERSION
    } else if list.iter().any(|t| t.kind.du_lot_4() || t.couleur.is_some()) {
        VERSION_LOT4
    } else {
        VERSION_SANS_LOT4
    };
    let sidecar = Sidecar { format: FORMAT.into(), version, transitions: list };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les transitions lues dans le texte de `transitions.json`. Numéros uniques et non nuls (un double
/// en reçoit un neuf), durées bornées, une seule transition par coupe, aucune sans plan.
pub(crate) fn parse(text: &str) -> Result<Vec<Transition>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de transitions".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.transitions.iter().map(|t| t.id).max().unwrap_or(0) + 1;
    let mut seen = HashSet::new();
    let mut cuts = HashSet::new();
    let mut out = Vec::with_capacity(sidecar.transitions.len());
    for mut t in sidecar.transitions {
        if (t.out.is_none() && t.inn.is_none()) || !cuts.insert((t.out, t.inn)) {
            continue;
        }
        if t.id == 0 || !seen.insert(t.id) {
            t.id = next;
            next += 1;
            seen.insert(t.id);
        }
        t.frames = t.frames.clamp(1, MAX_FRAMES);
        if !t.kind.allowed(t.between_plans()) {
            t.kind = Kind::Black;
        }
        // (Partie 177.) Un centre abîmé revient au milieu du cadre.
        t.centre = t.centre.filter(|c| c.iter().all(|v| v.is_finite())).map(|c| c.map(|v| v.clamp(0.0, 1.0)));
        out.push(t);
    }
    Ok(out)
}
