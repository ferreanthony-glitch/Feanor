//! L'aide animée (partie 153), après la relecture : le ▶ des outils d'annotation sur une ligne, chaque animation d'une suite vue quand
//! elle commence, les avertissements rendus (courts, seulement quand ils disent vrai), la ligne d'état des calques d'annotation, les
//! contextes sans animation, le contraste d'une touche tenue, « Afficher les explications » à la seule prise d'outil, le registre qui ne
//! peint que ce qu'on voit, et ses textes.

use eframe::egui::{self, pos2, vec2, Event, RawInput, Rect, Shape};

use crate::aideanim::{self, couleurs_touche, Teintes};
use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};

/// Un écran sans fenêtre, `dt` secondes entre deux images ; rend les textes peints avec leur place.
struct Ecran {
    ctx: egui::Context,
    t: f64,
    dt: f64,
}

impl Ecran {
    fn new(dt: f64) -> Self {
        Self { ctx: egui::Context::default(), t: 0.0, dt }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: &mut dyn FnMut(&mut App, &egui::Context)) -> Vec<(String, Rect)> {
        self.t += self.dt;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 1400.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| contenu(app, ctx));
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize, contenu: &mut dyn FnMut(&mut App, &egui::Context)) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new(), contenu);
        }
        textes
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    textes.iter().any(|(t, _)| t.contains(morceau))
}

/// Le panneau Réglages, puis la fenêtre flottante de l'aide (partie 155 : c'est elle qui joue, plus le panneau).
fn reglages(app: &mut App, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| app.tool_options_panel(ui));
    app.aide_flottante_ui(ctx);
}

/// Une application sans fenêtre, `n` calques de la sorte `kind` ; l'aide joue d'elle-même, rien n'est encore vu.
fn app_aide(name: &str, kind: LayerKind, n: usize) -> App {
    let dir = std::env::temp_dir().join(format!("shot-aiderelu-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(24);
    for k in 0..n {
        let cel = doc.new_cel(kind);
        let mut l = Layer::new(&format!("Dessin {k}"), kind);
        l.frames = vec![Some(cel); 24];
        doc.panels[0].layers.push(l);
    }
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.confort.aide.auto = true;
    app.settings.explications_vues.clear();
    app.settings.afficher_explications = false;
    app
}

fn vue(app: &App, id: &str) -> bool {
    app.settings.explications_vues.iter().any(|v| v == id)
}

// ------------------------------------------------------------------------------------------------------ 1. le ▶ des annotations

#[test]
fn the_play_button_of_the_annotation_tools_stays_on_one_line() {
    let mut app = app_aide("annot", LayerKind::Vector, 1);
    app.espace = crate::workspace::Espace::Vignettes;
    app.board_view = crate::app::BoardView::Panels;
    app.vignettes.annot.outil = Some(crate::annotationsui::Outil::Stylo);
    app.settings.explications_vues = aideanim::toutes().map(|a| a.id.to_string()).collect();
    let mut e = Ecran::new(1.0 / 60.0);
    let textes = e.chauffer(&mut app, 3, &mut reglages);
    assert_eq!(app.aide_contexte(), "vignettes.annotation");
    let (_, r) = textes.iter().find(|(t, _)| t == egui_phosphor::regular::PLAY).cloned().expect("le bouton ▶");
    assert!(r.top() < 60.0, "le ▶ est en haut du panneau, sur sa ligne, pas au milieu de toute sa hauteur : {r:?}");
}

// ------------------------------------------------------------------------------------------------------ 2. vue quand elle commence

#[test]
fn each_animation_of_a_suite_is_seen_only_when_it_begins() {
    let mut app = app_aide("suite", LayerKind::Vector, 1);
    let mut e = Ecran::new(0.25);
    app.set_tool(Tool::Trajectoire);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("trajectoire.tracer"));
    assert!(vue(&app, "trajectoire.tracer"));
    assert!(!vue(&app, "trajectoire.prolonger") && !vue(&app, "trajectoire.tordre"), "les suivantes ne sont pas encore vues");
    // On change d'outil pendant la première : la suite s'arrête ; reprendre l'outil joue celles qui restent.
    app.set_tool(Tool::Brush);
    e.chauffer(&mut app, 2, &mut reglages);
    app.set_tool(Tool::Trajectoire);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("trajectoire.prolonger"), "la suivante joue à la reprise");
    // Elle n'est vue qu'à son début ; la troisième, à son tour.
    for _ in 0..40 {
        e.image(&mut app, Vec::new(), &mut reglages);
    }
    assert!(vue(&app, "trajectoire.tordre"));
}

// ------------------------------------------------------------------------------------------------------ 3. les avertissements

#[test]
fn hidden_or_locked_chosen_layers_are_said_to_be_left_aside() {
    let mut app = app_aide("cote", LayerKind::Vector, 3);
    let id = app.doc.panels[0].layers[1].id;
    app.layer_sel.push(id);
    app.set_tool(Tool::Transform);
    let mut e = Ecran::new(1.0 / 60.0);
    assert!(!contient(&e.chauffer(&mut app, 3, &mut reglages), "laissé"), "rien de caché : pas d'avertissement");
    app.doc.panels[0].layers[1].visible = false;
    assert_eq!(app.calques_laisses_de_cote(), 1);
    assert!(contient(&e.chauffer(&mut app, 3, &mut reglages), "1 calque caché ou verrouillé : laissé de côté."), "Transformer");
    app.set_tool(Tool::Select);
    assert!(contient(&e.chauffer(&mut app, 3, &mut reglages), "laissé de côté"), "la sélection de plusieurs calques");
    app.doc.panels[0].layers[1].visible = true;
    app.doc.panels[0].layers[1].locked = true;
    assert_eq!(app.calques_laisses_de_cote(), 1, "verrouillé aussi");
    app.layer_sel.clear();
    assert_eq!(app.calques_laisses_de_cote(), 0, "un seul calque choisi : rien à dire");
    let source = include_str!("deformmulti.rs");
    assert!(source.contains("self.avertir_laisses_de_cote(ui)"), "la déformation de plusieurs calques le dit aussi");
}

#[test]
fn the_preferences_warn_only_when_it_is_true() {
    let mut app = app_aide("prefs", LayerKind::Vector, 1);
    let mut e = Ecran::new(1.0 / 60.0);
    let mut decor = |a: &mut App, ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| a.decor_prefs_ui(ui));
    };
    assert!(!contient(&e.chauffer(&mut app, 2, &mut decor), "garde sa sorte"));
    app.doc.panels[0].layers[0].decor = true;
    assert!(contient(&e.chauffer(&mut app, 2, &mut decor), "1 plan a déjà son calque Décor : il garde sa sorte."));
    let mut tremble = |a: &mut App, ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| a.tremblement_defaut_pref_ui(ui));
    };
    assert!(!contient(&e.chauffer(&mut app, 2, &mut tremble), "son propre tremblement"));
    app.doc.panels[0].layers[0].fx.push(crate::fx::Effect::new(crate::fx::EffectKind::Boil));
    assert!(contient(&e.chauffer(&mut app, 2, &mut tremble), "1 calque a son propre tremblement : il le garde."));
}

#[test]
fn an_online_sound_service_recalls_its_licences() {
    use crate::genreglages::{Mode, Role};
    let mut app = app_aide("licences", LayerKind::Vector, 1);
    for r in Role::ALL {
        if app.settings.generation_son.mode(r) == Mode::EnLigne {
            app.settings.generation_son.definir(r, Mode::Local);
        }
    }
    let mut e = Ecran::new(1.0 / 60.0);
    let mut corps = |a: &mut App, ctx: &egui::Context| {
        egui::CentralPanel::default().show(ctx, |ui| a.generation_prefs_corps(ui));
    };
    assert!(!contient(&e.chauffer(&mut app, 2, &mut corps), "vérifie ses licences"), "aucun service en ligne : rien");
    app.settings.generation_son.definir(Role::Lecture, Mode::EnLigne);
    assert!(contient(&e.chauffer(&mut app, 2, &mut corps), "Service en ligne : vérifie ses licences"));
}

#[test]
fn the_export_of_key_images_counts_the_layers_that_make_no_key() {
    let mut app = app_aide("cles", LayerKind::Vector, 3);
    assert_eq!(crate::exportui::calques_sans_cle(&app.doc, &[0]), 0);
    app.doc.panels[0].layers[0].visible = false;
    app.doc.panels[0].layers[1].annotation = true;
    app.doc.panels[0].layers[1].visible = false;
    assert_eq!(crate::exportui::calques_sans_cle(&app.doc, &[0]), 1, "un calque caché ; l'annotation suit sa case");
    assert_eq!(crate::exportui::calques_sans_cle(&app.doc, &[]), 0);
    assert!(include_str!("exportui.rs").contains("d'effet ou vidéo : pas de clé."));
}

// ------------------------------------------------------------------------------------------------------ 6. le contraste

#[test]
fn a_held_key_is_readable_in_both_themes() {
    for claire in [false, true] {
        let t = Teintes::pour(claire);
        for tenue in [false, true] {
            let (fond, encre, _) = couleurs_touche(&t, tenue);
            let c = crate::theme::contraste(fond, encre);
            assert!(c >= 4.5, "thème {} touche {} : {c:.2}:1", if claire { "clair" } else { "sombre" }, if tenue { "tenue" } else { "au repos" });
        }
    }
}

// ------------------------------------------------------------------------------------------------------ 7. la prise d'outil

#[test]
fn showing_the_explanations_replays_only_when_the_tool_changes() {
    let mut app = app_aide("prise", LayerKind::Vector, 1);
    let cel = app.doc.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new("Bitmap", LayerKind::Bitmap);
    l.frames = vec![Some(cel); 24];
    app.doc.panels[0].layers.push(l);
    app.settings.afficher_explications = true;
    let mut e = Ecran::new(0.25);
    app.set_tool(Tool::Select);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("selection.boite"));
    for _ in 0..80 {
        e.image(&mut app, Vec::new(), &mut reglages);
    }
    assert!(app.confort.aide.en_cours().is_none());
    // Le même outil, sur un calque bitmap : le lasso, jamais vu, joue ; pas la boîte déjà vue.
    app.cur_layer = 1;
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("selection.lasso"));
    for _ in 0..80 {
        e.image(&mut app, Vec::new(), &mut reglages);
    }
    // De retour sur le calque vectoriel, le même outil : rien ne rejoue.
    app.cur_layer = 0;
    e.chauffer(&mut app, 2, &mut reglages);
    assert!(app.confort.aide.en_cours().is_none(), "pas de prise d'outil : rien ne rejoue");
    // Un autre outil, puis la Sélection : elle rejoue.
    app.set_tool(Tool::Brush);
    e.chauffer(&mut app, 2, &mut reglages);
    app.set_tool(Tool::Select);
    e.chauffer(&mut app, 2, &mut reglages);
    assert_eq!(app.confort.aide.en_cours(), Some("selection.boite"));
}

#[test]
fn the_aplat_of_a_single_layer_is_not_a_selection_of_several_layers() {
    let mut app = app_aide("aplat", LayerKind::Bitmap, 1);
    app.toggle_fond(0);
    app.aplat_cible = Some(app.doc.panels[0].layers[0].id);
    app.set_tool(Tool::Select);
    assert!(app.aplat_en_cible());
    assert_eq!(app.aide_contexte(), "selection.bitmap");
}

// ------------------------------------------------------------------------------------------------------ 8. le registre

#[test]
fn the_register_paints_only_the_thumbnails_in_view() {
    let mut app = app_aide("visibles", LayerKind::Vector, 1);
    app.run_action("aide.registre");
    let mut e = Ecran::new(1.0 / 60.0);
    let mut fenetre = |a: &mut App, ctx: &egui::Context| a.aide_registre_fenetre(ctx);
    e.chauffer(&mut app, 2, &mut fenetre);
    let avant = crate::aideregistre::PEINTES.with(|n| n.get());
    e.image(&mut app, Vec::new(), &mut fenetre);
    let peintes = crate::aideregistre::PEINTES.with(|n| n.get()) - avant;
    let toutes = aideanim::toutes().count();
    assert!(peintes > 0 && peintes < toutes, "{peintes} vignettes peintes sur {toutes}");
}

// ------------------------------------------------------------------------------------------------------ 9 à 12. les textes

#[test]
fn the_register_texts_do_not_repeat_themselves() {
    let prolonger = aideanim::anim("trajectoire.prolonger").unwrap();
    assert!(prolonger.texte.contains("Maj + un trait depuis un bout") && !prolonger.texte.contains("Tirer sur le chemin"), "{}", prolonger.texte);
    for a in aideanim::toutes().filter(|a| a.texte.contains("Plusieurs cartes choisies") || a.titre.contains("vue")) {
        assert!(a.titre.contains("cartes") || a.texte.to_lowercase().contains("vue"), "titre et texte vont ensemble : {} / {}", a.titre, a.texte);
    }
    for t in crate::aidetextes::tous() {
        if !t.sujet.is_empty() {
            assert!(!t.texte.starts_with(&format!("{} :", t.sujet)), "« {} » répété : {}", t.sujet, t.texte);
        }
    }
}

#[test]
fn the_register_names_no_operating_system() {
    let tous = aideanim::toutes().map(|a| format!("{} {}", a.titre, a.texte)).chain(crate::aidetextes::tous().map(|t| format!("{} {} {}", t.fenetre, t.sujet, t.texte)));
    for texte in tous {
        assert!(!texte.contains("Windows"), "{texte}");
    }
    assert!(aideanim::anim("dessin.pression").unwrap().texte.contains("le pilote du stylet"));
    assert!(crate::aidefenetres::FENETRES.iter().any(|t| t.texte.contains("le nom de session de l'ordinateur")));
}

#[test]
fn the_camera_mode_key_and_the_space_choice_read_well() {
    assert!(aideanim::anim("vignettes.camera").unwrap().texte.contains("la touche du mode caméra{vignettes.camera} tenue : pareil"));
    assert!(aideanim::anim("trajectoire.espace").unwrap().texte.contains("Ctrl+clic choisit une carte ou la caméra, seule ; Maj+Ctrl+clic en ajoute une, ou la lâche."));
}
