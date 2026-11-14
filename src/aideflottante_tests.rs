//! La fenêtre flottante de l'aide animée (partie 155) : la première prise d'un outil l'ouvre à côté de son icône et plus rien ne
//! joue dans « Réglages » ; elle s'efface à la fin et l'application se repose ; un appui dessus la ferme ; un trait sur la toile
//! dessine pendant qu'elle joue ; elle reste dans l'écran ; « Revoir l'explication » la rejoue ; la suite des Vignettes s'enchaîne
//! dans une seule fenêtre ; le thème clair.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::aideanim;
use crate::aideflottante::{self, placer, taille, BORD_ECRAN, ECART};
use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, se_reposer, Banc};

const NONE: Modifiers = Modifiers::NONE;

/// Un banc (toute l'application, 1600 × 900) dont l'aide joue d'elle-même, rien encore vu, le pinceau en main.
fn banc_aide() -> Banc {
    let mut b = banc_taille(Document::with_frame(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.app.settings.explications_vues = aideanim::toutes().map(|a| a.id.to_string()).collect();
    b.app.confort.aide.auto = true;
    b.app.settings.afficher_explications = false;
    b.chauffer(3);
    assert!(!b.app.confort.aide.joue(), "tout est vu : rien ne joue");
    b
}

fn textes(b: &Banc) -> Vec<(String, Rect)> {
    fn collecter(s: &Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
            Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        collecter(&s.shape, &mut out);
    }
    out
}

fn bouton(pos: egui::Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton { pos, button, pressed, modifiers: NONE }
}

fn clic(b: &mut Banc, p: egui::Pos2, button: PointerButton) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![bouton(p, button, true)], 1.0 / 60.0);
    b.image(vec![bouton(p, button, false)], 1.0 / 60.0);
}

// ------------------------------------------------------------------------------------------------------ la place

#[test]
fn the_first_take_of_a_tool_opens_the_window_beside_its_icon_and_nothing_plays_in_the_settings() {
    let mut b = banc_aide();
    b.app.settings.explications_vues.retain(|v| !v.starts_with("trajectoire."));
    b.app.set_tool(Tool::Trajectoire);
    let m = b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.confort.aide.en_cours(), Some("trajectoire.tracer"), "première prise : l'animation joue");
    assert!(m.reveil.is_zero(), "pendant qu'elle joue, l'écran est repeint");
    let icone = aideflottante::ancre(&b.ctx, Tool::Trajectoire.action()).expect("l'icône de la Trajectoire est dans la barre");
    let f = b.app.confort.aide.fenetre().expect("la fenêtre est peinte");
    assert!((f.left() - icone.right() - ECART).abs() < 0.5, "à droite de l'icône : {f:?} / {icone:?}");
    assert!(f.top() <= icone.bottom() && f.bottom() >= icone.top(), "à sa hauteur : {f:?} / {icone:?}");
    assert!((f.size() - taille()).length() < 0.5, "une petite fenêtre : {:?}", f.size());
    // Le panneau Réglages ne joue plus rien : il ne peint pas une forme de plus pendant qu'elle joue.
    assert!(!include_str!("panels.rs").contains("aide_joueur_ui"), "plus de joueur dans Réglages");
    let ctx = egui::Context::default();
    let formes = |app: &mut App| ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
    }).shapes.len();
    let pendant = formes(&mut b.app);
    let lecture = b.app.confort.aide.lecture.take();
    let sans = formes(&mut b.app);
    b.app.confort.aide.lecture = lecture;
    assert_eq!(pendant, sans, "Réglages peint la même chose, que l'animation joue ou non");
}

#[test]
fn a_click_on_a_tool_icon_opens_the_window_and_the_click_does_not_close_it() {
    let mut b = banc_aide();
    b.app.settings.explications_vues.retain(|v| !v.starts_with("trajectoire."));
    let icone = aideflottante::ancre(&b.ctx, Tool::Trajectoire.action()).expect("l'icône de la Trajectoire");
    clic(&mut b, icone.center(), PointerButton::Primary);
    assert_eq!(b.app.tool, Tool::Trajectoire);
    let premiere = b.app.confort.aide.fenetre().expect("le clic sur l'icône ouvre la fenêtre");
    for _ in 0..6 {
        b.image(vec![Event::PointerMoved(icone.center())], 1.0 / 60.0);
    }
    assert_eq!(b.app.confort.aide.en_cours(), Some("trajectoire.tracer"), "elle joue encore");
    assert_eq!(b.app.confort.aide.fenetre(), Some(premiere), "à la même place");
    assert!(!premiere.contains(icone.center()), "elle ne couvre pas l'icône");
}

#[test]
fn the_space_3d_window_stays_beside_the_icon_that_opened_the_view() {
    let mut b = banc_aide();
    let ids: Vec<&str> = aideanim::du_contexte("espace3d").iter().map(|a| a.id).collect();
    b.app.settings.explications_vues.retain(|v| !ids.contains(&v.as_str()));
    let cube = aideflottante::ancre(&b.ctx, aideflottante::ESPACE3D).expect("l'icône de l'Espace 3D dans la barre de la vue");
    clic(&mut b, cube.center(), PointerButton::Primary);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    assert!(b.app.e3_vue_montree(), "la vue est ouverte");
    assert_eq!(b.app.confort.aide.en_cours(), ids.first().copied());
    let place = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert_eq!(place, placer(cube, taille(), b.ctx.screen_rect()), "à côté de l'icône qui a ouvert la vue");
    for _ in 0..8 {
        b.image(Vec::new(), 1.0 / 60.0);
        assert_eq!(b.app.confort.aide.fenetre(), Some(place), "elle ne saute pas ailleurs");
    }
}

#[test]
fn nothing_plays_by_itself_where_there_is_no_tool_bar() {
    let mut b = banc_aide();
    b.app.set_espace(crate::workspace::Espace::Montage);
    b.chauffer(3);
    b.app.settings.explications_vues.clear();
    b.app.set_tool(Tool::Gradient);
    b.chauffer(3);
    assert!(!b.app.confort.aide.joue(), "le Montage n'a ni barre d'outils ni Réglages : rien ne joue");
    assert!(!b.app.settings.explications_vues.iter().any(|v| v == "degrade.axe"), "et rien n'est retenu comme vu");
}

#[test]
fn the_window_fades_away_at_the_end_and_then_the_application_rests() {
    let mut b = banc_aide();
    b.app.settings.explications_vues.retain(|v| v != "degrade.axe");
    b.app.set_tool(Tool::Gradient);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.confort.aide.en_cours(), Some("degrade.axe"));
    let mut images = 0;
    while b.app.confort.aide.joue() && images < 200 {
        b.image(Vec::new(), 0.1);
        images += 1;
    }
    assert!(!b.app.confort.aide.joue(), "la fenêtre est partie seule");
    assert!(images > 40, "elle a joué jusqu'au bout ({images} images de 0,1 s)");
    assert!(se_reposer(&mut b, 3000) < 3000, "puis l'application se repose{}", b.raison_du_reveil());
    let m = b.image(Vec::new(), 0.5);
    assert!(m.reveil >= std::time::Duration::from_secs(1), "à l'arrêt, plus de repaint : {:?}", m.reveil);
}

#[test]
fn a_press_on_the_window_closes_it() {
    let mut b = banc_aide();
    b.app.settings.explications_vues.retain(|v| v != "camera.cadrer");
    b.app.set_tool(Tool::Camera);
    b.chauffer(3);
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    clic(&mut b, f.center(), PointerButton::Primary);
    assert!(!b.app.confort.aide.joue(), "un clic dessus la ferme tout de suite");
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.confort.aide.fenetre().is_none());
}

/// Lance l'animation du pinceau dans une fenêtre posée au milieu de la toile (à côté d'une ancre d'essai).
fn fenetre_sur_la_toile(b: &mut Banc) -> Rect {
    let c = b.app.canvas_rect;
    let ancre = Rect::from_min_size(pos2(c.left() + 60.0, c.top() + 60.0), vec2(20.0, 20.0));
    aideflottante::retenir(&b.ctx, "essai", ancre);
    let t = b.ctx.input(|i| i.time);
    b.app.aide_jouer(vec!["dessin.pression"], vec!["essai"], t);
    b.chauffer(2);
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert!(c.contains_rect(f), "la fenêtre est sur la toile : {f:?} dans {c:?}");
    f
}

#[test]
fn a_stroke_on_the_canvas_draws_while_the_window_plays() {
    let mut b = banc_aide();
    assert!(aideanim::anim("dessin.pression").is_some());
    // Un trait qui commence sur la fenêtre : il dessine, et la fenêtre s'en va.
    let f = fenetre_sur_la_toile(&mut b);
    let avant = b.app.history.undo_len();
    let (a, z) = (f.center(), f.center() + vec2(120.0, 30.0));
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, PointerButton::Primary, true)], 1.0 / 60.0);
    assert!(!b.app.confort.aide.joue(), "l'appui ferme la fenêtre");
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(z, PointerButton::Primary, false)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(b.app.history.undo_len(), avant + 1, "le trait est dessiné");
    // Un trait qui passe sous la fenêtre : elle s'efface devant le pointeur, le trait est dessiné.
    let f = fenetre_sur_la_toile(&mut b);
    let avant = b.app.history.undo_len();
    let (a, z) = (pos2(f.right() + 80.0, f.center().y), f.center());
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    b.image(vec![bouton(a, PointerButton::Primary, true)], 1.0 / 60.0);
    assert!(b.app.confort.aide.joue(), "loin d'elle, elle joue encore");
    for k in 1..=10 {
        b.image(vec![Event::PointerMoved(a + (z - a) * (k as f32 / 10.0))], 1.0 / 60.0);
    }
    assert!(!b.app.confort.aide.joue(), "elle ne couvre pas le pointeur pendant le geste");
    b.image(vec![bouton(z, PointerButton::Primary, false)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(b.app.history.undo_len(), avant + 1, "le trait est dessiné");
}

#[test]
fn the_window_stays_on_screen_even_near_an_edge() {
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 900.0));
    let zone = ecran.shrink(BORD_ECRAN);
    let t = taille();
    // Une icône de la barre de gauche : à sa droite.
    let icone = Rect::from_min_size(pos2(10.0, 200.0), vec2(32.0, 32.0));
    let r = placer(icone, t, ecran);
    assert_eq!(r.left(), icone.right() + ECART);
    assert_eq!(r.top(), icone.top());
    // Tout en bas : remontée dans l'écran, toujours à droite, jamais par-dessus la barre.
    let bas = Rect::from_min_size(pos2(10.0, 870.0), vec2(32.0, 28.0));
    let r = placer(bas, t, ecran);
    assert!(zone.contains_rect(r) && r.left() == bas.right() + ECART, "{r:?}");
    // Contre le bord droit : à gauche de l'ancre.
    let droite = Rect::from_min_size(pos2(1570.0, 300.0), vec2(28.0, 28.0));
    let r = placer(droite, t, ecran);
    assert!(zone.contains_rect(r) && r.right() <= droite.left(), "{r:?}");
    // Un onglet en haut à droite : dessous, sans sortir.
    let onglet = Rect::from_min_size(pos2(1480.0, 2.0), vec2(110.0, 22.0));
    let r = placer(onglet, t, ecran);
    assert!(zone.contains_rect(r) && r.top() >= onglet.bottom(), "{r:?}");
    // Un écran plus petit que la fenêtre : collée en haut à gauche.
    let petit = Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
    let r = placer(Rect::from_min_size(pos2(150.0, 50.0), vec2(10.0, 10.0)), t, petit);
    assert_eq!(r.min, petit.shrink(BORD_ECRAN).min);
    // Dans l'application : une ancre au bord, la fenêtre tient dans l'écran.
    let mut b = banc_aide();
    aideflottante::retenir(&b.ctx, "essai", Rect::from_min_size(pos2(1590.0, 890.0), vec2(8.0, 8.0)));
    let t0 = b.ctx.input(|i| i.time);
    b.app.aide_jouer(vec!["camera.cadrer"], vec!["essai"], t0);
    b.image(Vec::new(), 1.0 / 60.0);
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert!(zone.contains_rect(f), "{f:?}");
}

// ------------------------------------------------------------------------------------------------------ revoir

#[test]
fn see_the_explanation_again_from_the_right_click_of_a_tool_icon() {
    let mut b = banc_aide();
    let icone = aideflottante::ancre(&b.ctx, Tool::Trajectoire.action()).expect("l'icône de la Trajectoire");
    clic(&mut b, icone.center(), PointerButton::Secondary);
    b.chauffer(2);
    let (_, r) = textes(&b).into_iter().find(|(t, _)| t.contains("Revoir l'explication")).unwrap_or_else(|| panic!("pas d'entrée « Revoir l'explication » : {:?}", textes(&b).iter().map(|t| &t.0).collect::<Vec<_>>()));
    clic(&mut b, r.center(), PointerButton::Primary);
    b.chauffer(2);
    assert_eq!(b.app.confort.aide.en_cours(), Some("trajectoire.tracer"), "elle rejoue, même déjà vue");
    assert_eq!(b.app.tool, Tool::Brush, "sans changer d'outil");
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert!((f.left() - icone.right() - ECART).abs() < 0.5, "à côté de l'icône : {f:?} / {icone:?}");
    // Aussi sur l'onglet Vignettes et le bouton de l'Espace 3D ; pas sur une commande quelconque.
    assert!(aideflottante::a_une_explication("espace.vignettes") && aideflottante::a_une_explication("espace3d.basculer"));
    assert!(!aideflottante::a_une_explication("fichier.enregistrer"));
    for t in Tool::ALL {
        assert!(aideflottante::a_une_explication(t.action()), "{t:?}");
    }
}

#[test]
fn the_play_button_of_the_settings_opens_the_window_beside_the_tool() {
    let mut b = banc_aide();
    b.chauffer(2);
    let play = egui_phosphor::regular::PLAY;
    // (Le ▶ sur la ligne du titre de Réglages, « Pinceau » ; pas celui du transport.)
    let tous = textes(&b);
    let titre = tous.iter().find(|(t, _)| t.ends_with(Tool::Brush.label()) && t.contains(Tool::Brush.icon())).map(|(_, r)| *r).expect("le titre de Réglages");
    let (_, r) = tous.into_iter().find(|(t, r)| t == play && (r.center().y - titre.center().y).abs() < 12.0 && r.left() > titre.right()).expect("le ▶ de Réglages");
    clic(&mut b, r.center(), PointerButton::Primary);
    b.chauffer(2);
    assert_eq!(b.app.confort.aide.en_cours(), aideanim::du_contexte("pinceau").first().map(|a| a.id));
    let icone = aideflottante::ancre(&b.ctx, Tool::Brush.action()).expect("l'icône du pinceau");
    let f = b.app.confort.aide.fenetre().expect("la fenêtre joue");
    assert!((f.left() - icone.right() - ECART).abs() < 0.5, "à côté de l'icône du pinceau, pas dans Réglages : {f:?} / {icone:?}");
}

// ------------------------------------------------------------------------------------------------------ les Vignettes

/// Une application sans fenêtre où seule l'aide flottante est peinte, image après image.
fn app_seule() -> App {
    let dir = std::env::temp_dir().join(format!("shot-aideflot-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(LayerKind::Vector);
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.confort.aide.auto = true;
    app.settings.explications_vues.clear();
    app.settings.afficher_explications = false;
    app
}

fn image(ctx: &egui::Context, app: &mut App, t: f64) -> Vec<egui::epaint::ClippedShape> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0))), time: Some(t), ..RawInput::default() };
    ctx.run(input, |ctx| app.aide_flottante_ui(ctx)).shapes
}

#[test]
fn the_vignettes_suite_plays_one_after_the_other_in_a_single_window() {
    let mut app = app_seule();
    app.espace = crate::workspace::Espace::Vignettes;
    let ctx = egui::Context::default();
    let mut t = 0.0;
    image(&ctx, &mut app, t);
    let place = app.confort.aide.fenetre().expect("l'espace Vignettes ouvert la première fois : la fenêtre");
    let mut vues: Vec<&'static str> = Vec::new();
    while app.confort.aide.joue() && t < 120.0 {
        if let Some(id) = app.confort.aide.en_cours() {
            if vues.last() != Some(&id) {
                vues.push(id);
            }
        }
        if let Some(f) = app.confort.aide.fenetre() {
            assert_eq!(f, place, "la même fenêtre, à la même place, d'une animation à l'autre");
        }
        t += 0.1;
        image(&ctx, &mut app, t);
    }
    let suite: Vec<&'static str> = aideanim::du_contexte("vignettes").iter().map(|a| a.id).collect();
    assert!(suite.len() >= 2, "{suite:?}");
    assert_eq!(vues, suite, "toute la suite, dans l'ordre, sans que la fenêtre se ferme");
    assert!(!app.confort.aide.joue());
}

/// Partie 156 (Anthony : « ça mérite d'afficher le fonctionnement de Vignettes quand on ouvre le logiciel pour la première fois sur
/// cette page ») : FEANOR démarre sur la page Vignettes (l'espace où il l'avait quittée), d'abord sans projet (l'accueil), puis un
/// projet s'ouvre. La suite des Vignettes joue d'elle-même, sans changer d'onglet ; déjà vue, rien.
fn demarrer_sur_les_vignettes(vues: Vec<String>) -> Banc {
    let mut b = banc_taille(Document::with_frame(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.set_espace(crate::workspace::Espace::Vignettes);
    b.chauffer(3);
    // Le démarrage : l'accueil, aucun projet ; l'aide est neuve, comme à l'ouverture du logiciel.
    b.app.welcome = true;
    b.app.path = None;
    b.app.modified = false;
    b.app.history.clear();
    b.app.confort.aide = aideanim::Joueur::new(true);
    b.app.settings.explications_vues = vues;
    b.app.settings.afficher_explications = false;
    assert!(b.app.sans_projet());
    b.chauffer(4);
    assert!(!b.app.confort.aide.joue(), "sans projet, rien ne joue");
    // Le projet s'ouvre (comme `finish_open_avec`).
    assert!(b.app.charger_projet(Document::with_frame(400, 300, 24)));
    assert!(!b.app.sans_projet());
    b.chauffer(6);
    assert_eq!(b.app.espace, crate::workspace::Espace::Vignettes, "toujours sur la page Vignettes");
    b
}

#[test]
fn opening_feanor_for_the_first_time_on_the_vignettes_page_plays_the_vignettes_suite() {
    let mut b = demarrer_sur_les_vignettes(Vec::new());
    let suite: Vec<&'static str> = aideanim::du_contexte("vignettes").iter().map(|a| a.id).collect();
    assert_eq!(b.app.confort.aide.en_cours(), suite.first().copied(), "au démarrage sur la page Vignettes, la suite joue d'elle-même");
    let f = b.app.confort.aide.fenetre().expect("la fenêtre est peinte");
    let onglet = aideflottante::ancre(&b.ctx, aideflottante::VIGNETTES).expect("l'onglet Vignettes est à l'écran");
    assert_eq!(f, placer(onglet, taille(), b.ctx.screen_rect()), "sous l'onglet Vignettes");
    // Elle joue jusqu'au bout, toute la suite.
    let mut vues: Vec<&'static str> = Vec::new();
    let mut images = 0;
    while b.app.confort.aide.joue() && images < 600 {
        if let Some(id) = b.app.confort.aide.en_cours() {
            if vues.last() != Some(&id) {
                vues.push(id);
            }
        }
        b.image(Vec::new(), 0.1);
        images += 1;
    }
    assert_eq!(vues, suite, "toute la suite, dans l'ordre");
    assert!(suite.iter().all(|id| b.app.settings.explications_vues.iter().any(|v| v == id)), "retenue comme vue");
}

#[test]
fn the_vignettes_suite_at_startup_survives_a_second_tab_and_the_return_to_the_first() {
    let mut b = demarrer_sur_les_vignettes(Vec::new());
    let suite: Vec<&'static str> = aideanim::du_contexte("vignettes").iter().map(|a| a.id).collect();
    assert_eq!(b.app.confort.aide.en_cours(), suite.first().copied());
    // La séance d'avant avait deux projets : le second s'ouvre dans un onglet à lui, puis l'onglet affiché revient.
    b.app.settings.open_in_new_tab = true;
    assert!(b.app.charger_projet(Document::with_frame(400, 300, 24)));
    b.chauffer(3);
    assert!(b.app.activer_onglet(0));
    b.chauffer(3);
    let mut vues: Vec<&'static str> = Vec::new();
    let mut images = 0;
    while b.app.confort.aide.joue() && images < 600 {
        if let Some(id) = b.app.confort.aide.en_cours() {
            if vues.last() != Some(&id) {
                vues.push(id);
            }
        }
        b.image(Vec::new(), 0.1);
        images += 1;
    }
    assert_eq!(vues, suite, "toute la suite, malgré l'ouverture des autres projets de la séance");
}

/// Ce qui la faisait manquer au démarrage : FEANOR rouvre les projets de la séance d'avant l'un après l'autre (« Ouverture… »),
/// et la suite commençait dès le premier, derrière cette fenêtre ; une image qui tardait (un gros projet adopté) sautait une
/// animation entière, retenue comme vue. Maintenant elle attend que la séance soit revenue, et une image en retard ne saute rien.
#[test]
fn the_vignettes_suite_waits_for_the_session_to_come_back_and_skips_nothing_on_a_slow_frame() {
    let mut b = banc_taille(Document::with_frame(400, 300, 24), false, 1600, 900);
    b.garder_formes = true;
    b.app.set_espace(crate::workspace::Espace::Vignettes);
    b.chauffer(3);
    b.app.welcome = true;
    b.app.path = None;
    b.app.modified = false;
    b.app.history.clear();
    b.app.confort.aide = aideanim::Joueur::new(true);
    b.app.settings.explications_vues.clear();
    b.app.settings.afficher_explications = false;
    b.chauffer(3);
    // La séance d'avant revient : le premier projet est là, les suivants s'ouvrent encore (une image chacun ici).
    b.app.session_prete = false;
    b.app.session_file = (0..30).map(|i| std::env::temp_dir().join(format!("feanor-absent-{i}.shot"))).collect();
    assert!(b.app.charger_projet(Document::with_frame(400, 300, 24)));
    let mut images = 0;
    while !b.app.session_prete && images < 100 {
        assert!(!b.app.confort.aide.joue(), "pendant que la séance revient, rien ne commence (image {images})");
        b.image(Vec::new(), 0.1);
        images += 1;
    }
    assert!(images >= 30, "la séance a mis {images} images à revenir");
    assert!(b.app.session_prete);
    b.chauffer(2);
    let suite: Vec<&'static str> = aideanim::du_contexte("vignettes").iter().map(|a| a.id).collect();
    assert_eq!(b.app.confort.aide.en_cours(), suite.first().copied(), "la séance revenue : la suite commence, au début");
    // Une image qui tarde dix secondes (plus qu'une animation entière) : l'animation reprend où elle était.
    assert!(aideanim::anim(suite[0]).is_some_and(|a| (a.duree as f64) < 10.0));
    b.image(Vec::new(), 10.0);
    assert_eq!(b.app.confort.aide.en_cours(), suite.first().copied(), "une image en retard ne saute pas l'animation");
    assert!(!b.app.settings.explications_vues.iter().any(|v| Some(v.as_str()) == suite.get(1).copied()), "la suivante n'est pas retenue comme vue");
    let mut vues: Vec<&'static str> = Vec::new();
    let mut n = 0;
    while b.app.confort.aide.joue() && n < 600 {
        if let Some(id) = b.app.confort.aide.en_cours() {
            if vues.last() != Some(&id) {
                vues.push(id);
            }
        }
        b.image(Vec::new(), 0.1);
        n += 1;
    }
    assert_eq!(vues, suite, "toute la suite, dans l'ordre");
}

#[test]
fn opening_feanor_on_the_vignettes_page_again_plays_nothing_once_seen() {
    let vues = aideanim::toutes().map(|a| a.id.to_string()).collect();
    let b = demarrer_sur_les_vignettes(vues);
    assert!(!b.app.confort.aide.joue(), "déjà vue : rien ne joue");
}

#[test]
fn a_press_on_the_startup_vignettes_window_closes_it() {
    let mut b = demarrer_sur_les_vignettes(Vec::new());
    let f = b.app.confort.aide.fenetre().expect("la suite joue");
    // Un appui sur la fenêtre la ferme ; le clic continue son chemin.
    b.image(vec![Event::PointerMoved(f.center())], 1.0 / 60.0);
    b.image(vec![bouton(f.center(), PointerButton::Primary, true)], 1.0 / 60.0);
    assert!(!b.app.confort.aide.joue(), "l'appui la ferme aussitôt");
    b.image(vec![bouton(f.center(), PointerButton::Primary, false)], 1.0 / 60.0);
}

// ------------------------------------------------------------------------------------------------------ le thème clair

#[test]
fn the_window_follows_the_light_theme() {
    use crate::theme::{self, ChoixTheme, ReglageTheme};
    let mut app = app_seule();
    // (Après l'application, qui installe le thème de ses réglages.)
    theme::installer(&ReglageTheme { choix: ChoixTheme::Papier, ..ReglageTheme::default() });
    let ctx = egui::Context::default();
    app.set_tool(Tool::Camera);
    image(&ctx, &mut app, 0.0);
    let formes = image(&ctx, &mut app, 1.0);
    theme::installer(&ReglageTheme::default());
    let c = |v: theme::Rvb| egui::Color32::from_rgb(v[0], v[1], v[2]);
    // (Seule la fenêtre flottante est peinte sur cet écran.)
    let pleins: Vec<egui::Color32> = formes
        .iter()
        .filter_map(|s| if let Shape::Rect(r) = &s.shape { Some(r.fill) } else { None })
        .collect();
    assert!(pleins.contains(&c(theme::PAPIER_PANNEAU)), "le cadre est un panneau du thème clair : {pleins:?}");
    assert!(pleins.contains(&c(theme::PAPIER_CANEVAS)), "la vignette est sur le canevas du thème clair : {pleins:?}");
    assert!(app.confort.aide.joue());
}
