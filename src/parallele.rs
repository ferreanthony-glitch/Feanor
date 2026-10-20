//! Travail réparti sur les cœurs du processeur, avec des fils de la bibliothèque standard
//! (`std::thread::scope` ; rayon n'est pas dans les dépendances).
//!
//! `par_map` rend les résultats dans l'ordre des éléments, comme `map` : le résultat est le même
//! qu'en une seule boucle, seul le temps change. Les éléments sont distribués un à un (un fil libre
//! prend le suivant), ce qui convient à des travaux longs et inégaux : un dessin à lire ou à
//! écrire, un trait à rejouer.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Plafond de fils de calcul choisi dans les Préférences ou par le diagnostic des performances
/// (0 : tous les cœurs).
static PLAFOND: AtomicUsize = AtomicUsize::new(0);

pub fn set_plafond(fils: usize) {
    PLAFOND.store(fils, Ordering::Relaxed);
}

/// Les cœurs dont les calculs répartis disposent : ceux du processeur, au plus le plafond.
pub fn coeurs() -> usize {
    coeurs_avec(std::thread::available_parallelism().map_or(4, |c| c.get()), PLAFOND.load(Ordering::Relaxed))
}

/// `coeurs` pour `total` cœurs et un plafond `plafond` (0 : aucun).
pub fn coeurs_avec(total: usize, plafond: usize) -> usize {
    if plafond == 0 {
        total.max(1)
    } else {
        total.min(plafond).max(1)
    }
}

#[cfg(test)]
thread_local! {
    /// Essais : tout le travail de ce fil se fait en une seule boucle (le chemin « séquentiel »
    /// de l'enregistrement et de la lecture, celui d'une machine à un seul cœur).
    pub static EN_SERIE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// ---------------------------------------------------------------------------- priorité des fils

thread_local! {
    /// Ce fil de fond travaille « en dessous » de l'interface (export) : les fils qu'il lance pour répartir
    /// son travail (`render::par_chunks`, `par_map`) en font autant.
    static PRIORITE_BASSE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(windows)]
mod os {
    extern "system" {
        fn GetCurrentThread() -> isize;
        fn SetThreadPriority(thread: isize, priority: i32) -> i32;
    }

    /// THREAD_PRIORITY_BELOW_NORMAL (-1) pour le fil appelant : l'interface, au niveau normal, passe avant.
    pub fn sous_la_normale() {
        // SAFETY : deux appels Win32 sans pointeur ; `GetCurrentThread` rend un pseudo-descripteur du fil appelant.
        unsafe {
            SetThreadPriority(GetCurrentThread(), -1);
        }
    }
}

#[cfg(not(windows))]
mod os {
    pub fn sous_la_normale() {}
}

/// Ce fil (et les fils qu'il lance pour répartir son travail) passe sous la priorité normale : un export
/// ne prend le processeur à l'interface que si elle n'en a pas besoin.
pub fn baisser_la_priorite_ici() {
    PRIORITE_BASSE.with(|p| p.set(true));
    os::sous_la_normale();
}

/// La priorité basse est-elle demandée pour ce fil ? (À lire avant de lancer des fils.)
pub(crate) fn priorite_basse() -> bool {
    PRIORITE_BASSE.with(std::cell::Cell::get)
}

/// En tête d'un fil lancé par un fil de priorité basse (`basse` lu avant de le lancer) : il l'est aussi.
pub(crate) fn heriter_la_priorite(basse: bool) {
    if basse {
        PRIORITE_BASSE.with(|p| p.set(true));
        os::sous_la_normale();
    }
}

/// Nombre de fils pour `n` travaux, au plus `max` (et jamais plus que de cœurs).
fn fils(n: usize, max: usize) -> usize {
    #[cfg(test)]
    if EN_SERIE.with(|s| s.get()) {
        return 1;
    }
    coeurs().clamp(1, max.max(1)).min(n.max(1))
}

/// `f(rang, élément)` pour chaque élément de `items`, sur au plus `max` fils ; les résultats dans
/// l'ordre de `items`. Un travail qui panique fait paniquer l'appel (comme dans une boucle).
pub fn par_map<T: Sync, R: Send>(items: &[T], max: usize, f: impl Fn(usize, &T) -> R + Sync) -> Vec<R> {
    let threads = fils(items.len(), max);
    if threads <= 1 {
        return items.iter().enumerate().map(|(i, x)| f(i, x)).collect();
    }
    let next = AtomicUsize::new(0);
    let (next, f) = (&next, &f);
    let basse = priorite_basse();
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(move || {
                    heriter_la_priorite(basse);
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(x) = items.get(i) else { break };
                        mine.push((i, f(i, x)));
                    }
                    mine
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    });
    done.sort_unstable_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}
