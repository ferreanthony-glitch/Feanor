//! Tests de la fenêtre « À propos » : la version vient de Cargo.toml, les noms d'avant sont dits, la
//! commande est dans la liste des actions (donc dans Ctrl+K), et les polices de marque sont créditées.

use crate::aproposui::{ancien_nom_text, polices_text, version_text};

#[test]
fn version_and_old_names_are_written() {
    assert_eq!(version_text(), format!("Version {}", env!("CARGO_PKG_VERSION")));
    assert!(version_text().chars().any(|c| c.is_ascii_digit()));
    assert_eq!(ancien_nom_text(), "Anciennement SHOT, Pelure");
    assert!(polices_text().contains("Big Shoulders Display") && polices_text().contains("IBM Plex") && polices_text().contains("OFL"));
}

#[test]
fn the_command_is_listed_for_the_palette() {
    let a = crate::keymap::action_info("aide.a_propos").expect("action déclarée");
    assert!(a.implemented);
    assert!(a.label.contains("FEANOR"));
}