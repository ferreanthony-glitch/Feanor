//! Le démarrage de SHOT, minuté (`docs/FIABILITE.md`, « Démarrage »).
//!
//! La fenêtre de SHOT reste cachée tant que la première image n'est pas peinte (eframe : « Start hidden
//! until we render the first frame »). Tout ce qui s'exécute avant — le constructeur de l'application
//! (`App::new`), ce qui est demandé au lancement — se paie donc en fenêtre qui n'apparaît pas, puis en
//! fenêtre noire : sur une machine chargée (compilations en parallèle), une seconde de calcul devient dix.
//! Chaque étape de `App::new` laisse ici une marque, et la première interface construite en laisse une
//! dernière ; le rapport dit combien chaque étape a coûté.
//!
//! Avec la variable d'environnement `SHOT_DEMARRAGE` définie, le rapport est ajouté à
//! `%LOCALAPPDATA%\SHOT\demarrage.log` (SHOT n'a pas de console : c'est le seul moyen de le lire).
//! Les marques sont par fil : le constructeur et la première image s'exécutent sur le même fil de
//! l'interface, et les essais (un fil chacun) ne se mélangent pas.

use std::cell::RefCell;
use std::time::Instant;

struct Minuterie {
    debut: Instant,
    marques: Vec<(&'static str, Instant)>,
    premiere_image: bool,
}

thread_local! {
    static MINUTERIE: RefCell<Option<Minuterie>> = const { RefCell::new(None) };
}

/// Début du démarrage (le premier appel compte ; les suivants ne font rien tant que la première image
/// n'est pas faite).
pub fn debut() {
    MINUTERIE.with(|m| {
        let mut m = m.borrow_mut();
        if m.is_none() {
            *m = Some(Minuterie { debut: Instant::now(), marques: Vec::new(), premiere_image: false });
        }
    });
}

/// Une étape vient de finir.
pub fn marque(nom: &'static str) {
    MINUTERIE.with(|m| {
        if let Some(m) = m.borrow_mut().as_mut() {
            if !m.premiere_image {
                m.marques.push((nom, Instant::now()));
            }
        }
    });
}

/// Le rapport : une ligne par étape (durée de l'étape, temps depuis le début).
pub fn rapport() -> String {
    MINUTERIE.with(|m| {
        let m = m.borrow();
        let Some(m) = m.as_ref() else { return "démarrage non minuté".to_string() };
        let mut avant = m.debut;
        let mut s = String::new();
        for (nom, t) in &m.marques {
            s += &format!("  {nom:<46} {:>8.1} ms   (à {:>8.1} ms)\n", t.duration_since(avant).as_secs_f64() * 1e3, t.duration_since(m.debut).as_secs_f64() * 1e3);
            avant = *t;
        }
        s
    })
}

/// Temps écoulé entre le début et la dernière marque, en millisecondes.
pub fn duree_ms() -> f64 {
    MINUTERIE.with(|m| m.borrow().as_ref().and_then(|m| m.marques.last().map(|(_, t)| t.duration_since(m.debut).as_secs_f64() * 1e3)).unwrap_or(0.0))
}

/// L'interface de la première image vient d'être construite : dernière marque, et le rapport est écrit
/// si `SHOT_DEMARRAGE` est défini. Ne fait rien ensuite.
pub fn premiere_image() {
    let fait = MINUTERIE.with(|m| match m.borrow_mut().as_mut() {
        Some(m) if !m.premiere_image => {
            m.marques.push(("première interface construite", Instant::now()));
            m.premiere_image = true;
            false
        }
        _ => true,
    });
    if fait {
        return;
    }
    if std::env::var_os("SHOT_DEMARRAGE").is_some() {
        if let Some(dir) = crate::donnees::local() {
            let _ = std::fs::create_dir_all(&dir);
            let texte = format!("--- démarrage de SHOT ({} s depuis 1970) : {:.0} ms jusqu'à la première interface\n{}", crate::recovery::now_secs(), duree_ms(), rapport());
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("demarrage.log")) {
                let _ = f.write_all(texte.as_bytes());
            }
        }
    }
}
