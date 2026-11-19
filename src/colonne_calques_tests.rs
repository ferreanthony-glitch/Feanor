//! Partie 174 : la colonne des calques de la timeline (Animation) dans la disposition validée par Anthony : une rangée d'en-tête
//! (⊖ / ⊕, « Calques », une icône par colonne de réglage), les colonnes alignées sous elle pour tous les calques, le calque courant
//! sur trois rangées (le mode de fusion écrit), les autres sur une ligne, et dans la bande une icône avant la première image et après
//! la dernière de chaque calque pour son comportement (Aucun, Boucle, Va-et-vient, Maintenir). Anthony, 2026-11-18.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, Pos2, Rect, Shape};

use crate::blend::BlendMode;
use crate::bulles::vus;
use crate::document::{Behaviour, Document, Layer, LayerKind};
use crate::layerheads::{Colonne, ROW_FINE, ROW_FULL};
use crate::perf_tests::Banc;
use crate::pistes_tests::{clic, etapes, textes};

/// Un plan de 24 images, de bas en haut : « Ciel » (dans le dossier « Décors », images 0 à 11), le dossier, « Fond » (0 à 11),
/// « Perso » (4 à 9, le calque courant) et « Ombre » (2 à 7).
pub(crate) fn doc_174() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let calque = |doc: &mut Document, nom: &str, de: usize, a: usize| {
        let cel = doc.new_cel(LayerKind::Vector);
        let mut l = Layer::new(nom, LayerKind::Vector);
        l.frames = (0..a).map(|f| (f >= de).then_some(cel)).collect();
        l
    };
    let dossier = Layer::new_folder("Décors");
    let mut ciel = calque(&mut doc, "Ciel", 0, 12);
    ciel.parent = Some(dossier.id);
    let fond = calque(&mut doc, "Fond", 0, 12);
    let perso = calque(&mut doc, "Perso", 4, 10);
    let ombre = calque(&mut doc, "Ombre", 2, 8);
    let mut layers = vec![ciel, dossier, fond, perso, ombre];
    crate::folders::normalize(&mut layers);
    doc.panels[0].layers = layers;
    doc
}

/// Le rang du calque `nom` dans le plan.
fn rang(b: &Banc, nom: &str) -> usize {
    b.app.layers().iter().position(|l| l.name == nom).unwrap_or_else(|| panic!("pas de calque « {nom} »"))
}

/// Un banc de 1920 × 1080 en Animation sur `doc_174`, « Perso » courant, les formes gardées.
fn banc_animation() -> Banc {
    let mut b = crate::perf_tests::banc_taille(doc_174(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    let perso = rang(&b, "Perso");
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, perso, 0);
    b.garder_formes = true;
    b.chauffer(8);
    // (Le pointeur entre d'abord dans la fenêtre, loin de la timeline, le temps de quelques images : le premier clic du banc
    // tombait sinon dans le vide.)
    b.image(vec![Event::PointerMoved(pos2(960.0, 200.0))], 1.0 / 60.0);
    b.chauffer(4);
    b
}

/// Les lignes de la colonne des calques peintes à la dernière image : (rang du calque, haut, bas).
fn lignes() -> Vec<(usize, f32, f32)> {
    crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone())
}

/// La ligne du calque `li` dans la colonne.
fn ligne(li: usize) -> (f32, f32) {
    lignes().into_iter().find(|l| l.0 == li).map(|l| (l.1, l.2)).unwrap_or_else(|| panic!("pas de ligne pour le calque {li} : {:?}", lignes()))
}

/// Les cases de la colonne `col` de l'en-tête et de chaque ligne : celles dont la bulle a son nom, dans la colonne des calques
/// (à gauche de la grille).
fn cases(col: Colonne) -> Vec<Rect> {
    let haut = lignes().iter().map(|l| l.1).fold(f32::MAX, f32::min) - crate::layerheads::ENTETE_H - 1.0;
    let bas = lignes().iter().map(|l| l.2).fold(0.0, f32::max);
    vus::ou(col.nom()).into_iter().filter(|r| r.center().y >= haut && r.center().y <= bas && r.width() <= 24.0).collect()
}

/// La case de référence de la colonne `col` : celle du calque courant. (Partie 179.5, Anthony : les icônes des colonnes barrées sur la
/// rangée « Calques » ; elles l'ont quittée, et c'est la ligne du calque courant, ses réglages éteints en icônes pâles, qui dit
/// quelle colonne est laquelle. Avant : la case de la rangée d'en-tête, la plus haute.)
fn entete(b: &Banc, col: Colonne) -> Rect {
    case_du_calque(col, b.app.cur_layer).unwrap_or_else(|| panic!("« {} » : aucune case sur le calque courant ({:?})", col.nom(), vus::noms()))
}

/// La case de la colonne `col` sur la ligne du calque `li`, s'il en a une.
fn case_du_calque(col: Colonne, li: usize) -> Option<Rect> {
    let (haut, bas) = ligne(li);
    cases(col).into_iter().find(|r| r.center().y > haut && r.center().y < bas)
}

/// L'abscisse du début de l'image `f` dans la grille (depuis la prise de la fin du plan, au bout de la règle).
fn x_image(b: &Banc, f: usize) -> f32 {
    let prise = crate::timeline::PRISE_FIN_DU_PLAN.with(|c| c.get()).expect("la prise de la fin du plan");
    prise.left() - (b.app.frame_count() as f32 - f as f32) * b.app.timeline_cell
}

/// L'icône de comportement du calque `li` (`apres` : après la dernière image), et son nom de bulle.
fn icone_comportement(b: &Banc, li: usize, apres: bool) -> Option<(String, Rect)> {
    let debut = if apres { "Après la dernière image : " } else { "Avant la première image : " };
    let _ = b;
    let (haut, bas) = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().1.iter().find(|r| r.0 == li).map(|r| (r.1, r.2)))?;
    vus::noms().into_iter().filter(|n| n.starts_with(debut)).flat_map(|n| vus::ou(&n).into_iter().map(move |r| (n.clone(), r))).find(|(_, r)| r.center().y > haut && r.center().y < bas)
}

#[test]
fn the_header_row_shows_the_seven_columns_in_order() {
    // (Partie 179.5 : les sept colonnes, dans l'ordre, sont sur la ligne du calque courant ; la rangée d'en-tête n'a plus que ⊖,
    // « Calques » et les commandes des calques.)
    let b = banc_animation();
    let rects: Vec<Rect> = Colonne::TOUTES.iter().map(|&c| entete(&b, c)).collect();
    for k in 1..rects.len() {
        assert!(rects[k].left() >= rects[k - 1].right() - 0.5, "dans l'ordre, sans chevauchement : {rects:?}");
        // (Partie 179.5 : sur la ligne du calque courant, la case de l'œil est un bouton un point plus haut que les autres.)
        assert!((rects[k].center().y - rects[0].center().y).abs() < 1.0, "sur une rangée : {rects:?}");
    }
    let premiere = lignes().iter().map(|l| l.1).fold(f32::MAX, f32::min);
    let (e0, e1) = (premiere - crate::layerheads::ENTETE_H - 1.0, premiere + 0.5);
    let calques = textes(&b).into_iter().find(|(t, r)| t == "Calques" && r.center().y > e0 && r.center().y < e1).expect("« Calques » sur la rangée d'en-tête");
    for col in Colonne::TOUTES {
        assert!(vus::ou(col.nom()).iter().all(|r| r.center().y < e0 || r.center().y > e1), "« {} » n'est plus sur la rangée d'en-tête", col.nom());
    }
    let replier = vus::ou("Replier tous les calques");
    assert_eq!(replier.len(), 1, "le ⊖ de l'en-tête : {:?}", vus::noms());
    assert!(replier[0].right() <= calques.1.left() + 0.5 && (replier[0].center().y - calques.1.center().y).abs() < 3.0, "⊖ juste avant « Calques »");
    assert_eq!(b.app.keymap.label(crate::layerheads::ACTION_REPLIER), "Maj+Alt+C", "son raccourci (Alt+Maj+C dans le fichier)");
}

#[test]
fn every_layer_has_its_columns_on_the_x_of_the_header() {
    let b = banc_animation();
    let dossier = rang(&b, "Décors");
    assert_eq!(lignes().len(), 5, "cinq lignes : {:?}", lignes());
    for col in Colonne::TOUTES {
        let t = entete(&b, col);
        for (li, _, _) in lignes() {
            let nom = b.app.layers()[li].name.clone();
            let case = case_du_calque(col, li);
            if li == dossier && !matches!(col, Colonne::Afficher | Colonne::Verrouiller) {
                assert!(case.is_none(), "« {nom} » (dossier) n'a pas « {} »", col.nom());
                continue;
            }
            let case = case.unwrap_or_else(|| panic!("« {nom} » a sa case « {} » : {:?}", col.nom(), cases(col)));
            assert!((case.center().x - t.center().x).abs() < 0.5, "« {nom} », « {} » sous l'en-tête : {case:?} / {t:?}", col.nom());
        }
    }
    // (Partie 179.13 : tous les calques sur leur ligne complète, les dossiers sur une ligne fine.)
    for (li, haut, bas) in lignes() {
        let h = if b.app.layers()[li].folder { ROW_FINE } else { ROW_FULL };
        assert!((bas - haut - h).abs() < 0.5, "calque {li} : {haut} {bas}");
    }
}

#[test]
fn a_click_in_a_column_of_a_layer_that_is_not_current_toggles_its_setting_in_one_undo_step() {
    let mut b = banc_animation();
    let ombre = rang(&b, "Ombre");
    assert_ne!(b.app.cur_layer, ombre);
    for (col, lire) in [
        (Colonne::Pelure, (|l: &Layer| l.onion) as fn(&Layer) -> bool),
        (Colonne::Transparence, |l: &Layer| l.keep_alpha),
        (Colonne::Pochoir, |l: &Layer| l.stencil),
        (Colonne::Aplat, |l: &Layer| l.fond.is_some()),
        (Colonne::Camera, |l: &Layer| l.accroche),
        (Colonne::Verrouiller, |l: &Layer| l.locked),
        (Colonne::Afficher, |l: &Layer| l.visible),
    ] {
        let avant = lire(&b.app.layers()[ombre]);
        let n = etapes(&b);
        let case = case_du_calque(col, ombre).unwrap_or_else(|| panic!("« {} » sur Ombre", col.nom()));
        clic(&mut b, case.center(), Modifiers::NONE);
        assert_eq!(lire(&b.app.layers()[ombre]), !avant, "« {} » basculé sur Ombre ({})", col.nom(), b.app.status);
        assert_eq!(etapes(&b), n + 1, "« {} » : une étape d'annulation", col.nom());
        assert_eq!(b.app.cur_layer, rang(&b, "Perso"), "le calque courant ne change pas");
        b.app.run_action("edition.annuler");
        b.chauffer(3);
        assert_eq!(lire(&b.app.layers()[ombre]), avant, "« {} » : Ctrl+Z le remet", col.nom());
    }
}

#[test]
fn the_blend_mode_of_the_current_layer_is_written_and_its_list_sets_every_chosen_layer() {
    let mut b = banc_animation();
    let (perso, ombre) = (rang(&b, "Perso"), rang(&b, "Ombre"));
    let (haut, bas) = ligne(perso);
    let liste = vus::ou("Mode de fusion").into_iter().find(|r| r.center().y > haut && r.center().y < bas).expect("la liste du mode sur la ligne du calque courant");
    assert!(textes(&b).iter().any(|(t, r)| t == "Normal" && liste.contains(r.center())), "« Normal » écrit dans la liste : {liste:?}");
    // Sous la vignette, avant la barre d'opacité et le losange.
    let barre = vus::ou("Opacité du calque").into_iter().find(|r| r.center().y > haut && r.center().y < bas).expect("la barre");
    let losange = vus::ou("Créer une clé").into_iter().find(|r| r.center().y > haut && r.center().y < bas).expect("le losange");
    assert!(liste.right() <= barre.left() + 0.5 && barre.right() <= losange.left() + 0.5, "mode, opacité, losange : {liste:?} {barre:?} {losange:?}");
    // (Partie 179 : la ligne complète sur deux rangées, le mode sur la seconde ; la troisième avant.)
    assert!(liste.top() >= haut + 19.0, "sur la seconde rangée : {liste:?} / {haut}");
    // Deux calques choisis : un mode de la liste va aux deux, en une étape.
    b.app.layer_sel = vec![b.app.layers()[perso].id, b.app.layers()[ombre].id];
    b.chauffer(2);
    let n = etapes(&b);
    clic(&mut b, liste.center(), Modifiers::NONE);
    let produit = textes(&b).into_iter().filter(|(t, _)| t == "Produit").map(|(_, r)| r).last().expect("la liste ouverte");
    clic(&mut b, produit.center(), Modifiers::NONE);
    assert_eq!((b.app.layers()[perso].blend, b.app.layers()[ombre].blend), (BlendMode::Multiply, BlendMode::Multiply), "{}", b.app.status);
    assert_eq!(etapes(&b), n + 1);
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.layers()[perso].blend, b.app.layers()[ombre].blend), (BlendMode::Normal, BlendMode::Normal));
}

#[test]
fn the_behaviour_icons_sit_beside_the_images_show_the_behaviour_and_their_menu_sets_the_chosen_layers() {
    let mut b = banc_animation();
    let (perso, ombre, fond) = (rang(&b, "Perso"), rang(&b, "Ombre"), rang(&b, "Fond"));
    b.app.layers_mut()[perso].post = Behaviour::Loop;
    b.chauffer(3);
    let (nom_avant, avant) = icone_comportement(&b, perso, false).expect("l'icône avant les images de Perso");
    let (nom_apres, apres) = icone_comportement(&b, perso, true).expect("l'icône après les images de Perso");
    assert_eq!(nom_avant, "Avant la première image : Aucun");
    assert_eq!(nom_apres, "Après la dernière image : Boucle");
    // Juste avant l'image 4 et juste après l'image 9, hors des prises du début et de la fin (5 points).
    let (x4, x10) = (x_image(&b, 4), x_image(&b, 10));
    assert!(avant.right() <= x4 - 5.0 && avant.right() >= x4 - 12.0, "avant : {avant:?} / {x4}");
    assert!(apres.left() >= x10 + 5.0 && apres.left() <= x10 + 12.0, "après : {apres:?} / {x10}");
    // Fond commence à l'image 0 : son icône est dans la marge avant la grille.
    let (_, f0) = icone_comportement(&b, fond, false).expect("l'icône avant les images de Fond");
    assert!(f0.right() <= x_image(&b, 0) - 5.0, "dans la marge : {f0:?}");
    assert!(icone_comportement(&b, rang(&b, "Décors"), true).is_none(), "rien pour un dossier");
    // Perso et Ombre choisis : « Va-et-vient » dans le menu de l'icône d'après de Perso les change tous les deux, en une étape.
    b.app.layer_sel = vec![b.app.layers()[perso].id, b.app.layers()[ombre].id];
    b.chauffer(2);
    let n = etapes(&b);
    clic(&mut b, apres.center(), Modifiers::NONE);
    for mot in ["Aucun", "Boucle", "Va-et-vient", "Maintenir"] {
        assert!(textes(&b).iter().any(|(t, _)| t == mot), "« {mot} » dans le menu : {:?}", textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>());
    }
    let va = textes(&b).into_iter().filter(|(t, _)| t == "Va-et-vient").map(|(_, r)| r).last().unwrap();
    clic(&mut b, va.center(), Modifiers::NONE);
    assert_eq!((b.app.layers()[perso].post, b.app.layers()[ombre].post), (Behaviour::PingPong, Behaviour::PingPong), "{}", b.app.status);
    assert_eq!(etapes(&b), n + 1, "une étape");
    assert_eq!(icone_comportement(&b, perso, true).map(|i| i.0).as_deref(), Some("Après la dernière image : Va-et-vient"));
    b.app.run_action("edition.annuler");
    assert_eq!((b.app.layers()[perso].post, b.app.layers()[ombre].post), (Behaviour::Loop, Behaviour::Nothing));
}

/// Les blocs pleins peints dans la grille sur la hauteur `haut..bas`.
fn blocs(b: &Banc, haut: f32, bas: f32) -> Vec<Rect> {
    fn chercher(s: &Shape, out: &mut Vec<Rect>) {
        match s {
            Shape::Rect(r) if r.fill.a() > 0 => out.push(r.rect),
            Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut out);
    }
    out.retain(|r| r.top() >= haut && r.bottom() <= bas && r.height() >= 6.0 && r.width() >= 8.0);
    out
}

#[test]
fn the_key_blocks_keep_their_look_beside_the_behaviour_icons() {
    let b = banc_animation();
    let ombre = rang(&b, "Ombre");
    let (haut, bas) = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().1.iter().find(|r| r.0 == ombre).map(|r| (r.1, r.2))).unwrap();
    let (x2, x8) = (x_image(&b, 2), x_image(&b, 8));
    // Le bloc de la clé d'Ombre (images 2 à 7) : de l'image 2 à l'image 8, à un point près de chaque bord, comme avant.
    let bloc = blocs(&b, haut, bas).into_iter().find(|r| (r.left() - (x2 + 1.0)).abs() < 0.5).expect("le bloc de la clé");
    assert!((bloc.right() - (x8 - 1.0)).abs() < 0.5, "{bloc:?} / {x8}");
    assert!((bloc.top() - (haut + 3.0)).abs() < 0.5, "{bloc:?}");
    for apres in [false, true] {
        let (_, i) = icone_comportement(&b, ombre, apres).unwrap();
        assert!(!i.intersects(bloc), "l'icône ne passe pas sur le bloc : {i:?} / {bloc:?}");
    }
}

#[test]
fn fold_all_puts_every_layer_on_one_line_and_unfold_gives_back_the_three_rows() {
    let mut b = banc_animation();
    let perso = rang(&b, "Perso");
    let n = etapes(&b);
    let x0 = x_image(&b, 0);
    clic(&mut b, vus::ou("Replier tous les calques")[0].center(), Modifiers::NONE);
    // (Partie 179.10 : le réglage « Déplier le calque actif », éteint.)
    assert!(b.app.calques_tout_replies(), "replié");
    b.chauffer(2);
    assert!((x_image(&b, 0) - x0).abs() < 0.5, "la grille ne bouge pas : {x0} / {}", x_image(&b, 0));
    let droite = entete(&b, Colonne::Camera).right();
    assert!(droite < x0 - crate::timeline::MARGE_AVANT, "toute la colonne est à gauche de la grille : {droite} / {x0}");
    assert!(lignes().iter().all(|l| (l.2 - l.1 - ROW_FINE).abs() < 0.5), "toutes les lignes fines : {:?}", lignes());
    assert!(case_du_calque(Colonne::Pelure, perso).is_some(), "les colonnes du calque courant restent");
    assert!(vus::ou("Mode de fusion").is_empty() || vus::ou("Mode de fusion").iter().all(|r| { let (h, bs) = ligne(perso); r.center().y < h || r.center().y > bs }), "plus de liste du mode");
    assert_eq!(etapes(&b), n, "un réglage d'affichage : rien à annuler");
    // ⊕ de l'en-tête : dépliés.
    clic(&mut b, vus::ou("Déplier les calques")[0].center(), Modifiers::NONE);
    assert!(!b.app.calques_tout_replies());
    b.chauffer(2);
    let (h, bs) = ligne(perso);
    assert!((bs - h - ROW_FULL).abs() < 0.5);
    // Le raccourci fait de même ; ⊕ sur la ligne d'un autre calque le rend courant, déplié.
    b.app.run_action(crate::layerheads::ACTION_REPLIER);
    assert!(b.app.calques_tout_replies());
    b.chauffer(3);
    let ombre = rang(&b, "Ombre");
    let (h, bs) = ligne(ombre);
    let plus = vus::ou("Déplier le calque").into_iter().find(|r| r.center().y > h && r.center().y < bs).expect("le ⊕ de la ligne d'Ombre");
    clic(&mut b, plus.center(), Modifiers::NONE);
    assert_eq!(b.app.cur_layer, ombre);
    assert!(!b.app.calques_tout_replies());
    b.chauffer(2);
    let (h, bs) = ligne(ombre);
    assert!((bs - h - ROW_FULL).abs() < 0.5);
    // Gardé dans les réglages du poste (pas dans le projet).
    let texte = serde_json::to_string(&b.app.settings).unwrap();
    assert!(texte.contains("timeline_tout_replie"), "{texte}");
}

/// La colonne des calques seule, peinte dans une interface plus étroite que sa largeur la plus petite (`HEAD_W`) : ce que les
/// bulles ont noté, et le rectangle de la colonne.
fn colonne_au_minimum(app: &mut crate::app::App, ctx: &egui::Context) -> Rect {
    let mut colonne = Rect::NOTHING;
    for k in 0..4 {
        vus::vider();
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 900.0))), time: Some(k as f64 / 60.0), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                colonne = app.timeline_heads(ui, crate::timeline::HEADER_H, 0.0);
            });
        });
    }
    colonne
}

#[test]
fn nothing_overflows_or_overlaps_at_the_smallest_width_of_the_column() {
    let mut app = crate::pistes_tests::app_with("colonne174", doc_174());
    app.cur_layer = app.layers().iter().position(|l| l.name == "Perso").unwrap();
    // Un calque avec des effets (la case fx), un calque partagé n'est pas nécessaire : tout ce qui peut être là l'est.
    app.layers_mut()[0].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::Boil));
    let cur = app.cur_layer;
    app.layers_mut()[cur].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::Boil));
    app.regler_mode_calques(app.cur_layer, BlendMode::Overlay);
    let ctx = egui::Context::default();
    let colonne = colonne_au_minimum(&mut app, &ctx);
    assert!((colonne.width() - crate::layerheads::HEAD_W).abs() < 0.5, "la colonne à sa plus petite largeur : {colonne:?}");
    let mut tout: Vec<(String, Rect)> = Vec::new();
    for n in vus::noms() {
        for r in vus::ou(&n) {
            if !tout.iter().any(|(m, q)| *m == n && *q == r) {
                tout.push((n.clone(), r));
            }
        }
    }
    assert!(tout.len() > 30, "{tout:?}");
    let lignes = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.clone());
    for (n, r) in &tout {
        assert!(r.right() <= colonne.right() + 0.5 && r.left() >= colonne.left() - 0.5, "« {n} » sort de la colonne : {r:?} / {colonne:?}");
    }
    // Deux éléments de la même ligne (ou de l'en-tête) ne se chevauchent pas (les bulles du ⋮ et des boutons de la timeline mis à part,
    // au-dessus de l'en-tête des colonnes).
    let premiere = lignes.iter().map(|l| l.1).fold(f32::MAX, f32::min);
    let zone = |r: &Rect| lignes.iter().position(|l| r.center().y > l.1 && r.center().y < l.2).map(|k| k as i32).unwrap_or(if r.center().y > premiere - crate::layerheads::ENTETE_H { -1 } else { -2 });
    for (i, (n, r)) in tout.iter().enumerate() {
        if zone(r) == -2 {
            continue;
        }
        for (m, q) in &tout[i + 1..] {
            if zone(q) == zone(r) && r.shrink(0.6).intersects(q.shrink(0.6)) {
                panic!("« {n} » {r:?} et « {m} » {q:?} se chevauchent");
            }
        }
    }
}

/// Pour les images : un plan sans dossier ou avec peu de calques reçoit, en mémoire seulement (rien n'est enregistré), un dossier
/// « Décors » avec une copie de son calque dessiné dedans (« Fond »), et deux autres copies plus courtes (« Perso », « Ombre ») ; les
/// dessins sont copiés.
fn enrichir(doc: &mut Document, plan: usize) {
    let p = &doc.panels[plan];
    if p.layers.iter().any(|l| l.folder) && p.layers.len() >= 4 {
        return;
    }
    let Some(base) = p.layers.iter().find(|l| !l.folder && l.video.is_none() && l.frames.iter().any(Option::is_some)) else { return };
    let (frames, kind) = (base.frames.clone(), base.kind);
    let n = frames.len();
    let copie = |doc: &mut Document, nom: &str, de: usize, a: usize| {
        let mut vus: std::collections::HashMap<crate::document::CelId, crate::document::CelId> = Default::default();
        let mut l = Layer::new(nom, kind);
        l.frames = frames
            .iter()
            .enumerate()
            .map(|(f, c)| {
                c.filter(|_| f >= de && f < a).map(|c| {
                    *vus.entry(c).or_insert_with(|| {
                        let neuf = doc.new_cel(kind);
                        if let Some(src) = doc.cels.get(&c).cloned() {
                            doc.cels.insert(neuf, src);
                        }
                        neuf
                    })
                })
            })
            .collect();
        l
    };
    let dossier = Layer::new_folder("Décors");
    let mut fond = copie(doc, "Fond", 0, n);
    fond.parent = Some(dossier.id);
    fond.locked = true;
    let perso = copie(doc, "Perso", n / 6, n * 5 / 6);
    let mut ombre = copie(doc, "Ombre", n / 10, n / 2);
    ombre.onion = true;
    ombre.group = 10;
    let layers = &mut doc.panels[plan].layers;
    layers.insert(0, dossier);
    layers.insert(0, fond);
    layers.push(perso);
    layers.push(ombre);
    crate::folders::normalize(layers);
}

/// Les images de la partie 174, à taille réelle (un pixel de l'écran du banc pour un point, aucun agrandissement), dans le dossier
/// `SHOT_CAPTURE_DIR`, sur le projet `SHOT_CAPTURE_PROJET` (une copie : le banc n'enregistre rien ; sinon `doc_174`). Sans carte
/// graphique : rien.
#[test]
#[ignore]
fn colonne_calques_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let charger = || match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet").doc,
        None => doc_174(),
    };
    let doc = charger();
    let mut b = crate::perf_tests::banc_taille(doc, true, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.settings.calques_replies = false;
    // (Le plan le plus riche en calques, et un dossier s'il y en a un.)
    let plan = (0..b.app.doc.panels.len()).max_by_key(|&p| (b.app.doc.panels[p].layers.iter().any(|l| l.folder), b.app.doc.panels[p].layers.len())).unwrap_or(0);
    enrichir(&mut b.app.doc, plan);
    b.app.cur_panel = plan;
    b.app.cur_layer = b.app.layers().iter().position(|l| l.name == "Perso").or_else(|| b.app.layers().iter().rposition(|l| !l.folder && l.frames.iter().any(Option::is_some))).unwrap_or(0);
    b.app.cur_frame = 0;
    b.chauffer(10);
    b.garder_formes = true;
    let sauver = |b: &mut Banc, nom: &str, zone: Rect| {
        b.chauffer(4);
        let Some(rgba) = b.capturer() else { return println!("CAPTURE pas de carte graphique : {nom}") };
        let img = image::RgbaImage::from_raw(1920, 1080, rgba).expect("image");
        let z = zone.intersect(Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0)));
        let crop = image::imageops::crop_imm(&img, z.left() as u32, z.top() as u32, z.width() as u32, z.height() as u32).to_image();
        crop.save(dossier.join(nom)).expect("png");
        println!("CAPTURE {nom} {}x{}", crop.width(), crop.height());
    };
    let timeline = |_: &Banc| {
        let l = lignes();
        let haut = l.iter().map(|l| l.1).fold(f32::MAX, f32::min) - crate::timeline::HEADER_H - 4.0;
        let bas = l.iter().map(|l| l.2).fold(0.0, f32::max) + 80.0;
        Rect::from_min_max(pos2(0.0, haut.max(0.0)), pos2(1920.0, bas.min(1080.0)))
    };
    let colonne = |_: &Banc| {
        let l = lignes();
        let haut = l.iter().map(|l| l.1).fold(f32::MAX, f32::min) - crate::layerheads::ENTETE_H - 4.0;
        let bas = l.iter().map(|l| l.2).fold(0.0, f32::max) + 4.0;
        let droite = Colonne::TOUTES.iter().filter_map(|&c| cases(c).into_iter().map(|r| r.right()).reduce(f32::max)).fold(0.0, f32::max);
        Rect::from_min_max(pos2(0.0, haut), pos2(droite + 120.0, bas))
    };
    let z = timeline(&b);
    sauver(&mut b, "174-1-timeline-animation.png", z);
    let z = colonne(&b);
    sauver(&mut b, "174-2-colonne-des-calques.png", z);
    // Un calque en Boucle après ses images, puis le menu ouvert sur son icône d'après.
    let li = b.app.cur_layer;
    b.app.layers_mut()[li].post = Behaviour::Loop;
    b.chauffer(4);
    let z = timeline(&b);
    sauver(&mut b, "174-3-calque-en-boucle-apres.png", z);
    if let Some((_, i)) = icone_comportement(&b, li, true) {
        clic(&mut b, i.center(), Modifiers::NONE);
        let zone = Rect::from_min_max(pos2((i.left() - 200.0).max(0.0), (i.top() - 160.0).max(0.0)), pos2(i.right() + 260.0, (i.bottom() + 160.0).min(1080.0)));
        sauver(&mut b, "174-4-menu-apres-les-images.png", zone);
        b.image(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.image(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.chauffer(3);
    }
    // Tout replié.
    b.app.settings.calques_replies = true;
    b.chauffer(4);
    let z = timeline(&b);
    sauver(&mut b, "174-5-tout-replie.png", z);
    b.app.settings.calques_replies = false;
    b.chauffer(4);
    // La piste du Storyboard.
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.chauffer(8);
    let (h, o) = (vus::ou("Commandes du calque"), vus::ou("Opacité du calque"));
    if let (Some(h), Some(o)) = (h.first(), o.first()) {
        let g = vus::ou("Largeur de la colonne").first().copied().unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(340.0, 10.0)));
        sauver(&mut b, "174-7-piste-storyboard.png", Rect::from_min_max(pos2(0.0, h.top() - 30.0), pos2(g.right() + 300.0, o.bottom() + 60.0)));
    }
}


/// L'image de la timeline dans la plus petite fenêtre de FEANOR (`TAILLE_MIN_TRAVAIL`), à part : deux bancs sur la carte graphique
/// dans le même essai ne vont pas ensemble.
#[test]
#[ignore]
fn colonne_calques_capture_etroite() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let charger = || match std::env::var_os("SHOT_CAPTURE_PROJET") {
        Some(p) => crate::project::load(std::path::Path::new(&p)).expect("le projet").doc,
        None => doc_174(),
    };
    let plan = { let d = charger(); (0..d.panels.len()).max_by_key(|&p| (d.panels[p].layers.iter().any(|l| l.folder), d.panels[p].layers.len())).unwrap_or(0) };
    let [lmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let (lmin, hmin) = (lmin as u32, hmin as u32);
    let mut doc = charger();
    enrichir(&mut doc, plan);
    let mut etroit = crate::perf_tests::banc_taille(doc, true, lmin, hmin);
    etroit.app.set_mode(crate::app::Mode::Animation);
    etroit.app.settings.calques_replies = false;
    let li = etroit.app.doc.panels[plan].layers.iter().position(|l| l.name == "Perso").unwrap_or(0);
    (etroit.app.cur_panel, etroit.app.cur_layer) = (plan, li);
    etroit.app.layers_mut()[li].post = Behaviour::Loop;
    etroit.garder_formes = true;
    etroit.chauffer(10);
    let l = lignes();
    let haut = l.iter().map(|l| l.1).fold(f32::MAX, f32::min) - crate::timeline::HEADER_H - 4.0;
    let bas = l.iter().map(|l| l.2).fold(0.0, f32::max) + 6.0;
    etroit.chauffer(4);
    if let Some(rgba) = etroit.capturer() {
        let img = image::RgbaImage::from_raw(lmin, hmin, rgba).expect("image");
        let z = Rect::from_min_max(pos2(0.0, haut.max(0.0)), pos2(lmin as f32, bas.min(hmin as f32)));
        let crop = image::imageops::crop_imm(&img, 0, z.top() as u32, z.width() as u32, z.height() as u32).to_image();
        crop.save(dossier.join("174-6-timeline-largeur-minimale.png")).expect("png");
        let w = vus::ou("Commandes du calque").iter().map(|r| r.right()).fold(0.0, f32::max);
        println!("CAPTURE 174-6-timeline-largeur-minimale.png {}x{} (colonne jusqu'à {w:.0})", crop.width(), crop.height());
    }
}

