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

/// Les couleurs avec lesquelles les textes d'une image sont dessinés (celle de chaque morceau de
/// texte : la sienne si elle est posée, sinon celle que le widget donne au dessin).
fn text_colors(output: &egui::FullOutput) -> Vec<egui::Color32> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => {
                for section in &t.galley.job.sections {
                    let own = if section.format.color == egui::Color32::PLACEHOLDER { t.fallback_color } else { section.format.color };
                    out.push(t.override_text_color.unwrap_or(own));
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

#[test]
fn a_toggled_on_icon_button_is_drawn_in_the_colour_that_reads_on_the_accent_in_every_theme() {
    use crate::theme::{self, ChoixTheme, ReglageTheme};
    for choix in ChoixTheme::TOUS {
        theme::installer(&ReglageTheme { choix, ..ReglageTheme::default() });
        let ink = theme::sur_accent();
        let ctx = egui::Context::default();
        theme::appliquer(&ctx);
        let (mut on, mut pick) = (true, 1);
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))), time: Some(0.0), ..Default::default() };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                // Les aides de bulles.rs.
                bulles::etat(ui, true, "A", "", "", "");
                bulles::interrupteur(ui, &mut on, "B", "", "", "");
                bulles::choix(ui, &mut pick, 1, "C", "", "", "");
                // Un `SelectableLabel` d'egui (barre de commandes, d'outils, des panneaux).
                ui.add(egui::SelectableLabel::new(true, egui::RichText::new("D").size(17.0)));
                // Un bouton fait à la main avec `.selected(on)`.
                ui.add(egui::Button::new(bulles::symbole_etat("E", true)).selected(true));
                // Un nom en gras dans un choix (le nom d'un effet choisi, les modes).
                ui.add(egui::SelectableLabel::new(true, bulles::encre_etat(egui::RichText::new("F").strong(), true)));
            });
        });
        let colors = text_colors(&output);
        assert_eq!(colors.len(), 6, "{choix:?} : un texte par bouton");
        for c in colors {
            assert_eq!(c, ink, "{choix:?} : l'icône d'un bouton allumé doit avoir la couleur de `sur_accent`");
        }
    }
    theme::installer(&ReglageTheme::default());
}

#[test]
fn the_ink_of_a_toggle_only_changes_when_it_is_on() {
    use crate::theme;
    let off = bulles::encre_etat(egui::RichText::new("x"), false);
    let on = bulles::encre_etat(egui::RichText::new("x"), true);
    assert_eq!(off, egui::RichText::new("x"), "éteint : l'encre du widget reste");
    assert_ne!(on, egui::RichText::new("x"), "allumé : l'encre est posée");
    assert_eq!(on, egui::RichText::new("x").color(theme::sur_accent()));
}

#[test]
fn no_hand_made_selected_button_forgets_its_ink() {
    // Un `Button::…selected(…)` dessine son texte avec la couleur du survol, pas celle de la
    // sélection : chaque appel doit passer son texte par `symbole_etat` / `encre_etat`.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(dir).expect("dossier src") {
        let path = entry.expect("fichier").path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !name.ends_with(".rs") || name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source lisible");
        let lines: Vec<&str> = text.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            // Le texte est passé par l'aide sur la ligne, ou sur celle d'avant (`let text = …`).
            let ink = |l: &str| l.contains("symbole_etat") || l.contains("encre_etat");
            if line.contains(".selected(") && line.contains("Button::new(") && !ink(line) && !(n > 0 && ink(lines[n - 1])) {
                bad.push(format!("{name}:{}", n + 1));
            }
            // Un texte « fort » ou coloré dans un choix d'egui garde sa couleur même choisi : blanc sur
            // le gris clair de l'accent (le nom d'un effet choisi, les onglets, les modes).
            let selectable = line.contains("SelectableLabel::new(") || line.contains("selectable_label(") || line.contains("selectable_value(");
            if selectable && (line.contains(".strong()") || line.contains(".color(")) && !ink(line) {
                bad.push(format!("{name}:{} (choix avec un texte qui force sa couleur)", n + 1));
            }
        }
    }
    // (Seul le choix d'un groupe de calques garde la couleur de son groupe, exprès.)
    bad.retain(|b| !b.starts_with("layerheads.rs"));
    assert!(bad.is_empty(), "boutons « selected » sans l'encre de l'accent : {bad:?}");
}

#[test]
fn every_tool_and_3d_tool_has_a_description() {
    for tool in crate::app::Tool::ALL {
        let d = tool.description();
        assert!(d.ends_with('.') && d.len() > 10, "{} : « {d} »", tool.label());
    }
    for mode in crate::scene3dgizmo::Mode::ALL {
        assert!(crate::scene3dui::mode_description(mode).ends_with('.'), "{}", mode.label());
    }
}
