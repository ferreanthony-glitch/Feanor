//! Passe de simplicité du 2026-10-21 : les panneaux Calques, Couleur, Effets et Réglages.
//!
//! - Calques : cinq icônes (« + », dupliquer, supprimer, monter, descendre) et « … » ; la sorte de chaque ligne en
//!   icône ; rien ne se perd (le « + », le « … » et la palette de commandes portent tout ce qui a quitté la barre) ;
//! - Couleur : le nuancier d'abord (noir, cinq gris, blanc, puis les couleurs), le reste en dépliant ;
//! - Effets : chaque effet se replie ; les effets du trait ne se montrent que là où ils servent ;
//! - Réglages : peu de réglages d'abord, « Plus de réglages » replié.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};

// ------------------------------------------------------------------------------------ l'outillage

/// Une application sans fenêtre, dans un bac à sable, avec ce document.
fn app_with_doc(name: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("shot-panneaux-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app
}

/// Un plan de 24 images avec un calque par sorte donnée, du plus bas au plus haut.
fn app_layers(name: &str, kinds: &[LayerKind]) -> App {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    for (i, kind) in kinds.iter().enumerate() {
        let cel = doc.new_cel(*kind);
        let mut l = Layer::new(format!("C{}", i + 1), *kind);
        l.frames = vec![Some(cel); 24];
        doc.panels[0].layers.push(l);
    }
    app_with_doc(name, doc)
}

fn app_one_layer(name: &str, kind: LayerKind) -> App {
    app_layers(name, &[kind])
}

/// Un écran sans fenêtre : on y peint un morceau d'interface image après image, et on lit les textes peints
/// (avec leur place) ; les clics sont des événements du pointeur.
struct Ecran {
    ctx: egui::Context,
    t: f64,
}

impl Ecran {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.image_et_cases(app, events, contenu).0
    }

    /// Une image : les textes peints, et les carrés pleins (pastilles de couleur : de 14 à 32 points de côté) avec leur
    /// couleur.
    fn image_et_cases(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> (Vec<(String, Rect)>, Vec<(egui::Color32, Rect)>) {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 900.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        let mut textes = Vec::new();
        let mut cases = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
            collecter_cases(&s.shape, &mut cases);
        }
        (textes, cases)
    }

    /// Les pastilles de couleur de l'image courante du panneau (après quelques images pour qu'il s'installe).
    fn cases(&mut self, app: &mut App, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(egui::Color32, Rect)> {
        self.chauffer(app, 3, contenu);
        self.image_et_cases(app, Vec::new(), contenu).1
    }

    fn chauffer(&mut self, app: &mut App, n: usize, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new(), contenu);
        }
        textes
    }

    /// Un clic gauche en `p`, puis quelques images pour que l'interface réagisse.
    fn cliquer(&mut self, app: &mut App, p: egui::Pos2, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], contenu);
        self.chauffer(app, 3, contenu)
    }

    /// Le pointeur se pose en `p` (sans cliquer) : un sous-menu s'ouvre au survol.
    fn survoler(&mut self, app: &mut App, p: egui::Pos2, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        // (Deux temps : le premier survol ferme le sous-menu qui était ouvert, le second ouvre le nouveau.)
        self.image(app, vec![Event::PointerMoved(p - vec2(0.0, 1.0))], contenu);
        self.chauffer(app, 4, contenu);
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.chauffer(app, 6, contenu)
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn collecter_cases(shape: &Shape, out: &mut Vec<(egui::Color32, Rect)>) {
    match shape {
        Shape::Rect(r) if (14.0..=32.0).contains(&r.rect.width()) && (r.rect.width() - r.rect.height()).abs() < 0.5 && r.fill != egui::Color32::TRANSPARENT => out.push((r.fill, r.rect)),
        Shape::Vec(v) => v.iter().for_each(|s| collecter_cases(s, out)),
        _ => {}
    }
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

/// Le premier texte qui contient `morceau`.
fn trouver<'a>(textes: &'a [(String, Rect)], morceau: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.contains(morceau))
}

// ============================================================================================= 1. Calques

use egui_phosphor::regular as ph;

#[test]
fn the_layers_toolbar_has_five_icons_and_a_more_button() {
    let mut app = app_one_layer("barre", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    // Les cinq icônes et le « … ».
    for (icone, nom) in [(ph::PLUS, "+"), (ph::COPY_SIMPLE, "dupliquer"), (ph::TRASH, "supprimer"), (ph::ARROW_UP, "monter"), (ph::ARROW_DOWN, "descendre"), (ph::DOTS_THREE, "…")] {
        assert!(contient(&textes, icone), "l'icône « {nom} » est là : {textes:?}");
    }
    // Les autres ont quitté la barre : ni dossier, ni calque de couleur, ni effet, ni copier, ni coller, ni fusionner.
    for (icone, nom) in [
        (ph::FOLDER_PLUS, "nouveau dossier"),
        (ph::FOLDER_NOTCH_PLUS, "ranger dans un dossier"),
        (ph::PALETTE, "calque de couleur"),
        (ph::SPARKLE, "calque d'effet"),
        (ph::COPY, "copier"),
        (ph::CLIPBOARD, "coller"),
        (ph::ARROWS_IN_SIMPLE, "fusionner"),
        (ph::PAINT_BRUSH_BROAD, "nouveau calque bitmap"),
    ] {
        assert!(!contient(&textes, icone), "l'icône « {nom} » n'est plus dans la barre : {textes:?}");
    }
    // Les icônes de la barre sont toutes au-dessus de la liste, et « … » est le plus à droite.
    let (_, plus) = trouver(&textes, ph::PLUS).unwrap();
    let (_, plus_loin) = trouver(&textes, ph::DOTS_THREE).unwrap();
    let (_, ligne) = trouver(&textes, "C1").unwrap();
    assert!(plus.center().y < ligne.center().y && plus_loin.center().y < ligne.center().y);
    assert!(plus_loin.center().x > plus.center().x + 100.0, "« … » est à droite : {plus:?} {plus_loin:?}");
}

#[test]
fn a_layer_row_shows_its_kind_as_an_icon_not_as_a_word() {
    let mut app = app_layers("sortes", &[LayerKind::Bitmap, LayerKind::Vector]);
    let mut dossier = Layer::new_folder("Rangement");
    dossier.collapsed = false;
    app.layers_mut().push(dossier);
    app.cur_layer = 1;
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(!contient(&textes, "vectoriel") && !contient(&textes, "bitmap") && !contient(&textes, "dossier"), "plus de mot de sorte : {textes:?}");
    // Dans la liste (sous la barre) : l'icône vectoriel, l'icône bitmap, l'icône dossier.
    let (_, mode) = trouver(&textes, "Normal").unwrap();
    let dessous = |icone: &str| textes.iter().any(|(t, r)| t.contains(icone) && r.center().y > mode.bottom() + 10.0);
    assert!(dessous(ph::BEZIER_CURVE) && dessous(ph::PAINT_BRUSH_BROAD) && dessous(ph::FOLDER), "{textes:?}");
}

#[test]
fn the_kind_of_each_layer_has_its_own_icon() {
    use crate::calquemenu::Sorte;
    let mut app = app_layers("sortes-fn", &[LayerKind::Bitmap, LayerKind::Vector, LayerKind::Bitmap]);
    app.layers_mut()[2].video = Some(crate::video::VideoClip { id: 1, name: "v.mp4".into(), frames: 10, fps: 24.0, start: 0, ext: "jpg".into() });
    app.layers_mut().push(Layer::new_folder("D"));
    assert_eq!(Sorte::of(&app.layers()[0]), Sorte::Bitmap);
    assert_eq!(Sorte::of(&app.layers()[1]), Sorte::Vecto);
    assert_eq!(Sorte::of(&app.layers()[2]), Sorte::Video);
    assert_eq!(Sorte::of(&app.layers()[3]), Sorte::Dossier);
    // Sept sortes (le calque d'annotation est la septième), sept icônes différentes.
    let mut icones: Vec<&str> = Sorte::TOUTES.iter().map(|s| s.icone()).collect();
    icones.sort_unstable();
    icones.dedup();
    assert_eq!(icones.len(), 7);
}

#[test]
fn the_plus_menu_offers_every_kind_of_layer_with_a_sentence() {
    let mut app = app_one_layer("plus", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    let (_, plus) = trouver(&textes, ph::PLUS).unwrap().clone();
    let textes = ecran.cliquer(&mut app, plus.center(), &mut ui_calques);
    for s in crate::calquemenu::Sorte::TOUTES {
        assert!(contient(&textes, s.nom()), "« {} » est dans le menu : {textes:?}", s.nom());
        assert!(contient(&textes, s.phrase()), "sa phrase aussi : {}", s.phrase());
    }
    assert!(contient(&textes, "Nom, groupe, place"), "{textes:?}");
    // Le calque d'effet ouvre ses préréglages ; l'effet vide y est.
    let (_, effet) = trouver(&textes, "Calque d'effet").unwrap().clone();
    let textes = ecran.survoler(&mut app, effet.center(), &mut ui_calques);
    assert!(contient(&textes, "Calque d'effet vide"), "{textes:?}");
}

#[test]
fn the_plus_menu_creates_the_layers_the_old_buttons_did() {
    let mut app = app_one_layer("creer", LayerKind::Vector);
    let n = app.layers().len();
    // Chaque entrée lance l'action qu'elle avait déjà : le calque est le même, et les raccourcis aussi.
    for action in ["calque.nouveau_vectoriel", "calque.nouveau_bitmap", "calque.nouveau_couleur", "dossier.nouveau", "effet.vide"] {
        let avant = app.layers().len();
        // (Un calque de couleur se pose sous un calque de dessin : le courant est un dessin quand on le crée.)
        if action == "calque.nouveau_couleur" {
            app.cur_layer = 0;
        }
        app.run_action(action);
        assert!(app.layers().len() > avant, "{action} crée un calque : {}", app.status);
    }
    assert!(app.layers().len() >= n + 5, "{}", app.status);
    let sortes: Vec<_> = app.layers().iter().map(crate::calquemenu::Sorte::of).collect();
    for s in [crate::calquemenu::Sorte::Vecto, crate::calquemenu::Sorte::Bitmap, crate::calquemenu::Sorte::Dossier, crate::calquemenu::Sorte::Effet] {
        assert!(sortes.contains(&s), "{s:?} dans {sortes:?}");
    }
    // Chaque entrée du menu est une commande de la palette (Ctrl+K).
    for id in crate::calquemenu::Sorte::actions_du_menu() {
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == id && a.implemented), "{id}");
    }
    // Les raccourcis de création sont ceux d'avant.
    let km = &app.keymap;
    assert!(!km.label("calque.nouveau_vectoriel").is_empty() && !km.label("calque.nouveau_bitmap").is_empty());
}

#[test]
fn every_command_removed_from_the_layers_toolbar_is_still_reachable() {
    // Les commandes qui avaient une icône dans les deux rangées d'avant.
    let retirees = [
        "calque.nouveau_vectoriel",
        "calque.nouveau_bitmap",
        "dossier.nouveau",
        "pinceau.fond",
        "calque.effets",
        "dossier.ranger",
        "calque.nouveau_couleur",
        "effet.vide",
        "calque.copier",
        "calque.coller",
        "calque.fusionner_choisis",
        "calque.fusionner_dessous",
    ];
    for id in retirees {
        // La palette de commandes (Ctrl+K) les trouve toutes, et leur raccourci ne change pas.
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == id && a.implemented), "{id} est une commande de la palette");
    }
    // Et un menu les porte : le « + » (créer), le « … » ou le clic droit d'un calque (le reste). On lit le texte des
    // menus : chaque identifiant y figure.
    let menus = [include_str!("calquemenu.rs"), include_str!("layerops.rs"), include_str!("layersui.rs"), include_str!("fondui.rs"), include_str!("fxui.rs"), include_str!("prereglagesui.rs"), include_str!("folders.rs")].join("\n");
    // (« calque.effets » est l'interrupteur « fx » : sa case est dans le « … » par `fx_switch_item`.)
    for id in ["calque.nouveau_vectoriel", "calque.nouveau_bitmap", "dossier.nouveau", "calque.nouveau_couleur", "effet.vide", "calque.copier", "calque.coller", "calque.fusionner_choisis", "calque.fusionner_dessous", "dossier.ranger", "pinceau.fond"] {
        assert!(menus.contains(&format!("\"{id}\"")), "{id} est dans un menu");
    }
    assert!(include_str!("layersui.rs").contains("fx_switch_item") && include_str!("layersui.rs").contains("aplat_item"));
}

/// Ouvre le menu « … » du panneau Calques et rend tous les textes que ses sous-menus montrent au survol.
fn textes_du_menu_plus(app: &mut App) -> Vec<String> {
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let mut vus: Vec<String> = Vec::new();
    // (Un écran neuf par sous-menu : le survol d'un second sous-menu n'ouvre rien tant que le premier est ouvert.)
    for sous in ["Presse-papiers", "Fusionner", "Dossiers", "Aplat sous le trait", "Ordre", "Groupes"] {
        let mut ecran = Ecran::new();
        let textes = ecran.chauffer(app, 4, &mut ui_calques);
        let (_, plus) = trouver(&textes, ph::DOTS_THREE).unwrap().clone();
        let ouvert = ecran.cliquer(app, plus.center(), &mut ui_calques);
        vus.extend(ouvert.iter().map(|(t, _)| t.clone()));
        if let Some((_, r)) = trouver(&ouvert, sous).cloned() {
            let t = ecran.survoler(app, r.center(), &mut ui_calques);
            vus.extend(t.iter().map(|(t, _)| t.clone()));
        }
    }
    vus
}

#[test]
fn the_more_menu_carries_copy_paste_merge_folders_and_the_background() {
    let mut app = app_layers("plus-menu", &[LayerKind::Bitmap, LayerKind::Vector]);
    let vus = textes_du_menu_plus(&mut app);
    let a = |morceau: &str| vus.iter().any(|t| t.contains(morceau));
    assert!(a("Copier le calque") && a("Coller le calque"), "copier et coller : {vus:?}");
    assert!(a("Avec le calque du dessous"), "fusionner : {vus:?}");
    assert!(a("Placer les calques choisis dans un dossier") && a("Nouveau dossier"), "dossiers : {vus:?}");
    assert!(a("Ajouter un aplat"), "l'aplat du calque courant : {vus:?}");
    // Le « + » est dans la barre : le menu « … » ne répète ni les nouveaux calques ni dupliquer, supprimer, monter, descendre.
    assert!(!a("Nouveau calque") && !a("Monter le calque") && !a("Supprimer le calque"), "{vus:?}");
}

#[test]
fn the_layers_panel_shows_blend_mode_and_opacity_once_for_the_current_layer() {
    let mut app = app_layers("fusion", &[LayerKind::Bitmap, LayerKind::Vector, LayerKind::Vector]);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert_eq!(textes.iter().filter(|(t, _)| t.contains("Normal")).count(), 1, "un seul mode de fusion : {textes:?}");
    assert_eq!(textes.iter().filter(|(t, _)| t.contains("100 %")).count(), 1, "une seule opacité : {textes:?}");
}

// ============================================================================================= 2. Couleur

#[test]
fn the_neutral_row_is_black_five_greys_and_white_in_that_order() {
    use crate::couleur::NEUTRES;
    assert_eq!(NEUTRES.len(), 7);
    assert_eq!(NEUTRES[0].0, [0, 0, 0]);
    assert_eq!(NEUTRES[6].0, [255, 255, 255]);
    // Cinq gris, du plus sombre au plus clair, régulièrement répartis, sans teinte.
    let gris: Vec<u8> = NEUTRES[1..6].iter().map(|n| n.0[0]).collect();
    assert!(NEUTRES.iter().all(|n| n.0[0] == n.0[1] && n.0[1] == n.0[2]), "des gris");
    assert!(gris.windows(2).all(|w| w[0] < w[1]), "{gris:?}");
    let pas: Vec<i32> = NEUTRES.windows(2).map(|w| w[1].0[0] as i32 - w[0].0[0] as i32).collect();
    assert!(pas.iter().all(|p| (p - 42).abs() <= 2), "écarts réguliers : {pas:?}");
}

/// Le panneau Couleur au repos : la roue chromatique (réglage d'origine) et « Plus de couleurs » replié.
fn app_couleur(name: &str) -> App {
    let mut app = app_one_layer(name, LayerKind::Vector);
    app.couleurs.prefs.more_open = false;
    app.color = [24, 24, 28];
    app
}

#[test]
fn the_colour_panel_shows_the_swatches_first_from_top_to_bottom() {
    let mut app = app_couleur("couleur-ordre");
    let mut ecran = Ecran::new();
    let mut ui_couleur = |a: &mut App, ui: &mut egui::Ui| a.color_panel(ui);
    let cases = ecran.cases(&mut app, &mut ui_couleur);
    let textes = ecran.chauffer(&mut app, 2, &mut ui_couleur);
    // Les neutres : noir, cinq gris, blanc, de gauche à droite, sur une même rangée.
    // (Le blanc se trouve aussi dans le carré d'arrière-plan, plus haut : on prend le plus bas.)
    let neutres: Vec<&(egui::Color32, Rect)> = crate::couleur::NEUTRES
        .iter()
        .map(|(c, nom)| {
            cases.iter().filter(|(f, _)| *f == egui::Color32::from_rgb(c[0], c[1], c[2])).max_by(|a, b| a.1.top().total_cmp(&b.1.top())).unwrap_or_else(|| panic!("{nom} : {cases:?}"))
        })
        .collect();
    assert!(neutres.windows(2).all(|w| w[0].1.right() <= w[1].1.left() + 0.1 && (w[0].1.center().y - w[1].1.center().y).abs() < 0.5), "une rangée, dans l'ordre : {neutres:?}");
    // Les deux couleurs (avant-plan et arrière-plan) sont au-dessus des neutres, et les couleurs du nuancier en dessous.
    let avant = cases.iter().find(|(f, r)| *f == egui::Color32::from_rgb(24, 24, 28) && r.width() > 25.0).expect("le carré d'avant-plan");
    assert!(avant.1.center().y < neutres[0].1.top(), "les deux couleurs d'abord");
    let rouge = cases.iter().find(|(f, _)| *f == egui::Color32::from_rgb(205, 45, 45)).expect("une couleur du nuancier");
    assert!(rouge.1.top() > neutres[0].1.bottom(), "le nuancier sous les neutres : {rouge:?} / {:?}", neutres[0]);
    // Le reste est replié : ni curseurs, ni code, ni couleurs récentes.
    assert!(contient(&textes, "Plus de couleurs"), "{textes:?}");
    assert!(contient(&textes, ph::CARET_RIGHT));
    assert!(!contient(&textes, "#18181C") && !contient(&textes, "TSV") && !contient(&textes, "pour l'instant"), "replié : {textes:?}");
}

#[test]
fn the_colour_panel_top_row_has_the_palette_buttons_and_the_chooser_on_the_right() {
    let mut app = app_couleur("couleur-haut");
    let nuancier = app.shown_palette().name.clone();
    let mut ecran = Ecran::new();
    let mut ui_couleur = |a: &mut App, ui: &mut egui::Ui| a.color_panel(ui);
    let cases = ecran.cases(&mut app, &mut ui_couleur);
    let textes = ecran.chauffer(&mut app, 2, &mut ui_couleur);
    // Intervertir et les couleurs par défaut n'ont plus de bouton dans le panneau.
    assert!(!contient(&textes, ph::SWAP) && !contient(&textes, ph::SQUARE_HALF), "{textes:?}");
    // « + », « − », « … » sur la ligne des carrés, à droite d'eux, dans cet ordre ; le nuancier montré tout à droite.
    let avant = cases.iter().find(|(f, r)| *f == egui::Color32::from_rgb(24, 24, 28) && r.width() > 25.0).expect("le carré d'avant-plan").1;
    let (plus, moins, dots) = (
        trouver(&textes, ph::PLUS).expect("le + du nuancier").1,
        trouver(&textes, ph::MINUS).expect("le − du nuancier").1,
        trouver(&textes, ph::DOTS_THREE).expect("le … du nuancier").1,
    );
    let (_, nom) = trouver(&textes, &nuancier).expect("le nuancier montré");
    for b in [plus, moins, dots] {
        assert!(b.center().y > avant.top() && b.center().y < avant.bottom(), "sur la ligne des carrés : {b:?} / {avant:?}");
    }
    assert!(plus.left() > avant.right() && plus.left() < moins.left() && moins.left() < dots.left(), "+ − … dans cet ordre");
    assert!(nom.left() > dots.right(), "le nuancier à droite des commandes : {nom:?} / {dots:?}");
    let blanc = cases.iter().filter(|(f, _)| *f == egui::Color32::WHITE).map(|c| c.1).max_by(|a, b| a.top().total_cmp(&b.top())).expect("le blanc des neutres");
    assert!(blanc.right() - nom.right() < 40.0, "le nuancier calé au bord droit : {nom:?} / {blanc:?}");
}

#[test]
fn the_colour_panel_unfolds_with_one_picker_at_a_time() {
    let mut app = app_couleur("couleur-plus");
    let mut ecran = Ecran::new();
    let mut ui_couleur = |a: &mut App, ui: &mut egui::Ui| a.color_panel(ui);
    // Un clic sur « Plus de couleurs » le déplie.
    let textes = ecran.chauffer(&mut app, 3, &mut ui_couleur);
    let (_, titre) = trouver(&textes, "Plus de couleurs").unwrap().clone();
    let textes = ecran.cliquer(&mut app, titre.center(), &mut ui_couleur);
    assert!(app.couleurs.prefs.more_open, "{textes:?}");
    // La roue (réglage d'origine) : le code, les récentes et les nuanciers, mais pas les curseurs.
    assert!(contient(&textes, "#18181C") && contient(&textes, "SHOT") && contient(&textes, ph::CARET_DOWN), "{textes:?}");
    assert!(!contient(&textes, "RVB") && !contient(&textes, "TSL"), "pas les curseurs avec la roue : {textes:?}");
    // Les curseurs à la place : plus de carré ni de triangle de la roue.
    app.couleurs.prefs.wheel_shown = false;
    let textes = ecran.chauffer(&mut app, 3, &mut ui_couleur);
    assert!(contient(&textes, "RVB") && contient(&textes, "TSV") && contient(&textes, "TSL"), "{textes:?}");
    assert!(!contient(&textes, ph::SQUARE) && !contient(&textes, ph::TRIANGLE), "un seul à la fois : {textes:?}");
}

#[test]
fn the_fold_and_the_picker_are_remembered_in_the_settings_not_in_the_project() {
    let mut app = app_couleur("couleur-retenu");
    let ctx = egui::Context::default();
    app.couleurs.prefs.more_open = true;
    app.couleurs.prefs.wheel_shown = false;
    app.color_tick(&ctx);
    assert!(app.settings.colors.more_open && !app.settings.colors.wheel_shown, "enregistré dans les réglages de l'application");
    // Le fichier de réglages le relit, et un ancien fichier sans ce réglage le laisse replié.
    let text = toml::to_string(&app.settings).expect("écrit");
    let back: crate::settings::Settings = toml::from_str(&text).expect("relu");
    assert!(back.colors.more_open && !back.colors.wheel_shown);
    let ancien: crate::settings::Settings = toml::from_str("[couleurs]\nroue_visible = true\n").expect("ancien fichier");
    assert!(!ancien.colors.more_open, "replié d'office");
    // Rien dans le projet : le dossier du projet ne porte aucune trace des couleurs ou de leur pli.
    let dir = std::env::temp_dir().join(format!("shot-panneaux-couleur-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("C.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let json = std::fs::read_to_string(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(!json.contains("plus_ouvert") && !json.contains("roue_visible"), "pas dans le projet");
}

#[test]
fn a_click_on_a_grey_sets_the_foreground_colour_and_undo_leaves_the_palette_alone() {
    let mut app = app_couleur("couleur-clic");
    let mut ecran = Ecran::new();
    let mut ui_couleur = |a: &mut App, ui: &mut egui::Ui| a.color_panel(ui);
    let cases = ecran.cases(&mut app, &mut ui_couleur);
    let gris3 = crate::couleur::NEUTRES[3].0;
    let (_, r) = cases.iter().find(|(f, _)| *f == egui::Color32::from_rgb(gris3[0], gris3[1], gris3[2])).unwrap().clone();
    ecran.cliquer(&mut app, r.center(), &mut ui_couleur);
    assert_eq!(app.color, gris3);
    // L'arrière-plan (mélange de deux couleurs de certains pinceaux) est toujours là.
    app.set_back([200, 10, 10]);
    assert_eq!(app.couleurs.prefs.back, [200, 10, 10]);
    // Ctrl+Z ne défait pas le nuancier (décision d'Anthony) : ce sont des réglages de l'application.
    let avant = app.shown_palette().swatches.len();
    app.add_to_palette([1, 2, 3]);
    app.record_step();
    app.run_action("edition.annuler");
    assert_eq!(app.shown_palette().swatches.len(), avant + 1, "Ctrl+Z laisse le nuancier");
    assert_eq!(app.couleurs.prefs.back, [200, 10, 10]);
}

#[test]
fn the_new_default_palette_has_colours_only_the_neutrals_are_in_their_own_row() {
    let p = crate::nuancier::Palette::pelure();
    for (c, nom) in crate::couleur::NEUTRES {
        assert!(p.position(c).is_none(), "{nom} est dans la rangée des neutres, pas dans le nuancier");
    }
    assert_eq!(p.swatches.len(), 10);
}

// ============================================================================================= 3. Effets

#[test]
fn a_new_effect_opens_and_the_others_fold() {
    use crate::fxui::plis_apres;
    // Un calque vu pour la première fois : un ou deux effets ouverts ; plus, tout replié sauf le choisi.
    assert_eq!(plis_apres(None, 1, None), vec![true]);
    assert_eq!(plis_apres(None, 2, None), vec![true, true]);
    assert_eq!(plis_apres(None, 4, Some(2)), vec![false, false, true, false]);
    assert_eq!(plis_apres(None, 0, None), Vec::<bool>::new());
    // Un effet de plus : lui seul est ouvert, même si d'autres l'étaient.
    assert_eq!(plis_apres(Some(&[true, false, true]), 4, None), vec![false, false, false, true]);
    assert_eq!(plis_apres(Some(&[true]), 3, None), vec![false, true, true], "deux d'un coup (collage) : les deux s'ouvrent");
    // Rien de changé : les états restent. Un effet de moins : les autres gardent le leur.
    assert_eq!(plis_apres(Some(&[true, false, true]), 3, Some(0)), vec![true, false, true]);
    assert_eq!(plis_apres(Some(&[true, false, true]), 2, None), vec![true, false]);
}

fn app_avec_effets(name: &str, kind: LayerKind, kinds: &[crate::fx::EffectKind]) -> App {
    let mut app = app_one_layer(name, kind);
    for k in kinds {
        app.layers_mut()[0].fx.push(crate::fx::Effect::new(*k));
    }
    app
}

#[test]
fn each_effect_folds_and_unfolds_with_its_chevron_and_a_new_one_opens() {
    use crate::fx::EffectKind::{Glow, GaussianBlur, Invert};
    let mut app = app_avec_effets("effets-plis", LayerKind::Vector, &[GaussianBlur, Glow, Invert]);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    // Trois effets, calque vu pour la première fois : tous repliés (aucun n'est choisi).
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    let cles = |t: &[(String, Rect)]| t.iter().filter(|(s, _)| s.contains("Créer une clé")).count();
    assert_eq!(cles(&textes), 0, "tous repliés : {textes:?}");
    assert_eq!(textes.iter().filter(|(s, _)| s.contains(ph::CARET_RIGHT)).count(), 3, "trois flèches : {textes:?}");
    // Un clic sur la flèche du deuxième le déplie.
    let (_, flèche) = textes.iter().filter(|(s, _)| s.contains(ph::CARET_RIGHT)).nth(1).unwrap().clone();
    let textes = ecran.cliquer(&mut app, flèche.center(), &mut ui_fx);
    assert_eq!(cles(&textes), 1, "un seul déplié : {textes:?}");
    assert_eq!(textes.iter().filter(|(s, _)| s.trim() == ph::CARET_DOWN).count(), 1, "une seule flèche vers le bas (celle de l'effet déplié)");
    // Un nouvel effet s'ouvre, et celui qu'on avait ouvert se replie.
    app.add_effect_once(0, crate::fx::EffectKind::Posterize);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    assert_eq!(cles(&textes), 1, "le nouveau seul : {textes:?}");
    let ouvert = app.effets_ui.open.get(&app.layers()[0].id).cloned().unwrap();
    assert_eq!(ouvert, vec![false, false, false, true]);
    // Replié, l'effet joue toujours : la pile n'a pas changé.
    assert_eq!(app.layers()[0].fx.len(), 4);
    assert!(app.layers()[0].fx.iter().all(|e| e.enabled));
}

#[test]
fn the_fold_of_the_effects_is_not_kept_in_the_project_nor_the_settings() {
    let mut app = app_avec_effets("effets-session", LayerKind::Vector, &[crate::fx::EffectKind::Glow]);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    ecran.chauffer(&mut app, 3, &mut ui_fx);
    assert!(!app.effets_ui.open.is_empty());
    let text = toml::to_string(&app.settings).unwrap();
    assert!(!text.contains("open") && !text.contains("plis"), "rien dans les réglages");
}

#[test]
fn what_cannot_apply_is_hidden_not_greyed_in_the_effects_panel() {
    // Calque bitmap sans effet : pas de « Tout supprimer » ni de « Tout réinitialiser », pas de menu « Trait ».
    let mut app = app_one_layer("effets-cache", LayerKind::Bitmap);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    assert!(!contient(&textes, "Tout supprimer") && !contient(&textes, "Tout réinitialiser"), "{textes:?}");
    assert!(trouver(&textes, "Trait").is_none_or(|(t, _)| t.contains("Effets du trait")), "pas de menu Trait sur un bitmap : {textes:?}");
    assert!(contient(&textes, "Effets du trait"), "une ligne dit où le trouver : {textes:?}");
    assert!(contient(&textes, "Matière"), "la matière va sur les deux sortes de calques");
    // Calque vectoriel avec un effet : les deux boutons, le menu « Trait » (où est le tremblement).
    let mut app = app_avec_effets("effets-vecto", LayerKind::Vector, &[crate::fx::EffectKind::Glow]);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    assert!(contient(&textes, "Tout supprimer") && contient(&textes, "Tout réinitialiser"), "{textes:?}");
    assert!(textes.iter().any(|(t, _)| t.trim_start().starts_with("Trait")), "{textes:?}");
    // Il n'y a plus de bouton Copier sur chaque effet : Ctrl+C / Ctrl+V et le clic droit sur son nom.
    assert!(!contient(&textes, ph::COPY));
}

// ============================================================================================= 4. Réglages

#[test]
fn the_settings_panel_keeps_the_most_used_first_and_folds_the_rest() {
    let mut app = app_one_layer("reglages-pot", LayerKind::Bitmap);
    app.set_tool(crate::app::Tool::Fill);
    let mut ecran = Ecran::new();
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    // Le pot : la tolérance et d'où viennent les contours ; le débord est plié dans « Plus de réglages ».
    assert!(contient(&textes, "Tolérance") && contient(&textes, "Contours pris sur"), "{textes:?}");
    assert!(contient(&textes, "Plus de réglages") && !contient(&textes, "Débord") && !contient(&textes, "Astuce"), "{textes:?}");
    // Un clic sur la flèche le déplie ; l'état est retenu par outil dans les réglages de l'application.
    let (_, titre) = trouver(&textes, "Plus de réglages").unwrap().clone();
    let textes = ecran.cliquer(&mut app, titre.center(), &mut ui_outil);
    assert!(contient(&textes, "Débord"), "{textes:?}");
    assert!(app.pinceaux.plus_ouvert(crate::app::Tool::Fill.action()));
    assert!(!app.pinceaux.plus_ouvert(crate::app::Tool::Select.action()), "un autre outil reste replié");
    let text = toml::to_string(&crate::settings::Settings { brushes_panel: app.pinceaux.clone(), ..Default::default() }).unwrap();
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert!(back.brushes_panel.plus_ouvert(crate::app::Tool::Fill.action()));
    // Un fichier de réglages d'avant ne sait rien de ce pli.
    let ancien: crate::settings::Settings = toml::from_str("[panneau_pinceaux]\nliste = false\n").unwrap();
    assert!(ancien.brushes_panel.plus_outils.is_empty());
}

#[test]
fn the_brush_keeps_size_and_opacity_first_and_folds_its_extras() {
    let mut app = app_one_layer("reglages-pinceau", LayerKind::Vector);
    app.set_tool(crate::app::Tool::Brush);
    let mut ecran = Ecran::new();
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, "Taille") && contient(&textes, "Opacité"), "{textes:?}");
    assert!(contient(&textes, "Plus de réglages"), "le même titre partout : {textes:?}");
    // « Peint sur l'aplat » et l'aplat du calque sont au-dessus de « Plus de réglages », toujours visibles (Anthony,
    // 2026-10-23) ; « Remplir la forme » n'existe plus, ni la ligne du tremblement du trait.
    let y = |textes: &[(String, Rect)], t: &str| textes.iter().find(|(s, _)| s.trim() == t).map(|(_, r)| r.center().y);
    let plus = y(&textes, "Plus de réglages").expect("le titre");
    let derriere = y(&textes, ph::SELECTION_BACKGROUND).unwrap_or_else(|| panic!("« Peindre derrière » est visible replié : {textes:?}"));
    let sur_aplat = y(&textes, ph::SQUARE_HALF_BOTTOM).unwrap_or_else(|| panic!("« Peint sur l'aplat » est visible replié : {textes:?}"));
    let aplat = y(&textes, ph::USER_SQUARE).unwrap_or_else(|| panic!("l'aplat du calque est visible replié : {textes:?}"));
    assert!(derriere < plus && sur_aplat < plus && aplat < plus, "au-dessus de « Plus de réglages »");
    app.pinceaux.reglages_ouverts = true;
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, ph::SELECTION_BACKGROUND), "déplié aussi : {textes:?}");
    // (L'icône du tremblement, une sinusoïde, sert aussi à la catégorie « Lissage » : on cherche sa phrase.)
    for absent in [ph::PAINT_BUCKET, "Le trait de ce calque", "Remplir la forme"] {
        assert!(!contient(&textes, absent), "« {absent} » n'est plus dans les réglages du pinceau : {textes:?}");
    }
    // La gomme n'a ni l'un ni l'autre.
    app.set_tool(crate::app::Tool::Eraser);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(!contient(&textes, ph::SELECTION_BACKGROUND) && !contient(&textes, ph::SQUARE_HALF_BOTTOM), "{textes:?}");
}

#[test]
fn the_selection_tool_hides_what_needs_a_selection_instead_of_greying_it() {
    let mut app = app_one_layer("reglages-selection", LayerKind::Vector);
    app.set_tool(crate::app::Tool::Select);
    let mut ecran = Ecran::new();
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, ph::SELECTION_ALL), "tout sélectionner est toujours là : {textes:?}");
    for (icone, nom) in [(ph::COPY, "copier"), (ph::TRASH, "supprimer"), (ph::FLIP_HORIZONTAL, "miroir"), (ph::DROP, "couleur courante"), (ph::CLIPBOARD, "coller")] {
        assert!(!contient(&textes, icone), "« {nom} » est caché sans sélection : {textes:?}");
    }
}

#[test]
fn the_transform_tool_keeps_its_fields_first_and_folds_the_position_keys() {
    let mut app = app_one_layer("reglages-transformer", LayerKind::Bitmap);
    app.set_tool(crate::app::Tool::Transform);
    let mut ecran = Ecran::new();
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, "largeur") || contient(&textes, "Plus de réglages"), "{textes:?}");
    assert!(contient(&textes, "Plus de réglages") && !contient(&textes, "Clés de position"), "les clés sont pliées : {textes:?}");
    app.pinceaux.regler_plus(crate::app::Tool::Transform.action(), true);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, "Clés de position"), "dépliées : {textes:?}");
}