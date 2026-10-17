//! Journal de plantage : le rapport dit quoi et où, et n'est signalé qu'une fois.

use crate::plantage::{report_text, take_new, write_report};

#[test]
fn a_report_says_what_and_where() {
    let t = report_text("index out of bounds", "src/propager.rs:120:5", "propagation", "0: pelure::x");
    assert!(t.contains("Erreur : index out of bounds"));
    assert!(t.contains("Endroit : src/propager.rs:120:5"));
    assert!(t.contains("Fil : propagation"));
    assert!(t.contains("0: pelure::x"));
}

#[test]
fn the_next_session_sees_the_report_once() {
    let dir = std::env::temp_dir().join(format!("pelure-plantage-essai-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(take_new(&dir).is_none(), "rien avant");
    let path = write_report(&dir, "rapport").expect("écrit");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "rapport");
    assert_eq!(take_new(&dir), Some(path.clone()), "signalé à la séance suivante");
    assert!(take_new(&dir).is_none(), "une seule fois");
    assert!(path.exists(), "le rapport reste");
    let _ = std::fs::remove_dir_all(&dir);
}
