//! L'aide animée (partie 153) : la première prise d'un outil joue son animation (dans la fenêtre flottante depuis la partie 155,
//! `aideflottante_tests.rs`), la seconde non (sauf avec
//! Aide > « Afficher les explications ») ; ▶ la rejoue ; à l'arrêt, plus de repaint ; le registre liste tout ; plus aucun texte gris
//! explicatif dans les réglages des outils ; les vieux réglages se relisent ; chaque animation se peint sans paniquer.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape, ViewportId};

use crate::aideanim::{self, avec_touches, par_les_marques, Teintes};
use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::settings::Settings;

/// Un écran sans fenêtre où l'on peint le panneau Réglages (ou une autre interface) image après image, `dt` secondes entre deux.
struct Ecran {
    ctx: egui::Context,
    t: f64,
    dt: f64,
    /// Le délai de repaint demandé par la dernière image.
    reveil: std::time::Duration,
}

impl Ecran {
    fn new(dt: f64) -> Self {
        Self { ctx: egui::Context::default(), t: 0.0, dt, reveil: std::time::Duration::MAX }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &egui::Context)) -> Vec<(String, Rect, f32, egui::Color32)> {
        self.t += self.dt;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 1400.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| contenu(app, ctx));
        self.reveil = out.viewport_output.get(&ViewportId::ROOT).map_or(std::time::Duration::MAX, |v| v.repaint_delay);
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize, contenu: &mut dyn FnMut(&mut App, &egui::Context)) -> Vec<(String, Rect, f32, egui::Color32)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new(), contenu);
        }
        textes
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect, f32, egui::Color32)>) {
    match shape {
        Shape::Text(t) => {
            let format = t.galley.job.sections.first().map(|s| (s.format.font_id.size, s.format.color)).unwrap_or((0.0, egui::Color32::TRANSPARENT));
            out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()), format.0, format.1));
        }
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

/// Le panneau Réglages, puis la fenêtre flottante de l'aide (partie 155 : c'est elle qui joue, plus le panneau).
fn reglages(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
    app.aide_flottante_ui(ctx);
}

/// Une application sans fenêtre, un seul calque de la sorte `kind` ; l'aide joue d'elle-même (comme l'application lancée par
/// l'utilisateur), rien n'est encore vu.
fn app_aide(name: &str, kind: LayerKind) -> App {
    let dir = std::env::temp_dir().join(format!("shot-aideanim-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
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
    app.confort.aide.auto = true;
    app.settings.explications_vues.clear();
    app.settings.afficher_explications = false;
    app
}

/// Assez d'images (de 0,25 s) pour que toute lecture en cours soit finie et repliée.
fn jusqu_au_bout(e: &mut Ecran, app: &mut App) {
    for _ in 0..200 {
        e.image(app, Vec::new(), &mut reglages);
        if !app.confort.aide.joue() {
            break;
        }
    }
    e.chauffer(app, 2, &mut reglages);
}

// ------------------------------------------------------------------------------------------------------ le joueur

#[test]
fn the_first_take_of_a_tool_plays_its_animation_and_the_second_does_not() {
    let mut app = app_aide("premiere", LayerKind::Vector);
    let mut e = Ecran::new(0.25);
    app.set_tool(Tool::Trajectoire);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("trajectoire.tracer"), "première prise : l'animation joue");
    assert!(e.reveil.is_zero(), "pendant qu'elle joue, l'écran est repeint");
    assert!(app.settings.explications_vues.iter().any(|v| v == "trajectoire.tracer"), "elle est retenue comme vue");
    assert!(app.settings_a_ecrire, "les réglages sont à écrire");
    jusqu_au_bout(&mut e, &mut app);
    assert!(!app.confort.aide.joue(), "puis elle s'efface");
    assert_eq!(e.reveil, std::time::Duration::MAX, "à l'arrêt, plus aucun repaint");
    // Un autre outil, puis de nouveau la Trajectoire : rien.
    app.set_tool(Tool::Brush);
    e.chauffer(&mut app, 2, &mut reglages);
    app.set_tool(Tool::Trajectoire);
    e.chauffer(&mut app, 2, &mut reglages);
    assert!(!app.confort.aide.joue(), "seconde prise : rien");
    // Aide > « Afficher les explications » : elle rejoue à chaque prise.
    app.run_action("aide.explications");
    assert!(app.settings.afficher_explications);
    app.set_tool(Tool::Brush);
    e.chauffer(&mut app, 2, &mut reglages);
    app.set_tool(Tool::Trajectoire);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("trajectoire.tracer"), "avec la case cochée, elle rejoue");
}

#[test]
fn an_animation_of_another_tool_stops_when_the_tool_changes() {
    let mut app = app_aide("change", LayerKind::Vector);
    let mut e = Ecran::new(0.25);
    app.set_tool(Tool::Gradient);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("degrade.axe"));
    app.settings.explications_vues = aideanim::toutes().map(|a| a.id.to_string()).collect();
    app.set_tool(Tool::Picker);
    e.chauffer(&mut app, 2, &mut reglages);
    assert!(!app.confort.aide.joue(), "l'animation du dégradé ne continue pas sous la pipette");
}

#[test]
fn the_play_button_replays_the_animation() {
    let mut app = app_aide("rejouer", LayerKind::Vector);
    app.settings.explications_vues = aideanim::toutes().map(|a| a.id.to_string()).collect();
    let mut e = Ecran::new(1.0 / 60.0);
    app.set_tool(Tool::Camera);
    let textes = e.chauffer(&mut app, 3, &mut reglages);
    assert!(!app.confort.aide.joue(), "déjà vue : rien ne joue");
    let play = egui_phosphor::regular::PLAY;
    let (_, r, _, _) = textes.iter().find(|(t, ..)| t == play).cloned().unwrap_or_else(|| panic!("pas de bouton ▶ : {:?}", textes.iter().map(|t| &t.0).collect::<Vec<_>>()));
    let p = r.center();
    e.image(&mut app, vec![Event::PointerMoved(p)], &mut reglages);
    e.image(&mut app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], &mut reglages);
    e.image(&mut app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], &mut reglages);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("camera.cadrer"), "▶ la rejoue");
}

#[test]
fn nothing_plays_by_itself_in_the_tests_nor_without_animation() {
    // L'application des essais (comme `perf_tests::banc`) ne joue rien d'elle-même : les essais de repos restent au repos.
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    assert!(!app.confort.aide.auto);
}

// ------------------------------------------------------------------------------------------------------ les réglages

#[test]
fn old_settings_files_are_read_and_nothing_is_written_by_default() {
    let ecrit = toml::to_string(&Settings::default()).unwrap();
    assert!(!ecrit.contains("explications_vues") && !ecrit.contains("afficher_explications"), "{ecrit}");
    let ancien: Settings = toml::from_str("fonctions_experimentales = true\ncopie_secours_minutes = 3\n").expect("des réglages d'avant se relisent");
    assert!(ancien.explications_vues.is_empty() && !ancien.afficher_explications && ancien.experimental);
    let mut s = Settings::default();
    s.explications_vues = vec!["trajectoire.tracer".into(), "camera.cadrer".into()];
    s.afficher_explications = true;
    let relu: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(relu.explications_vues, s.explications_vues);
    assert!(relu.afficher_explications);
}

// ------------------------------------------------------------------------------------------------------ le catalogue

#[test]
fn every_animation_draws_at_the_start_the_middle_and_the_end_in_both_themes() {
    let ctx = egui::Context::default();
    let mut ids = std::collections::BTreeSet::new();
    for a in aideanim::toutes() {
        assert!(ids.insert(a.id), "identifiant en double : {}", a.id);
        assert!((3.5..=7.0).contains(&a.duree), "{} : {} s", a.id, a.duree);
        assert!(!a.titre.is_empty() && !a.texte.is_empty() && !a.contextes.is_empty(), "{}", a.id);
        for claire in [false, true] {
            let teintes = Teintes::pour(claire);
            for t in [0.0, a.duree / 2.0, a.duree] {
                let out = ctx.run(RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let r = Rect::from_min_size(pos2(10.0, 10.0), vec2(240.0, 150.0));
                        aideanim::peindre(ui.painter(), r, a, t, &teintes);
                    });
                });
                assert!(out.shapes.len() > 3, "{} à {t} s : rien de peint", a.id);
            }
        }
    }
}

#[test]
fn the_colours_are_those_of_the_brand_and_nothing_is_red() {
    for claire in [false, true] {
        let t = Teintes::pour(claire);
        for c in [t.fond, t.cadre, t.encre, t.douce, t.forte, t.jade] {
            assert!(!(c.r() > 150 && c.g() < 110 && c.b() < 110), "rouge : {c:?}");
        }
    }
    assert_eq!(Teintes::pour(false).fond, egui::Color32::from_rgb(0x2C, 0x2C, 0x2E));
    assert_eq!(Teintes::pour(false).jade, egui::Color32::from_rgb(0x2F, 0xD4, 0xC0));
}

#[test]
fn equal_time_between_marks_slows_down_where_they_are_close() {
    let marques = [0.3, 0.4, 0.5];
    assert_eq!(par_les_marques(0.0, &marques), 0.0);
    assert_eq!(par_les_marques(1.0, &marques), 1.0);
    assert!((par_les_marques(0.25, &marques) - 0.3).abs() < 1e-5);
    assert!((par_les_marques(0.5, &marques) - 0.4).abs() < 1e-5);
}

#[test]
fn the_shortcuts_are_read_in_the_texts() {
    let r = |a: &str| if a == "outil.selection" { "S".to_string() } else { String::new() };
    assert_eq!(avec_touches("l'outil Sélection{outil.selection}, même texturé", r), "l'outil Sélection (S), même texturé");
    assert_eq!(avec_touches("rien{outil.inconnu}.", r), "rien.");
    assert_eq!(avec_touches("{ pas une commande }", r), "{ pas une commande }");
    // Les champs d'un modèle de nom (sans point) restent tels quels.
    assert_eq!(avec_touches("{projet} {plan:03}", r), "{projet} {plan:03}");
}

// ------------------------------------------------------------------------------------------------------ les fenêtres

/// Les textes des Préférences et des fenêtres : ils ont quitté leur fichier et sont dans le registre (`aidefenetres.rs`).
#[test]
fn the_window_texts_left_their_window_for_the_register() {
    let sources = [
        include_str!("prefsui.rs"),
        include_str!("genprefsui.rs"),
        include_str!("vignettesprefs.rs"),
        include_str!("exportui.rs"),
        include_str!("scenarioui.rs"),
        include_str!("cooperationui.rs"),
    ];
    let mut restent = Vec::new();
    for t in crate::aidefenetres::FENETRES.iter().filter(|t| t.fenetre != crate::aidetextes::EXPORTS) {
        // (Le libellé tel qu'il s'écrivait dans le code, les barres obliques inverses doublées ; une info-bulle qui dit la même chose
        // reste.)
        let ecrit = format!("RichText::new(\"{}\")", t.texte.replace('\\', "\\\\"));
        if sources.iter().any(|s| s.contains(&ecrit)) {
            restent.push(t.texte);
        }
        assert!(crate::aideregistre::fenetres().contains(&t.fenetre));
    }
    assert!(restent.is_empty(), "encore dans leur fenêtre : {restent:#?}");
    // La fenêtre « Exporter » ne répète plus sous chaque bouton ce que dit son info-bulle.
    assert!(!include_str!("prefsui.rs").contains("ui.label(RichText::new(*hint).small().weak())"));
}

#[test]
fn the_texts_name_no_commercial_software() {
    let tous = aideanim::toutes().map(|a| format!("{} {}", a.titre, a.texte)).chain(crate::aidetextes::tous().map(|t| format!("{} {} {}", t.fenetre, t.sujet, t.texte)));
    for texte in tous {
        for interdit in ["Storyboard Pro", "TVPaint", "Photoshop", "Procreate", "EbSynth", "Toon Boom", "Blender", "Krita"] {
            assert!(!texte.contains(interdit), "{interdit} : {texte}");
        }
    }
}

// ------------------------------------------------------------------------------------------------------ le registre

#[test]
fn the_register_lists_every_animation_and_every_text() {
    let mut app = app_aide("registre", LayerKind::Vector);
    app.run_action("aide.registre");
    assert!(app.confort.aide.registre);
    let rubriques = crate::aideregistre::rubriques();
    for a in aideanim::toutes() {
        assert!(rubriques.contains(&a.rubrique), "{}", a.id);
    }
    let fenetres = crate::aideregistre::fenetres();
    for t in crate::aidetextes::tous() {
        assert!(fenetres.contains(&t.fenetre) && !t.texte.is_empty());
    }
    // À l'écran : la fenêtre, chaque rubrique, et les titres et les textes qui y tiennent (la suite est sous le défilement).
    let mut e = Ecran::new(1.0 / 60.0);
    let mut fenetre = |app: &mut App, ctx: &egui::Context| app.aide_registre_fenetre(ctx);
    let textes = e.chauffer(&mut app, 4, &mut fenetre);
    let tous: Vec<&str> = textes.iter().map(|t| t.0.as_str()).collect();
    assert!(tous.contains(&"Registre des explications"), "{tous:?}");
    let premiere = aideanim::toutes().next().unwrap();
    assert!(tous.contains(&premiere.rubrique) && tous.contains(&premiere.titre), "{tous:?}");
    assert!(tous.iter().any(|t| t.starts_with("Trace le chemin du calque")), "le texte accompagne l'animation");
    // Rejouer une animation du registre, puis le repos.
    app.confort.aide.registre_joue = Some((premiere.id, e.t));
    e.chauffer(&mut app, 2, &mut fenetre);
    assert!(e.reveil.is_zero(), "pendant qu'elle rejoue, l'écran est repeint");
    e.dt = 0.5;
    e.chauffer(&mut app, 20, &mut fenetre);
    assert!(app.confort.aide.registre_joue.is_none());
    assert_eq!(e.reveil, std::time::Duration::MAX, "puis plus rien");
}

#[test]
fn the_help_menu_has_the_box_and_the_register_and_the_commands_exist() {
    for id in ["aide.explications", "aide.registre"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    let source = include_str!("panels.rs");
    let aide = &source[source.find("ui.menu_button(\"Aide\"").unwrap()..];
    assert!(aide[..aide.find("aide.a_propos").unwrap()].contains("self.aide_menu_ui(ui)"));
    let menu = include_str!("aideanim.rs");
    assert!(menu.contains("\"Afficher les explications\"") && menu.contains("\"Registre des explications…\""));
}

// ------------------------------------------------------------------------------------------------------ plus de texte gris

/// Les outils dont les réglages n'ont plus aucun texte gris explicatif (petit et pâle, de plus de `LONG` caractères).
/// (Avec la sorte de calque visée ; `None` : les deux.) Tous les outils.
const SANS_TEXTE_GRIS: &[(Tool, Option<LayerKind>)] = &[
    (Tool::Brush, None),
    (Tool::Mixer, None),
    (Tool::Pencil, None),
    (Tool::Eraser, None),
    (Tool::Cutter, None),
    (Tool::Fill, None),
    (Tool::Gradient, None),
    (Tool::Line, None),
    (Tool::Rect, None),
    (Tool::Ellipse, None),
    (Tool::Polyline, None),
    (Tool::Text, None),
    (Tool::Select, None),
    (Tool::SelectColor, None),
    (Tool::Picker, None),
    (Tool::Pan, None),
    (Tool::Camera, None),
    (Tool::Transform, None),
    (Tool::Perspective, None),
    (Tool::Grab, None),
    (Tool::Pull, None),
    (Tool::Trajectoire, None),
];
const LONG: usize = 60;

#[test]
fn every_tool_is_in_the_list_of_tools_without_grey_text() {
    for t in Tool::ALL {
        assert!(SANS_TEXTE_GRIS.iter().any(|(o, _)| *o == t), "{t:?}");
    }
}

#[test]
fn every_tool_has_its_animation() {
    for contexte in ["trajectoire", "trajectoire.espace", "selection", "selection.bitmap", "selection.couleur", "selection.multi", "pinceau", "crayon", "melangeur", "gomme", "cutter", "remplissage", "degrade", "formes", "texte", "deformer", "sculpter", "pipette", "main", "camera", "espace3d", "vignettes", "vignettes.camera", "vignettes.annotation", "transformer", "gomme.bitmap"] {
        assert!(!aideanim::du_contexte(contexte).is_empty(), "rien pour « {contexte} »");
    }
}

#[test]
fn the_first_opening_of_the_vignettes_plays_their_animations() {
    let mut app = app_aide("vignettes", LayerKind::Vector);
    app.espace = crate::workspace::Espace::Vignettes;
    let mut e = Ecran::new(0.25);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.aide_contexte(), "vignettes");
    assert_eq!(app.confort.aide.en_cours(), Some("vignettes.coupe"), "l'espace Vignettes ouvert la première fois : la coupe et la continuité");
    jusqu_au_bout(&mut e, &mut app);
    for id in ["vignettes.coupe", "vignettes.trait", "vignettes.pont"] {
        assert!(app.settings.explications_vues.iter().any(|v| v == id), "{id} a joué");
    }
    // L'outil Caméra sur la planche : les caméras dans les vignettes.
    app.set_tool(Tool::Camera);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("vignettes.camera"));
}

#[test]
fn no_grey_explanation_is_left_in_the_tool_settings() {
    let mut fautes = Vec::new();
    for kind in [LayerKind::Vector, LayerKind::Bitmap] {
        let mut app = app_aide("gris", kind);
        app.confort.aide.auto = false;
        app.pinceaux.reglages_ouverts = true;
        let mut e = Ecran::new(1.0 / 60.0);
        e.chauffer(&mut app, 1, &mut reglages);
        let (petit, pale) = e.ctx.style().text_styles.get(&egui::TextStyle::Small).map(|f| f.size).zip(Some(e.ctx.style().visuals.weak_text_color())).unwrap();
        for &(tool, sorte) in SANS_TEXTE_GRIS {
            if sorte.is_some_and(|s| s != kind) {
                continue;
            }
            // (« Plus de réglages » déplié : c'est là qu'étaient les modes d'emploi.)
            app.pinceaux.regler_plus(tool.action(), true);
            app.set_tool(tool);
            for (t, _, taille, couleur) in e.chauffer(&mut app, 3, &mut reglages) {
                if (taille - petit).abs() < 0.01 && couleur == pale && t.chars().count() > LONG {
                    fautes.push(format!("{tool:?} ({kind:?}) : {t}"));
                }
            }
        }
    }
    assert!(fautes.is_empty(), "des textes gris restent dans Réglages :\n{}", fautes.join("\n"));
}
