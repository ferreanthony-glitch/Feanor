//! Effets de caméra en préréglages : des clés de caméra posées toutes seules (tremblements,
//! explosion, zooms), à partir de la caméra telle qu'elle est (Anthony, 2026-10-18 : « ajouter des
//! effets de caméra en préréglage qui posent des clés automatiquement : tremblement de caméra
//! type found footage, explosions, zoom »).
//!
//! Un préréglage n'invente aucune notion nouvelle : il écrit des clés de caméra ordinaires, que
//! l'on déplace, copie, supprime ou règle ensuite comme les autres. Il part de la caméra de
//! chaque image (`base`, avec ses clés et ses adoucis) et lui ajoute un décalage : il ne remet
//! rien à zéro. Les valeurs d'amplitude, de fréquence et de durée sont des choix de SHOT, à régler
//! à l'œil.
//!
//! Tout est déterministe : même préréglage, mêmes réglages, même graine, mêmes clés. Le bruit est
//! une somme de sinus aux phases tirées de la graine (lisse, borné par l'intensité) ; les
//! secousses sont des oscillations amorties. Un tremblement continu part de zéro et y revient
//! (la dernière clé est neutre) ; l'explosion et la secousse commencent d'un coup et s'éteignent.
//! Les décalages se comptent en part de la largeur du cadre filmé (le tremblement a donc le même
//! effet à tous les zooms), les rotations en degrés, les zooms en facteur de la focale.
//!
//! Les fonctions sont pures (`Generator`, `plan`, `write`) ; `camfxui.rs` les branche sur le
//! document, l'historique et le panneau de la caméra.
//!
//! Le lot 2 de la liste des préréglages (partie 171, Anthony, 2026-11-18 : « Oui, le lot 2 ») ajoute la caméra 2D : dézoom
//! choc, zooms par paliers, de tension et vers un point, travellings avant et arrière, Ken Burns, panoramiques (à plat, zip
//! pan, avec dépassement, de découverte, en rotation), cadre penché, tonneau, tremblement de terre, pas lourds, marche,
//! véhicule, houle, respiration, recadrage sec, arrêt net. Ceux qui posent peu de clés suivent une courbe (`courbe_de`) aux
//! adoucis de clés ordinaires ; ceux qui vont quelque part ont une flèche ou une cible (`Params::vers`), tirée dans le cadre
//! avec l'outil Caméra (`effetpoignee.rs`).

use crate::camera::{self, Camera, CameraKey};

/// Les préréglages proposés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Caméra à l'épaule (found footage) : dérive lente au hasard, avec de petites secousses.
    Shoulder,
    /// Tremblement léger et continu.
    LightShake,
    /// Explosion / impact : forte secousse qui s'éteint, avec un coup de zoom en option.
    Explosion,
    /// Secousse rapide : un à-coup court.
    Jolt,
    /// Zoom avant, adouci au départ et à l'arrivée.
    ZoomIn,
    /// Zoom arrière, adouci.
    ZoomOut,
    /// Zoom choc (crash zoom) : un zoom avant brutal qui ralentit en arrivant.
    CrashZoom,
    /// Vertigo (dolly zoom) : la focale change, la caméra recule en même temps, le sujet (au plan
    /// de référence) garde sa taille et le fond se déforme.
    Vertigo,
    // Lot 2 de la liste des préréglages (partie 171, 2026-11-18) : ses numéros entre parenthèses.
    /// Dézoom choc (C1) : on part serré sur la cible, le cadre s'ouvre d'un coup.
    DezoomChoc,
    /// Zoom par paliers (C2) : des à-coups secs vers la cible, tenus entre deux.
    ZoomPaliers,
    /// Zoom lent de tension (C3).
    ZoomTension,
    /// Zoom vers un point (C4) : la cible arrive au centre du cadre.
    ZoomPoint,
    /// Travelling avant (C5) : la caméra avance (la distance change, pas la focale).
    TravellingAvant,
    /// Travelling arrière (C5).
    TravellingArriere,
    /// Ken Burns (C6) : glissement et zoom lents, réguliers.
    KenBurns,
    /// Panoramique à plat (C7) : la caméra glisse dans le sens de la flèche.
    PanoPlat,
    /// Zip pan (C8) : panoramique éclair.
    ZipPan,
    /// Panoramique avec dépassement (C9).
    PanoDepassement,
    /// Panoramique de découverte (C10) : la caméra part du bout de la flèche et vient se poser.
    PanoDecouverte,
    /// Panoramique en rotation (C12) : la caméra tourne sur elle-même, en perspective.
    PanoRotation,
    /// Plan penché progressif (C18).
    Penche,
    /// Tonneau (C19) : un tour complet qui accélère.
    Tonneau,
    /// Tremblement de terre (C20).
    Seisme,
    /// Pas lourds (C21).
    PasLourds,
    /// Caméra qui marche (C22).
    Marche,
    /// Véhicule (C23).
    Vehicule,
    /// Houle (C24).
    Houle,
    /// Respiration (C25).
    Respiration,
    /// Recadrage sec (C26) : un saut de cadre sur une seule image.
    Recadrage,
    /// Arrêt net (C27) : la caméra glisse à vitesse constante et se fige.
    ArretNet,
}

/// Les familles du panneau Préréglages et des listes (les titres de la page de choix du 18 novembre 2026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Famille {
    Zooms,
    Panoramiques,
    Rotations,
    Secousses,
    Coupes,
}

impl Famille {
    pub const ALL: [Famille; 5] = [Famille::Zooms, Famille::Panoramiques, Famille::Rotations, Famille::Secousses, Famille::Coupes];

    pub fn label(self) -> &'static str {
        match self {
            Famille::Zooms => "Zooms et avancées",
            Famille::Panoramiques => "Panoramiques",
            Famille::Rotations => "Rotations",
            Famille::Secousses => "Secousses",
            Famille::Coupes => "Coupes dans le plan",
        }
    }

    /// Les préréglages de la famille, dans l'ordre de `Preset::ALL`.
    pub fn presets(self) -> impl Iterator<Item = Preset> {
        Preset::ALL.into_iter().filter(move |p| p.famille() == self)
    }
}

/// Ce que l'outil Caméra montre dans le cadre pour régler `Params::vers` (partie 171) : rien, une flèche (le chemin de la
/// caméra), une flèche qui revient (d'où part la caméra) ou une cible (où va la caméra).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poignee {
    Aucune,
    /// La caméra va du centre du cadre au bout de la flèche.
    Fleche,
    /// La caméra part du bout de la flèche et revient au cadre du plan.
    Depart,
    /// La caméra va vers ce point de l'image.
    Cible,
}

impl Preset {
    /// Tous les préréglages, rangés par famille (`Famille`).
    pub const ALL: [Preset; 30] = [
        Preset::ZoomIn,
        Preset::ZoomOut,
        Preset::CrashZoom,
        Preset::DezoomChoc,
        Preset::ZoomPaliers,
        Preset::ZoomTension,
        Preset::ZoomPoint,
        Preset::TravellingAvant,
        Preset::TravellingArriere,
        Preset::Vertigo,
        Preset::KenBurns,
        Preset::PanoPlat,
        Preset::ZipPan,
        Preset::PanoDepassement,
        Preset::PanoDecouverte,
        Preset::PanoRotation,
        Preset::Penche,
        Preset::Tonneau,
        Preset::Shoulder,
        Preset::LightShake,
        Preset::Explosion,
        Preset::Jolt,
        Preset::Seisme,
        Preset::PasLourds,
        Preset::Marche,
        Preset::Vehicule,
        Preset::Houle,
        Preset::Respiration,
        Preset::Recadrage,
        Preset::ArretNet,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Shoulder => "Caméra à l'épaule (found footage)",
            Preset::LightShake => "Tremblement léger",
            Preset::Explosion => "Explosion / impact",
            Preset::Jolt => "Secousse rapide",
            Preset::ZoomIn => "Zoom avant",
            Preset::ZoomOut => "Zoom arrière",
            Preset::CrashZoom => "Zoom choc (crash zoom)",
            Preset::Vertigo => "Vertigo (dolly zoom)",
            Preset::DezoomChoc => "Dézoom choc",
            Preset::ZoomPaliers => "Zoom par paliers",
            Preset::ZoomTension => "Zoom lent de tension",
            Preset::ZoomPoint => "Zoom vers un point",
            Preset::TravellingAvant => "Travelling avant",
            Preset::TravellingArriere => "Travelling arrière",
            Preset::KenBurns => "Ken Burns",
            Preset::PanoPlat => "Panoramique à plat",
            Preset::ZipPan => "Zip pan",
            Preset::PanoDepassement => "Panoramique avec dépassement",
            Preset::PanoDecouverte => "Panoramique de découverte",
            Preset::PanoRotation => "Panoramique en rotation",
            Preset::Penche => "Plan penché progressif",
            Preset::Tonneau => "Tonneau",
            Preset::Seisme => "Tremblement de terre",
            Preset::PasLourds => "Pas lourds",
            Preset::Marche => "Caméra qui marche",
            Preset::Vehicule => "Véhicule",
            Preset::Houle => "Houle",
            Preset::Respiration => "Respiration",
            Preset::Recadrage => "Recadrage sec",
            Preset::ArretNet => "Arrêt net",
        }
    }

    /// Sa famille (le panneau Préréglages et les listes les rangent ainsi).
    pub fn famille(self) -> Famille {
        use Preset::*;
        match self {
            ZoomIn | ZoomOut | CrashZoom | DezoomChoc | ZoomPaliers | ZoomTension | ZoomPoint | TravellingAvant | TravellingArriere | Vertigo | KenBurns => Famille::Zooms,
            PanoPlat | ZipPan | PanoDepassement | PanoDecouverte | PanoRotation => Famille::Panoramiques,
            Penche | Tonneau => Famille::Rotations,
            Shoulder | LightShake | Explosion | Jolt | Seisme | PasLourds | Marche | Vehicule | Houle | Respiration => Famille::Secousses,
            Recadrage | ArretNet => Famille::Coupes,
        }
    }

    /// Ce que l'outil Caméra montre dans le cadre pour régler `Params::vers`.
    pub fn poignee(self) -> Poignee {
        use Preset::*;
        match self {
            DezoomChoc | ZoomPaliers | ZoomTension | ZoomPoint | Recadrage => Poignee::Cible,
            KenBurns | PanoPlat | ZipPan | PanoDepassement | Vehicule | ArretNet => Poignee::Fleche,
            PanoDecouverte => Poignee::Depart,
            _ => Poignee::Aucune,
        }
    }

    /// Se règle par une intensité et une fréquence (les secousses).
    pub fn a_intensite(self) -> bool {
        self.famille() == Famille::Secousses
    }

    /// Tire des nombres au hasard (graine).
    pub fn a_graine(self) -> bool {
        matches!(self, Preset::Shoulder | Preset::LightShake | Preset::Explosion | Preset::Jolt | Preset::Seisme | Preset::Vehicule | Preset::Respiration)
    }

    /// Se règle par un facteur (zoom, ou avancée de la caméra : `est_une_avancee`).
    pub fn a_zoom(self) -> bool {
        use Preset::*;
        matches!(self, ZoomIn | ZoomOut | CrashZoom | Vertigo | DezoomChoc | ZoomPaliers | ZoomTension | ZoomPoint | TravellingAvant | TravellingArriere | KenBurns | Tonneau | Recadrage | Marche)
    }

    /// Le facteur est celui de l'avancée de la caméra (le sujet grandit d'autant), pas de la focale.
    pub fn est_une_avancee(self) -> bool {
        matches!(self, Preset::TravellingAvant | Preset::TravellingArriere | Preset::Marche)
    }

    /// Se règle par un angle (`Params::degres`).
    pub fn a_degres(self) -> bool {
        matches!(self, Preset::PanoRotation | Preset::Penche | Preset::Tonneau)
    }

    /// Se règle par un nombre de paliers.
    pub fn a_paliers(self) -> bool {
        self == Preset::ZoomPaliers
    }

    /// Le nom de la fréquence dans le panneau Caméra.
    pub fn nom_frequence(self) -> &'static str {
        match self {
            Preset::Shoulder => "Vitesse",
            Preset::PasLourds | Preset::Marche => "Pas",
            Preset::Houle | Preset::Respiration => "Rythme",
            _ => "Fréquence",
        }
    }

    /// Ce que dit la fréquence, en une phrase (bulle du champ).
    pub fn note_frequence(self) -> &'static str {
        match self {
            Preset::Shoulder => "Vitesse de la dérive : plus haut, la caméra bouge plus vite.",
            Preset::PasLourds | Preset::Marche => "Pas par seconde.",
            Preset::Houle => "Vagues par seconde : 0,3 Hz, une vague toutes les 3 secondes.",
            Preset::Respiration => "Souffles par seconde : 0,25 Hz, un souffle toutes les 4 secondes.",
            Preset::Seisme => "Vitesse du grondement.",
            Preset::Vehicule => "Vitesse de la vibration du moteur.",
            _ => "Vitesse des à-coups, en secousses par seconde.",
        }
    }

    /// Le nom rangé dans le projet (format 52) : stable, sans accent, indépendant du libellé.
    pub fn nom(self) -> &'static str {
        match self {
            Preset::Shoulder => "epaule",
            Preset::LightShake => "tremblement",
            Preset::Explosion => "explosion",
            Preset::Jolt => "secousse",
            Preset::ZoomIn => "zoom_avant",
            Preset::ZoomOut => "zoom_arriere",
            Preset::CrashZoom => "zoom_choc",
            Preset::Vertigo => "vertigo",
            // (Format 58, partie 171.)
            Preset::DezoomChoc => "dezoom_choc",
            Preset::ZoomPaliers => "zoom_paliers",
            Preset::ZoomTension => "zoom_tension",
            Preset::ZoomPoint => "zoom_point",
            Preset::TravellingAvant => "travelling_avant",
            Preset::TravellingArriere => "travelling_arriere",
            Preset::KenBurns => "ken_burns",
            Preset::PanoPlat => "pano_plat",
            Preset::ZipPan => "zip_pan",
            Preset::PanoDepassement => "pano_depassement",
            Preset::PanoDecouverte => "pano_decouverte",
            Preset::PanoRotation => "pano_rotation",
            Preset::Penche => "penche",
            Preset::Tonneau => "tonneau",
            Preset::Seisme => "seisme",
            Preset::PasLourds => "pas_lourds",
            Preset::Marche => "marche",
            Preset::Vehicule => "vehicule",
            Preset::Houle => "houle",
            Preset::Respiration => "respiration",
            Preset::Recadrage => "recadrage",
            Preset::ArretNet => "arret_net",
        }
    }

    /// Le préréglage de ce nom (`nom`) ; rien pour un nom inconnu.
    pub fn du_nom(nom: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.nom() == nom)
    }

    /// Ce que fait le préréglage, en une phrase (bulle).
    pub fn note(self) -> &'static str {
        match self {
            Preset::Shoulder => "Dérive lente et irrégulière de la position et de la rotation, avec de petites secousses de temps en temps. Une clé toutes les 2 images.",
            Preset::LightShake => "Tremblement fin et continu, à peine visible. Une clé par image.",
            Preset::Explosion => "Forte secousse qui s'éteint en décroissance exponentielle, avec un coup de zoom en option. Une clé par image.",
            Preset::Jolt => "Un à-coup court, qui s'éteint vite. Une clé par image.",
            Preset::ZoomIn => "Resserre le cadre (la focale monte), doucement au départ et à l'arrivée. Deux clés.",
            Preset::ZoomOut => "Élargit le cadre (la focale baisse), doucement au départ et à l'arrivée. Deux clés.",
            Preset::CrashZoom => "Zoom avant brutal qui ralentit en arrivant. Deux clés.",
            Preset::Vertigo => "La focale monte et la caméra recule d'autant : le sujet au plan de référence garde sa taille, les calques en profondeur bougent. Deux douzaines de clés.",
            Preset::DezoomChoc => "On part serré sur la cible, le cadre s'ouvre d'un coup et ralentit en arrivant. La cible se tire dans le cadre avec l'outil Caméra. Deux clés.",
            Preset::ZoomPaliers => "Des à-coups secs vers la cible, tenus entre deux, façon anime. Deux clés par palier.",
            Preset::ZoomTension => "Un zoom presque imperceptible vers la cible, sur toute la durée. Deux clés.",
            Preset::ZoomPoint => "La caméra va vers la cible en zoomant : la cible arrive au centre du cadre. La cible se tire dans le cadre avec l'outil Caméra. Deux clés.",
            Preset::TravellingAvant => "La caméra avance au lieu de zoomer : ce qui est près grandit vite, le lointain à peine (calques en profondeur). Deux clés.",
            Preset::TravellingArriere => "La caméra recule au lieu de dézoomer : ce qui est près rapetisse vite, le lointain à peine. Deux clés.",
            Preset::KenBurns => "Glissement et zoom lents et réguliers, dans le sens de la flèche : une image fixe qui vit. Deux clés.",
            Preset::PanoPlat => "La caméra glisse sur le décor dans le sens de la flèche, doucement au départ et à l'arrivée. La flèche se tire dans le cadre avec l'outil Caméra. Deux clés.",
            Preset::ZipPan => "Panoramique éclair dans le sens de la flèche, qui se pose avec un petit rebond. Trois clés.",
            Preset::PanoDepassement => "La caméra va un peu plus loin que la flèche, puis revient se poser. Trois clés.",
            Preset::PanoDecouverte => "La caméra part du bout de la flèche (le ciel, par exemple) et vient se poser sur le cadre du plan en ralentissant. Deux clés.",
            Preset::PanoRotation => "La caméra tourne sur elle-même, en perspective, de l'angle réglé (vers la droite s'il est positif). Deux clés.",
            Preset::Penche => "Le cadre penche peu à peu jusqu'à l'angle réglé, puis le garde. Deux clés.",
            Preset::Tonneau => "Le cadre fait un tour complet qui accélère, en zoomant un peu. Deux clés.",
            Preset::Seisme => "Long et grave : il monte, gronde, puis s'éteint. Une clé par image.",
            Preset::PasLourds => "Des secousses verticales amorties, une à chaque pas : un géant marche. Une clé par image.",
            Preset::Marche => "La caméra avance avec le balancement régulier de la marche. Une clé toutes les 2 images.",
            Preset::Vehicule => "Vibration fine, une bosse de temps en temps, et la caméra glisse dans le sens de la flèche. Une clé par image.",
            Preset::Houle => "Roulis lent d'un bateau, qui monte et descend. Une clé toutes les 2 images.",
            Preset::Respiration => "Une dérive infime, à peine un souffle, pour qu'un plan fixe ne soit pas mort. Une clé toutes les 4 images.",
            Preset::Recadrage => "Un saut de cadre sur une seule image, vers la cible, dans le même plan. Deux clés, et une calme juste avant.",
            Preset::ArretNet => "La caméra glisse à vitesse constante dans le sens de la flèche et se fige d'un coup. Deux clés.",
        }
    }

    /// Un tremblement ou une secousse d'avant le lot 2 (intensité, fréquence, graine) : ceux que le préréglage d'effet
    /// « Tremblement de l'image » propose aussi.
    pub fn is_shake(self) -> bool {
        matches!(self, Preset::Shoulder | Preset::LightShake | Preset::Explosion | Preset::Jolt)
    }

    /// Commence d'un coup (la caméra est calme juste avant) plutôt que de monter doucement.
    pub fn is_abrupt(self) -> bool {
        matches!(self, Preset::Explosion | Preset::Jolt | Preset::CrashZoom | Preset::DezoomChoc | Preset::PanoDecouverte | Preset::Recadrage | Preset::PasLourds)
    }

    /// Durée proposée, en secondes.
    pub fn default_seconds(self) -> f32 {
        match self {
            Preset::Shoulder => 4.0,
            Preset::LightShake => 3.0,
            Preset::Explosion => 1.2,
            Preset::Jolt => 0.35,
            Preset::ZoomIn | Preset::ZoomOut => 2.0,
            Preset::CrashZoom => 0.3,
            Preset::Vertigo => 3.0,
            Preset::DezoomChoc => 0.6,
            Preset::ZoomPaliers => 2.4,
            Preset::ZoomTension => 5.0,
            Preset::ZoomPoint => 2.6,
            Preset::TravellingAvant | Preset::TravellingArriere => 3.0,
            Preset::KenBurns => 6.0,
            Preset::PanoPlat => 3.0,
            Preset::ZipPan => 0.6,
            Preset::PanoDepassement => 2.2,
            Preset::PanoDecouverte => 3.2,
            Preset::PanoRotation | Preset::Penche => 3.0,
            Preset::Tonneau => 2.4,
            Preset::Seisme => 4.0,
            Preset::PasLourds | Preset::Marche | Preset::Vehicule => 3.0,
            Preset::Houle => 6.0,
            Preset::Respiration => 8.0,
            Preset::Recadrage => 1.0,
            Preset::ArretNet => 2.0,
        }
    }

    /// Fréquence proposée, en Hz (de la dérive pour la caméra à l'épaule, du tremblement sinon).
    pub fn default_freq(self) -> f32 {
        match self {
            Preset::Shoulder => 0.9,
            Preset::LightShake => 7.0,
            Preset::Explosion => 12.0,
            Preset::Jolt => 10.0,
            Preset::Seisme => 6.0,
            Preset::PasLourds => 1.3,
            Preset::Marche => 1.8,
            Preset::Vehicule => 9.0,
            Preset::Houle => 0.3,
            Preset::Respiration => 0.25,
            _ => 0.0,
        }
    }

    /// Facteur de zoom proposé (focale d'arrivée / focale de départ), ou d'avancée (`est_une_avancee`).
    pub fn default_factor(self) -> f32 {
        match self {
            Preset::ZoomIn | Preset::ZoomOut => 1.5,
            Preset::CrashZoom => 3.0,
            Preset::Vertigo => 2.0,
            Preset::DezoomChoc => 2.6,
            Preset::ZoomPaliers => 2.5,
            Preset::ZoomTension => 1.3,
            Preset::ZoomPoint => 3.0,
            Preset::TravellingAvant | Preset::TravellingArriere => 1.6,
            Preset::KenBurns | Preset::Tonneau | Preset::Marche => 1.25,
            Preset::Recadrage => 2.0,
            _ => 1.0,
        }
    }

    /// Angle proposé, en degrés (`a_degres`).
    pub fn default_degres(self) -> f32 {
        match self {
            Preset::PanoRotation => 35.0,
            Preset::Penche => 14.0,
            Preset::Tonneau => 360.0,
            _ => 0.0,
        }
    }

    /// Flèche ou cible proposée (`Params::vers`), en part de la largeur du cadre depuis son centre (x vers la droite, y vers
    /// le bas).
    pub fn default_vers(self) -> [f32; 2] {
        match self {
            Preset::ZoomPoint => [0.22, -0.08],
            Preset::KenBurns => [0.1, -0.03],
            Preset::PanoPlat => [0.6, 0.0],
            Preset::ZipPan => [1.0, 0.0],
            Preset::PanoDepassement => [0.5, 0.0],
            Preset::PanoDecouverte => [0.0, -0.45],
            Preset::Vehicule => [0.3, 0.0],
            Preset::ArretNet => [0.4, 0.0],
            _ => [0.0, 0.0],
        }
    }

    /// Écart entre deux clés, en images, avant le plafond du nombre de clés (`usize::MAX` : les clés de sa courbe, ou les
    /// deux du début et de la fin).
    fn step(self) -> usize {
        match self {
            Preset::Shoulder | Preset::Marche | Preset::Houle => 2,
            Preset::LightShake | Preset::Explosion | Preset::Jolt | Preset::Seisme | Preset::PasLourds | Preset::Vehicule => 1,
            Preset::Respiration => 4,
            _ => usize::MAX,
        }
    }
}

/// Les réglages d'un effet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    pub preset: Preset,
    /// 1 = 100 % : la force de départ du préréglage (tremblements).
    pub intensity: f32,
    /// Images entre la première et la dernière clé.
    pub duration: usize,
    /// Fréquence, en Hz (tremblements).
    pub freq: f32,
    /// Graine du hasard (tremblements).
    pub seed: u32,
    /// Focale d'arrivée / focale de départ (zooms, vertigo) ; au moins 1.
    pub zoom_factor: f32,
    /// Coup de zoom avec l'explosion.
    pub punch: bool,
    /// La flèche ou la cible (`Preset::poignee`), en part de la largeur du cadre depuis son centre, dans l'image (x vers la
    /// droite, y vers le bas) : le chemin de la caméra, son point de départ ou le point qu'elle vise (partie 171).
    pub vers: [f32; 2],
    /// Angle, en degrés (`Preset::a_degres`) : du panoramique en rotation (positif : vers la droite), du cadre penché ou du
    /// tonneau.
    pub degres: f32,
    /// Nombre d'à-coups du zoom par paliers.
    pub paliers: u32,
}

/// Plafond du nombre de clés posées d'un coup (un effet très long espace ses clés).
pub const MAX_KEYS: usize = 1000;

/// Nombre de paliers proposé (zoom par paliers), et le plus grand.
pub const PALIERS: u32 = 3;
pub const PALIERS_MAX: u32 = 12;

impl Params {
    /// Les réglages proposés pour `preset` à `fps` images par seconde.
    pub fn new(preset: Preset, fps: u32) -> Params {
        Params {
            preset,
            intensity: 1.0,
            duration: ((preset.default_seconds() * fps as f32).round() as usize).max(1),
            freq: preset.default_freq(),
            seed: 1,
            zoom_factor: preset.default_factor(),
            punch: preset == Preset::Explosion,
            vers: preset.default_vers(),
            degres: preset.default_degres(),
            paliers: PALIERS,
        }
    }

    /// Les mêmes réglages pour un autre préréglage : durée, fréquence, facteur, flèche et angle repartent de ses valeurs,
    /// l'intensité et la graine restent.
    pub fn with_preset(&self, preset: Preset, fps: u32) -> Params {
        Params { intensity: self.intensity, seed: self.seed, ..Params::new(preset, fps) }
    }

    /// Réglages ramenés dans des bornes sûres.
    pub fn sanitized(&self, fps: u32) -> Params {
        let ok = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        let d = self.preset.default_vers();
        Params {
            intensity: ok(self.intensity, 0.0, 10.0, 1.0),
            duration: self.duration.clamp(1, 100_000),
            freq: ok(self.freq, 0.0, (fps as f32 / 2.0).max(1.0), self.preset.default_freq()),
            zoom_factor: ok(self.zoom_factor, 1.0, 20.0, self.preset.default_factor().max(1.0)),
            vers: [ok(self.vers[0], -VERS_MAX, VERS_MAX, d[0]), ok(self.vers[1], -VERS_MAX, VERS_MAX, d[1])],
            degres: ok(self.degres, -DEGRES_MAX, DEGRES_MAX, self.preset.default_degres()),
            paliers: self.paliers.clamp(1, PALIERS_MAX),
            ..*self
        }
    }
}

/// Bornes de la flèche (en largeurs de cadre) et de l'angle (en degrés : trois tours).
pub const VERS_MAX: f32 = 4.0;
pub const DEGRES_MAX: f32 = 1080.0;

/// Une clé d'effet : le décalage ajouté à la caméra de base à l'image `frame` (comptée depuis le
/// début de l'effet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub frame: usize,
    /// Décalages de la position, en part de la largeur du cadre filmé (x vers la droite, y vers le bas).
    pub dx: f32,
    pub dy: f32,
    /// Rotation ajoutée, en degrés.
    pub angle: f32,
    /// Facteur de la focale (1 : inchangée).
    pub zoom: f32,
    /// Adoucis à poser sur la clé (images), pour les zooms.
    pub ease_in: f32,
    pub ease_out: f32,
    /// Panoramique ajouté, en degrés (la caméra tourne sur elle-même, en perspective).
    pub pan: f32,
    /// Avancée de la caméra : sa distance au plan de référence est divisée par ce facteur (1 : inchangée ; plus grand : elle
    /// avance, le plan de référence grandit d'autant).
    pub avance: f32,
}

/// Les bornes de ce qu'un effet peut ajouter à la caméra.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// Décalage de position, en part de la largeur du cadre filmé (par axe).
    pub pos: f32,
    /// Rotation, en degrés.
    pub angle: f32,
    /// Coup de zoom : le facteur reste dans 1 ± cette part.
    pub punch: f32,
}

// ---------------------------------------------------------------------- hasard et bruit

/// Générateur de nombres (splitmix64) : déterministe, sans dépendance.
struct Rng(u64);

impl Rng {
    fn new(seed: u32, salt: u64) -> Rng {
        Rng(((seed as u64) << 20) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Dans [0, 1[.
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

const TAU: f32 = std::f32::consts::TAU;

/// Bruit lisse dans [-1, 1] : trois sinus de fréquences voisines, aux phases tirées au sort.
#[derive(Clone, Copy, Debug)]
struct Wave {
    phase: [f32; 3],
}

impl Wave {
    const RATIO: [f32; 3] = [1.0, 1.93, 3.31];
    const WEIGHT: [f32; 3] = [1.0, 0.55, 0.3];

    fn new(rng: &mut Rng) -> Wave {
        Wave { phase: [rng.range(0.0, TAU), rng.range(0.0, TAU), rng.range(0.0, TAU)] }
    }

    fn at(&self, freq: f32, t: f32) -> f32 {
        let sum: f32 = Self::WEIGHT.iter().sum();
        (0..3).map(|k| Self::WEIGHT[k] * (TAU * freq * Self::RATIO[k] * t + self.phase[k]).sin()).sum::<f32>() / sum
    }
}

/// 0 en 0, 1 en 1, sans à-coup aux deux bouts.
fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Une secousse de la caméra à l'épaule.
#[derive(Clone, Copy, Debug)]
struct Jolt {
    /// Instant, en secondes.
    t0: f32,
    dir: (f32, f32),
    mag: f32,
    sign: f32,
}

// Amplitudes à 100 % (part de la largeur du cadre filmé ; degrés).
const SHOULDER_X: f32 = 0.012;
const SHOULDER_Y: f32 = 0.009;
const SHOULDER_ANGLE: f32 = 0.7;
const SHOULDER_JOLT: f32 = 0.025;
const SHOULDER_JOLT_ANGLE: f32 = 1.2;
const LIGHT_X: f32 = 0.004;
const LIGHT_Y: f32 = 0.0035;
const LIGHT_ANGLE: f32 = 0.15;
const EXPLOSION_POS: f32 = 0.045;
const EXPLOSION_ANGLE: f32 = 2.5;
const EXPLOSION_PUNCH: f32 = 0.06;
const JOLT_POS: f32 = 0.03;
const JOLT_ANGLE: f32 = 1.5;
/// Durée de vie d'une secousse de la caméra à l'épaule, et sa fréquence (Hz).
const JOLT_DECAY: f32 = 0.12;
const JOLT_FREQ: f32 = 5.0;
// Les secousses du lot 2 (partie 171), à 100 %.
const SEISME_X: f32 = 0.012;
const SEISME_Y: f32 = 0.03;
const SEISME_ANGLE: f32 = 1.2;
/// Hauteur du sommet de l'enveloppe du tremblement de terre (`smoothstep(3,5 u) (1 - u)^1,3`), pour la ramener à 1.
const SEISME_PIC: f32 = 0.6457;
const PAS_Y: f32 = 0.028;
const PAS_ANGLE: f32 = 0.35;
/// Durée de vie de la secousse d'un pas, et sa fréquence (Hz).
const PAS_DECAY: f32 = 0.11;
const PAS_FREQ: f32 = 5.0;
const MARCHE_X: f32 = 0.006;
const MARCHE_Y: f32 = 0.014;
const MARCHE_ANGLE: f32 = 0.6;
const VEHICULE_VIB: f32 = 0.0025;
const VEHICULE_ANGLE: f32 = 0.08;
const VEHICULE_BOSSE: f32 = 0.012;
const VEHICULE_BOSSE_ANGLE: f32 = 0.4;
const HOULE_X: f32 = 0.004;
const HOULE_Y: f32 = 0.025;
const HOULE_ANGLE: f32 = 3.0;
const SOUFFLE_POS: f32 = 0.004;
const SOUFFLE_ZOOM: f32 = 0.02;

/// Une clé de la courbe d'un préréglage du lot 2 qui pose peu de clés : à l'image `frame` de l'effet, l'avancement `valeur`
/// de son mouvement (0 : la caméra du plan ; 1 : à l'arrivée ; au-delà : un dépassement), avec ses adoucis en images.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cle {
    frame: usize,
    valeur: f32,
    ease_in: f32,
    ease_out: f32,
}

/// La courbe des préréglages qui posent peu de clés, adoucis compris (vide pour les autres) : entre deux clés, la caméra suit
/// la même loi que deux clés de caméra ordinaires (`camera::ease`), si bien que l'effet vivant et ses clés converties
/// montrent la même chose.
fn courbe_de(p: &Params, fps: f32) -> Vec<Cle> {
    use Preset::*;
    let d = p.duration;
    let df = d as f32;
    let cle = |frame: usize, valeur: f32, ease_in: f32, ease_out: f32| Cle { frame, valeur, ease_in, ease_out };
    let deux = |de: f32, a: f32, sortie: f32, entree: f32| vec![cle(0, de, 0.0, sortie), cle(d, a, entree, 0.0)];
    match p.preset {
        // (Le dézoom choc part serré et ralentit en arrivant, comme le zoom choc à l'envers.)
        DezoomChoc => deux(1.0, 0.0, 0.0, df),
        ZoomTension => deux(0.0, 1.0, 0.3 * df, 0.3 * df),
        ZoomPoint | TravellingAvant | TravellingArriere | PanoPlat | PanoRotation | Penche => deux(0.0, 1.0, 0.5 * df, 0.5 * df),
        KenBurns => deux(0.0, 1.0, 0.15 * df, 0.15 * df),
        PanoDecouverte => deux(1.0, 0.0, 0.3 * df, 0.6 * df),
        // (Le tonneau accélère jusqu'au bout.)
        Tonneau => deux(0.0, 1.0, df, 0.0),
        Recadrage => deux(1.0, 1.0, 0.0, 0.0),
        ArretNet => deux(0.0, 1.0, 0.0, 0.0),
        ZipPan | PanoDepassement => {
            let de_trop = if p.preset == ZipPan { 1.06 } else { 1.12 };
            let k = ((df * 0.7).round() as usize).max(1);
            if k >= d {
                return deux(0.0, 1.0, 0.5 * df, 0.5 * df);
            }
            let (a, b) = (k as f32, (d - k) as f32);
            vec![cle(0, 0.0, 0.0, 0.5 * a), cle(k, de_trop, 0.5 * a, 0.5 * b), cle(d, 1.0, 0.5 * b, 0.0)]
        }
        ZoomPaliers => {
            // Un à-coup de 0,12 s qui ralentit en arrivant, puis le palier tient jusqu'au suivant.
            let r = ((0.12 * fps).round() as usize).max(1);
            let mut n = p.paliers.max(1) as usize;
            while n > 1 && d / n <= r {
                n -= 1;
            }
            let seg = d / n;
            if seg <= r {
                return deux(0.0, 1.0, 0.0, df);
            }
            let mut v = vec![cle(0, 0.0, 0.0, 0.0)];
            for i in 0..n {
                let a = i * seg;
                if i > 0 {
                    v.push(cle(a, i as f32 / n as f32, 0.0, 0.0));
                }
                v.push(cle(a + r, (i + 1) as f32 / n as f32, r as f32, 0.0));
            }
            if v.last().is_some_and(|c| c.frame < d) {
                v.push(cle(d, 1.0, 0.0, 0.0));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// Calcule les clés d'un effet, image par image.
#[derive(Clone, Debug)]
pub struct Generator {
    p: Params,
    fps: f32,
    step: usize,
    waves: [Wave; 3],
    dir: (f32, f32),
    phase: f32,
    jolts: Vec<Jolt>,
    /// La courbe des préréglages qui posent peu de clés (`courbe_de`).
    courbe: Vec<Cle>,
}

impl Generator {
    pub fn new(p: &Params, fps: u32) -> Generator {
        let p = p.sanitized(fps);
        let fps_f = fps.max(1) as f32;
        // Le vertigo pose deux douzaines de clés : la distance se mêle en ligne droite entre deux
        // clés alors que le zoom se mêle en proportion, l'écart reste sous 0,1 % de la taille du sujet.
        let base_step = if p.preset == Preset::Vertigo { p.duration.div_ceil(24) } else { p.preset.step() };
        let step = base_step.max(p.duration.div_ceil(MAX_KEYS)).max(1);
        let mut rng = Rng::new(p.seed, p.preset as u64 + 1);
        let waves = [Wave::new(&mut rng), Wave::new(&mut rng), Wave::new(&mut rng)];
        let theta = rng.range(0.0, TAU);
        let phase = rng.range(0.0, TAU);
        let mut jolts = Vec::new();
        if p.preset == Preset::Shoulder {
            let total = p.duration as f32 / fps_f;
            let mut t = rng.range(0.5, 1.5);
            // (Aucune secousse dans la dernière seconde : l'effet finit calme.)
            while t < total - 1.0 {
                let a = rng.range(0.0, TAU);
                let snapped = (t * fps_f / step as f32).round() * step as f32 / fps_f;
                jolts.push(Jolt { t0: snapped, dir: (a.cos(), a.sin()), mag: rng.range(0.6, 1.0), sign: if rng.unit() < 0.5 { -1.0 } else { 1.0 } });
                t += rng.range(0.9, 2.4);
            }
        }
        if p.preset == Preset::Vehicule {
            // Les bosses de la route : de temps en temps, jamais dans la dernière demi-seconde.
            let total = p.duration as f32 / fps_f;
            let mut t = rng.range(0.3, 0.9);
            while t < total - 0.5 {
                let snapped = (t * fps_f / step as f32).round() * step as f32 / fps_f;
                jolts.push(Jolt { t0: snapped, dir: (0.0, 1.0), mag: rng.range(0.5, 1.0), sign: if rng.unit() < 0.5 { -1.0 } else { 1.0 } });
                t += rng.range(0.7, 1.6);
            }
        }
        let courbe = courbe_de(&p, fps_f);
        Generator { p, fps: fps_f, step, waves, dir: (theta.cos(), theta.sin()), phase, jolts, courbe }
    }

    pub fn params(&self) -> &Params {
        &self.p
    }

    /// Les images (depuis le début de l'effet) où l'effet pose une clé : 0, de `step` en `step`, et la dernière (ou les clés
    /// de sa courbe).
    pub fn frames(&self) -> Vec<usize> {
        if !self.courbe.is_empty() {
            return self.courbe.iter().map(|c| c.frame).collect();
        }
        let mut out: Vec<usize> = (0..self.p.duration).step_by(self.step).collect();
        out.push(self.p.duration);
        out
    }

    /// Ses décalages sont dans l'image (la flèche, la cible) : `plan` les tourne avec le cadre.
    pub fn dans_l_image(&self) -> bool {
        self.p.preset.poignee() != Poignee::Aucune
    }

    /// L'avancement du mouvement à l'image `frame`, sur la courbe (`courbe_de`) ; ses adoucis si une clé y est.
    fn sur_la_courbe(&self, frame: usize) -> Sample {
        let c = &self.courbe;
        let i = c.partition_point(|k| k.frame < frame);
        let (e, ease_in, ease_out) = if i < c.len() && c[i].frame == frame {
            (c[i].valeur, c[i].ease_in, c[i].ease_out)
        } else if i == 0 {
            (c[0].valeur, 0.0, 0.0)
        } else if i >= c.len() {
            (c[c.len() - 1].valeur, 0.0, 0.0)
        } else {
            let (a, b) = (c[i - 1], c[i]);
            let span = (b.frame - a.frame).max(1) as f32;
            let t = (frame - a.frame) as f32 / span;
            (a.valeur + (b.valeur - a.valeur) * camera::ease(t, span, a.ease_out, b.ease_in), 0.0, 0.0)
        };
        Sample { ease_in, ease_out, ..self.pose(e, frame) }
    }

    /// Ce qu'ajoute à la caméra un préréglage à courbe, à l'avancement `e` de son mouvement.
    fn pose(&self, e: f32, frame: usize) -> Sample {
        use Preset::*;
        let p = &self.p;
        let [vx, vy] = p.vers;
        let f = p.zoom_factor;
        let mut s = Sample { frame, dx: 0.0, dy: 0.0, angle: 0.0, zoom: 1.0, ease_in: 0.0, ease_out: 0.0, pan: 0.0, avance: 1.0 };
        match p.preset {
            // (Le centre va vers la cible pendant que le cadre se resserre : à l'arrivée, la cible est au centre.)
            DezoomChoc | ZoomPaliers | ZoomTension | ZoomPoint | Recadrage | KenBurns => {
                s.dx = vx * e;
                s.dy = vy * e;
                s.zoom = f.powf(e);
            }
            PanoPlat | ZipPan | PanoDepassement | PanoDecouverte | ArretNet => {
                s.dx = vx * e;
                s.dy = vy * e;
            }
            TravellingAvant => s.avance = f.powf(e),
            TravellingArriere => s.avance = f.powf(-e),
            PanoRotation => s.pan = p.degres * e,
            Penche => s.angle = p.degres * e,
            Tonneau => {
                s.angle = p.degres * e;
                s.zoom = f.powf(e);
            }
            _ => {}
        }
        s
    }

    /// Les bornes de ce que l'effet ajoute à la caméra.
    pub fn bounds(&self) -> Bounds {
        let i = self.p.intensity;
        match self.p.preset {
            Preset::Shoulder => Bounds {
                pos: i * (SHOULDER_X.max(SHOULDER_Y) + SHOULDER_JOLT * 1.01),
                angle: i * (SHOULDER_ANGLE + SHOULDER_JOLT_ANGLE * 1.01),
                punch: 0.0,
            },
            Preset::LightShake => Bounds { pos: i * LIGHT_X.max(LIGHT_Y), angle: i * LIGHT_ANGLE, punch: 0.0 },
            Preset::Explosion => Bounds { pos: i * EXPLOSION_POS, angle: i * EXPLOSION_ANGLE, punch: if self.p.punch { i * EXPLOSION_PUNCH } else { 0.0 } },
            Preset::Jolt => Bounds { pos: i * JOLT_POS, angle: i * JOLT_ANGLE, punch: 0.0 },
            Preset::Seisme => Bounds { pos: i * SEISME_X.max(SEISME_Y), angle: i * SEISME_ANGLE, punch: 0.0 },
            Preset::PasLourds => Bounds { pos: i * PAS_Y, angle: i * PAS_ANGLE, punch: 0.0 },
            Preset::Marche => Bounds { pos: i * MARCHE_X.max(MARCHE_Y * 0.5), angle: i * MARCHE_ANGLE, punch: 0.0 },
            Preset::Vehicule => Bounds {
                pos: self.p.vers[0].abs().max(self.p.vers[1].abs()) + i * (VEHICULE_VIB * 1.2 + VEHICULE_BOSSE * 1.01),
                angle: i * (VEHICULE_ANGLE + VEHICULE_BOSSE_ANGLE * 1.01),
                punch: 0.0,
            },
            Preset::Houle => Bounds { pos: i * HOULE_X.max(HOULE_Y), angle: i * HOULE_ANGLE, punch: 0.0 },
            Preset::Respiration => Bounds { pos: i * SOUFFLE_POS, angle: 0.0, punch: i * SOUFFLE_ZOOM },
            _ => Bounds { pos: 0.0, angle: 0.0, punch: 0.0 },
        }
    }

    /// Le décalage à l'image `frame` de l'effet (0 = sa première clé). Au-delà de la durée, le
    /// résultat est celui de la dernière image.
    pub fn at(&self, frame: usize) -> Sample {
        let frame = frame.min(self.p.duration);
        let last = frame == self.p.duration;
        let t = frame as f32 / self.fps;
        let total = self.p.duration as f32 / self.fps;
        let neutral = Sample { frame, dx: 0.0, dy: 0.0, angle: 0.0, zoom: 1.0, ease_in: 0.0, ease_out: 0.0, pan: 0.0, avance: 1.0 };
        if !self.courbe.is_empty() {
            return self.sur_la_courbe(frame);
        }
        let i = self.p.intensity;
        let f = self.p.freq;
        let [wx, wy, wa] = self.waves;
        // L'enveloppe des secousses continues : elles montent et s'éteignent sans à-coup, sur `ramp` secondes.
        let env = |ramp: f32| {
            let ramp = ramp.min(total / 3.0).max(1e-3);
            smoothstep(t / ramp) * smoothstep((total - t) / ramp)
        };
        // Où en est l'effet, de 0 à 1.
        let u = frame as f32 / self.p.duration.max(1) as f32;
        match self.p.preset {
            Preset::Seisme => {
                if last {
                    return neutral;
                }
                let u = t / total.max(1e-3);
                // Il monte sur le premier quart, gronde, puis s'éteint jusqu'à la fin.
                let e = (smoothstep(u * 3.5) * (1.0 - u).max(0.0).powf(1.3) / SEISME_PIC).min(1.0);
                Sample {
                    dx: i * SEISME_X * e * (0.7 * wx.at(f, t) + 0.3 * wx.at(f * 0.31, t + 1.7)),
                    dy: i * SEISME_Y * e * (0.7 * wy.at(f, t) + 0.3 * wy.at(f * 0.31, t + 0.9)),
                    angle: i * SEISME_ANGLE * e * wa.at(f * 0.6, t),
                    ..neutral
                }
            }
            Preset::PasLourds => {
                if last {
                    return neutral;
                }
                // Un pas toutes les 1 / f secondes : une secousse verticale qui s'éteint vite, le cadre penche d'un côté puis
                // de l'autre.
                let periode = 1.0 / f.max(0.05);
                let n = (t / periode).floor();
                let dt = t - n * periode;
                let k = (-dt / PAS_DECAY).exp() * (TAU * PAS_FREQ * dt).cos();
                let pied = if (n as i64) % 2 == 0 { 1.0 } else { -1.0 };
                Sample { dy: i * PAS_Y * k, angle: i * PAS_ANGLE * k * pied, ..neutral }
            }
            Preset::Marche => {
                // La caméra avance à vitesse constante : sa distance passe de d à d / facteur.
                let avance = 1.0 / (1.0 - (1.0 - 1.0 / self.p.zoom_factor) * u.min(1.0));
                if last {
                    return Sample { avance, ..neutral };
                }
                let e = env(0.3);
                let pas = (std::f32::consts::PI * f * t).sin();
                Sample {
                    dx: i * MARCHE_X * e * (std::f32::consts::PI * f * 0.5 * t).sin(),
                    dy: i * MARCHE_Y * e * (pas.abs() - 0.5),
                    angle: i * MARCHE_ANGLE * e * pas,
                    avance,
                    ..neutral
                }
            }
            Preset::Vehicule => {
                let [vx, vy] = self.p.vers;
                if last {
                    return Sample { dx: vx, dy: vy, ..neutral };
                }
                let e = env(0.3);
                let mut s = Sample {
                    dx: vx * u + i * VEHICULE_VIB * e * wx.at(f, t),
                    dy: vy * u + i * VEHICULE_VIB * 1.2 * e * wy.at(f * 1.13, t),
                    angle: i * VEHICULE_ANGLE * e * wa.at(f * 0.5, t),
                    ..neutral
                };
                for j in &self.jolts {
                    let dt = t - j.t0;
                    if (0.0..1.0).contains(&dt) {
                        let k = (-dt / 0.1).exp() * (TAU * 4.0 * dt).cos() * j.mag * i * e;
                        s.dy += VEHICULE_BOSSE * k;
                        s.angle += VEHICULE_BOSSE_ANGLE * k * j.sign;
                    }
                }
                s
            }
            Preset::Houle => {
                if last {
                    return neutral;
                }
                let e = env(0.8);
                let a = TAU * f * t;
                Sample { dx: i * HOULE_X * e * (a * 0.77 + self.phase).sin(), dy: i * HOULE_Y * e * (a + 1.0).sin(), angle: i * HOULE_ANGLE * e * a.sin(), ..neutral }
            }
            Preset::Respiration => {
                if last {
                    return neutral;
                }
                let e = env(1.0);
                Sample {
                    dx: i * SOUFFLE_POS * e * wx.at(f * 0.8, t),
                    dy: i * SOUFFLE_POS * 0.75 * e * wy.at(f * 0.72, t),
                    zoom: 1.0 + i * SOUFFLE_ZOOM * e * (0.5 - 0.5 * (TAU * f * t).cos()),
                    ..neutral
                }
            }
            Preset::Shoulder | Preset::LightShake => {
                if last {
                    return neutral;
                }
                let ramp = (0.3_f32).min(total / 3.0).max(1e-3);
                let env = smoothstep(t / ramp) * smoothstep((total - t) / ramp);
                let (ax, ay, aa) = if self.p.preset == Preset::Shoulder { (SHOULDER_X, SHOULDER_Y, SHOULDER_ANGLE) } else { (LIGHT_X, LIGHT_Y, LIGHT_ANGLE) };
                let mut s = Sample {
                    dx: i * ax * env * wx.at(f, t),
                    dy: i * ay * env * wy.at(f, t),
                    angle: i * aa * env * wa.at(f, t),
                    ..neutral
                };
                for j in &self.jolts {
                    let dt = t - j.t0;
                    if (0.0..1.5).contains(&dt) {
                        let k = (-dt / JOLT_DECAY).exp() * (TAU * JOLT_FREQ * dt).cos() * j.mag * i;
                        s.dx += SHOULDER_JOLT * k * j.dir.0;
                        s.dy += SHOULDER_JOLT * k * j.dir.1;
                        s.angle += SHOULDER_JOLT_ANGLE * k * j.sign;
                    }
                }
                s
            }
            Preset::Explosion | Preset::Jolt => {
                if last {
                    return neutral;
                }
                let (pos, rot, tau) = if self.p.preset == Preset::Explosion {
                    (EXPLOSION_POS, EXPLOSION_ANGLE, total / 4.0)
                } else {
                    (JOLT_POS, JOLT_ANGLE, total / 3.0)
                };
                let env = (-t / tau.max(1e-3)).exp();
                let main = (TAU * f * t).cos();
                let zoom = if self.p.preset == Preset::Explosion && self.p.punch { 1.0 + EXPLOSION_PUNCH * i * env } else { 1.0 };
                Sample {
                    dx: i * pos * env * (0.75 * self.dir.0 * main + 0.25 * wx.at(f, t)),
                    dy: i * pos * env * (0.75 * self.dir.1 * main + 0.25 * wy.at(f, t)),
                    angle: i * rot * env * (0.6 * (TAU * f * 0.7 * t + self.phase).cos() + 0.4 * wa.at(f, t)),
                    zoom,
                    ..neutral
                }
            }
            Preset::ZoomIn | Preset::ZoomOut | Preset::CrashZoom | Preset::Vertigo => {
                let factor = self.p.zoom_factor;
                let end = if self.p.preset == Preset::ZoomOut { 1.0 / factor } else { factor };
                let dur = self.p.duration as f32;
                let (out, inn) = self.zoom_eases();
                let zoom = if last {
                    end
                } else {
                    end.powf(camera::ease(frame as f32 / dur, dur, out, inn))
                };
                // (Le vertigo a l'adoucissement dans ses clés ; les zooms le posent sur leurs deux clés.)
                let (ease_in, ease_out) = if self.p.preset == Preset::Vertigo || (0 < frame && !last) {
                    (0.0, 0.0)
                } else if frame == 0 {
                    (0.0, out)
                } else {
                    (inn, 0.0)
                };
                Sample { zoom, ease_in, ease_out, ..neutral }
            }
            // (Les préréglages à courbe sont rendus plus haut, par `sur_la_courbe`.)
            _ => neutral,
        }
    }

    /// Adoucis (départ, arrivée) du mouvement d'un zoom, en images.
    fn zoom_eases(&self) -> (f32, f32) {
        let dur = self.p.duration as f32;
        match self.p.preset {
            Preset::CrashZoom => (0.0, dur),
            _ => (dur * 0.5, dur * 0.5),
        }
    }

    /// Le vertigo garde la taille du sujet : la caméra recule avec la focale.
    pub fn keeps_subject_size(&self) -> bool {
        self.p.preset == Preset::Vertigo
    }

    /// L'effet commence d'un coup (une clé calme juste avant).
    pub fn abrupt(&self) -> bool {
        self.p.preset.is_abrupt()
    }
}

// ---------------------------------------------------------------------- plan et écriture

/// Une clé à poser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlannedKey {
    /// Image du plan.
    pub frame: usize,
    pub cam: Camera,
    /// Adoucis à poser (sinon ceux de la clé, s'il y en a déjà une).
    pub ease_in: Option<f32>,
    pub ease_out: Option<f32>,
}

/// Ce qu'un effet va écrire dans les clés d'un plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub keys: Vec<PlannedKey>,
    /// Clé du début du plan, si le plan n'avait aucune clé et que l'effet commence plus tard : la
    /// caméra de continuité y reste en place jusque-là.
    pub home: Option<(usize, Camera)>,
    /// Clé calme juste avant un effet qui commence d'un coup.
    pub before: Option<(usize, Camera)>,
    /// L'effet a été coupé par la fin du plan.
    pub truncated: bool,
}

impl Plan {
    /// Dernière image touchée.
    pub fn last_frame(&self) -> Option<usize> {
        self.keys.last().map(|k| k.frame)
    }
}

/// Prépare les clés de l'effet `g` posé à partir de l'image `start` d'un plan de `plan_len`
/// images. `base` : la caméra du plan avant l'effet ; `extra` : images de clés déjà là, à garder
/// (elles reçoivent aussi le décalage) ; `has_keys` : le plan avait déjà des clés.
pub fn plan(g: &Generator, frame: [usize; 2], start: usize, plan_len: usize, extra: &[usize], has_keys: bool, base: &dyn Fn(usize) -> Camera) -> Plan {
    let mut out = Plan { keys: Vec::new(), home: None, before: None, truncated: false };
    let Some(last_allowed) = plan_len.checked_sub(1).filter(|&l| start <= l) else { return out };
    let duration = g.params().duration;
    let mut rels = g.frames();
    rels.extend(extra.iter().filter(|&&f| f > start && f < start + duration).map(|&f| f - start));
    rels.sort_unstable();
    rels.dedup();
    if start + duration > last_allowed {
        out.truncated = true;
        rels.retain(|&r| start + r <= last_allowed);
        rels.push(last_allowed - start);
        rels.dedup();
    }
    let d0 = camera::ref_distance(frame);
    for rel in rels {
        let s = g.at(rel);
        let at = start + rel;
        let b = base(at);
        let width = frame[0] as f32 / b.magnification(frame).max(1e-3);
        let mut mul = s.zoom;
        if g.keeps_subject_size() {
            // La distance suit la focale : mul ne sort ni des bornes du zoom ni de celles de la distance.
            let dist = b.distance(frame);
            let lo = (0.05 / b.zoom).max(0.05 * d0 / dist);
            let hi = (50.0 / b.zoom).min(20.0 * d0 / dist);
            mul = mul.clamp(lo, hi.max(lo));
        }
        let zoom = (b.zoom * mul).clamp(0.05, 50.0);
        let dolly = if g.keeps_subject_size() {
            d0 - b.distance(frame) * (zoom / b.zoom)
        } else if s.avance != 1.0 {
            // (Un travelling, partie 171 : la distance divisée par l'avancée, jamais plus près que la borne de `distance`.)
            let dist = b.distance(frame);
            let a = s.avance.clamp(0.05, (dist / (0.02 * d0)).max(0.05));
            d0 - dist / a
        } else {
            b.dolly
        };
        // Une flèche ou une cible est tirée dans l'image (partie 171) : son décalage tourne avec le cadre. Les secousses d'avant
        // gardent le leur sur la feuille, comme toujours.
        let (ox, oy) = if g.dans_l_image() {
            let (sn, cs) = b.angle.to_radians().sin_cos();
            (s.dx * cs - s.dy * sn, s.dx * sn + s.dy * cs)
        } else {
            (s.dx, s.dy)
        };
        let cam = Camera { x: b.x + ox * width, y: b.y + oy * width, angle: b.angle + s.angle, zoom, dolly, pan: b.pan + s.pan, ..b }.limited(frame);
        out.keys.push(PlannedKey { frame: at, cam, ease_in: (s.ease_in > 0.0).then_some(s.ease_in), ease_out: (s.ease_out > 0.0).then_some(s.ease_out) });
    }
    if g.abrupt() && start > 0 {
        out.before = Some((start - 1, base(start - 1)));
    }
    if !has_keys && start > 0 {
        out.home = Some((0, base(0)));
    }
    out
}

// ---------------------------------------------------------------------- l'effet vivant

/// Un effet de caméra **vivant** (Anthony, 2026-11-08 : « les effets de caméra doivent être appliqués et visibles
/// immédiatement sans création de clés ») : un réglage du plan, rangé dans le projet, calculé quand la caméra est
/// évaluée (`Document::panel_camera`). Aucune clé n'est écrite ; « Convertir en clés » les pose (`cuire`).
///
/// La règle qui garantit « la cuisson donne la même image » : l'effet vivant **est** la cuisson, faite à la volée. La
/// caméra montrée est celle qu'auraient les clés cuites (`cuire`), interpolées et postérisées comme des clés
/// ordinaires ; convertir ne fait qu'écrire ces clés-là.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vivant {
    /// Préréglage, intensité, durée (en images), fréquence, graine, facteur de zoom, coup de zoom.
    pub params: Params,
    /// Image du plan où l'effet commence.
    pub start: usize,
}

impl Vivant {
    /// Les réglages ramenés dans leurs bornes.
    pub fn sanitized(&self, fps: u32) -> Vivant {
        Vivant { params: self.params.sanitized(fps), start: self.start }
    }

    /// Dernière image touchée par l'effet dans un plan de `plan_len` images (coupée par la fin du plan).
    pub fn last_frame(&self, plan_len: usize) -> usize {
        (self.start + self.params.duration).min(plan_len.saturating_sub(1))
    }
}

/// Les clés de caméra d'un plan une fois l'effet `v` cuit dedans : `keys` (ses clés), `base` (la caméra du plan sans
/// l'effet, continuité comprise). Le même calcul sert à l'affichage (effet vivant) et à « Convertir en clés ». Rend
/// aussi ce que l'effet a posé (`Plan`), rien si l'effet commence après la fin du plan.
pub fn cuire(v: &Vivant, fps: u32, frame: [usize; 2], plan_len: usize, keys: &[CameraKey], base: &dyn Fn(usize) -> Camera) -> Option<(Vec<CameraKey>, Plan)> {
    let g = Generator::new(&v.params, fps);
    let extra: Vec<usize> = keys.iter().map(|k| k.frame).collect();
    let plan = plan(&g, frame, v.start, plan_len, &extra, !keys.is_empty(), base);
    plan.last_frame()?;
    let mut out = keys.to_vec();
    write(&mut out, &plan);
    Some((out, plan))
}

/// Comme `cuire`, sans le `Plan`, et gardé en mémoire : la caméra est évaluée bien des fois par image d'écran (cadre,
/// vignettes, montage, lecture), le calcul ne se refait que si quelque chose a changé. La clé de la mémoire porte tout
/// ce dont dépend le résultat : l'effet, la cadence, le cadre, la durée du plan, ses clés, et (plan sans clé) la caméra
/// de continuité, qui est alors la même à toutes les images.
pub fn cuire_vite(v: &Vivant, fps: u32, frame: [usize; 2], plan_len: usize, keys: &[CameraKey], base: &dyn Fn(usize) -> Camera) -> Option<Vec<CameraKey>> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let cam = |h: &mut std::collections::hash_map::DefaultHasher, c: &Camera| {
        for x in [c.x, c.y, c.zoom, c.angle, c.dolly, c.tilt, c.pan] {
            x.to_bits().hash(h);
        }
    };
    let p = &v.params;
    (p.preset as u8, p.intensity.to_bits(), p.duration, p.freq.to_bits(), p.seed, p.zoom_factor.to_bits(), p.punch, v.start).hash(&mut h);
    (p.vers[0].to_bits(), p.vers[1].to_bits(), p.degres.to_bits(), p.paliers).hash(&mut h);
    (fps, frame, plan_len, keys.len()).hash(&mut h);
    for k in keys {
        (k.frame, k.smooth, k.ease_in.to_bits(), k.ease_out.to_bits()).hash(&mut h);
        cam(&mut h, &k.cam);
    }
    if keys.is_empty() {
        cam(&mut h, &base(0));
    }
    let id = h.finish();
    // (Une table par fil, jusqu'à `MEMOIRE_MAX` cuissons : une planche de centaines de plans à effet ne recalcule pas en
    // boucle. Pleine, elle repart de zéro : les plans affichés se recalculent une fois.)
    type Table = std::collections::HashMap<u64, Option<std::rc::Rc<Vec<CameraKey>>>>;
    thread_local! {
        static MEMOIRE: std::cell::RefCell<Table> = std::cell::RefCell::new(Table::new());
    }
    if let Some(hit) = MEMOIRE.with(|m| m.borrow().get(&id).cloned()) {
        return hit.map(|r| (*r).clone());
    }
    let out = cuire(v, fps, frame, plan_len, keys, base).map(|(k, _)| std::rc::Rc::new(k));
    MEMOIRE.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() >= MEMOIRE_MAX {
            m.clear();
        }
        m.insert(id, out.clone());
    });
    out.map(|r| (*r).clone())
}

/// Nombre de cuissons gardées par fil (`cuire_vite`).
pub const MEMOIRE_MAX: usize = 4096;

/// Tous les effets vivants d'un plan, **cuits l'un après l'autre** dans l'ordre de la liste (partie 170 : les blocs de la piste
/// Caméra). Le premier part des clés du plan (`keys`) et de sa caméra sans effet (`base`, continuité comprise) ; chacun des
/// suivants part des clés que le précédent a écrites, et de la caméra qu'elles donnent : un tremblement posé sur un zoom tremble
/// autour du zoom. C'est exactement ce que ferait « Convertir en clés » sur chaque effet à tour de rôle : convertir ne change
/// donc aucune image. Rien si aucun effet ne pose de clé (liste vide, ou tous commencent après la fin du plan).
pub fn cuire_tous(effets: &[Vivant], fps: u32, frame: [usize; 2], plan_len: usize, keys: &[CameraKey], base: &dyn Fn(usize) -> Camera, default: Camera) -> Option<Vec<CameraKey>> {
    let mut cuites: Option<Vec<CameraKey>> = None;
    for v in effets {
        let suivantes = {
            let k: &[CameraKey] = cuites.as_deref().unwrap_or(keys);
            if cuites.is_none() || k.is_empty() {
                // (Rien n'a encore été écrit : la base est la caméra du plan sans effet, continuité comprise.)
                cuire_vite(v, fps, frame, plan_len, k, base)
            } else {
                let deja = |f: usize| camera::camera_at(k, f, default);
                cuire_vite(v, fps, frame, plan_len, k, &deja)
            }
        };
        if suivantes.is_some() {
            cuites = suivantes;
        }
    }
    cuites
}

/// Écrit `plan` dans les clés d'un plan : les clés posées remplacent celles de la même image (en
/// gardant leurs adoucis, sauf ceux que l'effet pose), les autres restent.
pub fn write(keys: &mut Vec<CameraKey>, plan: &Plan) {
    if let (true, Some((frame, cam))) = (keys.is_empty(), plan.home) {
        keys.push(CameraKey::new(frame, cam));
    }
    if let Some((frame, cam)) = plan.before {
        if !keys.iter().any(|k| k.frame == frame) {
            camera::put_key(keys, CameraKey::new(frame, cam));
        }
    }
    for k in &plan.keys {
        match keys.iter_mut().find(|x| x.frame == k.frame) {
            Some(x) => {
                x.cam = k.cam;
                if let Some(e) = k.ease_in {
                    x.ease_in = e;
                    x.smooth = false;
                }
                if let Some(e) = k.ease_out {
                    x.ease_out = e;
                    x.smooth = false;
                }
            }
            None => {
                let mut key = CameraKey::new(k.frame, k.cam);
                key.ease_in = k.ease_in.unwrap_or(0.0);
                key.ease_out = k.ease_out.unwrap_or(0.0);
                camera::put_key(keys, key);
            }
        }
    }
}
