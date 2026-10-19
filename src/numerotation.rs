//! Numérotation des plans et des séquences (Anthony, 2026-10-20) : « Suivie » ou « Verrouillée ».
//!
//! # Deux modes
//!
//! - **Suivie** (l'ancien comportement, par défaut) : le numéro d'un plan est son rang dans le film
//!   (1, 2, 3…), recalculé à chaque changement. Insérer un plan avant le 12 fait glisser tous les
//!   suivants.
//! - **Verrouillée** : chaque plan (et chaque séquence) garde un numéro **à lui**, enregistré avec le
//!   projet (`Scene::number`, `Sequence::number`). Rien ne glisse plus :
//!   - un plan **inséré** entre le 12 et le 13 reçoit **12A** (puis 12B si on en insère un autre
//!     juste après le 12A, et ainsi de suite) ; couper un plan, le dupliquer, en coller, en créer un
//!     nouveau suivent la même règle ;
//!   - un plan **supprimé** laisse un trou : les autres ne changent pas, et son numéro n'est **jamais
//!     redonné** (un plan inséré à sa place reçoit une lettre : le montage a peut-être déjà un « 13 ») ;
//!   - un plan **déplacé** garde son numéro (les numéros ne sont alors plus dans l'ordre du film
//!     tant qu'on ne renumérote pas) ;
//!   - « Renuméroter » remet des numéros propres, 1, 2, 3… dans l'ordre du film (une étape
//!     d'annulation, après confirmation).
//!
//! # Forme d'un numéro
//!
//! Un numéro est un entier (`base`) suivi d'une **queue** de chiffres et de lettres majuscules
//! (`tail`, vide au départ) : « 12 », « 12A », « 12AA », « 12A1 ». L'ordre est celui de la base,
//! puis celui de la queue lue comme du texte (chiffres avant lettres, une queue plus courte avant
//! une plus longue qui la commence) : 12 < 12A < 12A1 < 12AA < 12B < 13.
//!
//! La règle d'insertion (`Num::between`) : après un numéro `P`, avant un numéro `N` :
//! 1. si `N` est d'une autre base que `P`, `P` avec sa dernière lettre suivante (12 → 12A, 12A → 12B,
//!    12Z → 12ZA) ;
//! 2. sinon, la même chose si elle reste avant `N` ;
//! 3. sinon (12A et 12B se touchent), on prolonge `P` d'un chiffre : 12A → 12A1, puis 12A2…
//!    (12A1 reste avant 12AA, qui reste avant 12B) ;
//! 4. entre 12 et 12A (rien ne tient entre eux avec des lettres) la queue commence par un chiffre,
//!    et le numéro s'écrit avec un point : « 12.5 » (12 < 12.5 < 12A) ;
//! 5. avant le premier plan, le numéro est celui d'avant (le premier est « 1 » : le nouveau est
//!    « 0 », puis « -1 »…).
//!
//! Un numéro est toujours unique dans le projet : si le candidat est déjà pris, on passe au suivant.
//!
//! # Fichier
//!
//! Champs facultatifs du manifeste (`numero` d'une séquence et d'un plan, `numerotation_verrouillee`)
//! : un projet d'avant s'ouvre tel quel (numérotation suivie), sans changer `VERSION`. Une version
//! plus ancienne de SHOT ouvre un projet verrouillé en ignorant ces champs, puis les perd à
//! l'enregistrement (la numérotation redevient suivie).
//!
//! # Noms pour le montage
//!
//! `Document::plan_ref` / `sequence_ref` : « PL012A », « SQ01 » (3 et 2 chiffres au moins pour la
//! base) ; `plan_full_ref` : « SQ01_PL012A » ; ce sont les noms des clips des exports pour le
//! montage (`editorial.rs`).

use std::cmp::Ordering;
use std::collections::HashSet;

use crate::document::Document;

/// Les « chiffres » d'une queue : 0-9 puis A-Z.
const DIGITS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";

fn digit(c: u8) -> usize {
    DIGITS.iter().position(|&d| d == c).unwrap_or(0)
}

/// Un numéro : un entier, puis une queue éventuelle.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Num {
    pub base: i64,
    /// Chiffres et lettres majuscules, jamais terminée par un 0.
    pub tail: String,
}

impl Num {
    pub fn new(base: i64) -> Num {
        Num { base, tail: String::new() }
    }

    /// Lit « 12 », « 12A », « 12.5 », « -1 » (majuscules ou minuscules). Rien si ce n'est pas un
    /// numéro de cette forme.
    pub fn parse(text: &str) -> Option<Num> {
        let t = text.trim();
        let (neg, rest) = match t.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, t),
        };
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 15 {
            return None;
        }
        let base: i64 = rest[..digits].parse().ok()?;
        let mut tail = &rest[digits..];
        let dotted = tail.starts_with('.');
        if dotted {
            tail = &tail[1..];
            // Avec le point, la queue commence par un chiffre (sinon l'écriture n'aurait pas de point).
            if !tail.bytes().next().is_some_and(|c| c.is_ascii_digit()) {
                return None;
            }
        }
        if !tail.bytes().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }
        let tail = tail.to_ascii_uppercase();
        if tail.ends_with('0') {
            return None;
        }
        Some(Num { base: if neg { -base } else { base }, tail })
    }

    /// « 12 », « 12A », « 12.5 ».
    pub fn text(&self) -> String {
        self.padded(0)
    }

    /// Comme `text`, la base complétée de zéros à `width` chiffres au moins (« 012A »).
    pub fn padded(&self, width: usize) -> String {
        let base = if self.base < 0 { format!("-{:0w$}", -self.base, w = width) } else { format!("{:0w$}", self.base, w = width) };
        if self.tail.bytes().next().is_some_and(|c| c.is_ascii_digit()) {
            format!("{base}.{}", self.tail)
        } else {
            format!("{base}{}", self.tail)
        }
    }

    /// Le numéro qui suit celui-ci dans la même base : 12 → 12A, 12A → 12B, 12Z → 12ZA, 12A1 → 12A2.
    pub fn next_in_base(&self) -> Num {
        let mut tail = self.tail.clone().into_bytes();
        match tail.last().copied() {
            None => tail.push(b'A'),
            Some(b'Z') => tail.push(b'A'),
            Some(c) => {
                let n = tail.len() - 1;
                tail[n] = DIGITS[digit(c) + 1];
            }
        }
        Num { base: self.base, tail: String::from_utf8(tail).expect("queue ASCII") }
    }

    /// Un numéro strictement entre `prev` et `next` (le plan est inséré entre eux), selon les règles
    /// de l'en-tête du module. `next` peut manquer (fin du film), `prev` aussi (début).
    pub fn between(prev: Option<&Num>, next: Option<&Num>) -> Num {
        match (prev, next) {
            (None, None) => Num::new(1),
            (Some(p), None) => Num::new(p.base + 1),
            (None, Some(n)) => {
                if n.tail.is_empty() {
                    Num::new(n.base - 1)
                } else {
                    Num::new(n.base)
                }
            }
            (Some(p), Some(n)) => {
                if n <= p {
                    // Numéros dans le désordre (un plan a été déplacé) : juste après le précédent.
                    return p.next_in_base();
                }
                let simple = p.next_in_base();
                if n.base != p.base || simple < *n {
                    return simple;
                }
                Num { base: p.base, tail: midpoint(p.tail.as_bytes(), Some(n.tail.as_bytes())) }
            }
        }
    }
}

impl Ord for Num {
    fn cmp(&self, other: &Num) -> Ordering {
        self.base.cmp(&other.base).then_with(|| self.tail.as_bytes().cmp(other.tail.as_bytes()))
    }
}

impl PartialOrd for Num {
    fn partial_cmp(&self, other: &Num) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Une queue strictement entre `a` et `b` (`a` < `b`, `b` absent = sans borne), sans 0 à la fin :
/// la subdivision classique d'un ordre de textes (indices fractionnaires).
fn midpoint(a: &[u8], b: Option<&[u8]>) -> String {
    let mut out = Vec::new();
    let (mut a, mut b) = (a, b);
    loop {
        // Début commun (en lisant `a` complété de 0).
        if let Some(bb) = b {
            let mut n = 0;
            while n < bb.len() && a.get(n).map_or(0, |&c| digit(c)) == digit(bb[n]) {
                n += 1;
            }
            if n > 0 {
                out.extend_from_slice(&bb[..n]);
                a = a.get(n..).unwrap_or(&[]);
                b = Some(&bb[n..]);
                continue;
            }
        }
        let da = a.first().map_or(0, |&c| digit(c));
        let db = b.and_then(|bb| bb.first()).map_or(36, |&c| digit(c));
        if db > da + 1 {
            // Sans borne haute : le chiffre suivant (12A → 12A1) ; sinon le milieu.
            let mid = if b.is_none() { da + 1 } else { (da + db) / 2 };
            out.push(DIGITS[mid]);
            break;
        }
        // Chiffres voisins : si `b` continue, son premier chiffre seul tient entre les deux.
        if let Some(bb) = b.filter(|bb| bb.len() > 1) {
            out.push(bb[0]);
            break;
        }
        out.push(DIGITS[da]);
        a = a.get(1..).unwrap_or(&[]);
        b = None;
    }
    String::from_utf8(out).expect("queue ASCII")
}

/// Donne un numéro à chaque entrée vide de `nums` (dans l'ordre du film), entre ses voisines, sans
/// jamais en répéter un.
fn fill(nums: &mut [Option<Num>]) {
    let mut used: HashSet<Num> = nums.iter().flatten().cloned().collect();
    for i in 0..nums.len() {
        if nums[i].is_some() {
            continue;
        }
        let prev = i.checked_sub(1).and_then(|k| nums[k].clone());
        let next = nums[i + 1..].iter().flatten().next().cloned();
        let mut cand = Num::between(prev.as_ref(), next.as_ref());
        while used.contains(&cand) {
            cand = if prev.is_some() || next.is_none() { cand.next_in_base() } else { Num::new(cand.base - 1) };
        }
        used.insert(cand.clone());
        nums[i] = Some(cand);
    }
}

impl Document {
    /// Le numéro du plan : le sien en numérotation verrouillée, sinon son rang dans le film.
    pub fn plan_num(&self, scene: u64) -> Num {
        if self.numbering_locked {
            if let Some(n) = self.scene(scene).and_then(|s| Num::parse(&s.number)) {
                return n;
            }
        }
        Num::new(self.scene_number(scene) as i64)
    }

    /// Le numéro de la séquence (même règle).
    pub fn sequence_num(&self, sequence: u64) -> Num {
        if self.numbering_locked {
            if let Some(n) = self.sequences.iter().find(|s| s.id == sequence).and_then(|s| Num::parse(&s.number)) {
                return n;
            }
        }
        Num::new(self.sequence_number(sequence) as i64)
    }

    /// Le numéro du plan, en texte (« 12A ») : ce que montrent le montage, la planche, les titres
    /// et les exports quand le plan n'a pas de nom à lui.
    pub fn plan_code(&self, scene: u64) -> String {
        self.plan_num(scene).text()
    }

    pub fn sequence_code(&self, sequence: u64) -> String {
        self.sequence_num(sequence).text()
    }

    /// « PL012A » : le plan pour un nom de clip ou de fichier.
    pub fn plan_ref(&self, scene: u64) -> String {
        format!("PL{}", self.plan_num(scene).padded(3))
    }

    /// « SQ01 ».
    pub fn sequence_ref(&self, sequence: u64) -> String {
        format!("SQ{}", self.sequence_num(sequence).padded(2))
    }

    /// « SQ01_PL012A » : la séquence et le plan du panneau `index`.
    pub fn plan_full_ref(&self, index: usize) -> String {
        format!("{}_{}", self.sequence_ref(self.sequence_of(index)), self.plan_ref(self.panels[index].scene))
    }

    /// Donne un numéro aux plans et aux séquences qui n'en ont pas (numérotation verrouillée) :
    /// entre leurs voisins, dans l'ordre du film. Appelée à la fin de `normalize`.
    pub fn assign_numbers(&mut self) {
        if !self.numbering_locked {
            return;
        }
        let scenes = self.scene_order();
        let mut nums: Vec<Option<Num>> = scenes.iter().map(|&id| self.scene(id).and_then(|s| Num::parse(&s.number))).collect();
        if nums.iter().any(Option::is_none) {
            fill(&mut nums);
            for (id, n) in scenes.iter().zip(nums) {
                if let (Some(s), Some(n)) = (self.scene_mut(*id), n) {
                    if Num::parse(&s.number).is_none() {
                        s.number = n.text();
                    }
                }
            }
        }
        let seqs = self.sequence_order();
        let mut nums: Vec<Option<Num>> = seqs.iter().map(|&id| self.sequences.iter().find(|s| s.id == id).and_then(|s| Num::parse(&s.number))).collect();
        if nums.iter().any(Option::is_none) {
            fill(&mut nums);
            for (id, n) in seqs.iter().zip(nums) {
                if let (Some(s), Some(n)) = (self.sequence_mut(*id), n) {
                    if Num::parse(&s.number).is_none() {
                        s.number = n.text();
                    }
                }
            }
        }
    }

    /// Verrouille la numérotation : les numéros d'aujourd'hui (1, 2, 3… dans l'ordre du film)
    /// deviennent ceux des plans et des séquences. Rien ne change à l'écran.
    pub fn lock_numbering(&mut self) {
        self.numbering_locked = true;
        self.write_clean_numbers();
    }

    /// Repasse en numérotation suivie : les numéros propres sont oubliés, le rang dans le film
    /// redevient le numéro.
    pub fn unlock_numbering(&mut self) {
        self.numbering_locked = false;
        for s in &mut self.scenes {
            s.number.clear();
        }
        for s in &mut self.sequences {
            s.number.clear();
        }
    }

    /// Renumérote : 1, 2, 3… dans l'ordre du film, plans et séquences (numérotation verrouillée ;
    /// en numérotation suivie les numéros sont déjà ceux du rang). Renvoie le nombre de plans dont
    /// le numéro change.
    pub fn renumber(&mut self) -> usize {
        if !self.numbering_locked {
            return 0;
        }
        let before: Vec<(u64, String)> = self.scene_order().iter().map(|&id| (id, self.plan_code(id))).collect();
        self.write_clean_numbers();
        before.iter().filter(|(id, old)| self.plan_code(*id) != *old).count()
    }

    fn write_clean_numbers(&mut self) {
        for (k, id) in self.scene_order().into_iter().enumerate() {
            if let Some(s) = self.scene_mut(id) {
                s.number = Num::new(k as i64 + 1).text();
            }
        }
        for (k, id) in self.sequence_order().into_iter().enumerate() {
            if let Some(s) = self.sequence_mut(id) {
                s.number = Num::new(k as i64 + 1).text();
            }
        }
    }
}
