//! Le sens de lecture de la planche des Vignettes et les textes qui s'affichent à côté des cases (Anthony, 2026-10-24, le panneau
//! « Vignettes »), sans interface.
//!
//! - **Sens de lecture** (`Sens`) : « En lignes » (d'origine) range les cases de gauche à droite, rangée par rangée, leur texte dessous ;
//!   « À la japonaise » les range de haut en bas, colonne par colonne (la colonne suivante est à droite), leur texte à droite de la case,
//!   comme une feuille d'e-konte (`boardjapon.rs`).
//! - **Textes** (`Textes`) : lesquels des champs du plan (l'action, le dialogue, les notes : ceux de la fiche) se montrent à côté des cases.
//!   D'origine, les notes seules (ce qui se voyait avant le panneau). **Chaque champ coché fait exactement une ligne** sous la case (à sa
//!   droite à la japonaise), dans l'ordre de la fiche ; un texte plus long que la ligne est coupé par « … » ; **aucun champ coché : aucune
//!   ligne** et la place sous les cases se resserre (Anthony, 2026-10-24). Quand plusieurs champs sont montrés, chacun porte une courte
//!   étiquette colorée ; un champ seul reste sans étiquette, comme avant.
//!
//! Ces deux réglages sont des réglages de l'application, dans la disposition de la fenêtre (`Layout::board_sens`, `Layout::board_textes`),
//! à côté de la taille des cases et de leur nombre par ligne : ils suivent l'utilisateur, pas le projet.

use serde::{Deserialize, Serialize};

use crate::document::Document;

/// Le sens de lecture de la planche.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sens {
    /// Les cases de gauche à droite, puis la rangée suivante dessous ; le texte sous la case.
    #[default]
    #[serde(rename = "lignes")]
    Lignes,
    /// Les cases de haut en bas, puis la colonne suivante à droite ; le texte à droite de la case.
    #[serde(rename = "japonais")]
    Japonais,
}

impl Sens {
    pub const TOUS: [Sens; 2] = [Sens::Lignes, Sens::Japonais];

    pub fn nom(self) -> &'static str {
        match self {
            Sens::Lignes => "En lignes",
            Sens::Japonais => "À la japonaise",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Sens::Lignes => "Les cases se suivent de gauche à droite, puis la ligne suivante dessous ; leur texte est sous chaque case.",
            Sens::Japonais => "Les cases se suivent de haut en bas, puis la colonne suivante à droite (comme une feuille d'e-konte) ; leur texte est à droite de chaque case.",
        }
    }

    pub fn japonais(self) -> bool {
        self == Sens::Japonais
    }
}

/// Un champ de texte d'un plan (ou d'une marque d'image).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Champ {
    Action,
    Dialogue,
    Notes,
}

impl Champ {
    pub const TOUS: [Champ; 3] = [Champ::Action, Champ::Dialogue, Champ::Notes];

    pub fn nom(self) -> &'static str {
        match self {
            Champ::Action => "Action",
            Champ::Dialogue => "Dialogue",
            Champ::Notes => "Notes",
        }
    }

    /// L'étiquette courte, devant le texte quand plusieurs champs sont montrés.
    pub fn etiquette(self) -> &'static str {
        match self {
            Champ::Action => "Action",
            Champ::Dialogue => "Dialogue",
            Champ::Notes => "Note",
        }
    }

    /// La couleur de son étiquette (lisible sur le papier blanc).
    pub fn encre(self) -> [u8; 3] {
        match self {
            Champ::Action => [48, 96, 160],
            Champ::Dialogue => [160, 88, 24],
            Champ::Notes => [92, 92, 92],
        }
    }
}

/// Les champs montrés à côté des cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Textes {
    pub action: bool,
    pub dialogue: bool,
    pub notes: bool,
}

impl Default for Textes {
    /// Les notes seules : la valeur de la mise en page (`board::Options`, export) quand rien n'est demandé. Le réglage de l'application, lui,
    /// n'a aucun champ coché d'origine depuis le 2026-10-30 (`Textes::aucun`, `Layout::default`).
    fn default() -> Self {
        Textes { action: false, dialogue: false, notes: true }
    }
}

impl Textes {
    /// Aucun champ : la planche n'est que du dessin tant qu'on n'a pas coché Action, Dialogue ou Notes (Anthony, 2026-10-30).
    pub fn aucun() -> Textes {
        Textes { action: false, dialogue: false, notes: false }
    }

    pub fn montre(self, c: Champ) -> bool {
        match c {
            Champ::Action => self.action,
            Champ::Dialogue => self.dialogue,
            Champ::Notes => self.notes,
        }
    }

    pub fn regler(&mut self, c: Champ, on: bool) {
        match c {
            Champ::Action => self.action = on,
            Champ::Dialogue => self.dialogue = on,
            Champ::Notes => self.notes = on,
        }
    }

    /// Les champs montrés, dans l'ordre de la fiche : action, dialogue, notes.
    pub fn champs(self) -> Vec<Champ> {
        Champ::TOUS.into_iter().filter(|c| self.montre(*c)).collect()
    }

    /// Chaque champ porte son étiquette quand plusieurs sont montrés (un champ seul n'en a pas besoin).
    pub fn etiquettes(self) -> bool {
        self.champs().len() >= 2
    }

    /// Les lignes de la bande de texte d'une case : une par champ montré, qu'il ait un texte ou non (zéro : pas de bande).
    pub fn lignes(self) -> usize {
        self.champs().len()
    }

    /// La ligne qu'occupe le champ `c` dans la bande (0 : la première), s'il est montré.
    pub fn ligne_du(self, c: Champ) -> Option<usize> {
        self.champs().iter().position(|x| *x == c)
    }

    /// Une valeur qui change avec les champs montrés (clés de mémoire).
    pub fn bits(self) -> u8 {
        u8::from(self.action) | u8::from(self.dialogue) << 1 | u8::from(self.notes) << 2
    }
}

/// Les champs qui ont un texte quelque part dans le film : le plan ou l'une de ses images (marques).
pub fn champs_remplis(doc: &Document) -> Textes {
    let plein = |s: &str| !s.trim().is_empty();
    let mut t = Textes::aucun();
    for p in &doc.panels {
        t.action |= plein(&p.action) || p.marks.iter().any(|m| plein(&m.action));
        t.dialogue |= plein(&p.dialogue) || p.marks.iter().any(|m| plein(&m.dialogue));
        t.notes |= plein(&p.notes) || p.marks.iter().any(|m| plein(&m.notes));
    }
    t
}

/// Le texte du champ `champ` de la case `id` du plan `panel` (son plan pour sa première case, sa marque d'image sinon).
pub fn texte_du_champ(doc: &Document, panel: usize, id: u64, frame: usize, champ: Champ) -> &str {
    let Some(p) = doc.panels.get(panel) else { return "" };
    if frame == 0 {
        match champ {
            Champ::Action => &p.action,
            Champ::Dialogue => &p.dialogue,
            Champ::Notes => &p.notes,
        }
    } else {
        p.marks.iter().find(|m| m.id == id).map_or("", |m| match champ {
            Champ::Action => m.action.as_str(),
            Champ::Dialogue => m.dialogue.as_str(),
            Champ::Notes => m.notes.as_str(),
        })
    }
}

/// Les champs montrés de la case, ceux qui ont un texte : (champ, texte), dans l'ordre de la fiche.
#[cfg(test)]
pub fn champs_de_la_case<'a>(doc: &'a Document, panel: usize, id: u64, frame: usize, textes: Textes) -> Vec<(Champ, &'a str)> {
    textes.champs().into_iter().map(|c| (c, texte_du_champ(doc, panel, id, frame, c))).filter(|(_, t)| !t.trim().is_empty()).collect()
}

/// Un texte mis sur une seule ligne : les retours à la ligne et les espaces qui se suivent deviennent un seul espace (la ligne de la case
/// est ensuite coupée par « … » à sa largeur).
pub fn une_ligne(texte: &str) -> String {
    texte.split_whitespace().collect::<Vec<_>>().join(" ")
}