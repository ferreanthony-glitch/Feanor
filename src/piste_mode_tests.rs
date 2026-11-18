//! Partie 173 : sur la piste du calque actif (vue Storyboard), le mode de fusion est une vraie liste « Normal ▾ » qui écrit le nom
//! du mode, comme la liste de la timeline ; la pelure d'oignon du calque, dont l'icône (trois ronds) était à gauche de la deuxième
//! rangée, passe dans le menu du clic droit de la piste (et du ⋮ : c'est le même menu), avec son raccourci. Anthony, 2026-11-18.

use eframe::egui::{pos2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::blend::BlendMode;
use crate::bulles::vus;
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::{clic, doc_with, etapes, textes};
use crate::workspace::Espace;

/// Un banc sur un film de deux plans (trois calques, puis un), dans l'espace `espace`, les formes gardées (pour lire les textes).
fn banc_dans(espace: Espace) -> Banc {
    let mut b = banc(doc_with(&[&["Décor", "Perso", "Ombre"], &["Seul"]]), false);
    b.app.set_espace(espace);
    b.garder_formes = true;
    b.chauffer(6);
    b
}

/// Le rectangle de l'élément dont la bulle s'appelle `nom` (il n'y en a qu'un à l'écran).
fn ou(nom: &str) -> Rect {
    let v = vus::ou(nom);
    assert_eq!(v.len(), 1, "« {nom} » : un seul à l'écran : {v:?} ({:?})", vus::noms());
    v[0]
}

/// Les calques choisis avec le calque courant (0, « Ombre ») : `garder`.
fn choisir(b: &mut Banc, garder: &[usize]) {
    b.app.layer_sel = garder.iter().map(|&li| b.app.doc.panels[0].layers[li].id).collect();
    assert_eq!(b.app.chosen_layers(), std::iter::once(0).chain(garder.iter().copied()).collect::<Vec<_>>());
}

fn modes(b: &Banc) -> Vec<BlendMode> {
    b.app.doc.panels[0].layers.iter().map(|l| l.blend).collect()
}

/// Le texte `mot` peint dans `zone` (son centre y est), avec sa place.
fn texte_dans(b: &Banc, mot: &str, zone: Rect) -> Option<Rect> {
    textes(b).into_iter().find(|(t, r)| t == mot && zone.contains(r.center())).map(|(_, r)| r)
}

/// Le texte `mot` peint n'importe où (le plus bas, s'il y en a plusieurs : un menu ouvert passe par-dessus le reste).
fn texte(b: &Banc, mot: &str) -> Option<Rect> {
    textes(b).into_iter().filter(|(t, _)| t == mot).map(|(_, r)| r).max_by(|a, c| a.top().total_cmp(&c.top()))
}

fn clic_droit(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

/// Le pointeur se pose en `p` le temps qu'un sous-menu s'ouvre.
fn survoler(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p - eframe::egui::vec2(0.0, 1.0))], 1.0 / 60.0);
    b.chauffer(3);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(12);
}

/// Sur la deuxième rangée de la piste active, la liste du mode, le losange et la barre d'opacité se suivent sans se toucher, le nom
/// du mode `label` est écrit dans la liste sans en sortir, et rien ne passe le bord droit de la colonne.
fn rien_ne_deborde(b: &Banc, label: &str, quoi: &str) {
    let (liste, losange, barre) = (ou("Mode de fusion"), ou("Créer une clé"), ou("Opacité du calque"));
    let colonne = ou("Largeur de la colonne");
    assert!(liste.right() <= losange.left() + 0.5, "{quoi} : la liste touche le losange : {liste:?} / {losange:?}");
    assert!(losange.right() <= barre.left() + 0.5, "{quoi} : le losange touche la barre : {losange:?} / {barre:?}");
    assert!(barre.right() <= colonne.right() + 0.5, "{quoi} : la barre sort de la colonne : {barre:?} / {colonne:?}");
    assert!(barre.width() >= 20.0, "{quoi} : la barre d'opacité reste lisible : {barre:?}");
    if let Some(camera) = vus::ou(crate::accroche::NOM).first() {
        assert!(camera.right() <= liste.left() + 0.5, "{quoi} : la caméra touche la liste : {camera:?} / {liste:?}");
    }
    let t = texte_dans(b, label, liste).unwrap_or_else(|| panic!("{quoi} : « {label} » n'est pas écrit dans la liste {liste:?} : {:?}", textes(b).into_iter().filter(|(_, r)| liste.intersects(*r)).collect::<Vec<_>>()));
    assert!(t.left() >= liste.left() - 0.5 && t.right() <= liste.right() + 0.5, "{quoi} : « {label} » sort de la liste : {t:?} / {liste:?}");
    assert!(t.right() <= losange.left(), "{quoi} : « {label} » passe sur le losange : {t:?} / {losange:?}");
}

#[test]
fn the_active_track_shows_the_name_of_its_blend_mode_in_a_list() {
    let mut b = banc_dans(Espace::Storyboard);
    let liste = ou("Mode de fusion");
    // Le nom du mode, en entier, dans la liste (plus l'icône ◐ seule).
    assert!(texte_dans(&b, "Normal", liste).is_some(), "« Normal » dans la liste : {liste:?} / {:?}", textes(&b));
    assert!(texte_dans(&b, egui_phosphor::regular::CIRCLE_HALF, liste.expand(2.0)).is_none(), "l'icône ◐ seule n'y est plus");
    assert!(liste.width() >= 40.0, "une vraie liste, pas une case d'icône : {liste:?}");
    // Un autre mode : son nom.
    b.app.regler_mode_calques(0, BlendMode::Multiply);
    b.chauffer(3);
    assert!(texte_dans(&b, "Produit", ou("Mode de fusion")).is_some(), "{:?}", textes(&b));
    // La même mise en page qu'à l'ordinaire : sur la deuxième rangée, à gauche du losange et de la barre d'opacité.
    rien_ne_deborde(&b, "Produit", "Produit");
}

#[test]
fn a_long_mode_name_is_shortened_inside_the_list_at_the_narrowest_track() {
    // La colonne à sa largeur la plus petite, dans le Storyboard (la barre du montage dedans) et dans l'espace Montage (104 points) ;
    // le calque a des effets (la case « fx » prend sa place) : le pire.
    for espace in [Espace::Storyboard, Espace::Montage] {
        let mut b = banc_dans(espace);
        b.app.montage_head_w = crate::montagehead::HEAD_MIN;
        b.app.doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::GaussianBlur));
        for mode in [BlendMode::Normal, BlendMode::Screen, BlendMode::Add] {
            b.app.regler_mode_calques(0, mode);
            b.chauffer(4);
            rien_ne_deborde(&b, mode.label(), &format!("{espace:?}, {}", mode.label()));
        }
        // Plus large : « Superposition » s'écrit en entier.
        b.app.montage_head_w = 330.0;
        b.app.regler_mode_calques(0, BlendMode::Screen);
        b.chauffer(4);
        let liste = ou("Mode de fusion");
        let t = texte_dans(&b, "Superposition", liste).expect("le nom");
        let entier = b.ctx.fonts(|f| f.layout_no_wrap("Superposition".into(), eframe::egui::FontId::proportional(11.0), eframe::egui::Color32::WHITE).size().x);
        assert!(t.width() >= entier - 0.5, "{espace:?} : à 330 points, le nom entier : {t:?} / {entier}");
    }
}

#[test]
fn choosing_a_mode_in_the_track_list_changes_every_chosen_layer_in_one_undo() {
    let mut b = banc_dans(Espace::Storyboard);
    choisir(&mut b, &[1, 2]);
    let n0 = etapes(&b);
    // La liste s'ouvre au clic : la liste des modes en texte, comme avant.
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    for m in [BlendMode::Normal, BlendMode::Multiply, BlendMode::Screen, BlendMode::Luminosity] {
        assert!(texte(&b, m.label()).is_some(), "« {} » dans la liste ouverte", m.label());
    }
    let produit = texte(&b, "Produit").expect("« Produit » dans la liste ouverte");
    clic(&mut b, produit.center(), Modifiers::NONE);
    assert_eq!(modes(&b), vec![BlendMode::Multiply; 3], "les trois calques choisis : {}", b.app.status);
    assert_eq!(etapes(&b), n0 + 1, "une seule étape d'annulation");
    assert!(texte(&b, "Luminosité").is_none(), "la liste s'est refermée");
    assert!(texte_dans(&b, "Produit", ou("Mode de fusion")).is_some(), "la liste dit le nouveau mode");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(modes(&b), vec![BlendMode::Normal; 3], "un Ctrl+Z les remet tous");
}

#[test]
fn the_onion_of_the_layer_left_the_track_for_its_right_click_menu() {
    let mut b = banc_dans(Espace::Storyboard);
    // Plus d'icône de la pelure du calque sur la piste : la place va à la liste du mode.
    assert!(vus::ou("Pelure d'oignon du calque").is_empty(), "l'icône est partie de la piste : {:?}", vus::ou("Pelure d'oignon du calque"));
    choisir(&mut b, &[2]);
    let avant: Vec<bool> = b.app.doc.panels[0].layers.iter().map(|l| l.onion).collect();
    let n0 = etapes(&b);
    // Clic droit sur la piste (sur le nom du calque) : le menu du calque ; Apparence ▸ Pelure d'oignon du calque, avec Ctrl+L.
    let nom = texte(&b, "Ombre").expect("le nom du calque actif");
    clic_droit(&mut b, nom.center());
    let apparence = texte(&b, &format!("{}  Apparence", egui_phosphor::regular::EYE)).or_else(|| textes(&b).into_iter().filter(|(t, _)| t.contains("Apparence")).map(|(_, r)| r).last()).expect("« Apparence » dans le menu du clic droit");
    survoler(&mut b, apparence.center());
    let entree = textes(&b).into_iter().filter(|(t, _)| t.contains("Pelure d'oignon du calque")).map(|(_, r)| r).last().unwrap_or_else(|| panic!("« Pelure d'oignon du calque » sous Apparence : {:?}", textes(&b).iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>()));
    let touche = b.app.keymap.label("calque.pelure");
    assert!(!touche.is_empty());
    assert!(textes(&b).iter().any(|(t, r)| *t == touche && (r.center().y - entree.center().y).abs() < 4.0), "le raccourci {touche} sur sa ligne");
    // Le panneau de la pelure (l'ancien clic droit de l'icône) est juste à côté.
    assert!(textes(&b).iter().any(|(t, r)| t.contains("Panneau de la pelure d'oignon") && r.top() > entree.top()), "le panneau de la pelure, sous l'entrée");
    clic(&mut b, pos2(entree.left() + 20.0, entree.center().y), Modifiers::NONE);
    let apres: Vec<bool> = b.app.doc.panels[0].layers.iter().map(|l| l.onion).collect();
    assert_eq!(apres[0], !avant[0], "le calque actif bascule : {}", b.app.status);
    assert_eq!(apres[2], apres[0], "le calque choisi aussi");
    assert_eq!(apres[1], avant[1], "le calque non choisi ne bouge pas");
    assert_eq!(etapes(&b), n0 + 1, "une seule étape");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[0].layers.iter().map(|l| l.onion).collect::<Vec<_>>(), avant, "un Ctrl+Z");
    // Le raccourci fait toujours la même chose.
    b.app.run_action("calque.pelure");
    assert_eq!(b.app.doc.panels[0].layers[0].onion, !avant[0]);
}

#[test]
fn the_animation_timeline_keeps_its_own_line_untouched() {
    // La ligne du calque courant de la timeline (Animation) garde sa pelure, ses flèches ‹ › et sa liste : rien n'y change.
    let b = banc_dans(Espace::Animation);
    assert_eq!(vus::ou("Pelure d'oignon du calque").len(), 1);
    assert_eq!(vus::ou("Mode précédent").len(), 1);
    assert_eq!(vus::ou("Mode suivant").len(), 1);
    assert!(texte_dans(&b, "Normal", ou("Mode de fusion")).is_some());
}
