//! La recherche du panneau Scénario (Ctrl+F dans le panneau ; Anthony, 2026-10-30).
//!
//! - **Sans casse ni accents** : « ete » trouve « Été ». Le texte est normalisé caractère pour caractère (une lettre
//!   en donne une), donc la place d'une occurrence dans le texte normalisé est aussi sa place dans le texte affiché.
//! - **Zéro latence** : le texte normalisé de tout le scénario est calculé une fois par scénario (`Index`), la liste
//!   des occurrences une fois par frappe (ou par changement de scénario), jamais à chaque image.
//! - On cherche dans ce qui est affiché dans le panneau (intitulés en majuscules, noms de personnages, répliques,
//!   parenthèses, notes), partie par partie : une occurrence ne traverse pas deux parties.
//! - Rien n'est enregistré dans le projet ; le dernier terme est gardé le temps de la séance.

use std::ops::Range;

use crate::scenariogarde::Gardee;
use crate::scenariopanneau::parties;

/// Une lettre sans accent ni majuscule (une lettre pour une lettre ; les retours à la ligne deviennent des espaces).
pub(crate) fn normaliser_char(c: char) -> char {
    let c = if c == '\n' || c == '\r' || c == '\t' || c == '\u{a0}' { ' ' } else { c };
    let m = c.to_lowercase().next().unwrap_or(c);
    match m {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
        'ñ' | 'ń' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
        'ý' | 'ÿ' => 'y',
        'š' | 'ś' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        '’' | '‘' => '\'',
        _ => m,
    }
}

/// Le texte sans casse ni accents (même nombre de lettres que `t`).
pub(crate) fn normaliser(t: &str) -> String {
    t.chars().map(normaliser_char).collect()
}

/// Une partie affichée d'un élément, normalisée.
struct PartieNorm {
    element: usize,
    partie: usize,
    lettres: Vec<char>,
}

/// Le texte normalisé de tout un scénario, pour un scénario donné.
struct Index {
    scenario: u64,
    parties: Vec<PartieNorm>,
}

impl Index {
    fn nouveau(g: &Gardee) -> Index {
        let mut out = Vec::new();
        for (i, e) in g.script.elements.iter().enumerate() {
            for (k, p) in parties(e).iter().enumerate() {
                out.push(PartieNorm { element: i, partie: k, lettres: p.texte.chars().map(normaliser_char).collect() });
            }
        }
        Index { scenario: g.id, parties: out }
    }
}

/// Une occurrence : la partie `partie` de l'élément `element`, `len` lettres à partir de la lettre `debut`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Occurrence {
    pub element: usize,
    pub partie: usize,
    pub debut: usize,
    pub len: usize,
}

/// L'état de la recherche (garde le terme, les occurrences et laquelle est la courante).
#[derive(Default)]
pub(crate) struct Recherche {
    pub ouverte: bool,
    /// Le champ de texte (le terme tel que tapé). Gardé quand on ferme : la prochaine ouverture le reprend.
    pub terme: String,
    pub courant: usize,
    /// Mettre le curseur dans le champ à cette image (et tout sélectionner).
    pub focus: bool,
    /// Faire défiler vers l'occurrence courante à cette image.
    pub a_defiler: bool,
    /// Image où le pointeur était sur le panneau (voir `App::recherche_du_panneau`).
    pub survol_image: Option<u64>,
    index: Option<Index>,
    resultats: Vec<Occurrence>,
    /// Pour chaque élément, la plage de `resultats` qui est à lui (un de plus que d'éléments).
    par_element: Vec<usize>,
    calcule_pour: Option<(u64, String)>,
}

impl Recherche {
    /// Ouvre la barre (le terme d'avant est repris et sélectionné).
    pub fn ouvrir(&mut self) {
        self.ouverte = true;
        self.focus = true;
        if !self.terme.is_empty() {
            self.a_defiler = true;
        }
    }

    /// Ferme la barre : plus de surlignage (le terme reste en mémoire pour la prochaine ouverture).
    pub fn fermer(&mut self) {
        self.ouverte = false;
        self.focus = false;
        self.a_defiler = false;
    }

    /// Les occurrences à jour pour ce scénario et le terme tapé. Ne refait le calcul que si l'un des deux a changé.
    pub fn mettre_a_jour(&mut self, g: &Gardee) {
        let terme: String = if self.ouverte { normaliser(self.terme.trim()) } else { String::new() };
        let cle = (g.id, terme);
        if self.calcule_pour.as_ref() == Some(&cle) {
            return;
        }
        let meme_terme = self.calcule_pour.as_ref().is_some_and(|(_, t)| *t == cle.1);
        self.resultats.clear();
        self.par_element.clear();
        let n = g.script.elements.len();
        if !cle.1.is_empty() {
            if self.index.as_ref().map(|i| i.scenario) != Some(g.id) {
                self.index = Some(Index::nouveau(g));
            }
            let aiguille: Vec<char> = cle.1.chars().collect();
            let index = self.index.as_ref().expect("index");
            for p in &index.parties {
                let mut i = 0;
                while i + aiguille.len() <= p.lettres.len() {
                    if p.lettres[i..i + aiguille.len()] == aiguille[..] {
                        self.resultats.push(Occurrence { element: p.element, partie: p.partie, debut: i, len: aiguille.len() });
                        i += aiguille.len();
                    } else {
                        i += 1;
                    }
                }
            }
        } else {
            self.index = self.index.take().filter(|i| i.scenario == g.id);
        }
        // Les plages par élément (les occurrences sont dans l'ordre du scénario).
        self.par_element = Vec::with_capacity(n + 1);
        let mut k = 0;
        for e in 0..=n {
            while k < self.resultats.len() && self.resultats[k].element < e {
                k += 1;
            }
            self.par_element.push(k);
        }
        // Un nouveau terme repart de la première occurrence ; un nouveau scénario garde le rang, ramené dans les limites.
        if !meme_terme || self.courant >= self.resultats.len() {
            self.courant = 0;
        }
        if !self.resultats.is_empty() && !meme_terme {
            self.a_defiler = true;
        }
        self.calcule_pour = Some(cle);
    }

    pub fn nombre(&self) -> usize {
        self.resultats.len()
    }

    /// Le terme tapé est vide (rien à chercher).
    pub fn sans_terme(&self) -> bool {
        self.terme.trim().is_empty()
    }

    /// « 3 / 12 », « Aucun résultat », ou rien quand aucun terme n'est tapé.
    pub fn compteur(&self) -> String {
        if !self.ouverte || self.sans_terme() {
            String::new()
        } else if self.nombre() == 0 {
            "Aucun résultat".to_string()
        } else {
            format!("{} / {}", self.courant + 1, self.nombre())
        }
    }

    /// Occurrence suivante (revient à la première après la dernière).
    pub fn suivant(&mut self) {
        if self.resultats.is_empty() {
            return;
        }
        self.courant = (self.courant + 1) % self.resultats.len();
        self.a_defiler = true;
    }

    /// Occurrence précédente (passe à la dernière avant la première).
    pub fn precedent(&mut self) {
        if self.resultats.is_empty() {
            return;
        }
        self.courant = (self.courant + self.resultats.len() - 1) % self.resultats.len();
        self.a_defiler = true;
    }

    pub fn courante(&self) -> Option<Occurrence> {
        self.resultats.get(self.courant).copied()
    }

    /// Les occurrences de l'élément `i`, avec leur rang (pour reconnaître la courante). Rien si la barre est fermée.
    pub fn de_l_element(&self, i: usize) -> Option<(Range<usize>, &[Occurrence])> {
        let (a, b) = (*self.par_element.get(i)?, *self.par_element.get(i + 1)?);
        (a < b).then(|| (a..b, &self.resultats[a..b]))
    }
}
