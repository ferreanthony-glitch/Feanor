//! Thème de l'interface : une palette centrale, lue à chaque image par les panneaux, et les
//! `Visuals` d'egui posés une seule fois quand le thème change. Seulement des couleurs et des
//! tailles : aucun élément ni ressource graphique repris.
//!
//! Six choix. **Gris sombre** est le thème de base (Anthony, 2026-11-05) : l'interface reste grise, avec les couleurs FEANOR des sortes de
//! calques et deux petits rehauts jade (Couleurs::jade_discret). **Atelier** (la marque FEANOR en sombre) et **Papier** (la marque en clair) suivent le livre de marque :
//! les jetons `sol`, `panneau`, `canevas`, `ligne`, `ligne forte`, `encre`, `encre douce`, `jade`, `jade texte` et `sur jade`
//! (constantes `ATELIER_*`, `PAPIER_*`, `JADE`, `SUR_JADE` plus bas). Le jade est l'accent (sélection, onglet actif, curseurs) avec
//! l'encre `sur jade` écrite dessus ; les bordures des champs et des boutons sont en `ligne forte` (3:1 au moins) ; pas d'ombres
//! portées. Puis trois thèmes de gris neutres (Anthony, 2026-10-18 : « un gris sombre/clair ») : **Gris clair** et
//! **Sombre** (l'ancien presque noir) ; ils gardent leurs couleurs d'avant la marque. Un dernier, **Personnalisé**, part du thème
//! courant et se règle dans Fichier > Préférences… avec neuf couleurs ; tout le reste de la palette (survol, onglet actif, texte
//! secondaire, couleur des dessins sur la timeline…) en est déduit, et choisi pour rester lisible sur le thème (contraste WCAG,
//! `contraste`).
//!
//! La palette courante vit dans le fil de l'interface (`installer`) ; chaque couleur se lit par une
//! petite fonction (`theme::fond()`, `theme::texte()`…) qui copie quatre octets.
//!
//! La police est Segoe UI si Windows l'a (sinon celle d'egui). Les couleurs du dessin (papier,
//! traits, couleurs de projet) sont des données du document, pas de l'interface : elles ne passent
//! pas par ici.

use std::cell::RefCell;

use eframe::egui::{self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2};
use serde::{Deserialize, Serialize};

use crate::document::LayerKind;

/// Une couleur d'interface : rouge, vert, bleu.
pub type Rvb = [u8; 3];

const fn gris(v: u8) -> Color32 {
    Color32::from_rgb(v, v, v)
}

fn couleur(c: Rvb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

// ------------------------------------------------------------------- choix et réglages

/// Le thème choisi dans les préférences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoixTheme {
    /// « Atelier », la marque FEANOR en sombre.
    Shot,
    /// Le thème clair de la marque : « Papier ».
    Papier,
    /// Le thème d'une installation neuve (Anthony, 2026-11-05).
    #[default]
    GrisSombre,
    GrisClair,
    Sombre,
    Personnalise,
}

impl ChoixTheme {
    pub const TOUS: [ChoixTheme; 6] = [ChoixTheme::GrisSombre, ChoixTheme::Shot, ChoixTheme::Papier, ChoixTheme::GrisClair, ChoixTheme::Sombre, ChoixTheme::Personnalise];

    pub fn nom(self) -> &'static str {
        match self {
            ChoixTheme::Shot => "Atelier",
            ChoixTheme::Papier => "Papier",
            ChoixTheme::GrisSombre => "Gris sombre",
            ChoixTheme::GrisClair => "Gris clair",
            ChoixTheme::Sombre => "Sombre",
            ChoixTheme::Personnalise => "Personnalisé",
        }
    }
}

/// Les neuf couleurs qu'on règle dans les préférences. Tout le reste en est déduit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
// (Une couleur absente d'un fichier de réglages garde le gris sombre d'avant : un thème personnalisé
// écrit avant la charte ne change pas d'aspect.)
#[serde(default = "Couleurs::gris_sombre")]
pub struct Couleurs {
    /// Fond des panneaux.
    pub panneau: Rvb,
    /// Barres d'onglets et d'outils, règle de la timeline.
    pub entete: Rvb,
    /// Fond des fenêtres et des menus.
    pub fenetre: Rvb,
    /// Boutons, cases, curseurs au repos.
    pub widgets: Rvb,
    /// Texte.
    pub texte: Rvb,
    /// Accent : sélection, cadre du panneau actif.
    pub accent: Rvb,
    /// Fond autour du dessin et entre les panneaux.
    pub fond: Rvb,
    /// Fond des rangées de la timeline et du montage.
    pub timeline: Rvb,
    /// Traits de séparation, bordures.
    pub bordure: Rvb,
    /// Vrai pour les thèmes de la marque (Atelier, Papier) : la palette prend alors les jetons exacts du livre de marque
    /// (texte secondaire, texte d'accent, bordures fortes, rehaut jade, pas d'ombres). Pas réglé
    /// dans les préférences ; absent d'un fichier de réglages, il vaut « non ».
    #[serde(default)]
    pub identite: bool,
    /// Vrai pour Gris sombre : une interface grise avec les couleurs FEANOR des sortes de calques, et le jade en petites touches
    /// seulement (trait sous l'espace actif, pastille de l'onglet). Pas réglé dans les préférences ; absent d'un fichier de réglages, il vaut « non ».
    #[serde(default)]
    pub jade_discret: bool,
}

// Les jetons de couleur du livre de marque FEANOR (sombre « Atelier » / clair « Papier »).
/// Fond de l'application.
pub const ATELIER_SOL: Rvb = [0x12, 0x12, 0x14];
pub const PAPIER_SOL: Rvb = [0xF5, 0xF1, 0xE8];
/// Panneaux, barres d'outils, cartes.
pub const ATELIER_PANNEAU: Rvb = [0x1C, 0x1C, 0x1F];
pub const PAPIER_PANNEAU: Rvb = [0xFF, 0xFF, 0xFF];
/// Le pourtour de la zone de dessin : neutre, jamais teinté.
pub const ATELIER_CANEVAS: Rvb = [0x2C, 0x2C, 0x2E];
pub const PAPIER_CANEVAS: Rvb = [0xE4, 0xE4, 0xE0];
/// Séparateurs décoratifs.
pub const ATELIER_LIGNE: Rvb = [0x2E, 0x2E, 0x32];
pub const PAPIER_LIGNE: Rvb = [0xD6, 0xD6, 0xD1];
/// Bordures des champs et des commandes (3:1 au moins).
pub const ATELIER_LIGNE_FORTE: Rvb = [0x6B, 0x6B, 0x72];
pub const PAPIER_LIGNE_FORTE: Rvb = [0x8A, 0x8A, 0x86];
/// Texte principal.
pub const ATELIER_ENCRE: Rvb = [0xED, 0xED, 0xEF];
pub const PAPIER_ENCRE: Rvb = [0x12, 0x12, 0x14];
/// Texte secondaire, légendes, timecodes.
pub const ATELIER_ENCRE_DOUCE: Rvb = [0x9A, 0x9A, 0xA3];
pub const PAPIER_ENCRE_DOUCE: Rvb = [0x5C, 0x5C, 0x63];
/// L'identité : un aplat, jamais un texte sur fond clair.
pub const JADE: Rvb = crate::logo::JADE;
#[allow(dead_code)] // (jeton du livre de marque, pas encore utilisé par l'interface)
pub const JADE_PROFOND: Rvb = crate::logo::JADE_PROFOND;
/// Liens et texte d'accent (et anneau de focus de 2 px).
pub const ATELIER_JADE_TEXTE: Rvb = [0x2F, 0xD4, 0xC0];
pub const PAPIER_JADE_TEXTE: Rvb = [0x0A, 0x6A, 0x69];
/// Texte et icônes posés sur un aplat jade.
pub const SUR_JADE: Rvb = [0x06, 0x2B, 0x2A];
/// Erreurs bloquantes et actions irréversibles : une couleur fonctionnelle, pas de la marque. Texte, icône ou mince
/// contour seulement, jamais un aplat, et toujours avec un mot ou une icône.
pub const ATELIER_ALERTE: Rvb = [0xFF, 0x6B, 0x6B];
pub const PAPIER_ALERTE: Rvb = [0xB3, 0x26, 0x1E];
/// L'étincelle, le point d'enregistrement : quelques pixels, jamais une surface.
pub const BRAISE: Rvb = [0xF2, 0xA3, 0x3A];
/// Sortes de calques dans les thèmes de la marque (Anthony, 2026-11-05) : le vectoriel est en jade sombre, les autres viennent
/// de l'irisation et de la braise. Chacune a sa version profonde, sur laquelle un nom clair se lit.
pub const IRIS_LILAS: Rvb = [0xC7, 0xB5, 0xFF];
pub const IRIS_LILAS_PROFOND: Rvb = [0x6A, 0x55, 0xB8];
pub const IRIS_ROSE: Rvb = [0xFF, 0xB3, 0xD1];
pub const IRIS_ROSE_PROFOND: Rvb = [0xA8, 0x43, 0x7A];
pub const IRIS_PALE: Rvb = [0xFF, 0xF1, 0xA8];
pub const IRIS_PALE_PROFOND: Rvb = [0x6B, 0x70, 0x28];
pub const BRAISE_PROFONDE: Rvb = [0x9A, 0x5F, 0x12];
/// Mélange du jade et du lilas : la sixième sorte (couleur unie).
pub const CIEL: Rvb = [0x7F, 0xC4, 0xE0];
pub const CIEL_PROFOND: Rvb = [0x3A, 0x62, 0x9E];
impl Default for Couleurs {
    fn default() -> Self {
        Couleurs::gris_sombre()
    }
}

impl Couleurs {
    /// « Atelier », la marque en sombre : sol, panneaux, canevas, lignes et encres du livre de marque, jade en accent.
    pub const fn shot() -> Couleurs {
        Couleurs {
            panneau: ATELIER_PANNEAU,
            entete: ATELIER_SOL,
            fenetre: ATELIER_PANNEAU,
            widgets: ATELIER_CANEVAS,
            texte: ATELIER_ENCRE,
            accent: JADE,
            fond: ATELIER_CANEVAS,
            timeline: ATELIER_CANEVAS,
            bordure: ATELIER_LIGNE,
            identite: true,
            jade_discret: false,
        }
    }

    /// « Papier », la marque en clair.
    pub const fn papier() -> Couleurs {
        Couleurs {
            panneau: PAPIER_PANNEAU,
            entete: PAPIER_SOL,
            fenetre: PAPIER_PANNEAU,
            widgets: PAPIER_SOL,
            texte: PAPIER_ENCRE,
            accent: JADE,
            fond: PAPIER_CANEVAS,
            timeline: PAPIER_SOL,
            bordure: PAPIER_LIGNE,
            identite: true,
            jade_discret: false,
        }
    }
    pub const fn gris_sombre() -> Couleurs {
        Couleurs {
            panneau: [64, 64, 64],
            entete: [54, 54, 54],
            fenetre: [56, 56, 56],
            widgets: [90, 90, 90],
            texte: [230, 230, 230],
            // Gris clair (Anthony, 2026-10-18 : l'accent et la sélection en gris clair plutôt
            // qu'en bleu) ; ce qui s'écrit dessus passe en presque noir (`sur_accent`).
            accent: [200, 200, 200],
            fond: [46, 46, 46],
            timeline: [72, 72, 72],
            bordure: [80, 80, 80],
            identite: false,
            jade_discret: true,
        }
    }

    pub const fn gris_clair() -> Couleurs {
        Couleurs {
            panneau: [192, 192, 192],
            entete: [176, 176, 176],
            fenetre: [210, 210, 210],
            widgets: [224, 224, 224],
            texte: [28, 28, 28],
            // Bleu clair : un texte presque noir s'y lit (comme la sélection de Windows).
            accent: [120, 165, 230],
            fond: [158, 158, 158],
            timeline: [205, 205, 205],
            bordure: [150, 150, 150],
            identite: false,
            jade_discret: false,
        }
    }

    /// L'ancien thème, presque noir (gris du système Spectrum, bleu d'accent saturé).
    pub const fn sombre() -> Couleurs {
        Couleurs {
            panneau: [34, 34, 34],
            entete: [27, 27, 27],
            fenetre: [34, 34, 34],
            widgets: [57, 57, 57],
            texte: [219, 219, 219],
            accent: [64, 105, 253],
            fond: [17, 17, 17],
            timeline: [40, 40, 40],
            bordure: [50, 50, 50],
            identite: false,
            jade_discret: false,
        }
    }

    /// Les noms et les accès des neuf couleurs, dans l'ordre de la fenêtre Préférences.
    pub fn champs(&mut self) -> [(&'static str, &'static str, &mut Rvb); 9] {
        [
            ("Fond des panneaux", "Le fond de tous les panneaux.", &mut self.panneau),
            ("Barres", "Barres d'onglets, barres d'outils et règle de la timeline.", &mut self.entete),
            ("Fenêtres et menus", "Le fond des fenêtres, des menus et des listes déroulantes.", &mut self.fenetre),
            ("Boutons et curseurs", "Les boutons, cases et curseurs au repos ; le survol et l'appui en sont déduits.", &mut self.widgets),
            ("Texte", "La couleur du texte ; le texte secondaire et le texte fort en sont déduits.", &mut self.texte),
            ("Accent et sélection", "La sélection, le cadre du panneau actif, les poignées au survol.", &mut self.accent),
            ("Autour du dessin", "Le fond autour de la feuille et de la caméra, et entre les panneaux.", &mut self.fond),
            ("Timeline et montage", "Le fond des rangées de la timeline, du montage et de la feuille d'exposition.", &mut self.timeline),
            ("Bordures", "Les traits de séparation et le quadrillage.", &mut self.bordure),
        ]
    }
}

/// Le réglage de l'application (`reglages.toml`, `[theme]`) : le thème choisi et, gardées à part,
/// les couleurs du thème personnalisé. Un fichier sans cette section (une installation neuve) donne
/// Gris sombre ; un poste qui a déjà choisi un thème le garde (son choix est écrit dans le fichier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglageTheme {
    pub choix: ChoixTheme,
    pub perso: Couleurs,
}

impl ReglageTheme {
    /// Les neuf couleurs du thème courant.
    pub fn couleurs(&self) -> Couleurs {
        match self.choix {
            ChoixTheme::Shot => Couleurs::shot(),
            ChoixTheme::Papier => Couleurs::papier(),
            ChoixTheme::GrisSombre => Couleurs::gris_sombre(),
            ChoixTheme::GrisClair => Couleurs::gris_clair(),
            ChoixTheme::Sombre => Couleurs::sombre(),
            ChoixTheme::Personnalise => self.perso,
        }
    }

    /// Choisit un thème. Passer à « Personnalisé » part du thème qu'on quitte.
    pub fn choisir(&mut self, choix: ChoixTheme) {
        if choix == ChoixTheme::Personnalise && self.choix != ChoixTheme::Personnalise {
            self.perso = self.couleurs();
        }
        self.choix = choix;
    }

    /// Une couleur a été changée à la main : les nouvelles couleurs deviennent le thème
    /// « Personnalisé » (si elles diffèrent vraiment du thème courant).
    pub fn modifier(&mut self, nouvelles: Couleurs) {
        if nouvelles != self.couleurs() {
            self.perso = nouvelles;
            self.choix = ChoixTheme::Personnalise;
        }
    }

    /// « Rétablir » : le thème par défaut, Gris sombre (les couleurs personnalisées sont aussi remises à
    /// celles de ce thème).
    pub fn retablir(&mut self) {
        *self = ReglageTheme::default();
    }
}

// ------------------------------------------------------------------- contraste

/// Luminance relative (WCAG) d'une couleur opaque.
pub fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let s = v as f32 / 255.0;
        if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// Rapport de contraste (WCAG) entre deux couleurs opaques : de 1 (pareil) à 21 (noir et blanc).
pub fn contraste(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `a` vers `b` de la fraction `t` (0 : `a`, 1 : `b`).
fn melange(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// `c` poussée vers `pole` par pas de 5 %, juste assez pour atteindre `min` sur chacun des `fonds`.
fn assurer(c: Color32, fonds: &[Color32], min: f32, pole: Color32) -> Color32 {
    (0..=20)
        .map(|k| melange(c, pole, k as f32 * 0.05))
        .find(|x| fonds.iter().all(|f| contraste(*x, *f) >= min))
        .unwrap_or(pole)
}

// ------------------------------------------------------------------- palette

/// Toutes les couleurs de l'interface, déduites des neuf réglées dans les préférences.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Thème clair (texte plus sombre que le fond) : `Visuals::light`, logo clair.
    pub claire: bool,
    // Fonds.
    pub fond: Color32,
    pub entete: Color32,
    pub panneau: Color32,
    pub fenetre: Color32,
    pub eleve: Color32,
    pub bord: Color32,
    /// Bordure des champs et des commandes (3:1 au moins sur les panneaux) ; ligne forte de la marque.
    pub bord_fort: Color32,
    /// Anneau de focus clavier (2 px).
    pub focus: Color32,
    pub champ: Color32,
    // Boutons.
    pub controle: Color32,
    pub controle_survol: Color32,
    pub controle_actif: Color32,
    // Textes.
    pub texte: Color32,
    pub texte_doux: Color32,
    pub texte_fort: Color32,
    // Accent : le fond de la sélection, ce qui s'écrit dessus, et sa version pour les traits et
    // les textes sur un panneau.
    pub accent: Color32,
    pub sur_accent: Color32,
    /// Fond de la sélection des textes et des lignes choisies d'egui (`Visuals::selection`) : l'accent, ou, dans un thème de la marque
    /// où l'encre ne s'y lit pas (Atelier : 1,6:1 sur le jade), du jade profond assombri jusqu'à 4,5:1 avec le texte.
    pub selection: Color32,
    /// L'encre des textes et des icônes posés sur `selection` (lignes choisies, boutons allumés) : l'encre du thème dans la marque,
    /// `sur_accent` ailleurs.
    pub sur_selection: Color32,
    pub accent_texte: Color32,
    /// Thème de la marque (`Couleurs::identite`) : jetons exacts, bordures fortes, pas d'ombres.
    pub identite: bool,
    /// Gris sombre (Couleurs::jade_discret) : couleurs FEANOR des sortes de calques et petites touches de jade.
    pub jade_discret: bool,
    /// Le rehaut : le jade de la marque (sa version « texte » sur fond clair) ; dans un autre thème, l'accent des
    /// traits (`accent_texte`). Trait sous l'espace de travail actif,
    /// icône de « Nouveau projet », angles du petit logotype : rien d'autre.
    pub rehaut: Color32,
    // Timeline et montage.
    pub regle: Color32,
    pub rangee: Color32,
    pub rangee_sombre: Color32,
    pub etiquette: Color32,
    pub graduation: Color32,
    pub graduation_seconde: Color32,
    pub bloc: Color32,
    pub poignee: Color32,
    /// Fond de l'aperçu du trait dans les réglages du pinceau : un gris un peu plus clair que le
    /// panneau (et non le papier blanc du projet) ; voir `fond_apercu_trait`.
    pub apercu_trait: Color32,
    /// Contour des éléments choisis (clés, séquences), blanc ou presque noir.
    pub contour: Color32,
    /// Encre sur les dessins vifs de la timeline, et fond des pastilles posées sur une rangée.
    pub encre: Color32,
    /// Texte écrit sur les blocs sourds du montage et les extraits son : clair sur les blocs
    /// foncés des thèmes sombres, foncé sur les blocs pâles des thèmes clairs.
    pub sur_calque: Color32,
    /// Nom écrit sur le fond sourd d'une sorte de calque (`calque_vectoriel`…) : clair sur les fonds
    /// profonds des thèmes de la marque, `sur_calque` ailleurs.
    pub sur_sorte: Color32,
    // Sortes de calques et repères.
    pub dessin_vectoriel: Color32,
    pub dessin_bitmap: Color32,
    pub dessin_vide: Color32,
    pub calque_vectoriel: Color32,
    pub calque_bitmap: Color32,
    /// Calque de couleur (calquecouleur.rs) : rose (c'était celui du calque CTG, fondu dans le calque de couleur).
    pub dessin_couleur: Color32,
    pub calque_couleur: Color32,
    /// Calque d'effet (`calqueffet.rs`) : cyan, vif et sourd.
    pub dessin_cfx: Color32,
    pub calque_cfx: Color32,
    /// Calque d'annotation (`calqueannot.rs`, et ceux de la planche) : rouge-rose, vif et sourd.
    pub dessin_annotation: Color32,
    pub calque_annotation: Color32,
    /// Calque « Couleur unie » (`couleurunie.rs`) : vert citron, vif et sourd.
    pub dessin_unie: Color32,
    pub calque_unie: Color32,
    /// Calques de toute la scène (le décor) : la teinte « fond » des noms, et celle de leurs dessins
    /// sous les plans du montage.
    pub decor: Color32,
    pub calque_decor: Color32,
    pub image_courante: Color32,
    pub camera: Color32,
    pub marque: Color32,
    /// Clés d'effets (violet).
    pub effet: Color32,
    pub son_extrait: Color32,
    pub son_choisi: Color32,
    pub son_onde: Color32,
    pub son_muet: Color32,
    /// Barres des séquences et des scènes : séquence paire, impaire, scène paire, impaire.
    pub barres: [Color32; 4],
    // Messages.
    /// Avertissement non bloquant. Thèmes de la marque : le texte neutre (la braise n'est qu'une pastille, `braise`).
    pub avertissement: Color32,
    /// Erreur : thèmes de la marque, le jeton `alerte`.
    pub erreur: Color32,
    /// `alerte` de la marque (erreurs bloquantes, actions irréversibles) et `braise` (pastille d'avertissement, point d'enregistrement).
    pub alerte: Color32,
    pub braise: Color32,
    pub succes: Color32,
}

impl Palette {
    /// La palette qui découle de neuf couleurs.
    pub fn depuis(c: &Couleurs) -> Palette {
        let (panneau, entete, fenetre, controle) = (couleur(c.panneau), couleur(c.entete), couleur(c.fenetre), couleur(c.widgets));
        let (texte, accent, fond, rangee, bord) = (couleur(c.texte), couleur(c.accent), couleur(c.fond), couleur(c.timeline), couleur(c.bordure));
        let claire = luminance(texte) < luminance(panneau);
        // Le texte fort tend vers le blanc (thème sombre) ou le presque noir (thème clair).
        let pole = if claire { gris(15) } else { Color32::WHITE };

        let eleve = melange(panneau, texte, 0.06);
        let champ = if claire { melange(panneau, Color32::WHITE, 0.55) } else { melange(panneau, Color32::BLACK, 0.35) };
        let rangee_sombre = melange(rangee, Color32::BLACK, if claire && c.identite { 0.06 } else if claire { 0.12 } else { 0.25 });
        let texte_fort = melange(texte, pole, 0.55);
        // Le texte secondaire est le plus doux qui reste lisible (4,5:1) sur les fonds d'interface.
        let lisibles = [panneau, entete, fenetre, rangee, rangee_sombre];
        let texte_doux = (0..=9).map(|k| melange(texte, panneau, 0.45 - 0.05 * k as f32)).find(|x| lisibles.iter().all(|f| contraste(*x, *f) >= 4.5)).unwrap_or(texte);
        let accent_texte = (0..=20).map(|k| melange(accent, texte_fort, k as f32 * 0.05)).find(|x| contraste(*x, panneau) >= 4.5).unwrap_or(texte_fort);
        let sur_accent = if contraste(Color32::WHITE, accent) >= 4.5 || contraste(Color32::WHITE, accent) >= contraste(gris(15), accent) { Color32::WHITE } else { gris(15) };
                // Les thèmes de la marque posent les jetons exacts du livre de marque.
        let (texte_doux, accent_texte, sur_accent) = if c.identite {
            (couleur(if claire { PAPIER_ENCRE_DOUCE } else { ATELIER_ENCRE_DOUCE }), couleur(if claire { PAPIER_JADE_TEXTE } else { ATELIER_JADE_TEXTE }), couleur(SUR_JADE))
        } else {
            (texte_doux, accent_texte, sur_accent)
        };
        let bord_fort = if c.identite { couleur(if claire { PAPIER_LIGNE_FORTE } else { ATELIER_LIGNE_FORTE }) } else { assurer(bord, &[panneau, entete, fenetre], 3.0, texte_fort) };
        let focus = accent_texte;
        let selection = if c.identite && contraste(texte, accent) < 4.5 {
            let base = couleur(JADE_PROFOND);
            (0..=20).map(|k| melange(base, panneau, k as f32 * 0.05)).find(|x| contraste(texte, *x) >= 4.5).unwrap_or(base)
        } else {
            accent
        };
        let alerte = if c.identite { couleur(if claire { PAPIER_ALERTE } else { ATELIER_ALERTE }) } else if claire { Color32::from_rgb(190, 30, 20) } else { Color32::from_rgb(247, 92, 70) };
        // Le rehaut est l'accent des traits : le jade de la marque (sa version « texte » sur fond clair).
        let rehaut = if c.jade_discret { couleur(JADE) } else { accent_texte };
        // Couleurs des repères : un jeu vif pour les fonds sombres, un jeu plus profond pour les
        // clairs ; chacune est poussée si besoin jusqu'à 3,2:1 sur les rangées.
        let jeu = |sombre: [u8; 3], clair: [u8; 3]| assurer(couleur(if claire { clair } else { sombre }), &[rangee, rangee_sombre], 3.2, anti_de(claire));
        let dessin_vide = (6..=20).map(|k| melange(rangee, texte, k as f32 * 0.05)).find(|x| contraste(*x, rangee) >= 3.2 && contraste(*x, rangee_sombre) >= 3.2).unwrap_or(texte);
        // Survolé, enfoncé : le bouton s'éloigne de son repos vers la couleur du texte, tant que le
        // texte fort y reste lisible (4,5:1).
        let vers_texte = |t: f32| (0..=16).map(|k| melange(controle, texte, t - 0.02 * k as f32)).find(|x| contraste(texte_fort, *x) >= 4.5).unwrap_or(controle);
        let controle_survol = vers_texte(0.07);
        let controle_actif = vers_texte(0.32);
        let palier = |t: f32| melange(rangee, texte, t);
        let sur_calque = if claire { gris(20) } else { gris(242) };
        let mut palette = Palette {
            claire,
            fond,
            entete,
            panneau,
            fenetre,
            eleve,
            bord,
            bord_fort,
            focus,
            champ,
            controle,
            controle_survol,
            controle_actif,
            texte,
            texte_doux,
            texte_fort,
            accent,
            sur_accent,
            selection,
            sur_selection: if c.identite { texte } else { sur_accent },
            accent_texte,
            identite: c.identite,
            jade_discret: c.jade_discret,
            rehaut,
            regle: entete,
            rangee,
            rangee_sombre,
            etiquette: eleve,
            graduation: if c.identite { melange(rangee, bord_fort, 0.55) } else { bord },
            graduation_seconde: palier(0.23),
            bloc: controle,
            poignee: texte_doux,
            // Un peu plus clair que le panneau : vers le blanc, de 12 % (thèmes sombres) ou de
            // 55 % (thème clair, dont le panneau est déjà pâle).
            apercu_trait: melange(panneau, Color32::WHITE, if claire { 0.55 } else { 0.12 }),
            contour: if claire { gris(15) } else { Color32::WHITE },
            encre: if claire { Color32::WHITE } else { melange(rangee, Color32::BLACK, 0.6) },
            sur_calque,
            sur_sorte: sur_calque,
            // Vectoriel : l'orange de SHOT (#E76F3C, assombri pour que les noms se lisent dessus) ; bitmap : le bleu qu'avait le
            // vectoriel (Anthony, 2026-10-28). Le vert d'avant du bitmap n'est plus utilisé.
            dessin_vectoriel: jeu([238, 124, 76], [200, 84, 36]),
            dessin_bitmap: jeu([86, 129, 255], [40, 90, 210]),
            dessin_vide,
            calque_vectoriel: if claire { Color32::from_rgb(240, 150, 110) } else { Color32::from_rgb(178, 74, 30) },
            calque_bitmap: if claire { Color32::from_rgb(110, 150, 230) } else { Color32::from_rgb(52, 91, 248) },
            dessin_couleur: jeu([229, 91, 182], [180, 30, 130]),
            calque_couleur: if claire { Color32::from_rgb(210, 120, 185) } else { Color32::from_rgb(176, 52, 136) },
            dessin_cfx: jeu([48, 208, 236], [0, 128, 168]),
            calque_cfx: if claire { Color32::from_rgb(120, 200, 224) } else { Color32::from_rgb(14, 130, 160) },
            dessin_annotation: jeu([226, 54, 80], [186, 24, 52]),
            calque_annotation: if claire { Color32::from_rgb(226, 130, 146) } else { Color32::from_rgb(168, 36, 62) },
            dessin_unie: jeu([176, 206, 64], [100, 130, 14]),
            calque_unie: if claire { Color32::from_rgb(196, 218, 120) } else { Color32::from_rgb(112, 140, 28) },
            decor: jeu([214, 170, 92], [140, 92, 10]),
            calque_decor: if claire { Color32::from_rgb(228, 196, 130) } else { Color32::from_rgb(214, 170, 92) },
            image_courante: jeu([252, 67, 46], [205, 30, 20]),
            camera: jeu([255, 196, 60], [150, 92, 0]),
            marque: jeu([244, 129, 12], [170, 70, 0]),
            effet: jeu([182, 103, 230], [120, 50, 190]),
            son_extrait: if claire { Color32::from_rgb(110, 185, 215) } else { Color32::from_rgb(3, 99, 140) },
            son_choisi: if claire { Color32::from_rgb(80, 160, 205) } else { Color32::from_rgb(8, 115, 168) },
            son_onde: if claire { Color32::from_rgb(10, 80, 130) } else { Color32::from_rgb(140, 205, 240) },
            son_muet: if claire { Color32::from_rgb(200, 40, 30) } else { Color32::from_rgb(223, 52, 34) },
            barres: [rangee, palier(0.07), palier(0.07), palier(0.13)],
            succes: if claire { Color32::from_rgb(20, 120, 50) } else { Color32::from_rgb(120, 210, 130) },
            avertissement: if c.identite { texte_fort } else if claire { Color32::from_rgb(176, 100, 0) } else { Color32::from_rgb(232, 153, 34) },
            erreur: if c.identite { alerte } else if claire { Color32::from_rgb(190, 30, 20) } else { Color32::from_rgb(247, 92, 70) },
            alerte,
            braise: couleur(BRAISE),
        };
        // Atelier, Papier et Gris sombre : les sortes de calques prennent les couleurs de la marque. Le fond du vectoriel est le jade sombre
        // de la sélection d'Atelier ; dans Papier, le même jade profond, juste assez foncé pour qu'un nom clair s'y lise.
        if c.identite || c.jade_discret {
            palette.sur_sorte = gris(242);
            // (Le jade profond, à peine éclairci s'il le faut pour garder 3:1 sur les rangées.)
            palette.dessin_vectoriel = assurer(couleur(JADE_PROFOND), &[rangee, rangee_sombre], 3.0, anti_de(claire));
            palette.calque_vectoriel = if claire { melange(couleur(JADE_PROFOND), Color32::BLACK, 0.06) } else { jade_sombre() };
            palette.dessin_bitmap = jeu(IRIS_LILAS, IRIS_LILAS_PROFOND);
            palette.calque_bitmap = couleur(IRIS_LILAS_PROFOND);
            palette.dessin_couleur = jeu(IRIS_ROSE, IRIS_ROSE_PROFOND);
            palette.calque_couleur = couleur(IRIS_ROSE_PROFOND);
            palette.dessin_cfx = jeu(IRIS_PALE, IRIS_PALE_PROFOND);
            palette.calque_cfx = couleur(IRIS_PALE_PROFOND);
            palette.dessin_annotation = jeu(BRAISE, BRAISE_PROFONDE);
            palette.calque_annotation = couleur(BRAISE_PROFONDE);
            palette.dessin_unie = jeu(CIEL, CIEL_PROFOND);
            palette.calque_unie = couleur(CIEL_PROFOND);
        }
        palette
    }

    /// Les `Visuals` d'egui de cette palette.
    pub fn visuels(&self) -> egui::Visuals {
        let mut v = if self.claire { egui::Visuals::light() } else { egui::Visuals::dark() };
        let arrondi = CornerRadius::same(4);
        v.panel_fill = self.panneau;
        v.window_fill = self.fenetre;
        v.window_stroke = Stroke::new(1.0_f32, if self.identite { self.bord_fort } else { self.bord });
        // Marque : panneaux et fenêtres à 8 px, sans ombre portée.
        v.window_corner_radius = CornerRadius::same(if self.identite { 8 } else { 6 });
        v.menu_corner_radius = CornerRadius::same(if self.identite { 8 } else { 5 });
        let (ombre_fenetre, ombre_menu) = if self.identite { (0, 0) } else if self.claire { (60, 50) } else { (120, 110) };
        v.window_shadow = Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(ombre_fenetre) };
        v.popup_shadow = Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(ombre_menu) };
        v.extreme_bg_color = self.champ;
        v.faint_bg_color = self.eleve;
        v.code_bg_color = self.entete;
        v.hyperlink_color = self.accent_texte;
        v.warn_fg_color = self.avertissement;
        v.error_fg_color = self.erreur;
        // Marque : le texte choisi et les lignes choisies ont le fond `selection` ; leur encre (et le cadre d'un champ actif, 2 px) est
        // l'encre du thème, lisible sur ce fond et sur le panneau. Les accents jade passent par `accent()`, pas par ici.
        v.selection.bg_fill = self.selection;
        v.selection.stroke = if self.identite { Stroke::new(2.0_f32, self.sur_selection) } else { Stroke::new(1.0_f32, self.sur_accent) };
        let w = &mut v.widgets;
        // Textes et cadres qui ne réagissent pas.
        w.noninteractive.bg_fill = self.panneau;
        w.noninteractive.weak_bg_fill = self.panneau;
        w.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.bord);
        w.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.texte);
        w.noninteractive.corner_radius = arrondi;
        // Boutons, cases, curseurs : au repos, survolés, enfoncés, ouverts.
        for (etat, fond, texte) in [
            (&mut w.inactive, self.controle, self.texte),
            (&mut w.hovered, self.controle_survol, self.texte_fort),
            (&mut w.active, self.controle_actif, self.texte_fort),
            (&mut w.open, self.controle_survol, self.texte_fort),
        ] {
            etat.bg_fill = fond;
            etat.weak_bg_fill = fond;
            etat.bg_stroke = Stroke::NONE;
            etat.fg_stroke = Stroke::new(1.0_f32, texte);
            etat.corner_radius = arrondi;
            etat.expansion = 0.0;
        }
        // Sur un fond clair, un bouton au repos a besoin d'un trait pour se détacher du panneau.
        if self.claire {
            w.inactive.bg_stroke = Stroke::new(1.0_f32, self.bord);
        }
        w.hovered.bg_stroke = Stroke::new(1.0_f32, self.controle_actif);
        // Marque : les commandes ont une bordure `ligne forte` (3:1), jade au survol et à l'appui.
        if self.identite {
            w.inactive.bg_stroke = Stroke::new(1.0_f32, self.bord_fort);
            for etat in [&mut w.hovered, &mut w.active, &mut w.open] {
                etat.bg_stroke = Stroke::new(1.0_f32, self.accent_texte);
            }
        }

        v
    }
}

/// Le jade sombre d'Atelier : le jade profond, éclairci vers le panneau d'Atelier jusqu'à ce que l'encre s'y lise (4,5:1).
fn jade_sombre() -> Color32 {
    let base = couleur(JADE_PROFOND);
    (0..=20).map(|k| melange(base, couleur(ATELIER_PANNEAU), k as f32 * 0.05)).find(|x| contraste(couleur(ATELIER_ENCRE), *x) >= 4.5).unwrap_or(base)
}

/// Vers quoi pousser une couleur de repère pour la rendre plus visible : le blanc sur un thème
/// sombre, le noir sur un thème clair.
fn anti_de(claire: bool) -> Color32 {
    if claire { Color32::BLACK } else { Color32::WHITE }
}

thread_local! {
    static COURANTE: RefCell<Palette> = RefCell::new(Palette::depuis(&Couleurs::default()));
}

/// Installe le thème : les couleurs que lisent les panneaux changent aussitôt. Les `Visuals`
/// d'egui suivent avec `appliquer`.
pub fn installer(reglage: &ReglageTheme) {
    let palette = Palette::depuis(&reglage.couleurs());
    COURANTE.with(|p| *p.borrow_mut() = palette);
}

/// La palette courante, en entier (à lire une fois pour plusieurs couleurs).
pub fn palette() -> Palette {
    COURANTE.with(|p| *p.borrow())
}

macro_rules! couleurs_courantes {
    ($($(#[$doc:meta])* $nom:ident),* $(,)?) => {
        $(
            $(#[$doc])*
            // Toute la palette se lit par là, même les couleurs que rien n'utilise encore.
            #[allow(dead_code)]
            pub fn $nom() -> Color32 {
                COURANTE.with(|p| p.borrow().$nom)
            }
        )*
    };
}

couleurs_courantes! {
    /// Autour de la feuille et entre les panneaux.
    fond,
    /// Barres d'onglets et barres d'outils.
    entete,
    /// Fond des panneaux.
    panneau,
    /// Fond des fenêtres et des menus.
    fenetre,
    /// Onglet actif, ligne choisie, survol léger.
    eleve,
    /// Traits de séparation.
    bord,
    /// Bordures des champs et des commandes, anneau de focus.
    bord_fort,
    focus,
    /// Champs de saisie.
    champ,
    /// Boutons au repos, survolés, enfoncés.
    controle,
    controle_survol,
    controle_actif,
    /// Texte, texte secondaire, texte fort.
    texte,
    texte_doux,
    texte_fort,
    /// Accent (fonds), ce qui s'écrit dessus, accent des traits et des textes.
    accent,
    sur_accent,
    selection,
    sur_selection,
    accent_texte,
    /// Le rehaut jade de la marque : trait sous l'espace actif, icône de « Nouveau projet ».
    rehaut,
    /// Règle des images (au-dessus des rangées).
    regle,
    /// Rangée d'un calque ; rangées de la caméra, du son et des pistes, plus sombres.
    rangee,
    rangee_sombre,
    /// Étiquette d'une rangée dans la colonne des noms.
    etiquette,
    /// Graduations : une image, une seconde.
    graduation,
    graduation_seconde,
    /// Bloc d'un panneau dans le montage, et sa poignée de durée.
    bloc,
    poignee,
    /// Fond de l'aperçu du trait (réglages du pinceau).
    apercu_trait,
    /// Contour des éléments choisis.
    contour,
    /// Encre sur les dessins vifs, fond des pastilles posées sur une rangée.
    encre,
    /// Texte écrit sur les blocs sourds et les extraits son.
    sur_calque,
    /// Nom écrit sur le fond sourd d'une sorte de calque.
    sur_sorte,
    /// Dessins exposés (timeline) : vectoriel, bitmap, vide.
    dessin_vectoriel,
    dessin_bitmap,
    dessin_vide,
    /// Les mêmes, plus sourds, sous les panneaux du montage.
    calque_vectoriel,
    calque_bitmap,
    /// Calque de couleur : rose, vif et sourd.
    dessin_couleur,
    calque_couleur,
    /// Calque d'effet : cyan, vif et sourd.
    dessin_cfx,
    calque_cfx,
    /// Calque d'annotation : rouge-rose, vif et sourd.
    dessin_annotation,
    calque_annotation,
    /// Calque « Couleur unie » : vert citron, vif et sourd.
    dessin_unie,
    calque_unie,
    /// Calques de toute la scène (le décor) : le nom, puis les dessins sous les plans du montage.
    decor,
    calque_decor,
    /// Image courante, clés de caméra, marques d'image, clés d'effets.
    image_courante,
    camera,
    marque,
    effet,
    /// Son : extrait, extrait choisi, forme d'onde, piste muette.
    son_extrait,
    son_choisi,
    son_onde,
    son_muet,
    /// Messages d'avertissement et d'erreur.
    avertissement,
    erreur,
    succes,
    /// Jeton `alerte` de la marque, pastille `braise`.
    alerte,
    braise,
}

/// Couleur vive d'une sorte de calque (repères, barres, étiquettes) ; `couleur` : le calque est un calque de couleur.
pub fn couleur_calque(kind: LayerKind, couleur: bool) -> Color32 {
    let p = palette();
    match (couleur, kind) {
        (true, _) => p.dessin_couleur,
        (false, LayerKind::Vector) => p.dessin_vectoriel,
        (false, LayerKind::Bitmap) => p.dessin_bitmap,
    }
}

/// La couleur vive d'un calque : celle de sa sorte, ou le cyan d'un calque d'effet.
pub fn couleur_du_calque(l: &crate::document::Layer) -> Color32 {
    if l.effet.is_some() {
        palette().dessin_cfx
    } else if l.annotation {
        palette().dessin_annotation
    } else if l.unie.is_some() {
        palette().dessin_unie
    } else {
        couleur_calque(l.kind, l.couleur.is_some())
    }
}

/// La même, sourde (voir `fond_calque`), pour un calque.
pub fn fond_du_calque(l: &crate::document::Layer) -> Color32 {
    if l.effet.is_some() {
        palette().calque_cfx
    } else if l.annotation {
        palette().calque_annotation
    } else if l.unie.is_some() {
        palette().calque_unie
    } else {
        fond_calque(l.kind, l.couleur.is_some())
    }
}

/// La même, sourde : fond des dessins sous les panneaux du montage, où un nom s'écrit dessus.
pub fn fond_calque(kind: LayerKind, couleur: bool) -> Color32 {
    let p = palette();
    match (couleur, kind) {
        (true, _) => p.calque_couleur,
        (false, LayerKind::Vector) => p.calque_vectoriel,
        (false, LayerKind::Bitmap) => p.calque_bitmap,
    }
}

/// Contraste minimal (WCAG) entre le trait de l'aperçu et son fond : en dessous, le trait se
/// perdrait dans le gris.
pub const CONTRASTE_APERCU: f32 = 1.8;

/// Le fond de l'aperçu du trait pour un trait de cette couleur : le gris `apercu_trait`, ou, si le
/// trait lui ressemble trop (une brosse grise, la gomme), ce gris poussé vers le blanc ou le noir —
/// celui qui fait le mieux ressortir le trait — jusqu'à `CONTRASTE_APERCU`.
pub fn fond_apercu_trait(trait_: Color32) -> Color32 {
    let base = palette().apercu_trait;
    if contraste(trait_, base) >= CONTRASTE_APERCU {
        return base;
    }
    let pole = if contraste(trait_, Color32::WHITE) >= contraste(trait_, Color32::BLACK) { Color32::WHITE } else { Color32::BLACK };
    assurer(base, &[trait_], CONTRASTE_APERCU, pole)
}

/// Barres des séquences et des scènes, alternées pour qu'on voie les limites.
pub fn barre(sequence: bool, impaire: bool) -> Color32 {
    palette().barres[match (sequence, impaire) {
        (true, false) => 0,
        (true, true) => 1,
        (false, false) => 2,
        (false, true) => 3,
    }]
}

// ------------------------------------------------------------------- surimpressions du dessin

// Couleurs posées PAR-DESSUS le dessin : poignées et contours de sélection, cadre de la caméra,
// aides au cadrage. Elles ne suivent pas les neuf couleurs de l'interface, parce qu'elles doivent se
// voir sur le papier du projet, quel qu'il soit (blanc, noir, couleur, damier) ; mais chacune n'est
// écrite qu'ici (avant, le même bleu était recopié dans six fichiers). La caméra prend la couleur
// de la palette (`camera()`), comme ses clés dans la timeline.

/// Poignées, contours et lasso de la sélection, de la boîte de transformation et de la 3D.
pub const POIGNEE_SELECTION: Color32 = Color32::from_rgb(70, 165, 255);
// Sélection (captures d'Anthony, 2026-10-22). Tout le style de la
// sélection est ici : pour le changer, on ne touche qu'à ces constantes (couleurs) et à
// `selbox::HANDLE_SIDE` (taille des poignées).
/// La boîte (1 px, pleine), le bord des poignées, le pivot, le halo des traits choisis.
pub const SELECTION_BOITE: Color32 = Color32::from_rgb(232, 120, 16);
/// Le fond des poignées carrées.
pub const SELECTION_POIGNEE_FOND: Color32 = Color32::WHITE;
/// Le lasso en cours : une ligne beige très fine…
pub const LASSO_TRAIT: Color32 = Color32::from_rgb(224, 200, 176);
/// … semée de petits points ambrés.
pub const LASSO_POINTS: Color32 = Color32::from_rgb(192, 128, 0);

/// Poignées de l'outil Déformer (enveloppe de Bézier).
pub const POIGNEE_DEFORMER: Color32 = Color32::from_rgb(255, 150, 60);
/// Boîte de l'outil Transformer le calque.
pub const POIGNEE_CALQUE: Color32 = Color32::from_rgb(120, 220, 140);
/// Le losange d'une clé d'opacité (`opacitecles.rs`) : gris clair, comme un verre.
pub const CLE_OPACITE: Color32 = Color32::from_rgb(214, 218, 228);
/// Zones de sécurité (action, titres).
pub const CADRAGE_SECURITE: Color32 = Color32::from_rgb(0, 165, 195);
// (Le cadre d'un format de diffusion a sa couleur dans les réglages depuis le 2026-10-28 : `Settings::cadre_format_couleur`.)

// ------------------------------------------------------------------- egui

/// Couleurs des fenêtres, panneaux et contrôles du thème courant.
pub fn visuels() -> egui::Visuals {
    palette().visuels()
}

/// Interface dense : espacements serrés, textes un peu plus petits qu'egui par défaut.
#[allow(dead_code)] // (les essais d'une fenêtre seule, sans polices Plex)
pub fn densifier(style: &mut egui::Style) {
    densifier_avec(style, false);
}

/// Comme `densifier` ; `plex` : les polices d'IBM Plex sont chargées dans ce contexte (`marque::polices_pretes`), donc les
/// boutons et les titres peuvent prendre leurs familles Medium et SemiBold (sans elles, egui refuserait la famille).
pub fn densifier_avec(style: &mut egui::Style, plex: bool) {
    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(6.0, 3.0);
    s.button_padding = Vec2::new(7.0, 2.0);
    s.interact_size = Vec2::new(32.0, 18.0);
    s.indent = 14.0;
    s.slider_width = 110.0;
    s.window_margin = Margin::same(6);
    s.menu_margin = Margin::same(4);
    s.scroll = egui::style::ScrollStyle::thin();
    // Bulles d'aide : seulement quand la souris reste une seconde sur un bouton (12d).
    style.interaction.tooltip_delay = crate::bulles::DELAI;
    let p = FontFamily::Proportional;
    style.text_styles = [
        (TextStyle::Small, FontId::new(10.5, p.clone())),
        (TextStyle::Body, FontId::new(12.5, p.clone())),
        (TextStyle::Button, FontId::new(12.5, if plex { FontFamily::Name(crate::marque::FAMILLE_MEDIUM.into()) } else { p.clone() })),
        (TextStyle::Heading, FontId::new(16.0, if plex { FontFamily::Name(crate::marque::FAMILLE_SEMIBOLD.into()) } else { p })),
        (TextStyle::Monospace, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into();
}

/// Segoe UI (police de Windows) en tête de la famille proportionnelle, si elle est là : les
/// polices d'egui restent derrière (caractères manquants, icônes). Vrai si elle a été ajoutée.
pub fn polices(fonts: &mut FontDefinitions) -> bool {
    let dossier = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
    let Ok(octets) = std::fs::read(dossier.join("Fonts").join("segoeui.ttf")) else { return false };
    fonts.font_data.insert("segoe-ui".into(), std::sync::Arc::new(FontData::from_owned(octets)));
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "segoe-ui".into());
    true
}

/// Onglets et séparations des panneaux (egui_dock) : barres d'onglets, onglet actif de la couleur
/// du panneau, séparations du fond qui prennent l'accent au survol.
pub fn dock(mut s: egui_dock::Style) -> egui_dock::Style {
    let p = palette();
    s.main_surface_border_stroke = Stroke::NONE;
    s.separator.width = 4.0;
    // Le bord entre deux panneaux se voit sans le survol (la poignée du montage, `montagehauteur.rs`) : un peu plus clair que
    // le fond, et large à saisir.
    s.separator.color_idle = melange(p.fond, p.texte_fort, 0.14);
    s.separator.extra_interact_width = 6.0;
    s.separator.color_hovered = p.accent_texte;
    s.separator.color_dragged = p.accent_texte;
    s.tab_bar.bg_fill = p.entete;
    s.tab_bar.hline_color = p.entete;
    s.tab_bar.height = 26.0;
    // Des onglets qui ne tiennent plus dans un panneau rétréci : une petite barre pour les
    // atteindre (Anthony, 2026-10-17 : « toujours utilisables même lorsqu'ils sont réduits »).
    s.tab_bar.show_scroll_bar_on_overflow = true;
    s.tab_bar.corner_radius = CornerRadius::ZERO;
    let onglet = |fond: Color32, texte: Color32| egui_dock::TabInteractionStyle {
        outline_color: Color32::TRANSPARENT,
        corner_radius: CornerRadius::ZERO,
        bg_fill: fond,
        text_color: texte,
    };
    s.tab.inactive = onglet(p.entete, p.texte_doux);
    s.tab.hovered = onglet(p.entete, p.texte);
    s.tab.active = onglet(p.panneau, p.texte);
    s.tab.focused = onglet(p.panneau, p.texte_fort);
    s.tab.inactive_with_kb_focus = s.tab.inactive.clone();
    s.tab.active_with_kb_focus = s.tab.active.clone();
    s.tab.focused_with_kb_focus = s.tab.focused.clone();
    s.tab.hline_below_active_tab_name = false;
    s.tab.tab_body.bg_fill = p.panneau;
    s.tab.tab_body.stroke = Stroke::NONE;
    s.tab.tab_body.corner_radius = CornerRadius::ZERO;
    s.buttons.close_tab_color = p.texte_doux;
    s.buttons.close_tab_active_color = p.texte_fort;
    s.buttons.close_tab_bg_fill = p.controle_survol;
    s.overlay.selection_color = p.accent.gamma_multiply(0.5);
    s
}

/// Le trait jade sous l'espace de travail actif (Vignettes, Storyboard, Animation, Montage) : deux points de
/// haut, juste sous son étiquette (sans toucher le mot), dans un thème de la marque (Atelier, Papier) et dans Gris sombre ; les autres n'ont rien.
pub fn souligner_actif(ui: &egui::Ui, etiquette: egui::Rect) {
    let p = palette();
    if p.identite || p.jade_discret {
        let trait_ = egui::Rect::from_min_max(egui::pos2(etiquette.left() + 3.0, etiquette.bottom() - 0.5), egui::pos2(etiquette.right() - 3.0, etiquette.bottom() + 1.5));
        ui.painter().rect_filled(trait_, 0.0, p.rehaut);
    }
}
/// Applique le thème courant à egui : couleurs, densité. À refaire quand le thème change
/// (`installer` d'abord).
///
/// **Le thème d'egui est celui de SHOT, jamais celui de Windows** (2026-10-29). egui garde deux styles (sombre et
/// clair) et prend celui du système tant que sa préférence est « Système » (c'est le départ) ; eframe donne le thème de
/// Windows à la première image. Avant, on ne posait que le style actif au moment du lancement (le sombre, le système
/// n'étant pas encore connu) : sur un poste dont Windows est en thème clair, egui passait au style clair d'origine à la
/// première image, avec ses fonds blancs et ses textes pâles, pendant que les panneaux de SHOT, qui lisent la palette
/// directement, restaient sombres. Maintenant la préférence est fixée (sombre, ou clair pour un thème clair) et les deux
/// styles reçoivent les mêmes couleurs : quel que soit le thème que Windows annonce, l'interface est d'un seul thème.
pub fn appliquer(ctx: &egui::Context) {
    let claire = palette().claire;
    ctx.set_theme(if claire { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
    let visuels = visuels();
    let plex = crate::marque::polices_pretes(ctx);
    ctx.all_styles_mut(|style| {
        style.visuals = visuels.clone();
        densifier_avec(style, plex);
    });
}

/// Un avertissement non bloquant. Thèmes de la marque : une petite pastille braise puis le texte, en encre neutre (jamais
/// de texte braise). Autres thèmes : le texte coloré d'avant.
pub fn avertir(ui: &mut egui::Ui, texte: impl Into<String>) -> egui::Response {
    avertir_en(ui, texte.into(), false)
}

/// Comme `avertir`, en petit.
pub fn avertir_petit(ui: &mut egui::Ui, texte: impl Into<String>) -> egui::Response {
    avertir_en(ui, texte.into(), true)
}

fn avertir_en(ui: &mut egui::Ui, texte: String, petit: bool) -> egui::Response {
    let p = palette();
    let corps = |t: String, c: Color32| {
        let r = egui::RichText::new(t).color(c);
        if petit { r.small() } else { r }
    };
    if !p.identite {
        return ui.label(corps(texte, p.avertissement));
    }
    // La pastille est posée à la première ligne ; le texte passe à la ligne dans la place qui reste.
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (r, _) = ui.allocate_exact_size(Vec2::new(8.0, 13.0), egui::Sense::hover());
        ui.painter().circle_filled(egui::pos2(r.center().x, r.top() + 7.0), 3.5, p.braise);
        ui.add(egui::Label::new(corps(texte, p.texte_fort)).wrap());
    })
    .response
}

/// Le fond d'une grande surface active (bloc d'un plan du montage, rangée choisie de la timeline, bande de séquence) :
/// `selection` (le jade) à la force `force` (0 à 1) dans les thèmes qui n'ont pas la marque ; dans Atelier et Papier, pas de
/// grand aplat jade près du dessin : le panneau tirant vers `ligne forte` (le jade ne reste que sur les petits éléments et
/// sur le bord).
pub fn surface_active(selection: Color32, force: f32) -> Color32 {
    let p = palette();
    if p.identite {
        melange(p.panneau, p.bord_fort, (force * 0.9).clamp(0.0, 1.0))
    } else {
        selection.gamma_multiply(force)
    }
}
