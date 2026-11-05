//! Essais de « l'animation n'est plus un onglet » : l'onglet disparaît du sélecteur d'espaces, le double-clic sur un plan
//! (de la bande du Storyboard comme de celle du Montage) entre en animation sur ce plan, le bouton « Retour au storyboard »
//! en sort, et un ancien réglage qui retenait l'animation retombe sur le Storyboard.

use eframe::egui::{epaint::Shape, Modifiers, Pos2};

use crate::app::Mode;
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::{clic, double_clic};
use crate::settings::Settings;
use crate::storyboard::BLOCS_DU_MONTAGE;
use crate::workspace::Espace;

fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for _ in 0..3 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

fn banc() -> Banc {
    let mut b = banc_taille(film(), false, 1600, 900);
    b.garder_textes = true;
    b.garder_formes = true;
    b.chauffer(6);
    b
}

/// Le centre du premier texte peint par la dernière image qui est exactement `texte` ou, avec `contient`, qui le contient.
fn texte_a_l_ecran(b: &Banc, texte: &str, contient: bool) -> Option<Pos2> {
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            let ce_texte = &t.galley.job.text;
            if (contient && ce_texte.contains(texte)) || (!contient && ce_texte == texte) {
                return Some(t.pos + t.galley.size() / 2.0);
            }
        }
    }
    None
}

/// Le centre du plan `k` dans la bande peinte par la dernière image.
fn centre_du_plan(k: usize) -> Pos2 {
    let r = BLOCS_DU_MONTAGE.with(|b| b.borrow()[k]);
    r.center()
}

#[test]
fn the_animation_tab_is_gone_and_the_other_three_stay_in_work_order() {
    let mut b = banc();
    assert!(texte_a_l_ecran(&b, " Animation ", false).is_none(), "plus d'onglet Animation");
    let (v, s, m) = (texte_a_l_ecran(&b, " Vignettes ", false).unwrap(), texte_a_l_ecran(&b, " Storyboard ", false).unwrap(), texte_a_l_ecran(&b, " Montage ", false).unwrap());
    assert!(v.x < s.x && s.x < m.x, "Vignettes, Storyboard, Montage : {v:?} {s:?} {m:?}");
    // L'animation, elle, n'est dans aucune liste d'espaces à choisir.
    assert_eq!(Espace::VISIBLES.map(Espace::nom), ["Vignettes", "Storyboard", "Montage"]);
    assert!(crate::keymap::action_info("espace.animation").is_none(), "plus d'action « espace.animation »");
    assert!(crate::palette::palette_commands().iter().all(|(id, _)| *id != "espace.animation"));
    // Et hors animation, le bouton de retour ne se montre pas.
    for espace in Espace::VISIBLES {
        b.app.set_espace(espace);
        b.chauffer(3);
        assert!(texte_a_l_ecran(&b, "Retour au storyboard", true).is_none(), "{espace:?} : pas de bouton de retour");
    }
}

#[test]
fn a_double_click_on_a_plan_of_the_storyboard_strip_enters_the_animation_on_that_plan() {
    // (Un banc neuf par plan : deux séries de clics rapprochées feraient un triple-clic.)
    for k in [2usize, 1, 3] {
        let mut b = banc();
        b.app.set_espace(Espace::Storyboard);
        b.chauffer(4);
        b.app.select_panel(0);
        b.chauffer(2);
        double_clic(&mut b, centre_du_plan(k));
        assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation), "plan {k}");
        assert_eq!(b.app.cur_panel, k, "le double-clic entre sur le plan visé");
    }
}

#[test]
fn a_double_click_on_a_plan_of_the_montage_strip_enters_the_animation_on_that_plan() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    b.app.select_panel(0);
    b.chauffer(2);
    double_clic(&mut b, centre_du_plan(3));
    assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation));
    assert_eq!(b.app.cur_panel, 3);
}

#[test]
fn the_back_button_shows_only_in_the_animation_and_returns_to_the_storyboard_on_the_same_plan() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    b.chauffer(3);
    b.app.enter_animation(2);
    b.chauffer(4);
    assert_eq!(b.app.espace, Espace::Animation);
    let p = texte_a_l_ecran(&b, "Retour au storyboard", true).expect("le bouton « Retour au storyboard » est à l'écran en animation");
    assert!(p.y < 40.0, "dans la barre du haut : {p:?}");
    clic(&mut b, p, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard), "toujours l'espace Storyboard, même venu du Montage");
    assert_eq!(b.app.cur_panel, 2, "sur le même plan");
    assert!(texte_a_l_ecran(&b, "Retour au storyboard", true).is_none(), "le bouton s'en va avec l'animation");
}

#[test]
fn an_old_setting_that_remembered_the_animation_falls_back_on_the_storyboard() {
    let s: Settings = toml::from_str("espace_storyboard = \"animation\"\n").unwrap();
    assert_eq!(s.storyboard_space, Espace::Animation, "le fichier se lit encore");
    assert_eq!(Settings::apres_lecture(s).storyboard_space, Espace::Storyboard);
    // Et un espace restauré en animation (projet rouvert, onglet restauré) reste utilisable : son bouton de retour est là.
    let mut b = banc();
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    assert!(texte_a_l_ecran(&b, "Retour au storyboard", true).is_some());
    b.app.run_action("mode.basculer");
    assert_eq!(b.app.espace, Espace::Storyboard, "Tab, laissé tel quel, revient aussi");
}