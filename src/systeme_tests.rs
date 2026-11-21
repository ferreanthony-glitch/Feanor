//! Les programmes du système : montrer un fichier, ouvrir un texte, sur Windows, Mac et ailleurs.

use std::path::Path;
use crate::systeme::{commande_editer, commande_montrer, Systeme};

#[test]
fn a_file_is_shown_in_the_file_manager_of_each_system() {
    let fichier = Path::new("/Films/Bus/Bus.pdf");
    // Mac : `open -R` choisit le fichier dans le Finder ; un dossier s'ouvre tel quel.
    assert_eq!(commande_montrer(Systeme::Mac, fichier, true), ("open", vec!["-R".to_string(), "/Films/Bus/Bus.pdf".to_string()]));
    assert_eq!(commande_montrer(Systeme::Mac, Path::new("/Films/Bus"), false), ("open", vec!["/Films/Bus".to_string()]));
    // Windows : comme avant, `/select,` pour un fichier.
    let (programme, arguments) = commande_montrer(Systeme::Windows, Path::new("C:\\Films\\Bus.pdf"), true);
    assert_eq!(programme, "explorer.exe");
    assert_eq!(arguments, vec!["/select,\"C:\\Films\\Bus.pdf\"".to_string()]);
    // Ailleurs : le dossier du fichier.
    assert_eq!(commande_montrer(Systeme::Autre, fichier, true), ("xdg-open", vec!["/Films/Bus".to_string()]));
}

#[test]
fn a_text_file_opens_in_the_editor_of_each_system() {
    let f = Path::new("/reglages/keymap.toml");
    assert_eq!(commande_editer(Systeme::Windows, f).0, "notepad.exe");
    let (programme, arguments, nom) = commande_editer(Systeme::Mac, f);
    assert_eq!((programme, arguments), ("open", vec!["-t".to_string(), "/reglages/keymap.toml".to_string()]));
    assert!(!nom.contains("Bloc-notes"));
}
