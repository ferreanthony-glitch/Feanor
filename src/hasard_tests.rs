//! Essais au hasard des gestes (« fuzz », passe de débogage du 2026-10-22, Anthony : « tu peux me faire une passe de
//! débogage ? »). L'application tourne pour de vrai sur le banc (`perf_tests::Banc` : de vraies images d'écran, de vrais
//! événements d'egui) et reçoit une suite de gestes tirés d'un hasard reproductible : des plans qu'on crée, supprime,
//! duplique, réunit, coupe et déplace ; des barres de coupe des Vignettes ; des traits dans le dessin et dans les cases
//! (marges qui débordent et bande de note comprises) ; le lasso ; des calques ; le Décor ; la caméra des Vignettes
//! (Ctrl + Maj, Ctrl + Alt + Maj, « Retirer ce cadre ») ; les clés de la timeline ; annuler et rétablir ; les espaces
//! de travail ; les onglets ; enregistrer et rouvrir ; des touches (Suppr, flèches, Ctrl+Z…), la molette, les colonnes
//! de la planche ; et des gestes interrompus : une commande, un espace, un onglet ou un enregistrement pendant que le
//! bouton est tenu (`Geste::Interruption`).
//!
//! Après chaque geste, l'état est vérifié (`Essai::verifier`) : pas de plantage, plan et calque courants valables,
//! sélections qui désignent des choses qui existent, planche sans chevauchement et dans l'ordre de lecture, chaque case
//! sur un plan qui existe, dessins référencés présents. Un geste qui fait une étape d'annulation est annulé puis
//! rétabli : le projet doit redevenir exactement celui d'avant, puis celui d'après (`instantane`). Un enregistrement
//! se relit à l'identique.
//!
//! Un échec affiche la graine, le pas et la suite des gestes (en Rust : elle se recopie telle quelle dans un essai de
//! non-régression, `rejouer`), après l'avoir réduite au plus court qui échoue encore (`reduire`).
//!
//! Le passage court tourne avec la suite (`GRAINES` × `PAS`) ; le long se lance à la demande (`GRAINES_LONGUES` ×
//! `PAS_LONGS`, sur quatre fils) :
//! `cargo test --release --offline --target-dir target\msvc hasard_long -- --ignored --nocapture`.
//! `SHOT_HASARD_GRAINES=300..400`, `SHOT_HASARD_PAS=80`, `SHOT_HASARD_FILS=6` changent les graines, la longueur et les
//! fils ; `SHOT_HASARD_ETAPES=1` affiche les étapes d'annulation faites par graine (ce que le hasard atteint),
//! `SHOT_HASARD_TEMPS=1` le temps de chaque geste.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{epaint::Shape, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, Rect};

use crate::app::{BoardView, Tool};
use crate::document::{CelId, Document};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

/// Passage court (dans la suite) : graines et gestes par graine.
pub(crate) const GRAINES: u64 = 4;
pub(crate) const PAS: usize = 40;
/// Passage long (`--ignored`) : graines et gestes par graine.
pub(crate) const GRAINES_LONGUES: u64 = 150;
pub(crate) const PAS_LONGS: usize = 120;
/// La réduction d'un échec s'arrête au bout de ce temps (elle rejoue la suite bien des fois).
const REDUCTION_MAX: Duration = Duration::from_secs(900);

// ------------------------------------------------------------------ le hasard

/// Un hasard reproductible (xorshift64*).
pub(crate) struct Hasard(u64);

impl Hasard {
    pub fn new(graine: u64) -> Hasard {
        Hasard(graine.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03 | 1)
    }

    pub fn suivant(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// De 0 à `n` (exclu).
    pub fn sous(&mut self, n: usize) -> usize {
        (self.suivant() % n.max(1) as u64) as usize
    }

    /// De `a` à `b`, au centième (la suite s'affiche courte).
    pub fn entre(&mut self, a: f32, b: f32) -> f32 {
        let t = (self.suivant() >> 40) as f32 / (1u64 << 24) as f32;
        ((a + (b - a) * t) * 100.0).round() / 100.0
    }

    pub fn pile(&mut self, sur: usize) -> bool {
        self.sous(sur) == 0
    }
}

// ------------------------------------------------------------------ les gestes

/// Les commandes que le hasard lance (`App::run_action`) : aucune n'ouvre de fenêtre de fichier de Windows.
pub(crate) const COMMANDES: &[&str] = &[
    // Plans.
    "panneau.nouveau",
    "panneau.dupliquer",
    "panneau.supprimer",
    "scene.joindre_precedente",
    "panneau.couper",
    "plan.avancer",
    "plan.reculer",
    "panneau.fusionner_suivant",
    "panneau.fusionner_choisis",
    "sequence.nouvelle",
    "plan.nouveau_avant",
    "marque.basculer",
    "panneau.ajouter_image",
    "panneau.retirer_image",
    "panneau.precedent",
    "panneau.suivant",
    "panneau.copier",
    "panneau.coller",
    "storyboard.verrouiller",
    "version.nouvelle",
    "version.precedente",
    "version.suivante",
    "transition.ajouter",
    "transition.supprimer",
    // Calques.
    "calque.nouveau_bitmap",
    "calque.nouveau_vectoriel",
    "calque.supprimer",
    "calque.monter",
    "calque.descendre",
    "calque.dupliquer",
    "calque.dessus",
    "calque.dessous",
    "calque.fusionner_dessous",
    "calque.choisir_tout",
    "calque.choisir_aucun",
    "calque.choisir_dessus",
    "calque.copier",
    "calque.coller",
    "calque.supprimer_vides",
    "dossier.nouveau",
    "dossier.ranger",
    "dossier.sortir",
    "dossier.supprimer_seul",
    "piste.basculer",
    "piste.replier_tout",
    "piste.deplier_tout",
    // Timeline : images et clés.
    "image.suivante",
    "image.precedente",
    "image.premiere",
    "image.derniere",
    "dessin.nouveau",
    "dessin.dupliquer",
    "dessin.nouvelle_cle",
    "dessin.nouvelle_cle_vide",
    "dessin.supprimer",
    "dessin.vider",
    "exposition.allonger",
    "exposition.raccourcir",
    "instance.casser",
    "images.ajouter",
    "images.inserer",
    "images.supprimer_plan",
    "images.tout_choisir",
    "images.supprimer",
    "images.copier",
    "images.coller",
    "images.dupliquer",
    "camera.cle_ici",
    "camera.cle_supprimer",
    "calque.cle_position",
    "calque.cle_position_supprimer",
    "signet.basculer",
    // Édition.
    "edition.tout_selectionner",
    "edition.tout_deselectionner",
    "edition.supprimer",
    "edition.copier",
    "edition.coller",
    "edition.couper",
];

/// Les entrées du menu d'une case (clic droit) que le hasard peut cliquer.
pub(crate) const MENU_CASE: &[&str] = &["Retirer ce cadre", "Ajouter un cadre", "Nouveau plan après", "Dupliquer le plan", "Faire de cette image un nouveau plan", "Réunir avec le plan précédent"];

/// Les outils d'un trait.
const OUTILS: [Tool; 11] = [Tool::Brush, Tool::Pencil, Tool::Eraser, Tool::Brush, Tool::Pencil, Tool::Fill, Tool::Line, Tool::Rect, Tool::Ellipse, Tool::Select, Tool::Cutter];

/// Un geste. Ses nombres sont relatifs (une case parmi celles qu'on voit, une fraction de l'image) : une suite se
/// rejoue telle quelle sur un autre état, ce qui permet de la réduire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Geste {
    /// Une commande de `COMMANDES`.
    Commande(&'static str),
    /// Un espace de travail (`Espace::TOUS`).
    Espace(u8),
    /// Un trait dans le dessin en grand (fractions de la vue), avec l'outil `outil` de `OUTILS`.
    TraitDessin { outil: u8, x0: f32, y0: f32, x1: f32, y1: f32 },
    /// Un trait dans une case des Vignettes : départ en fractions de l'image (au-delà de 0..1 : la marge, la bande de
    /// note), déplacement en largeurs d'image.
    TraitCase { case: u16, outil: u8, x0: f32, y0: f32, dx: f32, dy: f32 },
    /// Un trait dans une case pâle (un plan neuf).
    TraitCaseVide { e: u8, dx: f32, dy: f32 },
    /// Un clic sur une barre de coupe.
    Barre { k: u16 },
    /// La note d'une case : l'icône, un texte, Entrée (ou Échap).
    Note { case: u16, texte: u8, valider: bool },
    /// Le lasso dans le dessin en grand (outil Sélection), puis déplacer ce qu'il a pris, puis peut-être supprimer.
    LassoDessin { x0: f32, y0: f32, x1: f32, y1: f32, dx: f32, dy: f32, supprimer: bool },
    /// Ctrl + glisser dans une case (le lasso des Vignettes), puis peut-être supprimer.
    LassoCase { case: u16, x0: f32, y0: f32, x1: f32, y1: f32, supprimer: bool },
    /// Un trait de Décor dans une case (`Genre::ALL[genre]`), puis peut-être lâcher le décor.
    Decor { genre: u8, case: u16, x0: f32, y0: f32, dx: f32, dy: f32, lacher: bool },
    /// Ctrl + Maj (Alt : un cadre de plus) + glisser dans une case : un mouvement de caméra.
    Camera { case: u16, x0: f32, y0: f32, dx: f32, dy: f32, ajoute: bool },
    /// Clic droit sur une case (sur son cadre de rang `cadre`, s'il y en a) puis une entrée de `MENU_CASE`.
    MenuCase { case: u16, cadre: u8, entree: u8 },
    /// Maj (+ Alt : une copie) + glisser le cadre d'une vignette jusqu'à une autre : déplacer le plan.
    DeplacerPlan { de: u16, vers: u16, alt: bool },
    /// Annuler `n` fois.
    Annuler(u8),
    /// Rétablir `n` fois.
    Retablir(u8),
    /// Lecture pendant `n` images, puis pause.
    Lecture(u8),
    /// Un onglet de plus (avec un petit projet) ou l'onglet voisin.
    Onglet { nouveau: bool },
    /// Enregistrer dans un dossier neuf et relire ; `rouvrir` : rouvrir aussi la copie dans un onglet.
    Enregistrer { rouvrir: bool },
    /// Une touche de `TOUCHES` (Suppr, flèches, Ctrl+Z…).
    Touche(u8),
    /// Un clic (outil Sélection) sur une case, Maj tenue ou non : choisir des plans.
    ChoisirCase { case: u16, maj: bool },
    /// La molette sur la planche (Ctrl : le zoom).
    Molette { dy: f32, ctrl: bool },
    /// Le nombre de colonnes de la planche (2 à 6).
    Colonnes(u8),
    /// Un geste interrompu : appui et début du glissé (`geste` : trait dans une case, caméra, plan déplacé, trait dans le
    /// dessin, lasso, Décor), puis `INTERRUPTIONS[quoi]` pendant que le bouton est tenu, puis la fin du glissé et le lâcher.
    Interruption { geste: u8, case: u16, quoi: u8, dx: f32, dy: f32 },
}

/// Ce qui arrive pendant un geste (une touche de raccourci tenue, un clic ailleurs…) : une commande, un espace de travail
/// (`espace:`), l'onglet voisin (`onglet`), enregistrer (`enregistrer`).
const INTERRUPTIONS: &[&str] = &[
    "edition.annuler",
    "edition.retablir",
    "panneau.supprimer",
    "panneau.nouveau",
    "panneau.dupliquer",
    "scene.joindre_precedente",
    "panneau.couper",
    "plan.avancer",
    "plan.reculer",
    "panneau.suivant",
    "panneau.precedent",
    "sequence.nouvelle",
    "calque.supprimer",
    "calque.nouveau_bitmap",
    "calque.dessous",
    "image.suivante",
    "images.supprimer_plan",
    "panneau.retirer_image",
    "marque.basculer",
    "lecture.basculer",
    "storyboard.verrouiller",
    "version.nouvelle",
    "espace:0",
    "espace:2",
    "espace:3",
    "onglet",
    "enregistrer",
];

/// Les touches que le hasard presse : aucune n'ouvre de fenêtre de fichier (pas de Ctrl+S, Ctrl+O…).
const TOUCHES: [(Key, Modifiers); 12] = [
    (Key::Delete, Modifiers::NONE),
    (Key::Backspace, Modifiers::NONE),
    (Key::ArrowLeft, Modifiers::NONE),
    (Key::ArrowRight, Modifiers::NONE),
    (Key::ArrowUp, Modifiers::NONE),
    (Key::ArrowDown, Modifiers::NONE),
    (Key::Escape, Modifiers::NONE),
    (Key::Enter, Modifiers::NONE),
    (Key::Z, CTRL),
    (Key::Y, CTRL),
    (Key::Z, Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE }),
    (Key::A, CTRL),
];

/// Ctrl sous Windows (egui pose aussi `command`).
const CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };

use Geste::*;

impl Geste {
    pub(crate) fn tirer(h: &mut Hasard) -> Geste {
        let case = h.sous(64) as u16;
        match h.sous(124) {
            0..=21 => Commande(COMMANDES[h.sous(COMMANDES.len())]),
            22..=25 => Espace(h.sous(4) as u8),
            26..=33 => TraitDessin { outil: h.sous(OUTILS.len()) as u8, x0: h.entre(0.05, 0.95), y0: h.entre(0.05, 0.95), x1: h.entre(-0.2, 1.2), y1: h.entre(-0.2, 1.2) },
            34..=45 => TraitCase { case, outil: h.sous(OUTILS.len()) as u8, x0: h.entre(-0.12, 1.12), y0: h.entre(-0.12, 1.35), dx: h.entre(-1.3, 1.3), dy: h.entre(-1.0, 1.0) },
            46..=48 => TraitCaseVide { e: h.sous(3) as u8, dx: h.entre(-0.6, 0.6), dy: h.entre(-0.4, 0.4) },
            49..=53 => Barre { k: case },
            54..=55 => Note { case, texte: h.sous(4) as u8, valider: !h.pile(4) },
            56..=58 => LassoDessin { x0: h.entre(0.1, 0.5), y0: h.entre(0.1, 0.5), x1: h.entre(0.5, 0.95), y1: h.entre(0.5, 0.95), dx: h.entre(-0.3, 0.3), dy: h.entre(-0.3, 0.3), supprimer: h.pile(2) },
            59..=60 => LassoCase { case, x0: h.entre(0.0, 0.5), y0: h.entre(0.0, 0.5), x1: h.entre(0.5, 1.0), y1: h.entre(0.5, 1.0), supprimer: h.pile(2) },
            61..=64 => Decor { genre: h.sous(3) as u8, case, x0: h.entre(0.0, 1.0), y0: h.entre(0.0, 1.0), dx: h.entre(-0.8, 0.8), dy: h.entre(-0.6, 0.6), lacher: h.pile(3) },
            65..=72 => Camera { case, x0: h.entre(0.1, 0.9), y0: h.entre(0.1, 0.9), dx: h.entre(-1.2, 1.2), dy: h.entre(-0.8, 0.8), ajoute: h.pile(3) },
            73..=78 => MenuCase { case, cadre: h.sous(4) as u8, entree: h.sous(MENU_CASE.len()) as u8 },
            79..=81 => DeplacerPlan { de: case, vers: h.sous(64) as u16, alt: h.pile(4) },
            82..=87 => Annuler(1 + h.sous(3) as u8),
            88..=91 => Retablir(1 + h.sous(2) as u8),
            92..=93 => Lecture(2 + h.sous(10) as u8),
            94..=95 => Onglet { nouveau: h.pile(2) },
            96..=97 => Enregistrer { rouvrir: h.pile(2) },
            98..=103 => Touche(h.sous(TOUCHES.len()) as u8),
            104..=108 => ChoisirCase { case, maj: h.pile(2) },
            109..=110 => Molette { dy: h.entre(-240.0, 240.0), ctrl: h.pile(2) },
            111 => Colonnes(h.sous(5) as u8),
            _ => Interruption { geste: h.sous(6) as u8, case, quoi: h.sous(INTERRUPTIONS.len()) as u8, dx: h.entre(-1.0, 1.0), dy: h.entre(-0.7, 0.7) },
        }
    }

    /// Un geste dont l'étape d'annulation se vérifie (annuler, puis rétablir).
    fn verifie_l_annulation(&self) -> bool {
        match self {
            Annuler(_) | Retablir(_) | Onglet { .. } | Enregistrer { .. } | Espace(_) | Lecture(_) => false,
            // (Enregistrer date le projet, `Properties::modified`, ce qu'aucune annulation ne défait.)
            Interruption { quoi, .. } => !matches!(INTERRUPTIONS[*quoi as usize % INTERRUPTIONS.len()], "edition.annuler" | "edition.retablir" | "onglet" | "enregistrer"),
            _ => true,
        }
    }

    /// Un geste qui ne doit pas changer le projet sans étape d'annulation (annuler, rétablir et leurs touches défont ou
    /// refont sans en prendre ; un enregistrement date le projet ; un onglet en montre un autre).
    fn change_avec_une_etape(&self) -> bool {
        match self {
            Touche(n) => !matches!(TOUCHES[*n as usize % TOUCHES.len()].0, Key::Z | Key::Y),
            Espace(_) | Lecture(_) => true,
            _ => self.verifie_l_annulation(),
        }
    }
}

// ------------------------------------------------------------------ un essai

/// Les documents de départ : petits, faits dans le code.
pub(crate) fn depart(graine: u64) -> Document {
    match graine % 4 {
        0 => crate::vignettes_tests::quatre_plans(),
        1 => crate::vignettes_camera_tests::plan_court(),
        2 => petit_projet(5, 2, 6),
        // (Deux séquences, trois calques : la planche ne tient plus à l'écran.)
        _ => petit_projet(14, 3, 8),
    }
}

/// Comme `perf_tests::projet_long`, sur une petite feuille (l'instantané de chaque geste relit tous les pixels) : `plans` plans
/// (une séquence de plus tous les dix), `calques` calques bitmap de `images` images, un dessin neuf toutes les deux images sur le
/// premier, tenu sur les autres.
fn petit_projet(plans: usize, calques: usize, images: usize) -> Document {
    use crate::document::{Cel, Layer, LayerKind};
    let mut doc = Document::with_frame(160, 90, 24);
    let mut at = 0;
    for i in 1..plans {
        at = if i % 10 == 0 { doc.add_sequence_after(at) } else { doc.add_scene_after(at) };
    }
    for p in 0..doc.panels.len() {
        let mut ls = Vec::new();
        for l in 0..calques {
            let mut layer = Layer::new(&format!("Calque {}", l + 1), LayerKind::Bitmap);
            let mut dernier = None;
            for f in 0..images {
                if f == 0 || (l == 0 && f % 2 == 0) {
                    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
                    let mut t = crate::raster::empty_tile();
                    for (k, px) in t.iter_mut().enumerate() {
                        if (k + p * 7 + l * 3 + f) % 9 < 2 {
                            *px = eframe::egui::Color32::from_rgba_premultiplied(30, (20 + p * 13) as u8, 40, 200);
                        }
                    }
                    let n = cel.raster.tiles.len();
                    cel.raster.tiles[(p + f) % n] = Some(t);
                    dernier = Some(doc.add_cel(cel));
                }
                layer.frames.push(dernier);
            }
            ls.push(layer);
        }
        doc.panels[p].layers = ls;
        doc.panels[p].length = images;
    }
    doc.into_plans();
    doc
}

/// Ce qui a mal tourné : l'espèce (pour reconnaître le même échec pendant la réduction) et le détail.
#[derive(Debug, Clone)]
pub(crate) struct Echec {
    pub espece: String,
    pub detail: String,
}

fn echec(espece: &str, detail: String) -> Echec {
    Echec { espece: espece.into(), detail }
}

/// Ce qui ne dépend pas d'une valeur : les chiffres retirés (pour reconnaître le même plantage).
fn sans_chiffres(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_digit()).take(120).collect()
}

pub(crate) struct Essai {
    pub b: Banc,
    dossier: PathBuf,
    enregistrements: usize,
    /// Les étapes d'annulation que les gestes ont faites, par nom (`SHOT_HASARD_ETAPES` les affiche) : ce que le hasard
    /// atteint vraiment.
    pub etapes: std::collections::BTreeMap<String, usize>,
}

const DT: f64 = 1.0 / 60.0;

fn appui(pos: Pos2, bouton: PointerButton, down: bool, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: bouton, pressed: down, modifiers }
}

fn touche(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers }
}

/// Les textes peints, avec leur centre (en entrant dans les formes groupées).
fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

/// Les dessins dont l'image ne se compare pas : les dessins vectoriels (leur image est refaite à la lecture et en
/// arrière-plan, au rendu exact ; leurs traits, eux, se comparent).
fn sans_pixels(doc: &Document) -> BTreeSet<CelId> {
    doc.cels.iter().filter(|(_, c)| c.vector.is_some()).map(|(id, _)| *id).collect()
}

/// L'instantané du projet (`fiabilite_fixture::instantane`), la liste des scènes rangée par numéro : son ordre en mémoire
/// ne dit rien (une scène se cherche par son numéro), le fichier les écrit dans l'ordre du film.
fn photo(doc: &Document) -> String {
    // (De même pour les séquences : leur numéro d'ordre se calcule sur les plans, `Document::sequence_order`.)
    let mut scenes = doc.scenes.clone();
    scenes.sort_by_key(|s| s.id);
    let mut sequences = doc.sequences.clone();
    sequences.sort_by_key(|s| s.id);
    let (scenes, sequences) = (format!("scenes {scenes:?}"), format!("sequences {sequences:?}"));
    instantane(doc, &sans_pixels(doc))
        .lines()
        .map(|l| if l.starts_with("scenes ") { scenes.as_str() } else if l.starts_with("sequences ") { sequences.as_str() } else { l })
        .collect::<Vec<_>>()
        .join("\n")
}

/// L'instantané de ce que l'enregistrement garde : la réserve d'un calque supprimé (le rognage, `rognage::to_bytes`) n'est pas
/// écrite. Elle reste en mémoire le temps d'une annulation ; on la retire le temps de l'instantané.
fn photo_enregistrable(doc: &mut Document) -> String {
    let gardees = doc.reserves.clone();
    for p in &doc.panels {
        if let Some(r) = doc.reserves.get_mut(&p.id) {
            for piece in [&mut r.head, &mut r.tail] {
                piece.layers.retain(|lp| p.layers.iter().any(|l| l.id == lp.layer));
            }
        }
    }
    let s = photo(doc);
    doc.reserves = gardees;
    s
}

impl Essai {
    pub(crate) fn new(doc: Document, nom: &str) -> Essai {
        let mut b = banc_taille(doc, false, 1600, 900);
        b.app.board_view = BoardView::Panels;
        b.app.layout.board_columns = 4;
        b.app.layout.board_rows = 0;
        b.app.layout.card_size = 170.0;
        b.app.board_zoom = 0.0;
        b.chauffer(4);
        let dossier = std::env::temp_dir().join(format!("shot-hasard-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).expect("dossier d'essai");
        Essai { b, dossier, enregistrements: 0, etapes: Default::default() }
    }

    fn img(&mut self, ev: Vec<Event>) {
        self.b.image(ev, DT);
    }

    /// Un glissé du bouton `bouton` avec `m` tenues, point par point.
    fn glisser(&mut self, m: Modifiers, bouton: PointerButton, chemin: &[Pos2]) {
        let (de, a) = (chemin[0], *chemin.last().expect("un point"));
        self.b.touches = m;
        self.img(vec![Event::PointerMoved(de)]);
        self.img(vec![appui(de, bouton, true, m)]);
        for q in &chemin[1..] {
            self.img(vec![Event::PointerMoved(*q)]);
        }
        self.img(vec![appui(a, bouton, false, m)]);
        self.b.touches = Modifiers::NONE;
        self.img(vec![Event::PointerGone]);
        self.b.chauffer(1);
    }

    fn clic(&mut self, p: Pos2, bouton: PointerButton, m: Modifiers) {
        self.glisser(m, bouton, &[p]);
    }

    fn trajet(de: Pos2, a: Pos2, n: usize) -> Vec<Pos2> {
        (0..=n)
            .map(|k| {
                let t = k as f32 / n as f32;
                de + (a - de) * t + vec2(0.0, (t * 7.0).sin() * 4.0)
            })
            .collect()
    }

    /// Les Vignettes à l'écran (l'espace Vignettes s'il le faut).
    fn vers_vignettes(&mut self) {
        if !self.b.app.board_shown() {
            self.b.app.set_espace(Espace::Vignettes);
            self.b.chauffer(3);
        }
        if !self.b.app.board_shown() {
            self.b.app.board_view = BoardView::Panels;
            self.b.chauffer(3);
        }
    }

    /// Le dessin en grand à l'écran.
    fn vers_dessin(&mut self) {
        if self.b.app.board_shown() || self.b.app.espace == Espace::Vignettes {
            self.b.app.set_espace(Espace::Storyboard);
            self.b.chauffer(3);
        }
        if self.b.app.board_shown() {
            self.b.app.board_view = BoardView::Scene;
            self.b.chauffer(3);
        }
    }

    /// Les cases dont l'image est entièrement à l'écran : (rang dans la planche, image à l'écran).
    fn cases_visibles(&self) -> Vec<(usize, Rect)> {
        let l = self.b.app.board_layout();
        let (xf, view) = (self.b.app.vignettes.xf, self.b.app.vignettes.view);
        l.cards.iter().enumerate().map(|(k, c)| (k, xf.rect(c.thumb))).filter(|(_, r)| view.contains_rect(*r) && r.width() > 8.0).collect()
    }

    fn case(&mut self, n: u16) -> Option<(usize, Rect)> {
        self.vers_vignettes();
        let v = self.cases_visibles();
        (!v.is_empty()).then(|| v[n as usize % v.len()])
    }

    /// La zone du dessin en grand, si elle est à l'écran.
    fn toile(&mut self) -> Option<Rect> {
        self.vers_dessin();
        let r = self.b.app.canvas_rect;
        let ecran = Rect::from_min_size(Pos2::ZERO, self.b.taille);
        (r.width() > 60.0 && r.height() > 60.0 && ecran.contains_rect(r)).then_some(r)
    }

    fn outil(&mut self, k: u8) {
        self.b.app.set_tool(OUTILS[k as usize % OUTILS.len()]);
    }

    /// Joue un geste.
    pub(crate) fn jouer(&mut self, g: Geste) {
        match g {
            Commande(id) => {
                self.b.app.run_action(id);
                self.b.chauffer(2);
            }
            Espace(k) => {
                self.b.app.set_espace(Espace::TOUS[k as usize % 4]);
                self.b.chauffer(3);
            }
            TraitDessin { outil, x0, y0, x1, y1 } => {
                let Some(r) = self.toile() else { return };
                self.outil(outil);
                let (de, a) = (r.lerp_inside(vec2(x0, y0)), r.lerp_inside(vec2(x1, y1)));
                let a = Rect::from_min_size(Pos2::ZERO, self.b.taille).shrink(2.0).clamp(a);
                self.glisser(Modifiers::NONE, PointerButton::Primary, &Self::trajet(de, a, 10));
            }
            TraitCase { case, outil, x0, y0, dx, dy } => {
                let Some((_, r)) = self.case(case) else { return };
                self.outil(outil);
                let de = r.min + vec2(x0 * r.width(), y0 * r.height());
                let a = de + vec2(dx, dy) * r.width();
                let a = self.b.app.vignettes.view.shrink(2.0).clamp(a);
                self.glisser(Modifiers::NONE, PointerButton::Primary, &Self::trajet(de, a, 10));
            }
            TraitCaseVide { e, dx, dy } => {
                self.vers_vignettes();
                let l = self.b.app.board_layout();
                let view = self.b.app.vignettes.view;
                let vides: Vec<Rect> = l.empty.iter().map(|r| self.b.app.vignettes.xf.rect(*r)).filter(|r| view.contains_rect(*r)).collect();
                if vides.is_empty() {
                    return;
                }
                let r = vides[e as usize % vides.len()];
                self.b.app.set_tool(Tool::Brush);
                let a = view.shrink(2.0).clamp(r.center() + vec2(dx, dy) * r.width());
                self.glisser(Modifiers::NONE, PointerButton::Primary, &Self::trajet(r.center(), a, 8));
            }
            Barre { k } => {
                self.vers_vignettes();
                let l = self.b.app.board_layout();
                let view = self.b.app.vignettes.view;
                let barres: Vec<Rect> = l.bars.iter().map(|b| self.b.app.vignettes.xf.rect(b.zone)).filter(|r| view.contains(r.center())).collect();
                if barres.is_empty() {
                    return;
                }
                let p = barres[k as usize % barres.len()].center();
                self.clic(p, PointerButton::Primary, Modifiers::NONE);
            }
            Note { case, texte, valider } => {
                let Some((k, _)) = self.case(case) else { return };
                let place = self.b.app.board_layout().cards[k];
                let p = self.b.app.note_icone_rect(&place).center();
                if !self.b.app.vignettes.view.contains(p) {
                    return;
                }
                self.clic(p, PointerButton::Primary, Modifiers::NONE);
                self.b.chauffer(2);
                let t = ["Plan large", "Elle se retourne, la porte claque derrière elle et le couloir reste vide un long moment.", "", "Contre-champ\nsur lui"][texte as usize % 4];
                self.img(vec![Event::Text(t.into())]);
                let k = if valider { Key::Enter } else { Key::Escape };
                self.img(vec![touche(k, true, Modifiers::NONE)]);
                self.img(vec![touche(k, false, Modifiers::NONE)]);
                self.b.chauffer(2);
            }
            LassoDessin { x0, y0, x1, y1, dx, dy, supprimer } => {
                let Some(r) = self.toile() else { return };
                self.b.app.set_tool(Tool::Select);
                let (de, a) = (r.lerp_inside(vec2(x0, y0)), r.lerp_inside(vec2(x1, y1)));
                let coin = pos2(a.x, de.y);
                let mut tour = Self::trajet(de, coin, 4);
                tour.extend(Self::trajet(coin, a, 4));
                tour.extend(Self::trajet(a, pos2(de.x, a.y), 4));
                tour.push(de);
                self.glisser(Modifiers::NONE, PointerButton::Primary, &tour);
                let m = de.lerp(a, 0.5);
                let vers = r.clamp(m + vec2(dx * r.width(), dy * r.height()));
                self.glisser(Modifiers::NONE, PointerButton::Primary, &Self::trajet(m, vers, 6));
                if supprimer {
                    self.b.app.run_action("edition.supprimer");
                    self.b.chauffer(2);
                }
            }
            LassoCase { case, x0, y0, x1, y1, supprimer } => {
                let Some((_, r)) = self.case(case) else { return };
                let (de, a) = (r.lerp_inside(vec2(x0, y0)), r.lerp_inside(vec2(x1, y1)));
                let coin = pos2(a.x, de.y);
                let mut tour = Self::trajet(de, coin, 3);
                tour.extend(Self::trajet(coin, a, 3));
                tour.extend(Self::trajet(a, pos2(de.x, a.y), 3));
                tour.push(de);
                self.glisser(CTRL, PointerButton::Primary, &tour);
                if supprimer {
                    self.b.app.run_action("edition.supprimer");
                    self.b.chauffer(2);
                }
            }
            Decor { genre, case, x0, y0, dx, dy, lacher } => {
                let Some((_, r)) = self.case(case) else { return };
                self.b.app.decor_choisir(crate::decor::Genre::ALL[genre as usize % 3]);
                self.b.chauffer(1);
                let de = r.lerp_inside(vec2(x0, y0));
                let a = self.b.app.vignettes.view.shrink(2.0).clamp(de + vec2(dx, dy) * r.width());
                self.glisser(Modifiers::NONE, PointerButton::Primary, &Self::trajet(de, a, 8));
                if lacher {
                    self.b.app.decor_lacher(true);
                    self.b.chauffer(1);
                }
            }
            Camera { case, x0, y0, dx, dy, ajoute } => {
                let Some((_, r)) = self.case(case) else { return };
                let m = Modifiers { ctrl: true, shift: true, command: true, alt: ajoute, ..Modifiers::NONE };
                let de = r.lerp_inside(vec2(x0, y0));
                let a = self.b.app.vignettes.view.shrink(2.0).clamp(de + vec2(dx, dy) * r.width());
                self.glisser(m, PointerButton::Primary, &Self::trajet(de, a, 8));
            }
            MenuCase { case, cadre, entree } => {
                let Some((k, r)) = self.case(case) else { return };
                let layout = self.b.app.board_layout();
                let (quads, _) = self.b.app.cadres_sur_la_planche(&layout, k);
                let xf = self.b.app.vignettes.xf;
                let p = match quads.get(cadre as usize) {
                    Some(q) if cadre > 0 => xf.to_screen(pos2((q[0].x + q[2].x) * 0.5, (q[0].y + q[2].y) * 0.5)),
                    _ => r.center(),
                };
                if !self.b.app.vignettes.view.contains(p) {
                    return;
                }
                self.img(vec![Event::PointerMoved(p)]);
                self.img(vec![appui(p, PointerButton::Secondary, true, Modifiers::NONE)]);
                self.img(vec![appui(p, PointerButton::Secondary, false, Modifiers::NONE)]);
                self.b.garder_formes = true;
                self.img(Vec::new());
                self.img(Vec::new());
                self.b.garder_formes = false;
                let mut vus = Vec::new();
                for s in &self.b.formes {
                    textes(&s.shape, &mut vus);
                }
                self.b.formes.clear();
                let offertes: Vec<Pos2> = MENU_CASE.iter().filter_map(|e| vus.iter().find(|(t, _)| t == e).map(|(_, p)| *p)).collect();
                if offertes.is_empty() {
                    self.img(vec![touche(Key::Escape, true, Modifiers::NONE)]);
                    self.img(vec![touche(Key::Escape, false, Modifiers::NONE)]);
                } else {
                    let q = offertes[entree as usize % offertes.len()];
                    self.img(vec![Event::PointerMoved(q)]);
                    self.img(vec![appui(q, PointerButton::Primary, true, Modifiers::NONE)]);
                    self.img(vec![appui(q, PointerButton::Primary, false, Modifiers::NONE)]);
                }
                self.img(vec![Event::PointerGone]);
                self.b.chauffer(2);
            }
            DeplacerPlan { de, vers, alt } => {
                let Some((_, a)) = self.case(de) else { return };
                let Some((_, c)) = self.case(vers) else { return };
                // (Le filet de gauche de la vignette, à mi-hauteur : son cadre.)
                let p = pos2(a.left(), a.center().y);
                let q = pos2(c.left() + c.width() * 0.6, c.center().y);
                let m = Modifiers { shift: true, alt, ..Modifiers::NONE };
                self.glisser(m, PointerButton::Primary, &Self::trajet(p, q, 8));
            }
            Annuler(n) => {
                for _ in 0..n {
                    self.b.app.run_action("edition.annuler");
                    self.b.chauffer(1);
                }
            }
            Retablir(n) => {
                for _ in 0..n {
                    self.b.app.run_action("edition.retablir");
                    self.b.chauffer(1);
                }
            }
            Lecture(n) => {
                self.b.app.run_action("lecture.basculer");
                for _ in 0..n {
                    self.b.image(Vec::new(), 1.0 / 24.0);
                }
                if self.b.app.playing {
                    self.b.app.run_action("lecture.basculer");
                }
                self.b.chauffer(1);
            }
            Onglet { nouveau } => {
                if nouveau && self.b.app.onglets.len() < 3 {
                    if self.b.app.nouvel_onglet() {
                        self.b.app.charger_projet(crate::vignettes_tests::quatre_plans());
                        self.b.app.welcome = false;
                    }
                } else {
                    self.b.app.onglet_voisin(1);
                }
                self.b.chauffer(3);
            }
            Touche(n) => {
                let (k, m) = TOUCHES[n as usize % TOUCHES.len()];
                self.b.touches = m;
                self.img(vec![touche(k, true, m)]);
                self.img(vec![touche(k, false, m)]);
                self.b.touches = Modifiers::NONE;
                self.b.chauffer(2);
            }
            ChoisirCase { case, maj } => {
                let Some((_, r)) = self.case(case) else { return };
                self.b.app.set_tool(Tool::Select);
                self.clic(r.center(), PointerButton::Primary, if maj { Modifiers::SHIFT } else { Modifiers::NONE });
            }
            Molette { dy, ctrl } => {
                self.vers_vignettes();
                let p = self.b.app.vignettes.view.center();
                let m = if ctrl { CTRL } else { Modifiers::NONE };
                self.b.touches = m;
                self.img(vec![Event::PointerMoved(p)]);
                let ev = self.b.molette(vec2(0.0, dy), m);
                self.img(ev);
                self.b.touches = Modifiers::NONE;
                self.img(vec![Event::PointerGone]);
                self.b.chauffer(2);
            }
            Colonnes(n) => {
                self.b.app.layout.board_columns = 2 + n as usize % 5;
                self.b.chauffer(2);
            }
            Interruption { geste, case, quoi, dx, dy } => {
                // Le départ et ce qui est tenu.
                let (de, m, largeur) = match geste % 6 {
                    3 => {
                        let Some(r) = self.toile() else { return };
                        self.b.app.set_tool(Tool::Brush);
                        (r.lerp_inside(vec2(0.4, 0.4)), Modifiers::NONE, r.width() * 0.3)
                    }
                    g => {
                        let Some((_, r)) = self.case(case) else { return };
                        match g {
                            0 => {
                                self.b.app.set_tool(Tool::Brush);
                                (r.lerp_inside(vec2(0.3, 0.4)), Modifiers::NONE, r.width())
                            }
                            1 => (r.center(), Modifiers { ctrl: true, shift: true, command: true, ..Modifiers::NONE }, r.width()),
                            2 => (pos2(r.left(), r.center().y), Modifiers::SHIFT, r.width()),
                            4 => (r.lerp_inside(vec2(0.2, 0.2)), CTRL, r.width()),
                            _ => {
                                self.b.app.decor_choisir(crate::decor::Genre::Feutre);
                                self.b.chauffer(1);
                                (r.lerp_inside(vec2(0.5, 0.5)), Modifiers::NONE, r.width())
                            }
                        }
                    }
                };
                let a = Rect::from_min_size(Pos2::ZERO, self.b.taille).shrink(4.0).clamp(de + vec2(dx, dy) * largeur);
                let chemin = Self::trajet(de, a, 8);
                self.b.touches = m;
                self.img(vec![Event::PointerMoved(de)]);
                self.img(vec![appui(de, PointerButton::Primary, true, m)]);
                for q in &chemin[1..4] {
                    self.img(vec![Event::PointerMoved(*q)]);
                }
                match INTERRUPTIONS[quoi as usize % INTERRUPTIONS.len()] {
                    "onglet" => self.b.app.onglet_voisin(1),
                    "enregistrer" => self.enregistrer(false),
                    e if e.starts_with("espace:") => self.b.app.set_espace(Espace::TOUS[e[7..].parse::<usize>().unwrap_or(0) % 4]),
                    id => self.b.app.run_action(id),
                }
                for q in &chemin[4..] {
                    self.img(vec![Event::PointerMoved(*q)]);
                }
                self.img(vec![appui(a, PointerButton::Primary, false, m)]);
                self.b.touches = Modifiers::NONE;
                self.img(vec![Event::PointerGone]);
                if self.b.app.playing {
                    self.b.app.run_action("lecture.basculer");
                }
                self.b.chauffer(2);
            }
            Enregistrer { rouvrir } => self.enregistrer(rouvrir),
        }
    }

    /// Enregistre dans un dossier neuf et relit ; `rouvrir` : rouvre aussi une copie dans un onglet.
    fn enregistrer(&mut self, rouvrir: bool) {
        self.enregistrements += 1;
        let manifeste = self.dossier.join(format!("p{}", self.enregistrements)).join("Film.pelure");
        if !self.b.app.save_to_blocking(manifeste.clone()) {
            return;
        }
        let attendu = photo_enregistrable(&mut self.b.app.doc);
        match crate::project::load(&manifeste) {
            Ok(l) => {
                if let Err(e) = meme_projet(&attendu, &photo(&l.doc)) {
                    panic!("INVARIANT relecture : le projet relu n'est pas celui qu'on a enregistré.\n{e}");
                }
            }
            Err(e) => panic!("INVARIANT relecture : le projet enregistré ne se relit pas : {e}"),
        }
        self.b.chauffer(1);
        if rouvrir && self.b.app.onglets.len() < 3 {
            let attendu = photo_enregistrable(&mut self.b.app.doc);
            let copie = self.dossier.join(format!("c{}", self.enregistrements)).join("Film.pelure");
            crate::project::save(&self.b.app.doc, &copie, &mut Default::default()).expect("copie");
            if self.b.app.nouvel_onglet() {
                self.b.app.open_path(copie);
                let t = Instant::now();
                while self.b.app.open_job.is_some() && t.elapsed() < Duration::from_secs(30) {
                    self.img(Vec::new());
                    std::thread::sleep(Duration::from_millis(2));
                }
                self.b.chauffer(3);
                if let Err(e) = meme_projet(&attendu, &photo(&self.b.app.doc)) {
                    panic!("INVARIANT réouverture : le projet rouvert dans un onglet n'est pas celui qu'on a enregistré.\n{e}");
                }
            }
        }
    }

    /// Les invariants de l'état, après un geste.
    pub(crate) fn verifier(&self) -> Result<(), Echec> {
        let app = &self.b.app;
        let doc = &app.doc;
        if doc.panels.is_empty() {
            return Err(echec("aucun plan", "le projet n'a plus de plan".into()));
        }
        if app.cur_panel >= doc.panels.len() {
            return Err(echec("plan courant", format!("cur_panel {} pour {} plans", app.cur_panel, doc.panels.len())));
        }
        let layers = &doc.panels[app.cur_panel].layers;
        if app.cur_layer >= layers.len() {
            return Err(echec("calque courant", format!("cur_layer {} pour {} calques", app.cur_layer, layers.len())));
        }
        let ids: BTreeSet<u64> = doc.panels.iter().map(|p| p.id).collect();
        if ids.len() != doc.panels.len() {
            return Err(echec("numéros de plans", "deux plans ont le même numéro".into()));
        }
        if let Some(id) = app.board_sel.iter().find(|id| !ids.contains(id)) {
            return Err(echec("plans choisis", format!("le plan choisi {id} n'existe plus")));
        }
        // (Les calques choisis d'un autre plan restent choisis en changeant de plan : chaque usage ne prend que ceux du plan
        // courant, `chosen_layers`. Ils doivent seulement exister.)
        if let Some(id) = app.layer_sel.iter().find(|id| !doc.panels.iter().any(|p| p.layers.iter().any(|l| l.id == **id))) {
            return Err(echec("calques choisis", format!("le calque choisi {id} n'existe plus")));
        }
        for (i, p) in doc.panels.iter().enumerate() {
            if p.frame_count() == 0 {
                return Err(echec("plan vide", format!("le plan {i} n'a aucune image")));
            }
            // Les dossiers en ordre (`folders::normalize` : ses calques juste sous chaque dossier, un parent qui est un dossier du plan) :
            // c'est ce que la lecture d'un projet refait, la pile en mémoire doit déjà l'être.
            if p.layers.iter().any(|l| l.folder || l.parent.is_some()) {
                let mut rangee = p.layers.clone();
                crate::folders::normalize(&mut rangee);
                let cle = |ls: &[crate::document::Layer]| ls.iter().map(|l| (l.id, l.parent)).collect::<Vec<_>>();
                if cle(&rangee) != cle(&p.layers) {
                    return Err(echec("dossiers en désordre", format!("plan {i} : {:?} au lieu de {:?}", cle(&p.layers), cle(&rangee))));
                }
            }
            let lids: BTreeSet<u64> = p.layers.iter().map(|l| l.id).collect();
            if lids.len() != p.layers.len() {
                return Err(echec("numéros de calques", format!("plan {i} : deux calques ont le même numéro")));
            }
            for l in &p.layers {
                if let Some(par) = l.parent {
                    if !p.layers.iter().any(|x| x.id == par && x.folder) {
                        return Err(echec("dossier de calque", format!("plan {i} : « {} » est rangé dans un dossier qui n'existe pas", l.name)));
                    }
                }
                if let Some(id) = l.frames.iter().flatten().find(|id| !doc.cels.contains_key(id)) {
                    return Err(echec("dessin absent", format!("plan {i}, calque « {} » : le dessin {id} n'existe pas", l.name)));
                }
                // Un calque de dessin ordinaire ne montre que des dessins de sa sorte (vectoriel ou bitmap).
                if !l.folder && l.couleur.is_none() && l.video.is_none() && l.effet.is_none() {
                    let vectoriel = l.kind == crate::document::LayerKind::Vector;
                    if let Some(id) = l.frames.iter().flatten().find(|id| doc.cels.get(id).is_some_and(|c| c.vector.is_some() != vectoriel)) {
                        return Err(echec("sorte de dessin", format!("plan {i}, calque « {} » ({:?}) : le dessin {id} n'est pas de sa sorte", l.name, l.kind)));
                    }
                }
            }
        }
        // La planche : chaque case sur un plan qui existe, dans l'ordre du film et de lecture, sans chevauchement.
        let l = app.board_layout();
        for (k, c) in l.cards.iter().enumerate() {
            match doc.panels.get(c.panel) {
                Some(p) if p.id == c.plan => {}
                _ => return Err(echec("case sans plan", format!("la case {k} vise le plan {} (numéro {}) : {} plans", c.panel, c.plan, doc.panels.len()))),
            }
            if k > 0 {
                let a = l.cards[k - 1];
                if (c.panel, c.frame) <= (a.panel, a.frame) {
                    return Err(echec("ordre du film", format!("les cases {} et {k} ne suivent pas l'ordre des plans : {:?} puis {:?}", k - 1, (a.panel, a.frame), (c.panel, c.frame))));
                }
                if !(c.row > a.row || (c.row == a.row && c.thumb.left() > a.thumb.left())) {
                    return Err(echec("ordre de lecture", format!("entre les cases {} et {k} : rangées {} {}, x {} {}", k - 1, a.row, c.row, a.thumb.left(), c.thumb.left())));
                }
            }
        }
        if let Some(i) = doc.panels.iter().enumerate().position(|(i, _)| !l.cards.iter().any(|c| c.panel == i)) {
            return Err(echec("plan sans case", format!("le plan {i} n'a pas de case sur la planche")));
        }
        sans_chevauchement(&l)?;
        Ok(())
    }
}

/// Aucune image (ou enveloppe) n'en recouvre une autre, ni une bande de note, ni une case pâle.
fn sans_chevauchement(l: &crate::board::BoardLayout) -> Result<(), Echec> {
    use crate::vignettecam::{coins_de, se_recouvrent};
    for (i, a) in l.cards.iter().enumerate() {
        let za = l.zones_of(i);
        for j in (i + 1)..l.cards.len() {
            for p in &za {
                for q in l.zones_of(j) {
                    if se_recouvrent(p, &q, 0.5) {
                        return Err(echec("chevauchement", format!("les images {i} et {j} se chevauchent : {p:?} {q:?}")));
                    }
                }
            }
        }
        for j in (0..l.cards.len()).filter(|&j| j != i) {
            for q in l.zones_of(j) {
                if se_recouvrent(&coins_de(a.note).to_vec(), &q, 0.5) {
                    return Err(echec("chevauchement de note", format!("la note de {i} recouvre l'image de {j}")));
                }
            }
        }
        for e in &l.empty {
            for p in &za {
                if se_recouvrent(p, &coins_de(*e).to_vec(), 0.5) {
                    return Err(echec("chevauchement de case pâle", format!("l'image {i} recouvre une case pâle")));
                }
            }
        }
    }
    Ok(())
}

fn message(e: &(dyn std::any::Any + Send)) -> String {
    e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_else(|| "(sans message)".into())
}

impl Essai {
    /// Un geste, ses vérifications et celles de son annulation.
    pub(crate) fn pas(&mut self, g: Geste) -> Result<(), Echec> {
        let onglet = self.b.app.actif;
        let (h0, s0, serie0) = (self.b.app.history.undo_len(), photo(&self.b.app.doc), self.b.app.history.serial());
        // (Une zone bitmap soulevée, déplacée par un geste d'avant, n'a son étape qu'en se reposant : l'annuler revient avant
        // le soulèvement, plus loin que l'état d'avant ce geste-ci. De même pour une sélection sur plusieurs calques.)
        let s0_net = self.b.app.bsel.as_ref().is_none_or(|s| s.lifted.is_none()) && self.b.app.multi.sel.is_none();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.jouer(g);
            // Un déplacement de sélection lâché attend son rendu exact : la prochaine action de la main le termine (le dessin
            // prend alors la géométrie de ses pixels, au pixel près, `flush_flottante_on_input`). On la devance, pour que l'état
            // d'après soit celui que garde l'étape.
            self.b.app.flush_flottante();
            self.b.app.multi_flush();
        }));
        if let Err(e) = r {
            let m = message(&*e);
            let espece = if m.starts_with("INVARIANT") { m.lines().next().unwrap_or("").to_string() } else { format!("plantage : {}", sans_chiffres(&m)) };
            return Err(echec(&espece, m));
        }
        self.verifier()?;
        if self.b.app.actif == onglet && self.b.app.history.undo_len() > h0 {
            let nom = self.b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default();
            *self.etapes.entry(nom).or_default() += 1;
        }
        // Le projet ne change pas sans que l'historique bouge : ce qu'on ne peut pas annuler serait perdu pour l'annulation.
        // (Sauf une zone bitmap encore soulevée, qui prend son étape en se reposant.)
        let s1_net = self.b.app.bsel.as_ref().is_none_or(|s| s.lifted.is_none()) && self.b.app.multi.sel.is_none();
        if s0_net && s1_net && g.change_avec_une_etape() && self.b.app.actif == onglet && self.b.app.history.serial() == serie0 {
            // (Replier ou déplier une piste est de l'affichage, sans étape : `pistes.rs`.)
            let sans_replis = |s: &str| s.replace("collapsed: true", "collapsed: false");
            if let Err(e) = meme_projet(&sans_replis(&s0), &sans_replis(&photo(&self.b.app.doc))) {
                return Err(echec("changement sans étape", format!("le projet a changé sans étape d'annulation.\n{e}")));
            }
        }
        if s0_net && g.verifie_l_annulation() && self.b.app.actif == onglet && self.b.app.history.undo_len() == h0 + 1 {
            let s1 = photo(&self.b.app.doc);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.b.app.run_action("edition.annuler");
                self.b.chauffer(1);
            }));
            if let Err(e) = r {
                return Err(echec(&format!("plantage en annulant : {}", sans_chiffres(&message(&*e))), message(&*e)));
            }
            if let Err(e) = meme_projet(&s0, &photo(&self.b.app.doc)) {
                let nom = self.b.app.history.names().1.last().map(|s| s.to_string()).unwrap_or_default();
                return Err(echec("annuler", format!("annuler « {nom} » ne rend pas le projet d'avant.\n{e}")));
            }
            self.verifier()?;
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.b.app.run_action("edition.retablir");
                self.b.chauffer(1);
            }));
            if let Err(e) = r {
                return Err(echec(&format!("plantage en rétablissant : {}", sans_chiffres(&message(&*e))), message(&*e)));
            }
            if let Err(e) = meme_projet(&s1, &photo(&self.b.app.doc)) {
                return Err(echec("rétablir", format!("rétablir ne rend pas le projet d'après.\n{e}")));
            }
            self.verifier()?;
        }
        Ok(())
    }
}

impl Drop for Essai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

// ------------------------------------------------------------------ passages, réduction, rejeu

/// Rejoue `gestes` depuis `depart(graine)` : l'échec au premier geste qui échoue (son rang).
pub(crate) fn rejouer_depuis(graine: u64, gestes: &[Geste]) -> Result<(), (usize, Echec)> {
    rejouer_sur(depart(graine), graine, gestes)
}

fn rejouer_sur(doc: Document, graine: u64, gestes: &[Geste]) -> Result<(), (usize, Echec)> {
    let mut e = Essai::new(doc, &format!("g{graine}"));
    for (k, g) in gestes.iter().enumerate() {
        e.pas(*g).map_err(|x| (k, x))?;
    }
    Ok(())
}

/// Pour un essai de non-régression : la suite rejouée ne doit pas échouer. (C'est elle que cite le rapport d'un échec : la suite
/// réduite s'y recopie telle quelle.)
#[allow(dead_code)]
pub(crate) fn rejouer(graine: u64, gestes: &[Geste]) {
    if let Err((k, x)) = rejouer_depuis(graine, gestes) {
        panic!("échec au geste {k} ({:?}) : {}\n{}", gestes[k], x.espece, x.detail);
    }
}

/// Les documents de départ des premières suites réduites (avant le quatrième, et avant la petite feuille de `petit_projet`) :
/// leurs essais de non-régression rejouent sur ceux-là.
fn depart_v1(graine: u64) -> Document {
    match graine % 3 {
        0 => crate::vignettes_tests::quatre_plans(),
        1 => crate::vignettes_camera_tests::plan_court(),
        _ => {
            let mut d = crate::perf_tests::projet_long(5, 2, 6, 1);
            d.into_plans();
            d
        }
    }
}

/// `rejouer` sur `depart_v1`.
fn rejouer_v1(graine: u64, gestes: &[Geste]) {
    if let Err((k, x)) = rejouer_sur(depart_v1(graine), graine, gestes) {
        panic!("échec au geste {k} ({:?}) : {}\n{}", gestes[k], x.espece, x.detail);
    }
}

/// Réduit une suite qui échoue (même espèce d'échec) : retire des morceaux de plus en plus petits (ddmin, simplifié).
fn reduire(graine: u64, gestes: Vec<Geste>, espece: &str) -> Vec<Geste> {
    let debut = Instant::now();
    let echoue = |s: &[Geste]| matches!(rejouer_depuis(graine, s), Err((_, x)) if x.espece == espece);
    let mut s = gestes;
    let mut morceau = s.len().div_ceil(2).max(1);
    while morceau >= 1 && debut.elapsed() < REDUCTION_MAX {
        let mut i = 0;
        let mut retire = false;
        while i < s.len() && debut.elapsed() < REDUCTION_MAX {
            let mut essai = s.clone();
            essai.drain(i..(i + morceau).min(s.len()));
            if !essai.is_empty() && echoue(&essai) {
                s = essai;
                retire = true;
            } else {
                i += morceau;
            }
        }
        if !retire {
            if morceau == 1 {
                break;
            }
            morceau = morceau.div_ceil(2);
        }
    }
    s
}

/// Une graine : `pas` gestes tirés au hasard. Un échec est réduit puis rapporté.
pub(crate) fn passage(graine: u64, pas: usize) -> Result<(), String> {
    let mut h = Hasard::new(graine);
    let temps = std::env::var_os("SHOT_HASARD_TEMPS").is_some();
    let t0 = Instant::now();
    let mut e = Essai::new(depart(graine), &format!("g{graine}"));
    if temps {
        eprintln!("HASARD graine {graine} : banc {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);
    }
    let mut journal = Vec::new();
    for k in 0..pas {
        let g = Geste::tirer(&mut h);
        journal.push(g);
        let t = Instant::now();
        let r = e.pas(g);
        if temps {
            eprintln!("HASARD graine {graine} geste {k} : {:.0} ms {g:?}", t.elapsed().as_secs_f64() * 1e3);
        }
        if let Err(x) = r {
            drop(e);
            let court = reduire(graine, journal.clone(), &x.espece);
            let detail = rejouer_depuis(graine, &court).err().map(|(_, x)| x.detail).unwrap_or_else(|| x.detail.clone());
            return Err(format!(
                "HASARD graine {graine}, geste {k} ({g:?}) : {}\n{}\n\nSuite réduite ({} gestes sur {}) :\nrejouer({graine}, &[{}]);\n\nDétail (suite réduite) :\n{detail}\n",
                x.espece,
                x.detail,
                court.len(),
                journal.len(),
                court.iter().map(|g| format!("{g:?}")).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    if std::env::var_os("SHOT_HASARD_ETAPES").is_some() {
        eprintln!("HASARD graine {graine} : étapes {:?}", e.etapes);
    }
    Ok(())
}

/// Les graines `graines`, réparties sur `fils` fils (chacun son application).
fn passages(graines: Vec<u64>, pas: usize, fils: usize) {
    let t = Instant::now();
    let toutes = graines;
    let echecs = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for f in 0..fils.max(1) {
            let (toutes, echecs) = (&toutes, &echecs);
            std::thread::Builder::new()
                .stack_size(16 << 20)
                .spawn_scoped(s, move || {
                    for g in toutes.iter().skip(f).step_by(fils.max(1)) {
                        if let Err(m) = passage(*g, pas) {
                            eprintln!("{m}");
                            echecs.lock().unwrap_or_else(|e| e.into_inner()).push(m);
                        }
                    }
                })
                .expect("fil d'essai");
        }
    });
    let echecs = echecs.into_inner().unwrap_or_else(|e| e.into_inner());
    println!("HASARD {} graines × {pas} gestes en {:.1} s : {} échec(s)", toutes.len(), t.elapsed().as_secs_f64(), echecs.len());
    assert!(echecs.is_empty(), "{}", echecs.join("\n"));
}

/// Le passage court, dans la suite.
#[test]
fn hasard_random_gestures_keep_the_application_consistent() {
    passages((0..GRAINES).collect(), PAS, 1);
}

/// Le passage long, à la demande (`-- --ignored hasard`). `SHOT_HASARD_GRAINES=a..b` choisit les graines,
/// `SHOT_HASARD_PAS` le nombre de gestes, `SHOT_HASARD_FILS` le nombre de fils (4).
#[test]
#[ignore]
fn hasard_long_run_of_random_gestures() {
    // (`a..b`, ou une liste `a,b,c` : une même graine plusieurs fois rejoue la même suite sur plusieurs fils à la fois.)
    let graines: Vec<u64> = std::env::var("SHOT_HASARD_GRAINES")
        .ok()
        .and_then(|v| match v.split_once("..") {
            Some((a, b)) => Some((a.trim().parse().ok()?..b.trim().parse().ok()?).collect()),
            None => v.split(',').map(|g| g.trim().parse().ok()).collect(),
        })
        .unwrap_or_else(|| (1000..1000 + GRAINES_LONGUES).collect());
    let pas = std::env::var("SHOT_HASARD_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(PAS_LONGS);
    let fils = std::env::var("SHOT_HASARD_FILS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    passages(graines, pas, fils);
}

/// Le hasard est reproductible : la même graine tire les mêmes gestes.
#[test]
fn hasard_the_same_seed_draws_the_same_gestures() {
    let tirer = |g| {
        let mut h = Hasard::new(g);
        (0..50).map(|_| Geste::tirer(&mut h)).collect::<Vec<_>>()
    };
    assert_eq!(tirer(7), tirer(7));
    assert_ne!(tirer(7), tirer(8));
}

// ------------------------------------------------------------------ non-régression (suites réduites)

/// Dupliquer un calque choisit la copie ; annuler la retire : elle ne reste pas choisie (un numéro de calque qui
/// n'existe plus).
#[test]
fn hasard_undoing_a_layer_duplicate_leaves_no_chosen_layer_that_does_not_exist() {
    rejouer_v1(34, &[Commande("calque.dupliquer")]);
}

/// Annuler la création de plans faits par des gestes de caméra et un lasso : les plans choisis qui n'existent plus ne
/// restent pas choisis (`board_sel.len() > 1` disait « plusieurs plans choisis » au color script, à la planche…).
#[test]
fn hasard_undoing_new_plans_leaves_no_chosen_plan_that_does_not_exist() {
    rejouer_v1(
        0,
        &[
            Camera { case: 48, x0: 0.82, y0: 0.62, dx: -0.87, dy: -0.07, ajoute: false },
            Camera { case: 19, x0: 0.48, y0: 0.66, dx: -1.1, dy: -0.37, ajoute: true },
            LassoDessin { x0: 0.42, y0: 0.38, x1: 0.52, y1: 0.91, dx: -0.04, dy: -0.2, supprimer: true },
            Annuler(3),
        ],
    );
    rejouer_v1(
        16,
        &[
            Barre { k: 31 },
            Decor { genre: 1, case: 2, x0: 0.99, y0: 0.6, dx: 0.31, dy: 0.12, lacher: false },
            TraitDessin { outil: 1, x0: 0.72, y0: 0.59, x1: 0.92, y1: 0.65 },
            Annuler(3),
        ],
    );
}

/// Une commande tapée pendant un Ctrl + Maj + glisser de caméra (réunir avec le plan précédent, supprimer le plan) : le
/// mouvement prend d'abord son étape, la commande la sienne ; annuler défait la commande, puis le mouvement.
#[test]
fn hasard_a_command_during_a_camera_drag_keeps_the_move_undoable() {
    rejouer_v1(210, &[Interruption { geste: 1, case: 10, quoi: 5, dx: 0.46, dy: -0.12 }]);
    rejouer_v1(253, &[Interruption { geste: 1, case: 28, quoi: 2, dx: 0.46, dy: 0.11 }]);
}

/// Passer au Montage pendant un Ctrl + Maj + glisser de caméra : le mouvement se pose. Il restait ouvert (le lâcher n'arrive
/// qu'aux Vignettes), et la planche gardait sa réserve de place : la case A restait sur les cases d'avant (chevauchement).
#[test]
fn hasard_a_workspace_switch_during_a_camera_drag_ends_the_move() {
    rejouer_v1(214, &[Interruption { geste: 2, case: 36, quoi: 4, dx: 0.94, dy: 0.38 }, Interruption { geste: 1, case: 61, quoi: 24, dx: -0.91, dy: -0.23 }]);
}

/// Réunir deux plans à une barre quand le second a un dossier de calques du même nom qu'un dossier du premier : ses calques
/// rejoignent ce dossier. Ils désignaient le dossier du second plan, réuni à l'autre et disparu.
#[test]
fn hasard_joining_two_plans_keeps_the_layers_of_the_second_in_an_existing_folder() {
    rejouer_v1(211, &[Commande("calque.cle_position"), Commande("dossier.ranger"), Colonnes(4), Camera { case: 56, x0: 0.81, y0: 0.25, dx: 1.11, dy: 0.14, ajoute: true }, Barre { k: 32 }]);
}

/// Déplacer une sélection vectorielle, puis la supprimer par la commande, sans clic ni touche entre les deux (la feuille
/// flottante du déplacement attend encore son rendu exact) : plus tard, la feuille qui se termine ne repose pas les éléments
/// d'avant aux rangs d'avant (l'élément supprimé revenait, l'autre disparaissait, sans étape d'annulation).
#[test]
fn hasard_deleting_a_moved_vector_selection_before_its_exact_render_keeps_the_drawing() {
    let mut e = Essai::new(crate::vignettes_tests::quatre_plans(), "flottante");
    // Deux traits, à gauche et à droite de la feuille.
    e.jouer(TraitDessin { outil: 0, x0: 0.15, y0: 0.3, x1: 0.3, y1: 0.7 });
    e.jouer(TraitDessin { outil: 0, x0: 0.7, y0: 0.3, x1: 0.85, y1: 0.7 });
    let cel = e.b.app.cel_here().expect("le dessin");
    let elements = |e: &Essai| e.b.app.doc.cels[&cel].vector.as_ref().map(|v| v.elements.len()).unwrap_or(0);
    assert_eq!(elements(&e), 2, "deux traits vectoriels");
    // Le lasso autour du trait de gauche, puis le déplacer.
    let r = e.toile().expect("le dessin à l'écran");
    e.b.app.set_tool(Tool::Select);
    let (de, a) = (r.lerp_inside(vec2(0.05, 0.1)), r.lerp_inside(vec2(0.45, 0.9)));
    let tour = [de, pos2(a.x, de.y), a, pos2(de.x, a.y), de];
    let mut chemin = Vec::new();
    for w in tour.windows(2) {
        chemin.extend(Essai::trajet(w[0], w[1], 4));
    }
    e.glisser(Modifiers::NONE, PointerButton::Primary, &chemin);
    assert!(e.b.app.vsel.as_ref().is_some_and(|s| s.items == vec![0]), "le trait de gauche est choisi : {:?}", e.b.app.vsel.as_ref().map(|s| s.items.clone()));
    // (Dans la boîte, loin de son centre : le pivot.)
    let m = r.lerp_inside(vec2(0.17, 0.36));
    let vers = m + vec2(60.0, 30.0);
    e.img(vec![Event::PointerMoved(m)]);
    e.img(vec![appui(m, PointerButton::Primary, true, Modifiers::NONE)]);
    for q in Essai::trajet(m, vers, 12) {
        e.img(vec![Event::PointerMoved(q)]);
    }
    e.img(vec![appui(vers, PointerButton::Primary, false, Modifiers::NONE)]);
    assert_eq!(crate::vignettes_tests::dernier_nom(&e.b), "Déplacer");
    assert!(e.b.app.flottante.as_ref().is_some_and(|f| f.pending.is_some()), "la feuille flottante du déplacement attend son rendu exact");
    // Aussitôt (aucune image d'écran : le rendu exact n'a pas pu arriver), la commande.
    e.b.app.run_action("edition.supprimer");
    let traits = |e: &Essai| -> Vec<String> { e.b.app.doc.cels[&cel].vector.as_ref().expect("vectoriel").elements.iter().map(|el| serde_json::to_string(el).expect("élément")).collect() };
    let apres = traits(&e);
    assert_eq!(apres.len(), 1, "le trait de gauche est supprimé");
    // La feuille d'un geste en attente se termine (un clic, une touche, le rendu exact) : le dessin ne change pas.
    e.b.app.flush_flottante();
    e.b.app.multi_flush();
    e.b.chauffer(3);
    assert_eq!(traits(&e), apres, "le trait qui reste est celui de droite, tel quel");
}

/// « Réunir avec le plan précédent » depuis le clic droit sur un cadre d'un plan qui bouge, dans sa tuile (le dessin de la case
/// courante s'y montre, avec son menu) : la planche de cette image n'est plus peinte, la tuile du plan qui bouge après lui ne
/// vise pas un rang de plan disparu (elle plantait : « index out of bounds » dans `panel_plan_at`, graine 461).
#[test]
fn hasard_joining_from_the_menu_of_a_frame_in_a_moving_tile_does_not_crash() {
    use crate::vignettes_camera_tests::{cadre_ecran, centre, ctrl_glisser, largeur_cadre_ecran, pas, plan_court, K};
    for decalage in [vec2(1.2, 0.3), vec2(0.6, 0.0), vec2(-0.8, 0.5)] {
        let mut b = crate::vignettes_tests::ouvert(plan_court(), 6);
        let w = largeur_cadre_ecran(&b);
        // Le dernier plan bouge aussi : sa tuile se peint après celle du plan 2.
        let derniere = crate::vignettes_tests::vignette(&b, b.app.board_layout().cards.len() - 1);
        let d = vec2(0.7, 0.2) * w;
        ctrl_glisser(&mut b, derniere.center(), &pas(derniere.center(), d, 6), derniere.center() + d, true);
        b.chauffer(3);
        let case = crate::vignettes_tests::vignette(&b, K);
        let d = decalage * w;
        ctrl_glisser(&mut b, case.center(), &pas(case.center(), d, 6), case.center() + d, true);
        b.chauffer(3);
        assert_eq!(b.app.doc.panels[1].camera.len(), 2, "le plan 2 a un mouvement");
        assert_eq!(b.app.doc.panels.last().map(|p| p.camera.len()), Some(2), "le dernier plan a un mouvement");
        assert_eq!(b.app.cur_panel, 1);
        let avant = b.app.doc.panels.len();
        // Le clic droit sur le cadre B, dans la tuile du plan.
        let p = centre(cadre_ecran(&b, 1));
        b.image(vec![Event::PointerMoved(p)], DT);
        b.image(vec![appui(p, PointerButton::Secondary, true, Modifiers::NONE)], DT);
        b.image(vec![appui(p, PointerButton::Secondary, false, Modifiers::NONE)], DT);
        b.garder_formes = true;
        b.image(Vec::new(), DT);
        b.image(Vec::new(), DT);
        let mut vus = Vec::new();
        for s in &b.formes {
            textes(&s.shape, &mut vus);
        }
        b.garder_formes = false;
        let Some(q) = vus.iter().find(|(t, _)| t == "Réunir avec le plan précédent").map(|(_, q)| *q) else { panic!("l'entrée du menu est là ({decalage:?}) : {:?}", vus.iter().map(|v| &v.0).collect::<Vec<_>>()) };
        b.image(vec![Event::PointerMoved(q)], DT);
        b.image(vec![appui(q, PointerButton::Primary, true, Modifiers::NONE)], DT);
        b.image(vec![appui(q, PointerButton::Primary, false, Modifiers::NONE)], DT);
        b.chauffer(4);
        assert_eq!(b.app.doc.panels.len(), avant - 1, "les deux plans sont réunis ({decalage:?})");
        assert!(b.app.cur_panel < b.app.doc.panels.len());
    }
}

/// « Réunir avec le plan précédent » depuis le clic droit sur le dessin de la case courante, quand c'est le dernier plan : la
/// suite de cette case (sa bande de note) ne vise pas le rang disparu (« index out of bounds » dans `card_notes`, graine 514).
#[test]
fn hasard_joining_the_last_plan_from_its_drawing_menu_does_not_crash() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    let mut b = crate::vignettes_tests::ouvert(doc, 4);
    b.app.select_board(2, 0);
    b.chauffer(3);
    let p = crate::vignettes_tests::vignette(&b, 2).center();
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![appui(p, PointerButton::Secondary, true, Modifiers::NONE)], DT);
    b.image(vec![appui(p, PointerButton::Secondary, false, Modifiers::NONE)], DT);
    b.garder_formes = true;
    b.image(Vec::new(), DT);
    b.image(Vec::new(), DT);
    let mut vus = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut vus);
    }
    b.garder_formes = false;
    let q = vus.iter().find(|(t, _)| t == "Réunir avec le plan précédent").map(|(_, q)| *q).expect("l'entrée du menu est là");
    b.image(vec![Event::PointerMoved(q)], DT);
    b.image(vec![appui(q, PointerButton::Primary, true, Modifiers::NONE)], DT);
    b.image(vec![appui(q, PointerButton::Primary, false, Modifiers::NONE)], DT);
    b.chauffer(4);
    assert_eq!(b.app.doc.panels.len(), 2, "les deux derniers plans sont réunis");
    assert!(b.app.cur_panel < b.app.doc.panels.len());
}

/// Un trait de Décor tenu pendant qu'on passe à l'Animation (le calque Décor vient d'être fait pour lui), puis, plus tard, un trait
/// dans une case pendant « Supprimer l'image » : chaque geste garde son étape d'annulation. La création attendait un lâcher sur la
/// planche, et prenait dans son étape tout ce qui s'était fait depuis.
#[test]
fn hasard_a_creating_stroke_interrupted_by_a_workspace_switch_or_a_command_keeps_undo_steps_apart() {
    rejouer(2442, &[Interruption { geste: 5, case: 22, quoi: 23, dx: -0.43, dy: -0.57 }, Interruption { geste: 0, case: 60, quoi: 16, dx: 0.76, dy: 0.53 }]);
}

/// Réunir deux plans quand le second a un calque de plus, sous les calques qu'il partage avec le premier (un calque vectoriel
/// « Dessin » sous « Calque 1 » et « Calque 2 », bitmap) : chaque calque reçoit les images du sien. Les rangs des calques d'arrivée
/// étaient pris avant d'ajouter « Dessin » en bas : les images de « Calque 1 » partaient dans « Dessin » (des dessins bitmap dans un
/// calque vectoriel, qu'un trait puis une annulation vidaient), celles de « Calque 2 » dans « Calque 1 ».
#[test]
fn hasard_joining_a_plan_with_an_extra_bottom_layer_keeps_each_layer_its_own_drawings() {
    use crate::document::{Cel, Layer, LayerKind};
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    let calque = |doc: &mut Document, nom: &str, sorte: LayerKind| {
        let id = doc.add_cel(Cel::new(doc.width, doc.height, sorte));
        let mut l = Layer::new(nom, sorte);
        l.frames = vec![Some(id); 4];
        (l, id)
    };
    let (a1, ca1) = calque(&mut doc, "Calque 1", LayerKind::Bitmap);
    let (a2, ca2) = calque(&mut doc, "Calque 2", LayerKind::Bitmap);
    let (b0, cb0) = calque(&mut doc, "Dessin", LayerKind::Vector);
    let (b1, cb1) = calque(&mut doc, "Calque 1", LayerKind::Bitmap);
    let (b2, cb2) = calque(&mut doc, "Calque 2", LayerKind::Bitmap);
    doc.panels[0].layers = vec![a1, a2];
    doc.panels[1].layers = vec![b0, b1, b2];
    for p in &mut doc.panels {
        p.length = 4;
    }
    assert_eq!(doc.sequence_of(0), doc.sequence_of(1));
    let i = doc.join_next_plan(0).expect("réunis");
    let dessins = |nom: &str| -> Vec<CelId> {
        let mut v: Vec<CelId> = doc.panels[i].layers.iter().find(|l| l.name == nom).expect("le calque").frames.iter().flatten().copied().collect();
        v.dedup();
        v
    };
    assert_eq!(dessins("Calque 1"), vec![ca1, cb1]);
    assert_eq!(dessins("Calque 2"), vec![ca2, cb2]);
    assert_eq!(dessins("Dessin"), vec![cb0]);
    for l in &doc.panels[i].layers {
        for id in l.frames.iter().flatten() {
            assert_eq!(doc.cels[id].vector.is_some(), l.kind == LayerKind::Vector, "« {} » : le dessin {id} est de sa sorte", l.name);
        }
    }
}
/// Copier des images d'un calque bitmap et les coller sur un calque vectoriel (d'un autre onglet, ou d'un plan neuf) : rien de la
/// mauvaise sorte n'arrive dans le calque (des dessins bitmap dans un calque vectoriel).
#[test]
fn hasard_pasting_images_on_a_layer_of_the_other_kind_pastes_nothing_of_the_wrong_kind() {
    rejouer(3586, &[Commande("images.copier"), Onglet { nouveau: true }, Commande("images.coller")]);
    rejouer(3666, &[Commande("panneau.nouveau"), TraitDessin { outil: 5, x0: 0.74, y0: 0.8, x1: 0.64, y1: 0.94 }, Commande("images.copier"), Interruption { geste: 0, case: 22, quoi: 17, dx: -0.76, dy: -0.19 }, Commande("images.coller")]);
}

/// Deux plans voisins dont la caméra monte au-dessus de la première rangée (graine 3694) : leurs enveloppes ne se recouvrent pas là-haut.
/// La planche ne notait pas ce que prennent les enveloppes au-dessus de la page (ces rangées n'existent pas) : la seconde s'y posait sur
/// la première.
#[test]
fn hasard_two_moving_plans_rising_above_the_first_row_do_not_overlap() {
    use crate::camera::{set_key, Camera};
    let mut doc = Document::with_frame(160, 90, 24);
    for i in 0..4 {
        doc.add_scene_after(i);
    }
    for i in 0..5 {
        doc.set_duration(i, 6);
    }
    let cam = |x: f32, y: f32| Camera { x, y, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
    set_key(&mut doc.panels[0].camera, 0, cam(160.0, 90.0));
    set_key(&mut doc.panels[0].camera, 5, cam(206.4, 12.247292));
    set_key(&mut doc.panels[1].camera, 0, cam(235.65042, 205.46805));
    set_key(&mut doc.panels[1].camera, 5, cam(51.19999, 42.506195));
    let s = crate::board::BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let l = crate::board::layout(&doc, &s);
    if let Err(e) = sans_chevauchement(&l) {
        panic!("{}", e.detail);
    }
}

/// Réunir deux plans quand le second a rangé son calque Décor dans un dossier (« Choisir tous les calques » puis « Placer les calques
/// choisis dans un nouveau dossier », graine 4747) : le Décor passe tout en bas du plan réuni, hors du dossier. Il gardait son dossier,
/// loin au-dessus de lui : la pile était en désordre, et la lecture du projet enregistré la remettait autrement.
#[test]
fn hasard_joining_a_plan_whose_decor_is_in_a_folder_keeps_the_stack_in_order() {
    use crate::document::{Cel, Layer, LayerKind};
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    let dessin = |doc: &mut Document| doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
    let mut a = Layer::new("Calque 1", LayerKind::Bitmap);
    a.frames = vec![Some(dessin(&mut doc)); 4];
    let mut dossier = Layer::new("Dossier", LayerKind::Bitmap);
    dossier.folder = true;
    let mut decor = Layer::new("Décor", LayerKind::Bitmap);
    decor.decor = true;
    decor.parent = Some(dossier.id);
    decor.frames = vec![Some(dessin(&mut doc)); 4];
    let mut autre = Layer::new("Autre", LayerKind::Bitmap);
    autre.parent = Some(dossier.id);
    autre.frames = vec![Some(dessin(&mut doc)); 4];
    doc.panels[0].layers = vec![a];
    doc.panels[1].layers = vec![decor, autre, dossier];
    for p in &mut doc.panels {
        p.length = 4;
    }
    let i = doc.join_next_plan(0).expect("réunis");
    let layers = &doc.panels[i].layers;
    let mut rangee = layers.clone();
    crate::folders::normalize(&mut rangee);
    let cle = |ls: &[Layer]| ls.iter().map(|l| (l.name.clone(), l.parent)).collect::<Vec<_>>();
    assert_eq!(cle(layers), cle(&rangee), "la pile est en ordre");
    assert!(layers[0].decor && layers[0].parent.is_none(), "le Décor est tout en bas, hors du dossier : {:?}", cle(layers));
}

/// « Nouvelle version du plan » pendant qu'on déplace une zone bitmap choisie au lasso (graine 4667) : le déplacement se pose
/// d'abord, avec son étape, puis la version prend la sienne. La zone soulevée restait sur le dessin de la version d'avant, rangée et
/// invisible : la nouvelle version gardait les pixels à mi-chemin sans étape pour les défaire, et le premier Ctrl+Z ne défaisait
/// rien de visible.
#[test]
fn hasard_a_new_version_during_a_bitmap_selection_move_lays_the_move_down_first() {
    let mut e = Essai::new(petit_projet(3, 1, 6), "version-lasso");
    // Un trait (bitmap) au milieu du dessin : la zone choisie a des pixels à déplacer.
    e.jouer(TraitDessin { outil: 0, x0: 0.3, y0: 0.35, x1: 0.6, y1: 0.6 });
    let r = e.toile().expect("le dessin à l'écran");
    e.b.app.set_tool(Tool::Select);
    let (de, a) = (r.lerp_inside(vec2(0.1, 0.1)), r.lerp_inside(vec2(0.9, 0.9)));
    let tour = [de, pos2(a.x, de.y), a, pos2(de.x, a.y), de];
    let mut chemin = Vec::new();
    for w in tour.windows(2) {
        chemin.extend(Essai::trajet(w[0], w[1], 4));
    }
    e.glisser(Modifiers::NONE, PointerButton::Primary, &chemin);
    assert!(e.b.app.bsel.is_some(), "une zone bitmap est choisie");
    // Le glissé de la zone (loin de son pivot, au centre) ; la commande en plein geste.
    let m = r.lerp_inside(vec2(0.3, 0.3));
    e.img(vec![Event::PointerMoved(m)]);
    e.img(vec![appui(m, PointerButton::Primary, true, Modifiers::NONE)]);
    for k in 1..=5 {
        e.img(vec![Event::PointerMoved(m + vec2(6.0 * k as f32, 3.0 * k as f32))]);
    }
    assert!(e.b.app.bsel.as_ref().is_some_and(|s| s.lifted.is_some()), "la zone est soulevée, en plein déplacement");
    e.b.app.run_action("version.nouvelle");
    for k in 6..=10 {
        e.img(vec![Event::PointerMoved(m + vec2(6.0 * k as f32, 3.0 * k as f32))]);
    }
    e.img(vec![appui(m + vec2(60.0, 30.0), PointerButton::Primary, false, Modifiers::NONE)]);
    e.b.chauffer(2);
    let noms = e.b.app.history.names().0.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(noms.last().map(String::as_str), Some("Nouvelle version du plan"), "la version a la dernière étape : {noms:?}");
    assert!(noms.len() >= 2 && noms[noms.len() - 2] == "Déplacer", "le déplacement s'est posé avant, avec son étape : {noms:?}");
    // Aucune zone soulevée ne reste sur un dessin qui n'est plus celui du plan.
    let dessins: BTreeSet<CelId> = e.b.app.layers().iter().flat_map(|l| l.frames.iter().flatten().copied()).collect();
    assert!(e.b.app.bsel.as_ref().is_none_or(|s| dessins.contains(&s.cel)), "la zone choisie est sur un dessin du plan courant");
}

/// Couper un plan à une barre, après avoir choisi un calque : le calque choisi ne désigne pas un calque disparu.
#[test]
fn hasard_a_cut_bar_after_choosing_a_layer_leaves_no_chosen_layer_that_does_not_exist() {
    rejouer_v1(
        18,
        &[
            TraitCase { case: 4, outil: 4, x0: 0.78, y0: 0.92, dx: 1.07, dy: -0.7 },
            Commande("calque.nouveau_vectoriel"),
            LassoCase { case: 21, x0: 0.23, y0: 0.31, x1: 0.56, y1: 1.0, supprimer: false },
            Commande("panneau.suivant"),
            Commande("calque.choisir_dessus"),
            Barre { k: 27 },
        ],
    );
}
