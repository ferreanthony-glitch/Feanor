//! Les effets au clavier (Anthony, 2026-10-21) : on choisit des effets dans le panneau Effets (clic, Ctrl /
//! Maj + clic), Ctrl+C les copie avec tous leurs réglages et leurs clés, Ctrl+V les colle sur un autre calque
//! (cliqué dans le panneau Calques, ou le panneau Effets ouvert dessus), en une étape d'annulation. Le bouton
//! « Copier / coller » du panneau a disparu ; le tremblement du trait est le premier du menu « Trait ».

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::perf_tests::{banc, Banc};

// ---------------------------------------------------------------------- l'outillage

fn blur() -> Effect {
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.set_key(2, vec![4.0]);
    e.set_key(6, vec![12.0]);
    e
}

fn glow() -> Effect {
    Effect { enabled: false, ..Effect::new(EffectKind::Glow) }
}

fn kinds(stack: &[Effect]) -> Vec<EffectKind> {
    stack.iter().map(|e| e.kind).collect()
}

/// Un plan de `frames` images à quatre calques : A (bitmap, flou aux clés 2 et 6, lueur éteinte, bruit), B
/// (bitmap), C (vectoriel), D (bitmap). Le calque courant est A.
fn bench(frames: usize) -> Banc {
    let mut doc = Document::with_frame(64, 36, 24);
    let mut a = Layer::new("A", LayerKind::Bitmap);
    a.fx = vec![blur(), glow(), Effect::new(EffectKind::Noise)];
    doc.panels[0].layers = vec![a, Layer::new("B", LayerKind::Bitmap), Layer::new("C", LayerKind::Vector), Layer::new("D", LayerKind::Bitmap)];
    doc.panels[0].length = frames;
    let mut b = banc(doc, false);
    b.app.cur_layer = 0;
    b.app.layer_sel.clear();
    b
}

fn stack_of(b: &Banc, li: usize) -> Vec<Effect> {
    b.app.layers()[li].fx.clone()
}

fn step_names(b: &Banc) -> Vec<String> {
    b.app.history.names().0.iter().map(|s| s.to_string()).collect()
}

/// Un écran sans fenêtre où l'on peint un morceau d'interface ; rend les textes peints et leur place.
struct Ecran {
    ctx: egui::Context,
    t: f64,
}

impl Ecran {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 900.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.fx_panel(ui));
        });
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new());
        }
        textes
    }

    fn cliquer(&mut self, app: &mut App, p: egui::Pos2) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p)]);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]);
        self.chauffer(app, 3)
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn trouver<'a>(textes: &'a [(String, Rect)], debut: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.trim_start().starts_with(debut))
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

/// Une application sans fenêtre avec un seul calque de la sorte `kind`.
fn app_one_layer(name: &str, kind: LayerKind) -> App {
    let dir = std::env::temp_dir().join(format!("shot-fxclavier-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    let cel = doc.new_cel(kind);
    let mut l = Layer::new("Dessin", kind);
    l.frames = vec![Some(cel); 24];
    doc.panels[0].layers.push(l);
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app
}

// ---------------------------------------------------------------------- choisir des effets

#[test]
fn clicking_an_effect_name_chooses_it_and_gives_the_pane_the_keyboard() {
    let mut b = bench(8);
    b.app.images_focus = true;
    b.app.key_focus = true;
    b.app.fx_click_name(1, false, false);
    assert_eq!(b.app.fx_selected(), vec![1]);
    assert_eq!(b.app.fx_index(), Some(1), "ses clés se voient");
    assert!(b.app.fx_pane_has_focus(), "le panneau a la main");
    assert!(!b.app.images_focus && !b.app.key_focus, "la timeline la perd");
    // Un second clic sur l'unique effet choisi le lâche.
    b.app.fx_click_name(1, false, false);
    assert!(b.app.fx_selected().is_empty());
    // Un clic sur un autre remplace le choix.
    b.app.fx_click_name(0, false, false);
    b.app.fx_click_name(2, false, false);
    assert_eq!(b.app.fx_selected(), vec![2]);
}

#[test]
fn ctrl_click_adds_and_removes_and_shift_click_takes_a_range() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.fx_click_name(2, true, false);
    assert_eq!(b.app.fx_selected(), vec![0, 2]);
    assert_eq!(b.app.fx_index(), Some(2), "le dernier ajouté montre ses clés");
    // Ctrl + clic sur un choisi : il sort du choix.
    b.app.fx_click_name(2, true, false);
    assert_eq!(b.app.fx_selected(), vec![0]);
    b.app.fx_click_name(0, true, false);
    assert!(b.app.fx_selected().is_empty(), "plus rien de choisi");
    // Maj + clic : de l'effet choisi à celui-ci.
    b.app.fx_click_name(0, false, false);
    b.app.fx_click_name(2, false, true);
    assert_eq!(b.app.fx_selected(), vec![0, 1, 2]);
    assert_eq!(b.app.fx_index(), Some(0), "la plage part de l'effet choisi");
    // Un clic simple revient à un seul.
    b.app.fx_click_name(1, false, false);
    assert_eq!(b.app.fx_selected(), vec![1]);
}

#[test]
fn the_choice_is_dropped_when_the_layer_changes() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.fx_click_name(1, true, false);
    b.app.select_layer(1);
    assert!(b.app.fx_selected().is_empty(), "les effets choisis étaient ceux du calque A");
}

// ---------------------------------------------------------------------- copier, coller

#[test]
fn ctrl_c_ctrl_v_copies_an_effect_to_another_layer_with_the_same_settings_and_keys() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.run_action("edition.copier");
    assert_eq!(kinds(&b.app.fx_stack_clipboard.clone().unwrap().effects), vec![EffectKind::GaussianBlur]);
    // Un clic sur le calque B dans le panneau Calques : il a la main, le panneau Effets non.
    b.app.select_layer(1);
    b.app.fx_pane_focus = false;
    b.app.take_layers_pane_focus();
    let steps = step_names(&b).len();
    b.app.run_action("edition.coller");
    assert_eq!(stack_of(&b, 1), vec![blur()], "mêmes réglages, mêmes clés, aux mêmes images");
    assert_eq!(stack_of(&b, 0).len(), 3, "la source ne change pas");
    assert_eq!(step_names(&b).len(), steps + 1, "une étape d'annulation");
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Coller l'effet"));
    b.app.undo();
    assert!(stack_of(&b, 1).is_empty());
}

#[test]
fn several_chosen_effects_are_copied_and_pasted_in_one_named_step() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.fx_click_name(1, true, false);
    b.app.run_action("edition.copier");
    assert_eq!(b.app.fx_stack_clipboard.as_ref().map(|c| c.effects.len()), Some(2));
    assert!(b.app.status.starts_with("2 effets copiés"), "{}", b.app.status);
    b.app.select_layer(3);
    b.app.fx_pane_focus = false;
    b.app.take_layers_pane_focus();
    let steps = step_names(&b).len();
    b.app.run_action("edition.coller");
    assert_eq!(stack_of(&b, 3), vec![blur(), glow()], "dans l'ordre de la pile, la lueur éteinte reste éteinte");
    assert_eq!(step_names(&b).len(), steps + 1);
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Coller 2 effets"));
    // Les effets collés sont les effets choisis : on peut les recopier aussitôt.
    assert_eq!(b.app.fx_selected(), vec![0, 1]);
    b.app.undo();
    assert!(stack_of(&b, 3).is_empty(), "un seul Ctrl+Z défait les deux");
}

#[test]
fn an_effect_of_the_same_kind_is_added_again_not_replaced() {
    let mut b = bench(8);
    b.app.layers_mut()[1].fx = vec![Effect::new(EffectKind::GaussianBlur)];
    b.app.fx_click_name(0, false, false);
    b.app.run_action("edition.copier");
    b.app.select_layer(1);
    b.app.run_action("edition.coller");
    assert_eq!(kinds(&stack_of(&b, 1)), vec![EffectKind::GaussianBlur, EffectKind::GaussianBlur], "le flou de B reste, celui de A s'ajoute après");
    assert_eq!(stack_of(&b, 1)[1], blur());
}

#[test]
fn the_palette_replace_command_stays_available() {
    let mut b = bench(8);
    b.app.layers_mut()[1].fx = vec![Effect::new(EffectKind::Invert)];
    b.app.fx_click_name(0, false, false);
    b.app.run_action("fx.copier");
    b.app.select_layer(1);
    b.app.run_action("fx.remplacer");
    assert_eq!(stack_of(&b, 1), vec![blur()]);
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Remplacer les effets"));
    b.app.run_action("fx.coller");
    assert_eq!(stack_of(&b, 1).len(), 2, "fx.coller ajoute toujours, panneaux ou non");
}

#[test]
fn the_calques_panel_only_pastes_effects_when_effects_were_the_last_thing_copied() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.run_action("edition.copier");
    assert!(b.app.fx_pile_recent);
    // On copie autre chose (les plans) : la copie d'effets n'est plus la dernière.
    b.app.copy_panels();
    assert!(!b.app.fx_pile_recent);
    b.app.select_layer(1);
    b.app.fx_pane_focus = false;
    b.app.take_layers_pane_focus();
    b.app.run_action("edition.coller");
    assert!(stack_of(&b, 1).is_empty(), "Ctrl+V garde son sens d'avant");
    // Une copie par Ctrl+C ailleurs (rien de choisi) fait de même.
    b.app.fx_pane_focus = true;
    b.app.fx_click_name(0, false, false);
    b.app.select_layer(0);
    b.app.run_action("edition.copier");
    assert!(b.app.fx_pile_recent);
    b.app.fx_pane_focus = false;
    b.app.run_action("edition.copier");
    assert!(!b.app.fx_pile_recent, "une autre copie lâche celle des effets");
}

#[test]
fn without_either_panel_in_hand_ctrl_v_does_not_paste_effects() {
    let mut b = bench(8);
    b.app.fx_click_name(0, false, false);
    b.app.run_action("edition.copier");
    b.app.fx_pane_focus = false;
    b.app.layers_pane_focus = false;
    b.app.select_layer(1);
    b.app.run_action("edition.coller");
    assert!(stack_of(&b, 1).is_empty(), "le dessin a la main : Ctrl+V colle un dessin, pas des effets");
    assert!(!b.app.fx_paste_has_hand());
}

#[test]
fn a_press_anywhere_gives_the_calques_panel_and_the_effects_pane_back_their_hand() {
    let mut b = bench(8);
    b.app.take_layers_pane_focus();
    b.app.take_fx_pane_focus();
    b.image(
        vec![
            egui::Event::PointerMoved(egui::pos2(300.0, 5.0)),
            egui::Event::PointerButton { pos: egui::pos2(300.0, 5.0), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
        ],
        1.0 / 60.0,
    );
    assert!(!b.app.layers_pane_focus && !b.app.fx_pane_focus);
}

#[test]
fn ctrl_c_with_the_pane_in_hand_and_no_choice_keeps_asking_which_effect() {
    let mut b = bench(8);
    b.app.take_fx_pane_focus();
    b.app.run_action("edition.copier");
    assert!(b.app.fx_stack_clipboard.is_none());
    assert!(b.app.status.starts_with("Choisis d'abord"), "{}", b.app.status);
}

#[test]
fn an_effect_pasted_from_a_project_of_another_sheet_size_is_scaled() {
    let mut b = banc(
        {
            let mut doc = Document::with_frame(320, 180, 24);
            let mut a = Layer::new("A", LayerKind::Bitmap);
            a.fx = vec![blur()];
            doc.panels[0].layers = vec![a];
            doc.panels[0].length = 8;
            doc
        },
        false,
    );
    b.app.cur_layer = 0;
    b.app.fx_click_name(0, false, false);
    b.app.run_action("edition.copier");
    assert!(b.app.fx_stack_clipboard.is_some(), "{}", b.app.status);
    // Un autre onglet, une feuille deux fois plus grande.
    let mut doc = Document::with_frame(640, 360, 24);
    doc.panels[0].layers = vec![Layer::new("Calque", LayerKind::Bitmap)];
    doc.panels[0].length = 8;
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    assert!(b.app.charger_projet(doc));
    b.app.welcome = false;
    b.app.cur_layer = 0;
    b.app.fx_pane_focus = false;
    b.app.take_layers_pane_focus();
    b.app.run_action("edition.coller");
    let pasted = b.app.layers()[0].fx.clone();
    assert_eq!(pasted.len(), 1, "{}", b.app.status);
    assert!((pasted[0].keys[0].values[0] - 8.0).abs() < 1e-3 && (pasted[0].keys[1].values[0] - 24.0).abs() < 1e-3, "les clés aussi : {:?}", pasted[0].keys);
    assert_eq!(pasted[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 6], "les images ne bougent pas");
}

// ---------------------------------------------------------------------- le panneau

#[test]
fn the_copy_paste_button_is_gone_from_the_effects_panel() {
    let mut app = app_one_layer("bouton", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let textes = ecran.chauffer(&mut app, 4);
    assert!(trouver(&textes, "Ajouter un effet").is_some() || contient(&textes, "Ajouter un effet"), "{textes:?}");
    assert!(!contient(&textes, "Copier / coller"), "le bouton a disparu : {textes:?}");
    assert!(contient(&textes, "Préréglages") && contient(&textes, "Trait"), "le reste du panneau est là");
}

#[test]
fn the_boil_leads_the_trait_menu_and_is_not_a_general_entry_of_the_add_menu() {
    let mut app = app_one_layer("menus", LayerKind::Vector);
    let mut ecran = Ecran::new();
    let textes = ecran.chauffer(&mut app, 4);
    // « Ajouter un effet » : pas d'entrée « Tremblement du trait » en tête, avant les familles.
    let (_, r) = trouver(&textes, &format!("{}  Ajouter un effet", egui_phosphor::regular::PLUS)).cloned().expect("le bouton Ajouter un effet");
    let ouvert = ecran.cliquer(&mut app, r.center());
    assert!(!contient(&ouvert, "Tremblement du trait"), "plus d'entrée à part dans la liste générale : {ouvert:?}");
    assert!(contient(&ouvert, "Flou"), "les familles sont là : {ouvert:?}");
    // Le menu « Trait » : le tremblement en premier, puis les effets du trait.
    let mut ecran = Ecran::new();
    let textes = ecran.chauffer(&mut app, 4);
    let (_, r) = trouver(&textes, "Trait").cloned().expect("le menu Trait");
    let ouvert = ecran.cliquer(&mut app, r.center());
    let boil = format!("{}  {}", crate::fxtrait::icon(EffectKind::Boil), EffectKind::Boil.label());
    let write = format!("{}  {}", crate::fxtrait::icon(EffectKind::Write), EffectKind::Write.label());
    let (_, rb) = trouver(&ouvert, &boil).cloned().unwrap_or_else(|| panic!("le tremblement dans le menu Trait : {ouvert:?}"));
    let (_, rw) = trouver(&ouvert, &write).cloned().expect("l'écriture du trait");
    assert!(rb.min.y < rw.min.y, "le tremblement est le premier de la liste");
    // Un clic le coche : une étape d'annulation, comme les autres.
    let n0 = app.history.names().0.len();
    ecran.cliquer(&mut app, rb.center());
    assert!(app.fx_on(0, EffectKind::Boil), "{}", app.status);
    assert_eq!(app.history.names().0.len(), n0 + 1);
}

#[test]
fn on_a_bitmap_layer_the_boil_stays_listed_in_the_trait_menu_but_greyed() {
    let mut app = app_one_layer("bitmap", LayerKind::Bitmap);
    let mut ecran = Ecran::new();
    let textes = ecran.chauffer(&mut app, 4);
    let (_, r) = trouver(&textes, "Trait").cloned().unwrap();
    let ouvert = ecran.cliquer(&mut app, r.center());
    let boil = format!("{}  {}", crate::fxtrait::icon(EffectKind::Boil), EffectKind::Boil.label());
    let (_, rb) = trouver(&ouvert, &boil).cloned().expect("l'entrée grisée est listée");
    assert!(matches!(app.fx_offer(0, EffectKind::Boil), crate::fxui::Offer::Disabled(why) if why.contains("vectoriel")));
    ecran.cliquer(&mut app, rb.center());
    assert!(app.layers()[0].fx.is_empty(), "le tremblement ne se pose pas sur un bitmap");
}

#[test]
fn the_boil_is_of_the_trait_family_but_saved_exactly_as_before() {
    assert_eq!(EffectKind::Boil.family(), "Trait");
    assert!(crate::fxtrait::menu_kinds().starts_with(&[EffectKind::Boil]));
    assert_eq!(crate::fxtrait::menu_kinds()[1..], crate::fxtrait::KINDS, "les effets du trait suivent, dans le même ordre");
    // Un projet avec un tremblement se recharge à l'identique (la famille n'est pas enregistrée).
    let mut doc = Document::new(64, 64, 24);
    let id = doc.new_cel(LayerKind::Vector);
    let layer = &mut doc.panels[0].layers[0];
    layer.kind = LayerKind::Vector;
    layer.frames = vec![Some(id)];
    let mut boil = Effect::new(EffectKind::Boil);
    boil.values = vec![7.5, 20.0, 5.0, 3.0, 35.0, 42.0, 1.0];
    boil.set_key(4, vec![2.0, 80.0, 4.0, 1.0, 0.0, 42.0, 1.0]);
    layer.fx = vec![blur(), boil];
    let dir = std::env::temp_dir().join(format!("shot-test-fxclavier-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"tremblement\"") && !text.contains("Déformation"), "{text}");
    let back = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(back.panels[0].layers[0].fx, doc.panels[0].layers[0].fx);
    std::fs::remove_dir_all(&dir).ok();
}
