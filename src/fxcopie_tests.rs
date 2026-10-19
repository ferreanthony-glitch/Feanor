//! Copier-coller des effets d'un calque à un autre : la logique pure (`paste_onto`, `message`), puis
//! l'application sans fenêtre (le banc de `perf_tests`) pour les calques choisis, l'annulation, la
//! barre d'état et la priorité du focus sur les autres presse-papiers.

use crate::document::{Document, Layer, LayerKind};
use crate::fx::{Effect, EffectKind};
use crate::fxcopie::{fit_to_plan, message, paste_onto, Landed, Report};
use crate::perf_tests::{banc, Banc};

/// Un flou gaussien aux clés 2 et 6, et sa pile : flou, lueur (sans clé).
fn blur() -> Effect {
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.set_key(2, vec![4.0]);
    e.set_key(6, vec![12.0]);
    e
}

fn glow() -> Effect {
    Effect { enabled: false, ..Effect::new(EffectKind::Glow) }
}

fn boil() -> Effect {
    Effect::new(EffectKind::Boil)
}

fn kinds(stack: &[Effect]) -> Vec<EffectKind> {
    stack.iter().map(|e| e.kind).collect()
}

// ---------------------------------------------------------------------- logique pure

#[test]
fn a_pasted_effect_keeps_its_kind_state_values_and_keys() {
    let mut src = blur();
    src.enabled = false;
    src.values = vec![9.0];
    let mut stack = vec![Effect::new(EffectKind::Invert)];
    let landed = paste_onto(&mut stack, std::slice::from_ref(&src), false, 24, false);
    assert_eq!(landed.added, vec![EffectKind::GaussianBlur]);
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].kind, EffectKind::Invert, "les effets du calque restent, les collés s'ajoutent à la fin");
    assert_eq!(stack[1], src, "l'effet collé est une copie exacte : sorte, état, valeurs et clés");
    assert_eq!(stack[1].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2, 6], "les clés gardent leurs images");
}

#[test]
fn replacing_swaps_the_whole_stack() {
    let mut stack = vec![Effect::new(EffectKind::Invert), Effect::new(EffectKind::Noise)];
    let landed = paste_onto(&mut stack, &[blur(), glow()], false, 24, true);
    assert_eq!(kinds(&stack), vec![EffectKind::GaussianBlur, EffectKind::Glow]);
    assert_eq!(landed.removed, 2);
    assert_eq!(landed.added.len(), 2);
}

#[test]
fn replacing_with_nothing_pastable_leaves_the_stack_alone() {
    let mut stack = vec![Effect::new(EffectKind::Invert)];
    let landed = paste_onto(&mut stack, &[boil()], false, 24, true);
    assert_eq!(kinds(&stack), vec![EffectKind::Invert], "un calque bitmap garde sa pile si le seul effet copié est un tremblement");
    assert!(landed.added.is_empty());
    assert_eq!(landed.vector_only_skipped, vec![EffectKind::Boil]);
    assert_eq!(landed.removed, 0);
}

#[test]
fn the_boil_is_left_out_on_a_bitmap_layer_and_goes_on_a_vector_one() {
    let clip = [blur(), boil()];
    let mut bitmap = Vec::new();
    let landed = paste_onto(&mut bitmap, &clip, false, 24, false);
    assert_eq!(kinds(&bitmap), vec![EffectKind::GaussianBlur]);
    assert_eq!(landed.vector_only_skipped, vec![EffectKind::Boil]);
    let mut vector = Vec::new();
    let landed = paste_onto(&mut vector, &clip, true, 24, false);
    assert_eq!(kinds(&vector), vec![EffectKind::GaussianBlur, EffectKind::Boil]);
    assert!(landed.vector_only_skipped.is_empty());
}

#[test]
fn a_layer_never_gets_a_second_boil() {
    let mut stack = vec![boil()];
    let landed = paste_onto(&mut stack, &[boil(), blur()], true, 24, false);
    assert_eq!(kinds(&stack), vec![EffectKind::Boil, EffectKind::GaussianBlur]);
    assert_eq!(landed.duplicates_skipped, vec![EffectKind::Boil]);
    // Remplacer: la pile qui part n'a plus de tremblement à doubler.
    let mut stack = vec![boil()];
    let landed = paste_onto(&mut stack, &[boil()], true, 24, true);
    assert_eq!(kinds(&stack), vec![EffectKind::Boil]);
    assert!(landed.duplicates_skipped.is_empty());
}

#[test]
fn keys_after_the_end_of_the_plan_are_dropped() {
    let (fit, dropped) = fit_to_plan(&blur(), 4);
    assert_eq!(dropped, 1);
    assert_eq!(fit.keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2]);
    // Une clé juste sur la dernière image (3) reste, une sur la 4e (hors d'un plan de 4) part.
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.set_key(3, vec![1.0]);
    e.set_key(4, vec![2.0]);
    let (fit, dropped) = fit_to_plan(&e, 4);
    assert_eq!((fit.keys.len(), dropped), (1, 1));
    // Aucune clé hors du plan : la copie est identique.
    let (same, dropped) = fit_to_plan(&blur(), 24);
    assert_eq!((same, dropped), (blur(), 0));
}

#[test]
fn when_every_key_is_dropped_the_effect_keeps_the_values_of_its_first_key() {
    let mut e = Effect::new(EffectKind::GaussianBlur);
    e.set_key(10, vec![7.0]);
    e.set_key(12, vec![15.0]);
    let shown = e.values_at(0);
    let (fit, dropped) = fit_to_plan(&e, 5);
    assert_eq!(dropped, 2);
    assert!(fit.keys.is_empty());
    assert_eq!(fit.values, shown, "ce que l'effet montrait dans tout le plan");
    assert_eq!(fit.values_at(3), vec![7.0]);
}

#[test]
fn keys_left_out_do_not_count_when_nothing_is_pasted() {
    let mut late = boil();
    late.set_key(50, vec![3.0, 55.0, 3.0, 2.0, 0.0, 1.0, 0.0]);
    let mut stack = Vec::new();
    let landed = paste_onto(&mut stack, &[late], false, 4, false);
    assert_eq!(landed, Landed { vector_only_skipped: vec![EffectKind::Boil], ..Landed::default() });
}

#[test]
fn the_status_names_what_went_where() {
    let mut rep = Report::default();
    for li in [0, 2, 3] {
        rep.absorb(li, &Landed { added: vec![EffectKind::GaussianBlur], ..Landed::default() });
    }
    assert_eq!(message(&rep, false, "A"), "Flou gaussien collé sur 3 calques.");

    let mut one = Report::default();
    one.absorb(1, &Landed { added: vec![EffectKind::Glow], ..Landed::default() });
    assert_eq!(message(&one, false, "Fond"), "Lueur collée sur le calque « Fond ».");

    let mut many = Report::default();
    many.absorb(0, &Landed { added: vec![EffectKind::GaussianBlur, EffectKind::Glow], ..Landed::default() });
    assert_eq!(message(&many, false, "A"), "2 effets collés sur le calque « A ».");
    assert_eq!(message(&many, true, "A"), "Effets remplacés sur le calque « A » : Flou gaussien, Lueur.");

    let mut skipped = Report::default();
    skipped.absorb(0, &Landed { added: vec![EffectKind::GaussianBlur], keys_dropped: 2, ..Landed::default() });
    skipped.absorb(1, &Landed { added: vec![EffectKind::GaussianBlur], vector_only_skipped: vec![EffectKind::Boil], ..Landed::default() });
    skipped.folders = 1;
    let text = message(&skipped, false, "A");
    assert!(text.starts_with("Flou gaussien collé sur 2 calques"), "{text}");
    assert!(text.contains("Tremblement du trait ignoré sur 1 calque bitmap"), "{text}");
    assert!(text.contains("2 clés après la fin du plan laissées de côté"), "{text}");
    assert!(text.contains("1 dossier ignoré"), "{text}");

    let nothing = Report { bitmap_layers: 1, vector_only: vec![EffectKind::Boil], ..Report::default() };
    assert!(message(&nothing, false, "A").starts_with("Rien collé"), "{}", message(&nothing, false, "A"));
}

#[test]
fn the_participle_agrees_with_the_effect() {
    let one = |kind| {
        let mut r = Report::default();
        r.absorb(0, &Landed { added: vec![kind], ..Landed::default() });
        message(&r, false, "A")
    };
    assert!(one(EffectKind::DropShadow).starts_with("Ombre portée collée"));
    assert!(one(EffectKind::Levels).starts_with("Niveaux collés"));
    assert!(one(EffectKind::Invert).starts_with("Négatif collé"));
}

// ---------------------------------------------------------------------- l'application

/// Un plan de 8 images à quatre calques : A (bitmap, un flou aux clés 2 et 6 et une lueur éteinte), B
/// (bitmap), C (vectoriel), D (bitmap). Le calque courant est A.
fn bench(frames: usize) -> Banc {
    let mut doc = Document::with_frame(64, 36, 24);
    let mut a = Layer::new("A", LayerKind::Bitmap);
    a.fx = vec![blur(), glow()];
    let layers = vec![a, Layer::new("B", LayerKind::Bitmap), Layer::new("C", LayerKind::Vector), Layer::new("D", LayerKind::Bitmap)];
    doc.panels[0].layers = layers;
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

/// Choisit les calques `lis` (le calque courant en plus).
fn choose(b: &mut Banc, lis: &[usize]) {
    b.app.layer_sel = lis.iter().map(|&li| b.app.layers()[li].id).collect();
}

#[test]
fn copy_one_effect_with_its_keys_and_paste_it_on_another_layer() {
    let mut b = bench(8);
    b.app.fx_copy_effect(0, 0);
    assert_eq!(b.app.fx_stack_clipboard.as_ref().map(|c| c.effects.len()), Some(1));
    assert!(b.app.status.contains("Flou gaussien") && b.app.status.contains("2 clés"), "{}", b.app.status);
    b.app.select_layer(1);
    let before = step_names(&b).len();
    b.app.run_action("fx.coller");
    assert_eq!(stack_of(&b, 1), vec![blur()], "le calque B a le flou, ses clés aux mêmes images");
    assert_eq!(stack_of(&b, 0).len(), 2, "le calque A ne change pas");
    assert_eq!(b.app.status, "Flou gaussien collé sur le calque « B ».");
    assert_eq!(step_names(&b).len(), before + 1);
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Coller l'effet"));
    // Le premier effet collé est choisi sur le calque courant (ses clés se voient).
    assert_eq!(b.app.fx_index(), Some(0));
}

#[test]
fn pasting_adds_on_top_of_the_stack_and_undo_restores_it() {
    let mut b = bench(8);
    b.app.fx_copy_effect(0, 0);
    b.app.select_layer(3);
    b.app.fx_paste(vec![3], false);
    b.app.fx_paste(vec![3], false);
    assert_eq!(stack_of(&b, 3).len(), 2, "deux collages, deux effets");
    b.app.undo();
    assert_eq!(stack_of(&b, 3).len(), 1);
    b.app.undo();
    assert!(stack_of(&b, 3).is_empty());
    b.app.redo();
    assert_eq!(stack_of(&b, 3), vec![blur()]);
}

#[test]
fn copy_all_and_paste_on_several_chosen_layers_in_one_undo_step() {
    let mut b = bench(8);
    b.app.run_action("fx.copier_tout");
    assert_eq!(b.app.fx_stack_clipboard.as_ref().map(|c| c.effects.len()), Some(2));
    assert!(b.app.status.contains("2 effets"), "{}", b.app.status);
    // B, C et D choisis (A, la source, reste courant) : on colle depuis B.
    b.app.select_layer(1);
    choose(&mut b, &[2, 3]);
    let steps = step_names(&b).len();
    b.app.run_action("fx.coller");
    for li in [1, 2, 3] {
        assert_eq!(kinds(&stack_of(&b, li)), vec![EffectKind::GaussianBlur, EffectKind::Glow], "calque {li}");
        assert!(!stack_of(&b, li)[1].enabled, "la lueur éteinte reste éteinte");
    }
    assert_eq!(stack_of(&b, 0).len(), 2, "le calque non choisi ne change pas");
    assert_eq!(b.app.status, "2 effets collés sur 3 calques.");
    assert_eq!(step_names(&b).len(), steps + 1, "une seule étape pour trois calques");
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Coller les effets"));
    b.app.undo();
    for li in [1, 2, 3] {
        assert!(stack_of(&b, li).is_empty(), "l'annulation d'un coup restaure les trois calques ({li})");
    }
}

#[test]
fn a_single_effect_pasted_on_three_layers_says_so() {
    let mut b = bench(8);
    b.app.fx_copy_effect(0, 0);
    b.app.select_layer(1);
    choose(&mut b, &[2, 3]);
    b.app.run_action("fx.coller");
    assert_eq!(b.app.status, "Flou gaussien collé sur 3 calques.");
}

#[test]
fn a_menu_on_a_layer_outside_the_choice_targets_that_layer_alone() {
    let mut b = bench(8);
    b.app.select_layer(1);
    choose(&mut b, &[2]);
    assert_eq!(b.app.fx_paste_targets(1), vec![1, 2]);
    assert_eq!(b.app.fx_paste_targets(3), vec![3], "D n'est pas choisi : le menu de D ne vise que D");
}

#[test]
fn replace_swaps_the_stack_of_each_target_in_one_step() {
    let mut b = bench(8);
    b.app.fx_copy_effect(0, 1);
    b.app.layers_mut()[1].fx = vec![Effect::new(EffectKind::Invert), Effect::new(EffectKind::Noise)];
    b.app.select_layer(1);
    b.app.select_fx(1);
    let steps = step_names(&b).len();
    b.app.run_action("fx.remplacer");
    assert_eq!(kinds(&stack_of(&b, 1)), vec![EffectKind::Glow]);
    assert_eq!(step_names(&b).len(), steps + 1);
    assert_eq!(step_names(&b).last().map(String::as_str), Some("Remplacer les effets"));
    assert!(b.app.status.starts_with("Effets remplacés sur le calque « B » : Lueur"), "{}", b.app.status);
    b.app.undo();
    assert_eq!(kinds(&stack_of(&b, 1)), vec![EffectKind::Invert, EffectKind::Noise]);
}

#[test]
fn the_boil_is_skipped_on_bitmap_layers_with_a_message_naming_it() {
    let mut b = bench(8);
    // C, vectoriel, a un tremblement et un flou ; on copie toute sa pile.
    b.app.layers_mut()[2].fx = vec![Effect::new(EffectKind::Boil), blur()];
    b.app.select_layer(2);
    b.app.fx_copy_all(2);
    // Sur A (bitmap) : le flou seul.
    b.app.select_layer(3);
    b.app.fx_paste(vec![3], false);
    assert_eq!(kinds(&stack_of(&b, 3)), vec![EffectKind::GaussianBlur]);
    assert!(b.app.status.contains("Tremblement du trait ignoré sur 1 calque bitmap"), "{}", b.app.status);
    // Sur un autre calque vectoriel, les deux.
    b.app.layers_mut()[1].kind = LayerKind::Vector;
    b.app.fx_paste(vec![1], false);
    assert_eq!(kinds(&stack_of(&b, 1)), vec![EffectKind::Boil, EffectKind::GaussianBlur]);
    // Un calque bitmap seul avec seulement un tremblement copié : rien n'est collé, pas d'étape.
    b.app.fx_copy_effect(2, 0);
    let steps = step_names(&b).len();
    b.app.fx_paste(vec![0], false);
    assert_eq!(stack_of(&b, 0).len(), 2);
    assert_eq!(step_names(&b).len(), steps, "rien collé : aucune étape d'annulation");
    assert!(b.app.status.starts_with("Rien collé") && b.app.status.contains("Tremblement du trait"), "{}", b.app.status);
}

#[test]
fn keys_beyond_the_end_of_a_shorter_plan_are_dropped_with_a_message() {
    // Un plan de 4 images : la clé 6 du flou tombe hors du plan, la clé 2 reste.
    let mut b = bench(4);
    b.app.fx_copy_effect(0, 0);
    b.app.fx_paste(vec![1], false);
    let pasted = stack_of(&b, 1);
    assert_eq!(pasted[0].keys.iter().map(|k| k.frame).collect::<Vec<_>>(), vec![2]);
    assert!(b.app.status.contains("1 clé après la fin du plan laissée de côté"), "{}", b.app.status);
    // La source garde ses deux clés.
    assert_eq!(stack_of(&b, 0)[0].keys.len(), 2);
}

#[test]
fn nothing_to_paste_or_a_locked_storyboard_changes_nothing() {
    let mut b = bench(8);
    b.app.run_action("fx.coller");
    assert!(b.app.status.starts_with("Aucun effet copié"), "{}", b.app.status);
    b.app.fx_copy_effect(0, 0);
    b.app.doc.storyboard_locked = true;
    let steps = step_names(&b).len();
    b.app.fx_paste(vec![1], false);
    assert!(stack_of(&b, 1).is_empty());
    assert_eq!(step_names(&b).len(), steps);
}

#[test]
fn the_clipboard_survives_a_new_document() {
    let mut b = bench(8);
    b.app.run_action("fx.copier_tout");
    let mut other = Document::with_frame(64, 36, 24);
    other.panels[0].layers = vec![Layer::new("Autre", LayerKind::Bitmap)];
    other.panels[0].length = 8;
    b.app.doc = other;
    b.app.cur_panel = 0;
    b.app.cur_layer = 0;
    b.app.layer_sel.clear();
    b.app.run_action("fx.coller");
    assert_eq!(kinds(&stack_of(&b, 0)), vec![EffectKind::GaussianBlur, EffectKind::Glow]);
}

#[test]
fn copy_chosen_takes_the_chosen_effect_or_the_only_one() {
    let mut b = bench(8);
    b.app.run_action("fx.copier");
    assert!(b.app.fx_stack_clipboard.is_none(), "deux effets, aucun choisi : on demande lequel");
    assert!(b.app.status.starts_with("Choisis d'abord"), "{}", b.app.status);
    b.app.select_fx(1);
    b.app.run_action("fx.copier");
    assert_eq!(kinds(&b.app.fx_stack_clipboard.clone().unwrap().effects), vec![EffectKind::Glow]);
    b.app.select_layer(1);
    b.app.layers_mut()[1].fx = vec![Effect::new(EffectKind::Noise)];
    b.app.deselect_fx();
    b.app.run_action("fx.copier");
    assert_eq!(kinds(&b.app.fx_stack_clipboard.clone().unwrap().effects), vec![EffectKind::Noise], "un seul effet : pas besoin de le choisir");
}

// ---------------------------------------------------------------------- focus et clavier

#[test]
fn with_the_effects_pane_in_focus_ctrl_c_and_ctrl_v_copy_and_paste_effects() {
    let mut b = bench(8);
    b.app.select_fx(0);
    b.app.take_fx_pane_focus();
    b.app.run_action("edition.copier");
    assert_eq!(kinds(&b.app.fx_stack_clipboard.clone().unwrap().effects), vec![EffectKind::GaussianBlur]);
    b.app.select_layer(1);
    b.app.take_fx_pane_focus();
    b.app.run_action("edition.coller");
    assert_eq!(stack_of(&b, 1), vec![blur()]);
}

#[test]
fn effect_keys_chosen_on_the_timeline_are_still_copied_by_ctrl_c() {
    let mut b = bench(8);
    b.app.select_fx(0);
    // Le dernier clic est une clé d'effet de la timeline : elle a la main.
    b.app.click_fx_marker(0, 0, 2, false, false);
    assert!(b.app.fx_keys_have_focus());
    assert!(!b.app.fx_pane_has_focus());
    b.app.run_action("edition.copier");
    assert!(b.app.fx_key_clipboard.is_some(), "Ctrl+C copie la clé d'effet");
    assert!(b.app.fx_stack_clipboard.is_none(), "et pas l'effet");
    // Ctrl+V dans la timeline colle les clés, même si un effet est aussi copié (avant les clés).
    let keys_before = stack_of(&b, 0)[0].keys.len();
    b.app.cur_frame = 4;
    b.app.run_action("edition.coller");
    assert_eq!(stack_of(&b, 0)[0].keys.len(), keys_before + 1, "la clé collée par Ctrl+V de la timeline");
    assert_eq!(stack_of(&b, 0).len(), 2, "aucun effet n'a été collé");
}

#[test]
fn copying_an_effect_does_not_disturb_the_effect_keys_clipboard() {
    let mut b = bench(8);
    b.app.fx_copy_effect(0, 0);
    b.app.select_fx(0);
    b.app.click_fx_marker(0, 0, 2, false, false);
    b.app.run_action("edition.copier");
    assert!(b.app.fx_key_clipboard.is_some() && b.app.fx_stack_clipboard.is_some(), "deux presse-papiers distincts");
    assert_eq!(b.app.fx_key_clipboard.as_ref().map(|c| c.keys.len()), Some(1));
    assert_eq!(b.app.fx_stack_clipboard.as_ref().map(|c| c.effects.len()), Some(1));
}

#[test]
fn creating_a_key_gives_the_hand_back_to_the_key_not_the_pane() {
    let mut b = bench(8);
    b.app.select_fx(0);
    b.app.take_fx_pane_focus();
    b.app.fx_create_key();
    assert!(!b.app.fx_pane_has_focus(), "la clé qu'on vient de créer a la main");
    assert!(b.app.fx_keys_have_focus());
    b.app.run_action("edition.copier");
    assert!(b.app.fx_key_clipboard.is_some());
    assert!(b.app.fx_stack_clipboard.is_none());
}

#[test]
fn the_pane_takes_the_hand_from_the_other_strips_and_a_click_elsewhere_gives_it_back() {
    let mut b = bench(8);
    b.app.images_focus = true;
    b.app.key_focus = true;
    b.app.panel_focus = true;
    b.app.take_fx_pane_focus();
    assert!(b.app.fx_pane_has_focus() && !b.app.images_focus && !b.app.key_focus && !b.app.panel_focus);
    // Un appui n'importe où ailleurs (la barre de menus) lui reprend la main.
    b.image(
        vec![
            egui::Event::PointerMoved(egui::pos2(300.0, 5.0)),
            egui::Event::PointerButton { pos: egui::pos2(300.0, 5.0), button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE },
        ],
        1.0 / 60.0,
    );
    assert!(!b.app.fx_pane_has_focus());
}

#[test]
fn without_effects_on_the_layer_the_pane_does_not_take_ctrl_c_from_the_other_copies() {
    let mut b = bench(8);
    b.app.select_layer(1);
    b.app.take_fx_pane_focus();
    b.app.run_action("edition.copier");
    assert!(b.app.fx_stack_clipboard.is_none(), "B n'a pas d'effet : Ctrl+C suit les règles d'avant");
}

#[test]
fn with_the_pane_in_focus_but_nothing_copied_ctrl_v_is_left_to_the_other_clipboards() {
    let mut b = bench(8);
    b.app.take_fx_pane_focus();
    b.app.select_layer(1);
    b.app.run_action("edition.coller");
    assert!(stack_of(&b, 1).is_empty());
}

#[test]
fn the_four_actions_are_registered_without_a_default_shortcut() {
    use crate::keymap::{action_info, DEFAULT_KEYMAP};
    for id in ["fx.copier", "fx.copier_tout", "fx.coller", "fx.remplacer"] {
        assert!(action_info(id).is_some_and(|a| a.implemented), "{id} n'est pas dans keymap::ACTIONS");
        assert!(!DEFAULT_KEYMAP.contains(id), "{id} n'a pas de raccourci par défaut");
    }
}

#[test]
fn the_command_palette_finds_the_copy_and_paste_commands() {
    use crate::keymap::ACTIONS;
    use crate::palette::rank;
    let commands: Vec<(&str, &str)> = ACTIONS.iter().filter(|a| a.implemented).map(|a| (a.id, a.label)).collect();
    for (word, id) in [("copier tous les effets", "fx.copier_tout"), ("remplacer les effets", "fx.remplacer"), ("coller les effets", "fx.coller")] {
        let hits = rank(&commands, word);
        let ids: Vec<&str> = hits.iter().map(|&i| commands[i].0).collect();
        assert!(ids.contains(&id), "« {word} » ne trouve pas {id} : {ids:?}");
    }
}

use eframe::egui;
