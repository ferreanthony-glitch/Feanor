//! Panneau Historique : le nom de chaque étape suit l'annulation et le rétablissement.

use crate::document::Document;
use crate::history::{Edit, History, Nav};
use crate::historyui::{step_name, steps_to};

fn nav() -> Nav {
    Nav { panel: 0, layer: 0 }
}

#[test]
fn each_step_keeps_its_name_through_undo_and_redo() {
    let mut doc = Document::new(64, 48, 24);
    let mut n = nav();
    let mut h = History::default();
    h.context = Some("Pinceau".into());
    h.push(Edit::Group(Vec::new()));
    h.context = Some("Retourner le plan".into());
    h.push(Edit::Group(Vec::new()));
    h.context = None;
    h.push(Edit::Vector { cel: doc.new_cel(crate::document::LayerKind::Vector), drawing: Default::default() });
    assert_eq!(h.names(), (vec!["Pinceau", "Retourner le plan", "Dessin vectoriel"], vec![]));
    assert!(h.undo(&mut doc, &mut n));
    assert!(h.undo(&mut doc, &mut n));
    assert_eq!(h.names(), (vec!["Pinceau"], vec!["Retourner le plan", "Dessin vectoriel"]), "la prochaine à rétablir d'abord");
    assert!(h.redo(&mut doc, &mut n));
    assert_eq!(h.names(), (vec!["Pinceau", "Retourner le plan"], vec!["Dessin vectoriel"]));
    // Une nouvelle modification remplace les étapes annulées.
    h.context = Some("Gomme".into());
    h.push(Edit::Group(Vec::new()));
    assert_eq!(h.names(), (vec!["Pinceau", "Retourner le plan", "Gomme"], vec![]));
    h.clear();
    assert_eq!(h.names(), (vec![], vec![]));
}

#[test]
fn names_stay_in_step_with_the_limit() {
    let mut h = History::default();
    for i in 0..250 {
        h.context = Some(format!("étape {i}"));
        h.push(Edit::Group(Vec::new()));
    }
    let (done, _) = h.names();
    assert_eq!(done.len(), 200);
    assert_eq!((done[0], done[199]), ("étape 50", "étape 249"));
}

#[test]
fn going_to_a_step() {
    assert_eq!(steps_to(5, 0, 2), -3, "revenir après la 2e : annuler 3 fois");
    assert_eq!(steps_to(2, 3, 5), 3, "rétablir jusqu'à la 5e");
    assert_eq!(steps_to(2, 3, 9), 3, "pas plus loin que la dernière annulée");
    assert_eq!(steps_to(4, 0, 4), 0);
    assert_eq!(steps_to(4, 0, 0), -4, "début de l'historique");
}

#[test]
fn a_command_names_its_step_without_its_details() {
    assert_eq!(step_name("Panneau Historique des annulations (ouvrir / fermer)"), "Panneau Historique des annulations");
    assert_eq!(step_name("Coller sur place"), "Coller sur place");
}
