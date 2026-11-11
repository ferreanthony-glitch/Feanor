//! Les notes d'Anthony du 2026-10-20 sur les calques, les effets, le montage, l'export et la caméra :
//! menus « Trait » et « Matière » des effets, dossiers dans le panneau Calques, fond sous le trait et effets
//! en boutons du calque, durées propres à chaque version d'un plan, GIF animé, cadre fin et passe-partout.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::App;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};

// ------------------------------------------------------------------------------------ l'outillage

/// Une application sans fenêtre, dans un bac à sable, avec ce document.
fn app_with_doc(name: &str, doc: Document) -> App {
    let dir = std::env::temp_dir().join(format!("shot-notes-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
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

/// Un plan de 24 images avec un seul calque de la sorte `kind`.
fn app_one_layer(name: &str, kind: LayerKind) -> App {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(kind);
    let mut l = Layer::new("Dessin", kind);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app_with_doc(name, doc)
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

    /// Une image d'écran : `contenu` peint le morceau d'interface ; rend les textes peints.
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

    /// Un clic gauche en `p` (le pointeur s'y pose, appuie, relâche), puis quelques images pour que l'interface réagisse.
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

/// Le premier texte qui commence par `debut`.
fn trouver<'a>(textes: &'a [(String, Rect)], debut: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.trim_start().starts_with(debut))
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

// ============================================================================================= 1. effets

#[test]
fn the_effects_panel_has_two_dropdowns_not_two_icon_rows() {
    let mut app = app_one_layer("menus", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    // Deux boutons déroulants, « Trait » et « Matière », et plus aucune icône d'effet dans le panneau.
    assert!(trouver(&textes, "Trait").is_some(), "{textes:?}");
    assert!(trouver(&textes, "Matière").is_some());
    for kind in crate::fxtrait::KINDS {
        assert!(!contient(&textes, crate::fxtrait::icon(kind)), "{kind:?} : plus d'icône dans une rangée");
    }
    for kind in crate::fxmatiere::KINDS {
        assert!(!contient(&textes, crate::fxmatiere::icon(kind)), "{kind:?} : plus d'icône dans une rangée");
    }
    // Ouvert, le menu « Trait » liste tous les effets du trait (icône et nom) et ses préréglages en section.
    let (_, r) = trouver(&textes, "Trait").unwrap();
    let textes = ecran.cliquer(&mut app, r.center(), &mut ui_fx);
    for kind in crate::fxtrait::KINDS {
        assert!(contient(&textes, &format!("{}  {}", crate::fxtrait::icon(kind), kind.label())), "{} absent du menu Trait : {textes:?}", kind.label());
    }
    assert!(contient(&textes, "Préréglages"));
    for preset in crate::fxtrait::Preset::ALL {
        assert!(contient(&textes, preset.label()), "{}", preset.label());
    }
}

#[test]
fn a_dropdown_entry_toggles_the_effect_in_one_undo_step_and_the_title_counts_them() {
    let mut app = app_one_layer("entree", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    let (_, r) = trouver(&textes, "Trait").unwrap();
    let textes = ecran.cliquer(&mut app, r.center(), &mut ui_fx);
    let n0 = app.history.names().0.len();
    let nom = format!("{}  {}", crate::fxtrait::icon(EffectKind::Dust), EffectKind::Dust.label());
    let (_, entree) = trouver(&textes, &nom).expect("l'entrée Désagrégation").clone();
    let textes = ecran.cliquer(&mut app, entree.center(), &mut ui_fx);
    assert!(app.fx_on(0, EffectKind::Dust), "{}", app.status);
    assert_eq!(app.history.names().0.len(), n0 + 1, "une étape d'annulation");
    // Le menu se referme ; le bouton compte les effets du trait allumés.
    assert!(trouver(&textes, "Trait (1)").is_some(), "{textes:?}");
    app.undo();
    assert!(!app.fx_on(0, EffectKind::Dust) && app.layers()[0].fx.is_empty());
    assert_eq!(app.fx_count_on(0, &crate::fxtrait::KINDS), 0);
}

#[test]
fn on_a_bitmap_layer_the_vector_only_entries_are_hidden_not_greyed() {
    // Passe de simplicité du 2026-10-21 : un effet que le calque ne prend pas ne se montre pas (le menu d'un calque,
    // lui, garde la raison en grisé : `fxui::offer`).
    let mut app = app_one_layer("bitmap", LayerKind::Bitmap);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    assert!(textes.iter().all(|(t, _)| !t.trim_start().starts_with("Trait")), "plus de menu Trait : {textes:?}");
    assert!(matches!(app.fx_offer(0, EffectKind::Write), crate::fxui::Offer::Disabled(why) if why.contains("vectoriel")));
    // Le menu « Matière » ne liste que ce qui va sur un bitmap.
    let (_, r) = trouver(&textes, "Matière").unwrap();
    let textes = ecran.cliquer(&mut app, r.center(), &mut ui_fx);
    for kind in crate::fxmatiere::KINDS {
        let listed = contient(&textes, &format!("{}  {}", crate::fxmatiere::icon(kind), kind.label()));
        assert_eq!(listed, crate::fxui::offer(kind, LayerKind::Bitmap, false) == crate::fxui::Offer::Enabled, "{} : {textes:?}", kind.label());
    }
    app.toggle_fx(0, EffectKind::Write);
    assert!(app.layers()[0].fx.is_empty(), "un effet du trait ne se pose pas sur un bitmap");
}

#[test]
fn the_matter_menu_lists_its_effects_and_presets_and_applies_a_preset() {
    let mut app = app_one_layer("matiere", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_fx = |a: &mut App, ui: &mut egui::Ui| a.fx_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_fx);
    let (_, r) = trouver(&textes, "Matière").unwrap();
    let textes = ecran.cliquer(&mut app, r.center(), &mut ui_fx);
    for kind in crate::fxmatiere::KINDS {
        assert!(contient(&textes, &format!("{}  {}", crate::fxmatiere::icon(kind), kind.label())), "{} absent du menu Matière : {textes:?}", kind.label());
    }
    for preset in crate::fxmatiere::Preset::ALL {
        assert!(contient(&textes, preset.label()), "{}", preset.label());
    }
    let preset = crate::fxmatiere::Preset::ALL[0];
    let (_, entree) = trouver(&textes, preset.label()).unwrap().clone();
    let n0 = app.history.names().0.len();
    ecran.cliquer(&mut app, entree.center(), &mut ui_fx);
    assert!(!app.layers()[0].fx.is_empty(), "le préréglage « {} » est posé : {}", preset.label(), app.status);
    assert_eq!(app.history.names().0.len(), n0 + 1);
}

// ============================================================================================= 2. dossiers

/// Un plan : du bas vers le haut « C1 », « C2 » (rangés dans « Décors »), « Décors », « Hors dossier ».
fn app_with_folder(name: &str) -> App {
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
    let mut out = Layer::new("Hors dossier", LayerKind::Vector);
    out.frames = vec![Some(cel); 6];
    layers.push(out);
    crate::folders::normalize(&mut layers);
    doc.panels[0].layers = layers;
    let mut app = app_with_doc(name, doc);
    app.cur_layer = 3;
    app
}

#[test]
fn the_layers_panel_shows_the_folder_its_layers_indented_and_a_new_folder_button() {
    let mut app = app_with_folder("panneau");
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    for nom in ["Décors", "C1", "C2", "Hors dossier"] {
        assert!(trouver(&textes, nom).is_some(), "{nom} : {textes:?}");
    }
    // La ligne du dossier porte l'icône de sa sorte et le nombre de ses calques (plus le mot « dossier »).
    assert!(contient(&textes, egui_phosphor::regular::FOLDER) && textes.iter().any(|(t, _)| t.trim() == "2"), "{textes:?}");
    assert!(!contient(&textes, "dossier ·"), "{textes:?}");
    // Le nouveau dossier se crée par le « + » du panneau (plus de bouton à part).
    assert!(!contient(&textes, egui_phosphor::regular::FOLDER_PLUS));
    let (_, plus) = trouver(&textes, egui_phosphor::regular::PLUS).unwrap().clone();
    let menu = ecran.cliquer(&mut app, plus.center(), &mut ui_calques);
    assert!(contient(&menu, egui_phosphor::regular::FOLDER_PLUS) && contient(&menu, "Pour ranger des calques ensemble"), "{menu:?}");
    let textes = ecran.cliquer(&mut app, egui::pos2(300.0, 600.0), &mut ui_calques);
    let _ = textes;
    // Un calque rangé dans le dossier est décalé vers la droite.
    let x = |nom: &str| trouver(&textes, nom).unwrap().1.left();
    assert!(x("C1") > x("Hors dossier") + 10.0, "C1 {} contre {}", x("C1"), x("Hors dossier"));
    assert!((x("C1") - x("C2")).abs() < 0.5);
}

#[test]
fn a_folder_closes_and_opens_from_its_triangle_in_the_layers_panel() {
    let mut app = app_with_folder("plier");
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    let (_, ligne) = trouver(&textes, "Décors").unwrap().clone();
    let (_, caret) = textes
        .iter()
        .find(|(t, r)| t.contains(egui_phosphor::regular::CARET_DOWN) && (r.center().y - ligne.center().y).abs() < 30.0 && r.right() < ligne.left())
        .expect("le triangle du dossier")
        .clone();
    let textes = ecran.cliquer(&mut app, caret.center(), &mut ui_calques);
    assert!(app.layers()[2].collapsed, "le dossier est fermé : {textes:?}");
    assert!(trouver(&textes, "C1").is_none() && trouver(&textes, "C2").is_none(), "ses calques n'ont plus de rang : {textes:?}");
    assert!(trouver(&textes, "Hors dossier").is_some() && trouver(&textes, "Décors").is_some());
    // Rouvert, ses calques reviennent.
    let (_, caret) = textes.iter().find(|(t, _)| t.contains(egui_phosphor::regular::CARET_RIGHT)).expect("le triangle fermé").clone();
    let textes = ecran.cliquer(&mut app, caret.center(), &mut ui_calques);
    assert!(!app.layers()[2].collapsed);
    assert!(trouver(&textes, "C1").is_some() && trouver(&textes, "C2").is_some());
}

#[test]
fn the_current_layer_stays_listed_even_in_a_closed_folder() {
    let mut app = app_with_folder("courant");
    app.layers_mut()[2].collapsed = true;
    app.cur_layer = 0;
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(trouver(&textes, "C1").is_some(), "le calque courant ne se perd pas de vue : {textes:?}");
    assert!(trouver(&textes, "C2").is_none());
}

#[test]
fn the_new_folder_button_creates_one_folder_in_one_undo_step() {
    let mut app = app_one_layer("nouveau-dossier", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    // Le « + » du panneau ouvre le menu des sortes de calques ; l'entrée « Dossier » crée le dossier.
    let (_, plus) = textes.iter().find(|(t, _)| t.contains(egui_phosphor::regular::PLUS)).expect("le bouton +").clone();
    let menu = ecran.cliquer(&mut app, plus.center(), &mut ui_calques);
    let (_, bouton) = menu.iter().find(|(t, _)| t.contains("Pour ranger des calques ensemble")).expect("l'entrée Dossier").clone();
    let n0 = app.history.names().0.len();
    ecran.cliquer(&mut app, bouton.center(), &mut ui_calques);
    assert_eq!(app.layers().iter().filter(|l| l.folder).count(), 1, "{}", app.status);
    assert_eq!(app.history.names().0.len(), n0 + 1, "une étape d'annulation");
    app.undo();
    assert!(app.layers().iter().all(|l| !l.folder));
}

#[test]
fn dragging_a_layer_in_and_out_of_a_folder_and_moving_the_folder_carries_its_layers() {
    let mut app = app_with_folder("deplacer");
    let folder_id = app.layers()[2].id;
    // « Hors dossier » (rang 3) se glisse entre C1 et C2 (juste au-dessus de C1) : il entre dans le dossier.
    let sur_c1 = crate::folders::Pose::AuDessusDe(app.layers()[0].id);
    app.poser_les_calques(3, sur_c1);
    let l = app.layers();
    assert_eq!(l.iter().find(|x| x.name == "Hors dossier").and_then(|x| x.parent), Some(folder_id), "{:?}", l.iter().map(|x| (&x.name, x.parent.is_some())).collect::<Vec<_>>());
    // Il en ressort quand on le pose au-dessus du dossier.
    let top = app.layers().len() - 1;
    let rank = app.layers().iter().position(|x| x.name == "Hors dossier").unwrap();
    let sur_le_dossier = crate::folders::Pose::AuDessusDe(app.layers()[top].id);
    app.poser_les_calques(rank, sur_le_dossier);
    assert_eq!(app.layers().iter().find(|x| x.name == "Hors dossier").unwrap().parent, None);
    // Choisir le dossier et le monter emmène ses calques avec lui (ils restent d'un seul tenant sous lui).
    let fi = app.layers().iter().position(|x| x.folder).unwrap();
    assert_eq!(fi, 2);
    app.cur_layer = fi;
    app.move_layer(true);
    let layers = app.layers();
    let fi = layers.iter().position(|x| x.folder).unwrap();
    assert_eq!(fi, 3, "le dossier est passé au-dessus de « Hors dossier »");
    assert_eq!(layers[0].name, "Hors dossier");
    assert_eq!(crate::folders::children_of(layers, fi), vec![1, 2], "ses calques le suivent, juste sous lui");
}

#[test]
fn a_folder_hides_locks_and_dims_its_layers() {
    let mut app = app_with_folder("effets-dossier");
    let layers = app.layers().to_vec();
    assert_eq!(crate::folders::effect(&layers, 0), Some(1.0));
    app.layers_mut()[2].opacity = 0.5;
    assert_eq!(crate::folders::effect(app.layers(), 0), Some(0.5), "l'opacité du dossier multiplie celle de ses calques");
    app.layers_mut()[2].visible = false;
    assert_eq!(crate::folders::effect(app.layers(), 0), None, "un dossier caché cache ses calques");
    app.layers_mut()[2].locked = true;
    assert!(crate::folders::locked_by_folder(app.layers(), 1), "un dossier verrouillé verrouille ses calques");
}

// ============================================================================================= 3. fond

#[test]
fn the_background_is_a_button_of_the_layer_and_each_layer_has_its_own() {
    let mut app = app_with_folder("fond");
    assert!(app.layers().iter().all(|l| l.fond.is_none()));
    // Sur un autre calque que le courant : le courant ne change pas.
    let n0 = app.history.names().0.len();
    app.toggle_fond(1);
    assert!(app.layers()[1].fond.is_some() && app.layers()[0].fond.is_none() && app.layers()[3].fond.is_none());
    assert_eq!(app.cur_layer, 3, "le calque courant n'a pas changé");
    assert_eq!(app.history.names().0.len(), n0 + 1, "une étape d'annulation");
    app.toggle_fond(1);
    assert!(app.layers()[1].fond.is_none());
    app.undo();
    assert!(app.layers()[1].fond.is_some(), "Ctrl+Z remet le fond");
    // Un dossier n'en prend pas.
    let n = app.history.names().0.len();
    app.toggle_fond(2);
    assert!(app.layers()[2].fond.is_none());
    assert_eq!(app.history.names().0.len(), n, "rien n'est enregistré");
    assert!(app.status.contains("n'en prend pas"), "{}", app.status);
    // L'action du pinceau (raccourci d'avant) vise le calque courant.
    app.run_action("pinceau.fond");
    assert!(app.layers()[3].fond.is_some());
}

#[test]
fn the_layer_row_and_the_brush_settings_carry_the_aplat_button() {
    let mut app = app_with_folder("fond-bouton");
    let ph_fond = egui_phosphor::regular::USER_SQUARE;
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    // Un bouton sur chaque ligne de calque de dessin (pas sur le dossier) : 3 calques ; plus de bouton dans la barre
    // de commandes (l'aplat du calque courant est dans le menu « … », `panneaux_tests.rs`).
    let n = textes.iter().filter(|(t, _)| t.contains(ph_fond)).count();
    assert_eq!(n, 3, "{textes:?}");
    // Un clic sur celui de la ligne « C1 » lui donne un fond.
    let (_, ligne) = trouver(&textes, "C1").unwrap().clone();
    let (_, bouton) = textes
        .iter()
        .filter(|(t, r)| t.contains(ph_fond) && r.center().y > ligne.center().y && r.center().y - ligne.center().y < 30.0 && r.left() > ligne.left() - 5.0)
        .min_by(|a, b| a.1.top().total_cmp(&b.1.top()))
        .expect("le bouton de la ligne C1")
        .clone();
    let textes = ecran.cliquer(&mut app, bouton.center(), &mut ui_calques);
    assert!(app.layers()[0].fond.is_some(), "{} / {textes:?}", app.status);
    // Les réglages du pinceau ont « Peint sur l'aplat » et, à côté, le même bouton d'aplat que la ligne du calque
    // (Anthony, 2026-10-23), au-dessus de « Plus de réglages », replié.
    app.set_tool(crate::app::Tool::Brush);
    app.cur_layer = 3;
    let mut ui_outil = |a: &mut App, ui: &mut egui::Ui| a.tool_options_panel(ui);
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(contient(&textes, ph_fond), "le bouton d'aplat du calque courant est dans les réglages du pinceau : {textes:?}");
    assert!(contient(&textes, egui_phosphor::regular::SELECTION_BACKGROUND), "« Peindre derrière » y est");
    assert!(contient(&textes, egui_phosphor::regular::SQUARE_HALF_BOTTOM), "« Peint sur l'aplat » y est");
    assert!(!contient(&textes, egui_phosphor::regular::PAINT_BUCKET), "plus de « Remplir la forme »");
    assert!(!contient(&textes, "Peindre dans"), "la bascule Trait / Fond a disparu");
    // Un calque qui en a un ne montre rien non plus sous le pinceau : ses réglages sont dans Réglages quand sa
    // sous-ligne est choisie (`aplat_tests.rs`).
    app.cur_layer = 0;
    let textes = ecran.chauffer(&mut app, 4, &mut ui_outil);
    assert!(!contient(&textes, "Peindre dans") && !contient(&textes, "Couleur de l'aplat"), "{textes:?}");
}

#[test]
fn the_background_of_a_layer_is_saved_with_the_layer_as_before() {
    // Le fond a toujours été un champ du calque (`fond`) : l'ancien bouton du pinceau n'a jamais rien écrit dans
    // les réglages de pinceau. Un projet d'avant se lit donc tel quel, un projet d'après se relit à l'identique.
    let mut app = app_with_folder("fond-fichier");
    app.toggle_fond(0);
    let dir = std::env::temp_dir().join(format!("shot-notes-fond-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("F.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let back = crate::project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.doc.panels[0].layers[0].fond, app.layers()[0].fond);
    assert!(back.doc.panels[0].layers[1].fond.is_none());
    // Et les réglages de pinceau n'ont aucun champ de fond.
    let reglages = toml::to_string(&app.brush).unwrap();
    assert!(!reglages.contains("fond_sous"), "{reglages}");
}

// ============================================================================================= 4. bouton fx

fn with_effects(name: &str) -> App {
    let mut app = app_one_layer(name, LayerKind::Vector);
    app.layers_mut()[0].fx.push(Effect::new(EffectKind::GaussianBlur));
    app
}

#[test]
fn the_fx_button_switches_the_whole_stack_off_and_on_in_one_undo_step() {
    let mut app = with_effects("fx");
    let steps = |app: &App, f: usize| crate::fx::layer_steps(&app.layers()[0], f, None).len();
    assert!(app.layers()[0].fx_enabled && steps(&app, 3) > 0, "la pile joue");
    let n0 = app.history.names().0.len();
    app.toggle_layer_fx(0);
    assert!(!app.layers()[0].fx_enabled);
    assert_eq!(steps(&app, 3), 0, "éteinte, aucun effet ne s'applique");
    assert_eq!(app.layers()[0].fx.len(), 1, "mais la pile est gardée (non destructif)");
    assert_eq!(app.history.names().0.len(), n0 + 1, "une étape d'annulation");
    app.undo();
    assert!(app.layers()[0].fx_enabled && steps(&app, 3) > 0, "Ctrl+Z rallume");
    app.redo();
    assert!(!app.layers()[0].fx_enabled);
    // L'action (raccourci à donner) vise le calque courant.
    app.run_action("calque.effets");
    assert!(app.layers()[0].fx_enabled);
}

#[test]
fn switched_off_effects_leave_the_plan_untouched_for_the_screen_and_the_export() {
    let mut app = with_effects("fx-plan");
    let fx_count = |app: &App| app.build_plan(3, false, true).items.iter().map(|i| i.fx.len()).sum::<usize>();
    let on = fx_count(&app);
    assert!(on > 0, "le flou est dans le plan");
    app.toggle_layer_fx(0);
    assert_eq!(fx_count(&app), 0);
    // Le rendu de l'export (même chemin que l'image filmée) ne les voit pas non plus.
    let plan = crate::render::panel_plan_at(&app.doc, 0, 3, None);
    assert!(plan.items.iter().all(|i| i.fx.is_empty()));
    app.toggle_layer_fx(0);
    assert_eq!(fx_count(&app), on);
}

#[test]
fn the_fx_button_only_shows_on_a_layer_with_effects_or_with_effects_switched_off() {
    let mut app = app_one_layer("fx-visible", LayerKind::Vector);
    assert!(!app.fx_switch_shown(0), "pas d'effets, pas de bouton");
    app.layers_mut()[0].fx.push(Effect::new(EffectKind::GaussianBlur));
    assert!(app.fx_switch_shown(0));
    app.layers_mut()[0].fx_enabled = false;
    app.layers_mut()[0].fx.clear();
    assert!(app.fx_switch_shown(0), "éteint, il reste pour pouvoir rallumer");
    // Un dossier n'a pas d'effets.
    let mut app = app_with_folder("fx-dossier");
    assert!(!app.fx_switch_shown(2));
    let n = app.history.names().0.len();
    app.toggle_layer_fx(2);
    assert_eq!(app.history.names().0.len(), n);
}

#[test]
fn the_fx_button_is_painted_in_the_layers_panel_and_the_timeline_header() {
    let mut app = app_one_layer("fx-peint", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let mut ui_calques = |a: &mut App, ui: &mut egui::Ui| a.layers_panel(ui);
    let mut ui_tete = |a: &mut App, ui: &mut egui::Ui| {
        a.timeline_heads(ui, 24.0, 0.0);
    };
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(trouver(&textes, "fx").is_none(), "sans effets : pas de bouton fx");
    let textes = ecran.chauffer(&mut app, 4, &mut ui_tete);
    assert!(trouver(&textes, "fx").is_none());
    app.layers_mut()[0].fx.push(Effect::new(EffectKind::GaussianBlur));
    let textes = ecran.chauffer(&mut app, 4, &mut ui_calques);
    assert!(trouver(&textes, "fx").is_some(), "{textes:?}");
    let textes = ecran.chauffer(&mut app, 4, &mut ui_tete);
    let (_, bouton) = trouver(&textes, "fx").expect("le bouton fx de la ligne de la timeline").clone();
    let n0 = app.history.names().0.len();
    ecran.cliquer(&mut app, bouton.center(), &mut ui_tete);
    assert!(!app.layers()[0].fx_enabled, "un clic éteint les effets : {}", app.status);
    assert_eq!(app.history.names().0.len(), n0 + 1);
}

#[test]
fn the_fx_switch_is_saved_in_the_project_and_old_projects_open_with_effects_on() {
    let mut app = with_effects("fx-fichier");
    let dir = std::env::temp_dir().join(format!("shot-notes-fx-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("E.pelure");
    // Allumé : le manifeste n'écrit rien de nouveau (un projet sans bouton éteint garde son contenu d'avant).
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(!text.contains("effets_eteints"), "rien de plus tant qu'on n'éteint pas");
    app.toggle_layer_fx(0);
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"effets_eteints\": true") && text.contains(&format!("\"version\": {}", crate::project::VERSION)));
    let back = crate::project::load(&manifest).unwrap();
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);
    assert!(!back.doc.panels[0].layers[0].fx_enabled && back.doc.panels[0].layers[0].fx.len() == 1, "éteint et gardé");
    // Un projet d'avant (numéro 32, aucun champ) s'ouvre effets allumés.
    app.toggle_layer_fx(0);
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let old = std::fs::read_to_string(&manifest).unwrap().replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 32");
    std::fs::write(&manifest, old).unwrap();
    let back = crate::project::load(&manifest).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(back.doc.panels[0].layers[0].fx_enabled);
    assert!(crate::project::VERSION >= 33, "33 : l'interrupteur fx du calque");
}

#[test]
fn copying_or_duplicating_a_layer_keeps_its_fx_switch() {
    let mut app = with_effects("fx-copie");
    app.toggle_layer_fx(0);
    app.duplicate_layer();
    assert!(app.layers().iter().filter(|l| !l.fx_enabled).count() == 2, "la copie garde l'interrupteur éteint");
}

// ============================================================================================= 5. durées par version

/// Un film de trois plans de 48 images (un calque bitmap, un dessin par plan).
fn film_trois_plans(name: &str) -> App {
    let mut doc = Document::with_frame(64, 48, 24);
    for k in 1..3 {
        doc.add_scene_after(k - 1);
    }
    for i in 0..3 {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let id = doc.add_cel(Cel::new(doc.width, doc.height, LayerKind::Bitmap));
        layer.frames = vec![Some(id); 48];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 48;
    }
    app_with_doc(name, doc)
}

#[test]
fn each_version_keeps_its_own_duration_and_the_following_plans_ripple() {
    let mut app = film_trois_plans("durees");
    let plan = app.doc.panels[1].id;
    app.nouvelle_version_du_plan(1);
    assert_eq!(app.doc.numero_de_version(plan), 2);
    // V2 est plus longue que V1 : 72 images au lieu de 48.
    assert_eq!(app.doc.set_duration(1, 72), 72);
    assert_eq!(app.doc.panel_start(2), 48 + 72, "le plan suivant recule");
    assert_eq!(app.doc.total_frames(), 48 + 72 + 48);
    // Retour à V1 : 48 images, les plans suivants avancent, en une étape d'annulation.
    let n0 = app.history.names().0.len();
    app.passer_a_la_version(1, 1);
    assert_eq!(app.doc.numero_de_version(plan), 1);
    assert_eq!(app.doc.panels[1].frame_count(), 48);
    assert_eq!(app.doc.panel_start(2), 96, "le plan suivant avance");
    assert_eq!(app.doc.total_frames(), 144);
    let (undo, _) = app.history.names();
    assert_eq!(undo.len(), n0 + 1, "{undo:?}");
    assert_eq!(undo.last().map(|s| s.to_string()).as_deref(), Some("Passer à la version V1"));
    assert!(app.status.contains("Durée du plan") && app.status.contains("avancent de 24 i"), "{}", app.status);
    // Ctrl+Z rend la durée de V2 d'un coup ; Ctrl+Maj+Z, celle de V1.
    app.undo();
    assert_eq!((app.doc.numero_de_version(plan), app.doc.panels[1].frame_count(), app.doc.panel_start(2)), (2, 72, 120));
    app.redo();
    assert_eq!((app.doc.numero_de_version(plan), app.doc.panels[1].frame_count(), app.doc.panel_start(2)), (1, 48, 96));
}

#[test]
fn the_cards_of_the_stack_show_each_versions_duration() {
    let mut app = film_trois_plans("cartes");
    let plan = app.doc.panels[1].id;
    app.nouvelle_version_du_plan(1);
    app.doc.set_duration(1, 80);
    app.nouvelle_version_du_plan(1);
    app.doc.set_duration(1, 24);
    let cartes = crate::pilevers::cartes(&app.doc, plan);
    assert_eq!(cartes.iter().map(|c| (c.numero, c.duree, c.actuelle)).collect::<Vec<_>>(), vec![(1, 48, false), (2, 80, false), (3, 24, true)]);
    // La phrase de l'info-bulle et de la barre d'état.
    assert_eq!(crate::planversui::phrase_duree(48, 48, 24), "Même durée (2s 00).");
    assert_eq!(crate::planversui::phrase_duree(48, 72, 24), "Durée du plan : 2s 00 → 3s 00 ; les plans suivants reculent de 24 i, le son reste en place.");
    assert!(crate::planversui::phrase_duree(80, 24, 24).contains("avancent de 56 i"));
}

#[test]
fn the_playhead_stays_inside_a_shorter_version() {
    let mut app = film_trois_plans("tete");
    app.nouvelle_version_du_plan(1);
    app.doc.set_duration(1, 80);
    app.cur_panel = 1;
    app.cur_frame = 70;
    app.passer_a_la_version(1, 1);
    assert_eq!(app.doc.panels[1].frame_count(), 48);
    assert!(app.cur_frame < 48, "la tête de lecture est restée dans le plan : {}", app.cur_frame);
}

#[test]
fn the_editorial_export_uses_the_active_versions_duration() {
    let mut app = film_trois_plans("montage-export");
    app.nouvelle_version_du_plan(1);
    app.doc.set_duration(1, 72);
    let o = crate::editorial::Options { whole_film: true, ..crate::editorial::Options::default() };
    let v2 = crate::editorial::build_plan(&app.doc, &o, 0, "Film").unwrap();
    assert_eq!((v2.clips[1].len, v2.clips[1].start, v2.clips[2].start), (72, 48, 120));
    assert!(v2.clips[1].name.ends_with("_V2"), "{}", v2.clips[1].name);
    app.passer_a_la_version(1, 1);
    let v1 = crate::editorial::build_plan(&app.doc, &o, 0, "Film").unwrap();
    assert_eq!((v1.clips[1].len, v1.clips[1].start, v1.clips[2].start), (48, 48, 96));
    assert!(v1.clips[1].name.ends_with("_V1"), "{}", v1.clips[1].name);
}

#[test]
fn the_digest_of_a_plan_follows_the_version_and_its_duration() {
    let mut app = film_trois_plans("empreinte");
    // (L'empreinte d'un envoi compte le contenu des dessins, pas leurs numéros : `envois.rs`.)
    let digest = |app: &App| {
        let cle = |id: u64| app.doc.cels.get(&id).map_or(id, |c| crate::empreinte::Donnees::de(c).empreinte());
        crate::empreinte::de_octets(&crate::project::digest_bytes(&app.doc.panels[1], &cle))
    };
    let v1 = digest(&app);
    app.nouvelle_version_du_plan(1);
    assert_eq!(digest(&app), v1, "une copie identique ne change pas l'empreinte");
    app.doc.set_duration(1, 72);
    let v2 = digest(&app);
    assert_ne!(v1, v2, "une autre durée : le plan est modifié depuis l'envoi");
    app.passer_a_la_version(1, 1);
    assert_eq!(digest(&app), v1, "de retour à V1, l'empreinte est celle d'avant");
}

#[test]
fn comparing_versions_of_different_durations_stays_inside_each_side() {
    let mut app = film_trois_plans("comparer");
    app.nouvelle_version_du_plan(1);
    app.doc.set_duration(1, 80);
    let plan = app.doc.panels[1].id;
    let courte = app.doc.versions_du_plan(plan).unwrap().rangee(1).unwrap().panel.clone();
    let longue = app.doc.panels[1].clone();
    let (a, b) = (crate::comparer::petit_projet(&app.doc, &courte), crate::comparer::petit_projet(&app.doc, &longue));
    assert_eq!((a.panels[0].frame_count(), b.panels[0].frame_count()), (48, 80));
    // Une image au-delà de la fin du côté court ne fait pas d'erreur : on y montre sa dernière image.
    let court = crate::comparer::rendre(&a, 70, 64);
    let derniere = crate::comparer::rendre(&a, 47, 64);
    assert_eq!(court.pixels, derniere.pixels);
    let _ = crate::comparer::rendre(&b, 70, 64);
}

// ============================================================================================= 6. GIF

#[test]
fn gif_sizes_keep_the_proportions_and_never_enlarge() {
    use crate::export::GifSize::{Percent, Width};
    assert_eq!(Width(480).dimensions([1920, 1080]), [480, 270]);
    assert_eq!(Width(720).dimensions([1920, 1080]), [720, 405]);
    assert_eq!(Percent(50).dimensions([1920, 1080]), [960, 540]);
    assert_eq!(Percent(100).dimensions([1920, 1080]), [1920, 1080]);
    assert_eq!(Width(720).dimensions([640, 360]), [640, 360], "on n'agrandit pas l'image finale");
    assert_eq!(Width(480).dimensions([4, 3]), [4, 3]);
    assert!(Width(480).label().contains("480") && Percent(100).label() == "Image finale");
}

#[test]
fn the_gif_rate_never_goes_above_the_projects() {
    use crate::export::{gif_fps, gif_fps_choices};
    assert_eq!(gif_fps(60, 24), 24);
    assert_eq!(gif_fps(0, 24), 1);
    assert_eq!(gif_fps(12, 24), 12);
    assert_eq!(gif_fps_choices(24), vec![6, 8, 10, 12, 15, 20, 24]);
    assert_eq!(gif_fps_choices(12), vec![6, 8, 10, 12], "celle du projet, et rien au-dessus");
    assert_eq!(gif_fps_choices(25), vec![6, 8, 10, 12, 15, 20, 24, 25]);
    assert!(gif_fps_choices(10).iter().all(|f| *f <= 10));
}

#[test]
fn a_gif_only_computes_the_frames_it_keeps_and_keeps_the_films_duration() {
    use crate::export::gif_sample;
    let frames: Vec<(usize, usize)> = (0..48).map(|f| (0, f)).collect();
    let tous = gif_sample(&frames, 24, 24);
    assert_eq!(tous, frames, "à la cadence du projet, toutes les images");
    let moitie = gif_sample(&frames, 24, 12);
    assert_eq!(moitie.len(), 24);
    assert_eq!(moitie[..3], [(0, 0), (0, 2), (0, 4)]);
    assert_eq!(moitie.last(), Some(&(0, 46)));
    // Une cadence qui ne divise pas : la durée reste (12 images à 24 par seconde = 0,5 s ; à 10 par seconde, 5 images).
    let douze: Vec<(usize, usize)> = (0..12).map(|f| (1, f)).collect();
    let dix = gif_sample(&douze, 24, 10);
    assert_eq!(dix.len(), 5);
    assert!(dix.iter().all(|(p, f)| *p == 1 && *f < 12));
    assert!(dix.windows(2).all(|w| w[0].1 < w[1].1), "toujours dans l'ordre : {dix:?}");
    // Plus que le projet : ramené au projet. Rien : rien. Une seule image : une image.
    assert_eq!(gif_sample(&frames, 24, 100), frames);
    assert!(gif_sample(&[], 24, 12).is_empty());
    assert_eq!(gif_sample(&[(3, 0)], 24, 6), vec![(3, 0)]);
    // Sur plusieurs plans, l'ordre du film est gardé.
    let deux: Vec<(usize, usize)> = (0..2).flat_map(|p| (0..24).map(move |f| (p, f))).collect();
    let g = gif_sample(&deux, 24, 8);
    assert_eq!(g.len(), 16);
    assert!(g.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn the_gif_command_line_uses_a_palette_the_size_the_rate_the_loop_and_the_dither() {
    use crate::export::{gif_args, GifSettings, GifSize};
    let out = std::path::Path::new("sortie.gif");
    let defaut = GifSettings::default();
    let a = gif_args([1920, 1080], 15, &defaut, out);
    let ligne = a.join(" ");
    assert!(ligne.contains("-framerate 15") && ligne.contains("-s 1920x1080") && ligne.contains("-pix_fmt rgba"), "{ligne}");
    assert!(ligne.contains("scale=480:270:flags=lanczos"), "{ligne}");
    assert!(ligne.contains("palettegen") && ligne.contains("paletteuse=dither=sierra2_4a"), "{ligne}");
    assert!(ligne.contains("-loop 0"), "en boucle sans fin d'office : {ligne}");
    assert_eq!(a.last().map(String::as_str), Some("sortie.gif"));
    // Une seule fois, sans tramage, à l'échelle de l'image finale (pas de redimensionnement).
    let b = gif_args([640, 360], 8, &GifSettings { size: GifSize::Percent(100), fps: 8, looped: false, dither: false }, out).join(" ");
    assert!(b.contains("-loop -1") && b.contains("dither=none") && b.contains("-framerate 8"), "{b}");
    assert!(!b.contains("scale="), "{b}");
}

#[test]
fn the_selection_of_plans_exports_only_those_plans_in_film_order() {
    use crate::export::{frames_for_chosen, Range};
    let app = film_trois_plans("selection");
    let (a, c) = (app.doc.panels[0].id, app.doc.panels[2].id);
    let f = frames_for_chosen(&app.doc, Range::Selection, 1, &[c, a]);
    assert_eq!(f.len(), 96);
    assert_eq!((f[0], f[47], f[48], f[95]), ((0, 0), (0, 47), (2, 0), (2, 47)), "dans l'ordre du film, pas du choix");
    // Rien de choisi : le plan courant.
    let f = frames_for_chosen(&app.doc, Range::Selection, 1, &[]);
    assert_eq!((f.len(), f[0].0), (48, 1));
    assert_eq!(frames_for_chosen(&app.doc, Range::Montage, 1, &[a]).len(), 144, "les autres étendues ne changent pas");
}

#[test]
fn the_gif_is_in_the_export_menus_the_command_palette_and_the_keymap() {
    assert_eq!(crate::export::Format::Gif.extension(), "gif");
    assert!(crate::export::Format::Gif.uses_ffmpeg() && !crate::export::Format::Gif.is_video());
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "fichier.exporter_gif" && a.implemented));
    let mut app = app_one_layer("gif-action", LayerKind::Vector);
    app.run_action("fichier.exporter_gif");
    let s = app.export_dialog.as_ref().expect("la fenêtre d'export s'ouvre");
    assert_eq!(s.format, crate::export::Format::Gif);
    // La fenêtre : le choix du GIF, sa taille, sa cadence, la boucle et le tramage.
    let mut ecran = Ecran::new();
    let mut ui_export = |a: &mut App, ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        a.export_windows(&ctx);
    };
    let textes = ecran.chauffer(&mut app, 4, &mut ui_export);
    for morceau in ["GIF animé", "480 px de large", "720 px de large", "Image finale", "images par seconde", "Tourne en boucle", "Tramage", "Plans choisis"] {
        assert!(contient(&textes, morceau), "« {morceau} » absent de la fenêtre : {textes:?}");
    }
}

#[test]
fn a_gif_is_encoded_in_the_background_with_ffmpeg_when_it_is_installed() {
    use crate::export::{self, ExportJob, GifSettings, GifSize};
    let Some(ffmpeg) = export::find_ffmpeg(None) else {
        eprintln!("ffmpeg absent : test du GIF sauté");
        return;
    };
    let doc = crate::export_tests::doc_with_drawing(2, 12);
    let dir = std::env::temp_dir().join(format!("shot-notes-gif-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("essai.gif");
    let reglages = GifSettings { size: GifSize::Width(100), fps: 6, looped: true, dither: true };
    let frames = export::frames_for(&doc, export::Range::Montage, 0);
    let rate = export::gif_fps(reglages.fps, doc.fps);
    let kept = export::gif_sample(&frames, doc.fps, rate);
    assert_eq!(kept.len(), 12, "24 images à 12 par seconde, gardées à 6 : 12 images");
    let args = export::gif_args(doc.frame, rate, &reglages, &out);
    let mut job = ExportJob::gif(&ffmpeg, args, &out, kept).unwrap();
    assert!(job.title.contains("GIF") && job.title.contains("12"), "{}", job.title);
    let debut = std::time::Instant::now();
    while job.outcome.is_none() && debut.elapsed() < std::time::Duration::from_secs(60) {
        job.step(&doc, std::time::Duration::from_millis(30));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let octets = std::fs::read(&out).unwrap();
    assert!(octets.starts_with(b"GIF89a"), "un vrai GIF animé");
    assert!(octets.len() > 500, "{} octets", octets.len());
    // Largeur et hauteur écrites dans l'en-tête (little endian) : celles demandées.
    let [w, h] = GifSize::Width(100).dimensions(doc.frame);
    assert_eq!((u16::from_le_bytes([octets[6], octets[7]]) as usize, u16::from_le_bytes([octets[8], octets[9]]) as usize), (w, h));
    // Plusieurs images, et la boucle (extension NETSCAPE).
    let images = octets.iter().filter(|b| **b == 0x2C).count();
    assert!(images >= 2, "plusieurs images");
    assert!(octets.windows(11).any(|w| w == b"NETSCAPE2.0"), "boucle sans fin");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_gif_removes_the_unfinished_file() {
    use crate::export::{self, ExportJob, GifSettings};
    let Some(ffmpeg) = export::find_ffmpeg(None) else { return };
    let doc = crate::export_tests::doc_with_drawing(1, 300);
    let dir = std::env::temp_dir().join(format!("shot-notes-gif-annule-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("annule.gif");
    let frames = export::frames_for(&doc, export::Range::Montage, 0);
    let args = export::gif_args(doc.frame, doc.fps, &GifSettings::default(), &out);
    let mut job = ExportJob::gif(&ffmpeg, args, &out, frames).unwrap();
    job.step(&doc, std::time::Duration::from_millis(30));
    job.cancel();
    assert!(matches!(job.outcome, Some(Err(_))));
    assert!(!out.exists(), "le GIF inachevé est effacé");
    std::fs::remove_dir_all(&dir).ok();
}

// ============================================================================================= 7. caméra

#[test]
fn the_passe_partout_opacity_runs_from_nothing_to_black() {
    use crate::camtools::{passe_partout_alpha as alpha, PASSE_PARTOUT_DEFAUT};
    assert_eq!(alpha(0), 0, "0 % : rien n'est couvert");
    assert_eq!(alpha(100), 255, "100 % : opaque");
    assert_eq!(alpha(200), 255, "jamais au-delà");
    assert_eq!(alpha(PASSE_PARTOUT_DEFAUT), 255, "d'origine : opaque");
    let mut avant = 0;
    for p in 0..=100u8 {
        let a = alpha(p);
        assert!(a >= avant, "croissante");
        avant = a;
    }
}

#[test]
fn the_passe_partout_setting_is_saved_and_the_old_veil_setting_is_ignored() {
    let vide: crate::settings::Settings = toml::from_str("").unwrap();
    assert_eq!(vide.passe_partout, 100, "des réglages d'avant : opaque");
    assert_eq!(vide.passe_partout_couleur, [0, 0, 0], "et noir");
    // L'ancien voile (27 % d'origine) n'est pas repris : un passe-partout est opaque et noir d'origine.
    let ancien: crate::settings::Settings = toml::from_str("opacite_passe_partout = 27").unwrap();
    assert_eq!(ancien.passe_partout, 100);
    let mut s = crate::settings::Settings::default();
    s.passe_partout = 65;
    s.passe_partout_couleur = [10, 20, 30];
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("passe_partout_opacite = 65") && texte.contains("passe_partout_couleur"), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).unwrap();
    assert_eq!((relu.passe_partout, relu.passe_partout_couleur), (65, [10, 20, 30]));
}

#[test]
fn the_preferences_window_has_the_passe_partout_slider() {
    let mut app = app_one_layer("prefs", LayerKind::Vector);
    app.settings.prefs_onglet = crate::prefsonglets::Onglet::Dessin;
    app.prefs_window = true;
    let mut ecran = Ecran::new();
    let mut ui_prefs = |a: &mut App, ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        a.preferences_window(&ctx);
    };
    let textes = ecran.chauffer(&mut app, 4, &mut ui_prefs);
    assert!(contient(&textes, "Opacité du passe-partout"), "{textes:?}");
    assert!(contient(&textes, "Couleur du passe-partout"), "{textes:?}");
    assert!(contient(&textes, "100 %"), "la valeur d'origine : {textes:?}");
}

#[test]
fn the_camera_frame_is_one_physical_pixel_thick_and_sits_on_the_pixel_grid() {
    use crate::camtools::{crisp, frame_halo_width, frame_line_width};
    for ppp in [1.0_f32, 1.25, 1.5, 2.0, 3.0] {
        let px = frame_line_width(ppp, false) * ppp;
        assert!((px - 1.0).abs() < 1e-4, "{ppp} : {px} pixel(s)");
        let edit = frame_line_width(ppp, true) * ppp;
        assert!((edit - 1.5).abs() < 1e-4, "avec l'outil Caméra : {edit}");
        // Le liseré sombre : un pixel de plus de chaque côté.
        assert!((frame_halo_width(ppp, false) * ppp - 3.0).abs() < 1e-4);
        // Les coins tombent au milieu d'un pixel.
        let p = crisp(pos2(10.3, 20.7), ppp);
        let (gx, gy) = (p.x * ppp - 0.5, p.y * ppp - 0.5);
        assert!((gx - gx.round()).abs() < 1e-3 && (gy - gy.round()).abs() < 1e-3, "{ppp} : {p:?}");
    }
    assert!(frame_line_width(1.0, false) < 3.0, "plus fin que l'ancien liseré de 3 points");
}

/// Un plan dont la caméra est resserrée (le cadre n'est plus la feuille entière : le voile se peint).
fn app_with_zoomed_camera(name: &str) -> App {
    let mut app = app_one_layer(name, LayerKind::Vector);
    let [fw, fh] = app.doc.frame;
    app.set_current_camera(crate::camera::Camera { zoom: 2.0, ..crate::camera::Camera::centered(fw, fh) });
    app
}

/// Les formes que peint `paint_camera` : (traits fermés avec leur épaisseur, sommets du voile).
fn peindre_camera(app: &App, ppp: f32) -> (Vec<f32>, Vec<u8>) {
    let ctx = egui::Context::default();
    let mut input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 480.0))), ..RawInput::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(ppp);
    let out = ctx.run(input, |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("essai_camera")));
        app.paint_camera(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 480.0)));
    });
    let (mut traits, mut voile) = (Vec::new(), Vec::new());
    fn parcourir(s: &Shape, traits: &mut Vec<f32>, voile: &mut Vec<u8>) {
        match s {
            Shape::Path(p) if p.closed => traits.push(p.stroke.width),
            Shape::Mesh(m) => voile.extend(m.vertices.iter().map(|v| v.color.a())),
            Shape::Vec(v) => v.iter().for_each(|s| parcourir(s, traits, voile)),
            _ => {}
        }
    }
    for s in &out.shapes {
        parcourir(&s.shape, &mut traits, &mut voile);
    }
    (traits, voile)
}

#[test]
fn the_camera_frame_is_painted_thin_on_a_dark_halo() {
    let app = app_with_zoomed_camera("cadre-fin");
    for ppp in [1.0_f32, 1.5, 2.0] {
        let (traits, _) = peindre_camera(&app, ppp);
        assert_eq!(traits.len(), 2, "un liseré sombre et le trait : {traits:?}");
        let px: Vec<f32> = traits.iter().map(|w| w * ppp).collect();
        assert!((px[0] - 3.0).abs() < 1e-3 && (px[1] - 1.0).abs() < 1e-3, "{ppp} : liseré {} px, trait {} px", px[0], px[1]);
    }
}

#[test]
fn the_passe_partout_uses_the_preferred_opacity() {
    let mut app = app_with_zoomed_camera("voile");
    app.settings.passe_partout_actif = true;
    let alpha_max = |app: &App| peindre_camera(app, 1.0).1.into_iter().max().unwrap_or(0);
    app.settings.passe_partout = crate::camtools::PASSE_PARTOUT_DEFAUT;
    // D'origine : opaque, sur la toile infinie comme ailleurs.
    assert_eq!(alpha_max(&app), 255, "le passe-partout d'origine est opaque");
    app.settings.passe_partout = 60;
    assert_eq!(alpha_max(&app), crate::camtools::passe_partout_alpha(60));
    // Pendant la lecture, il ne change pas d'aspect.
    app.playing = true;
    assert_eq!(alpha_max(&app), crate::camtools::passe_partout_alpha(60));
    app.playing = false;
    // 0 % : rien n'est couvert (comme si « Passe-partout » était éteint) ; le cadre, lui, reste.
    app.settings.passe_partout = 0;
    let (traits, voile) = peindre_camera(&app, 1.0);
    assert!(voile.is_empty() || voile.iter().all(|a| *a == 0), "{voile:?}");
    assert_eq!(traits.len(), 2);
    // Éteint : rien non plus, quelle que soit l'opacité.
    app.settings.passe_partout = 80;
    app.settings.passe_partout_actif = false;
    assert!(peindre_camera(&app, 1.0).1.iter().all(|a| *a == 0));
}
