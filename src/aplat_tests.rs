//! L'aplat sous le trait s'active depuis le calque (Anthony, 2026-10-21) : sa sous-ligne fine dans le panneau Calques
//! (et nulle part ailleurs) en fait la cible du dessin ; le pinceau, la gomme et le pot de peinture travaillent alors
//! l'aplat seul ; un clic sur le calque revient au trait. Ses réglages sont dans Réglages et dans le clic droit de la
//! sous-ligne. Le code dit encore `fond` (`Layer::fond`) : le format des projets ne change pas.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc, se_reposer, Banc};

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
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 900.0))), time: Some(self.t), events, ..RawInput::default() };
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

    /// Un clic (`bouton`) en `p`, puis quelques images pour que l'interface réagisse.
    fn cliquer_avec(&mut self, app: &mut App, p: egui::Pos2, bouton: PointerButton, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: bouton, pressed: true, modifiers: Modifiers::NONE }], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: bouton, pressed: false, modifiers: Modifiers::NONE }], contenu);
        self.chauffer(app, 3, contenu)
    }

    fn cliquer(&mut self, app: &mut App, p: egui::Pos2, contenu: &mut dyn FnMut(&mut App, &mut egui::Ui)) -> Vec<(String, Rect)> {
        self.cliquer_avec(app, p, PointerButton::Primary, contenu)
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn exact<'a>(textes: &'a [(String, Rect)], texte: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.trim() == texte)
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

/// Trois calques bitmap, de bas en haut A, B, C (le plan a 6 images).
fn app_abc(name: &str) -> App {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(6);
    for n in ["A", "B", "C"] {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let mut l = Layer::new(n, LayerKind::Bitmap);
        l.frames = vec![Some(cel); 6];
        doc.panels[0].layers.push(l);
    }
    let dir = std::env::temp_dir().join(format!("shot-aplat-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 2;
    app.cur_frame = 0;
    app
}

/// Un plan de 400 × 300 avec un calque bitmap, un anneau tracé au pinceau et un aplat sur le calque.
fn banc_anneau() -> Banc {
    let mut b = banc(Document::new(400, 300, 24), false);
    b.app.tool = Tool::Brush;
    trait_carre(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    b.app.toggle_fond(0);
    b
}

/// Un anneau carré tracé au pinceau, comme une main : un seul trait fermé.
fn trait_carre(app: &mut App, x0: f32, y0: f32, x1: f32, y1: f32) {
    let mut pts = Vec::new();
    let mut x = x0;
    while x < x1 {
        pts.push(pos2(x, y0));
        x += 4.0;
    }
    let mut y = y0;
    while y < y1 {
        pts.push(pos2(x1, y));
        y += 4.0;
    }
    let mut x = x1;
    while x > x0 {
        pts.push(pos2(x, y1));
        x -= 4.0;
    }
    let mut y = y1;
    while y > y0 - 8.0 {
        pts.push(pos2(x0, y));
        y -= 4.0;
    }
    app.begin_paint(pts[0], 1.0, false);
    for p in &pts[1..] {
        app.stroke_to(*p, 1.0);
    }
    app.end_stroke();
}

fn gliss(app: &mut App, a: egui::Pos2, b: egui::Pos2) {
    app.begin_paint(a, 1.0, false);
    for i in 1..=12 {
        let t = i as f32 / 12.0;
        app.stroke_to(a + (b - a) * t, 1.0);
    }
    app.end_stroke();
}

/// Le trait du calque 0 (à plat) et l'aplat de son dessin : (trait, peinture de l'aplat, coupe de l'aplat).
fn etat(app: &App) -> (Vec<egui::Color32>, Vec<egui::Color32>, Vec<egui::Color32>) {
    let cel = app.layers()[0].frames[0].expect("dessin");
    let c = &app.doc.cels[&cel];
    let (p, k) = match c.fond.as_ref() {
        Some(s) => (s.peinture.raster.to_flat(), s.coupe.to_flat()),
        None => (Vec::new(), Vec::new()),
    };
    (c.raster.to_flat(), p, k)
}

fn noms(app: &App) -> Vec<String> {
    app.history.names().0.iter().map(|n| n.to_string()).collect()
}

// ------------------------------------------------------------------------------------ l'activation

#[test]
fn the_aplat_is_activated_from_the_layer_only() {
    let mut app = app_abc("activation");
    let ph_aplat = egui_phosphor::regular::USER_SQUARE;
    let mut ecran = Ecran::new();
    // Dans les réglages de chaque outil de dessin, avec ou sans aplat sur le calque courant : ni bascule « Peindre dans »,
    // ni réglages d'aplat. Le pinceau et le crayon ont le bouton d'aplat du calque courant (le même que sur sa ligne,
    // Anthony, 2026-10-23, à côté de « Peint sur l'aplat ») ; la gomme, le pot et le cutter non.
    app.toggle_fond(2);
    assert!(app.layers()[2].fond.is_some());
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    for tool in [Tool::Brush, Tool::Pencil, Tool::Eraser, Tool::Fill, Tool::Cutter] {
        app.set_tool(tool);
        let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
        // (« Débord » est un réglage du pot de peinture lui-même, pas de l'aplat.)
        for interdit in ["Peindre dans", "Couleur de l'aplat", "Couleur du fond", "Fermer les trous", "Le trait", "Le fond"] {
            assert!(!contient(&textes, interdit), "{tool:?} : « {interdit} » ne devrait plus être dans les réglages : {textes:?}");
        }
        let bouton = matches!(tool, Tool::Brush | Tool::Pencil);
        assert_eq!(contient(&textes, ph_aplat), bouton, "{tool:?} : le bouton d'aplat du calque : {textes:?}");
    }
    // L'action de bascule du trait / fond n'existe plus.
    assert!(crate::keymap::ACTIONS.iter().all(|a| a.id != "pinceau.strate_fond"));
    // Le bouton de la ligne du calque : un par calque de dessin (plus de bouton dans les commandes : l'aplat du calque
    // courant est dans le menu « … », `panneaux_tests.rs`).
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert_eq!(textes.iter().filter(|(t, _)| t.contains(ph_aplat)).count(), 3, "{textes:?}");
}

#[test]
fn the_layer_menu_adds_and_removes_the_aplat() {
    let mut app = app_abc("menu");
    let mut ecran = Ecran::new();
    // Le clic droit du calque B (qui n'est pas le courant) : « Ajouter un aplat ».
    let mut menu_b = |a: &mut App, ui: &mut egui::Ui| a.layer_full_menu(ui, 1, false);
    let textes = ecran.chauffer(&mut app, 4, &mut menu_b);
    assert!(exact(&textes, "Retirer l'aplat").is_none(), "{textes:?}");
    let (_, bouton) = exact(&textes, "Ajouter un aplat").expect("l'entrée du menu").clone();
    let n0 = noms(&app).len();
    ecran.cliquer(&mut app, bouton.center(), &mut menu_b);
    assert!(app.layers()[1].fond.is_some() && app.layers()[0].fond.is_none() && app.layers()[2].fond.is_none());
    assert_eq!(app.cur_layer, 2, "le calque courant n'a pas changé");
    assert_eq!(noms(&app).len(), n0 + 1);
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : ajouter"));
    // Et il se retire de la même façon.
    let textes = ecran.chauffer(&mut app, 4, &mut menu_b);
    let (_, bouton) = exact(&textes, "Retirer l'aplat").expect("l'entrée devient « Retirer l'aplat »").clone();
    ecran.cliquer(&mut app, bouton.center(), &mut menu_b);
    assert!(app.layers()[1].fond.is_none());
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : retirer"));
    app.undo();
    assert!(app.layers()[1].fond.is_some(), "Ctrl+Z remet l'aplat");
}

#[test]
fn the_old_actions_still_toggle_the_aplat_of_the_layers() {
    let mut app = app_abc("alias");
    app.run_action("pinceau.fond");
    assert!(app.layers()[2].fond.is_some(), "le calque courant");
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : ajouter"));
    app.run_action("pinceau.fond");
    assert!(app.layers()[2].fond.is_none());
    // Les calques choisis (Ctrl + clic), puis tout le plan, puis les retirer.
    app.layer_sel = vec![app.layers()[0].id];
    app.run_action("fond.calques_choisis");
    assert!(app.layers()[0].fond.is_some() && app.layers()[2].fond.is_some() && app.layers()[1].fond.is_none());
    assert!(noms(&app).last().is_some_and(|n| n.contains("Aplat")), "{:?}", noms(&app));
    app.run_action("fond.tout_le_plan");
    assert!(app.layers().iter().all(|l| l.fond.is_some()));
    app.run_action("fond.retirer_choisis");
    assert!(app.layers()[0].fond.is_none() && app.layers()[2].fond.is_none() && app.layers()[1].fond.is_some());
    // Leur nom dans la palette (Ctrl+K) et dans la liste des raccourcis dit « aplat ».
    for id in ["pinceau.fond", "fond.calques_choisis", "fond.tout_le_plan", "fond.retirer_choisis", "fond.couleur_partout"] {
        let a = crate::keymap::ACTIONS.iter().find(|a| a.id == id).expect(id);
        assert!(a.label.to_lowercase().contains("aplat") && !a.label.contains("Fond sous"), "{id} : {}", a.label);
    }
}

// ------------------------------------------------------------------------------------ la sous-ligne

#[test]
fn the_sub_row_is_thin_and_only_in_the_layers_panel() {
    let mut app = app_abc("souslignes");
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let sans = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(exact(&sans, "Aplat").is_none(), "pas d'aplat, pas de sous-ligne : {sans:?}");
    let y = |t: &[(String, Rect)], nom: &str| exact(t, nom).unwrap_or_else(|| panic!("{nom} : {t:?}")).1.center().y;
    let ecart_sans = y(&sans, "B") - y(&sans, "C");
    // Un aplat sur C : sa sous-ligne vient juste sous le rang de C, avant celui de B.
    app.toggle_fond(2);
    let avec = ecran.chauffer(&mut app, 4, &mut ui_calques);
    let (_, sous) = exact(&avec, "Aplat").expect("la sous-ligne").clone();
    assert!(sous.center().y > y(&avec, "C") && sous.center().y < y(&avec, "B"), "entre C et B : {avec:?}");
    let ecart_avec = y(&avec, "B") - y(&avec, "C");
    let en_plus = ecart_avec - ecart_sans;
    assert!(en_plus > 8.0 && en_plus <= 24.0, "la sous-ligne est fine : {en_plus} px de plus qu'un rang (qui en fait {ecart_sans})");
    assert!(en_plus * 2.0 <= ecart_sans + 2.0, "moins de la moitié d'un rang de calque ({en_plus} contre {ecart_sans})");
    // Ni la timeline ni le montage n'ont de sous-ligne (on y voit pourtant les calques).
    let mut ui_timeline = |a: &mut App, ui: &mut egui::Ui| a.timeline_panel(ui);
    let timeline = ecran.chauffer(&mut app, 4, &mut ui_timeline);
    assert!(exact(&timeline, "C").is_some() && exact(&timeline, "B").is_some(), "la timeline montre bien les calques : {timeline:?}");
    assert!(!contient(&timeline, "Aplat"), "{timeline:?}");
    app.set_espace(crate::workspace::Espace::Montage);
    let mut ui_montage = |a: &mut App, ui: &mut egui::Ui| a.montage_panel(ui);
    let montage = ecran.chauffer(&mut app, 4, &mut ui_montage);
    assert!(!contient(&montage, "Aplat"), "{montage:?}");
    // Une sous-ligne par aplat : B aussi.
    app.toggle_fond(1);
    let deux = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert_eq!(deux.iter().filter(|(t, _)| t.trim() == "Aplat").count(), 2);
    // Retiré, elle part.
    app.toggle_fond(1);
    app.toggle_fond(2);
    let plus = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(exact(&plus, "Aplat").is_none());
}

#[test]
fn a_sub_row_follows_the_visibility_of_its_folder() {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(6);
    let folder = Layer::new_folder("Décors");
    let mut layers = Vec::new();
    for n in ["C1", "C2"] {
        let cel = doc.new_cel(LayerKind::Vector);
        let mut l = Layer::new(n, LayerKind::Vector);
        l.frames = vec![Some(cel); 6];
        l.parent = Some(folder.id);
        layers.push(l);
    }
    layers.push(folder);
    let cel = doc.new_cel(LayerKind::Vector);
    let mut hors = Layer::new("Hors dossier", LayerKind::Vector);
    hors.frames = vec![Some(cel); 6];
    layers.push(hors);
    crate::folders::normalize(&mut layers);
    doc.panels[0].layers = layers;
    let mut app = app_abc("dossier");
    app.doc = doc;
    app.cur_layer = 3;
    app.toggle_fond(0);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let ouvert = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(exact(&ouvert, "C1").is_some() && exact(&ouvert, "Aplat").is_some(), "{ouvert:?}");
    // Le dossier fermé cache C1 et sa sous-ligne.
    let fi = app.layers().iter().position(|l| l.folder).unwrap();
    app.layers_mut()[fi].collapsed = true;
    let ferme = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(exact(&ferme, "C1").is_none() && exact(&ferme, "Aplat").is_none(), "{ferme:?}");
    // Mais le calque courant, lui, reste visible : sa sous-ligne aussi.
    app.cur_layer = 0;
    let courant = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(exact(&courant, "C1").is_some() && exact(&courant, "Aplat").is_some(), "{courant:?}");
}

#[test]
fn clicking_the_sub_row_aims_the_drawing_at_the_aplat_and_the_layer_gives_the_line_back() {
    let mut app = app_abc("cible");
    app.toggle_fond(1);
    app.set_tool(Tool::Select);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert_eq!(app.cur_layer, 2);
    assert!(!app.aplat_en_cible());
    // Un clic sur la sous-ligne de B : B devient courant, son aplat la cible, un outil qui dessine est pris.
    let (_, sous) = exact(&textes, "Aplat").unwrap().clone();
    let n0 = noms(&app).len();
    let textes = ecran.cliquer(&mut app, sous.center(), &mut ui_calques);
    assert_eq!(app.cur_layer, 1);
    assert!(app.aplat_en_cible(), "{}", app.status);
    assert_eq!(app.tool, Tool::Select, "l'outil Sélection reste : il prend les pixels de l'aplat (2026-10-28)");
    assert!(app.status.starts_with("Aplat de B"), "la barre d'état le dit : {}", app.status);
    // Un outil qui ne travaille pas l'aplat (une forme) cède la place au pinceau.
    app.set_tool(Tool::Rect);
    app.select_aplat(1);
    assert_eq!(app.tool, Tool::Brush, "une forme cède la place au pinceau");
    assert_eq!(noms(&app).len(), n0, "choisir la cible n'est pas une étape d'annulation");
    assert!(contient(&textes, "Aplat"));
    // Le même clic sur la ligne du calque B ramène au trait.
    let (_, nom) = exact(&textes, "B").unwrap().clone();
    ecran.cliquer(&mut app, nom.center(), &mut ui_calques);
    assert_eq!(app.cur_layer, 1);
    assert!(!app.aplat_en_cible(), "le calque redonne le trait");
    // Cliquer l'aplat puis un autre calque : le trait aussi.
    app.select_aplat(1);
    assert!(app.aplat_en_cible());
    app.select_layer(0);
    assert!(!app.aplat_en_cible());
    app.select_layer(1);
    assert!(!app.aplat_en_cible(), "revenir sur le calque ne rallume pas la cible");
    // Retirer l'aplat ou l'annuler ôte la cible.
    app.select_aplat(1);
    app.toggle_fond(1);
    assert!(app.aplat_cible.is_none() && !app.aplat_en_cible());
    app.undo();
    app.tick_aplat();
    assert!(!app.aplat_en_cible());
}

#[test]
fn the_selected_sub_row_is_highlighted_and_the_canvas_hint_says_which_aplat() {
    let mut app = app_abc("indication");
    app.toggle_fond(2);
    app.select_aplat(2);
    let hint = app.aplat_indication();
    assert!(hint.starts_with("Aplat de C"), "{hint}");
    app.set_tool(Tool::Fill);
    assert!(app.aplat_indication().contains("remplir"));
    app.set_tool(Tool::Eraser);
    assert!(app.aplat_indication().contains("gomme"));
    // La ligne du bas de la vue le montre.
    let mut ecran = Ecran::new();
    let mut ui_barre = |a: &mut App, ui: &mut egui::Ui| a.view_tools_bar(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_barre);
    assert!(textes.iter().any(|(t, _)| t.starts_with("Aplat de C")), "{textes:?}");
    app.aplat_cible = None;
    let textes = ecran.chauffer(&mut app, 4, &mut ui_barre);
    assert!(!textes.iter().any(|(t, _)| t.starts_with("Aplat de")), "{textes:?}");
}

// ------------------------------------------------------------------------------------ le dessin

#[test]
fn brush_eraser_and_bucket_work_the_aplat_only_when_it_is_the_target() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    let (trait0, peinture0, coupe0) = etat(app);
    let peint = |p: &[egui::Color32]| p.iter().filter(|c| c.a() > 0).count();
    assert_eq!(peint(&peinture0), 0, "rien n'est encore peint dans l'aplat");
    app.select_aplat(0);
    assert!(app.aplat_en_cible());
    // Le pinceau : le trait ne bouge pas, la peinture de l'aplat oui, une seule étape « Aplat : peinture ».
    app.color = [10, 200, 10];
    let n0 = noms(app).len();
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    let (trait1, peinture1, coupe1) = etat(app);
    assert_eq!(trait1, trait0, "le trait n'est pas touché");
    assert!(peint(&peinture1) > 0, "la peinture de l'aplat a changé");
    assert_eq!(noms(app).len(), n0 + 1, "un geste, une étape : {:?}", noms(app));
    assert_eq!(noms(app).last().map(String::as_str), Some("Aplat : peinture"));
    assert_eq!(coupe1.iter().filter(|c| c.a() > 0).count(), coupe0.iter().filter(|c| c.a() > 0).count());
    // La gomme : le trait ne bouge pas, l'aplat est coupé.
    app.set_tool(Tool::Eraser);
    gliss(app, pos2(90.0, 130.0), pos2(130.0, 130.0));
    let (trait2, _, coupe2) = etat(app);
    assert_eq!(trait2, trait0, "la gomme ne touche pas au trait");
    assert!(coupe2.iter().filter(|c| c.a() > 0).count() > coupe1.iter().filter(|c| c.a() > 0).count(), "la gomme coupe l'aplat");
    assert_eq!(noms(app).last().map(String::as_str), Some("Aplat : gomme"));
    // Le pot de peinture : la zone fermée par le trait se remplit dans l'aplat.
    app.set_tool(Tool::Fill);
    app.color = [200, 10, 10];
    app.fill_at(pos2(80.0, 80.0));
    let (trait3, peinture3, _) = etat(app);
    assert_eq!(trait3, trait0, "le pot de peinture ne touche pas au trait");
    let i = 80 * app.doc.width + 80;
    assert_eq!((peinture3[i].r(), peinture3[i].a()), (200, 255), "le pot a rempli l'aplat");
    assert!(noms(app).last().is_some_and(|n| n.starts_with("Aplat")), "{:?}", noms(app));
    // Chaque étape s'annule.
    app.undo();
    assert_eq!(etat(app).1[i].a(), 0, "annuler le remplissage");
    app.undo();
    app.undo();
    assert_eq!(peint(&etat(app).1), 0, "tout est annulé jusqu'à l'aplat vierge");
    assert_eq!(etat(app).0, trait0);
}

#[test]
fn clicking_the_layer_again_draws_the_line_with_the_same_tools() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    app.select_aplat(0);
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    let (trait0, peinture0, _) = etat(app);
    // Un clic sur le calque : le trait de nouveau.
    app.click_layer_head(0, false, false);
    assert!(!app.aplat_en_cible());
    gliss(app, pos2(200.0, 200.0), pos2(260.0, 200.0));
    let (trait1, peinture1, _) = etat(app);
    assert_ne!(trait1, trait0, "le pinceau dessine le trait");
    assert_eq!(peinture1, peinture0, "et laisse la peinture de l'aplat");
    // La gomme efface le trait, pas l'aplat.
    app.set_tool(Tool::Eraser);
    gliss(app, pos2(55.0, 110.0), pos2(65.0, 110.0));
    let (trait2, _, coupe2) = etat(app);
    assert_ne!(trait2, trait1, "la gomme efface du trait");
    assert!(coupe2.iter().all(|c| c.a() == 0), "elle ne coupe pas l'aplat");
}

#[test]
fn a_character_goes_back_to_the_line_and_still_makes_its_aplat() {
    use crate::personnagesui::Choix;
    const ROUGE: [u8; 3] = [200, 30, 30];
    const ROSE: [u8; 3] = [250, 200, 200];
    let mut b = banc_anneau();
    let app = &mut b.app;
    let lou = app.perso_ajouter().expect("personnage");
    app.perso_ecrire(Choix::Personnage(lou), "Lou".into(), ROUGE, Some(ROSE));
    // Choisir un personnage pendant que l'aplat est la cible : on revient au trait (le personnage dessine les deux).
    app.select_aplat(0);
    assert!(app.aplat_en_cible());
    app.perso_choisir(Choix::Personnage(lou));
    assert!(!app.aplat_en_cible(), "le personnage redonne le trait");
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)));
    // Son trait fait son aplat, dans la même strate qu'avant.
    let (trait0, _, _) = etat(app);
    trait_carre(app, 200.0, 100.0, 300.0, 200.0);
    app.perso_attendre();
    let (trait1, peinture1, _) = etat(app);
    assert_ne!(trait1, trait0, "le trait du personnage");
    let i = 150 * app.doc.width + 250;
    assert_eq!(peinture1[i], eframe::egui::Color32::from_rgb(ROSE[0], ROSE[1], ROSE[2]), "l'aplat du personnage, dans l'aplat du calque");
    // Cliquer ensuite sur la sous-ligne : le pot de peinture prend la couleur d'aplat du personnage.
    app.select_aplat(0);
    assert!(app.aplat_en_cible());
    assert_eq!(app.perso.actif, Some(Choix::Personnage(lou)), "le personnage reste choisi");
    app.set_tool(Tool::Fill);
    app.fill_at(pos2(80.0, 80.0));
    let (_, peinture2, _) = etat(app);
    let j = 80 * app.doc.width + 80;
    assert_eq!(peinture2[j], eframe::egui::Color32::from_rgb(ROSE[0], ROSE[1], ROSE[2]));
}

// ------------------------------------------------------------------------------------ les réglages

#[test]
fn the_aplat_settings_are_in_the_settings_panel_when_the_sub_row_is_selected() {
    let mut app = app_abc("reglages");
    app.toggle_fond(2);
    app.set_tool(Tool::Brush);
    let mut ecran = Ecran::new();
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let sans = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(!contient(&sans, "Aplat de") && !contient(&sans, "Couleur de l'aplat"), "{sans:?}");
    app.select_aplat(2);
    let avec = ecran.chauffer(&mut app, 4, &mut ui_outil);
    for attendu in ["Aplat de C", "Couleur de l'aplat", "Débord", "Fermer les trous", "Cet aplat aussi sur"] {
        assert!(contient(&avec, attendu), "« {attendu} » dans Réglages : {avec:?}");
    }
    // Pour chaque outil qui travaille l'aplat.
    for tool in [Tool::Pencil, Tool::Eraser, Tool::Fill, Tool::Cutter] {
        app.set_tool(tool);
        let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
        assert!(contient(&textes, "Couleur de l'aplat"), "{tool:?} : {textes:?}");
    }
    // Revenu au trait, ils repartent.
    app.click_layer_head(2, false, false);
    let apres = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(!contient(&apres, "Couleur de l'aplat"), "{apres:?}");
}

#[test]
fn changing_the_aplat_settings_is_one_named_step_each() {
    let mut app = app_abc("etape-reglages");
    app.toggle_fond(2);
    app.select_aplat(2);
    app.set_tool(Tool::Brush);
    let avant = app.layers()[2].fond.expect("aplat");
    let mut ecran = Ecran::new();
    // Dans Réglages, la case « Le bord de l'image ferme la forme » : un clic, un réglage, une étape.
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    let n0 = noms(&app).len();
    let (_, case) = exact(&textes, "Le bord de l'image ferme la forme").expect("la case dans Réglages").clone();
    ecran.cliquer(&mut app, case.center(), &mut ui_outil);
    let apres = app.layers()[2].fond.expect("aplat");
    assert_ne!(apres.bord, avant.bord, "le réglage a changé");
    assert_eq!(noms(&app).len(), n0 + 1, "{:?}", noms(&app));
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : réglages"));
    app.undo();
    assert_eq!(app.layers()[2].fond, Some(avant));
    // La même case, dans le clic droit de la sous-ligne : le calque visé est celui de la sous-ligne.
    app.toggle_fond(0);
    let n1 = noms(&app).len();
    let mut menu = |a: &mut App, ui: &mut egui::Ui| a.aplat_menu(ui, 0);
    let textes = ecran.chauffer(&mut app, 4, &mut menu);
    let (_, case) = exact(&textes, "Le bord de l'image ferme la forme").expect("la case dans le menu").clone();
    ecran.cliquer(&mut app, case.center(), &mut menu);
    assert_eq!(noms(&app).len(), n1 + 1);
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : réglages"));
    assert_eq!(app.layers()[2].fond, Some(avant), "l'autre aplat n'a pas bougé");
}

#[test]
fn the_sub_row_menu_has_the_settings_and_removes_the_aplat() {
    let mut app = app_abc("menu-sous-ligne");
    app.toggle_fond(1);
    let mut ecran = Ecran::new();
    let mut menu = |a: &mut App, ui: &mut egui::Ui| a.aplat_menu(ui, 1);
    let textes = ecran.chauffer(&mut app, 4, &mut menu);
    for attendu in ["Aplat de « B »", "Couleur de l'aplat", "Débord", "Fermer les trous", "Retirer l'aplat"] {
        assert!(contient(&textes, attendu), "« {attendu} » : {textes:?}");
    }
    let (_, retirer) = exact(&textes, "Retirer l'aplat").unwrap().clone();
    ecran.cliquer(&mut app, retirer.center(), &mut menu);
    assert!(app.layers()[1].fond.is_none());
    assert_eq!(noms(&app).last().map(String::as_str), Some("Aplat : retirer"));
}

// ------------------------------------------------------------------------------------ le reste

#[test]
fn old_projects_are_read_unchanged_and_the_target_is_not_saved() {
    let mut app = app_abc("fichier");
    app.toggle_fond(0);
    app.select_aplat(0);
    let dir = std::env::temp_dir().join(format!("shot-aplat-fichier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("A.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap_or_default();
    let back = crate::project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.doc.panels[0].layers[0].fond, app.layers()[0].fond, "l'aplat est le même champ du calque qu'avant");
    assert!(back.doc.panels[0].layers[1].fond.is_none());
    assert!(!texte.contains("aplat_cible") && !texte.contains("Aplat"), "le format ne change pas : {texte}");
    // La cible, elle, n'est pas dans le projet : un projet rouvert dessine le trait.
    let autre = App::sans_fenetre(&egui::Context::default(), None);
    assert!(autre.aplat_cible.is_none());
}

#[test]
fn the_target_belongs_to_the_project_tab() {
    assert!(crate::onglets::CHAMPS_DE_PROJET.contains(&"aplat_cible"));
}

#[test]
fn the_app_still_rests_with_the_aplat_selected() {
    let mut b = banc_anneau();
    b.app.select_aplat(0);
    b.chauffer(4);
    gliss(&mut b.app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    // (L'aplat se recalcule sur un fil de fond, qui réveille l'écran une fois à sa fin : on l'attend d'abord, comme
    // l'application déjà ouverte depuis un instant.)
    let debut = std::time::Instant::now();
    while crate::fx::fond_pending() && debut.elapsed() < std::time::Duration::from_secs(30) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(se_reposer(&mut b, 3000) < 3000, "SHOT se repose avec la sous-ligne choisie");
    let mesures: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5)).collect();
    let pire = mesures.iter().map(|m| m.reveil).min().unwrap_or(std::time::Duration::MAX);
    assert!(pire >= std::time::Duration::from_secs(1), "au repos, SHOT demande à être repeint dans {pire:?}");
}
