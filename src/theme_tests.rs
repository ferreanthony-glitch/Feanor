//! Tests du thème (12a) : les textes restent lisibles sur leurs fonds, l'accent marque la
//! sélection, la police de Windows passe devant celles d'egui sans les remplacer.

use eframe::egui::{Color32, FontDefinitions, FontFamily};

use crate::theme::{self, contraste};

#[test]
fn contrast_is_computed_like_wcag() {
    assert!((contraste(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    assert!((contraste(theme::PANNEAU, theme::PANNEAU) - 1.0).abs() < 1e-4);
}

#[test]
fn texts_stay_readable_on_their_backgrounds() {
    // Texte courant : au moins 7:1 (niveau AAA) sur les panneaux, les barres et les champs.
    for fond in [theme::PANNEAU, theme::ENTETE, theme::CHAMP, theme::FOND, theme::ELEVE] {
        assert!(contraste(theme::TEXTE, fond) >= 7.0, "{fond:?} : {}", contraste(theme::TEXTE, fond));
    }
    // Texte secondaire : au moins 4,5:1 (AA) ; sur un bouton aussi.
    for fond in [theme::PANNEAU, theme::ENTETE] {
        assert!(contraste(theme::TEXTE_DOUX, fond) >= 4.5);
    }
    assert!(contraste(theme::TEXTE, theme::CONTROLE) >= 7.0);
    // Texte sur l'accent (éléments choisis), lien bleu sur le panneau.
    assert!(contraste(Color32::WHITE, theme::ACCENT) >= 4.5);
    assert_eq!(theme::visuels().selection.stroke.color, Color32::WHITE);
    assert!(contraste(theme::ACCENT_TEXTE, theme::PANNEAU) >= 4.5);
}

#[test]
fn timeline_and_montage_stay_readable() {
    // Noms écrits sur les extraits son, les calques sous les panneaux, le « M » d'une piste muette,
    // les barres de séquence et de scène : au moins 4,5:1.
    for fond in [theme::SON_EXTRAIT, theme::SON_CHOISI, theme::CALQUE_VECTORIEL, theme::CALQUE_BITMAP] {
        assert!(contraste(theme::TEXTE_FORT, fond) >= 4.5, "{fond:?} : {}", contraste(theme::TEXTE_FORT, fond));
    }
    assert!(contraste(Color32::WHITE, theme::SON_MUET) >= 4.5);
    for (sequence, impaire) in [(true, false), (true, true), (false, false), (false, true)] {
        assert!(contraste(theme::TEXTE, theme::barre(sequence, impaire)) >= 7.0);
    }
    // Repères dessinés sur les rangées (dessins exposés, image courante, clés de caméra, forme
    // d'onde) : au moins 3:1, le minimum pour un élément graphique.
    for fond in [theme::RANGEE, theme::RANGEE_SOMBRE] {
        for marque in [theme::DESSIN_VECTORIEL, theme::DESSIN_BITMAP, theme::DESSIN_VIDE, theme::IMAGE_COURANTE, theme::CAMERA, theme::MARQUE] {
            assert!(contraste(marque, fond) >= 3.0, "{marque:?} sur {fond:?} : {}", contraste(marque, fond));
        }
    }
    assert!(contraste(theme::SON_ONDE, theme::SON_EXTRAIT) >= 3.0);
}

#[test]
fn bands_alternate_so_limits_show() {
    for sequence in [true, false] {
        assert_ne!(theme::barre(sequence, false), theme::barre(sequence, true));
    }
    // Les séquences, au-dessus, sont plus sombres que les scènes.
    assert!(theme::barre(true, false).r() < theme::barre(false, false).r());
}

#[test]
fn the_visuals_use_the_palette() {
    let v = theme::visuels();
    assert!(v.dark_mode);
    assert_eq!(v.panel_fill, theme::PANNEAU);
    assert_eq!(v.selection.bg_fill, theme::ACCENT);
    assert_eq!(v.widgets.noninteractive.fg_stroke.color, theme::TEXTE);
    assert_eq!(v.widgets.inactive.weak_bg_fill, theme::CONTROLE);
    let d = theme::dock(egui_dock::Style::default());
    assert_eq!((d.tab_bar.bg_fill, d.tab.active.bg_fill, d.separator.color_hovered), (theme::ENTETE, theme::PANNEAU, theme::ACCENT));
}

#[test]
fn windows_font_goes_first_and_keeps_the_others() {
    let mut f = FontDefinitions::default();
    let avant = f.families[&FontFamily::Proportional].clone();
    if theme::polices(&mut f) {
        let apres = &f.families[&FontFamily::Proportional];
        assert_eq!(apres[0], "segoe-ui");
        assert_eq!(&apres[1..], &avant[..], "les polices d'egui restent derrière");
    } else {
        assert_eq!(f.families[&FontFamily::Proportional], avant);
    }
}
