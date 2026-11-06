//! Le modèle de noms des plans (pipeline, étape 1, 2026-10-30) : `SQ01_PL012A`, ou ce que le studio veut.
//!
//! Un modèle est un texte avec des **jetons** entre accolades : `{projet}`, `{episode}`, `{sequence}`, `{plan}`, `{version}`, `{case}`,
//! `{date}`, `{calque}`. Un jeton peut porter une **largeur** (`{sequence:03}` : au moins trois chiffres, avec des zéros devant) ; le reste du
//! modèle (préfixes fixes, séparateurs) est écrit tel quel. `{{` et `}}` donnent une accolade.
//!
//! Le modèle est un réglage **du projet** (`Properties::modele_noms`, vide : le modèle d'origine `MODELE_DEFAUT`, qui donne les mêmes noms qu'avant :
//! « SQ01_PL012A »). Tous les exports qui nomment des fichiers par plan passent par `Document::nom_de_plan` (donc `editorial::clip_name`) :
//! vidéos par plan, images par plan, calques séparés, fiche technique, liste des plans, paquet de livraison.
//!
//! - `{version}` : le numéro de la version du plan qui part (1 pour un plan à une seule version). Un modèle **sans** `{version}` garde la règle d'avant :
//!   un plan qui a plusieurs versions ajoute « _V2 » à la fin de son nom.
//! - `{case}` : le rang de la case dans le plan (images par case) ; vide pour un export par plan. Un modèle sans `{case}` ajoute « _C02 » aux cases
//!   d'après la première (règle d'avant).
//! - `{calque}` : le nom du calque (calques séparés) ; vide ailleurs.
//! - `{date}` : AAAAMMJJ, le jour de l'export.
//! - `{projet}` : le nom de la série ou du projet de production (`Properties::serie`), sinon le titre du projet.
//! - `{episode}` : le numéro de l'épisode (`Properties::episode`) ; avec une largeur, il est complété de zéros s'il est un nombre.
//!
//! Le texte produit n'a jamais de caractère que Windows refuse dans un nom (`< > : " / \ | ? *`, ni de caractère de contrôle) : chaque valeur est
//! nettoyée, et la fenêtre de réglage avertit si le modèle lui-même en contient.

use std::collections::BTreeMap;

use crate::document::Document;
use crate::numerotation::Num;

/// Le modèle d'origine : les noms de toujours (« SQ01_PL012A »).
pub const MODELE_DEFAUT: &str = "SQ{sequence:02}_PL{plan:03}";

/// Les modèles prêts (Préférences et fenêtre « Noms et production ») : nom, modèle, ce que ça donne.
pub const MODELES_PRETS: &[(&str, &str)] = &[
    ("FEANOR (d'origine)", MODELE_DEFAUT),
    ("Studio : épisode, séquence, plan, version", "{projet}_EP{episode:02}_SQ{sequence:03}_SH{plan:04}_v{version:03}"),
    ("Studio sans version", "{projet}_EP{episode:02}_SQ{sequence:03}_SH{plan:04}"),
    ("Séquence et plan", "{sequence:02}_{plan:03}"),
    ("Projet, séquence, plan", "{projet}_{sequence:02}_{plan:03}"),
    ("Daté", "{date}_SQ{sequence:02}_PL{plan:03}"),
];

/// Un jeton du modèle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Jeton {
    Projet,
    Episode,
    Sequence,
    Plan,
    Version,
    Case,
    Date,
    Calque,
    /// Le rang de la clé dans le plan : seulement dans la nomenclature des exports à la clé (`nomenclature.rs`), pas dans le modèle du projet.
    Cle,
}

impl Jeton {
    pub const TOUS: [Jeton; 8] = [Jeton::Projet, Jeton::Episode, Jeton::Sequence, Jeton::Plan, Jeton::Version, Jeton::Case, Jeton::Date, Jeton::Calque];
    /// Les jetons permis dans une nomenclature d'export : ceux du modèle du projet et la clé.
    pub const TOUS_EXPORT: [Jeton; 9] = [Jeton::Projet, Jeton::Episode, Jeton::Sequence, Jeton::Plan, Jeton::Version, Jeton::Case, Jeton::Date, Jeton::Calque, Jeton::Cle];

    pub fn nom(self) -> &'static str {
        match self {
            Jeton::Projet => "projet",
            Jeton::Episode => "episode",
            Jeton::Sequence => "sequence",
            Jeton::Plan => "plan",
            Jeton::Version => "version",
            Jeton::Case => "case",
            Jeton::Date => "date",
            Jeton::Calque => "calque",
            Jeton::Cle => "cle",
        }
    }

    /// Ce que le jeton met dans le nom (pour la fenêtre).
    pub fn aide(self) -> &'static str {
        match self {
            Jeton::Projet => "Le nom de la série ou du projet de production (à défaut, le titre du projet).",
            Jeton::Episode => "Le numéro de l'épisode ; {episode:02} le complète de zéros.",
            Jeton::Sequence => "Le numéro de la séquence ; {sequence:03} le complète de zéros (001).",
            Jeton::Plan => "Le numéro du plan (12, 12A…) ; {plan:04} le complète de zéros (0012A).",
            Jeton::Version => "Le numéro de la version du plan (1 s'il n'y en a qu'une). Sans ce jeton, une version d'après la première ajoute « _V2 » au nom.",
            Jeton::Case => "Le rang de la case dans le plan (images par case). Vide pour un export par plan.",
            Jeton::Date => "Le jour de l'export, AAAAMMJJ.",
            Jeton::Calque => "Le nom du calque (calques séparés). Vide ailleurs.",
            Jeton::Cle => "Le rang de la clé dans le plan (images à la clé) ; {cle:03} le complète de zéros. Vide ailleurs.",
        }
    }

    fn depuis(nom: &str, permis: &[Jeton]) -> Option<Jeton> {
        permis.iter().copied().find(|j| j.nom() == nom)
    }
}

/// Un morceau d'un modèle lu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Texte(String),
    Jeton { jeton: Jeton, largeur: usize },
}

/// Le modèle lu. Erreur : ce qui ne va pas, en une phrase.
pub fn analyser(modele: &str) -> Result<Vec<Piece>, String> {
    analyser_avec(modele, &Jeton::TOUS)
}

/// Comme `analyser`, avec les jetons `permis` (la nomenclature des exports à la clé permet aussi `{cle}`).
pub fn analyser_avec(modele: &str, permis: &[Jeton]) -> Result<Vec<Piece>, String> {
    let mut pieces: Vec<Piece> = Vec::new();
    let mut texte = String::new();
    let mut it = modele.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                texte.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                texte.push('}');
            }
            '{' => {
                let mut dedans = String::new();
                let mut ferme = false;
                for d in it.by_ref() {
                    if d == '}' {
                        ferme = true;
                        break;
                    }
                    dedans.push(d);
                }
                if !ferme {
                    return Err("une accolade « { » n'est jamais refermée".into());
                }
                let (nom, largeur) = match dedans.split_once(':') {
                    Some((n, l)) => {
                        let l = l.trim();
                        if l.is_empty() || !l.bytes().all(|b| b.is_ascii_digit()) || l.len() > 2 {
                            return Err(format!("largeur illisible dans « {{{dedans}}} » (un nombre de 1 à 99, comme {{plan:03}})"));
                        }
                        (n.trim(), l.parse::<usize>().unwrap_or(0))
                    }
                    None => (dedans.trim(), 0),
                };
                let Some(jeton) = Jeton::depuis(&nom.to_lowercase(), permis) else {
                    return Err(format!("jeton inconnu « {{{nom}}} » (permis : {})", permis.iter().map(|j| format!("{{{}}}", j.nom())).collect::<Vec<_>>().join(", ")));
                };
                if !texte.is_empty() {
                    pieces.push(Piece::Texte(std::mem::take(&mut texte)));
                }
                pieces.push(Piece::Jeton { jeton, largeur });
            }
            '}' => return Err("une accolade « } » seule (pour l'écrire, double-la : }})".into()),
            c => texte.push(c),
        }
    }
    if !texte.is_empty() {
        pieces.push(Piece::Texte(texte));
    }
    if pieces.is_empty() {
        return Err("le modèle est vide".into());
    }
    Ok(pieces)
}

/// Le modèle contient-il ce jeton ?
pub fn contient(pieces: &[Piece], jeton: Jeton) -> bool {
    pieces.iter().any(|p| matches!(p, Piece::Jeton { jeton: j, .. } if *j == jeton))
}

/// Les caractères que Windows refuse dans un nom de fichier.
pub fn caractere_interdit(c: char) -> bool {
    matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control()
}

/// Une valeur sans caractère interdit.
fn nettoyer(s: &str) -> String {
    s.trim().chars().map(|c| if caractere_interdit(c) { '_' } else { c }).collect()
}

/// Ce que le modèle a besoin de savoir d'un plan.
#[derive(Clone, Debug, Default)]
pub struct Contexte {
    pub projet: String,
    pub episode: String,
    pub sequence: String,
    pub sequence_num: Option<Num>,
    pub plan_num: Option<Num>,
    pub version: u32,
    pub case: Option<usize>,
    /// Le rang de la clé dans le plan (images à la clé, `exportcles.rs`) ; seulement pour les modèles des exports à la clé.
    pub cle: Option<usize>,
    pub calque: Option<String>,
    pub date: String,
}

fn numero(n: &Option<Num>, largeur: usize) -> String {
    n.as_ref().map(|n| n.padded(largeur)).unwrap_or_default()
}

/// Le nom : le modèle `pieces` rempli avec `cx`.
pub fn remplir(pieces: &[Piece], cx: &Contexte) -> String {
    let mut out = String::new();
    for p in pieces {
        match p {
            Piece::Texte(t) => out.push_str(t),
            Piece::Jeton { jeton, largeur } => {
                let w = *largeur;
                let v = match jeton {
                    Jeton::Projet => nettoyer(&cx.projet),
                    Jeton::Episode => {
                        let e = cx.episode.trim();
                        match e.parse::<u64>() {
                            Ok(n) => format!("{n:0w$}"),
                            Err(_) => nettoyer(e),
                        }
                    }
                    Jeton::Sequence => numero(&cx.sequence_num, w),
                    Jeton::Plan => numero(&cx.plan_num, w),
                    Jeton::Version => format!("{:0w$}", cx.version.max(1)),
                    Jeton::Case => cx.case.map(|k| format!("{k:0w$}")).unwrap_or_default(),
                    Jeton::Cle => cx.cle.map(|k| format!("{k:0w$}")).unwrap_or_default(),
                    Jeton::Date => cx.date.clone(),
                    Jeton::Calque => nettoyer(cx.calque.as_deref().unwrap_or("")),
                };
                out.push_str(&v);
            }
        }
    }
    // (Windows n'aime ni un point ni une espace à la fin d'un nom.)
    out.trim_end_matches(['.', ' ']).to_string()
}

/// AAAAMMJJ du jour (UTC).
pub fn date_du_jour() -> String {
    crate::incrustation::date_text(crate::incrustation::now_secs()).replace('-', "")
}

impl Document {
    /// Le modèle de noms du projet (d'origine si le projet n'en a pas, ou si le sien ne se lit pas).
    pub fn modele_de_noms(&self) -> Vec<Piece> {
        let m = self.properties.modele_noms.trim();
        if !m.is_empty() {
            if let Ok(p) = analyser(m) {
                return p;
            }
        }
        analyser(MODELE_DEFAUT).expect("modèle d'origine")
    }

    /// Le contexte du plan `p` (rang du panneau) pour le modèle de noms.
    pub fn contexte_de_nom(&self, p: usize) -> Contexte {
        let id = self.panels[p].id;
        let projet = match self.properties.serie.trim() {
            "" => self.properties.title.trim().to_string(),
            s => s.to_string(),
        };
        Contexte {
            projet,
            episode: self.properties.episode.clone(),
            sequence: String::new(),
            sequence_num: Some(self.sequence_num(self.sequence_of(p))),
            plan_num: Some(self.plan_num(self.panels[p].scene)),
            version: self.numero_de_version(id),
            case: None,
            cle: None,
            calque: None,
            date: date_du_jour(),
        }
    }

    /// Le nom du plan `p` pour les fichiers : le modèle du projet, avec sa version (règle d'avant : « _V2 » si le modèle n'a pas `{version}` et que le
    /// plan a plusieurs versions). `case` : le rang de la case (images par case), `calque` : le nom du calque.
    pub fn nom_de_plan_avec(&self, p: usize, case: Option<usize>, calque: Option<&str>) -> String {
        self.nom_avec_modele(&self.modele_de_noms(), p, case, calque)
    }

    /// Comme `nom_de_plan_avec`, avec un autre modèle que celui du projet (la fenêtre de réglage l'essaie avant de le garder).
    pub fn nom_avec_modele(&self, pieces: &[Piece], p: usize, case: Option<usize>, calque: Option<&str>) -> String {
        let mut cx = self.contexte_de_nom(p);
        cx.case = case;
        cx.calque = calque.map(str::to_string);
        self.nom_avec_contexte(pieces, p, cx)
    }

    /// Les noms de tous les plans du film (`nom_de_plan(p)` pour chaque `p`), trouvés en une passe : `nom_de_plan` refait l'ordre du film à chaque appel
    /// (le carré du nombre de plans), ce qui se sent quand on nomme tout le film d'un coup (envoi, rapport de numérotation).
    pub fn noms_de_tous_les_plans(&self) -> Vec<String> {
        let pieces = self.modele_de_noms();
        let scene_rang: std::collections::HashMap<u64, usize> = self.scene_order().into_iter().enumerate().map(|(k, s)| (s, k + 1)).collect();
        let seq_rang: std::collections::HashMap<u64, usize> = self.sequence_order().into_iter().enumerate().map(|(k, s)| (s, k + 1)).collect();
        let mut scenes: std::collections::HashMap<u64, &crate::document::Scene> = std::collections::HashMap::new();
        for s in &self.scenes {
            scenes.entry(s.id).or_insert(s);
        }
        let mut sequences: std::collections::HashMap<u64, &crate::document::Sequence> = std::collections::HashMap::new();
        for s in &self.sequences {
            sequences.entry(s.id).or_insert(s);
        }
        let ids = self.sequence_ids();
        let projet = match self.properties.serie.trim() {
            "" => self.properties.title.trim().to_string(),
            s => s.to_string(),
        };
        let date = date_du_jour();
        (0..self.panels.len())
            .map(|p| {
                let scene = self.panels[p].scene;
                let plan_num = scenes.get(&scene).filter(|_| self.numbering_locked).and_then(|s| Num::parse(&s.number)).unwrap_or_else(|| Num::new(scene_rang.get(&scene).copied().unwrap_or(0) as i64));
                let sequence_num = sequences.get(&ids[p]).filter(|_| self.numbering_locked).and_then(|s| Num::parse(&s.number)).unwrap_or_else(|| Num::new(seq_rang.get(&ids[p]).copied().unwrap_or(0) as i64));
                let cx = Contexte {
                    projet: projet.clone(),
                    episode: self.properties.episode.clone(),
                    sequence: String::new(),
                    sequence_num: Some(sequence_num),
                    plan_num: Some(plan_num),
                    version: self.numero_de_version(self.panels[p].id),
                    case: None,
                    cle: None,
                    calque: None,
                    date: date.clone(),
                };
                self.nom_avec_contexte(&pieces, p, cx)
            })
            .collect()
    }

    pub(crate) fn nom_avec_contexte(&self, pieces: &[Piece], p: usize, cx: Contexte) -> String {
        let (case, mut nom) = (cx.case, remplir(pieces, &cx));
        if !contient(pieces, Jeton::Version) && self.nombre_de_versions(self.panels[p].id) > 1 {
            nom = format!("{nom}_{}", crate::planvers::etiquette(cx.version));
        }
        if let Some(k) = case.filter(|k| *k > 1 && !contient(pieces, Jeton::Case)) {
            nom = format!("{nom}_C{k:02}");
        }
        nom
    }

    /// Le nom du plan `p` (sans case ni calque).
    pub fn nom_de_plan(&self, p: usize) -> String {
        self.nom_de_plan_avec(p, None, None)
    }
}

/// Ce que la fenêtre de réglage dit d'un modèle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Verification {
    /// Le modèle ne se lit pas : rien ne part avec lui.
    pub erreur: Option<String>,
    /// Ce qu'il faut savoir (doublons, caractère interdit…).
    pub avertissements: Vec<String>,
    /// Le nom du premier plan, et du dernier.
    pub exemples: Vec<String>,
}

/// Les noms de tous les plans avec le modèle `modele` (vide : d'origine), et ce qui cloche. Les doublons sont comptés sur les noms de fichiers
/// (sans tenir compte des majuscules : Windows n'en fait pas la différence).
pub fn verifier(doc: &Document, modele: &str) -> Verification {
    let mut v = Verification::default();
    let texte = if modele.trim().is_empty() { MODELE_DEFAUT } else { modele.trim() };
    let pieces = match analyser(texte) {
        Ok(p) => p,
        Err(e) => {
            v.erreur = Some(e);
            return v;
        }
    };
    let interdits: Vec<char> = pieces
        .iter()
        .filter_map(|p| if let Piece::Texte(t) = p { Some(t.chars().filter(|c| caractere_interdit(*c)).collect::<Vec<_>>()) } else { None })
        .flatten()
        .collect();
    if !interdits.is_empty() {
        let mut vus: Vec<char> = Vec::new();
        for c in interdits {
            if !vus.contains(&c) {
                vus.push(c);
            }
        }
        v.avertissements.push(format!("Windows refuse « {} » dans un nom de fichier : il sera remplacé par « _ ».", vus.iter().map(char::to_string).collect::<Vec<_>>().join(" ")));
    }
    let noms: Vec<String> = (0..doc.panels.len()).map(|p| doc.nom_avec_modele(&pieces, p, None, None)).collect();
    let mut par_nom: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (p, n) in noms.iter().enumerate() {
        par_nom.entry(crate::editorial::file_safe(n).to_lowercase()).or_default().push(p);
    }
    let doubles: Vec<(&String, &Vec<usize>)> = par_nom.iter().filter(|(_, l)| l.len() > 1).collect();
    if !doubles.is_empty() {
        let n: usize = doubles.iter().map(|(_, l)| l.len()).sum();
        let (nom, l) = doubles[0];
        v.avertissements.push(format!(
            "{n} plans auraient un nom déjà pris par un autre plan (par exemple « {} » pour les plans {}) : ajoute {{sequence}} ou {{version}} au modèle.",
            noms[l[0]],
            l.iter().take(4).map(|p| (p + 1).to_string()).collect::<Vec<_>>().join(", ")
        ));
        let _ = nom;
    }
    if noms.iter().any(|n| n.chars().count() > 100) {
        v.avertissements.push("Un nom dépasse 100 caractères : certains logiciels de production le refusent.".into());
    }
    for n in &noms {
        let base = n.split('.').next().unwrap_or(n).to_uppercase();
        if ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str()) || (base.len() == 4 && (base.starts_with("COM") || base.starts_with("LPT")) && base.as_bytes()[3].is_ascii_digit()) {
            v.avertissements.push(format!("« {n} » est un nom réservé par Windows."));
            break;
        }
    }
    if let (Some(a), Some(b)) = (noms.first(), noms.last()) {
        v.exemples.push(a.clone());
        if noms.len() > 1 {
            v.exemples.push(b.clone());
        }
    }
    v
}
