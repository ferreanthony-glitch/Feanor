//! « Nouvelle séquence avec les plans choisis » (Anthony, 2026-10-28) : plusieurs plans choisis dans le montage (ou la planche)
//! forment une séquence neuve, dans l'ordre du film.
//!
//! - Contigus, ils restent où ils sont ; sinon ils se rejoignent à la place du premier choisi. Les autres plans ne bougent pas
//!   d'une séquence à l'autre : les plans choisis quittent la leur, rien de plus.
//! - Une séquence que la nouvelle coupe en deux (les plans choisis étaient en son milieu) devient deux séquences : la première
//!   garde son identité (son nom, son numéro, ses variantes), la suite reçoit une séquence à elle, de même nom, sans numéro
//!   propre (en numérotation verrouillée elle en reçoit un entre ses voisines, `Document::assign_numbers`).
//! - Chaque plan garde sa scène, donc son nom et son numéro propre. Rien d'autre ne change : ni images, ni calques, ni son.
//! - Une seule étape d'annulation (`App::structure_op_dessins`) ; refusé sous le verrou du storyboard.
//! - Un plan qui a d'autres panneaux dans sa scène (fichiers d'avant les plans) les emmène : une scène ne se partage pas.

use std::collections::{HashMap, HashSet};

use crate::app::App;
use crate::document::Document;

impl Document {
    /// Met les plans `ids` dans une séquence neuve (voir le haut de ce fichier). Renvoie l'indice du premier plan choisi, ou rien
    /// (et rien n'a bougé) si aucun n'existe ou si ces plans forment déjà, tels quels, une séquence entière.
    pub fn new_sequence_from_plans(&mut self, ids: &[u64]) -> Option<usize> {
        // Les panneaux visés, avec tous ceux de leur scène.
        let mut at: Vec<usize> = Vec::new();
        for id in ids {
            if let Some(i) = self.panels.iter().position(|p| p.id == *id) {
                let (a, b) = self.scene_run(i);
                at.extend(a..b);
            }
        }
        at.sort_unstable();
        at.dedup();
        let &first = at.first()?;
        // Déjà une séquence entière, ni plus ni moins : rien à faire (la « nouvelle » séquence ferait disparaître l'ancienne, nom compris).
        let (a, b) = self.sequence_run(first);
        if at.len() == b - a && at[0] == a && at.windows(2).all(|w| w[1] == w[0] + 1) {
            return None;
        }
        let first_id = self.panels[first].id;
        let chosen: HashSet<u64> = at.iter().map(|&i| self.panels[i].id).collect();
        // Les plans choisis qui seraient les derniers d'une séquence qui a d'autres versions de séquence : elles disparaîtraient.
        if self.refuser_si_sequence_videe(|p| chosen.contains(&p)) {
            return None;
        }
        let seq = self.new_sequence();
        // Les plans choisis se suivent, à la place du premier.
        let (moved, mut kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.panels).into_iter().partition(|p| chosen.contains(&p.id));
        let moved_scenes: Vec<u64> = moved.iter().map(|p| p.scene).collect();
        kept.splice(first..first, moved);
        self.panels = kept;
        for s in &moved_scenes {
            if let Some(sc) = self.scene_mut(*s) {
                sc.sequence = seq;
            }
        }
        // Une séquence coupée en deux par la nouvelle : la suite reçoit une séquence à elle (la première garde la sienne).
        let order: Vec<(u64, u64)> = self.panels.iter().map(|p| (p.scene, self.scene(p.scene).map_or(0, |s| s.sequence))).collect();
        let mut seen: HashSet<u64> = HashSet::new();
        let mut run: Option<(u64, u64)> = None; // (séquence d'origine de la série en cours, séquence qu'elle a maintenant)
        for (scene, s) in order {
            let now = match run {
                Some((orig, now)) if orig == s => now,
                _ => {
                    let now = if seen.contains(&s) {
                        let name = self.sequences.iter().find(|q| q.id == s).map(|q| q.name.clone()).unwrap_or_default();
                        let neuve = self.new_sequence();
                        if let Some(q) = self.sequence_mut(neuve) {
                            q.name = name;
                        }
                        neuve
                    } else {
                        s
                    };
                    seen.insert(now);
                    run = Some((s, now));
                    now
                }
            };
            if now != s {
                if let Some(sc) = self.scene_mut(scene) {
                    sc.sequence = now;
                }
            }
        }
        Some(self.normalize_keeping(first_id))
    }
}

impl App {
    /// « Nouvelle séquence avec les plans choisis » : une étape d'annulation. Rien si aucun plan n'est choisi en plus du courant.
    pub(crate) fn new_sequence_from_chosen(&mut self) {
        self.new_sequence_from_ids(2);
    }

    /// « Nouvelle séquence avec les cases choisies » (les vignettes) : comme `new_sequence_from_chosen`, mais un seul plan (celui de la case
    /// courante) suffit : il quitte sa séquence et en forme une neuve (Anthony, 2026-10-30).
    pub(crate) fn new_sequence_from_cases(&mut self) {
        self.new_sequence_from_ids(1);
    }

    fn new_sequence_from_ids(&mut self, au_moins: usize) {
        let ids = self.board_selection();
        if ids.len() < au_moins {
            self.status = "Choisis au moins deux plans (Ctrl ou Maj + clic dans le montage) pour en faire une séquence.".into();
            return;
        }
        if self.storyboard_refuses() || !self.partage_peut_toucher(&ids) {
            return;
        }
        let avant: Vec<u64> = self.doc.sequence_order();
        let nom_avant: HashMap<u64, String> = avant.iter().map(|q| (*q, self.doc.sequence_label(*q))).collect();
        if self.structure_op_dessins(|d, _| d.new_sequence_from_plans(&ids)) {
            self.board_sel = ids.clone();
            let seq = self.doc.sequence_of(self.cur_panel);
            let mut msg = if ids.len() == 1 { format!("Ce plan forme la séquence {}.", self.doc.sequence_label(seq)) } else { format!("{} plans forment la séquence {}.", ids.len(), self.doc.sequence_label(seq)) };
            // Une séquence qui n'a plus aucun plan disparaît (comme en déplaçant ses derniers plans) : on le dit, Ctrl+Z la rend.
            let perdues: Vec<String> = avant.iter().filter(|q| !self.doc.sequences.iter().any(|s| s.id == **q)).filter_map(|q| nom_avant.get(q).cloned()).collect();
            if !perdues.is_empty() {
                msg += &format!(" Séquence vidée, retirée : {} (Ctrl+Z la rend).", perdues.join(", "));
            }
            self.status = msg;
        } else {
            self.status = if ids.len() == 1 { "Ce plan est déjà seul dans sa séquence : rien à changer.".into() } else { "Ces plans forment déjà une séquence entière : rien à changer.".into() };
        }
    }
}
