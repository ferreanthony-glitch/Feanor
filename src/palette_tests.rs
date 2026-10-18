//! Palette de commandes : on trouve une commande par quelques lettres de son nom.

use crate::palette::{fold, rank};

const COMMANDS: [(&str, &str); 5] = [
    ("outil.perspective", "Outil perspective"),
    ("calque.miroir_h", "Retourner le calque de gauche à droite"),
    ("plan.retourner", "Retourner les plans choisis de gauche à droite"),
    ("calque.flou", "Flou sur le calque (effet Flou gaussien, cocher / décocher)"),
    ("calque.propager", "Propager la peinture du calque"),
];

#[test]
fn accents_and_case_do_not_matter() {
    assert_eq!(fold("Éclaircir À Côté"), "eclaircir a cote");
}

#[test]
fn a_few_letters_find_the_command() {
    assert_eq!(rank(&COMMANDS, "persp"), vec![0]);
    assert_eq!(rank(&COMMANDS, "propag"), vec![4]);
    // Les deux retournements : celui dont la commande s'appelle « retourner » d'abord.
    assert_eq!(rank(&COMMANDS, "retourner"), vec![2, 1]);
    // « persp » : l'outil (outil.perspective) avant le guide de perspective, au nom plus court.
    let both = [("guide.activer", "Guide de perspective : afficher et utiliser"), ("outil.perspective", "Outil perspective (déformer par les quatre coins)")];
    assert_eq!(rank(&both, "persp"), vec![1, 0]);
    // Plusieurs mots : tous doivent y être.
    assert_eq!(rank(&COMMANDS, "retourner plans"), vec![2]);
    // L'identifiant compte aussi.
    assert_eq!(rank(&COMMANDS, "flou"), vec![3]);
    assert_eq!(rank(&COMMANDS, "miroir"), vec![1]);
    assert!(rank(&COMMANDS, "").is_empty());
    assert!(rank(&COMMANDS, "zzz").is_empty());
}