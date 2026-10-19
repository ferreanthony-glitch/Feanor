//! Sons proposés (Anthony, 2026-10-19) : SHOT lit le storyboard et propose des sons « témoins » de
//! qualité animatique (bruitages, ambiances, musique d'intention, voix des dialogues) que le
//! réalisateur accepte, régénère ou supprime. Ce module est le **modèle** d'une proposition (une
//! `Indication` sonore) et son fichier ; la lecture du storyboard est dans `lecturesb.rs`, les
//! fournisseurs dans `fournisseurs.rs`, l'écran dans `suggestionsui.rs`.
//!
//! - Où : une indication tient à un plan (son id) et à une image de ce plan (`start`, depuis 0), pour
//!   `frames` images. Elle suit son plan quand le montage change ; si le plan disparaît, elle n'est
//!   plus écrite (comme un commentaire).
//! - Vie d'une proposition : proposée → en génération → prête (un fichier est là, à écouter) →
//!   acceptée (le son est posé dans le montage) ou rejetée. Le fichier généré est dans le dossier de
//!   cache de SHOT (`generation::dossier_cache`), pas dans le projet : le projet ne reçoit un son
//!   que lorsqu'il est accepté (copié comme tout son importé).
//! - Rangement : `Document::sons_proposes`. Hors de l'annulation (une génération qui se termine ne
//!   se défait pas) ; seule la pose du son accepté est une étape d'annulation, et l'état « accepté »
//!   suit alors le son posé (`Indication::statut_reel`).
//! - Fichier : `sons_proposes.json` à côté du manifeste (`project.rs`), sans changer `VERSION` :
//!   absent, il n'y a pas de proposition ; une version qui ne le connaît pas le laisse là ; un
//!   fichier illisible ou plus récent n'est jamais effacé (`.illisible`, `write_sidecar`). Il garde
//!   aussi la confirmation d'envoi en ligne, une fois par projet.

use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Nom du fichier, dans le dossier du projet.
pub(crate) const FILE: &str = "sons_proposes.json";
const FORMAT: &str = "shot-sons-proposes";
const VERSION: u32 = 1;
/// Les textes plus longs sont coupés à la lecture (un fichier abîmé ne gèle pas l'interface).
const TEXTE_MAX: usize = 4_000;
const NOM_MAX: usize = 120;
/// Durée la plus longue d'une indication, en images (une heure à 24 i/s ; un fichier abîmé ne demande
/// pas des siècles de son).
pub(crate) const IMAGES_MAX: usize = 86_400;
/// Le nom de la piste où les sons acceptés se posent.
pub(crate) const PISTE: &str = "Sons proposés";

/// Sorte de son proposé.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Genre {
    #[serde(rename = "bruitage")]
    Bruitage,
    #[serde(rename = "ambiance")]
    Ambiance,
    /// Musique d'intention : une couleur musicale pour le plan, pas la musique du film.
    #[serde(rename = "musique")]
    Musique,
    #[serde(rename = "voix")]
    Voix,
}

impl Genre {
    pub const ALL: [Genre; 4] = [Genre::Bruitage, Genre::Ambiance, Genre::Musique, Genre::Voix];

    pub fn nom(self) -> &'static str {
        match self {
            Genre::Bruitage => "Bruitage",
            Genre::Ambiance => "Ambiance",
            Genre::Musique => "Musique d'intention",
            Genre::Voix => "Voix",
        }
    }

    pub fn icone(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            Genre::Bruitage => ph::LIGHTNING,
            Genre::Ambiance => ph::WIND,
            Genre::Musique => ph::MUSIC_NOTES,
            Genre::Voix => ph::MICROPHONE_STAGE,
        }
    }

    pub fn est_voix(self) -> bool {
        self == Genre::Voix
    }
}

/// Où en est une proposition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Statut {
    #[default]
    #[serde(rename = "proposee")]
    Proposee,
    #[serde(rename = "en_generation")]
    EnGeneration,
    #[serde(rename = "prete")]
    Prete,
    #[serde(rename = "acceptee")]
    Acceptee,
    #[serde(rename = "rejetee")]
    Rejetee,
}

impl Statut {
    pub fn nom(self) -> &'static str {
        match self {
            Statut::Proposee => "proposée",
            Statut::EnGeneration => "en génération",
            Statut::Prete => "prête",
            Statut::Acceptee => "acceptée",
            Statut::Rejetee => "rejetée",
        }
    }
}

/// Une proposition de son, telle qu'une lecture du storyboard la rend (avant de recevoir un numéro).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IndicationBrute {
    pub start: usize,
    pub frames: usize,
    pub kind: Option<Genre>,
    /// Ce que le réalisateur lit (français).
    pub description: String,
    /// Ce que le générateur reçoit (anglais).
    pub prompt: String,
    /// Voix : ce qui est dit, qui le dit, et la voix choisie (vide : celle des préférences).
    pub text: String,
    pub character: String,
    pub voice: String,
}

/// Une proposition de son pour un plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Indication {
    pub id: u64,
    /// Le plan (son id).
    #[serde(rename = "plan")]
    pub plan: u64,
    /// L'image du plan où le son commence, depuis 0.
    #[serde(rename = "image")]
    pub start: usize,
    /// Durée voulue, en images (celle du son généré, une fois posé, est la sienne).
    #[serde(rename = "images")]
    pub frames: usize,
    #[serde(rename = "genre")]
    pub kind: Genre,
    #[serde(rename = "description", default)]
    pub description: String,
    #[serde(rename = "prompt", default)]
    pub prompt: String,
    #[serde(rename = "texte", default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    #[serde(rename = "personnage", default, skip_serializing_if = "String::is_empty")]
    pub character: String,
    #[serde(rename = "voix", default, skip_serializing_if = "String::is_empty")]
    pub voice: String,
    #[serde(rename = "statut", default)]
    pub status: Statut,
    /// Le son généré (dans le cache de SHOT).
    #[serde(rename = "fichier", default, skip_serializing_if = "Option::is_none")]
    pub file: Option<std::path::PathBuf>,
    /// L'extrait posé dans le montage à l'acceptation.
    #[serde(rename = "extrait", default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<u64>,
    /// Modifiée à la main : une relecture du storyboard ne la retire plus.
    #[serde(rename = "modifiee", default, skip_serializing_if = "is_false")]
    pub edited: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Indication {
    /// L'état à montrer : « acceptée » suit le son posé dans le montage (annuler la pose le rend
    /// « prête », la rétablir le rend « acceptée »).
    pub fn statut_reel(&self, doc: &Document) -> Statut {
        let posed = self.clip.is_some_and(|c| doc.clip(c).is_some());
        match self.status {
            Statut::Acceptee if !posed => {
                if self.file.is_some() {
                    Statut::Prete
                } else {
                    Statut::Proposee
                }
            }
            Statut::Prete if posed => Statut::Acceptee,
            s => s,
        }
    }

    /// Ce que le générateur reçoit pour un bruitage, une ambiance ou une musique.
    pub fn prompt_effectif(&self) -> &str {
        if self.prompt.trim().is_empty() {
            &self.description
        } else {
            &self.prompt
        }
    }

    /// La durée voulue, en secondes, pour la cadence `fps`.
    pub fn secondes(&self, fps: u32) -> f32 {
        self.frames as f32 / fps.max(1) as f32
    }

    /// Une courte étiquette pour la liste et les messages : « Claquement de porte ».
    pub fn titre(&self) -> String {
        let source = if self.kind.est_voix() && self.description.trim().is_empty() { &self.text } else { &self.description };
        let mut t: String = source.split_whitespace().collect::<Vec<_>>().join(" ");
        if t.chars().count() > 60 {
            t = t.chars().take(59).collect::<String>() + "…";
        }
        if t.is_empty() {
            t = self.kind.nom().into();
        }
        t
    }

    /// Ce qui fait reconnaître la même proposition d'une lecture à l'autre.
    fn cle(&self) -> (Genre, String) {
        cle_de(self.kind, &self.description, &self.text)
    }
}

fn cle_de(kind: Genre, description: &str, text: &str) -> (Genre, String) {
    (kind, normaliser(if kind.est_voix() { text } else { description }))
}

/// Le texte en minuscules, sans accents ni ponctuation, mots séparés par une espace : pour comparer
/// deux phrases et chercher des mots-clés.
pub(crate) fn normaliser(texte: &str) -> String {
    let mut out = String::with_capacity(texte.len());
    let mut espace = true;
    for c in texte.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' | 'ì' => 'i',
            'ô' | 'ö' | 'ó' | 'ò' | 'õ' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'ÿ' | 'ý' => 'y',
            'œ' => {
                out.push('o');
                'e'
            }
            'æ' => {
                out.push('a');
                'e'
            }
            c => c,
        };
        if c.is_alphanumeric() {
            out.push(c);
            espace = false;
        } else if !espace {
            out.push(' ');
            espace = true;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Ce que les propositions d'un projet gardent, et la confirmation d'envoi en ligne (une fois par projet).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SonsProposes {
    pub indications: Vec<Indication>,
    /// Le réalisateur a confirmé que les images et les textes de ce projet peuvent partir vers un
    /// service en ligne (`suggestionsui.rs`).
    pub envoi_en_ligne_confirme: bool,
}

/// Ce qu'une relecture du storyboard a changé pour un plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fusion {
    pub ajoutees: usize,
    /// Des propositions d'avant, jamais touchées, que la nouvelle lecture ne refait plus.
    pub retirees: usize,
    /// Des propositions déjà là (quel que soit leur état, une rejetée comprise) : pas refaites.
    pub connues: usize,
}

impl SonsProposes {
    pub fn is_empty(&self) -> bool {
        self.indications.is_empty()
    }

    pub fn get(&self, id: u64) -> Option<&Indication> {
        self.indications.iter().find(|i| i.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Indication> {
        self.indications.iter_mut().find(|i| i.id == id)
    }

    fn id_neuf(&self) -> u64 {
        self.indications.iter().map(|i| i.id).max().unwrap_or(0) + 1
    }

    /// Ajoute une proposition à `plan` ; rend son numéro. Une sorte absente est un bruitage.
    pub fn ajouter(&mut self, plan: u64, brute: IndicationBrute) -> u64 {
        let id = self.id_neuf();
        let kind = brute.kind.unwrap_or(Genre::Bruitage);
        self.indications.push(Indication {
            id,
            plan,
            start: brute.start,
            frames: brute.frames.clamp(1, IMAGES_MAX),
            kind,
            description: brute.description,
            prompt: brute.prompt,
            text: brute.text,
            character: brute.character,
            voice: brute.voice,
            status: Statut::Proposee,
            file: None,
            clip: None,
            edited: false,
        });
        id
    }

    /// Range une lecture de `plan` : les propositions neuves sont ajoutées ; celles qu'on connaît déjà
    /// (même sorte, même description ou même réplique) ne sont pas refaites, rejetées comprises — un
    /// son que le réalisateur a refusé ne revient pas ; les anciennes propositions restées intactes et
    /// que la lecture ne refait plus sont retirées (le texte du plan a changé).
    pub fn fusionner(&mut self, plan: u64, nouvelles: Vec<IndicationBrute>) -> Fusion {
        let mut fusion = Fusion::default();
        let cles: Vec<(Genre, String)> = nouvelles.iter().map(|b| cle_de(b.kind.unwrap_or(Genre::Bruitage), &b.description, &b.text)).collect();
        let avant = self.indications.len();
        self.indications.retain(|i| i.plan != plan || i.status != Statut::Proposee || i.edited || i.file.is_some() || cles.contains(&i.cle()));
        fusion.retirees = avant - self.indications.len();
        for (brute, cle) in nouvelles.into_iter().zip(cles) {
            if self.indications.iter().any(|i| i.plan == plan && i.cle() == cle) {
                fusion.connues += 1;
            } else {
                self.ajouter(plan, brute);
                fusion.ajoutees += 1;
            }
        }
        fusion
    }

    pub fn supprimer(&mut self, id: u64) -> bool {
        let n = self.indications.len();
        self.indications.retain(|i| i.id != id);
        self.indications.len() != n
    }

    /// Les numéros des propositions de `plan`.
    pub fn du_plan(&self, plan: u64) -> impl Iterator<Item = &Indication> {
        self.indications.iter().filter(move |i| i.plan == plan)
    }
}

/// L'image du montage où commence la proposition (le début de son plan plus son image), si son plan
/// existe. Elle ne dépasse pas la dernière image du plan.
pub(crate) fn debut_montage(doc: &Document, ind: &Indication) -> Option<usize> {
    let index = doc.panels.iter().position(|p| p.id == ind.plan)?;
    Some(doc.panel_start(index) + ind.start.min(doc.panels[index].frame_count() - 1))
}

/// Est-ce une piste des sons proposés ? « Sons proposés », ou « Sons proposés 2 » quand la première
/// est occupée à cet endroit.
fn est_piste_proposee(nom: &str) -> bool {
    nom == PISTE || nom.strip_prefix(PISTE).and_then(|s| s.strip_prefix(' ')).is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// La piste où poser un son accepté sur `[start, end)` : la première piste « Sons proposés » libre à cet
/// endroit et non verrouillée ; sinon une piste neuve du même nom (numérotée si « Sons proposés »
/// existe déjà). Rend son rang. Les autres pistes ne sont jamais touchées.
pub(crate) fn piste_pour(doc: &mut Document, start: usize, end: usize) -> usize {
    let libre = (0..doc.audio_tracks.len()).find(|&t| est_piste_proposee(&doc.audio_tracks[t].name) && !doc.audio_tracks[t].locked && doc.track_free(t, start, end, 0));
    if let Some(t) = libre {
        return t;
    }
    doc.add_audio_track();
    let t = doc.audio_tracks.len() - 1;
    let existe = |doc: &Document, nom: &str| doc.audio_tracks.iter().any(|p| p.name == nom);
    let mut nom = PISTE.to_string();
    let mut k = 1;
    while existe(doc, &nom) {
        k += 1;
        nom = format!("{PISTE} {k}");
    }
    doc.audio_tracks[t].name = nom;
    t
}

// ------------------------------------------------------------------------------------- fichier

#[derive(Serialize, Deserialize)]
struct Sidecar {
    format: String,
    version: u32,
    #[serde(default, rename = "envoi_en_ligne_confirme")]
    envoi_en_ligne_confirme: bool,
    #[serde(default)]
    indications: Vec<Indication>,
}

/// Le contenu de `sons_proposes.json` pour `doc` ; rien s'il n'y a ni proposition ni confirmation (le
/// fichier est alors retiré). Les propositions d'un plan qui n'existe plus ne sont pas écrites.
pub(crate) fn to_bytes(doc: &Document) -> Result<Option<Vec<u8>>, String> {
    let order: std::collections::HashMap<u64, usize> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
    let mut list: Vec<Indication> = doc.sons_proposes.indications.iter().filter(|i| order.contains_key(&i.plan)).cloned().collect();
    if list.is_empty() && !doc.sons_proposes.envoi_en_ligne_confirme {
        return Ok(None);
    }
    // Dans l'ordre du film : le fichier se lit, et deux enregistrements se ressemblent.
    list.sort_by_key(|i| (order[&i.plan], i.start, i.id));
    let sidecar = Sidecar { format: FORMAT.into(), version: VERSION, envoi_en_ligne_confirme: doc.sons_proposes.envoi_en_ligne_confirme, indications: list };
    serde_json::to_vec_pretty(&sidecar).map(Some).map_err(|e| e.to_string())
}

fn couper(s: &mut String, max: usize) {
    if s.chars().count() > max {
        *s = s.chars().take(max).collect();
    }
}

/// Les propositions lues dans le texte de `sons_proposes.json`. Numéros uniques et non nuls (un
/// doublon reçoit un numéro neuf), textes et durées bornés ; une génération restée « en cours » (SHOT
/// s'est fermé pendant) redevient prête si son fichier est là, proposée sinon.
pub(crate) fn parse(text: &str) -> Result<SonsProposes, String> {
    let sidecar: Sidecar = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("illisible ({e})"))?;
    if sidecar.format != FORMAT {
        return Err("ce n'est pas un fichier de sons proposés".into());
    }
    if sidecar.version > VERSION {
        return Err(format!("écrit par une version plus récente (format {})", sidecar.version));
    }
    let mut next = sidecar.indications.iter().map(|i| i.id).max().unwrap_or(0) + 1;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(sidecar.indications.len());
    for mut i in sidecar.indications {
        if i.id == 0 || !seen.insert(i.id) {
            i.id = next;
            next += 1;
            seen.insert(i.id);
        }
        i.frames = i.frames.clamp(1, IMAGES_MAX);
        i.start = i.start.min(10_000_000);
        for (s, max) in [(&mut i.description, TEXTE_MAX), (&mut i.prompt, TEXTE_MAX), (&mut i.text, TEXTE_MAX), (&mut i.character, NOM_MAX), (&mut i.voice, NOM_MAX)] {
            couper(s, max);
        }
        if i.status == Statut::EnGeneration {
            i.status = if i.file.is_some() { Statut::Prete } else { Statut::Proposee };
        }
        if i.status == Statut::Prete && i.file.is_none() {
            i.status = Statut::Proposee;
        }
        out.push(i);
    }
    Ok(SonsProposes { indications: out, envoi_en_ligne_confirme: sidecar.envoi_en_ligne_confirme })
}

/// Après la lecture du fichier, sur le fil d'ouverture du projet : un son prêt dont le fichier n'est
/// plus là (cache vidé, autre ordinateur) redevient une proposition à générer.
pub(crate) fn verifier_fichiers(s: &mut SonsProposes) {
    for i in &mut s.indications {
        if i.file.as_ref().is_some_and(|f| !f.is_file()) {
            i.file = None;
            if i.status == Statut::Prete {
                i.status = Statut::Proposee;
            }
        }
    }
}
