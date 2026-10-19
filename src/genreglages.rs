//! Les réglages de la génération sonore (Anthony, 2026-10-19), dans `reglages.toml`, section
//! `[generation_son]` : pour chacun des trois rôles, un fournisseur **local** (le serveur de cet
//! ordinateur) ou un **service en ligne** ; la lecture du storyboard a en plus la lecture **par règles**,
//! sans modèle, qui est son choix d'origine (elle marche sans rien installer).
//!
//! **Jamais de clé ici** : les clés sont dans le Gestionnaire d'identifiants de Windows
//! (`clessecretes.rs`). Ce fichier ne dit que quel service est choisi.

use serde::{Deserialize, Serialize};

use crate::clessecretes::Service;
use crate::fournisseursapi::MODELE_CLAUDE;
use crate::sidecar::PORT_PAR_DEFAUT;

/// Les trois rôles de la génération sonore.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Lecture,
    Son,
    Voix,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Lecture, Role::Son, Role::Voix];

    pub fn nom(self) -> &'static str {
        match self {
            Role::Lecture => "Lecture du storyboard",
            Role::Son => "Bruitages et ambiances",
            Role::Voix => "Voix témoin des dialogues",
        }
    }

    /// Les services en ligne qui savent faire ce rôle.
    pub fn services(self) -> &'static [Service] {
        match self {
            Role::Lecture => &[Service::Claude],
            Role::Son => &[Service::ElevenLabs, Service::StableAudio],
            Role::Voix => &[Service::ElevenLabs],
        }
    }

    /// Le modèle que le serveur local installera pour ce rôle (phase 2).
    pub fn modele_local(self) -> &'static str {
        match self {
            Role::Lecture => "Qwen2.5-VL",
            Role::Son => "Stable Audio Open",
            Role::Voix => "Kokoro-82M",
        }
    }
}

/// Qui fait le travail d'un rôle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Lecture par règles, sans modèle (la lecture du storyboard seulement).
    #[serde(rename = "regles")]
    Regles,
    /// Le serveur local, sur cet ordinateur.
    #[serde(rename = "local")]
    Local,
    /// Un service en ligne.
    #[serde(rename = "en_ligne")]
    EnLigne,
}

impl Mode {
    pub fn nom(self) -> &'static str {
        match self {
            Mode::Regles => "Règles (sans modèle)",
            Mode::Local => "Local",
            Mode::EnLigne => "Service en ligne",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GenerationSonReglages {
    #[serde(rename = "lecture")]
    pub lecture: Mode,
    #[serde(rename = "lecture_service")]
    pub lecture_service: Service,
    #[serde(rename = "sons")]
    pub son: Mode,
    #[serde(rename = "sons_service")]
    pub son_service: Service,
    #[serde(rename = "voix")]
    pub voix: Mode,
    #[serde(rename = "voix_service")]
    pub voix_service: Service,
    /// Le port du serveur local, sur 127.0.0.1.
    #[serde(rename = "port_local")]
    pub port_local: u16,
    /// Le modèle de Claude qui lit les plans (à vérifier avec le compte de l'utilisateur).
    #[serde(rename = "modele_claude")]
    pub modele_claude: String,
    /// Joindre la vignette du plan à la lecture (modèle local ou Claude). Sans elle, seuls les textes sont lus.
    #[serde(rename = "envoyer_vignettes")]
    pub vignettes: bool,
    /// La voix de Kokoro par défaut (le serveur local), et l'identifiant de voix d'ElevenLabs par défaut.
    #[serde(rename = "voix_locale")]
    pub voix_locale: String,
    #[serde(rename = "voix_en_ligne")]
    pub voix_en_ligne: String,
}

impl Default for GenerationSonReglages {
    fn default() -> Self {
        GenerationSonReglages {
            lecture: Mode::Regles,
            lecture_service: Service::Claude,
            son: Mode::Local,
            son_service: Service::ElevenLabs,
            voix: Mode::Local,
            voix_service: Service::ElevenLabs,
            port_local: PORT_PAR_DEFAUT,
            modele_claude: MODELE_CLAUDE.into(),
            vignettes: true,
            voix_locale: "ff_siwis".into(),
            voix_en_ligne: String::new(),
        }
    }
}

impl GenerationSonReglages {
    pub fn mode(&self, role: Role) -> Mode {
        match role {
            Role::Lecture => self.lecture,
            Role::Son => self.son,
            Role::Voix => self.voix,
        }
    }

    pub fn service(&self, role: Role) -> Service {
        match role {
            Role::Lecture => self.lecture_service,
            Role::Son => self.son_service,
            Role::Voix => self.voix_service,
        }
    }

    pub fn definir(&mut self, role: Role, mode: Mode) {
        match role {
            Role::Lecture => self.lecture = mode,
            Role::Son => self.son = mode,
            Role::Voix => self.voix = mode,
        }
    }

    pub fn definir_service(&mut self, role: Role, service: Service) {
        if !role.services().contains(&service) {
            return;
        }
        match role {
            Role::Lecture => self.lecture_service = service,
            Role::Son => self.son_service = service,
            Role::Voix => self.voix_service = service,
        }
    }

    /// Le service en ligne que ce rôle utilise, s'il est en ligne.
    pub fn en_ligne(&self, role: Role) -> Option<Service> {
        (self.mode(role) == Mode::EnLigne).then(|| self.service(role))
    }

    /// Les rôles en ligne et leur service.
    pub fn roles_en_ligne(&self) -> Vec<(Role, Service)> {
        Role::ALL.into_iter().filter_map(|r| self.en_ligne(r).map(|s| (r, s))).collect()
    }

    /// La voix par défaut selon le fournisseur des voix.
    pub fn voix_par_defaut(&self) -> &str {
        if self.voix == Mode::EnLigne {
            &self.voix_en_ligne
        } else {
            &self.voix_locale
        }
    }

    /// Ramène chaque valeur dans ce qui existe (un fichier modifié à la main) : un rôle sans règles,
    /// un service qui ne sait pas faire ce rôle, un port nul, un nom de modèle vide.
    pub fn sanitized(mut self) -> Self {
        if self.son == Mode::Regles {
            self.son = Mode::Local;
        }
        if self.voix == Mode::Regles {
            self.voix = Mode::Local;
        }
        for role in Role::ALL {
            if !role.services().contains(&self.service(role)) {
                let premier = role.services()[0];
                self.definir_service(role, premier);
            }
        }
        if self.port_local == 0 {
            self.port_local = PORT_PAR_DEFAUT;
        }
        if self.modele_claude.trim().is_empty() {
            self.modele_claude = MODELE_CLAUDE.into();
        }
        self.modele_claude = self.modele_claude.trim().chars().take(80).collect();
        self.voix_locale = self.voix_locale.trim().chars().take(80).collect();
        self.voix_en_ligne = self.voix_en_ligne.trim().chars().take(80).collect();
        self
    }
}
