//! Essais des trois suites des calques d'annotation (Anthony, 2026-10-24) : la case « Avec les annotations » des fenêtres d'export (la planche,
//! les images, la vidéo, le storyboard PDF), Suppr qui efface les annotations choisies au lasso, et le glissé des calques dans la liste du
//! panneau Planche.

use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui::{pos2, vec2, Event, Key, Modifiers, Pos2};

use crate::annotations_calques_tests::{clic, entourer_trait, lasso, poser, texte_en};
use crate::calqueannot_tests::{banc_dessin, trait_au_milieu};
use crate::export::{ExportJob, ExportSettings, Format};
use crate::perf_tests::Banc;
use crate::vignettes_tests::{dernier_nom, pas_d_annulation, quatre_plans};

// ------------------------------------------------------------------------------------ « Avec les annotations »

fn attendre(job: &mut ExportJob, b: &Banc) {
    let t = Instant::now();
    while job.outcome.is_none() {
        job.step(&b.app.doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60), "export trop long");
    }
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
}

/// Un écran sans fenêtre pour les fenêtres d'export (`App::export_windows`) : on y peint la fenêtre image après image et on lit les textes
/// peints, avec leur place ; les clics sont des événements du pointeur (comme `notes_calques_tests`).
struct Fenetre {
    ctx: eframe::egui::Context,
    t: f64,
}

impl Fenetre {
    fn new() -> Self {
        Self { ctx: eframe::egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut crate::app::App, events: Vec<Event>) -> Vec<(String, Pos2)> {
        fn collecter(s: &eframe::egui::Shape, out: &mut Vec<(String, Pos2)>) {
            match s {
                eframe::egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                eframe::egui::Shape::Text(x) => out.push((x.galley.job.text.clone(), x.pos + x.galley.size() * 0.5)),
                _ => {}
            }
        }
        self.t += 1.0 / 60.0;
        let input = eframe::egui::RawInput { screen_rect: Some(eframe::egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 900.0))), time: Some(self.t), events, ..Default::default() };
        let out = self.ctx.run(input, |ctx| app.export_windows(ctx));
        let mut textes = Vec::new();
        out.shapes.iter().for_each(|s| collecter(&s.shape, &mut textes));
        textes
    }

    fn chauffer(&mut self, app: &mut crate::app::App, n: usize) -> Vec<(String, Pos2)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new());
        }
        textes
    }

    fn cliquer(&mut self, app: &mut crate::app::App, p: Pos2) {
        let bouton = |pressed| Event::PointerButton { pos: p, button: eframe::egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        // (Une fenêtre qui vient de s'ouvrir s'installe d'abord : un clic tout de suite ne la touche pas.)
        self.chauffer(app, 4);
        self.image(app, vec![Event::PointerMoved(p)]);
        self.chauffer(app, 2);
        self.image(app, vec![bouton(true)]);
        self.image(app, vec![bouton(false)]);
        self.chauffer(app, 3);
    }
}

/// Le centre du premier texte égal à `t`.
fn trouver(textes: &[(String, Pos2)], t: &str) -> Option<Pos2> {
    textes.iter().find(|(x, _)| x == t).map(|(_, p)| *p)
}

fn dossier(nom: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shot-test-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Des pixels d'un rouge franc (le trait d'annotation du plan) ou d'un vert franc (celui de la planche) : combien.
fn compte(png: &Path, teinte: fn(&[u8; 4]) -> bool) -> usize {
    image::open(png).expect("l'image est écrite").to_rgba8().pixels().filter(|p| teinte(&p.0)).count()
}

fn rouge(p: &[u8; 4]) -> bool {
    p[0] > 200 && p[1] < 70 && p[2] < 70
}

fn vert(p: &[u8; 4]) -> bool {
    p[1] > 200 && p[0] < 70 && p[2] < 70
}

/// Un plan avec un trait rouge dans un calque d'annotation du plan, et la vue Dessin.
fn plan_annote() -> Banc {
    let mut b = banc_dessin();
    b.app.run_action("calque.nouveau_annotation");
    b.chauffer(2);
    b.app.color = [255, 0, 0];
    trait_au_milieu(&mut b, 0.0);
    assert!(b.app.layers()[b.app.cur_layer].annotation);
    b
}

#[test]
fn the_export_check_box_is_off_by_default_and_the_choice_is_kept_by_the_workstation() {
    assert!(!ExportSettings::default().annotations, "décochée d'office");
    assert!(!crate::settings::Settings::default().export_annotations);
    // Retenu avec les réglages de l'application ; un fichier d'avant n'a pas la clé : décochée.
    let mut s = crate::settings::Settings::default();
    s.export_annotations = true;
    let relus: crate::settings::Settings = toml::from_str(&toml::to_string(&s).expect("toml")).expect("relus");
    assert!(relus.export_annotations);
    let ancien: crate::settings::Settings = toml::from_str("").expect("un fichier d'avant");
    assert!(!ancien.export_annotations);
    assert!(!toml::to_string(&crate::settings::Settings::default()).unwrap().contains("export_avec_annotations"), "rien d'écrit tant qu'elle est décochée");
    // La fenêtre d'export : la case est là (images, film, GIF, PDF), pas pour le son seul ; cocher la retient ; une nouvelle fenêtre s'ouvre cochée.
    let mut b = plan_annote();
    let mut ecran = Fenetre::new();
    for format in [Format::Png, Format::Pdf, Format::Mp4, Format::Gif] {
        b.app.open_export(format);
        let textes = ecran.chauffer(&mut b.app, 4);
        assert!(trouver(&textes, "Avec les annotations").is_some(), "{format:?} : la case est dans la fenêtre : {textes:?}");
        assert!(!b.app.export_dialog.as_ref().unwrap().annotations, "{format:?} : décochée d'office");
        b.app.export_dialog = None;
    }
    b.app.open_export(Format::Wav);
    let textes = ecran.chauffer(&mut b.app, 4);
    assert!(trouver(&textes, "Exporter").is_some() && trouver(&textes, "Avec les annotations").is_none(), "le son seul n'a pas d'image");
    b.app.export_dialog = None;
    b.app.open_export(Format::Png);
    let textes = ecran.chauffer(&mut b.app, 4);
    let p = trouver(&textes, "Avec les annotations").expect("la case");
    ecran.cliquer(&mut b.app, p);
    assert!(b.app.export_dialog.as_ref().unwrap().annotations, "cochée");
    assert!(b.app.settings.export_annotations, "retenue par poste, tout de suite");
    b.app.export_dialog = None;
    b.app.open_export(Format::Pdf);
    assert!(b.app.export_dialog.as_ref().unwrap().annotations, "la fenêtre suivante s'ouvre cochée, pour tous les formats");
}

#[test]
fn an_export_puts_the_plan_annotation_layers_in_only_when_asked() {
    let b = plan_annote();
    let doc = &b.app.doc;
    let dir = dossier("export-annotations");
    let frames = vec![(0usize, 0usize)];
    // Images PNG, sans puis avec : le trait rouge n'est que dans la seconde.
    let mut sans = ExportJob::images(&dir, "sans", frames.clone(), doc.frame, true);
    attendre(&mut sans, &b);
    let mut avec = ExportJob::images(&dir, "avec", frames.clone(), doc.frame, true).avec_annotations(true);
    attendre(&mut avec, &b);
    assert_eq!(compte(&dir.join("sans_0001.png"), rouge), 0, "d'office : jamais dans le film");
    assert!(compte(&dir.join("avec_0001.png"), rouge) > 50, "cochée : le trait d'annotation du plan est par-dessus");
    // Les mêmes images que `panel_plan_at` sans, et `panel_plan_at_avec(.., true)` avec.
    let attendu = |annotations: bool| {
        let plan = crate::render::panel_plan_at_avec(doc, 0, 0, Some(doc.paper), annotations);
        let img = crate::render::camera_image(doc, &plan, &doc.panel_camera(0, 0));
        let fichier = image::open(dir.join(if annotations { "avec_0001.png" } else { "sans_0001.png" })).unwrap().to_rgba8();
        fichier.pixels().zip(img.iter()).all(|(p, c)| p.0 == crate::raster::unpremultiply(*c))
    };
    assert!(attendu(false) && attendu(true), "les images sont celles du rendu du plan, sans ou avec les calques d'annotation");
    // Le rendu de l'export (`Rendu`) suit la même case.
    use crate::exportrender::{snapshot_of, Papier, Rendu};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let rendu = |on: bool| Rendu::new(snapshot_of(doc, &[0]), Papier::Fond, false, Vec::new(), Arc::new(AtomicBool::new(false))).avec_annotations(on);
    assert!(*rendu(false).image(0, 0).unwrap() != *rendu(true).image(0, 0).unwrap());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_board_export_has_the_same_check_box_and_puts_the_board_and_the_plan_annotations_in_only_when_asked() {
    use crate::planche::Options;
    let mut b = plan_annote();
    // Une annotation de la planche, en vert, sur la case du plan.
    let layout = b.app.board_layout();
    let c = layout.cards[0].thumb;
    let trait_vert: Vec<crate::vector::VPoint> = (0..=10).map(|k| crate::vector::VPoint { x: c.left() + c.width() * (0.1 + 0.08 * k as f32), y: c.top() + c.height() * 0.2 + 3.0, r: 3.0 }).collect();
    assert!(b.app.doc.annotations.ajouter_dans(&layout, None, &trait_vert, [0, 255, 0]));
    let dir = dossier("planche-annotations");
    let faire = |b: &Banc, annotations: bool, nom: &str| {
        let opt = Options { annotations, ..Options::default() };
        let mut job = b.app.planche_job(&[0], &opt, &dir.join(nom)).expect("la tâche");
        attendre(&mut job, b);
    };
    faire(&b, false, "sans.png");
    faire(&b, true, "avec.png");
    assert_eq!((compte(&dir.join("sans.png"), rouge), compte(&dir.join("sans.png"), vert)), (0, 0), "d'office : ni les annotations du plan ni celles de la planche");
    assert!(compte(&dir.join("avec.png"), rouge) > 20, "cochée : le calque d'annotation du plan est dans la vignette");
    assert!(compte(&dir.join("avec.png"), vert) > 20, "cochée : le calque d'annotation de la planche est par-dessus");
    // La fenêtre de l'export de la planche : la même case, retenue par poste de travail.
    b.garder_formes = true;
    b.app.open_planche();
    b.chauffer(3);
    assert!(texte_en(&b, "Annotations (1)").is_none() && texte_en(&b, "Annotations (aucune)").is_none(), "plus de case propre à la planche");
    let p = texte_en(&b, "Avec les annotations").expect("la case est dans la fenêtre de la planche");
    assert!(!b.app.settings.export_annotations);
    clic(&mut b, p);
    assert!(b.app.settings.export_annotations, "cochée et retenue");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ Suppr

fn suppr(b: &mut Banc) {
    let k = |pressed| Event::Key { key: Key::Delete, physical_key: Some(Key::Delete), pressed, repeat: false, modifiers: Modifiers::NONE };
    b.image(vec![k(true)], 1.0 / 60.0);
    b.image(vec![k(false)], 1.0 / 60.0);
    b.chauffer(2);
}

#[test]
fn the_delete_key_erases_the_strokes_chosen_with_the_annotation_lasso_in_one_undo_step() {
    let mut b = crate::vignettes_tests::ouvert(quatre_plans(), 3);
    let (id0, id1, id3) = (poser(&mut b, None, 0, 40.0), poser(&mut b, None, 1, 40.0), poser(&mut b, None, 3, 40.0));
    b.chauffer(2);
    lasso(&mut b);
    let etapes = pas_d_annulation(&b);
    // Rien de choisi : Suppr n'efface rien et ne prend aucune étape.
    suppr(&mut b);
    assert_eq!(b.app.doc.annotations.traces.len(), 3);
    assert_eq!(pas_d_annulation(&b), etapes);
    // Deux traits choisis (Maj ajoute) : Suppr les efface, le troisième reste ; une seule étape.
    entourer_trait(&mut b, &[id0], 6.0);
    assert_eq!(b.app.vignettes.annot.selection, vec![id0]);
    let r = b.app.vignettes.xf.to_screen(b.app.doc.annotations.points_planche_de(&b.app.board_layout(), id1).unwrap()[4].pos());
    let maj = Modifiers::SHIFT;
    b.touches = maj;
    b.image(vec![Event::PointerMoved(r)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(r, true, maj)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(r, false, maj)], 1.0 / 60.0);
    b.chauffer(2);
    b.touches = Modifiers::NONE;
    assert_eq!(b.app.vignettes.annot.selection.len(), 2, "Maj + clic ajoute un trait");
    suppr(&mut b);
    let restants: Vec<u64> = b.app.doc.annotations.traces.iter().map(|t| t.id).collect();
    assert_eq!(restants, vec![id3], "les deux traits choisis sont partis, le troisième reste");
    assert!(b.app.vignettes.annot.selection.is_empty(), "plus rien n'est choisi");
    assert_eq!((pas_d_annulation(&b), dernier_nom(&b)), (etapes + 1, "Supprimer des annotations".to_string()), "une seule étape, nommée");
    assert!(b.app.annot_actif(), "le lasso reste pris");
    // Ctrl+Z les rend, tous deux, d'un coup ; rétablir les efface de nouveau.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.traces.len(), 3);
    assert_eq!(pas_d_annulation(&b), etapes);
    b.app.run_action("edition.retablir");
    b.chauffer(2);
    assert_eq!(b.app.doc.annotations.traces.iter().map(|t| t.id).collect::<Vec<_>>(), vec![id3]);
    // Le calque n'a pas bougé.
    assert_eq!(b.app.doc.annotations.calques.len(), 1);
}

// (Le glissé des couches d'annotation dans la liste du panneau Planche n'existe plus, partie 160 : la liste est devenue la ligne
// « Annotations », ses couches sont au clic droit, avec Monter et Descendre, `panneaux_fins_tests.rs`.)
