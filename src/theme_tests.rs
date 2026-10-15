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
