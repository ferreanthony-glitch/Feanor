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
//! - **Depuis le 2026-10-29 (§70 de `docs/INTERFACE.md`) tout est une « brosse dédiée »**, de trois types (`Type`) : *Personnage* (celles
//!   du projet, ci-dessus), *Classique* (l'ancienne « couleur neutre ») et *Décor* (les anciens pinceaux de décor, avec un pinceau et une
//!   taille). Les Classiques et les Décors sont des réglages de l'*application* (`reglages.toml`, `Settings::brosses_classiques` et
//!   `Settings::brosses_decor`, tous deux des listes de `Brosse`) : ils servent dans tous les projets, sans numéro ni place dans le projet.
//! - Choisir une brosse règle la couleur du trait et allume l'aplat sous le trait : voir `personnagesui.rs` (la barre et le
//!   panneau « Brosses dédiées ») et `persofond.rs` (comment l'aplat d'un trait prend la couleur de la brosse qui l'a dessiné).

use serde::{Deserialize, Serialize};

use crate::brush::BrushSettings;
use crate::decor::Genre;
use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "personnages.json";
const FORMAT: &str = "shot-personnages";
const VERSION: u32 = 1;
/// Bornes d'un fichier abîmé (ou d'une barre qui n'aurait plus de place) : au-delà, c'est coupé.
pub const MAX_PERSONNAGES: usize = 48;
pub const MAX_CLASSIQUES: usize = 8;
pub const MAX_DECORS: usize = 8;
/// Longueur d'un nom, en caractères.
pub const NOM_MAX: usize = 40;

/// Un personnage du projet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// Sa brosse spécifique (§107) ; rien : elle suit le pinceau de l'utilisateur. Absent d'un ancien fichier : elle suit.
    #[serde(rename = "brosse", default, skip_serializing_if = "Option::is_none")]
    pub brosse: Option<Specifique>,
}

/// Le réglage propre à une brosse dédiée (« Brosse spécifique », §107) : l'outil (pinceau ou crayon) et le réglage complet de ce pinceau
/// (forme, taille, texture, opacité…, le même `BrushSettings` que tout pinceau ou toute brosse enregistrée). Il est **à elle** : choisie, la
/// brosse le prend à la place du pinceau courant, lâchée elle le rend tel qu'il était, et rien ne passe de l'un à l'autre.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Specifique {
    /// Vrai : l'outil Crayon ; faux : l'outil Pinceau.
    #[serde(rename = "crayon", default)]
    pub crayon: bool,
    #[serde(rename = "reglage", default)]
    pub settings: BrushSettings,
    /// Le nom de la brosse enregistrée dont elle est partie (pour le dire à l'écran) ; rien : elle est partie du pinceau de l'utilisateur.
    #[serde(rename = "nom", default, skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
}

impl Specifique {
    /// Le réglage d'un des trois anciens pinceaux de décor (crayon, feutre, pinceau plein), à la taille `taille`.
    pub fn depuis_genre(genre: Genre, taille: f32) -> Specifique {
        Specifique { crayon: genre.est_crayon(), settings: genre.reglage(taille), nom: Some(genre.nom().to_string()) }
    }

    /// Ce qui dessine, en deux mots : le nom de la brosse dont elle est partie (« modifiée » si le réglage a bougé depuis, d'après la bibliothèque) ;
    /// sans nom, celui de la brosse de la bibliothèque qui a exactement ces valeurs, sinon « Réglage propre ».
    pub fn libelle(&self, presets: &[crate::brushtex::Preset]) -> String {
        let brosses = || presets.iter().filter(|p| p.target == crate::brushtex::Target::Brush);
        match &self.nom {
            Some(n) => match brosses().find(|p| &p.name == n) {
                Some(p) if p.settings != self.settings => format!("{n} (modifiée)"),
                _ => n.clone(),
            },
            None => brosses().find(|p| p.settings == self.settings).map_or_else(|| "Réglage propre".to_string(), |p| p.name.clone()),
        }
    }
}

/// Le type d'une brosse dédiée (Anthony, 2026-10-29 : « on peut créer des brosses personnage, décor ou classique »).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// Dessine dans le calque de son personnage, créé au premier trait.
    Personnage,
    /// Dessine dans le calque courant, sans rien créer.
    Classique,
    /// Dessine dans le calque Décor du plan, créé au premier trait.
    Decor,
}

impl Type {
    pub const ALL: [Type; 3] = [Type::Personnage, Type::Classique, Type::Decor];

    /// Le mot du type, au singulier.
    pub fn nom(self) -> &'static str {
        match self {
            Type::Personnage => "Personnage",
            Type::Classique => "Classique",
            Type::Decor => "Décor",
        }
    }

    /// Le titre d'une section du panneau.
    pub fn titre(self) -> &'static str {
        match self {
            Type::Personnage => "Personnages",
            Type::Classique => "Classiques",
            Type::Decor => "Décors",
        }
    }

    /// Ce que fait une brosse de ce type, en une phrase (bulles).
    pub fn effet(self) -> &'static str {
        match self {
            Type::Personnage => "dessine dans un calque à son nom, créé au premier trait",
            Type::Classique => "dessine dans le calque courant, sans rien créer",
            Type::Decor => "dessine dans le calque Décor du plan, créé au premier trait",
        }
    }
}

/// Une brosse dédiée de **poste** (Classique ou Décor) : un réglage de l'application (`reglages.toml`), commun à tous les projets. Les
/// brosses Personnage sont celles du projet (`Personnage`). Avant le 2026-10-29, les Classiques s'appelaient les « couleurs neutres ».
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Brosse {
    #[serde(rename = "nom")]
    pub name: String,
    #[serde(rename = "trait")]
    pub line: [u8; 3],
    #[serde(rename = "fond", default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<[u8; 3]>,
    /// Sa brosse spécifique (§107) ; rien : elle suit le pinceau de l'utilisateur (le cas d'origine, pour les trois types).
    #[serde(rename = "brosse", default, skip_serializing_if = "Option::is_none")]
    pub specifique: Option<Specifique>,
    /// **Ancien** pinceau d'une brosse Décor (avant le §107) : lu, jamais écrit (`reprendre_ancien_pinceau`).
    #[serde(rename = "pinceau", default, skip_serializing_if = "Option::is_none")]
    pub pinceau: Option<Genre>,
    /// **Ancienne** taille d'une brosse Décor : lue, jamais écrite.
    #[serde(rename = "taille", default, skip_serializing_if = "taille_absente")]
    pub taille: f32,
}

fn taille_absente(t: &f32) -> bool {
    *t == 0.0
}

impl Brosse {
    /// Une brosse Classique (elle suit le pinceau).
    pub fn classique(name: &str, line: [u8; 3], fill: Option<[u8; 3]>) -> Brosse {
        Brosse { name: name.into(), line, fill, specifique: None, pinceau: None, taille: 0.0 }
    }

    /// Une brosse Décor (elle suit le pinceau).
    pub fn decor(name: &str, line: [u8; 3], fill: Option<[u8; 3]>) -> Brosse {
        Brosse::classique(name, line, fill)
    }

    /// Ancien fichier de réglages (avant le §107) : une brosse Décor avait toujours un pinceau (crayon, feutre ou pinceau plein) et une
    /// taille. Une brosse restée à ceux d'origine du pinceau (crayon 5, feutre 24, pinceau plein 90) **suit maintenant le pinceau** ; une
    /// brosse que l'utilisateur avait réglée à la main devient une **brosse spécifique** qui garde ce pinceau et cette taille. Les anciens
    /// champs sont vidés : le fichier ne s'écrit plus que dans la forme nouvelle, et rappeler cette fonction ne change plus rien.
    pub fn reprendre_ancien_pinceau(&mut self) {
        let (ancien, taille) = (self.pinceau.take(), std::mem::take(&mut self.taille));
        let Some(genre) = ancien else { return };
        if self.specifique.is_some() {
            return;
        }
        let taille = if taille.is_finite() && taille > 0.0 { taille.clamp(0.5, 400.0) } else { genre.taille_defaut() };
        if (taille - genre.taille_defaut()).abs() > 1e-3 {
            self.specifique = Some(Specifique::depuis_genre(genre, taille));
        }
    }
}

/// Les brosses Classique d'un SHOT neuf (Anthony, 2026-10-28) : trois niveaux de gris, le trait toujours plus foncé que l'aplat, et
/// l'ensemble qui s'éclaircit à chaque niveau (#1E1E1E sur #8C8C8C, #4A4A4A sur #B4B4B4, #7A7A7A sur #DADADA). Avant : noir sans
/// aplat, gris, rouge de correction sans aplat.
pub fn classiques_par_defaut() -> Vec<Brosse> {
    vec![
        Brosse::classique("Gris 1", [0x1E, 0x1E, 0x1E], Some([0x8C, 0x8C, 0x8C])),
        Brosse::classique("Gris 2", [0x4A, 0x4A, 0x4A], Some([0xB4, 0xB4, 0xB4])),
        Brosse::classique("Gris 3", [0x7A, 0x7A, 0x7A], Some([0xDA, 0xDA, 0xDA])),
    ]
}

/// Les brosses Décor d'un SHOT neuf (Anthony, 2026-10-29) : trois niveaux de gris, plus clairs que les Classiques, le trait toujours plus
/// foncé que l'aplat. **Depuis le §107, elles suivent toutes le pinceau de l'utilisateur** (avant : crayon 5 px, feutre 24 px, pinceau plein 90 px).
pub fn decors_par_defaut() -> Vec<Brosse> {
    vec![
        Brosse::decor("Décor 1", [0x6E, 0x6E, 0x6E], Some([0xC8, 0xC8, 0xC8])),
        Brosse::decor("Décor 2", [0x8E, 0x8E, 0x8E], Some([0xD8, 0xD8, 0xD8])),
        Brosse::decor("Décor 3", [0xAB, 0xAB, 0xAB], Some([0xE6, 0xE6, 0xE6])),
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

/// Les brosses de poste lues dans les réglages, ramenées à des bornes raisonnables (noms coupés, pas plus de `max`, tailles saines). Une
/// liste vide reste vide : on a le droit de n'en vouloir aucune.
pub fn brosses_nettoyees(list: Vec<Brosse>, max: usize) -> Vec<Brosse> {
    list.into_iter()
        .take(max)
        .map(|mut n| {
            n.name = nom_net(&n.name);
            if !n.taille.is_finite() || n.taille < 0.0 {
                n.taille = 0.0;
            }
            // L'ancien pinceau d'un Décor devient « suit le pinceau » ou « brosse spécifique » (§107).
            n.reprendre_ancien_pinceau();
            if let Some(s) = n.specifique.as_mut() {
                s.settings.size = if s.settings.size.is_finite() { s.settings.size.clamp(0.5, 400.0) } else { BrushSettings::default().size };
            }
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

/// Un nom « Classique N » ou « Décor N » que la liste de poste n'a pas encore.
pub fn nom_neuf_poste(list: &[Brosse], t: Type) -> String {
    let mut n = list.len() + 1;
    loop {
        let name = format!("{} {n}", t.nom());
        if !list.iter().any(|p| p.name == name) {
            return name;
        }
        n += 1;
    }
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
        if let Some(s) = p.brosse.as_mut() {
            s.settings.size = if s.settings.size.is_finite() { s.settings.size.clamp(0.5, 400.0) } else { BrushSettings::default().size };
        }
        out.push(p);
    }
    Ok(out)
}
