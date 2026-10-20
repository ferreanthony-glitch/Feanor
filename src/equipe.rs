//! L'équipier : qui je suis dans un film partagé (`docs/COOPERATION.md`, §16).
//!
//! Un nom, une couleur, un identifiant tiré au hasard à la première utilisation (jamais le nom de l'utilisateur de Windows,
//! qu'un dossier partagé n'a pas à connaître) ; une **séance** par fenêtre SHOT ouverte, tirée au hasard aussi : deux
//! fenêtres d'un même poste sont deux séances, qui se disputent les plans comme deux personnes (on peut ainsi essayer le
//! partage à deux fenêtres).

use std::hash::{BuildHasher, Hasher};

use serde::{Deserialize, Serialize};

/// Réglages de l'équipe, dans `reglages.toml` (Préférences > Équipe).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReglageEquipe {
    /// Identifiant d'équipier, tiré au hasard à la première utilisation (vide : pas encore tiré).
    #[serde(default, rename = "identifiant", skip_serializing_if = "String::is_empty")]
    pub id: String,
    /// Le nom montré aux autres ; vide : le nom des commentaires, sinon l'utilisateur de Windows.
    #[serde(default, rename = "nom", skip_serializing_if = "String::is_empty")]
    pub nom: String,
    /// Ma couleur ; absente : tirée de l'identifiant.
    #[serde(default, rename = "couleur", skip_serializing_if = "Option::is_none")]
    pub couleur: Option<[u8; 3]>,
    /// Minutes sans renouvellement après lesquelles une prise est périmée (§5.2).
    #[serde(default = "bail_defaut", rename = "bail_minutes")]
    pub bail_minutes: u32,
    /// Secondes entre deux balayages du dossier partagé (§7).
    #[serde(default = "balayage_defaut", rename = "balayage_secondes")]
    pub balayage_secondes: u32,
    /// Prendre le plan libre que je commence à modifier, au lieu de refuser (§5.5).
    #[serde(default, rename = "prise_automatique")]
    pub prise_automatique: bool,
    /// Dire dans la barre d'état ce que font les autres (« Léa a enregistré le plan 12 »).
    #[serde(default = "oui", rename = "notifications")]
    pub notifications: bool,
}

fn bail_defaut() -> u32 {
    10
}

fn balayage_defaut() -> u32 {
    3
}

fn oui() -> bool {
    true
}

impl Default for ReglageEquipe {
    fn default() -> Self {
        ReglageEquipe { id: String::new(), nom: String::new(), couleur: None, bail_minutes: bail_defaut(), balayage_secondes: balayage_defaut(), prise_automatique: false, notifications: true }
    }
}

impl ReglageEquipe {
    /// Valeurs dans des bornes sûres (un fichier modifié à la main ne fait pas balayer le disque dix fois par seconde).
    pub fn nettoye(mut self) -> Self {
        self.bail_minutes = self.bail_minutes.clamp(2, 240);
        self.balayage_secondes = self.balayage_secondes.clamp(1, 120);
        self.id = self.id.chars().filter(|c| c.is_ascii_alphanumeric()).take(32).collect();
        self.nom = self.nom.chars().filter(|c| !c.is_control()).take(40).collect();
        self
    }
}

/// Qui je suis pour la durée de cette fenêtre.
#[derive(Clone, Debug, PartialEq)]
pub struct Identite {
    pub id: String,
    pub nom: String,
    pub couleur: [u8; 3],
    pub machine: String,
    pub seance: String,
}

/// Une clé qui désigne mon équipier + ma séance (le nom de ma fiche).
impl Identite {
    pub fn cle(&self) -> Cle {
        Cle { equipier: self.id.clone(), seance: self.seance.clone() }
    }
}

/// Un équipier dans une séance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cle {
    #[serde(rename = "equipier")]
    pub equipier: String,
    #[serde(rename = "seance")]
    pub seance: String,
}

/// Un nombre au hasard (pas pour la sécurité : des identifiants qui ne se rencontrent pas).
pub fn aleatoire() -> u64 {
    static COMPTEUR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
    h.write_u32(std::process::id());
    h.write_u64(COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    h.finish()
}

/// Un identifiant neuf : 16 chiffres hexadécimaux.
pub fn nouvel_id() -> String {
    format!("{:016x}", aleatoire())
}

/// Une couleur pour un identifiant, parmi une palette de tons nets (lisibles sur le gris du montage).
pub fn couleur_pour(id: &str) -> [u8; 3] {
    const PALETTE: [[u8; 3]; 8] = [[231, 76, 60], [52, 152, 219], [46, 204, 113], [241, 196, 15], [155, 89, 182], [230, 126, 34], [26, 188, 156], [236, 112, 183]];
    let n = id.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(u32::from(b)));
    PALETTE[n as usize % PALETTE.len()]
}

/// Mon identité : les réglages, complétés (nom des commentaires, utilisateur de Windows, couleur de l'identifiant).
/// `id` vide : un identifiant temporaire (l'appelant le retient dans les réglages).
pub fn identite(reglage: &ReglageEquipe, nom_commentaires: &str) -> Identite {
    let id = if reglage.id.is_empty() { nouvel_id() } else { reglage.id.clone() };
    let nom = if !reglage.nom.trim().is_empty() { reglage.nom.trim().to_string() } else { crate::commentaires::author_name(nom_commentaires) };
    let couleur = reglage.couleur.unwrap_or_else(|| couleur_pour(&id));
    let machine = std::env::var("COMPUTERNAME").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "poste".into());
    Identite { id, nom, couleur, machine, seance: format!("{:08x}", aleatoire() as u32) }
}

/// Deux lettres pour une pastille : les initiales du nom (« Léa Martin » → « LM », « Anthony » → « AN »).
pub fn initiales(nom: &str) -> String {
    let mots: Vec<&str> = nom.split_whitespace().collect();
    let mut s: String = match mots.as_slice() {
        [] => "?".into(),
        [seul] => seul.chars().take(2).collect(),
        [premier, .., dernier] => premier.chars().take(1).chain(dernier.chars().take(1)).collect(),
    };
    s = s.to_uppercase();
    s
}

/// Millisecondes depuis 1970.
pub fn maintenant_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}
