//! Le compteur d'usage (2026-11-06, demande d'Anthony : « mesurer ce que tu utilises vraiment ») : la partie sans
//! interface. Chaque commande lancée est comptée, avec le chemin par lequel elle est partie (le clavier, une icône, un
//! menu, l'anneau sous le stylet, la palette). Après quelques jours, les chiffres disent quelles commandes servent tout le
//! temps (bonnes pour une touche ou pour l'anneau) et lesquelles ne servent jamais (bonnes à cacher).
//!
//! **Rien ne quitte le poste.** Le compte est un petit fichier à côté des réglages (`%APPDATA%\SHOT\usage.json`) :
//! FEANOR ne l'envoie nulle part, et n'y range que des identifiants de commandes et des nombres (aucun nom de projet, aucun
//! texte, aucun dessin). Il se remet à zéro et s'éteint dans la fenêtre « Mes habitudes ».
//!
//! Le **journal des dernières commandes** ([`Usage::recentes`]) reste en mémoire, jamais écrit ici : il sert à la note de
//! friction (`friction.rs`), qui dit ce qu'on venait de faire.
//!
//! L'application (compter, écrire au repos, la fenêtre) est dans `usageui.rs`. Essais : `usage_tests.rs`.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

/// Le fichier du compte, à côté des réglages.
pub const FICHIER: &str = "usage.json";
/// Combien de commandes le journal en mémoire retient.
pub const RECENTES: usize = 40;
/// Une même commande vue par deux chemins à moins de ce délai (le clic sur son icône, puis la commande qu'il lance) n'est
/// comptée qu'une fois, en secondes.
pub const DOUBLON: f64 = 0.25;

/// Par où une commande est partie.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Clavier,
    Icone,
    Menu,
    Anneau,
    Palette,
    /// Un chemin que le compteur ne sait pas nommer (un bouton d'un panneau, une commande lancée par une autre).
    Autre,
}

impl Source {
    pub const TOUTES: [Source; 6] = [Source::Clavier, Source::Icone, Source::Menu, Source::Anneau, Source::Palette, Source::Autre];

    pub fn nom(self) -> &'static str {
        match self {
            Source::Clavier => "clavier",
            Source::Icone => "icône",
            Source::Menu => "menu",
            Source::Anneau => "anneau",
            Source::Palette => "palette",
            Source::Autre => "autre",
        }
    }
}

/// Le compte d'une commande, par chemin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Compte {
    #[serde(skip_serializing_if = "est_zero")]
    pub clavier: u32,
    #[serde(skip_serializing_if = "est_zero")]
    pub icone: u32,
    #[serde(skip_serializing_if = "est_zero")]
    pub menu: u32,
    #[serde(skip_serializing_if = "est_zero")]
    pub anneau: u32,
    #[serde(skip_serializing_if = "est_zero")]
    pub palette: u32,
    #[serde(skip_serializing_if = "est_zero")]
    pub autre: u32,
    /// Le dernier jour où elle a servi (AAAA-MM-JJ).
    pub dernier: String,
}

fn est_zero(n: &u32) -> bool {
    *n == 0
}

impl Compte {
    pub fn de(&self, source: Source) -> u32 {
        match source {
            Source::Clavier => self.clavier,
            Source::Icone => self.icone,
            Source::Menu => self.menu,
            Source::Anneau => self.anneau,
            Source::Palette => self.palette,
            Source::Autre => self.autre,
        }
    }

    fn case(&mut self, source: Source) -> &mut u32 {
        match source {
            Source::Clavier => &mut self.clavier,
            Source::Icone => &mut self.icone,
            Source::Menu => &mut self.menu,
            Source::Anneau => &mut self.anneau,
            Source::Palette => &mut self.palette,
            Source::Autre => &mut self.autre,
        }
    }

    pub fn total(&self) -> u32 {
        Source::TOUTES.iter().map(|s| self.de(*s)).sum()
    }

    /// « clavier 12 · icône 3 » : les chemins qui ont servi, du plus fréquent au moins fréquent.
    pub fn detail(&self) -> String {
        let mut chemins: Vec<(Source, u32)> = Source::TOUTES.iter().map(|s| (*s, self.de(*s))).filter(|(_, n)| *n > 0).collect();
        chemins.sort_by(|a, b| b.1.cmp(&a.1));
        chemins.iter().map(|(s, n)| format!("{} {n}", s.nom())).collect::<Vec<_>>().join(" · ")
    }

    /// Le chemin le plus fréquent de cette commande.
    pub fn chemin_principal(&self) -> Option<Source> {
        Source::TOUTES.iter().copied().filter(|s| self.de(*s) > 0).max_by_key(|s| self.de(*s))
    }
}

/// Une commande du journal en mémoire.
#[derive(Clone, Debug, PartialEq)]
pub struct Recente {
    pub action: String,
    pub source: Source,
    /// L'instant où elle est partie, en secondes (horloge d'egui).
    pub instant: f64,
}

/// Le compte de ce poste.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Usage {
    /// Le jour où le comptage a commencé (AAAA-MM-JJ) ; vide tant que rien n'a été compté.
    pub depuis: String,
    /// Le compte de chaque commande, par identifiant (`keymap::ACTIONS`).
    pub commandes: BTreeMap<String, Compte>,
    /// Les dernières commandes, la plus récente à la fin. En mémoire seulement.
    #[serde(skip)]
    pub recentes: VecDeque<Recente>,
    /// Le compte a changé depuis la dernière écriture.
    #[serde(skip)]
    pub a_ecrire: bool,
}

impl Usage {
    /// Le compte lu dans son fichier. Un fichier absent, abîmé ou d'une autre version : un compte neuf, sans rien casser.
    pub fn lire(texte: &str) -> Usage {
        serde_json::from_str(texte).unwrap_or_default()
    }

    pub fn ecrire(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Une commande vient de partir. `compter` faux : elle entre au journal sans être comptée (compteur éteint).
    pub fn noter(&mut self, action: &str, source: Source, jour: &str, instant: f64, compter: bool) {
        if action.is_empty() {
            return;
        }
        // Le même geste vu par deux chemins : une seule fois, sous le chemin qui a un nom.
        if let Some(d) = self.recentes.back_mut().filter(|d| d.action == action && (instant - d.instant).abs() < DOUBLON && d.source != source) {
            if d.source == Source::Autre {
                d.source = source;
                if compter {
                    if let Some(c) = self.commandes.get_mut(action) {
                        if c.autre > 0 {
                            c.autre -= 1;
                            *c.case(source) += 1;
                            self.a_ecrire = true;
                        }
                    }
                }
                return;
            }
            if source == Source::Autre {
                return;
            }
        }
        self.recentes.push_back(Recente { action: action.to_string(), source, instant });
        while self.recentes.len() > RECENTES {
            self.recentes.pop_front();
        }
        if !compter {
            return;
        }
        if self.depuis.is_empty() {
            self.depuis = jour.to_string();
        }
        let c = self.commandes.entry(action.to_string()).or_default();
        *c.case(source) += 1;
        if c.dernier != jour {
            c.dernier = jour.to_string();
        }
        self.a_ecrire = true;
    }

    /// Les `n` commandes les plus utilisées, de la plus fréquente à la moins fréquente (à égalité : par identifiant).
    pub fn plus_utilisees(&self, n: usize) -> Vec<(&str, &Compte)> {
        let mut v: Vec<(&str, &Compte)> = self.commandes.iter().map(|(a, c)| (a.as_str(), c)).filter(|(_, c)| c.total() > 0).collect();
        v.sort_by(|a, b| b.1.total().cmp(&a.1.total()).then(a.0.cmp(b.0)));
        v.truncate(n);
        v
    }

    /// Parmi ces commandes, celles qui n'ont jamais servi, dans l'ordre donné.
    pub fn jamais<'a>(&self, parmi: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
        parmi.into_iter().filter(|a| self.commandes.get(*a).is_none_or(|c| c.total() == 0)).collect()
    }

    /// Le nombre de commandes comptées en tout.
    pub fn total(&self) -> u64 {
        self.commandes.values().map(|c| u64::from(c.total())).sum()
    }

    /// Depuis combien de jours on compte (0 le premier jour) ; rien tant que le comptage n'a pas commencé.
    pub fn jours(&self, aujourd_hui: &str) -> Option<i64> {
        Some((numero_du_jour(aujourd_hui)? - numero_du_jour(&self.depuis)?).max(0))
    }

    /// Remet le compte à zéro (le journal en mémoire reste).
    pub fn effacer(&mut self) {
        self.depuis.clear();
        self.commandes.clear();
        self.a_ecrire = true;
    }

    /// Les `n` dernières commandes du journal, de la plus ancienne à la plus récente.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn dernieres(&self, n: usize) -> impl Iterator<Item = &Recente> {
        self.recentes.iter().skip(self.recentes.len().saturating_sub(n))
    }
}

/// Le numéro d'un jour « AAAA-MM-JJ » dans le calendrier (jours depuis le 1er janvier 1970), pour compter un écart.
pub fn numero_du_jour(jour: &str) -> Option<i64> {
    let mut parties = jour.split('-').map(|p| p.parse::<i64>().ok());
    let (a, m, j) = (parties.next()??, parties.next()??, parties.next()??);
    if !(1..=12).contains(&m) || !(1..=31).contains(&j) {
        return None;
    }
    // (L'année commence en mars : le jour bissextile tombe à la fin.)
    let a = if m <= 2 { a - 1 } else { a };
    let ere = a.div_euclid(400);
    let an = a.rem_euclid(400);
    let jour_de_l_an = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + j - 1;
    let jour_de_l_ere = an * 365 + an / 4 - an / 100 + jour_de_l_an;
    Some(ere * 146_097 + jour_de_l_ere - 719_468)
}

/// « 6 novembre 2026 » pour « 2026-11-06 » (le texte tel quel s'il ne se lit pas).
pub fn jour_en_clair(jour: &str) -> String {
    const MOIS: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
    let p: Vec<&str> = jour.split('-').collect();
    match (p.first().and_then(|a| a.parse::<i64>().ok()), p.get(1).and_then(|m| m.parse::<usize>().ok()), p.get(2).and_then(|j| j.parse::<u32>().ok())) {
        (Some(a), Some(m), Some(j)) if (1..=12).contains(&m) => format!("{}{} {} {a}", j, if j == 1 { "er" } else { "" }, MOIS[m - 1]),
        _ => jour.to_string(),
    }
}
