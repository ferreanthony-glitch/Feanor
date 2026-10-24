//! La liste des pinceaux dans le panneau Réglages de l'outil (2026-10-24) : le panneau « Pinceaux » n'existe plus, sa
//! liste se déplie sous l'aperçu du trait, par un bouton à l'extrême droite de l'aperçu. Ces essais peignent le
//! panneau image après image, lisent les textes et les aperçus peints (avec leur place) et cliquent dedans.
//! (Les anciennes dispositions : `dock_tests`, `barrepanneaux_tests` ; l'ordre des dossiers : `pinceaux_tests`.)

use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::brushtex::Target;
use crate::dock::Pane;

/// Largeur de l'écran des essais : celle d'un panneau de côté un peu large.
const LARGEUR: f32 = 360.0;

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-pinceaux-reglages-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    // (La bibliothèque des pinceaux importés se lit sur un autre fil : on l'attend.)
    let debut = std::time::Instant::now();
    while app.brushes_job.is_some() && debut.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(2));
        app.poll_brushes();
    }
    app
}

/// Ce qu'une image du panneau a peint : les textes (avec leur place) et les aperçus du trait (images de 38 points de haut).
struct Peint {
    textes: Vec<(String, Rect)>,
    apercus: Vec<Rect>,
}

impl Peint {
    fn texte(&self, morceau: &str) -> Option<Rect> {
        self.textes.iter().find(|(t, _)| t.contains(morceau)).map(|(_, r)| *r)
    }

    fn contient(&self, morceau: &str) -> bool {
        self.texte(morceau).is_some()
    }
}

fn lire(shape: &Shape, p: &mut Peint) {
    match shape {
        Shape::Text(t) => p.textes.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Mesh(m) => {
            let r = m.calc_bounds();
            if (r.height() - 38.0).abs() < 0.5 {
                p.apercus.push(r);
            }
        }
        Shape::Vec(v) => v.iter().for_each(|s| lire(s, p)),
        _ => {}
    }
}

struct Ecran {
    ctx: egui::Context,
    t: f64,
}

impl Ecran {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Peint {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(LARGEUR, 900.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        let mut p = Peint { textes: Vec::new(), apercus: Vec::new() };
        for s in &out.shapes {
            lire(&s.shape, &mut p);
        }
        p
    }

    fn chauffer(&mut self, app: &mut App, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Peint {
        let mut p = self.image(app, Vec::new(), contenu);
        for _ in 0..4 {
            p = self.image(app, Vec::new(), contenu);
        }
        p
    }

    /// Un clic gauche en `p`, puis quelques images pour que l'interface réagisse.
    fn cliquer(&mut self, app: &mut App, p: egui::Pos2, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Peint {
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], contenu);
        self.chauffer(app, contenu)
    }
}

fn reglages(a: &mut App, ui: &mut egui::Ui) {
    a.brush_settings_ui(ui);
}

#[test]
fn the_list_button_sits_at_the_far_right_of_the_stroke_preview() {
    let mut app = app("bouton");
    app.set_tool(Tool::Brush);
    let mut ecran = Ecran::new();
    let p = ecran.chauffer(&mut app, &mut reglages);
    let bouton = p.texte(ph::BOOKS).unwrap_or_else(|| panic!("le bouton de la liste est là : {:?}", p.textes));
    let apercu = *p.apercus.first().unwrap_or_else(|| panic!("l'aperçu du trait est peint : {:?}", p.apercus));
    // À droite de l'aperçu, à sa hauteur, et tout au bord droit du panneau.
    assert!(bouton.left() >= apercu.right(), "à droite de l'aperçu : {bouton:?} / {apercu:?}");
    assert!(bouton.center().y > apercu.top() && bouton.center().y < apercu.bottom(), "à la hauteur de l'aperçu : {bouton:?} / {apercu:?}");
    assert!(bouton.right() > LARGEUR - 30.0, "tout à droite : {bouton:?}");
    // L'aperçu, lui, prend le reste de la largeur (il n'est pas rétréci à un timbre).
    assert!(apercu.width() > LARGEUR * 0.6, "{apercu:?}");
    // Replié d'office : ni liste, ni recherche, ni « Mes brosses ».
    assert!(!app.pinceaux.liste_ouverte);
    assert!(!p.contient("Mes brosses") && !p.contient("Rechercher"), "{:?}", p.textes);
}

#[test]
fn the_button_unfolds_and_folds_the_list_under_the_preview() {
    let mut app = app("deplier");
    app.set_tool(Tool::Brush);
    let mut ecran = Ecran::new();
    let p = ecran.chauffer(&mut app, &mut reglages);
    let apercu = p.apercus[0];
    let bouton = p.texte(ph::BOOKS).expect("le bouton");
    // Un clic : la liste se déplie sous l'aperçu, avec sa recherche, « Mes brosses » et les dossiers de SHOT.
    let p = ecran.cliquer(&mut app, bouton.center(), &mut reglages);
    assert!(app.pinceaux.liste_ouverte, "le clic déplie la liste");
    for attendu in ["Rechercher", "Mes brosses", "Crayons", "Encres et feutres"] {
        let r = p.texte(attendu).unwrap_or_else(|| panic!("« {attendu} » dans la liste : {:?}", p.textes));
        assert!(r.top() > apercu.bottom(), "« {attendu} » est sous l'aperçu : {r:?} / {apercu:?}");
    }
    // Les trois boutons du bas : importer, garder le réglage courant, supprimer.
    for icone in [ph::FILE_ARROW_DOWN, ph::PLUS, ph::TRASH] {
        assert!(p.contient(icone), "le bouton {icone:?} est en bas de la liste : {:?}", p.textes);
    }
    // « Plus de réglages » reste là, sous la liste.
    assert!(p.texte("Plus de réglages").is_some_and(|r| r.top() > p.texte("Mes brosses").unwrap().bottom()));
    // Le bouton reste au même endroit (l'aperçu ne bouge pas), et un second clic replie la liste.
    let bouton2 = p.texte(ph::BOOKS).expect("le bouton");
    assert!((bouton2.center() - bouton.center()).length() < 1.0, "{bouton:?} -> {bouton2:?}");
    let p = ecran.cliquer(&mut app, bouton2.center(), &mut reglages);
    assert!(!app.pinceaux.liste_ouverte, "le second clic replie la liste");
    assert!(!p.contient("Mes brosses") && !p.contient("Rechercher"), "{:?}", p.textes);
}

#[test]
fn my_brushes_are_the_first_folder_in_the_unfolded_list() {
    let mut app = app("mes-brosses");
    app.set_tool(Tool::Brush);
    app.pinceaux.liste_ouverte = true;
    let gardee = app.brush;
    app.brushes.save_mine("Ma plume", Target::Brush, gardee);
    let mut ecran = Ecran::new();
    let p = ecran.chauffer(&mut app, &mut reglages);
    let y = |t: &str| p.texte(t).unwrap_or_else(|| panic!("« {t} » : {:?}", p.textes)).center().y;
    assert!(y("Mes brosses") < y("Crayons") && y("Mes brosses") < y("Encres et feutres"), "{:?}", p.textes);
    assert!(y("Mes brosses") < y("Ma plume") && y("Ma plume") < y("Crayons"), "la brosse gardée est dans Mes brosses, avant celles de SHOT");
}

#[test]
fn choosing_a_brush_in_the_list_applies_it_like_the_old_panel_did() {
    let mut app = app("choisir");
    app.set_tool(Tool::Brush);
    app.pinceaux.liste_ouverte = true;
    // Une recherche ne laisse que « Gouache » : sa ligne est tout de suite visible.
    app.brush_previews.search = "gouache".into();
    let voulu = app.brushes.presets.iter().find(|p| p.name == "Gouache" && p.target == Target::Brush).expect("la gouache de SHOT").clone();
    assert_ne!(app.brush, voulu.settings, "le pinceau du moment n'est pas la gouache");
    let mut ecran = Ecran::new();
    let p = ecran.chauffer(&mut app, &mut reglages);
    let ligne = p.texte("Gouache").unwrap_or_else(|| panic!("la ligne « Gouache » : {:?}", p.textes));
    ecran.cliquer(&mut app, ligne.center(), &mut reglages);
    assert_eq!(app.brush, voulu.settings, "le pinceau prend le réglage choisi");
    assert_eq!(app.tool, Tool::Brush);
    assert_eq!(app.status, "Réglage « Gouache ».");
    assert_eq!(app.brush_previews.base, Some((Target::Brush, voulu.name.clone(), voulu.source.clone())), "c'est la brosse que « Enregistrer » remplacera");
    // La liste reste dépliée (on peut en essayer une autre).
    assert!(app.pinceaux.liste_ouverte);
    // Avec la gomme, la liste est celle des gommes, et le choix va à la gomme.
    app.set_tool(Tool::Eraser);
    app.brush_previews.search = "grain".into();
    let gomme = app.brushes.presets.iter().find(|p| p.name == "Gomme à grain" && p.target == Target::Eraser).expect("la gomme à grain").clone();
    let p = ecran.chauffer(&mut app, &mut reglages);
    assert!(p.contient("Gommes") && !p.contient("Crayons"), "{:?}", p.textes);
    let ligne = p.texte("Gomme à grain").unwrap_or_else(|| panic!("la ligne « Gomme à grain » : {:?}", p.textes));
    ecran.cliquer(&mut app, ligne.center(), &mut reglages);
    assert_eq!(app.eraser, gomme.settings);
    assert_eq!(app.tool, Tool::Eraser);
}

#[test]
fn everything_the_old_panel_offered_is_still_reachable_from_the_list() {
    let mut app = app("tout");
    app.set_tool(Tool::Brush);
    app.pinceaux.liste_ouverte = true;
    let mut ecran = Ecran::new();
    let p = ecran.chauffer(&mut app, &mut reglages);
    // Recherche, liste ou vignettes, importer, nouvelle brosse, supprimer.
    for icone in [ph::LIST_BULLETS, ph::SQUARES_FOUR, ph::FILE_ARROW_DOWN, ph::PLUS, ph::TRASH] {
        assert!(p.contient(icone), "{icone:?} : {:?}", p.textes);
    }
    // « Nouvelle brosse » demande un nom, dans la liste ; plier la liste le laisse de côté.
    let plus = p.texte(ph::PLUS).unwrap();
    let p = ecran.cliquer(&mut app, plus.center(), &mut reglages);
    assert!(app.preset_edit.as_ref().is_some_and(|e| !e.summary), "la saisie du nom est lancée");
    assert!(p.contient("Enregistrer sous"), "{:?}", p.textes);
    let bouton = p.texte(ph::BOOKS).expect("le bouton");
    let p = ecran.cliquer(&mut app, bouton.center(), &mut reglages);
    assert!(!app.pinceaux.liste_ouverte && app.preset_edit.is_none(), "la liste repliée ne garde pas une saisie invisible");
    assert!(!p.contient("Enregistrer sous"), "{:?}", p.textes);
    // Le bouton « Enregistrer » (disquette) du résumé, lui, reste dans le résumé.
    let resume = ecran.chauffer(&mut app, &mut |a, ui| a.brush_summary_ui(ui));
    assert!(resume.contient(ph::FLOPPY_DISK));
}

#[test]
fn the_settings_panel_of_the_other_tools_has_no_list_button() {
    let mut app = app("autres");
    app.pinceaux.liste_ouverte = true;
    let mut ecran = Ecran::new();
    for tool in [Tool::Fill, Tool::Select, Tool::Text, Tool::Line, Tool::Transform] {
        app.set_tool(tool);
        let p = ecran.chauffer(&mut app, &mut |a, ui| a.tool_options_panel(ui));
        assert!(!p.contient(ph::BOOKS) && !p.contient("Mes brosses") && !p.contient("Rechercher"), "{tool:?} : {:?}", p.textes);
    }
    // Le pinceau, le crayon et la gomme, eux, l'ont.
    for tool in [Tool::Brush, Tool::Pencil, Tool::Eraser] {
        app.set_tool(tool);
        let p = ecran.chauffer(&mut app, &mut |a, ui| a.tool_options_panel(ui));
        assert!(p.contient(ph::BOOKS) && p.contient("Mes brosses"), "{tool:?} : {:?}", p.textes);
    }
}

#[test]
fn the_old_open_brushes_action_opens_the_settings_with_the_list_unfolded() {
    let mut app = app("action");
    // Les réglages fermés, un outil sans brosses : l'action ouvre les réglages, prend le pinceau, déplie la liste.
    app.set_tool(Tool::Select);
    app.toggle_pane(Pane::ToolOptions);
    assert!(!app.dock_layout().contains(Pane::ToolOptions));
    assert!(!app.pinceaux.liste_ouverte);
    app.run_action("vue.pinceaux");
    assert!(app.dock_layout().contains(Pane::ToolOptions), "les réglages de l'outil sont ouverts");
    assert!(!app.dock_layout().contains(Pane::Brushes), "et il n'y a pas de panneau Pinceaux");
    assert_eq!(app.tool, Tool::Brush, "le pinceau est l'outil, la liste est la sienne");
    assert!(app.pinceaux.liste_ouverte, "la liste est dépliée");
    // Comme l'ancien panneau (qu'un second appui fermait) : un second appui replie la liste.
    app.run_action("vue.pinceaux");
    assert!(!app.pinceaux.liste_ouverte);
    app.run_action("vue.pinceaux");
    assert!(app.pinceaux.liste_ouverte);
    // Avec la gomme, on reste à la gomme (sa liste).
    app.pinceaux.liste_ouverte = false;
    app.set_tool(Tool::Eraser);
    app.run_action("vue.pinceaux");
    assert_eq!(app.tool, Tool::Eraser);
    assert!(app.pinceaux.liste_ouverte);
    // L'action est toujours dans la palette (et garde le raccourci qu'on lui avait donné).
    assert!(crate::keymap::action_info("vue.pinceaux").is_some_and(|a| a.implemented));
}
