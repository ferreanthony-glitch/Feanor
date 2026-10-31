//! Journal des différences entre deux envois (`docs/INTERFACE.md` §92) : « qu'est-ce qui a changé entre l'envoi 3 et l'envoi 4 ? ».
//!
//! Ce fichier est **le modèle et les deux formats** (la page HTML et le PDF), sans interface et sans fil : il ne touche ni au projet
//! ni au disque. La comparaison (`comparer`) ne regarde que des listes de plans (`PlanVu`) : l'empreinte de chaque plan, son titre, sa
//! version et son nombre d'images, tels que l'envoi les a gardés (`envois::PlanEnvoye`) ou tels qu'ils sont aujourd'hui. Les images
//! « avant / après » se calculent sur un autre fil (`journaljob.rs`), l'interface est dans `journalui.rs`.
//!
//! # Ce que dit le journal
//!
//! - **Modifiés** : les plans dont l'empreinte n'est plus la même, avec les deux images côte à côte, la part de pixels qui diffèrent (le
//!   même calcul que la fenêtre « Comparer »), la durée avant / après et l'écart, la version (V1 → V2) et le plan s'il a aussi changé de place ;
//! - **Ajoutés** et **retirés** : les plans qui n'étaient que d'un côté ;
//! - **Déplacés** : les plans qui ne sont plus au même endroit du film (la plus longue suite de plans restée dans l'ordre sert de repère,
//!   les autres sont dits déplacés : un plan qui en saute dix ne fait pas déplacer les dix) ;
//! - **Notes traitées entre les deux** (fil de décision, `fildecision.rs`) : celles dont le statut est « Fait », « Refusé » (avec sa raison)
//!   ou « En discussion », datées après le premier envoi et au plus tard au second, pour les plans que le journal liste.
//!
//! Le nom d'un plan est celui du modèle de noms d'aujourd'hui (`nommage.rs`) ; un plan retiré garde son titre d'alors (« Plan 3 »).
//! Aucun changement du format du projet.

use std::collections::{HashMap, HashSet};

use crate::document::Document;
use crate::envois::{Envoi, PlanEnvoye};
use crate::fildecision::Status;
use crate::pdf::{self, BoardImage, PdfFile};
use crate::relecture::{esc, plural};
use crate::storyboard::duration_label;

// ---------------------------------------------------------------------------------------------- les plans comparés

/// Un plan tel qu'un côté de la comparaison le connaît.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanVu {
    pub id: u64,
    pub titre: String,
    pub empreinte: u64,
    pub version: u32,
    pub images: usize,
}

impl From<&PlanEnvoye> for PlanVu {
    fn from(p: &PlanEnvoye) -> PlanVu {
        PlanVu { id: p.plan, titre: p.titre.clone(), empreinte: p.empreinte, version: p.version.max(1), images: p.images }
    }
}

/// Ce qui est arrivé à un plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Son contenu n'est plus le même (il a pu aussi changer de place : `Ligne::deplace`).
    Modifie,
    Ajoute,
    Retire,
    /// Le même contenu, ailleurs dans le film.
    Deplace,
}

/// Un plan qui a changé de quelque façon.
#[derive(Clone, Debug, PartialEq)]
pub struct Ligne {
    pub id: u64,
    pub genre: Genre,
    pub nom: String,
    /// Rang dans le film de chaque côté, à partir de 1.
    pub rang_a: Option<usize>,
    pub rang_b: Option<usize>,
    /// Le plan n'est plus à sa place dans l'ordre du film (en plus d'être modifié, s'il l'est).
    pub deplace: bool,
    pub images_a: Option<usize>,
    pub images_b: Option<usize>,
    pub version_a: Option<u32>,
    pub version_b: Option<u32>,
}

impl Ligne {
    /// Il y a des images à calculer pour cette ligne (un plan seulement déplacé n'en demande pas).
    pub fn a_des_images(&self) -> bool {
        self.genre != Genre::Deplace
    }
}

/// Le résultat de la comparaison de deux listes de plans.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Diff {
    /// Dans l'ordre du film d'après (puis les plans retirés, dans l'ordre d'avant).
    pub lignes: Vec<Ligne>,
    pub inchanges: usize,
    pub total_a: usize,
    pub total_b: usize,
}

impl Diff {
    fn compte(&self, g: Genre) -> usize {
        self.lignes.iter().filter(|l| l.genre == g).count()
    }

    pub fn modifies(&self) -> usize {
        self.compte(Genre::Modifie)
    }

    pub fn ajoutes(&self) -> usize {
        self.compte(Genre::Ajoute)
    }

    pub fn retires(&self) -> usize {
        self.compte(Genre::Retire)
    }

    /// Les plans qui ne sont plus à leur place (modifiés ou non).
    pub fn deplaces(&self) -> usize {
        self.lignes.iter().filter(|l| l.deplace).count()
    }

    pub fn est_vide(&self) -> bool {
        self.lignes.is_empty()
    }

    /// Les lignes pour lesquelles on calcule des images (avant et / ou après).
    pub fn a_rendre(&self) -> Vec<usize> {
        self.lignes.iter().enumerate().filter(|(_, l)| l.a_des_images()).map(|(i, _)| i).collect()
    }
}

/// Les plans de `a_ids` qui ne sont pas dans la plus longue suite (dans l'ordre de `b_ids`) restée en ordre : seuls les plans présents
/// des deux côtés comptent.
fn hors_d_ordre(a_ids: &[u64], b_ids: &[u64]) -> HashSet<u64> {
    let dans_a: HashSet<u64> = a_ids.iter().copied().collect();
    let communs_b: Vec<u64> = b_ids.iter().copied().filter(|i| dans_a.contains(i)).collect();
    let pos_b: HashMap<u64, usize> = communs_b.iter().enumerate().map(|(k, i)| (*i, k)).collect();
    let seq: Vec<(u64, usize)> = a_ids.iter().filter_map(|i| pos_b.get(i).map(|k| (*i, *k))).collect();
    // Plus longue suite croissante (patience), avec de quoi remonter.
    let mut queues: Vec<usize> = Vec::new();
    let mut prec: Vec<Option<usize>> = vec![None; seq.len()];
    for (i, (_, v)) in seq.iter().enumerate() {
        let place = queues.partition_point(|&q| seq[q].1 < *v);
        prec[i] = place.checked_sub(1).map(|p| queues[p]);
        if place == queues.len() {
            queues.push(i);
        } else {
            queues[place] = i;
        }
    }
    let mut garde: HashSet<usize> = HashSet::new();
    let mut cur = queues.last().copied();
    while let Some(i) = cur {
        garde.insert(i);
        cur = prec[i];
    }
    seq.iter().enumerate().filter(|(i, _)| !garde.contains(i)).map(|(_, (id, _))| *id).collect()
}

/// Compare deux listes de plans (dans l'ordre du film). `noms` : le nom d'aujourd'hui des plans qui sont encore dans le film ; un plan
/// qui n'y est plus garde son titre.
pub fn comparer(a: &[PlanVu], b: &[PlanVu], noms: &HashMap<u64, String>) -> Diff {
    let en_a: HashMap<u64, (usize, &PlanVu)> = a.iter().enumerate().map(|(k, p)| (p.id, (k, p))).collect();
    let en_b: HashMap<u64, (usize, &PlanVu)> = b.iter().enumerate().map(|(k, p)| (p.id, (k, p))).collect();
    let a_ids: Vec<u64> = a.iter().map(|p| p.id).collect();
    let b_ids: Vec<u64> = b.iter().map(|p| p.id).collect();
    let bouges = hors_d_ordre(&a_ids, &b_ids);
    let nom = |p: &PlanVu| noms.get(&p.id).cloned().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| p.titre.clone());
    let mut lignes = Vec::new();
    let mut inchanges = 0;
    for (k, pb) in b.iter().enumerate() {
        match en_a.get(&pb.id) {
            None => lignes.push(Ligne {
                id: pb.id,
                genre: Genre::Ajoute,
                nom: nom(pb),
                rang_a: None,
                rang_b: Some(k + 1),
                deplace: false,
                images_a: None,
                images_b: Some(pb.images),
                version_a: None,
                version_b: Some(pb.version),
            }),
            Some(&(ka, pa)) => {
                let deplace = bouges.contains(&pb.id);
                let genre = if pa.empreinte != pb.empreinte {
                    Genre::Modifie
                } else if deplace {
                    Genre::Deplace
                } else {
                    inchanges += 1;
                    continue;
                };
                lignes.push(Ligne {
                    id: pb.id,
                    genre,
                    nom: nom(pb),
                    rang_a: Some(ka + 1),
                    rang_b: Some(k + 1),
                    deplace,
                    images_a: Some(pa.images),
                    images_b: Some(pb.images),
                    version_a: Some(pa.version),
                    version_b: Some(pb.version),
                });
            }
        }
    }
    for (k, pa) in a.iter().enumerate() {
        if !en_b.contains_key(&pa.id) {
            lignes.push(Ligne {
                id: pa.id,
                genre: Genre::Retire,
                nom: nom(pa),
                rang_a: Some(k + 1),
                rang_b: None,
                deplace: false,
                images_a: Some(pa.images),
                images_b: None,
                version_a: Some(pa.version),
                version_b: None,
            });
        }
    }
    Diff { lignes, inchanges, total_a: a.len(), total_b: b.len() }
}

// ---------------------------------------------------------------------------------------------- les notes traitées

/// Une note traitée entre les deux envois.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteJ {
    pub id: u64,
    pub plan: u64,
    pub statut: Status,
    pub raison: String,
    pub texte: String,
    pub auteur: String,
    pub source: &'static str,
    /// Quand elle a été traitée (secondes depuis 1970) ; à défaut, quand elle a été écrite.
    pub date: u64,
}

/// Les notes traitées (fait, refusé, en discussion) dont la date est après `depuis` et au plus tard `jusqu_a` (`None` : jusqu'à
/// aujourd'hui). Dans l'ordre de leur date. Les notes vivent dans le projet d'aujourd'hui, pas dans les envois.
pub fn notes_entre(doc: &Document, depuis: u64, jusqu_a: Option<u64>) -> Vec<NoteJ> {
    let mut v: Vec<NoteJ> = doc
        .comments
        .iter()
        .filter(|c| c.status() != Status::Todo)
        .map(|c| NoteJ {
            id: c.id,
            plan: c.panel,
            statut: c.status(),
            raison: c.follow.reason.clone(),
            texte: c.text.trim().to_string(),
            auteur: c.author.trim().to_string(),
            source: c.source().label(),
            date: if c.follow.status_at > 0 { c.follow.status_at } else { c.created },
        })
        .filter(|n| n.date > depuis && jusqu_a.is_none_or(|j| n.date <= j))
        .collect();
    v.sort_by_key(|n| (n.date, n.id));
    v
}

/// La date d'un envoi pour borner les notes : le moment où SHOT l'a noté, à défaut le jour choisi.
pub fn instant_de(e: &Envoi) -> u64 {
    if e.note_le > 0 {
        e.note_le
    } else {
        e.date
    }
}

// ---------------------------------------------------------------------------------------------- le journal

/// Une image du journal, dans les formats qui sont demandés.
#[derive(Default)]
pub struct Vignette {
    /// JPEG pour la page HTML.
    pub html: Option<Vec<u8>>,
    /// JPEG pour le PDF.
    pub pdf: Option<BoardImage>,
}

/// Les images d'une ligne : avant, après, et la part des pixels qui diffèrent (de 0 à 1) quand les deux existent.
#[derive(Default)]
pub struct Rendu {
    pub avant: Option<Vignette>,
    pub apres: Option<Vignette>,
    pub pct: Option<f32>,
}

/// Tout ce que le journal dit, sans les images.
#[derive(Clone, Debug)]
pub struct Journal {
    pub projet: String,
    /// Le premier côté (« v3 au réal, 20/10/2026 ») et le second (un autre envoi ou « État actuel »).
    pub de: String,
    pub vers: String,
    /// La date où le journal est fait (« 30/10/2026 »).
    pub le: String,
    pub fps: u32,
    pub diff: Diff,
    pub notes: Vec<NoteJ>,
    /// Ce qui n'a pas pu être lu (l'instantané d'un envoi introuvable : pas d'images de ce côté).
    pub avertissements: Vec<String>,
    /// Largeur / hauteur du cadre.
    pub aspect: f32,
}

impl Journal {
    /// Les notes traitées qui concernent le plan `plan`.
    pub fn notes_du_plan(&self, plan: u64) -> Vec<&NoteJ> {
        self.notes.iter().filter(|n| n.plan == plan).collect()
    }

    /// « 7 plans modifiés, 1 ajouté, 2 déplacés, 44 inchangés. »
    pub fn resume(&self) -> String {
        let d = &self.diff;
        let mut parts = Vec::new();
        let accord = |n: usize, un: &str, plusieurs: &str| format!("{n} {}", if n > 1 { plusieurs } else { un });
        if d.modifies() > 0 {
            parts.push(format!("{} {}", plural(d.modifies(), "plan", "plans"), if d.modifies() > 1 { "modifiés" } else { "modifié" }));
        }
        if d.ajoutes() > 0 {
            parts.push(accord(d.ajoutes(), "ajouté", "ajoutés"));
        }
        if d.retires() > 0 {
            parts.push(accord(d.retires(), "retiré", "retirés"));
        }
        if d.deplaces() > 0 {
            parts.push(accord(d.deplaces(), "déplacé", "déplacés"));
        }
        if parts.is_empty() {
            return format!("Aucune différence : les {} plans sont les mêmes.", d.total_b);
        }
        if d.inchanges > 0 {
            parts.push(accord(d.inchanges, "inchangé", "inchangés"));
        }
        format!("{}.", parts.join(", "))
    }

    /// « 3 faites, 1 refusée, 1 en discussion » ; vide s'il n'y a aucune note.
    pub fn resume_notes(&self) -> String {
        let n = |s: Status| self.notes.iter().filter(|x| x.statut == s).count();
        let mut parts = Vec::new();
        let (f, r, d) = (n(Status::Done), n(Status::Refused), n(Status::Discussion));
        if f > 0 {
            parts.push(format!("{f} {}", if f > 1 { "faites" } else { "faite" }));
        }
        if r > 0 {
            parts.push(format!("{r} {}", if r > 1 { "refusées" } else { "refusée" }));
        }
        if d > 0 {
            parts.push(format!("{d} en discussion"));
        }
        parts.join(", ")
    }

    /// Le nom de fichier (sans extension) proposé : `Film_journal_v3-au-real_v4-au-real`.
    pub fn nom_de_fichier(&self, base: &str, a: &str, b: &str) -> String {
        let propre = |s: &str| -> String {
            let t: String = s.chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
            let t: Vec<&str> = t.split('-').filter(|x| !x.is_empty()).collect();
            t.join("-")
        };
        let base = match propre(base) {
            b if b.is_empty() => "Film".to_string(),
            b => b,
        };
        let (a, b) = (propre(a), propre(b));
        format!("{base}_journal_{}_{}", if a.is_empty() { "avant" } else { &a }, if b.is_empty() { "apres" } else { &b })
    }
}

/// Le journal (sans images) de l'envoi `ea` à l'envoi `eb`, ou à l'état d'aujourd'hui si `eb` est `None` : alors `actuels` donne les plans
/// d'aujourd'hui avec leurs empreintes (le suivi des plans, `envoisui.rs`). Les noms viennent de `doc`, les notes aussi (elles vivent
/// dans le projet, pas dans les envois) : seules celles des plans listés sont gardées. `maintenant` : secondes depuis 1970.
pub fn faire(doc: &Document, ea: &Envoi, eb: Option<&Envoi>, actuels: Vec<PlanVu>, maintenant: u64) -> Journal {
    let plans_a: Vec<PlanVu> = ea.plans.iter().map(PlanVu::from).collect();
    let plans_b: Vec<PlanVu> = match eb {
        Some(e) => e.plans.iter().map(PlanVu::from).collect(),
        None => actuels,
    };
    let noms: HashMap<u64, String> = doc.panels.iter().enumerate().map(|(i, p)| (p.id, doc.nom_de_plan(i))).collect();
    let diff = comparer(&plans_a, &plans_b, &noms);
    let listes: HashSet<u64> = diff.lignes.iter().map(|l| l.id).collect();
    let mut notes = notes_entre(doc, instant_de(ea), eb.map(instant_de));
    notes.retain(|n| listes.contains(&n.plan));
    let projet = match doc.properties.title.trim() {
        "" => crate::paquet::base_du_film(doc),
        t => t.to_string(),
    };
    Journal {
        projet,
        de: ea.libelle(),
        vers: eb.map_or_else(|| format!("État actuel, {}", crate::envois::date_courte(maintenant)), Envoi::libelle),
        le: crate::envois::date_courte(maintenant),
        fps: doc.fps,
        diff,
        notes,
        avertissements: Vec::new(),
        aspect: doc.frame[0] as f32 / doc.frame[1].max(1) as f32,
    }
}

/// La part de pixels qui diffèrent, écrite à la française : « 12,3 % », « < 0,1 % », « 0 % ».
pub fn pct_texte(p: f32) -> String {
    let c = p * 100.0;
    if c <= 0.0 {
        "0 %".into()
    } else if c < 0.05 {
        "< 0,1 %".into()
    } else {
        format!("{c:.1} %").replace('.', ",")
    }
}

/// La phrase qui dit la part de l'image qui diffère : « 12,3 % de l'image diffère », ou, quand les images sont les mêmes, que le changement est ailleurs.
pub fn pct_phrase(p: f32) -> String {
    if p <= 0.0 {
        "image identique : le changement est dans la durée, la caméra ou les réglages".into()
    } else {
        format!("{} de l'image diffère", pct_texte(p))
    }
}

/// « 2s 00 → 2s 12 (+12 i) » : la durée avant et après, et l'écart.
pub fn durees_texte(a: Option<usize>, b: Option<usize>, fps: u32, fleche: &str) -> String {
    match (a, b) {
        (Some(a), Some(b)) if a == b => format!("{} (inchangée)", duration_label(a, fps)),
        (Some(a), Some(b)) => {
            let ecart = if b > a { format!("+{}", duration_label(b - a, fps)) } else { format!("-{}", duration_label(a - b, fps)) };
            format!("{} {fleche} {} ({ecart})", duration_label(a, fps), duration_label(b, fps))
        }
        (Some(a), None) => duration_label(a, fps),
        (None, Some(b)) => duration_label(b, fps),
        (None, None) => String::new(),
    }
}

fn versions_texte(l: &Ligne, fleche: &str) -> Option<String> {
    let (a, b) = (l.version_a?, l.version_b);
    match b {
        Some(b) if a != b => Some(format!("V{a} {fleche} V{b}")),
        _ => None,
    }
}

fn rangs_texte(l: &Ligne, fleche: &str) -> String {
    match (l.rang_a, l.rang_b) {
        (Some(a), Some(b)) => format!("place {a} {fleche} {b}"),
        (Some(a), None) => format!("place {a}"),
        (None, Some(b)) => format!("place {b}"),
        (None, None) => String::new(),
    }
}

// ---------------------------------------------------------------------------------------------- la page HTML

const CSS: &str = ":root{color-scheme:light;--fond:#f4f3ef;--carte:#fff;--encre:#1c1c1e;--doux:#5d5d63;--trait:#d8d5cd;--accent:#1f5fd0;--accent-fond:#e7efff;--alerte:#b3261e;--ambre:#fff1cc;--ambre-encre:#7a4b00}\
*{box-sizing:border-box}body{margin:0;background:var(--fond);color:var(--encre);font:16px/1.5 system-ui,-apple-system,\"Segoe UI\",Roboto,\"Helvetica Neue\",Arial,sans-serif}\
h1,h2,h3,p{margin:0}header.haut{background:#fff;border-bottom:1px solid var(--trait);padding:12px 16px}\
.in{max-width:1100px;margin:0 auto}header.haut h1{font-size:1.3rem}.sous{color:var(--doux);font-size:.95rem;margin-top:2px}\
main{max-width:1100px;margin:0 auto;padding:16px}.resume{background:var(--accent-fond);border-radius:10px;padding:12px 14px;margin:0 0 16px}\
.avert{background:var(--ambre);color:var(--ambre-encre);border-radius:10px;padding:10px 14px;margin:0 0 16px}\
.sommaire{display:flex;flex-wrap:wrap;gap:6px;margin:0 0 18px}.sommaire a{padding:6px 10px;border:1px solid var(--trait);border-radius:999px;background:#fff;color:var(--encre);text-decoration:none;font-size:.9rem}\
h2{font-size:1.25rem;margin:26px 0 10px;padding-bottom:6px;border-bottom:2px solid var(--trait)}\
.plan{background:var(--carte);border:1px solid var(--trait);border-radius:14px;padding:14px;margin:0 0 18px;break-inside:avoid}\
.tete{display:flex;flex-wrap:wrap;align-items:baseline;gap:4px 12px;margin-bottom:6px}.tete h3{font-size:1.15rem}\
.pct{font-weight:700;color:var(--accent)}.etiq{display:inline-block;padding:0 8px;border-radius:999px;background:var(--ambre);color:var(--ambre-encre);font-size:.85rem;font-weight:700}\
.meta{color:var(--doux);font-size:.92rem;margin:0 0 8px}.paire{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr));gap:12px}\
figure{margin:0}figure img{display:block;width:100%;height:auto;border:1px solid var(--trait);border-radius:6px;background:#e9e7e1}\
figcaption{color:var(--doux);font-size:.88rem;margin:4px 0 0}.vide{display:flex;align-items:center;justify-content:center;min-height:120px;border:1px dashed var(--trait);border-radius:6px;color:var(--doux)}\
.notes{margin-top:10px}.note{border-left:4px solid var(--trait);padding:4px 0 4px 8px;margin:6px 0;font-size:.93rem}\
.note p{white-space:pre-line;overflow-wrap:anywhere}.badge{display:inline-block;padding:0 8px;border-radius:999px;background:#e4e2db;font-size:.8rem;font-weight:700}\
.note.fait{border-left-color:#1a8a3c}.note.fait .badge{background:#dff3e5;color:#13692d}\
.note.refuse{border-left-color:var(--alerte)}.note.refuse .badge{background:#fbe2e0;color:var(--alerte)}\
.note.discussion{border-left-color:var(--accent)}.note.discussion .badge{background:var(--accent-fond);color:var(--accent)}\
.raison{font-style:italic;color:var(--alerte)}table{border-collapse:collapse;width:100%;background:#fff;border:1px solid var(--trait)}\
td,th{padding:6px 10px;border-bottom:1px solid var(--trait);text-align:left;font-size:.93rem}\
footer{max-width:1100px;margin:0 auto;padding:8px 16px 24px;color:var(--doux);font-size:.85rem}\
@media print{body{background:#fff}header.haut{border:0}.sommaire{display:none}}";

fn classe_note(s: Status) -> &'static str {
    match s {
        Status::Done => "fait",
        Status::Refused => "refuse",
        Status::Discussion => "discussion",
        Status::Todo => "afaire",
    }
}

fn notes_html(j: &Journal, plan: u64) -> String {
    let notes = j.notes_du_plan(plan);
    if notes.is_empty() {
        return String::new();
    }
    let mut h = String::from("<div class=\"notes\">");
    for n in notes {
        h.push_str(&format!("<div class=\"note {}\"><span class=\"badge\">{}</span> <span class=\"meta\">{}{}</span>", classe_note(n.statut), esc(n.statut.label()), esc(n.source), if n.auteur.is_empty() { String::new() } else { format!(" · {}", esc(&n.auteur)) }));
        if !n.texte.is_empty() {
            h.push_str(&format!("<p>{}</p>", esc(&n.texte)));
        }
        if n.statut == Status::Refused && !n.raison.is_empty() {
            h.push_str(&format!("<p class=\"raison\">Raison : {}</p>", esc(&n.raison)));
        }
        h.push_str("</div>");
    }
    h.push_str("</div>");
    h
}

fn figure_html(v: Option<&Vignette>, legende: &str) -> String {
    match v.and_then(|v| v.html.as_ref()) {
        Some(jpeg) => format!("<figure><img alt=\"{0}\" src=\"data:image/jpeg;base64,{1}\"><figcaption>{0}</figcaption></figure>", esc(legende), crate::base64::encode(jpeg)),
        None => format!("<figure><div class=\"vide\">Image indisponible</div><figcaption>{}</figcaption></figure>", esc(legende)),
    }
}

/// La page HTML du journal, seule dans son fichier (images comprises), à joindre à un envoi ou à la page de relecture. `rendus` suit
/// `j.diff.lignes`.
pub fn page(j: &Journal, rendus: &[Option<Rendu>]) -> String {
    let d = &j.diff;
    let mut h = String::with_capacity(64 * 1024);
    h.push_str("<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str(&format!("<title>Journal des différences · {}</title><style>{CSS}</style></head><body>", esc(&j.projet)));
    h.push_str(&format!(
        "<header class=\"haut\"><div class=\"in\"><h1>Journal des différences · {}</h1><p class=\"sous\">{} → {} · fait le {}</p></div></header><main>",
        esc(&j.projet),
        esc(&j.de),
        esc(&j.vers),
        esc(&j.le)
    ));
    let notes = j.resume_notes();
    h.push_str(&format!("<div class=\"resume\"><p><b>{}</b></p>", esc(&j.resume())));
    if !j.notes.is_empty() {
        h.push_str(&format!("<p>{} traitée{} entre les deux{}.</p>", plural(j.notes.len(), "note", "notes"), if j.notes.len() > 1 { "s" } else { "" }, if notes.is_empty() { String::new() } else { format!(" : {}", esc(&notes)) }));
    }
    h.push_str("</div>");
    for a in &j.avertissements {
        h.push_str(&format!("<div class=\"avert\">{}</div>", esc(a)));
    }
    let par = |g: Genre| d.lignes.iter().enumerate().filter(move |(_, l)| l.genre == g);
    h.push_str("<nav class=\"sommaire\">");
    for (ancre, titre, n) in [("modifies", "Modifiés", d.modifies()), ("ajoutes", "Ajoutés", d.ajoutes()), ("retires", "Retirés", d.retires()), ("deplaces", "Déplacés", par(Genre::Deplace).count()), ("notes", "Notes traitées", j.notes.len())] {
        if n > 0 {
            h.push_str(&format!("<a href=\"#{ancre}\">{titre} ({n})</a>"));
        }
    }
    h.push_str("</nav>");
    let carte = |h: &mut String, i: usize, l: &Ligne| {
        let r = rendus.get(i).and_then(|r| r.as_ref());
        h.push_str(&format!("<article class=\"plan\" id=\"p{}\"><div class=\"tete\"><h3>{}</h3>", l.id, esc(&l.nom)));
        if let Some(p) = r.and_then(|r| r.pct) {
            h.push_str(&format!("<span class=\"pct\">{}</span>", esc(&pct_phrase(p))));
        }
        if l.deplace {
            h.push_str("<span class=\"etiq\">déplacé</span>");
        }
        h.push_str("</div>");
        let mut meta = Vec::new();
        let dur = durees_texte(l.images_a, l.images_b, j.fps, "→");
        if !dur.is_empty() {
            meta.push(format!("Durée : {dur}"));
        }
        if let Some(v) = versions_texte(l, "→") {
            meta.push(format!("Version : {v}"));
        }
        let place = rangs_texte(l, "→");
        if l.deplace && !place.is_empty() {
            meta.push(place);
        }
        if !meta.is_empty() {
            h.push_str(&format!("<p class=\"meta\">{}</p>", esc(&meta.join(" · "))));
        }
        h.push_str("<div class=\"paire\">");
        if l.genre != Genre::Ajoute {
            h.push_str(&figure_html(r.and_then(|r| r.avant.as_ref()), &format!("Avant : {}", j.de)));
        }
        if l.genre != Genre::Retire {
            h.push_str(&figure_html(r.and_then(|r| r.apres.as_ref()), &format!("Après : {}", j.vers)));
        }
        h.push_str("</div>");
        h.push_str(&notes_html(j, l.id));
        h.push_str("</article>");
    };
    for (g, ancre, titre) in [(Genre::Modifie, "modifies", "Plans modifiés"), (Genre::Ajoute, "ajoutes", "Plans ajoutés"), (Genre::Retire, "retires", "Plans retirés")] {
        let lignes: Vec<(usize, &Ligne)> = par(g).collect();
        if lignes.is_empty() {
            continue;
        }
        h.push_str(&format!("<section id=\"{ancre}\"><h2>{titre} ({})</h2>", lignes.len()));
        for (i, l) in lignes {
            carte(&mut h, i, l);
        }
        h.push_str("</section>");
    }
    let deplaces: Vec<&Ligne> = par(Genre::Deplace).map(|(_, l)| l).collect();
    if !deplaces.is_empty() {
        h.push_str(&format!("<section id=\"deplaces\"><h2>Plans déplacés ({})</h2><table><tr><th>Plan</th><th>Place</th></tr>", deplaces.len()));
        for l in deplaces {
            h.push_str(&format!("<tr><td>{}</td><td>{}</td></tr>", esc(&l.nom), esc(&rangs_texte(l, "→"))));
        }
        h.push_str("</table></section>");
    }
    if !j.notes.is_empty() {
        h.push_str(&format!("<section id=\"notes\"><h2>Notes traitées entre les deux ({})</h2><div class=\"notes\">", j.notes.len()));
        let noms: HashMap<u64, &str> = d.lignes.iter().map(|l| (l.id, l.nom.as_str())).collect();
        for n in &j.notes {
            let plan = noms.get(&n.plan).map_or_else(|| "autre plan".to_string(), |s| s.to_string());
            h.push_str(&format!("<div class=\"note {}\"><span class=\"badge\">{}</span> <span class=\"meta\">{} · {}{}</span>", classe_note(n.statut), esc(n.statut.label()), esc(&plan), esc(n.source), if n.auteur.is_empty() { String::new() } else { format!(" · {}", esc(&n.auteur)) }));
            if !n.texte.is_empty() {
                h.push_str(&format!("<p>{}</p>", esc(&n.texte)));
            }
            if n.statut == Status::Refused && !n.raison.is_empty() {
                h.push_str(&format!("<p class=\"raison\">Raison : {}</p>", esc(&n.raison)));
            }
            h.push_str("</div>");
        }
        h.push_str("</div></section>");
    }
    h.push_str(&format!("</main><footer>Journal des différences fait par SHOT le {}. Les noms des plans sont ceux d'aujourd'hui.</footer></body></html>", esc(&j.le)));
    h
}

// ---------------------------------------------------------------------------------------------- le PDF

const W: f32 = pdf::PAGE_W;
const H: f32 = pdf::PAGE_H;
const M: f32 = 28.0;
const INTERLIGNE: f32 = 10.5;
const PIED: f32 = 22.0;

struct Feuille {
    ops: String,
    images: Vec<usize>,
}

struct Mise {
    pdf: PdfFile,
    pages: Vec<Feuille>,
    y: f32,
}

fn rect(ops: &mut String, g: f32, x: f32, top: f32, w: f32, h: f32) {
    ops.push_str(&format!("{g:.2} G 0.5 w {x:.2} {:.2} {w:.2} {h:.2} re S\n", H - top - h));
}

impl Mise {
    fn nouvelle_page(&mut self) {
        self.pages.push(Feuille { ops: String::new(), images: Vec::new() });
        self.y = M + 8.0;
    }

    fn page(&mut self) -> &mut Feuille {
        if self.pages.is_empty() {
            self.nouvelle_page();
        }
        self.pages.last_mut().expect("une page")
    }

    fn assez_de_place(&self, h: f32) -> bool {
        self.y + h <= H - M - PIED
    }

    fn texte(&mut self, x: f32, s: &str, size: f32, bold: bool, gray: f32) {
        let y = self.y;
        self.page().ops.push_str(&pdf::op_text(H, x, y, s, size, bold, gray));
    }

    /// Des lignes à la suite, avec une nouvelle page quand il n'y a plus de place.
    fn paragraphe(&mut self, s: &str, size: f32, bold: bool, gray: f32, retrait: f32) {
        for ligne in pdf::wrap(s, bold, size, W - 2.0 * M - retrait, 0.0) {
            if !self.assez_de_place(size + 2.0) {
                self.nouvelle_page();
            }
            self.y += size + 1.5;
            self.texte(M + retrait, &ligne, size, bold, gray);
        }
    }

    fn jpeg(&mut self, img: &BoardImage, x: f32, top: f32, w: f32, h: f32) {
        let id = self.pdf.add_jpeg(img.width, img.height, &img.jpeg);
        let page = self.page();
        page.ops.push_str(&pdf::op_image(H, id, x, top, w, h));
        page.images.push(id);
        rect(&mut page.ops, 0.6, x, top, w, h);
    }
}

fn coupe(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>().trim_end())
    }
}

/// Lignes de texte d'une note pour le PDF (statut, texte, raison), coupées pour qu'une note ne prenne jamais plus d'une demi-page.
fn note_pdf(n: &NoteJ, nom: Option<&str>) -> String {
    let mut s = format!("{}", n.statut.label());
    if let Some(nom) = nom {
        s.push_str(&format!(" · {nom}"));
    }
    s.push_str(&format!(" · {}", n.source));
    if !n.auteur.is_empty() {
        s.push_str(&format!(" · {}", n.auteur));
    }
    if !n.texte.is_empty() {
        s.push_str(&format!(" : {}", coupe(&n.texte, 400)));
    }
    if n.statut == Status::Refused && !n.raison.is_empty() {
        s.push_str(&format!(" (raison : {})", coupe(&n.raison, 300)));
    }
    s
}

/// Le PDF du journal : A4 en paysage, une page de résumé puis les plans modifiés, ajoutés et retirés (avant / après), les plans déplacés
/// et les notes traitées. `rendus` suit `j.diff.lignes`.
pub fn pdf(j: &Journal, rendus: &[Option<Rendu>]) -> Vec<u8> {
    let d = &j.diff;
    let mut m = Mise { pdf: PdfFile::new(), pages: Vec::new(), y: M + 8.0 };
    m.nouvelle_page();
    m.y = M + 14.0;
    m.texte(M, &format!("Journal des différences · {}", coupe(&j.projet, 80)), 15.0, true, 0.1);
    m.y += 16.0;
    m.paragraphe(&format!("{} -> {}", j.de, j.vers), 10.0, false, 0.3, 0.0);
    m.paragraphe(&format!("Fait le {}. Les noms des plans sont ceux d'aujourd'hui.", j.le), 8.0, false, 0.45, 0.0);
    m.y += 8.0;
    m.paragraphe(&j.resume(), 11.0, true, 0.1, 0.0);
    if !j.notes.is_empty() {
        let r = j.resume_notes();
        m.paragraphe(&format!("{} traitée{} entre les deux{}.", plural(j.notes.len(), "note", "notes"), if j.notes.len() > 1 { "s" } else { "" }, if r.is_empty() { String::new() } else { format!(" : {r}") }), 9.0, false, 0.25, 0.0);
    }
    for a in &j.avertissements {
        m.paragraphe(&format!("Attention : {a}"), 8.5, false, 0.35, 0.0);
    }
    let listes: [(Genre, &str); 3] = [(Genre::Ajoute, "Ajoutés"), (Genre::Retire, "Retirés"), (Genre::Deplace, "Déplacés")];
    for (g, titre) in listes {
        let l: Vec<&Ligne> = d.lignes.iter().filter(|l| l.genre == g).collect();
        if l.is_empty() {
            continue;
        }
        m.y += 8.0;
        m.paragraphe(&format!("{titre} ({})", l.len()), 10.0, true, 0.1, 0.0);
        for x in l {
            let extra = match g {
                Genre::Deplace => rangs_texte(x, "->"),
                _ => durees_texte(x.images_a, x.images_b, j.fps, "->"),
            };
            m.paragraphe(&format!("- {}{}", x.nom, if extra.is_empty() { String::new() } else { format!("  ({extra})") }), 8.5, false, 0.2, 8.0);
        }
    }
    // Les plans qui ont des images : une carte chacun.
    let img_w = 270.0_f32;
    let img_h = (img_w / j.aspect.max(0.2)).min(190.0);
    let img_w = img_h * j.aspect.max(0.2);
    let mut premiere = true;
    for (i, l) in d.lignes.iter().enumerate().filter(|(_, l)| l.a_des_images()) {
        let r = rendus.get(i).and_then(|r| r.as_ref());
        let notes = j.notes_du_plan(l.id);
        // Les lignes de notes de la carte (au plus six notes, le reste est compté).
        let mut lignes_notes: Vec<String> = Vec::new();
        for (k, n) in notes.iter().enumerate() {
            if k == 6 {
                lignes_notes.push(format!("+ {} de plus dans la liste des notes.", plural(notes.len() - 6, "autre note", "autres notes")));
                break;
            }
            lignes_notes.extend(pdf::wrap(&note_pdf(n, None), false, 8.0, W - 2.0 * M - 6.0, 0.0).into_iter().take(4));
        }
        let h_carte = 30.0 + img_h + 12.0 + lignes_notes.len() as f32 * INTERLIGNE + 14.0;
        if premiere || !m.assez_de_place(h_carte) {
            m.nouvelle_page();
            m.y = M + 6.0;
            if premiere {
                premiere = false;
                m.texte(M, "Plans avec images (avant / après)", 9.0, true, 0.4);
                m.y += 12.0;
            }
        }
        let top = m.y;
        // Titre, part de l'image, durée.
        m.y = top + 11.0;
        let genre = match l.genre {
            Genre::Modifie => "",
            Genre::Ajoute => " (ajouté)",
            Genre::Retire => " (retiré)",
            Genre::Deplace => "",
        };
        m.texte(M, &format!("{}{genre}", coupe(&l.nom, 70)), 10.5, true, 0.1);
        if let Some(p) = r.and_then(|r| r.pct) {
            let t = pct_phrase(p);
            let x = W - M - pdf::text_width(&t, true, 9.0);
            m.texte(x, &t, 9.0, true, 0.25);
        }
        let mut meta = Vec::new();
        let dur = durees_texte(l.images_a, l.images_b, j.fps, "->");
        if !dur.is_empty() {
            meta.push(format!("Durée : {dur}"));
        }
        if let Some(v) = versions_texte(l, "->") {
            meta.push(format!("Version : {v}"));
        }
        if l.deplace {
            meta.push(format!("Déplacé : {}", rangs_texte(l, "->")));
        }
        m.y = top + 23.0;
        m.texte(M, &meta.join("   "), 8.5, false, 0.4);
        let img_top = top + 30.0;
        let place_gauche = M;
        let place_droite = M + img_w + 18.0;
        let avant = r.and_then(|r| r.avant.as_ref()).and_then(|v| v.pdf.as_ref());
        let apres = r.and_then(|r| r.apres.as_ref()).and_then(|v| v.pdf.as_ref());
        // Une seule image (ajouté ou retiré) : à gauche.
        let (g, dr): (Option<(&BoardImage, String)>, Option<(&BoardImage, String)>) = match l.genre {
            Genre::Ajoute => (apres.map(|a| (a, format!("Après : {}", j.vers))), None),
            Genre::Retire => (avant.map(|a| (a, format!("Avant : {}", j.de))), None),
            _ => (avant.map(|a| (a, format!("Avant : {}", j.de))), apres.map(|a| (a, format!("Après : {}", j.vers)))),
        };
        for (slot, x) in [(g, place_gauche), (dr, place_droite)] {
            if let Some((img, legende)) = slot {
                m.jpeg(img, x, img_top, img_w, img_h);
                m.y = img_top + img_h + 9.0;
                m.texte(x, &coupe(&legende, 60), 7.5, false, 0.45);
            }
        }
        if avant.is_none() && apres.is_none() {
            let page = m.page();
            rect(&mut page.ops, 0.7, place_gauche, img_top, img_w, img_h);
            m.y = img_top + img_h / 2.0;
            m.texte(place_gauche + 8.0, "Image indisponible", 8.5, false, 0.5);
        }
        m.y = img_top + img_h + 12.0;
        for ligne in &lignes_notes {
            m.y += INTERLIGNE;
            m.texte(M + 6.0, ligne, 8.0, false, 0.15);
        }
        m.y += 14.0;
    }
    // Les notes traitées, toutes, à la fin.
    if !j.notes.is_empty() {
        m.nouvelle_page();
        m.y = M + 10.0;
        m.texte(M, &format!("Notes traitées entre les deux ({})", j.notes.len()), 12.0, true, 0.1);
        m.y += 6.0;
        let noms: HashMap<u64, &str> = d.lignes.iter().map(|l| (l.id, l.nom.as_str())).collect();
        for n in &j.notes {
            let nom = noms.get(&n.plan).copied().unwrap_or("autre plan");
            m.paragraphe(&format!("- {}", note_pdf(n, Some(nom))), 8.5, false, 0.15, 6.0);
        }
    }
    // Les pieds de page, une fois le nombre de pages connu.
    let total = m.pages.len();
    let Mise { mut pdf, pages, .. } = m;
    for (k, mut p) in pages.into_iter().enumerate() {
        p.ops.push_str(&pdf::op_text(H, M, H - M + 6.0, &format!("Journal des différences · {}", coupe(&j.projet, 50)), 7.5, false, 0.5));
        let t = format!("page {} / {total}", k + 1);
        p.ops.push_str(&pdf::op_text(H, W - M - pdf::text_width(&t, false, 7.5), H - M + 6.0, &t, 7.5, false, 0.5));
        pdf.add_page(p.ops.as_bytes(), &p.images);
    }
    pdf.finish(&format!("Journal des différences · {}", j.projet))
}
