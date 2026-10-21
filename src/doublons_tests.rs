//! La passe « doublons » (`docs/DOUBLONS.md`) : chaque commande qui a perdu un de ses endroits reste atteignable, par la
//! palette Ctrl+K et par l'endroit principal qu'on lui a laissé ; aucune barre ne montre deux fois la même action ; les
//! panneaux qui ont leur icône n'en ont pas une seconde dans une barre de commandes.

use std::collections::BTreeSet;

use crate::barres::{dans_la_barre, dans_plus, Barre, Genre, ReglageBarre};
use crate::palette::palette_commands;

/// Les commandes qui ont quitté une barre (le transport, les plans et la pelure de la barre de commandes ou de la
/// timeline, les panneaux, les vues) : elles sont dans la palette.
const DEPLACEES: &[&str] = &[
    "image.premiere",
    "image.precedente",
    "lecture.basculer",
    "image.suivante",
    "image.derniere",
    "lecture.boucle",
    "son.lecture",
    "panneau.nouveau",
    "panneau.dupliquer",
    "panneau.supprimer",
    "vue.pelure",
    "pelure.panneau",
    "fx.panneau",
    "vue.bruitages",
    "mode.basculer",
    "camera.vue",
    "espace3d.basculer",
    "vue.planches",
    "vue.cadres_securite",
    "vue.ajuster",
    "vue.reinit_zoom",
    "vue.zoom_avant",
    "vue.zoom_arriere",
    "vue.rotation_horaire",
    "vue.rotation_antihoraire",
    "vue.reinit_rotation",
    "vue.miroir",
    "vue.table_lumineuse",
    "camera.cle_ici",
    "camera.cle_supprimer",
    "camera.cle_adoucir",
];

#[test]
fn every_command_that_lost_a_place_is_still_in_the_palette() {
    let palette: BTreeSet<&str> = palette_commands().into_iter().map(|(id, _)| id).collect();
    let mut manque = Vec::new();
    for a in DEPLACEES {
        let info = crate::keymap::action_info(a);
        assert!(info.is_some_and(|i| i.implemented), "{a} n'est pas une commande faite");
        if !palette.contains(a) {
            manque.push(*a);
        }
    }
    assert!(manque.is_empty(), "absentes de la palette Ctrl+K : {manque:?}");
}

#[test]
fn the_command_bar_has_no_transport_and_no_plan_commands_the_other_bars_carry_them() {
    let commandes: BTreeSet<&str> = Barre::Commandes.commandes().iter().map(|c| c.id).collect();
    for a in ["image.premiere", "image.precedente", "lecture.basculer", "image.suivante", "image.derniere", "lecture.boucle", "son.lecture", "panneau.nouveau", "panneau.dupliquer", "panneau.supprimer"] {
        assert!(!commandes.contains(a), "{a} ne doit plus être dans la barre de commandes");
    }
    // Le transport du plan est dans la timeline, celui du film dans le montage ; les plans dans le montage.
    for a in ["image.premiere", "image.precedente", "lecture.basculer", "image.suivante", "image.derniere", "lecture.boucle"] {
        assert!(Barre::Timeline.cherche(a).is_some(), "{a} : la timeline");
    }
    for a in ["lecture.basculer", "lecture.boucle", "montage.debut", "panneau.nouveau", "sequence.nouvelle", "son.lecture"] {
        assert!(Barre::Montage.cherche(a).is_some(), "{a} : le montage");
    }
}

#[test]
fn a_panel_with_an_icon_in_the_panel_bar_has_no_second_icon_in_a_command_bar() {
    use crate::barrepanneaux::GROUPES;
    let panneaux: BTreeSet<&str> = GROUPES.iter().flat_map(|g| g.iter()).map(|e| e.action).collect();
    for barre in Barre::TOUTES {
        for c in barre.commandes() {
            assert!(!panneaux.contains(c.action) || c.action.is_empty(), "{barre:?} : {} a déjà son icône dans la barre des panneaux", c.id);
        }
    }
}

#[test]
fn the_pelure_is_set_under_the_drawing_only_and_the_timeline_keeps_it_under_plus() {
    for id in ["vue.pelure", "pelure_avant", "pelure_apres", "pelure.panneau"] {
        let c = Barre::Timeline.cherche(id).unwrap_or_else(|| panic!("{id} reste sous « Plus… »"));
        assert!(!c.d_office && !dans_la_barre(Barre::Timeline, &ReglageBarre::default()).iter().any(|x| x.id == id), "{id} n'est pas d'office dans la timeline");
        assert!(dans_plus(Barre::Timeline, &ReglageBarre::default()).iter().any(|x| x.id == id), "{id} est sous « Plus… »");
    }
}

#[test]
fn no_action_has_two_icons_in_the_same_bar_and_every_button_has_a_tooltip_text() {
    for barre in Barre::TOUTES {
        let mut vues = BTreeSet::new();
        for c in barre.commandes().iter().filter(|c| matches!(c.genre, Genre::Bouton | Genre::Etat) && !c.action.is_empty() && c.action != "vue.planches") {
            assert!(vues.insert(c.action), "{barre:?} : {} deux fois", c.action);
            assert!(!c.nom.is_empty() && !c.description.is_empty(), "{} : bulle incomplète", c.id);
        }
    }
}

#[test]
fn the_bars_share_the_same_icon_size_case_and_gap() {
    // Les mêmes constantes pour toutes les barres (simplification des barres, 2026-10-21).
    assert_eq!(crate::barres::ICONE, 17.0);
    assert_eq!(crate::barres::CASE, [28.0, 26.0]);
    assert_eq!(crate::barres::ECART, 2.0);
    // Aucune barre n'écrit sa propre taille d'icône : plus de `size(19.0)`, `size(21.0)`, `[34.0, 30.0]`, `[38.0, 34.0]`
    // dans les fichiers des barres (les panneaux, eux, gardent leurs tailles).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for fichier in ["toolbars.rs", "barrepanneaux.rs", "viewbar.rs", "barres.rs", "legendecles.rs"] {
        let texte = std::fs::read_to_string(dir.join(fichier)).expect("source lisible");
        for interdit in ["size(19.0)", "size(21.0)", "[34.0, 30.0]", "[38.0, 34.0]", "[24.0, 26.0]", "min_size(vec2(24.0, 0.0))"] {
            assert!(!texte.contains(interdit), "{fichier} : « {interdit} » : une taille de plus");
        }
    }
}
