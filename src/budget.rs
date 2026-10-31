//! Budget de durée (Anthony, 2026-10-30, audit solution 5 : « l'animatique dépasse la durée de l'épisode ») : une **durée cible** pour le film
//! (l'épisode) et pour chaque séquence, et l'écart en direct.
//!
//! # Ce qui est gardé
//!
//! - `Properties::budget_ms` : la cible de tout le film, en millisecondes (0 : aucune) ;
//! - `Properties::budgets_sequences` : la cible de chaque séquence (numéro de la séquence → millisecondes), sans entrée quand il n'y en a pas.
//!
//! **Format 46** : `proprietes.budget_ms` et `proprietes.budgets_sequences`, absents quand ils sont vides ; un projet de version 45 ou moins
//! s'ouvre à l'identique, sans budget. Les cibles sont dans l'annulation avec le reste des propriétés.
//!
//! # L'écart
//!
//! L'écart est en **images** du projet : durée réelle − cible (la cible est arrondie à l'image la plus proche). Positif : le film (ou la
//! séquence) est trop long, c'est « dépassé » (rouge à l'écran). Où il se voit : la barre des séquences du montage (« 0:42 / 0:40 »), la fiche
//! du plan (lignes « Séquence » et « Film »), les propriétés du projet (une ligne par séquence), la colonne « Écart cumulé (s) » de la fiche
//! technique (décochée d'office).
//!
//! # L'écart cumulé de la fiche technique
//!
//! Sur le **dernier plan de chaque séquence** : (durée du film jusqu'à la fin de cette séquence) − (ce que les cibles permettaient jusque-là,
//! une séquence sans cible comptant pour sa durée réelle). Il dit, séquence après séquence, de combien le film avance sur son budget ou
//! prend du retard. Sans aucune cible de séquence, mais avec une cible du film : sur le dernier plan du film, l'écart au budget du film. Les
//! autres lignes sont vides, et rien n'est écrit sans budget.

use std::collections::HashMap;

use crate::document::Document;

/// Plus longue cible permise : 24 heures.
pub const MAX_MS: u32 = 24 * 3600 * 1000;

/// Une durée tapée en millisecondes : « 90 » (secondes), « 1:30 » (minutes:secondes), « 1:02:30 » (heures:minutes:secondes), avec une fraction de
/// seconde si l'on veut (« 1:30,5 », « 12.5 »). Rien ou « aucune » : `Some(0)` (pas de cible). Illisible : `None`.
pub fn lire_duree(texte: &str) -> Option<u32> {
    let t = texte.trim().to_lowercase();
    if t.is_empty() || t == "aucune" || t == "-" {
        return Some(0);
    }
    let t = t.trim_end_matches('s').trim().replace(',', ".");
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() > 3 || parts.iter().any(|p| p.trim().is_empty()) {
        return None;
    }
    let mut secondes = 0.0f64;
    for (k, p) in parts.iter().enumerate() {
        let v: f64 = p.trim().parse().ok().filter(|v: &f64| v.is_finite() && *v >= 0.0)?;
        // Seule la dernière partie (les secondes) peut avoir une fraction.
        if k + 1 < parts.len() && v.fract() != 0.0 {
            return None;
        }
        secondes = secondes * 60.0 + v;
    }
    let ms = (secondes * 1000.0).round();
    (ms <= MAX_MS as f64).then_some(ms as u32)
}

/// Une durée en millisecondes pour l'écran : « 1:30 », « 1:30,5 », « 1:02:30 » ; « aucune » pour zéro.
pub fn texte_duree(ms: u32) -> String {
    if ms == 0 {
        return "aucune".into();
    }
    let total = ms as f64 / 1000.0;
    let (h, m) = ((total / 3600.0).floor() as u64, ((total % 3600.0) / 60.0).floor() as u64);
    let s = total % 60.0;
    let s_txt = if (s - s.round()).abs() < 0.0005 { format!("{:02}", s.round() as u64) } else { format!("{s:06.3}").trim_end_matches('0').replace('.', ",") };
    if h > 0 {
        format!("{h}:{m:02}:{s_txt}")
    } else {
        format!("{m}:{s_txt}")
    }
}

/// Le texte d'un écart en images : « +2s 12 », « -12 i », « 0 ».
pub fn texte_ecart(ecart: i64, fps: u32) -> String {
    if ecart == 0 {
        return "0".into();
    }
    format!("{}{}", if ecart > 0 { "+" } else { "-" }, crate::storyboard::duration_label(ecart.unsigned_abs() as usize, fps))
}

/// Le budget d'un film ou d'une séquence, au regard de ce qui dure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Ce qui dure, en images.
    pub duree: usize,
    /// La cible, en images.
    pub cible: usize,
}

impl Budget {
    /// Durée réelle − cible, en images.
    pub fn ecart(&self) -> i64 {
        self.duree as i64 - self.cible as i64
    }

    /// Plus long que la cible.
    pub fn depasse(&self) -> bool {
        self.ecart() > 0
    }

    /// « 0:42 / 0:40 (+2s 00) » : ce qui dure, la cible, l'écart.
    pub fn texte(&self, fps: u32) -> String {
        format!("{} / {} ({})", duree_courte(self.duree, fps), duree_courte(self.cible, fps), texte_ecart(self.ecart(), fps))
    }
}

/// « 0:42 » ; avec les images qui restent quand ce n'est pas une seconde entière : « 0:42+06 ».
pub fn duree_courte(images: usize, fps: u32) -> String {
    let fps = fps.max(1) as usize;
    let (s, f) = (images / fps, images % fps);
    if f == 0 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        format!("{}:{:02}+{f:02}", s / 60, s % 60)
    }
}

/// Une cible en millisecondes, en images (au moins une image).
pub fn ms_en_images(ms: u32, fps: u32) -> usize {
    ((ms as f64 * fps.max(1) as f64 / 1000.0).round() as usize).max(1)
}

impl Document {
    /// Le budget de tout le film ; rien s'il n'a pas de cible.
    pub fn budget_du_film(&self) -> Option<Budget> {
        let ms = self.properties.budget_ms;
        (ms > 0).then(|| Budget { duree: self.panels.iter().map(|p| p.frame_count()).sum(), cible: ms_en_images(ms, self.fps) })
    }

    /// La cible d'une séquence, en millisecondes ; rien si elle n'en a pas (ou si la séquence n'existe plus).
    pub fn cible_de_sequence(&self, sequence: u64) -> Option<u32> {
        self.properties.budgets_sequences.get(&sequence).copied().filter(|&ms| ms > 0 && self.sequences.iter().any(|s| s.id == sequence))
    }

    /// Le budget de la séquence `sequence` (la durée de tous ses plans) ; rien sans cible.
    pub fn budget_de_sequence(&self, sequence: u64) -> Option<Budget> {
        let ms = self.cible_de_sequence(sequence)?;
        let ids = self.sequence_ids();
        let duree = self.panels.iter().zip(&ids).filter(|(_, s)| **s == sequence).map(|(p, _)| p.frame_count()).sum();
        Some(Budget { duree, cible: ms_en_images(ms, self.fps) })
    }

    /// Pose (ou retire, avec 0) la cible d'une séquence.
    pub fn poser_cible_de_sequence(&mut self, sequence: u64, ms: u32) {
        if ms == 0 {
            self.properties.budgets_sequences.remove(&sequence);
        } else {
            self.properties.budgets_sequences.insert(sequence, ms.min(MAX_MS));
        }
    }
}

/// Le budget de chaque séquence qui a une cible, en une passe : numéro de la séquence → budget. (La barre des séquences du montage le demande à chaque
/// image d'écran : une seule passe sur les plans, et rien du tout quand aucune cible n'est posée.)
pub fn budgets_des_sequences(doc: &Document) -> HashMap<u64, Budget> {
    let mut out = HashMap::new();
    if doc.properties.budgets_sequences.is_empty() {
        return out;
    }
    let ids = doc.sequence_ids();
    let mut durees: HashMap<u64, usize> = HashMap::new();
    for (p, s) in doc.panels.iter().zip(&ids) {
        *durees.entry(*s).or_default() += p.frame_count();
    }
    for (&seq, &ms) in &doc.properties.budgets_sequences {
        if ms > 0 && doc.sequences.iter().any(|s| s.id == seq) {
            out.insert(seq, Budget { duree: durees.get(&seq).copied().unwrap_or(0), cible: ms_en_images(ms, doc.fps) });
        }
    }
    out
}

/// L'écart cumulé (en images, signé) à écrire sur chaque plan de la fiche technique : rien partout, sauf sur le dernier plan de chaque séquence
/// (voir l'en-tête du module).
pub fn ecarts_cumules(doc: &Document) -> Vec<Option<i64>> {
    let n = doc.panels.len();
    let mut out: Vec<Option<i64>> = vec![None; n];
    if n == 0 {
        return out;
    }
    let fps = doc.fps;
    let ids = doc.sequence_ids();
    let durees: Vec<usize> = doc.panels.iter().map(|p| p.frame_count()).collect();
    let avec_cibles = ids.iter().any(|s| doc.cible_de_sequence(*s).is_some());
    if avec_cibles {
        let mut reel = 0i64;
        let mut cible = 0i64;
        let mut deja: Vec<u64> = Vec::new();
        let mut i = 0;
        while i < n {
            let mut e = i + 1;
            while e < n && ids[e] == ids[i] {
                e += 1;
            }
            let run: i64 = durees[i..e].iter().map(|d| *d as i64).sum();
            // (Une séquence coupée en deux morceaux : la cible ne compte qu'une fois, pour le premier.)
            let voulu = if deja.contains(&ids[i]) { None } else { doc.cible_de_sequence(ids[i]) };
            deja.push(ids[i]);
            reel += run;
            cible += voulu.map_or(run, |ms| ms_en_images(ms, fps) as i64);
            out[e - 1] = Some(reel - cible);
            i = e;
        }
    } else if doc.properties.budget_ms > 0 {
        let reel: i64 = durees.iter().map(|d| *d as i64).sum();
        out[n - 1] = Some(reel - ms_en_images(doc.properties.budget_ms, fps) as i64);
    }
    out
}

/// L'écart cumulé de la fiche en secondes (trois décimales), pour la cellule.
pub fn ecart_en_secondes(images: i64, fps: u32) -> f64 {
    (images as f64 / fps.max(1) as f64 * 1000.0).round() / 1000.0
}
