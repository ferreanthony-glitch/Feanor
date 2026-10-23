//! Rendre la mémoire d'un gros projet sans que l'interface l'attende (essai de charge du 2026-10-23, `docs/PERFORMANCE.md`,
//! section 25).
//!
//! Un film de 300 plans tient environ 160 000 tuiles de 16 Ko. Le tas segmenté de Windows (manifeste de `shot.exe`,
//! `build.rs`) les rend en une à deux secondes ; le tas par défaut y mettait un quart d'heure. Même en une seconde, ce
//! n'est pas au fil de l'interface de le faire : le document remplacé (Ouvrir, Nouveau, fermer le dernier onglet) et son
//! historique, l'onglet que l'on ferme, partent sur un fil à eux (`en_fond`). À la sortie de SHOT, une fois tout écrit
//! (projets, copies de secours, réglages), les documents ne sont pas rendus du tout (`App::sortie_sans_rendre`) : le
//! système reprend la mémoire du processus d'un coup.

use std::sync::atomic::{AtomicUsize, Ordering};

/// Fils de `en_fond` pas encore finis (essais).
static EN_COURS: AtomicUsize = AtomicUsize::new(0);

/// Lâche `v` sur un fil de fond détaché, sous la priorité de l'interface : elle n'attend pas, et ce fil ne lui prend pas
/// le processeur. (Si le fil ne peut pas naître, `v` est lâché ici, comme avant.)
pub(crate) fn en_fond<T: Send + 'static>(v: T) {
    EN_COURS.fetch_add(1, Ordering::Relaxed);
    let lance = std::thread::Builder::new().name("lâcher".into()).spawn(move || {
        crate::parallele::fil_sous_la_normale();
        drop(v);
        EN_COURS.fetch_sub(1, Ordering::Relaxed);
    });
    if lance.is_err() {
        // (La fermeture, et `v` avec elle, a été lâchée par `spawn`.)
        EN_COURS.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Combien de `en_fond` travaillent encore.
#[cfg(test)]
pub(crate) fn en_cours() -> usize {
    EN_COURS.load(Ordering::Relaxed)
}

/// Attend (au plus `max`) que les `en_fond` aient fini ; vrai s'ils ont fini (essais).
#[cfg(test)]
pub(crate) fn attendre(max: std::time::Duration) -> bool {
    let t = std::time::Instant::now();
    while en_cours() > 0 {
        if t.elapsed() > max {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    true
}

/// Le tas du processus est-il le tas segmenté ? `None` hors de Windows. Lu dans l'en-tête du tas : la signature d'un tas
/// segmenté (`_SEGMENT_HEAP`) est 0xDDEEDDEE, celle du tas par défaut (`_HEAP`) 0xFFEEFFEE, au même endroit (+16 octets).
/// Structure interne de Windows, stable depuis Windows 10 : ne sert qu'à vérifier le manifeste (essais, diagnostic).
#[cfg(all(windows, target_pointer_width = "64"))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn tas_segmente() -> Option<bool> {
    extern "system" {
        fn GetProcessHeap() -> *const u32;
    }
    // SAFETY : `GetProcessHeap` rend l'adresse du tas du processus, valide toute sa vie ; les 20 premiers octets de
    // l'en-tête (les deux structures en ont bien plus) se lisent.
    let signature = unsafe {
        let tas = GetProcessHeap();
        if tas.is_null() {
            return None;
        }
        std::ptr::read_volatile(tas.add(4))
    };
    match signature {
        0xDDEE_DDEE => Some(true),
        0xFFEE_FFEE => Some(false),
        _ => None,
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn tas_segmente() -> Option<bool> {
    None
}
