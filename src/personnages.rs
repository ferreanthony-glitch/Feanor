//! Brosse par personnage (Anthony, 2026-10-21 : « on ajoute un personnage et on lui attribue une couleur de
//! trait et une couleur de fond. Quand on sélectionne ce personnage, on dessine avec la couleur de trait et de
//! fond immédiatement. Ça permet, même avec un dessin très rough, d'identifier le personnage »).
//!
//! # Ce que c'est
//!
//! - Les **personnages** sont ceux du *projet* : un nom, une couleur de trait, une couleur de fond (ou aucune),
//!   dans un ordre. Ils sont dans `Structure` (l'annulation les rétablit avec le reste) et dans leur fichier,
//!   `personnages.json`, à côté du manifeste, sans changer `VERSION` (même principe que `transitions.json` :
//!   absent, il n'y a pas de personnage ; un fichier illisible ou d'une version plus récente n'empêche pas
//!   d'ouvrir le projet et n'est jamais effacé, `project::write_sidecar`). Un projet partagé les écrit avec la
//!   structure : les équipiers ont les mêmes personnages.
//! - Les **couleurs neutres** sont un réglage de l'*application* (`reglages.toml`, `Settings::neutral_colors`) :
//!   quelques entrées (trait et fond facultatif) qui servent dans tous les projets (noir sans fond, gris, rouge de
//!   correction). Ce ne sont pas des personnages : elles n'ont ni numéro ni place dans le projet.
//! - Choisir un personnage (ou une couleur neutre) règle la couleur du trait et allume le fond sous le trait :
//!   voir `personnagesui.rs` (la barre et le panneau) et `persofond.rs` (comment le fond d'un trait prend la
//!   couleur du personnage qui l'a dessiné).

use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "personnages.json";
const FORMAT: &str = "shot-personnages";
const VERSION: u32 = 1;
/// Bornes d'un fichier abîmé (ou d'une barre qui n'aurait plus de place) : au-delà, c'est coupé.
pub const MAX_PERSONNAGES: usize = 48;
pub const MAX_NEUTRES: usize = 8;
/// Longueur d'un nom, en caractères.
pub const NOM_MAX: usize = 40;

/// Un personnage du projet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Personnage {
    /// Numéro unique dans le projet (comme les commentaires et les transitions).
    pub id: u64,
    #[serde(rename = "nom")]
    pub name: String,
    /// Couleur du trait.
    #[serde(rename = "trait")]
    pub line: [u8; 3],
    /// Couleur du fond sous le trait ; rien : le personnage n'a pas de fond.
    #[serde(rename = "fond", default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 3]>,
}

/// Une couleur neutre (réglage de l'application).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Neutre {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "trait")]
    pub line: [u8; 3],
    #[serde(rename = "fond", default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 3]>,
}

/// Les couleurs neutres d'un SHOT neuf (Anthony, 2026-10-28) : trois niveaux de gris, le trait toujours plus foncé que l'aplat, et
/// l'ensemble qui s'éclaircit à chaque niveau (#1E1E1E sur #8C8C8C, #4A4A4A sur #B4B4B4, #7A7A7A sur #DADADA). Avant : noir sans
/// aplat, gris, rouge de correction sans aplat.
pub fn neutres_par_defaut() -> Vec<Neutre> {
    vec![
        Neutre { name: "Gris 1".into(), line: [0x1E, 0x1E, 0x1E], fill: Some([0x8C, 0x8C, 0x8C]) },
        Neutre { name: "Gris 2".into(), line: [0x4A, 0x4A, 0x4A], fill: Some([0xB4, 0xB4, 0xB4]) },
        Neutre { name: "Gris 3".into(), line: [0x7A, 0x7A, 0x7A], fill: Some([0xDA, 0xDA, 0xDA]) },
    ]
}

/// Les personnages d'un projet neuf (Anthony, 2026-10-28) : « Perso 1 », « Perso 2 », « Perso 3 », avec le trait et l'aplat qu'il avait
/// réglés sur ses trois premières pastilles d'origine (rouge foncé sur rose, bleu nuit sur bleu, vert foncé sur vert). Un projet
/// qui existe déjà n'en reçoit pas : seuls les projets créés par « Nouveau projet » (`nouveau::build_document`) les ont.
pub fn personnages_d_origine() -> [(&'static str, [u8; 3], [u8; 3]); 3] {
    [
        ("Perso 1", [39, 2, 2], [150, 97, 97]),
        ("Perso 2", [0, 28, 46], [78, 110, 145]),
        ("Perso 3", [19, 39, 17], [93, 114, 96]),
    ]
}

/// Les couleurs neutres lues dans les réglages, ramenées à des bornes raisonnables (noms coupés, pas plus de
/// `MAX_NEUTRES`). Une liste vide reste vide : on a le droit de n'en vouloir aucune.
pub fn neutres_nettoyees(list: Vec<Neutre>) -> Vec<Neutre> {
    list.into_iter()
        .take(MAX_NEUTRES)
        .map(|mut n| {
            n.name = nom_net(&n.name);
            n
        })
        .collect()
}

/// Un nom sans espaces de bord, coupé à `NOM_MAX` caractères.
pub fn nom_net(name: &str) -> String {
    name.trim().chars().take(NOM_MAX).collect()
}

/// Les couleurs d'un personnage neuf, selon sa place : une teinte différente à chaque rang, un trait franc et un
/// fond clair de la même teinte.
pub fn couleurs_neuves(rang: usize) -> ([u8; 3], [u8; 3]) {
    const TEINTES: [[u8; 3]; 8] =
        [[214, 69, 65], [52, 118, 201], [46, 160, 90], [226, 150, 30], [140, 80, 190], [30, 160, 170], [212, 80, 150], [110, 110, 50]];
    let line = TEINTES[rang % TEINTES.len()];
    (line, mele(line, [255, 255, 255], 0.72))
}

/// Un fond assorti à la couleur de trait `line` : la même teinte, éclaircie (quand on donne un fond à un
/// personnage qui n'en avait pas).
pub fn fond_assorti(line: [u8; 3]) -> [u8; 3] {
    mele(line, [255, 255, 255], 0.72)
}

/// `a` mêlé à `b` (`t` : la part de `b`).
fn mele(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// Un nom « Personnage N » que la liste n'a pas encore.
pub fn nom_neuf(list: &[Personnage]) -> String {
    let mut n = list.len() + 1;
    loop {
        let name = format!("Personnage {n}");
        if !list.iter().any(|p| p.name == name) {
            return name;
        }
        n += 1;
    }
}

/// Un nom pour la copie de `name` (« Léa (copie) », puis « Léa (copie 2) »…).
pub fn nom_de_copie(list: &[Personnage], name: &str) -> String {
    let base = name.split(" (copie").next().unwrap_or(name).trim();
    let first = format!("{base} (copie)");
    if !list.iter().any(|p| p.name == first) {
        return nom_net(&first);
    }
    (2..)
        .map(|k| format!("{base} (copie {k})"))
        .find(|c| !list.iter().any(|p| &p.name == c))
        .map(|c| nom_net(&c))
        .unwrap_or_default()
}

/// Déplace l'élément `from` de `delta` rangs (négatif : vers le début). Rend sa nouvelle place, `None` s'il ne
/// bouge pas (déjà au bout, ou rang inconnu).
pub fn deplacer<T>(list: &mut Vec<T>, from: usize, delta: isize) -> Option<usize> {
    if from >= list.len() {
        return None;
    }
    let to = (from as isize + delta).clamp(0, list.len() as isize - 1) as usize;
    if to == from {
        return None;
    }
    let item = list.remove(from);
    list.insert(to, item);
    Some(to)
}

// ------------------------------------------------------------------------------------ le fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    personnages: Vec<Personnage>,
}

/// Le texte de `personnages.json` pour `doc` ; rien s'il n'y a pas de personnage (le fichier part alors, s'il est
/// lisible : `write_sidecar`).
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    if doc.personnages.is_empty() {
        return Ok(None);
    }
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, personnages: doc.personnages.iter().take(MAX_PERSONNAGES).cloned().collect() };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

/// Les personnages lus dans le texte de `personnages.json`. Numéros uniques et non nuls (un double en reçoit un
/// neuf), noms nets, pas plus de `MAX_PERSONNAGES`.
pub(crate) fn parse(text: &str) -> Result<Vec<Personnage>, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de personnages".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.personnages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(sidecar.personnages.len().min(MAX_PERSONNAGES));
    for mut p in sidecar.personnages.into_iter().take(MAX_PERSONNAGES) {
        if p.id == 0 || !seen.insert(p.id) {
            p.id = next;
            next += 1;
            seen.insert(p.id);
        }
        p.name = nom_net(&p.name);
        out.push(p);
    }
    Ok(out)
}
