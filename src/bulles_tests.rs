//! Tests des bulles d'aide et des boutons icônes (12d) : la bulle attend une seconde, les
//! interrupteurs et les choix changent leur valeur au clic, un bouton grisé ne répond pas, et
//! chaque raccourci annoncé dans une bulle est une action du fichier de raccourcis.

use std::path::Path;

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Response, Ui};

use crate::bulles;
use crate::keymap;

#[test]
fn tooltips_wait_one_second() {
    let mut style = egui::Style::default();
    crate::theme::densifier(&mut style);
    assert_eq!(style.interaction.tooltip_delay, 1.0);
    assert_eq!(bulles::DELAI, 1.0);
}

/// Une image d'egui avec ces événements ; renvoie le rectangle du widget que `f` dessine.
fn frame(ctx: &egui::Context, time: &mut f64, events: Vec<Event>, f: &mut dyn FnMut(&mut Ui) -> Response) -> (Rect, bool) {
    *time += 1.0 / 60.0;
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))),
        time: Some(*time),
        events,
        ..Default::default()
    };
    // egui peut refaire une image (seconde passe, sans les événements) : le clic compte s'il a
    // été vu dans l'une des passes.
    let mut out = (Rect::NOTHING, false);
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let resp = f(ui);
            out = (resp.rect, out.1 || resp.clicked());
        });
    });
    out
}

/// Un contexte egui prêt : ses premières images sont passées, et un premier clic dans le vide
/// (egui ne voit pas le tout premier clic d'un contexte neuf).
fn context() -> (egui::Context, f64) {
    let ctx = egui::Context::default();
    let mut time = 0.0;
    let mut empty = |ui: &mut Ui| ui.allocate_rect(Rect::from_min_size(pos2(300.0, 200.0), vec2(20.0, 20.0)), egui::Sense::click());
    click(&ctx, &mut time, &mut empty);
    (ctx, time)
}

/// Clic au milieu du widget ; vrai si egui l'a vu comme un clic.
fn click(ctx: &egui::Context, time: &mut f64, f: &mut dyn FnMut(&mut Ui) -> Response) -> bool {
    // Une seconde entre deux clics : pas de double-clic.
    *time += 1.0;
    let (rect, _) = frame(ctx, time, Vec::new(), f);
    let p = rect.center();
    let button = |pressed| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    frame(ctx, time, vec![Event::PointerMoved(p)], f);
    frame(ctx, time, vec![button(true)], f);
    let clicked = frame(ctx, time, vec![button(false)], f).1;
    // La souris s'en va, comme entre deux gestes.
    frame(ctx, time, vec![Event::PointerGone], f);
    clicked
}

#[test]
fn a_switch_flips_its_value_on_click() {
    let (ctx, mut time) = context();
    let mut on = false;
    let mut changes = 0;
    let mut f = |ui: &mut Ui| {
        let resp = bulles::interrupteur(ui, &mut on, "x", "Nom", "Description", "");
        changes += usize::from(resp.changed());
        resp
    };
    assert!(click(&ctx, &mut time, &mut f));
    assert!(click(&ctx, &mut time, &mut f));
    assert!(click(&ctx, &mut time, &mut f));
    drop(f);
    assert!(on, "trois clics : allumé");
    assert_eq!(changes, 3, "chaque clic compte comme un changement (pour l'annulation)");
}

#[test]
fn a_choice_takes_its_value_and_keeps_it() {
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Vue {
        Dessin,
        Vignettes,
    }
    let (ctx, mut time) = context();
    let mut vue = Vue::Dessin;
    let mut changes = 0;
    let mut f = |ui: &mut Ui| {
        let resp = bulles::choix(ui, &mut vue, Vue::Vignettes, "v", "Vignettes", "", "");
        changes += usize::from(resp.changed());
        resp
    };
    click(&ctx, &mut time, &mut f);
    click(&ctx, &mut time, &mut f);
    drop(f);
    assert_eq!(vue, Vue::Vignettes, "un second clic ne revient pas en arrière");
    assert_eq!(changes, 1);
}

#[test]
fn a_greyed_button_does_not_answer() {
    let (ctx, mut time) = context();
    let mut f = |ui: &mut Ui| bulles::icone_si(ui, false, "x", "Nom", "", "");
    assert!(!click(&ctx, &mut time, &mut f));
    let mut f = |ui: &mut Ui| bulles::icone_si(ui, true, "x", "Nom", "", "");
    assert!(click(&ctx, &mut time, &mut f));
}

/// Les arguments d'un appel, jusqu'à sa parenthèse fermante (les parenthèses des textes ne
/// comptent pas).
fn call_args(src: &str) -> &str {
    let (mut depth, mut in_str, mut escaped) = (1, false, false);
    for (i, c) in src.char_indices() {
        match (in_str, c) {
            (true, _) if escaped => escaped = false,
            (true, '\\') => escaped = true,
            (true, '"') => in_str = false,
            (true, _) => {}
            (false, '"') => in_str = true,
            (false, '(') => depth += 1,
            (false, ')') => {
                depth -= 1;
                if depth == 0 {
                    return &src[..i];
                }
            }
            _ => {}
        }
    }
    src
}

/// Les textes entre guillemets.
fn literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut s = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '"' => break,
                c => s.push(c),
            }
        }
        out.push(s);
    }
    out
}

/// « famille.action » : minuscules, chiffres et soulignés, un seul point.
fn looks_like_action(s: &str) -> bool {
    let mut parts = s.split('.');
    let ok = |p: Option<&str>| p.is_some_and(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

#[test]
fn every_shortcut_announced_by_a_button_is_a_known_action() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let calls = ["bouton(", "bouton_si(", "bouton_etat(", "raccourci(", ".aide(", "command_button("];
    let (mut seen, mut unknown) = (0, Vec::new());
    for entry in std::fs::read_dir(dir).expect("dossier src") {
        let path = entry.expect("fichier").path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source lisible");
        for call in calls {
            for (i, _) in text.match_indices(call) {
                for lit in literals(call_args(&text[i + call.len()..])) {
                    if looks_like_action(&lit) {
                        seen += 1;
                        if keymap::action_info(&lit).is_none() {
                            unknown.push(format!("{name} : {lit}"));
                        }
                    }
                }
            }
        }
    }
    assert!(seen >= 40, "seulement {seen} actions trouvées : la recherche ne voit plus les boutons");
    assert!(unknown.is_empty(), "actions inconnues : {unknown:?}");
}

#[test]
fn every_tool_and_3d_tool_has_a_description() {
    for tool in crate::app::Tool::ALL {
        let d = tool.description();
        assert!(d.ends_with('.') && d.len() > 10, "{} : « {d} »", tool.label());
    }
    for t in crate::stage3dui::Nav3d::ALL {
        assert!(t.description().ends_with('.'), "{}", t.label());
    }
}
