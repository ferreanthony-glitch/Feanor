//! La fin d'un plan et le bord entre deux clés d'un calque (partie 206, Anthony, 2026-11-21 : « Feanor échoue à modifier la durée
//! d'un shot sans bouger les clés d'images, ou bouger des clés d'images sans bouger les suivantes. Le déplacement de clés n'est pas
//! fluide et clair. »). Les clés d'images ne sont pas dans le plan mais sur chaque calque : les règles s'appliquent clé par clé.
//!
//! - **Fin du plan** (la poignée de durée de la timeline, la barre de durée de la fiche du plan, le bord de fin d'un plan dans le
//!   montage) : seule la DERNIÈRE clé de chaque calque s'allonge ou raccourcit, aucune autre ne bouge (`Document::poser_fin_du_plan`).
//!   Raccourcir ne coupe ni ne range aucune clé : si la nouvelle fin passe avant le début de la dernière clé d'un calque, elle
//!   s'arrête au début de cette clé + 1 image (la butée, `butee_de_fin`). **Maj** (ou le bouton « Étirer en proportion ») : toutes les
//!   clés de chaque calque s'étirent en proportion, arrondies à l'image, au moins une image chacune, la somme juste
//!   (`etirer_en_proportion`). **Ctrl** (ou « Décaler la suite ») ne change pas : les clés suivent la poignée (partie 123).
//! - **Bord entre deux clés** (timeline du Storyboard, bord droit d'un dessin dans la timeline de l'Animation) : seules les deux clés
//!   de part et d'autre changent, la coupure se déplace, rien d'autre ne bouge (`handles::move_boundary`) ; chaque clé garde au moins
//!   une image. **Ctrl** (ou « Décaler la suite ») : la clé d'avant s'allonge ou raccourcit et toute la suite du calque se décale
//!   d'autant (`handles::ripple_at`, l'ancien geste par défaut ; comme avant, le plan garde sa durée : un calque tenu jusqu'à la fin
//!   le reste, sa dernière clé absorbe). La sélection agit sur tout : les autres calques dont une clé est choisie avec la clé tenue
//!   voient leur coupure correspondante bouger du même écart, chacun avec ses butées ; les calques verrouillés sont laissés de côté.
//!   Une étape d'annulation ; la barre d'état dit « Clé 2 : 12 i (+4) » pendant le geste.
//! - **Les deux boutons du stylet** (`cles.decaler_suite`, `cles.etirer_proportion`) : à bascule, ils tiennent lieu de Ctrl et de Maj
//!   pour ces gestes tant qu'ils sont allumés. Un réglage du poste (`reglages.toml`), écrit seulement quand il est allumé.

use crate::app::App;
use crate::document::{Document, Layer, Panel};
use crate::handles::{clamp_ripple_delta, move_boundary, ripple_at, runs, KeyMove};

/// Le calque porte des clés de dessin qui suivent la fin du plan : pas un dossier, pas une vidéo, au moins un dessin.
fn a_des_cles(l: &Layer) -> bool {
    !l.folder && l.video.is_none() && l.frames.iter().any(Option::is_some)
}

/// Le début de la dernière clé dessinée du calque.
fn debut_derniere_cle(l: &Layer) -> Option<usize> {
    let last = l.frames.iter().rposition(Option::is_some)?;
    Some(l.run_bounds(last).0)
}

/// La fin la plus courte que la poignée peut donner au plan sans couper aucune clé : le début de la dernière clé de chaque calque
/// + 1 image, la plus loin d'entre elles (les calques verrouillés comptent aussi : leur dernière clé raccourcit comme les autres, mais
/// aucune n'est rangée). Et le nom du calque qui fait la butée.
pub(crate) fn butee_de_fin(p: &Panel) -> (usize, Option<String>) {
    let mut out = (1, None);
    for l in p.layers.iter().filter(|l| a_des_cles(l)) {
        if let Some(s) = debut_derniere_cle(l) {
            if s + 1 > out.0 {
                out = (s + 1, Some(l.name.clone()));
            }
        }
    }
    out
}

/// Les expositions (début, fin exclue) du calque jusqu'à l'image `old` (exclue), plages vides comprises.
fn expositions(l: &Layer, old: usize) -> Vec<(usize, usize)> {
    runs(l).into_iter().filter(|r| r.0 < old).map(|r| (r.0, r.1.min(old))).collect()
}

/// La plus courte durée qu'un étirement en proportion peut donner au plan : chaque exposition de chaque calque garde une image ; un
/// calque verrouillé (qui ne s'étire pas) garde sa dernière clé entière jusqu'à son début + 1 image.
pub(crate) fn proportion_min(p: &Panel, old: usize) -> usize {
    let mut n = 1;
    for l in p.layers.iter().filter(|l| a_des_cles(l)) {
        if l.locked {
            n = n.max(debut_derniere_cle(l).map_or(1, |s| s + 1));
        } else {
            n = n.max(expositions(l, old).len());
        }
    }
    n
}

/// Les expositions du calque `l` (jusqu'à l'image `old`, la durée du plan) étirées en proportion pour un plan de `n` images : chaque
/// limite va à son image proportionnelle (arrondie), chaque exposition garde au moins une image, un calque tenu jusqu'à la fin y
/// reste (la somme tombe juste sur `n`). Les rôles posés au début d'une clé la suivent. `n` doit valoir au moins le nombre
/// d'expositions (`proportion_min`).
pub(crate) fn etirer_en_proportion(l: &mut Layer, old: usize, n: usize) {
    let ex = expositions(l, old);
    if ex.is_empty() || old == 0 || n == old {
        return;
    }
    let len = ex.last().map_or(0, |e| e.1);
    let k = ex.len();
    let scale = |f: usize| ((f as u64 * n as u64 + old as u64 / 2) / old as u64) as usize;
    let mut ends: Vec<usize> = ex.iter().map(|e| scale(e.1)).collect();
    // Chaque exposition garde au moins une image…
    let mut prev = 0;
    for e in ends.iter_mut() {
        *e = (*e).max(prev + 1);
        prev = *e;
    }
    // … la dernière finit à la fin du plan si le calque y était tenu, jamais après…
    ends[k - 1] = if len >= old { n } else { ends[k - 1].min(n) };
    // … et, de la fin vers le début, chacune laisse une image à la suivante.
    for i in (0..k - 1).rev() {
        ends[i] = ends[i].min(ends[i + 1] - 1);
    }
    let mut frames = Vec::with_capacity(ends[k - 1]);
    let mut starts = Vec::with_capacity(k);
    for (i, &(s, _)) in ex.iter().enumerate() {
        let a = if i == 0 { 0 } else { ends[i - 1] };
        starts.push(a);
        frames.resize(ends[i], l.frames[s]);
    }
    let old_roles = std::mem::take(&mut l.roles);
    for (f, r) in old_roles {
        let Some(i) = ex.iter().position(|e| f >= e.0 && f < e.1) else { continue };
        let at = if f == ex[i].0 { starts[i] } else { scale(f).clamp(starts[i], ends[i] - 1) };
        l.roles.insert(at, r);
    }
    l.frames = frames;
    l.trim();
}

/// Ce qu'a fait `poser_fin_du_plan`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FinPosee {
    /// La durée du plan après le geste.
    pub duree: usize,
    /// La poignée a été arrêtée : (la durée la plus courte permise, le calque qui la fixe).
    pub butee: Option<(usize, Option<String>)>,
}

impl Document {
    /// La fin du plan `pi` amenée à `n` images (règle 1 de la partie 206). Sans `proportion` : seule la dernière clé de chaque calque
    /// change (allonger tient le dernier dessin de chaque calque jusqu'à la nouvelle fin, `clesduree::etirer_fins` ; raccourcir coupe
    /// la fin, ce qui sort va dans la réserve comme avant, mais jamais avant le début de la dernière clé d'un calque + 1 image).
    /// `proportion` (Maj) : toutes les clés de chaque calque non verrouillé s'étirent en proportion ; raccourcir range dans la réserve
    /// ce qui sort encore (les clés de caméra, d'effets… d'après la nouvelle fin), comme avant.
    pub(crate) fn poser_fin_du_plan(&mut self, pi: usize, n: usize, proportion: bool) -> FinPosee {
        let Some(old) = self.panels.get(pi).map(Panel::frame_count) else { return FinPosee::default() };
        let voulu = n.clamp(1, crate::rognage::MAX_PLAN);
        if proportion {
            let min = proportion_min(&self.panels[pi], old);
            let n = voulu.max(min);
            let butee = (voulu < min).then(|| (min, None));
            if n != old {
                for l in self.panels[pi].layers.iter_mut().filter(|l| a_des_cles(l) && !l.locked) {
                    etirer_en_proportion(l, old, n);
                }
                if n < old {
                    self.set_duration(pi, n);
                } else {
                    self.panels[pi].length = n;
                }
            }
            return FinPosee { duree: self.panels[pi].frame_count(), butee };
        }
        if voulu >= old {
            self.set_duration(pi, voulu);
            let fin = self.panels[pi].frame_count();
            if fin > old {
                crate::clesduree::etirer_fins(&mut self.panels[pi], fin);
            }
            return FinPosee { duree: fin, butee: None };
        }
        let (min, qui) = butee_de_fin(&self.panels[pi]);
        let n = voulu.max(min);
        if n < old {
            self.set_duration(pi, n);
        }
        FinPosee { duree: self.panels[pi].frame_count(), butee: (voulu < min).then_some((min, qui)) }
    }
}

/// La coupure à l'image `at` du calque `l` déplacée de `want` images : sans `decaler`, seules les deux expositions voisines changent
/// (`move_boundary`, chaque dessin garde une image) ; avec `decaler`, ou s'il n'y a rien après (le bord droit de la dernière clé),
/// l'exposition d'avant change et la suite se décale (`ripple_at`, dans le plan de `plan_len` images). Renvoie l'écart fait.
pub(crate) fn deplacer_coupe(l: &mut Layer, at: usize, want: isize, decaler: bool, plan_len: usize) -> isize {
    let coupe = at > 0 && at < l.frames.len() && l.frames[at - 1] != l.frames[at];
    if !decaler && coupe {
        let to = (at as isize + want).max(0) as usize;
        move_boundary(l, at, to) as isize - at as isize
    } else {
        let d = clamp_ripple_delta(l, at, want, plan_len);
        ripple_at(l, at, d, plan_len);
        d
    }
}

/// « Clé 2 : 12 i (+4) » : la clé dessinée juste avant la coupure `at` du calque `l` (ou, si c'est une plage vide, celle juste après),
/// son rang sur le calque, sa durée et ce qu'elle a gagné.
pub(crate) fn texte_de_cle(l: &Layer, at: usize, d: isize) -> String {
    let rs = runs(l);
    let rang = |s: usize| rs.iter().filter(|r| r.2.is_some() && r.0 <= s).count();
    if at > 0 && l.frames.get(at - 1).copied().flatten().is_some() {
        let s = l.run_bounds(at - 1).0;
        return format!("Clé {} : {} i ({d:+})", rang(s), at - s);
    }
    if l.frames.get(at).copied().flatten().is_some() {
        let (s, e) = l.run_bounds(at);
        return format!("Clé {} : {} i ({:+})", rang(s), e - s, -d);
    }
    format!("Coupure à l'image {at} ({d:+})")
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

impl App {
    /// Ctrl tenue, ou le bouton « Décaler la suite » allumé : le bord d'une clé décale la suite, la fin du plan fait suivre les clés.
    pub(crate) fn decaler_la_suite(&self) -> bool {
        self.settings.cles_decaler_suite || crate::handles::keys_follow(&self.ctx.input(|i| i.modifiers))
    }

    /// Maj tenue, ou le bouton « Étirer en proportion » allumé : la fin du plan étire toutes les clés en proportion.
    pub(crate) fn etirer_la_proportion(&self) -> bool {
        self.settings.cles_etirer_proportion || self.ctx.input(|i| i.modifiers.shift)
    }

    /// Bascule le bouton « Décaler la suite » (il éteint « Étirer en proportion » : un seul geste à la fois). Écrit le réglage.
    pub(crate) fn basculer_decaler_suite(&mut self) {
        self.settings.cles_decaler_suite = !self.settings.cles_decaler_suite;
        if self.settings.cles_decaler_suite {
            self.settings.cles_etirer_proportion = false;
        }
        let _ = self.settings.save();
        self.status = if self.settings.cles_decaler_suite {
            "Décaler la suite allumé : le bord d'une clé décale toute la suite du calque ; la fin du plan fait suivre les clés (comme Ctrl tenue).".into()
        } else {
            "Décaler la suite éteint : le bord d'une clé ne change que ses deux voisines.".into()
        };
    }

    /// Bascule le bouton « Étirer en proportion » (il éteint « Décaler la suite »). Écrit le réglage.
    pub(crate) fn basculer_etirer_proportion(&mut self) {
        self.settings.cles_etirer_proportion = !self.settings.cles_etirer_proportion;
        if self.settings.cles_etirer_proportion {
            self.settings.cles_decaler_suite = false;
        }
        let _ = self.settings.save();
        self.status = if self.settings.cles_etirer_proportion {
            "Étirer en proportion allumé : tirer la fin d'un plan étire toutes ses clés en proportion (comme Maj tenue).".into()
        } else {
            "Étirer en proportion éteint : tirer la fin d'un plan ne change que la dernière clé de chaque calque.".into()
        };
    }

    /// Les coupures que déplace le bord tenu à l'image `at` du calque `layer` du plan `p` (l'état d'avant le geste) : la sienne, puis,
    /// si la clé tenue (celle d'avant la coupure ou celle d'après) est choisie, une par autre calque dont une clé est choisie : la fin
    /// (ou, si la clé tenue est celle d'après, le début) de sa clé choisie la plus proche de `at`. (calque, coupure).
    pub(crate) fn coupes_du_geste(&self, p: &Panel, layer: usize, at: usize) -> Vec<(usize, usize)> {
        let mut out = vec![(layer, at)];
        let Some(l) = p.layers.get(layer) else { return out };
        let choisies: Vec<crate::montagekeys::KeyRef> = self.cles_visees().into_iter().filter(|k| k.panel == p.id).collect();
        let avant = (at > 0).then(|| l.run_bounds(at - 1).0);
        let tenue_avant = avant.is_some_and(|s| choisies.iter().any(|k| k.layer == l.id && k.start == s));
        let tenue_apres = choisies.iter().any(|k| k.layer == l.id && k.start == at);
        if !tenue_avant && !tenue_apres {
            return out;
        }
        for (li, o) in p.layers.iter().enumerate() {
            if li == layer || o.folder || o.video.is_some() {
                continue;
            }
            let mut best: Option<usize> = None;
            for k in choisies.iter().filter(|k| k.layer == o.id) {
                let c = if tenue_avant { o.run_bounds(k.start).1 } else { k.start };
                if c == 0 || c >= o.frames.len() || o.frames[c - 1] == o.frames[c] {
                    continue;
                }
                if best.is_none_or(|b| c.abs_diff(at) < b.abs_diff(at)) {
                    best = Some(c);
                }
            }
            if let Some(c) = best {
                out.push((li, c));
            }
        }
        out
    }

    /// Le bord tenu (la coupure à l'image `at` du calque `layer`) amené à l'image `to` : chaque coupure du geste repart de l'état d'avant
    /// le geste et bouge du même écart (règle 2 de la partie 206). La phrase de la barre d'état.
    pub(crate) fn tirer_bord_de_cle(&mut self, pi: usize, before: &Panel, coupes: &[(usize, usize)], at: usize, to: usize) -> String {
        let plan_len = before.frame_count();
        let decaler = self.decaler_la_suite();
        self.key_drag_mode = if decaler { KeyMove::Ripple } else { KeyMove::Single };
        let want = to as isize - at as isize;
        let mut laisses: Vec<String> = Vec::new();
        let mut bouges = 0usize;
        let mut phrase = String::new();
        for (k, &(li, a)) in coupes.iter().enumerate() {
            let Some(mut l) = before.layers.get(li).cloned() else { continue };
            if l.locked {
                laisses.push(l.name.clone());
                continue;
            }
            let d = deplacer_coupe(&mut l, a, want, decaler, plan_len);
            if k == 0 {
                phrase = texte_de_cle(&l, (a as isize + d) as usize, d);
                // (Partie 208 : le bord tenu reste allumé pendant tout le geste, à sa place de l'instant.)
                if let Some(dr) = self.handle_drag.as_mut() {
                    dr.poser_bord(li, (a as isize + d) as usize);
                }
            }
            bouges += usize::from(d != 0);
            if let Some(slot) = self.doc.panels[pi].layers.get_mut(li) {
                *slot = l;
            }
        }
        let p = &mut self.doc.panels[pi];
        if p.frame_count() < plan_len {
            p.length = plan_len;
        }
        // (Les mêmes dessins à d'autres images : ce que l'image courante montre est dans le plan à composer.)
        self.renderer.invalidate_params();
        if phrase.is_empty() {
            phrase = "Calque verrouillé : sa clé ne bouge pas".into();
        }
        if coupes.len() > 1 {
            phrase += &format!(" · {bouges} calque{} sur {}", s(bouges), coupes.len());
        }
        phrase += if decaler {
            " · la suite du calque se décale (Ctrl ou « Décaler la suite »)"
        } else {
            " · seules les deux clés voisines changent (Ctrl : décaler la suite)"
        };
        if !laisses.is_empty() {
            phrase += &format!(" · laissé{} de côté (verrouillé{}) : {}", s(laisses.len()), s(laisses.len()), laisses.iter().map(|v| format!("« {v} »")).collect::<Vec<_>>().join(", "));
        }
        phrase + "."
    }
}
