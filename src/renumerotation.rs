//! Le rapport de numérotation pour le monteur (Anthony, 2026-10-30, audit solution 7 : « le contrat de numérotation ») : ce qui a changé dans les
//! plans depuis le dernier envoi — plans **ajoutés**, **supprimés**, **déplacés**, **renumérotés** — avec le tableau « ancien numéro → nouveau
//! numéro ». Le monteur sait ainsi quels clips de sa ligne de temps remplacer, déplacer ou renommer, au lieu de tout comparer à l'œil.
//!
//! # D'où vient la comparaison
//!
//! De l'état gardé par l'envoi de référence (`envois.rs` : le dernier, ou celui qu'on choisit) : pour chaque plan, son numéro de plan (le titre « Plan 12A »),
//! et, pour les envois faits depuis ce rapport, son **nom** selon le modèle de noms (`SQ01_PL012A`) et sa **séquence**. Les plans se reconnaissent par leur
//! identité interne (`Panel::id`), qui survit aux déplacements et aux renumérotations ; un plan copié par une variante de séquence se compare à celui dont
//! il vient (`Document::plan_dans_envoi`).
//!
//! # Ce que veut dire chaque mot
//!
//! - **Ajouté** : le plan n'était pas dans l'envoi. **Supprimé** : il y était et n'est plus dans le film.
//! - **Renuméroté** : son numéro de plan a changé (12 → 12A, 12 → 15), ou, quand l'envoi a gardé la séquence, sa séquence a changé de nom ou de numéro.
//!   Le nom complet du plan (`nouveau_nom`) est écrit à côté ; un changement du modèle de noms, ou d'une version de plan, n'est pas une renumérotation.
//! - **Déplacé** : le plan a changé de place par rapport aux autres plans qui n'ont pas bougé. Parmi les plans présents des deux côtés, on cherche le plus
//!   grand ensemble qui garde son ordre ; ceux qui n'en font pas partie sont les déplacés. Un plan ajouté ou supprimé n'en déplace aucun autre, et le
//!   déplacement d'un plan loin de sa place ne fait pas dire que tous les suivants ont bougé.
//!
//! # Les fichiers
//!
//! - `…_renumerotation.csv` : un tableau pour un tableur (UTF-8 avec marque d'ordre, séparateur `;`, une ligne par plan concerné) ;
//! - `…_renumerotation.txt` : une page lisible, par groupes (ajoutés, supprimés, déplacés, renumérotés), à envoyer telle quelle au monteur.
//!
//! Les deux sont écrits dans le dossier de l'export pour le montage (case « Rapport de numérotation »), ou à part depuis le hub Exporter.
//! Un envoi d'avant ce rapport n'a pas gardé le nom ni la séquence d'alors : seul le numéro est comparé, et le rapport le dit.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::envois::Envoi;
use crate::listeplans::cellule_csv;

/// Un plan concerné par un changement.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ligne {
    pub ajoute: bool,
    pub supprime: bool,
    pub deplace: bool,
    pub renumerote: bool,
    pub ancien_numero: String,
    pub nouveau_numero: String,
    pub ancien_nom: String,
    pub nouveau_nom: String,
    pub ancienne_sequence: String,
    pub nouvelle_sequence: String,
    /// Place dans le film (1, 2, 3…) ; rien quand le plan n'y était pas, ou n'y est plus.
    pub ancien_rang: Option<usize>,
    pub nouveau_rang: Option<usize>,
    pub images_avant: Option<usize>,
    pub images_apres: Option<usize>,
}

impl Ligne {
    /// « Ajouté », « Supprimé », « Déplacé », « Renuméroté » ou « Déplacé et renuméroté ».
    pub fn statut(&self) -> &'static str {
        match (self.ajoute, self.supprime, self.deplace, self.renumerote) {
            (true, ..) => "Ajouté",
            (_, true, ..) => "Supprimé",
            (_, _, true, true) => "Déplacé et renuméroté",
            (_, _, true, false) => "Déplacé",
            _ => "Renuméroté",
        }
    }
}

/// Ce qui a changé depuis un envoi.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rapport {
    pub envoi_id: u64,
    pub envoi_nom: String,
    pub envoi_date: u64,
    /// Les plans concernés, dans l'ordre du film d'aujourd'hui ; les supprimés à la suite, dans l'ordre de l'envoi.
    pub lignes: Vec<Ligne>,
    pub plans_avant: usize,
    pub plans_apres: usize,
    /// Plans présents des deux côtés et sans changement de numéro, de place ni de séquence.
    pub inchanges: usize,
    /// L'envoi n'a pas gardé le nom ni la séquence d'alors (envoi d'avant le rapport) : seul le numéro de plan est comparé.
    pub noms_anciens_inconnus: bool,
}

impl Rapport {
    pub fn ajoutes(&self) -> usize {
        self.lignes.iter().filter(|l| l.ajoute).count()
    }
    pub fn supprimes(&self) -> usize {
        self.lignes.iter().filter(|l| l.supprime).count()
    }
    pub fn deplaces(&self) -> usize {
        self.lignes.iter().filter(|l| l.deplace).count()
    }
    pub fn renumerotes(&self) -> usize {
        self.lignes.iter().filter(|l| l.renumerote).count()
    }

    /// Rien n'a changé depuis l'envoi.
    pub fn est_vide(&self) -> bool {
        self.lignes.is_empty()
    }
}

/// Les indices (dans `suite`) qui ne font pas partie d'une plus longue sous-suite strictement croissante : les éléments qui ont changé de place.
/// `suite[k]` est l'ancien rang du plan qui est aujourd'hui `k`-ième parmi les plans communs.
pub(crate) fn deplaces(suite: &[usize]) -> Vec<usize> {
    let n = suite.len();
    if n == 0 {
        return Vec::new();
    }
    // Plus longue sous-suite croissante : « patience », avec les liens pour la reconstruire.
    let mut queues: Vec<usize> = Vec::new(); // indices : queues[l] = l'indice de la plus petite fin d'une sous-suite de longueur l + 1
    let mut avant: Vec<Option<usize>> = vec![None; n];
    for k in 0..n {
        let pos = queues.partition_point(|&q| suite[q] < suite[k]);
        if pos > 0 {
            avant[k] = Some(queues[pos - 1]);
        }
        if pos == queues.len() {
            queues.push(k);
        } else {
            queues[pos] = k;
        }
    }
    let mut garde = vec![false; n];
    let mut cur = queues.last().copied();
    while let Some(k) = cur {
        garde[k] = true;
        cur = avant[k];
    }
    (0..n).filter(|&k| !garde[k]).collect()
}

/// « Plan 12A » → « 12A » (le titre qu'un envoi garde), sinon le texte tel quel.
fn numero_de(titre: &str) -> String {
    titre.trim().strip_prefix("Plan ").unwrap_or(titre.trim()).trim().to_string()
}

/// Ce qui a changé dans `doc` depuis `envoi`.
pub fn construire(doc: &Document, envoi: &Envoi) -> Rapport {
    let noms = doc.noms_de_tous_les_plans();
    let numeros = crate::numerotation::Numeros::of(doc);
    let sequences = doc.sequence_ids();
    // Chaque plan d'aujourd'hui, avec le plan de l'envoi auquel il répond (rang dans l'envoi), une seule fois chacun.
    let rang_envoi: HashMap<u64, usize> = envoi.plans.iter().enumerate().map(|(k, p)| (p.plan, k)).collect();
    let mut pris: Vec<bool> = vec![false; envoi.plans.len()];
    let mut reponses: Vec<Option<usize>> = Vec::with_capacity(doc.panels.len());
    for p in &doc.panels {
        let k = doc.plan_dans_envoi(envoi, p.id).and_then(|connu| rang_envoi.get(&connu).copied()).filter(|k| !pris[*k]);
        if let Some(k) = k {
            pris[k] = true;
        }
        reponses.push(k);
    }
    // Les plans communs, dans l'ordre d'aujourd'hui, avec leur ancien rang : ceux hors de la plus longue suite qui garde l'ordre sont déplacés.
    let communs: Vec<usize> = (0..doc.panels.len()).filter(|&i| reponses[i].is_some()).collect();
    let suite: Vec<usize> = communs.iter().map(|&i| reponses[i].unwrap_or(0)).collect();
    let deplaces_parmi: std::collections::HashSet<usize> = deplaces(&suite).into_iter().map(|k| communs[k]).collect();

    let noms_anciens_inconnus = envoi.plans.iter().any(|p| p.nom.is_empty());
    let mut lignes: Vec<Ligne> = Vec::new();
    let mut inchanges = 0;
    for (i, p) in doc.panels.iter().enumerate() {
        let nouveau_numero = doc.scene_label(p.scene);
        let nouvelle_sequence = numeros.sequence_label(doc, sequences[i]);
        let Some(k) = reponses[i] else {
            lignes.push(Ligne {
                ajoute: true,
                nouveau_numero,
                nouveau_nom: noms[i].clone(),
                nouvelle_sequence,
                nouveau_rang: Some(i + 1),
                images_apres: Some(p.frame_count()),
                ..Default::default()
            });
            continue;
        };
        let ancien = &envoi.plans[k];
        let ancien_numero = numero_de(&ancien.titre);
        let sequence_changee = !ancien.sequence.is_empty() && ancien.sequence != nouvelle_sequence;
        let renumerote = ancien_numero != nouveau_numero || sequence_changee;
        let deplace = deplaces_parmi.contains(&i);
        if !renumerote && !deplace {
            inchanges += 1;
            continue;
        }
        lignes.push(Ligne {
            deplace,
            renumerote,
            ancien_numero,
            nouveau_numero,
            ancien_nom: ancien.nom.clone(),
            nouveau_nom: noms[i].clone(),
            ancienne_sequence: ancien.sequence.clone(),
            nouvelle_sequence,
            ancien_rang: Some(k + 1),
            nouveau_rang: Some(i + 1),
            images_avant: Some(ancien.images),
            images_apres: Some(p.frame_count()),
            ..Default::default()
        });
    }
    for (k, ancien) in envoi.plans.iter().enumerate().filter(|(k, _)| !pris[*k]) {
        lignes.push(Ligne {
            supprime: true,
            ancien_numero: numero_de(&ancien.titre),
            ancien_nom: ancien.nom.clone(),
            ancienne_sequence: ancien.sequence.clone(),
            ancien_rang: Some(k + 1),
            images_avant: Some(ancien.images),
            ..Default::default()
        });
    }
    Rapport { envoi_id: envoi.id, envoi_nom: envoi.nom.clone(), envoi_date: envoi.date, lignes, plans_avant: envoi.plans.len(), plans_apres: doc.panels.len(), inchanges, noms_anciens_inconnus }
}

/// Le rapport depuis le dernier envoi du projet ; l'erreur dit quoi faire quand il n'y en a pas.
pub fn depuis_le_dernier_envoi(doc: &Document) -> Result<Rapport, String> {
    let Some(envoi) = doc.envois.dernier() else {
        return Err("Le projet n'a aucun envoi : « Marquer un envoi… » d'abord, pour que le rapport sache de quel état partir.".into());
    };
    Ok(construire(doc, envoi))
}

// ------------------------------------------------------------------------------------------------ les fichiers

const EN_TETES: [&str; 11] = [
    "statut",
    "ancien_numero",
    "nouveau_numero",
    "ancien_nom",
    "nouveau_nom",
    "ancienne_sequence",
    "nouvelle_sequence",
    "ancien_rang",
    "nouveau_rang",
    "images_avant",
    "images_apres",
];

fn rang(r: Option<usize>) -> String {
    r.map(|n| n.to_string()).unwrap_or_default()
}

/// Le CSV : UTF-8 avec marque d'ordre (un tableur français l'ouvre sans abîmer les accents), séparateur `;`, lignes terminées par CRLF.
pub fn csv(r: &Rapport) -> String {
    let mut out = String::from('\u{feff}');
    out.push_str(&EN_TETES.join(";"));
    out.push_str("\r\n");
    for l in &r.lignes {
        let cases = [
            l.statut().to_string(),
            l.ancien_numero.clone(),
            l.nouveau_numero.clone(),
            l.ancien_nom.clone(),
            l.nouveau_nom.clone(),
            l.ancienne_sequence.clone(),
            l.nouvelle_sequence.clone(),
            rang(l.ancien_rang),
            rang(l.nouveau_rang),
            rang(l.images_avant),
            rang(l.images_apres),
        ];
        out.push_str(&cases.iter().map(|c| cellule_csv(c, ';')).collect::<Vec<_>>().join(";"));
        out.push_str("\r\n");
    }
    out
}

fn nom_ou_numero(nom: &str, numero: &str) -> String {
    if nom.is_empty() {
        format!("plan {numero}")
    } else {
        format!("{nom} (plan {numero})")
    }
}

/// La page lisible. `aujourd_hui` : la date du jour, « 2026-10-30 ».
pub fn texte(r: &Rapport, doc: &Document, aujourd_hui: &str) -> String {
    use std::fmt::Write;
    let projet = match doc.properties.serie.trim() {
        "" => doc.properties.title.trim(),
        s => s,
    };
    let mut t = String::new();
    let _ = writeln!(t, "Rapport de numérotation{}", if projet.is_empty() { String::new() } else { format!(" — {projet}") });
    if !doc.properties.episode.trim().is_empty() {
        let _ = writeln!(t, "Épisode {}", doc.properties.episode.trim());
    }
    let _ = writeln!(t, "Depuis l'envoi « {} » du {} ; état d'aujourd'hui : {aujourd_hui}.", r.envoi_nom, crate::envois::date_courte(r.envoi_date));
    let _ = writeln!(t, "Plans : {} → {}.", r.plans_avant, r.plans_apres);
    let _ = writeln!(
        t,
        "Ajoutés : {} · Supprimés : {} · Déplacés : {} · Renumérotés : {} · Sans changement : {}.",
        r.ajoutes(),
        r.supprimes(),
        r.deplaces(),
        r.renumerotes(),
        r.inchanges
    );
    if r.noms_anciens_inconnus {
        let _ = writeln!(t, "(Cet envoi n'a pas gardé le nom ni la séquence d'alors des plans : seul le numéro de plan est comparé.)");
    }
    if r.est_vide() {
        let _ = writeln!(t, "\nRien n'a changé : les numéros, les places et la liste des plans sont ceux de l'envoi.");
        return t;
    }
    let groupe = |t: &mut String, titre: &str, lignes: Vec<String>| {
        if lignes.is_empty() {
            return;
        }
        let _ = writeln!(t, "\n{titre} ({})", lignes.len());
        for l in lignes {
            let _ = writeln!(t, "  {l}");
        }
    };
    groupe(
        &mut t,
        "AJOUTÉS",
        r.lignes.iter().filter(|l| l.ajoute).map(|l| format!("{}, séquence {}, place {}", nom_ou_numero(&l.nouveau_nom, &l.nouveau_numero), l.nouvelle_sequence, rang(l.nouveau_rang))).collect(),
    );
    groupe(
        &mut t,
        "SUPPRIMÉS",
        r.lignes
            .iter()
            .filter(|l| l.supprime)
            .map(|l| {
                let sequence = if l.ancienne_sequence.is_empty() { String::new() } else { format!(", séquence {}", l.ancienne_sequence) };
                format!("{}{sequence}, place {}", nom_ou_numero(&l.ancien_nom, &l.ancien_numero), rang(l.ancien_rang))
            })
            .collect(),
    );
    groupe(
        &mut t,
        "DÉPLACÉS",
        r.lignes.iter().filter(|l| l.deplace).map(|l| format!("{} : place {} → {}", nom_ou_numero(&l.nouveau_nom, &l.nouveau_numero), rang(l.ancien_rang), rang(l.nouveau_rang))).collect(),
    );
    groupe(
        &mut t,
        "RENUMÉROTÉS (ancien → nouveau)",
        r.lignes
            .iter()
            .filter(|l| l.renumerote)
            .map(|l| {
                let noms = if l.ancien_nom.is_empty() || l.ancien_nom == l.nouveau_nom { String::new() } else { format!("   [{} → {}]", l.ancien_nom, l.nouveau_nom) };
                let sequence = if !l.ancienne_sequence.is_empty() && l.ancienne_sequence != l.nouvelle_sequence { format!(", séquence {} → {}", l.ancienne_sequence, l.nouvelle_sequence) } else { String::new() };
                format!("{} → {}{sequence}{noms}", l.ancien_numero, l.nouveau_numero)
            })
            .collect(),
    );
    t
}

/// Les deux fichiers du rapport à côté de `base` (un chemin sans extension) : `base_renumerotation.csv` et `.txt`.
pub fn chemins(dossier: &Path, base: &str) -> (PathBuf, PathBuf) {
    let b = crate::editorial::file_safe(base);
    (dossier.join(format!("{b}_renumerotation.csv")), dossier.join(format!("{b}_renumerotation.txt")))
}

/// Écrit le CSV dans `csv` et la page lisible au même nom en `.txt` ; rend les deux chemins.
pub fn ecrire_vers(r: &Rapport, doc: &Document, csv_path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let txt = csv_path.with_extension("txt");
    let jour = crate::incrustation::date_text(crate::incrustation::now_secs());
    crate::export::write_atomic(csv_path, csv(r).as_bytes())?;
    crate::export::write_atomic(&txt, texte(r, doc, &jour).as_bytes())?;
    Ok((csv_path.to_path_buf(), txt))
}

/// Écrit le rapport dans `dossier`, sous `base_renumerotation.csv` et `.txt`.
pub fn ecrire(r: &Rapport, doc: &Document, dossier: &Path, base: &str) -> Result<(PathBuf, PathBuf), String> {
    ecrire_vers(r, doc, &chemins(dossier, base).0)
}
