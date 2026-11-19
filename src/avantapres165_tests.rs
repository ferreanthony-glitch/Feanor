//! Partie 165 : l'avant | après des Effets montre le vrai calque, calculé en petit au fil de fond, jamais sur celui de l'interface ;
//! le schéma de la partie 164 reste le repli d'un calque vide.

use eframe::egui::{pos2, Event, Modifiers, Rect};

use crate::avantapres165 as a;
use crate::bulles::vus;
use crate::document::{Document, Layer, LayerKind};
use crate::dock::Pane;
use crate::fx::EffectKind;
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::{clic, etapes, glisser, pointer_button};
use crate::vector::{Element, VPoint};
use crate::workspace::Espace;

/// Un plan de 960 × 540, un calque vectoriel « Dessin » ; avec `trait_`, un gros trait rouge en travers de la feuille.
fn banc_165(trait_: bool) -> Banc {
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Vector)];
    doc.panels[0].set_length(24);
    if trait_ {
        let cel = doc.new_cel(LayerKind::Vector);
        doc.panels[0].layers[0].frames = vec![Some(cel); 24];
        let points = (0..=20).map(|k| VPoint { x: 200.0 + 28.0 * k as f32, y: 270.0 + 60.0 * ((k as f32) * 0.3).sin(), r: 9.0 }).collect();
        let t = Element::Stroke { points, color: [210, 20, 20], opacity: 1.0, hardness: 1.0, brush: None, trace: Default::default() };
        doc.cels.get_mut(&cel).unwrap().vector.as_mut().unwrap().elements = vec![t];
        doc.cels.get_mut(&cel).unwrap().rerender();
    }
    let mut b = banc(doc, false);
    b.app.set_espace(Espace::Storyboard);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.chauffer(6);
    b.app.show_pane(Pane::Effects);
    b.chauffer(4);
    b
}

fn ou(nom: &str) -> Rect {
    vus::ou(nom).into_iter().max_by(|a, b| a.center().x.total_cmp(&b.center().x)).unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

/// Laisse le fil de fond finir et l'écran se refaire, jusqu'à ce que l'aperçu soit à jour (ou qu'on abandonne).
fn attendre(b: &mut Banc) -> bool {
    for _ in 0..40 {
        crate::fxcache::settle();
        b.chauffer(2);
        if b.app.effets_ui.apercu.a_jour() && b.app.effets_ui.apercu.images().is_some() {
            return true;
        }
    }
    false
}

/// Les pixels rouges (ceux du trait) d'une image.
fn rouges(im: &eframe::egui::ColorImage) -> usize {
    im.pixels.iter().filter(|c| c.a() > 150 && c.r() > 120 && c.g() < 90 && c.b() < 90).count()
}

/// L'opacité totale d'une image (une lueur en ajoute autour du trait).
fn encre(im: &eframe::egui::ColorImage) -> u64 {
    im.pixels.iter().map(|c| c.a() as u64).sum()
}

fn ajouter_lueur(b: &mut Banc) {
    clic(b, ou("Lueur").center(), Modifiers::NONE);
    assert_eq!(b.app.layers()[0].fx.iter().map(|e| e.kind).collect::<Vec<_>>(), vec![EffectKind::Glow]);
}

#[test]
fn the_before_after_shows_the_real_layer_and_the_after_side_follows_the_strength() {
    let mut b = banc_165(true);
    ajouter_lueur(&mut b);
    assert!(attendre(&mut b), "l'aperçu du vrai calque est arrivé du fil de fond");
    let (avant, apres) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    assert!(rouges(&avant) > 50, "l'avant montre les pixels du trait rouge du calque, pas le schéma (un trait gris) : {}", rouges(&avant));
    assert!(avant.size[0] >= 150 && avant.size[0] <= 1000, "une image à peu près de la largeur du panneau : {:?}", avant.size);
    assert!(encre(&apres) > encre(&avant), "l'après a la lueur autour du trait");
    // Plus fort : l'après change, l'avant non.
    b.app.layers_mut()[0].fx[0].set_param(1, 200.0);
    b.chauffer(2);
    assert!(attendre(&mut b));
    let (avant2, apres2) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    // (Le cadre peut avoir bougé d'un point avec la largeur du panneau : on compare l'après à l'avant de la même image.)
    assert!(rouges(&avant2) > 50, "l'avant reste le calque sans l'effet");
    let gain = |av: &eframe::egui::ColorImage, ap: &eframe::egui::ColorImage| encre(ap) as f64 / encre(av).max(1) as f64;
    assert!(gain(&avant2, &apres2) > gain(&avant, &apres) * 1.05, "le côté « après » suit la force : {} / {}", gain(&avant2, &apres2), gain(&avant, &apres));
}

#[test]
fn dragging_the_bar_never_renders_on_the_interface_thread_and_keeps_the_last_image() {
    let mut b = banc_165(true);
    ajouter_lueur(&mut b);
    assert!(attendre(&mut b));
    let (avant, apres) = b.app.effets_ui.apercu.images().map(|(x, y)| (x.clone(), y.clone())).unwrap();
    let r = ou("Force de l'effet");
    let (x0, z) = (pos2(r.center().x, r.center().y), pos2(r.left() + r.width() * 0.1, r.center().y));
    // (L'aperçu seul : la vue de dessin de ce banc d'essai, sans carte graphique, compose encore la lueur du calque sur ce fil.)
    let calculs = a::calculs_de_l_apercu();
    let pas = etapes(&b);
    b.image(vec![Event::PointerMoved(x0)], 1.0 / 60.0);
    b.image(vec![pointer_button(x0, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(x0 + (z - x0) * (k as f32 / 8.0))], 1.0 / 60.0);
        assert!(b.app.effets_ui.apercu.images().is_some(), "pendant le glissé, la dernière image prête reste montrée (jamais le schéma)");
    }
    b.image(vec![pointer_button(z, false, Modifiers::NONE)], 1.0 / 60.0);
    assert_eq!(a::calculs_de_l_apercu(), calculs, "aucun effet ni aucune réduction calculés sur le fil de l'interface pendant le geste");
    assert_eq!(etapes(&b) , pas + 1, "le geste reste une seule étape d'annulation");
    assert!(attendre(&mut b), "l'image du nouveau réglage arrive après le geste");
    let (avant2, apres2) = b.app.effets_ui.apercu.images().unwrap();
    let gain = |av: &eframe::egui::ColorImage, ap: &eframe::egui::ColorImage| encre(ap) as f64 / encre(av).max(1) as f64;
    assert!(gain(avant2, apres2) > gain(&avant, &apres) * 1.05, "la lueur plus forte se voit");
}

#[test]
fn an_empty_layer_keeps_the_drawing_of_the_effect() {
    let mut b = banc_165(false);
    ajouter_lueur(&mut b);
    crate::fxcache::settle();
    b.chauffer(4);
    assert!(b.app.effets_ui.apercu.images().is_none(), "rien de dessiné : le schéma de la partie 164 reste");
    assert!(!vus::ou("Force de l'effet").is_empty(), "et la barre se tire toujours");
}

// ------------------------------------------------------------------------------------ 165.3 : la ligne du calque en texte, de retour

/// Les textes peints à la dernière image.
fn textes(b: &Banc) -> Vec<(String, eframe::egui::Rect)> {
    fn rec(s: &eframe::egui::epaint::Shape, out: &mut Vec<(String, eframe::egui::Rect)>) {
        match s {
            eframe::egui::epaint::Shape::Text(t) => out.push((t.galley.job.text.clone(), eframe::egui::Rect::from_min_size(t.pos, t.galley.size()))),
            eframe::egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| rec(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        rec(&s.shape, &mut out);
    }
    out
}

/// Partie 165.3 (Anthony : « Je préfère la version texte classique. Reviens en arrière. ») : la ligne du calque courant de la timeline
/// écrit de nouveau « 100 % » (la barre d'opacité) et « Normal » (la liste « Normal ▾ » avec ses flèches) ; la piste du calque actif du
/// Storyboard, sa barre « 100 % » et son menu des modes, en texte. Plus aucune pastille de mode, nulle part.
#[test]
fn the_layer_rows_show_100_and_normal_again() {
    // Timeline (vue Animation).
    let mut b = banc_165(true);
    b.app.set_espace(Espace::Animation);
    b.chauffer(4);
    b.garder_formes = true;
    b.chauffer(2);
    let t = textes(&b);
    b.garder_formes = false;
    let haut = b.app.canvas_rect.bottom();
    assert!(t.iter().any(|(s, r)| s == "100 %" && r.top() > haut), "« 100 % » dans la ligne du calque : {:?}", t.iter().map(|x| &x.0).collect::<Vec<_>>());
    assert!(t.iter().any(|(s, r)| s.contains("Normal") && r.top() > haut), "« Normal ▾ » dans la ligne du calque");
    for nom in ["Mode précédent", "Mode suivant", "Mode de fusion", "Opacité du calque"] {
        assert!(!vus::ou(nom).is_empty(), "« {nom} » : {:?}", vus::noms());
    }
    // La liste est en texte : on l'ouvre, aucune pastille « Mode : … ».
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    assert!(vus::noms().iter().all(|n| !n.starts_with("Mode : ")), "plus de pastilles : {:?}", vus::noms());
    // Glisser sur la barre « 100 % » règle l'opacité, une étape ; Ctrl+Z la rend.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    let r = ou("Opacité du calque");
    assert!(r.width() > 60.0, "la barre, pas la miniature : {r:?}");
    let avant = etapes(&b);
    glisser(&mut b, r.center(), pos2(r.left() + r.width() * 0.25, r.center().y));
    let o = b.app.layers()[0].opacity_at(b.app.cur_frame);
    assert!((o - 0.25).abs() < 0.05, "la barre suit le pointeur : {o}");
    assert_eq!(etapes(&b), avant + 1, "un geste, une étape");
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(b.app.layers()[0].opacity_at(b.app.cur_frame), 1.0);

    // Piste du calque actif (vue Storyboard) : la barre « 100 % » et le menu des modes, en texte.
    let mut b = banc_165(true);
    b.garder_formes = true;
    b.chauffer(2);
    let t = textes(&b);
    b.garder_formes = false;
    assert!(t.iter().any(|(s, _)| s == "100 %"), "« 100 % » sur la piste : {:?}", t.iter().map(|x| &x.0).collect::<Vec<_>>());
    clic(&mut b, ou("Mode de fusion").center(), Modifiers::NONE);
    b.garder_formes = true;
    b.chauffer(2);
    let t = textes(&b);
    b.garder_formes = false;
    assert!(t.iter().any(|(s, _)| s.contains("Normal")) && t.iter().any(|(s, _)| s.contains("Produit")), "le menu des modes en texte : {:?}", t.iter().map(|x| &x.0).collect::<Vec<_>>());
    assert!(vus::noms().iter().all(|n| !n.starts_with("Mode : ")), "plus de pastilles");
}

#[test]
fn the_framing_follows_the_drawing_and_the_size_the_panel() {
    // Un dessin dans un coin : le cadre l'entoure, de la forme de l'aperçu, dans la feuille.
    let c = a::cadrage(Some((0, 0, 0, 0)), (1920, 1080), 3.0).unwrap();
    assert!(c.left() >= 0.0 && c.top() >= 0.0 && c.right() <= 1920.0 && c.bottom() <= 1080.0, "{c:?}");
    assert!((c.width() / c.height() - 3.0).abs() < 0.01, "la forme de l'aperçu : {c:?}");
    assert!(c.width() >= 1920.0 * 0.35 - 0.5, "au moins un bon tiers de la feuille : {c:?}");
    // Toute la feuille dessinée, un aperçu plus haut que large : la feuille entière, pas plus.
    let c = a::cadrage(Some((-4, -4, 40, 40)), (1920, 1080), 0.5).unwrap();
    assert!(c.width() <= 1920.0 && c.height() <= 1080.0);
    assert!(a::cadrage(None, (1920, 1080), 3.0).is_none(), "rien de dessiné : pas de cadre");
    // La taille de calcul : à peu près la largeur du panneau, jamais plus petite.
    assert_eq!(a::niveau(1920, 340.0), 2, "1 920 px montrés sur 340 : un quart (480 px)");
    assert_eq!(a::niveau(1920, 2000.0), 0);
    assert_eq!(a::niveau(64, 340.0), 0);
}

#[test]
fn an_effect_that_cannot_be_shown_small_keeps_the_drawing() {
    // Un effet du trait (il lit les traits en pleine résolution) : le schéma, pas deux moitiés pareilles.
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.fx.push(crate::fx::Effect::new(EffectKind::Sketch));
    assert!(a::piles(&l, 0, 0).is_none());
    // Une lueur, si : sans elle d'un côté, avec elle de l'autre.
    l.fx = vec![crate::fx::Effect::new(EffectKind::Glow)];
    let (sans, avec) = a::piles(&l, 0, 0).unwrap();
    assert!(sans.is_empty() && avec.len() == 1 && avec[0].kind == EffectKind::Glow);
}

// ------------------------------------------------------------------------------------ 165.4 : la ligne du calque courant, toujours entière

/// Les textes peints à la dernière image qui se voient en entier : dans leur découpe et dans la fenêtre.
fn textes_entiers(b: &Banc) -> Vec<(String, Rect)> {
    fn rec(s: &eframe::egui::epaint::Shape, clip: Rect, ecran: Rect, out: &mut Vec<(String, Rect)>) {
        match s {
            eframe::egui::epaint::Shape::Text(t) => {
                let r = Rect::from_min_size(t.pos, t.galley.size());
                if clip.expand(0.5).contains_rect(r) && ecran.contains_rect(r) {
                    out.push((t.galley.job.text.clone(), r));
                }
            }
            eframe::egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| rec(s, clip, ecran, out)),
            _ => {}
        }
    }
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), b.taille);
    let mut out = Vec::new();
    for s in &b.formes {
        rec(&s.shape, s.clip_rect, ecran, &mut out);
    }
    out
}

/// Partie 165.4 (Anthony : « je veux que cette version toujours visible », « jamais repliée ») : la fenêtre la plus petite permise
/// (960 × 600), chaque disposition de l'espace Animation (Standard, Dessin, Timeline), vingt-cinq calques et le calque courant tout
/// en bas de la pile, hors de ce que la timeline montre : sa ligne complète reste entière à l'écran, « 100 % », « Normal » et les
/// flèches lisibles et cliquables. Puis le calque courant dans un groupe caché, puis dans un dossier fermé : sa ligne aussi.
#[test]
fn the_current_layer_row_stays_whole_in_the_smallest_timeline() {
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = (0..25).map(|k| Layer::new(&format!("Calque {k}"), LayerKind::Vector)).collect();
    doc.panels[0].set_length(24);
    let [lmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut b = crate::perf_tests::banc_taille(doc, false, lmin as u32, hmin as u32);
    b.app.set_espace(Espace::Animation);
    b.chauffer(6);
    for disposition in ["espace.standard", "espace.dessin", "espace.timeline"] {
        b.app.run_action(disposition);
        // (Le calque du bas de la pile : sa place est sous ce que la timeline montre.)
        b.app.select_layer(0);
        b.chauffer(6);
        b.garder_formes = true;
        b.chauffer(2);
        let t = textes_entiers(&b);
        b.garder_formes = false;
        let bas = b.app.canvas_rect.bottom();
        let entier = |mot: &str| t.iter().any(|(s, r)| s.contains(mot) && r.top() > bas);
        assert!(entier("100 %") && entier("Normal") && entier("Calque 0"), "{disposition} : la ligne du calque courant entière : {:?}", t.iter().filter(|x| x.1.top() > bas).map(|x| &x.0).collect::<Vec<_>>());
        // Les flèches, cliquables : « Mode suivant » change le mode, une étape ; Ctrl+Z le rend.
        let fleche = ou("Mode suivant");
        assert!(fleche.top() > bas && fleche.bottom() <= b.taille.y, "{disposition} : {fleche:?}");
        let avant = etapes(&b);
        clic(&mut b, fleche.center(), Modifiers::NONE);
        assert_ne!(b.app.layers()[0].blend, crate::blend::BlendMode::Normal, "{disposition} : la flèche a changé le mode");
        assert_eq!(etapes(&b), avant + 1);
        b.app.run_action("edition.annuler");
        b.chauffer(3);
        assert_eq!(b.app.layers()[0].blend, crate::blend::BlendMode::Normal);
    }
    // Dans un groupe caché de la timeline, puis dans un dossier fermé : la ligne du calque courant reste.
    let g = 3;
    b.app.layers_mut()[0].group = g as _;
    b.app.set_group_shown(g, false);
    b.chauffer(4);
    assert!(crate::layerheads::rows_of(b.app.layers(), &b.app.doc.groups, 0).iter().any(|r| r.li == 0 && r.h == crate::layerheads::ROW_FULL), "groupe caché");
    assert!(!vus::ou("Mode suivant").is_empty());
}

#[test]
fn the_current_row_is_pulled_back_into_view() {
    use crate::layerheads::ligne_toujours_visible as v;
    let vue = Rect::from_min_max(pos2(0.0, 100.0), pos2(300.0, 300.0));
    let r = |y: f32| Rect::from_min_size(pos2(0.0, y), eframe::egui::vec2(300.0, 56.0));
    assert_eq!(v(r(150.0), vue), r(150.0), "à sa place quand elle tient");
    assert_eq!(v(r(400.0), vue), r(244.0), "sous la vue : collée en bas");
    assert_eq!(v(r(20.0), vue), r(100.0), "au-dessus : collée en haut");
}

// ------------------------------------------------------------------------------------ 165.5 : les boutons montés dans la colonne

/// Les noms (bulles) des commandes de `barre` montrées dans la ligne : boutons et états qui répondent, menus.
fn noms_de_la_barre(b: &Banc, barre: crate::barres::Barre) -> Vec<String> {
    use crate::barres::Genre;
    let reglage = crate::barres::ReglageBarre::default();
    // (Partie 179 : dans le Storyboard, ce qui ne tient pas sur la ligne de la barre du montage est sous « Plus… ».)
    let caches: Vec<&str> = if barre == crate::barres::Barre::Montage { crate::barres::DEBORDENT.with(|d| d.borrow().clone()) } else { Vec::new() };
    crate::barres::dans_la_barre(barre, &reglage)
        .into_iter()
        .filter(|c| !caches.contains(&c.id))
        .filter_map(|c| match c.genre {
            Genre::Menu => Some(c.nom.to_string()),
            Genre::Bouton | Genre::Etat => {
                let v = b.app.vue_de(barre, c);
                v.actif.then_some(v.nom)
            }
            _ => None,
        })
        .collect()
}

/// Chaque commande de la ligne est à l'écran, entière, dans la colonne (`colonne` : ses bords gauche et droit) et au-dessus de
/// `sous` (le haut de ce qui suit les boutons dans la colonne) ; rend le rectangle de chacune.
fn boutons_dans_la_colonne(b: &Banc, barre: crate::barres::Barre, colonne: (f32, f32), sous: f32) -> Vec<(String, Rect)> {
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), b.taille);
    noms_de_la_barre(b, barre)
        .into_iter()
        .map(|nom| {
            let r = *vus::ou(&nom).iter().filter(|r| r.left() >= colonne.0 - 1.0 && r.right() <= colonne.1 + 1.0).min_by(|a, c| a.top().total_cmp(&c.top())).unwrap_or_else(|| panic!("« {nom} » pas dans la colonne {colonne:?} : {:?}", vus::ou(&nom)));
            assert!(ecran.contains_rect(r), "« {nom} » entier à l'écran : {r:?}");
            assert!(r.bottom() <= sous + 1.0, "« {nom} » en haut de la colonne, au-dessus de {sous} : {r:?}");
            (nom, r)
        })
        .collect()
}

/// Partie 165.5 (Anthony : « Monter les boutons » ; « mettre les boutons sur la même ligne que la timeline » ; « toujours garder
/// cette colonne avec un maximum d'information. On peut verrouiller la taille minimale de la colonne s'il faut. ») : vue
/// Storyboard, fenêtre la plus petite. Plus de barre au-dessus de la bande : ses boutons sont sur une ligne en haut de la colonne,
/// au-dessus de « Caméra », tous entiers et cliquables ; la colonne ne se rétrécit pas sous leur largeur, même tirée.
#[test]
fn in_the_storyboard_the_band_buttons_sit_in_the_column_on_one_line_and_the_column_keeps_its_width() {
    use crate::barres::Barre;
    let [lmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Vector)];
    doc.panels[0].set_length(24);
    let mut b = crate::perf_tests::banc_taille(doc, false, lmin as u32, hmin as u32);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(8);
    // Une colonne réglée trop étroite (réglage d'avant) : elle est élargie à la ligne des boutons.
    b.app.montage_head_w = crate::montagehead::HEAD_MIN;
    b.chauffer(6);
    let grip = ou("Largeur de la colonne");
    let camera = vus::ou("Caméra").into_iter().filter(|r| r.right() <= grip.right() + 1.0).min_by(|a, c| a.top().total_cmp(&c.top()));
    let haut_des_pistes = camera.map_or(grip.top() + 40.0, |c| c.top());
    let boutons = boutons_dans_la_colonne(&b, Barre::Montage, (grip.left() - b.app.montage_head_w, grip.right()), haut_des_pistes);
    assert!(boutons.len() >= 3, "{boutons:?}");
    // En haut de la colonne, en face de la règle : la barre du haut n'existe plus. (Partie 179 : sur une seule ligne, même au plus
    // étroit ; ce qui n'y tient pas est sous « Plus… », partie 173.5 changée.)
    let y = boutons[0].1.center().y;
    assert!(boutons.iter().all(|(_, r)| (r.center().y - y).abs() < 2.0), "une seule ligne : {boutons:?}");
    assert!(boutons.iter().all(|(_, r)| r.top() >= grip.top() - 1.0), "aucun bouton au-dessus de la colonne (plus de barre en haut) : {boutons:?} / {grip:?}");
    let min = crate::montagehead::largeur_min_colonne(Barre::Montage);
    assert!(b.app.montage_head_w >= min - 0.5 && min >= crate::barres::largeur_mesuree(Barre::Montage) - 0.5, "{} / {min}", b.app.montage_head_w);
    // Tirer la poignée vers la gauche : la colonne s'arrête au minimum.
    crate::pistes_tests::glisser(&mut b, grip.center(), grip.center() - eframe::egui::vec2(300.0, 0.0));
    b.chauffer(3);
    assert!((b.app.montage_head_w - min).abs() < 1.0, "arrêtée au minimum : {} / {min}", b.app.montage_head_w);
    eprintln!("165.5 : largeur minimale de la colonne du Storyboard : {min:.0} points");
    let grip = ou("Largeur de la colonne");
    boutons_dans_la_colonne(&b, Barre::Montage, (grip.left() - b.app.montage_head_w, grip.right()), haut_des_pistes);
    // Cliquable : la boucle s'allume et s'éteint.
    let boucle = b.app.looping;
    clic(&mut b, ou("Lecture en boucle").center(), Modifiers::NONE);
    assert_ne!(b.app.looping, boucle, "la boucle répond au clic");
    // La colonne élargie au plus : toujours une seule ligne, avec plus de boutons.
    b.app.montage_head_w = crate::montagehead::HEAD_MAX;
    b.chauffer(6);
    let grip = ou("Largeur de la colonne");
    let larges = boutons_dans_la_colonne(&b, Barre::Montage, (grip.left() - b.app.montage_head_w, grip.right()), grip.top() + 80.0);
    let y = larges[0].1.center().y;
    assert!(larges.iter().all(|(_, r)| (r.center().y - y).abs() < 2.0), "une seule ligne : {larges:?}");
    assert!(larges.len() > boutons.len(), "plus large, plus de boutons : {} / {}", larges.len(), boutons.len());
}

/// Le haut de la première ligne de calque dans la colonne de la timeline, à la dernière image.
fn haut_des_calques() -> f32 {
    crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().0.iter().map(|x| x.1).fold(f32::MAX, f32::min))
}

/// Partie 165.5 puis 165.6 (Anthony : « Deux lignes de boutons »), vue Animation, fenêtre la plus petite, les trois dispositions :
/// les boutons de la timeline sont sur deux lignes en haut de la colonne des calques, au-dessus de la première ligne de calque ; la
/// colonne est au moins aussi large que la plus longue des deux, et ne dépasse pas 420 points ; aucun groupe n'est coupé (le plan et
/// le transport sur la première ligne, les dessins, la caméra et « Plus… » sur la seconde) ; les boutons répondent.
#[test]
fn in_the_animation_the_timeline_buttons_sit_in_the_layer_column_on_two_lines() {
    use crate::barres::Barre;
    let [lmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = (0..6).map(|k| Layer::new(&format!("Calque {k}"), LayerKind::Vector)).collect();
    doc.panels[0].set_length(24);
    let mut b = crate::perf_tests::banc_taille(doc, false, lmin as u32, hmin as u32);
    b.app.set_espace(Espace::Animation);
    b.chauffer(6);
    let reglage = crate::barres::ReglageBarre::default();
    let mut ici = Rect::NOTHING;
    for disposition in ["espace.standard", "espace.dessin", "espace.timeline"] {
        b.app.run_action(disposition);
        b.chauffer(8);
        let w = crate::layerheads::largeur_colonne_timeline();
        let mesure = crate::barres::largeur_mesuree(Barre::Timeline);
        assert!(w >= mesure - 0.5 && w >= crate::layerheads::HEAD_W, "{disposition} : {w} / {mesure}");
        assert!(w <= 420.0, "{disposition} : la colonne des calques ne fait plus qu'environ 400 points : {w}");
        // (Partie 174 : le bord gauche de la colonne, depuis le ⊖ de sa rangée d'en-tête.)
        let gauche = vus::ou("Replier tous les calques").iter().map(|r| r.left()).fold(f32::MAX, f32::min) - 10.0;
        let sous = haut_des_calques();
        let boutons = boutons_dans_la_colonne(&b, Barre::Timeline, (gauche, gauche + w + 90.0), sous);
        assert!(boutons.len() >= 15, "{disposition} : tous les boutons : {boutons:?}");
        ici = boutons.iter().find(|(n, _)| n == "Clé de caméra ici").map(|x| x.1).expect("« Clé de caméra ici » dans la colonne");
        // Deux lignes, pas plus, et pas un groupe à cheval.
        let mut ys: Vec<f32> = boutons.iter().map(|(_, r)| r.center().y).collect();
        ys.sort_by(f32::total_cmp);
        let (y1, y2) = (ys[0], ys[ys.len() - 1]);
        assert!(y2 - y1 > 20.0, "{disposition} : deux lignes : {boutons:?}");
        assert!(ys.iter().all(|y| (y - y1).abs() < 2.0 || (y - y2).abs() < 2.0), "{disposition} : deux lignes, pas plus : {boutons:?}");
        let groupe_de = |nom: &str| crate::barres::dans_la_barre(Barre::Timeline, &reglage).into_iter().find(|c| c.nom == nom || b.app.vue_de(Barre::Timeline, c).nom == nom).map(|c| c.groupe);
        for (nom, r) in &boutons {
            let premiere = (r.center().y - y1).abs() < 2.0;
            let g = groupe_de(nom).unwrap_or_else(|| panic!("groupe de « {nom} »"));
            assert_eq!(premiere, g <= crate::timeline::DERNIER_GROUPE_LIGNE1, "{disposition} : « {nom} » (groupe {g}) sur la bonne ligne : {r:?}");
        }
        let plus = vus::ou("Plus…").into_iter().find(|r| r.left() >= gauche - 1.0 && r.right() <= gauche + w + 91.0 && r.bottom() <= sous + 1.0 && r.top() >= y1 - 20.0);
        assert!(plus.is_some_and(|p| (p.center().y - y2).abs() < (p.center().y - y1).abs() && p.top() > y1), "{disposition} : « Plus… » au bout de la seconde ligne : {plus:?} / {y1} {y2}");
    }
    eprintln!("165.6 : largeur de la colonne des calques de la timeline : {:.0} points", crate::layerheads::largeur_colonne_timeline());
    let avant = b.app.cur_frame;
    clic(&mut b, ou("Image suivante").center(), Modifiers::NONE);
    assert_eq!(b.app.cur_frame, avant + 1, "le bouton de la première ligne répond");
    // Et un bouton de la seconde ligne : la clé de caméra ici.
    let cles = b.app.panel().camera.len();
    clic(&mut b, ici.center(), Modifiers::NONE);
    assert!(b.app.panel().camera.len() > cles, "le bouton de la seconde ligne répond : {cles} clés avant");
}

/// Partie 165.6 : avec deux lignes de boutons, l'en-tête de la colonne et la règle de la grille ont la même hauteur : chaque ligne de
/// calque commence et finit à la même hauteur que sa piste, dans les trois dispositions, avec 25 calques et après un défilement.
#[test]
fn with_two_button_lines_the_layer_rows_stay_aligned_with_their_tracks() {
    let [lmin, hmin] = crate::accueilfenetre::TAILLE_MIN_TRAVAIL;
    let mut doc = Document::with_frame(960, 540, 24);
    doc.panels[0].layers = (0..25).map(|k| Layer::new(&format!("Calque {k}"), LayerKind::Vector)).collect();
    doc.panels[0].set_length(24);
    let mut b = crate::perf_tests::banc_taille(doc, false, lmin as u32, hmin as u32);
    b.app.set_espace(Espace::Animation);
    b.chauffer(6);
    for disposition in ["espace.standard", "espace.dessin", "espace.timeline"] {
        b.app.run_action(disposition);
        b.chauffer(8);
        let (colonne, grille) = crate::timeline::LIGNES_PEINTES.with(|l| l.borrow().clone());
        assert_eq!(colonne.len(), 25, "{disposition} : {colonne:?}");
        assert_eq!(colonne.len(), grille.len(), "{disposition}");
        for (c, g) in colonne.iter().zip(&grille) {
            assert_eq!(c.0, g.0, "{disposition} : même calque");
            assert!((c.1 - g.1).abs() < 0.5 && (c.2 - g.2).abs() < 0.5, "{disposition} : calque {} aligné sur sa piste : {c:?} / {g:?}", c.0);
        }
        // La première ligne de calque commence sous les deux lignes de boutons.
        let haut = colonne.iter().map(|x| x.1).fold(f32::MAX, f32::min);
        let mode = vus::ou("Lecture / pause");
        assert!(mode.iter().any(|r| r.bottom() <= haut + 1.0), "{disposition} : boutons au-dessus des calques");
    }
}