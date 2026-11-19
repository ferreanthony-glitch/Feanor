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

/// Partie 173.5 : sur la rangée du bas de la piste active, comme sur la ligne de la timeline : la liste du mode (et ses flèches s'il y
/// a la place), la barre d'opacité, puis le losange à sa droite ; ils se suivent sans se toucher, le nom du mode `label` est écrit dans
/// la liste sans en sortir, et rien ne passe le bord droit de la colonne. Les icônes de la rangée du milieu ne se touchent pas et
/// restent dans la colonne.
fn rien_ne_deborde(b: &Banc, label: &str, quoi: &str) {
    let (liste, losange, barre) = (ou("Mode de fusion"), ou("Créer une clé"), ou("Opacité du calque"));
    let colonne = ou("Largeur de la colonne");
    let mut avant_la_barre = liste;
    if let (Some(p), Some(s)) = (vus::ou("Mode précédent").first(), vus::ou("Mode suivant").first()) {
        assert!(p.right() <= liste.left() + 0.5 && liste.right() <= s.left() + 0.5, "{quoi} : ‹ liste › : {p:?} {liste:?} {s:?}");
        avant_la_barre = *s;
    }
    assert!(avant_la_barre.right() <= barre.left() + 0.5, "{quoi} : la liste touche la barre : {avant_la_barre:?} / {barre:?}");
    assert!(barre.right() <= losange.left() + 0.5, "{quoi} : la barre touche le losange : {barre:?} / {losange:?}");
    assert!(losange.right() <= colonne.right() + 0.5, "{quoi} : le losange sort de la colonne : {losange:?} / {colonne:?}");
    assert!(barre.width() >= 30.0, "{quoi} : la barre d'opacité reste lisible : {barre:?}");
    assert!((liste.center().y - barre.center().y).abs() < 3.0 && (losange.center().y - barre.center().y).abs() < 3.0, "{quoi} : une rangée");
    let t = texte_dans(b, label, liste).unwrap_or_else(|| panic!("{quoi} : « {label} » n'est pas écrit dans la liste {liste:?} : {:?}", textes(b).into_iter().filter(|(_, r)| liste.intersects(*r)).collect::<Vec<_>>()));
    assert!(t.left() >= liste.left() - 0.5 && t.right() <= liste.right() + 0.5, "{quoi} : « {label} » sort de la liste : {t:?} / {liste:?}");
    // La rangée des icônes, au-dessus : dans la colonne, sans chevauchement.
    let mut icones: Vec<Rect> = ICONES_DE_LA_PISTE.iter().filter_map(|n| icone(n)).collect();
    icones.sort_by(|a, c| a.left().total_cmp(&c.left()));
    for (k, r) in icones.iter().enumerate() {
        assert!(r.right() <= colonne.right() + 0.5, "{quoi} : une icône sort de la colonne : {r:?} / {colonne:?}");
        // (Partie 179 : à la colonne la plus étroite, un indicateur qui laisserait trop peu de place au nom descend sur la rangée du
        // mode, après la vignette : à gauche de la liste, sans la toucher.)
        assert!(r.bottom() <= liste.top() + 1.0 || r.right() <= liste.left() + 0.5, "{quoi} : les icônes au-dessus de la rangée du mode, ou à gauche de la liste : {r:?} / {liste:?}");
        if k > 0 {
            assert!(r.left() >= icones[k - 1].right() - 0.5, "{quoi} : deux icônes se touchent : {icones:?}");
        }
    }
}

/// Partie 173.5 : les options de la ligne du calque de la timeline qui manquaient sur la piste (nom de leur bulle ; « Groupe : … »
/// commence ainsi), dans leur ordre sur la piste, après l'accroche à la caméra.
const ICONES_DE_LA_PISTE: &[&str] = &[crate::accroche::NOM, "Aplat sous le trait", "Préserver la transparence", "Pochoir", "Pochoir positif", "Verrouiller la position", "Groupe : ", "Pelure d'oignon du calque"];

/// Le rectangle de l'icône de la piste dont la bulle est `nom` (ou commence par `nom` pour le groupe).
fn icone(nom: &str) -> Option<Rect> {
    if nom.ends_with(": ") {
        return vus::noms().into_iter().find(|n| n.starts_with(nom)).and_then(|n| de_la_piste(vus::ou(&n)));
    }
    de_la_piste(vus::ou(nom))
}

/// (Partie 178 : la ligne « Calques » du Storyboard porte aussi les icônes des colonnes ; celle de la piste active est dans la colonne,
/// plus bas. Ailleurs, le même nom peut être dans un panneau, hors de la colonne : il compte quand la colonne n'est pas à l'écran.)
fn de_la_piste(v: Vec<Rect>) -> Option<Rect> {
    let colonne = vus::ou("Largeur de la colonne").first().map(|c| c.right());
    let dans: Vec<Rect> = v.iter().copied().filter(|r| colonne.is_none_or(|c| r.left() < c)).collect();
    dans.into_iter().max_by(|a, c| a.top().total_cmp(&c.top())).or_else(|| v.first().copied())
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
    // (Partie 173.5 : la pelure est aussi revenue en icône sur la piste, au bout des options ; le menu la garde.)
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
    // La ligne du calque courant de la timeline (Animation) garde sa pelure, ses flèches ‹ › et sa liste. (Partie 174 : la pelure
    // est une colonne de l'en-tête et de chaque calque : quatre cases pour trois calques ; la liste écrit le mode, comme ici. Partie
    // 179.5 : plus d'icônes des colonnes sur la rangée d'en-tête : trois cases, une par calque.)
    let b = banc_dans(Espace::Animation);
    assert_eq!(vus::ou("Pelure d'oignon du calque").len(), 3);
    assert_eq!(vus::ou("Mode précédent").len(), 1);
    assert_eq!(vus::ou("Mode suivant").len(), 1);
    assert!(texte_dans(&b, "Normal", ou("Mode de fusion")).is_some());
}

// ------------------------------------------------------------------------------------ 173.5 : la colonne, les options de la piste

/// Les boutons de la barre du montage, en tête de la colonne du Storyboard (noms de leurs bulles).
const BOUTONS_DE_LA_COLONNE: &[&str] = &["Début du montage", "Lire le montage / pause", "Lecture en boucle", "Plus…"];

/// Chaque bouton de la barre du montage qui ne passe jamais sous « Plus… » est entier dans la colonne, au-dessus de « Caméra »
/// (partie 179 : les autres y passent quand la place manque, les moins utilisés d'abord ; `timeline_serree_tests`).
fn les_boutons_tiennent(quoi: &str) {
    let colonne = ou("Largeur de la colonne");
    for nom in BOUTONS_DE_LA_COLONNE {
        // (Partie 179.5 : en tête de la colonne, pas plus haut : la barre de commandes du haut de la fenêtre a aussi un « Plus… », qui
        // tombait dans la colonne la plus étroite d'avant et n'y tombe plus.)
        let r = *vus::ou(nom).iter().find(|r| r.top() >= colonne.top() - 2.0 && r.top() < colonne.top() + 80.0).unwrap_or_else(|| panic!("{quoi} : « {nom} » n'est pas en tête de la colonne : {:?}", vus::ou(nom)));
        assert!(r.right() <= colonne.right() + 0.5, "{quoi} : « {nom} » sort de la colonne : {r:?} / {colonne:?}");
    }
}

#[test]
fn the_storyboard_column_is_narrower_by_default_and_everything_still_fits() {
    let b = banc_dans(Espace::Storyboard);
    let w = b.app.montage_head_w;
    // Avant : au moins 474 points (la barre du montage sur une ligne). Maintenant, la largeur d'origine, plus étroite.
    assert!((w - crate::montagehead::LARGEUR_STORYBOARD).abs() < 0.5, "la largeur d'origine : {w}");
    assert!(w < 400.0, "plus étroite qu'avant : {w}");
    assert!(w >= crate::montagehead::largeur_min_colonne(crate::barres::Barre::Montage) - 0.5);
    les_boutons_tiennent("défaut");
    rien_ne_deborde(&b, "Normal", "défaut");
    // Toutes les options ajoutées sont à l'écran, sur la piste.
    for nom in ICONES_DE_LA_PISTE {
        assert!(icone(nom).is_some(), "« {nom} » sur la piste : {:?}", vus::noms());
    }
    // La piste du calque actif a deux rangées (trois en partie 173.5 ; deux de 20 depuis la partie 179).
    assert_eq!(crate::montagerows::LAYER_H, 2.0 * crate::pistes::RANGEE_ACTIVE);
}

#[test]
fn the_grip_changes_the_width_keeps_it_and_stops_at_its_bounds() {
    use crate::montagehead::{largeur_min_colonne, HEAD_MAX};
    let mut b = banc_dans(Espace::Storyboard);
    let w0 = b.app.montage_head_w;
    let g = ou("Largeur de la colonne").center();
    crate::pistes_tests::glisser(&mut b, g, g + eframe::egui::vec2(60.0, 0.0));
    let w1 = b.app.montage_head_w;
    assert!((w1 - (w0 + 60.0)).abs() < 2.0, "la poignée élargit : {w0} → {w1}");
    assert!((b.app.settings.storyboard_head_w - w1).abs() < 0.5, "gardée dans les réglages : {}", b.app.settings.storyboard_head_w);
    let relu = crate::settings::Settings::load();
    assert!((relu.storyboard_head_w - w1).abs() < 0.5, "enregistrée : {}", relu.storyboard_head_w);
    // L'espace Montage garde sa propre largeur (sa barre est au-dessus de la bande) ; revenir au Storyboard rend la sienne.
    let montage = b.app.settings.montage_head_w;
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert!((b.app.montage_head_w - crate::montagehead::clamp_head_w(montage)).abs() < 0.5, "Montage : {}", b.app.montage_head_w);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert!((b.app.montage_head_w - w1).abs() < 0.5, "Storyboard : {}", b.app.montage_head_w);
    // Tirée loin à gauche : elle s'arrête au minimum, tout reste lisible.
    let g = ou("Largeur de la colonne").center();
    crate::pistes_tests::glisser(&mut b, g, g - eframe::egui::vec2(700.0, 0.0));
    b.chauffer(4);
    let min = largeur_min_colonne(crate::barres::Barre::Montage);
    assert!((b.app.montage_head_w - min).abs() < 1.0, "au minimum : {} / {min}", b.app.montage_head_w);
    assert!(min < crate::montagehead::LARGEUR_STORYBOARD, "le minimum est sous la largeur d'origine : {min}");
    les_boutons_tiennent("minimum");
    b.app.regler_mode_calques(0, BlendMode::Screen);
    b.chauffer(3);
    rien_ne_deborde(&b, "Superposition", "minimum");
    // Loin à droite : au plus `HEAD_MAX`.
    let g = ou("Largeur de la colonne").center();
    crate::pistes_tests::glisser(&mut b, g, g + eframe::egui::vec2(900.0, 0.0));
    assert!(b.app.montage_head_w <= HEAD_MAX + 0.5, "{}", b.app.montage_head_w);
    b.chauffer(3);
    rien_ne_deborde(&b, "Superposition", "large");
}

#[test]
fn the_options_added_to_the_track_do_what_the_timeline_line_does() {
    let mut b = banc_dans(Espace::Storyboard);
    choisir(&mut b, &[2]);
    let calque = |b: &Banc, li: usize| b.app.doc.panels[0].layers[li].clone();
    let pas = |b: &mut Banc, nom: &str| {
        let n0 = etapes(b);
        let r = icone(nom).unwrap_or_else(|| panic!("« {nom} » : {:?}", vus::noms()));
        clic(b, r.center(), Modifiers::NONE);
        assert_eq!(etapes(b), n0 + 1, "« {nom} » : une étape d'annulation ({})", b.app.status);
    };
    pas(&mut b, "Préserver la transparence");
    assert!(calque(&b, 0).keep_alpha);
    pas(&mut b, "Pochoir");
    assert!(calque(&b, 0).stencil && calque(&b, 2).stencil, "le pochoir va aux calques choisis, comme dans la timeline");
    pas(&mut b, "Pochoir positif");
    assert!(calque(&b, 0).stencil_negative);
    b.chauffer(2);
    assert!(icone("Pochoir négatif").is_some(), "la bulle suit le sens du pochoir");
    pas(&mut b, "Verrouiller la position");
    assert!(calque(&b, 0).position_locked);
    pas(&mut b, "Pelure d'oignon du calque");
    assert_eq!(calque(&b, 0).onion, calque(&b, 2).onion);
    pas(&mut b, crate::accroche::NOM);
    assert!(b.app.est_accroche(0), "{}", b.app.status);
    pas(&mut b, "Aplat sous le trait");
    assert!(calque(&b, 0).fond.is_some(), "{}", b.app.status);
    // Le groupe : son menu s'ouvre au clic, comme sur la ligne de la timeline.
    let r = icone("Groupe : ").expect("le groupe");
    let avant = textes(&b).len();
    clic(&mut b, r.center(), Modifiers::NONE);
    assert!(textes(&b).len() > avant, "le menu des groupes s'ouvre");
}

/// Partie 173.6 (Anthony : « les icônes au-dessus des options de fusion et opacité peuvent être plus espacées pour être plus
/// lisibles ») : à la largeur `w` de la colonne, toutes les options de la rangée du milieu sont là, dans la colonne, dans leurs cases
/// de la largeur de celles de la timeline (au moins 17 points, les icônes à la même taille), séparées d'au moins 2 points, d'au
/// moins `ICON_W` de centre à centre. Rend l'écart moyen d'un centre au suivant.
fn icones_espacees(b: &mut Banc, w: f32, quoi: &str) -> f32 {
    b.app.montage_head_w = w;
    b.chauffer(6);
    let colonne = ou("Largeur de la colonne");
    let mut icones: Vec<(String, Rect)> = ICONES_DE_LA_PISTE.iter().map(|n| (n.to_string(), icone(n).unwrap_or_else(|| panic!("{quoi} : « {n} » manque sur la piste : {:?}", vus::noms())))).collect();
    icones.sort_by(|a, c| a.1.left().total_cmp(&c.1.left()));
    for (k, (n, r)) in icones.iter().enumerate() {
        assert!(r.right() <= colonne.right() + 0.5, "{quoi} : « {n} » sort de la colonne : {r:?} / {colonne:?}");
        assert!(r.width() >= 19.0 - 2.0 - 0.5, "{quoi} : « {n} » dans une case presque aussi large que dans la timeline (19) : {r:?}");
        if k > 0 {
            let (m, p) = &icones[k - 1];
            // (Partie 179 : les cases se suivent, collées à droite comme celles de la timeline ; les icônes de 13 points y gardent 6
            // points entre elles.)
            assert!(r.left() - p.right() >= -0.5, "{quoi} : « {m} » et « {n} » se chevauchent : {p:?} {r:?}");
            assert!(r.center().x - p.center().x >= 19.0 - 0.5, "{quoi} : « {m} » et « {n} » trop serrés : {p:?} {r:?}");
        }
    }
    (icones.last().unwrap().1.center().x - icones[0].1.center().x) / (icones.len() - 1) as f32
}

#[test]
fn the_option_icons_of_the_track_are_spaced_like_the_timeline_line_at_every_width() {
    let mut b = banc_dans(Espace::Storyboard);
    let min = crate::montagehead::largeur_min_colonne(crate::barres::Barre::Montage);
    let a_min = icones_espacees(&mut b, min, "minimum");
    let a_defaut = icones_espacees(&mut b, crate::montagehead::LARGEUR_STORYBOARD, "défaut");
    let a_large = icones_espacees(&mut b, 480.0, "480");
    // (Partie 179, Anthony : « Justifie toutes les icônes à droite », « resserrer » : les icônes sont dans les cases de la timeline,
    // 19 points de centre à centre, collées au bord droit, à toute largeur ; l'écart ne grandit plus avec la colonne.)
    assert!(a_min >= 19.0 - 0.5 && a_defaut >= 19.0 - 0.5 && a_large >= 19.0 - 0.5, "des cases de 19 : {a_min} {a_defaut} {a_large}");
    // (Partie 179 : le minimum est maintenant celui de la ligne des boutons, sur une ligne : la lecture, le temps, « Plus… » et la
    // légende, un peu plus que les 250 points de la piste.)
    assert!(min >= crate::pistes::LARGEUR_PISTE_MIN - 0.5 && min <= 300.0, "{min}");
}

/// Les images de la partie 173, à taille réelle (un pixel de l'écran du banc pour un point, aucun agrandissement), dans le dossier
/// `SHOT_CAPTURE_DIR` : la piste avec « Normal ▾ », avec un mode long, la liste ouverte, le clic droit ouvert sur Apparence. Le projet
/// est `SHOT_CAPTURE_PROJET` (une copie : le banc n'enregistre rien). Sans carte graphique : rien.
#[test]
#[ignore]
fn piste_mode_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let doc = match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet").doc,
        None => doc_with(&[&["Décor", "Perso", "Ombre"], &["Seul"]]),
    };
    let mut b = crate::perf_tests::banc_taille(doc, true, 1920, 1080);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(10);
    b.garder_formes = true;
    let sauver = |b: &mut Banc, nom: &str, zone: Rect| {
        b.chauffer(4);
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom}") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let z = zone.intersect(Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(1920.0, 1080.0)));
        let crop = image::imageops::crop_imm(&img, z.left() as u32, z.top() as u32, z.width() as u32, z.height() as u32).to_image();
        crop.save(dossier.join(nom)).expect("png");
        println!("CAPTURE {nom} {}x{}", crop.width(), crop.height());
    };
    // Autour de la piste active : la colonne des calques et le début des pistes.
    let autour = |b: &Banc| {
        let l = ou("Mode de fusion");
        let _ = b;
        Rect::from_min_max(pos2(0.0, l.top() - 120.0), pos2(640.0, l.bottom() + 90.0))
    };
    b.chauffer(2);
    let z = autour(&b);
    sauver(&mut b, "173-1-piste-normal.png", z);
    let li = b.app.cur_layer;
    b.app.regler_mode_calques(li, BlendMode::Screen);
    b.chauffer(3);
    let z = autour(&b);
    sauver(&mut b, "173-2-piste-superposition.png", z);
    // La liste ouverte (elle monte au-dessus de la piste, en bas de l'écran).
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    let z = autour(&b);
    sauver(&mut b, "173-3-liste-ouverte.png", Rect::from_min_max(pos2(0.0, (z.top() - 560.0).max(0.0)), pos2(z.right(), 1080.0)));
    b.app.regler_mode_calques(li, BlendMode::Normal);
    // (Échap ferme la liste, sans cliquer dans le dessin.)
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    // Le clic droit sur la piste, Apparence ouvert : la pelure d'oignon du calque et son raccourci.
    let nom = b.app.layers()[li].name.clone();
    let liste = ou("Mode de fusion");
    let p = textes(&b).into_iter().filter(|(t, r)| *t == nom && (r.center().y - liste.center().y).abs() < 40.0).map(|(_, r)| r.center()).next().unwrap_or(pos2(liste.left() + 10.0, liste.top() - 12.0));
    clic_droit(&mut b, p);
    if let Some(a) = textes(&b).into_iter().filter(|(t, _)| t.contains("Apparence")).map(|(_, r)| r).last() {
        survoler(&mut b, a.center());
    }
    let z = autour(&b);
    sauver(&mut b, "173-4-clic-droit-apparence.png", Rect::from_min_max(pos2(0.0, (z.top() - 120.0).max(0.0)), pos2(900.0, 1080.0)));
    // La colonne la plus étroite (espace Montage, 104 points), un mode long : le nom raccourci dans sa case.
    b.image(vec![Event::Key { key: eframe::egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    b.app.set_espace(Espace::Montage);
    b.app.montage_head_w = crate::montagehead::HEAD_MIN;
    b.app.regler_mode_calques(li, BlendMode::Screen);
    b.chauffer(8);
    let l = ou("Mode de fusion");
    sauver(&mut b, "173-5-colonne-etroite-superposition.png", Rect::from_min_max(pos2(0.0, l.top() - 60.0), pos2(420.0, l.bottom() + 40.0)));
    // Partie 173.5 : le Storyboard, la colonne à sa largeur d'origine, à sa plus petite, plus grande ; un gros plan de la piste.
    b.app.set_espace(Espace::Storyboard);
    b.app.regler_mode_calques(li, BlendMode::Normal);
    b.chauffer(8);
    let bande = |b: &mut Banc, nom: &str| {
        b.chauffer(6);
        let g = ou("Largeur de la colonne");
        println!("CAPTURE {nom} : colonne de {:.0} points, barre sur {} ligne(s)", b.app.montage_head_w, 1);
        sauver(b, nom, Rect::from_min_max(pos2(0.0, g.top() - 2.0), pos2(g.right() + 260.0, g.bottom())));
    };
    bande(&mut b, "173-6-storyboard-largeur-origine.png");
    // Le gros plan de la piste du calque actif (partie 173.6 : à 340 points, au minimum, à 480).
    let gros_plan = |b: &mut Banc, nom: &str| {
        b.chauffer(6);
        let (h, o) = (ou("Commandes du calque"), ou("Opacité du calque"));
        let piste = Rect::from_min_max(pos2(40.0, h.top() - 4.0), pos2(ou("Largeur de la colonne").right() + 4.0, o.bottom() + 6.0));
        sauver(b, nom, piste);
    };
    gros_plan(&mut b, "173-9-gros-plan-piste-340.png");
    b.app.montage_head_w = 0.0;
    bande(&mut b, "173-7-storyboard-largeur-minimale.png");
    gros_plan(&mut b, "173-10-gros-plan-piste-minimum.png");
    b.app.montage_head_w = 480.0;
    bande(&mut b, "173-8-storyboard-largeur-480.png");
    gros_plan(&mut b, "173-11-gros-plan-piste-480.png");
}