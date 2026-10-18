//! Tests de la fenêtre « À propos » : la version vient de Cargo.toml, l'ancien nom est dit, la
//! commande est dans la liste des actions (donc dans Ctrl+K), et le logo suit le thème.

use crate::aproposui::{ancien_nom_text, version_text};
use crate::theme::{self, ChoixTheme, ReglageTheme};

#[test]
fn version_and_old_name_are_written() {
    assert_eq!(version_text(), format!("Version {}", env!("CARGO_PKG_VERSION")));
    assert!(version_text().chars().any(|c| c.is_ascii_digit()));
    assert_eq!(ancien_nom_text(), "Anciennement Pelure");
}

#[test]
fn the_command_is_listed_for_the_palette() {
    let a = crate::keymap::action_info("aide.a_propos").expect("action déclarée");
    assert!(a.implemented);
    assert!(a.label.contains("SHOT"));
}

#[test]
fn the_logo_follows_the_panel_background() {
    let mut r = ReglageTheme::default();
    theme::installer(&r);
    assert!(crate::accueil::logo_sombre(), "Gris sombre : logo des fonds sombres");
    r.choisir(ChoixTheme::GrisClair);
    theme::installer(&r);
    assert!(!crate::accueil::logo_sombre(), "Gris clair : logo des fonds clairs");
    r.choisir(ChoixTheme::Sombre);
    theme::installer(&r);
    assert!(crate::accueil::logo_sombre());
    theme::installer(&ReglageTheme::default());
}
