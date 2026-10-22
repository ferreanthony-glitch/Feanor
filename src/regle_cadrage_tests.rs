//! La règle, le menu Cadrage et la colonne des pastilles (Anthony, 2026-10-21) :
//! - la règle, au-dessus du dessin, est le seul lieu des guides : tout ce que faisait le panneau Guides y est ;
//! - le format et les zones de sécurité sont dans le menu Cadrage, à gauche de la barre du bas ; l'aspect des traits
//!   des repères est sous la règle, à côté d'eux (2026-10-22) ;
//! - les pastilles des personnages sont une colonne au bord droit du dessin (et de la planche des Vignettes), et
//!   cliquer dessus ne dessine rien.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::personnagesui::{Case, Choix};
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------ l'outillage

/// Un écran sans fenêtre : on y peint un morceau d'interface image après image, et on lit les textes peints (avec leur
/// place) ; les clics sont des événements du pointeur.
struct Ecran {
    ctx: egui::Context,
    t: f64,
}

impl Ecran {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 900.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new(), contenu);
        }
        textes
    }

    /// Un clic en `p`, puis quelques images pour que l'interface réagisse.
    fn cliquer(&mut self, app: &mut App, p: egui::Pos2, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], contenu);
        self.chauffer(app, 3, contenu)
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn trouver<'a>(textes: &'a [(String, Rect)], texte: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.trim() == texte)
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

fn appli(nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-regle-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

// ------------------------------------------------------------------------------------ la règle : les guides

/// Les six interrupteurs du panneau Guides, avec la façon de lire chacun.
fn interrupteurs() -> [(&'static str, fn(&App) -> bool); 6] {
    [
        (ph::PERSPECTIVE, |a| a.guides.enabled),
        (ph::MAGNET, |a| a.guides.align),
        (ph::LOCK_SIMPLE, |a| a.guides.locked),
        (ph::ASTERISK, |a| a.guides.reference),
        (ph::ERASER, |a| a.guides.eraser),
        (ph::CROSSHAIR, |a| a.guides.full_cursor),
    ]
}

#[test]
fn every_function_of_the_old_guides_panel_is_in_the_ruler_menu() {
    let mut app = appli("fonctions");
    app.add_guide(2);
    let mut ecran = Ecran::new();
    let mut menu = |a: &mut App, ui: &mut egui::Ui| a.guides_menu_ui(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut menu);
    // Les six interrupteurs, un guide de plus à 1, 2 ou 3 points, le nom du guide courant, le supprimer, la liste.
    for (icone, _) in interrupteurs() {
        assert!(trouver(&textes, icone).is_some(), "{icone:?} est dans le menu de la règle : {textes:?}");
    }
    for icone in [ph::NUMBER_SQUARE_ONE, ph::NUMBER_SQUARE_TWO, ph::NUMBER_SQUARE_THREE, ph::TRASH] {
        assert!(trouver(&textes, icone).is_some(), "{icone:?} est dans le menu de la règle");
    }
    assert!(contient(&textes, "Perspective 2 points"), "la liste des guides du projet : {textes:?}");
    // Et chaque interrupteur fait ce que faisait celui du panneau : un clic le bascule, un autre le remet.
    for (icone, lire) in interrupteurs() {
        let avant = lire(&app);
        let t = ecran.chauffer(&mut app, 2, &mut menu);
        let p = trouver(&t, icone).unwrap().1.center();
        ecran.cliquer(&mut app, p, &mut menu);
        assert_eq!(lire(&app), !avant, "un clic sur {icone:?} bascule le réglage");
        let t = ecran.chauffer(&mut app, 2, &mut menu);
        let p = trouver(&t, icone).unwrap().1.center();
        ecran.cliquer(&mut app, p, &mut menu);
        assert_eq!(lire(&app), avant, "un second clic le remet");
    }
    // Un guide de plus à 1, 2, 3 points : le guide courant, une étape d'annulation chacun.
    for (n, icone) in [(1usize, ph::NUMBER_SQUARE_ONE), (2, ph::NUMBER_SQUARE_TWO), (3, ph::NUMBER_SQUARE_THREE)] {
        let avant = app.doc.guides.len();
        let t = ecran.chauffer(&mut app, 2, &mut menu);
        let p = trouver(&t, icone).unwrap().1.center();
        ecran.cliquer(&mut app, p, &mut menu);
        assert_eq!(app.doc.guides.len(), avant + 1, "un guide à {n} point(s) de fuite de plus");
        assert_eq!(app.doc.guides[app.guides.current].points.len(), n);
    }
    // Choisir un guide dans la liste.
    let t = ecran.chauffer(&mut app, 2, &mut menu);
    let p = trouver(&t, "Perspective 2 points").map(|x| x.1.center());
    if let Some(p) = p {
        app.guides.enabled = false;
        ecran.cliquer(&mut app, p, &mut menu);
        assert!(app.guides.enabled, "choisir un guide dans la liste l'allume");
        assert!(app.doc.guides[app.guides.current].points.len() == 2);
    }
    // Le supprimer.
    let avant = app.doc.guides.len();
    let t = ecran.chauffer(&mut app, 2, &mut menu);
    let p = trouver(&t, ph::TRASH).unwrap().1.center();
    ecran.cliquer(&mut app, p, &mut menu);
    assert_eq!(app.doc.guides.len(), avant - 1, "le guide courant est supprimé");
}

#[test]
fn the_ruler_menu_keeps_only_guides_and_the_composition_marks() {
    let mut app = appli("seulement");
    let mut ecran = Ecran::new();
    let mut menu = |a: &mut App, ui: &mut egui::Ui| a.guides_menu_ui(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut menu);
    // Les repères de composition : tiers, grille d'or, spirale, orientation, diagonales, croix, tout éteindre.
    for icone in [ph::GRID_NINE, ph::DOTS_NINE, ph::SPIRAL, ph::ARROWS_CLOCKWISE, ph::LINE_SEGMENTS, ph::CROSSHAIR_SIMPLE, ph::X] {
        assert!(trouver(&textes, icone).is_some(), "{icone:?} reste dans la règle");
    }
    // L'aspect des traits des repères est à côté d'eux (Anthony, 2026-10-22).
    assert!(contient(&textes, "Aspect des traits des repères"), "l'aspect des traits est sous la règle : {textes:?}");
    // Plus de format ni de zones de sécurité : ils sont dans le menu Cadrage.
    for absent in ["Format", "Zones de sécurité", "Action 93 %", ph::CROP, ph::SELECTION] {
        assert!(!contient(&textes, absent), "« {absent} » n'est plus dans la règle : {textes:?}");
    }
    assert!(contient(&textes, "ne sont jamais exportées"), "la note des aides d'écran reste");
    // Le bouton de la règle s'allume avec un repère ou le guide de perspective.
    assert!(!app.guides_any_on());
    app.cadrage.thirds = true;
    assert!(app.guides_any_on());
    app.cadrage.thirds = false;
    app.guides.enabled = true;
    assert!(app.guides_any_on());
    // Un repère s'allume au clic, comme avant.
    app.guides.enabled = false;
    let t = ecran.chauffer(&mut app, 2, &mut menu);
    let p = trouver(&t, ph::GRID_NINE).unwrap().1.center();
    ecran.cliquer(&mut app, p, &mut menu);
    assert!(app.cadrage.thirds);
}

// ------------------------------------------------------------------------------------ le menu Cadrage

#[test]
fn the_framing_menu_is_the_first_icon_of_the_bottom_view_bar() {
    let mut app = appli("cadrage");
    let mut ecran = Ecran::new();
    let mut barre = |a: &mut App, ui: &mut egui::Ui| a.view_tools_bar(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut barre);
    let cadrage = trouver(&textes, ph::CROP).expect("l'icône Cadrage est dans la barre du bas");
    let ajuster = trouver(&textes, ph::ARROWS_OUT).expect("ajuster");
    assert!(cadrage.1.center().x < ajuster.1.center().x, "Cadrage est tout à gauche, avant « ajuster »");
    // Même taille d'icône que le reste de la barre.
    let loupe = trouver(&textes, ph::MAGNIFYING_GLASS).expect("loupe");
    assert!((cadrage.1.height() - loupe.1.height()).abs() < 0.5 && (cadrage.1.width() - loupe.1.width()).abs() < 0.5);
    assert!(!app.framing_any_on());
    // Un clic ouvre le petit panneau : format et zones de sécurité (l'aspect des traits est sous la règle).
    let t = ecran.cliquer(&mut app, cadrage.1.center(), &mut barre);
    for texte in ["Format", "Aucun", "16:9", "2.39:1", "Zones de sécurité"] {
        assert!(contient(&t, texte), "« {texte} » dans le menu Cadrage : {t:?}");
    }
    assert!(!contient(&t, "Aspect des traits"), "l'aspect des traits n'est plus dans le menu Cadrage : {t:?}");
    assert!(contient(&t, "Action 93 %") && contient(&t, "Action 90 %"), "le choix des zones de sécurité : {t:?}");
    assert!(trouver(&t, ph::SELECTION).is_some(), "l'interrupteur des zones de sécurité");
    // Choisir un format.
    let p = trouver(&t, "16:9").unwrap().1.center();
    let t = ecran.cliquer(&mut app, p, &mut barre);
    assert_eq!(app.layout.frame_format, Some([16.0, 9.0]));
    assert!(app.framing_any_on(), "l'icône s'allume avec un format");
    // Les zones de sécurité.
    let p = trouver(&t, ph::SELECTION).unwrap().1.center();
    let t = ecran.cliquer(&mut app, p, &mut barre);
    assert!(app.layout.safe_frames);
    // Revenir sans format.
    let p = trouver(&t, "Aucun").unwrap().1.center();
    ecran.cliquer(&mut app, p, &mut barre);
    assert_eq!(app.layout.frame_format, None);
    assert!(app.framing_any_on(), "les zones de sécurité allument encore l'icône");
}

#[test]
fn the_view_bar_above_the_drawing_has_the_ruler_not_the_framing() {
    let mut app = appli("barre_haut");
    let mut ecran = Ecran::new();
    let mut barre = |a: &mut App, ui: &mut egui::Ui| a.view_bar(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut barre);
    assert!(trouver(&textes, ph::RULER).is_some(), "la règle est dans la barre du haut");
    assert!(trouver(&textes, ph::CROP).is_none(), "le cadrage n'y est plus");
}

// ------------------------------------------------------------------------------------ les pastilles

fn banc() -> Banc {
    let mut b = banc_taille(Document::new(400, 300, 24), false, 1600, 900);
    b.garder_textes = true;
    b.garder_formes = true;
    b.app.tool = Tool::Brush;
    b.chauffer(6);
    b
}

fn appuyer(b: &mut Banc, p: egui::Pos2, bouton: PointerButton) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: bouton, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
}

fn relacher(b: &mut Banc, p: egui::Pos2, bouton: PointerButton) {
    b.image(vec![Event::PointerButton { pos: p, button: bouton, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(3);
}

fn clic(b: &mut Banc, p: egui::Pos2) {
    appuyer(b, p, PointerButton::Primary);
    relacher(b, p, PointerButton::Primary);
}

/// Un clic au milieu d'une case de la colonne.
fn clic_case(b: &mut Banc, c: Case) {
    let p = case(b, c).center();
    clic(b, p);
}

fn case(b: &Banc, c: Case) -> Rect {
    b.app.perso.cases.iter().find(|(_, k)| *k == c).map(|(r, _)| *r).unwrap_or_else(|| panic!("la case {c:?} n'est pas dans la colonne : {:?}", b.app.perso.cases))
}

#[test]
fn the_pastilles_are_a_slim_column_on_the_right_edge_of_the_drawing() {
    let mut b = banc();
    assert_eq!(b.app.espace, Espace::Storyboard);
    let dessin = b.app.canvas_rect;
    // Aucun personnage au départ : « aucun », les trois couleurs neutres, « + », l'icône du panneau.
    let cases: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    assert_eq!(cases.first(), Some(&Case::Aucun));
    assert!(cases.contains(&Case::Plus) && cases.contains(&Case::Panneau));
    assert_eq!(cases.iter().filter(|c| matches!(c, Case::Choix(Choix::Neutre(_)))).count(), 3, "{cases:?}");
    // Une colonne : toutes les cases l'une sous l'autre, le long du bord droit, sous le titre du plan.
    let rects: Vec<Rect> = b.app.perso.cases.iter().map(|(r, _)| *r).collect();
    for w in rects.windows(2) {
        assert!(w[1].top() >= w[0].bottom() - 0.5, "l'une sous l'autre : {w:?}");
    }
    let (gauche, droite) = (rects.iter().map(|r| r.left()).fold(f32::MAX, f32::min), rects.iter().map(|r| r.right()).fold(f32::MIN, f32::max));
    assert!(droite <= dessin.right() && dessin.right() - droite < 14.0, "au bord droit : {droite} dans {dessin:?}");
    assert!(droite - gauche < 34.0, "une colonne mince ({} points)", droite - gauche);
    assert!(rects[0].top() > dessin.top() + 30.0, "sous le titre du plan : {:?} dans {dessin:?}", rects[0]);
    // Plus de texte « Aucun personnage » ni de barre au-dessus du dessin : la bulle le dit.
    assert!(!b.textes.iter().any(|t| t.contains("Aucun personnage")), "{:?}", b.textes);
    // La rangée de la barre a disparu : le dessin garde la même place, pastilles montrées ou cachées.
    b.app.layout.character_bar = Some(false);
    b.chauffer(4);
    assert!(b.app.perso.cases.is_empty(), "cachées : plus de colonne");
    assert_eq!(b.app.canvas_rect, dessin, "la colonne ne prend pas de place au dessin");
    b.app.layout.character_bar = Some(true);
    b.chauffer(4);
    assert!(!b.app.perso.cases.is_empty());
}

#[test]
fn clicking_a_pastille_chooses_it_and_draws_nothing() {
    let mut b = banc();
    let pas = b.app.history.undo_len();
    // Un appui sur le dessin, lui, dessine : l'essai n'est pas vide de sens.
    let centre = b.app.canvas_rect.center();
    appuyer(&mut b, centre, PointerButton::Primary);
    assert!(b.app.stroke.is_some(), "un appui sur le dessin commence un trait");
    relacher(&mut b, centre, PointerButton::Primary);
    b.app.undo();
    b.chauffer(2);
    let pas = pas.min(b.app.history.undo_len());
    // Une couleur neutre : un clic la choisit, rien n'est tracé, aucune étape d'annulation.
    let neutre = case(&b, Case::Choix(Choix::Neutre(1)));
    assert_eq!(b.app.perso.actif, None);
    appuyer(&mut b, neutre.center(), PointerButton::Primary);
    assert!(b.app.stroke.is_none(), "l'appui sur une pastille ne commence aucun trait");
    // Même en glissant sur la colonne avant de lâcher.
    b.image(vec![Event::PointerMoved(neutre.center() + vec2(0.0, 40.0))], 1.0 / 60.0);
    assert!(b.app.stroke.is_none(), "glisser sur la colonne ne trace rien");
    relacher(&mut b, neutre.center() + vec2(0.0, 40.0), PointerButton::Primary);
    clic(&mut b, neutre.center());
    assert_eq!(b.app.perso.actif, Some(Choix::Neutre(1)), "le clic choisit la pastille");
    assert!(b.app.stroke.is_none());
    assert_eq!(b.app.history.undo_len(), pas, "choisir n'est pas une étape d'annulation");
    assert_eq!(b.app.tool, Tool::Brush);
    let cel = b.app.cel_here();
    assert!(cel.is_none_or(|c| b.app.doc.cels.get(&c).is_none_or(|c| c.raster.is_blank())), "rien n'est dessiné");
    // Un clic sur la pastille choisie la lâche.
    clic_case(&mut b, Case::Choix(Choix::Neutre(1)));
    assert_eq!(b.app.perso.actif, None);
    // Un clic entre deux pastilles (sur la colonne) ne dessine pas non plus.
    let entre = case(&b, Case::Aucun).center() + vec2(-9.0, 0.0);
    appuyer(&mut b, entre, PointerButton::Primary);
    assert!(b.app.stroke.is_none(), "les marges de la colonne ne dessinent pas");
    relacher(&mut b, entre, PointerButton::Primary);
}

#[test]
fn the_plus_adds_a_character_and_the_empty_pastille_lets_go() {
    let mut b = banc();
    assert!(b.app.doc.personnages.is_empty());
    clic_case(&mut b, Case::Plus);
    assert_eq!(b.app.doc.personnages.len(), 1, "le « + » ajoute un personnage");
    let id = b.app.doc.personnages[0].id;
    assert_eq!(b.app.perso.actif, Some(Choix::Personnage(id)), "il est choisi tout de suite");
    assert_eq!(b.app.perso.edition, Some(Choix::Personnage(id)), "et sa fenêtre Modifier s'ouvre");
    assert!(b.app.stroke.is_none());
    // Sa pastille est dans la colonne, avant les couleurs neutres.
    let rangs: Vec<Case> = b.app.perso.cases.iter().map(|(_, c)| *c).collect();
    let p = rangs.iter().position(|c| *c == Case::Choix(Choix::Personnage(id))).expect("la pastille du personnage");
    let n = rangs.iter().position(|c| *c == Case::Choix(Choix::Neutre(0))).unwrap();
    assert!(p < n, "les personnages d'abord : {rangs:?}");
    // « Aucun personnage » le lâche.
    clic_case(&mut b, Case::Aucun);
    assert_eq!(b.app.perso.actif, None);
    // La pastille du personnage le choisit de nouveau.
    clic_case(&mut b, Case::Choix(Choix::Personnage(id)));
    assert_eq!(b.app.perso.actif, Some(Choix::Personnage(id)));
}

#[test]
fn right_click_on_a_pastille_opens_its_menu() {
    let mut b = banc();
    clic_case(&mut b, Case::Plus);
    // (La fenêtre Modifier ouverte par le « + » est fermée : elle n'est pas le sujet.)
    b.app.perso.edition = None;
    b.chauffer(3);
    let id = b.app.doc.personnages[0].id;
    let c = case(&b, Case::Choix(Choix::Personnage(id))).center();
    appuyer(&mut b, c, PointerButton::Secondary);
    relacher(&mut b, c, PointerButton::Secondary);
    assert!(b.app.stroke.is_none());
    for texte in ["Modifier…", "Dupliquer", "Monter", "Descendre", "Supprimer"] {
        assert!(b.textes.iter().any(|t| t.trim() == texte), "« {texte} » dans le menu de la pastille : {:?}", b.textes);
    }
}

#[test]
fn the_pastilles_are_also_on_the_vignettes_board() {
    let mut b = banc();
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(6);
    assert!(b.app.barre_perso_visible());
    let planche = b.app.vignettes.view;
    let plus = case(&b, Case::Plus);
    assert!(planche.contains(plus.center()), "la colonne est sur la planche : {plus:?} dans {planche:?}");
    assert!(planche.right() - plus.right() < 14.0, "au bord droit de la planche");
    let panneaux = b.app.doc.panels.len();
    let pas = b.app.history.undo_len();
    // Un clic sur une pastille la choisit et ne touche pas à la planche : ni nouveau plan, ni trait, ni annulation.
    let neutre = case(&b, Case::Choix(Choix::Neutre(2)));
    clic(&mut b, neutre.center());
    assert_eq!(b.app.perso.actif, Some(Choix::Neutre(2)));
    assert_eq!(b.app.doc.panels.len(), panneaux);
    assert_eq!(b.app.history.undo_len(), pas);
    assert!(b.app.stroke.is_none());
    // Et le « + » y ajoute un personnage aussi.
    clic_case(&mut b, Case::Plus);
    assert_eq!(b.app.doc.personnages.len(), 1);
}
