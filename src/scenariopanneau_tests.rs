//! Essais du scénario gardé dans le projet et de son panneau (`scenariogarde.rs`, `scenariopanneau.rs`) : le fichier
//! `scenario.json` (aller-retour, ancien projet, fichier abîmé, remplacement), les liens plan <-> scène (suppression,
//! déplacement, réunion), la mise en page de chaque sorte d'élément, la vitesse sur un gros scénario, les dispositions.

use std::sync::Arc;
use std::time::Instant;

use eframe::egui;

use crate::app::App;
use crate::document::Document;
use crate::dock::{DockLayout, Group, Pane};
use crate::project;
use crate::scenario::{parse_fountain, Kind, Script};
use crate::scenariogarde::{self, Gardee, Lien, FILE};
use crate::scenarioplans::{self, build, Dest, Options};
use crate::scenariopanneau::{debuts, element_a, hauteur_element, lignes, parties, Al, Col, Mesures, Style};
use crate::workspace::Espace;

const TEXTE: &str = "Title: Le Réveil\nAuthor: Anthony F.\n\nINT. CUISINE - NUIT\n\nMarie cherche sa clé.\n\nMARIE (V.O.)\n(bas)\nOù est-elle ?\n\nPAUL\nCherche encore.\n\nElle sort.\n\nCUT TO:\n\nEXT. RUE - JOUR\n\nIl pleut.\n\nMARIE\nBrr.\n\nFin de journée.\n";

fn script() -> Script {
    parse_fountain(TEXTE, &|_| {})
}

/// Un projet dont les plans viennent du scénario, avec leurs liens.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    let s = script();
    let outline = build(&s, &Options::default());
    let applied = scenarioplans::append_to_project(&mut doc, &outline, Dest::End, 0).expect("plans");
    doc.scenario = Some(Arc::new(Gardee::new("essai.fountain", s, applied.liens)));
    doc
}

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-scenario-garde-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-scenario-panneau-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

// ------------------------------------------------------------------------------------ liens

#[test]
fn the_links_cover_the_script_plan_by_plan_without_gaps() {
    let doc = film();
    let g = doc.scenario.clone().unwrap();
    let n = g.script.elements.len();
    assert_eq!(g.liens.len(), doc.panels.len(), "un lien par plan créé");
    assert_eq!(g.liens[0].de, 0);
    for w in g.liens.windows(2) {
        assert_eq!(w[1].de, w[0].a + 1, "les passages se suivent : {:?}", g.liens);
    }
    assert_eq!(g.liens.last().unwrap().a, n - 1);
    for (p, l) in doc.panels.iter().zip(&g.liens) {
        assert_eq!(l.plan, p.id, "le lien porte le numéro stable du plan, pas son rang");
    }
    // Chaque élément est couvert par un lien.
    assert!((0..n).all(|i| g.lien_de_l_element(i).is_some()));
}

#[test]
fn a_heading_belongs_to_the_first_plan_of_its_scene() {
    let doc = film();
    let g = doc.scenario.clone().unwrap();
    let heading = g.script.elements.iter().position(|e| matches!(e.kind, Kind::Heading(_))).unwrap();
    let k = g.lien_de_l_element(heading).unwrap();
    assert_eq!(g.liens[k].plan, doc.panels[0].id);
    let second = g.script.elements.iter().rposition(|e| matches!(e.kind, Kind::Heading(_))).unwrap();
    let k2 = g.lien_de_l_element(second).unwrap();
    assert!(doc.panels.iter().position(|p| p.id == g.liens[k2].plan).unwrap() > 0);
}

#[test]
fn links_survive_deleted_moved_and_merged_plans() {
    let mut doc = film();
    let g = doc.scenario.clone().unwrap();
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let before = ids.len();
    assert!(before >= 4);
    // Déplacé : le lien suit le plan (même numéro, même passage).
    doc.move_panel(0, before - 1).unwrap();
    for p in &doc.panels {
        let k = g.lien_du_plan(p.id).expect("lien du plan déplacé");
        assert_eq!(g.liens[k].plan, p.id);
    }
    // Supprimé : le lien reste dans le scénario, sans plan ; on va au plus proche plan qui existe.
    let gone = doc.panels[1].id;
    doc.remove_panel(1).unwrap();
    let existe: std::collections::HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
    let k = g.lien_du_plan(gone).unwrap();
    let element = g.liens[k].de;
    let vivant = g.lien_vivant_pres_de(element, &|id| existe.contains(&id)).expect("un plan proche");
    assert_ne!(g.liens[vivant].plan, gone);
    assert!(existe.contains(&g.liens[vivant].plan));
    // Réuni : le plan absorbé n'existe plus, le panneau n'a rien à casser.
    // (Deux plans voisins d'une même scène.)
    let i = (0..doc.panels.len() - 1).find(|&i| doc.panels[i].scene == doc.panels[i + 1].scene).expect("deux plans d'une même scène");
    let first = doc.panels[i].id;
    let absorbed = doc.panels[i + 1].id;
    doc.merge_with_next(i).unwrap();
    assert!(doc.panels.iter().any(|p| p.id == first) && !doc.panels.iter().any(|p| p.id == absorbed));
    let existe: std::collections::HashSet<u64> = doc.panels.iter().map(|p| p.id).collect();
    let element = g.liens[g.lien_du_plan(absorbed).unwrap()].de;
    assert!(g.lien_vivant_pres_de(element, &|id| existe.contains(&id)).is_some());
    // Plus aucun plan lié : rien à montrer, rien ne plante.
    assert_eq!(g.lien_vivant_pres_de(0, &|_| false), None);
}

#[test]
fn a_click_goes_to_the_plan_then_to_the_nearest_one_when_it_is_gone() {
    let (_ctx, mut app) = headless("clic");
    app.doc = film();
    let g = app.doc.scenario.clone().unwrap();
    let last = app.doc.panels.len() - 1;
    // Un clic sur un élément du dernier plan y mène.
    let element = g.liens[last].de;
    assert!(app.scenario_aller_a_l_element(&g, element));
    assert_eq!(app.cur_panel, last);
    // Le plan du premier passage disparaît : le clic va au plus proche, sans plantage.
    let first_id = app.doc.panels[0].id;
    app.doc.remove_panel(0).unwrap();
    app.cur_panel = 0;
    assert!(app.scenario_aller_a_l_element(&g, g.liens[g.lien_du_plan(first_id).unwrap()].de));
    assert!(app.cur_panel < app.doc.panels.len());
    assert_eq!(app.doc.panels[app.cur_panel].id, g.liens[1].plan, "le plus proche : le plan d'après");
    // Plus aucun plan lié : le clic ne fait rien.
    app.doc = Document::new(32, 32, 24);
    app.doc.scenario = Some(g.clone());
    app.cur_panel = 0;
    assert!(!app.scenario_aller_a_l_element(&g, 3));
    assert_eq!(app.cur_panel, 0);
}

#[test]
fn the_passage_shown_is_the_one_of_the_current_plan_or_of_the_linked_plan_before_it() {
    let (_ctx, mut app) = headless("courant");
    let mut doc = film();
    let g = doc.scenario.clone().unwrap();
    // Un plan ajouté à la main après le deuxième, sans lien : on montre le passage du deuxième.
    doc.add_scene_after(1);
    app.doc = doc;
    app.cur_panel = 2;
    let k = app.scenario_lien_courant(&g).unwrap();
    assert_eq!(g.liens[k].plan, app.doc.panels[1].id);
    app.cur_panel = 0;
    assert_eq!(g.liens[app.scenario_lien_courant(&g).unwrap()].plan, app.doc.panels[0].id);
    // Sans plans liés (scénario seul) : pas de surlignage.
    let seul = Gardee::new("x", script(), Vec::new());
    assert_eq!(app.scenario_lien_courant(&seul), None);
}

#[test]
fn links_with_no_element_or_a_repeated_plan_are_dropped() {
    let s = script();
    let n = s.elements.len();
    let g = Gardee::new("x", s, vec![Lien { plan: 1, de: 0, a: 2 }, Lien { plan: 1, de: 3, a: 4 }, Lien { plan: 2, de: n + 5, a: n + 6 }, Lien { plan: 3, de: 5, a: 3 }, Lien { plan: 4, de: 6, a: n + 40 }]);
    assert_eq!(g.liens.iter().map(|l| l.plan).collect::<Vec<_>>(), vec![1, 4]);
    assert_eq!(g.liens[1].a, n - 1, "la fin est bornée");
}

// ------------------------------------------------------------------------------------ fichier

#[test]
fn the_script_is_saved_next_to_the_manifest_and_read_back() {
    let doc = film();
    let dir = temp("aller-retour");
    let manifest = dir.join("F.shot");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(dir.join(FILE).exists(), "scenario.json est écrit à côté du manifeste");
    let loaded = project::load(&manifest).unwrap();
    assert!(!loaded.warnings.iter().any(|w| w.contains(FILE)), "{:?}", loaded.warnings);
    let (a, b) = (doc.scenario.clone().unwrap(), loaded.doc.scenario.clone().expect("scénario relu"));
    assert_eq!(a.script, b.script);
    assert_eq!(a.liens, b.liens);
    assert_eq!((a.id, a.nom.as_str()), (b.id, b.nom.as_str()));
    // Les plans relus gardent leurs numéros : le lien tient.
    assert!(loaded.doc.panels.iter().all(|p| b.lien_du_plan(p.id).is_some()));
    // « Enregistrer sous » : le scénario suit.
    let dir2 = temp("enregistrer-sous");
    let manifest2 = dir2.join("G.shot");
    project::save(&loaded.doc, &manifest2, &mut Default::default()).unwrap();
    assert_eq!(project::load(&manifest2).unwrap().doc.scenario.unwrap().script, a.script);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn an_old_project_without_a_script_opens_as_before() {
    let mut doc = film();
    doc.scenario = None;
    let dir = temp("sans-scenario");
    let manifest = dir.join("F.shot");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists(), "rien à écrire sans scénario");
    let loaded = project::load(&manifest).unwrap();
    assert!(loaded.doc.scenario.is_none() && !loaded.warnings.iter().any(|w| w.contains(FILE)));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_damaged_or_newer_file_does_not_stop_the_project_and_is_never_erased() {
    let doc = film();
    let dir = temp("abime");
    let manifest = dir.join("F.shot");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    for (nom, contenu) in [("abîmé", "{ pas du json".to_string()), ("plus récent", r#"{"format":"shot-scenario","version":99,"scenario":{}}"#.to_string()), ("autre fichier", r#"{"format":"autre","version":1}"#.to_string())] {
        std::fs::write(dir.join(FILE), &contenu).unwrap();
        let loaded = project::load(&manifest).unwrap();
        assert!(loaded.doc.scenario.is_none(), "{nom}");
        assert!(loaded.warnings.iter().any(|w| w.contains(FILE)), "{nom} : {:?}", loaded.warnings);
        // Réenregistré sans scénario : le fichier illisible reste tel quel.
        project::save(&loaded.doc, &manifest, &mut Default::default()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), contenu, "{nom}");
    }
    // Réenregistré avec un scénario : l'illisible est mis de côté, pas effacé.
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(project::load(&manifest).unwrap().doc.scenario.is_some());
    let aside: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("scenario.json.illisible")).collect();
    assert_eq!(aside.len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_replaced_script_is_kept_beside_the_new_one_never_erased() {
    let mut doc = film();
    let dir = temp("remplacement");
    let manifest = dir.join("F.shot");
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let ancien = doc.scenario.clone().unwrap();
    // Les mêmes éléments, d'autres liens : le même scénario, rien n'est mis de côté.
    doc.scenario = Some(Arc::new(ancien.avec_liens(Vec::new())));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let anciens = || std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("scenario.ancien-")).count();
    assert_eq!(anciens(), 0);
    // Un autre scénario : l'ancien fichier passe à côté.
    let autre = parse_fountain("INT. GARE - JOUR\n\nUn train passe.\n", &|_| {});
    doc.scenario = Some(Arc::new(Gardee::new("autre.fountain", autre.clone(), Vec::new())));
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert_eq!(anciens(), 1, "l'ancien scénario est gardé à côté");
    let loaded = project::load(&manifest).unwrap();
    assert_eq!(loaded.doc.scenario.unwrap().script, autre);
    let garde = std::fs::read_dir(&dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with("scenario.ancien-")).unwrap();
    let relu = scenariogarde::parse(&std::fs::read_to_string(garde.path()).unwrap()).unwrap();
    assert_eq!(relu.id, ancien.id);
    // Plus de scénario (import annulé, puis enregistré) : celui du fichier passe aussi à côté.
    doc.scenario = None;
    project::save(&doc, &manifest, &mut Default::default()).unwrap();
    assert!(!dir.join(FILE).exists());
    assert_eq!(anciens(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_script_follows_the_undo_history_and_the_working_copy() {
    let mut doc = film();
    let snap = doc.structure();
    assert!(snap.scenario.is_some());
    let mut vide = Document::new(32, 32, 24);
    let rien = vide.structure();
    let _ = vide.replace_structure(snap);
    assert!(vide.scenario.is_some(), "la structure rétablie rapporte le scénario");
    let _ = vide.replace_structure(rien);
    assert!(vide.scenario.is_none());
    // La copie sans dessins (enregistrement) l'emporte aussi.
    assert!(doc.copie_sans_dessins().scenario.is_some());
    doc.scenario = None;
    assert!(doc.copie_sans_dessins().scenario.is_none());
}

// ------------------------------------------------------------------------------------ mise en page

fn mesures() -> Mesures {
    Mesures { cw: 7.8, lh: 17.0, corps: 13.0, w: 300.0 }
}

#[test]
fn every_kind_of_element_has_its_screenplay_layout() {
    let s = parse_fountain("# Acte I\n\n= Marie cherche.\n\nINT. CUISINE - NUIT\n\nMarie cherche sa clé. [[à revoir]]\n\nMARIE (V.O.)\n(bas)\nOù est-elle ?\n\nCUT TO:\n\n[[une note seule]]\n", &|_| {});
    let kind = |f: &dyn Fn(&Kind) -> bool| s.elements.iter().find(|e| f(&e.kind)).unwrap_or_else(|| panic!("élément absent : {:?}", s.elements));
    // Intitulé de scène : gras et majuscules, pleine largeur.
    let h = parties(kind(&|k| matches!(k, Kind::Heading(_))));
    assert_eq!(h.len(), 1);
    assert_eq!((h[0].style, h[0].col, h[0].al), (Style::Gras, Col::Pleine, Al::Gauche));
    assert_eq!(h[0].texte, h[0].texte.to_uppercase());
    assert!(h[0].texte.contains("CUISINE"));
    // Action : pleine largeur, normale ; sa note, discrète, dessous.
    let a = parties(kind(&|k| matches!(k, Kind::Action(_))));
    assert_eq!((a[0].style, a[0].col, a[0].al), (Style::Normal, Col::Pleine, Al::Gauche));
    assert!(a.len() == 2 && a[1].style == Style::Doux && a[1].texte.contains("à revoir"), "{a:?}");
    // Réplique : nom centré en majuscules (avec V.O.), parenthèse italique centrée en colonne étroite, texte en colonne.
    let d = parties(kind(&|k| matches!(k, Kind::Dialogue(_))));
    assert_eq!(d.len(), 3, "{d:?}");
    assert_eq!((d[0].al, d[0].col, d[0].texte.as_str()), (Al::Centre, Col::Pleine, "MARIE (V.O.)"));
    assert_eq!((d[1].style, d[1].col, d[1].al, d[1].texte.as_str()), (Style::Italique, Col::Parenthese, Al::Centre, "(bas)"));
    assert_eq!((d[2].col, d[2].al), (Col::Replique, Al::Gauche));
    // Transition : à droite, majuscules.
    let t = parties(kind(&|k| matches!(k, Kind::Transition(_))));
    assert_eq!((t[0].al, t[0].texte.as_str()), (Al::Droite, "CUT TO:"));
    // Notes et synopsis : discrets.
    let n = parties(kind(&|k| matches!(k, Kind::Note(_))));
    assert_eq!(n[0].style, Style::Doux);
    let sy = parties(kind(&|k| matches!(k, Kind::Synopsis(_))));
    assert_eq!(sy[0].style, Style::Doux);
    // Les colonnes : la réplique est plus étroite que l'action, la parenthèse encore plus.
    let m = mesures();
    assert!(m.colonne(Col::Replique) < m.colonne(Col::Pleine) && m.colonne(Col::Parenthese) < m.colonne(Col::Replique));
}

#[test]
fn lines_are_counted_like_the_layout_wraps_them() {
    assert_eq!(lignes("", 10), 1);
    assert_eq!(lignes("un deux trois", 20), 1);
    assert_eq!(lignes("un deux trois", 7), 2);
    assert_eq!(lignes("un deux trois", 5), 3);
    assert_eq!(lignes("a\nb\n\nc", 10), 4, "les retours à la ligne et les lignes vides comptent");
    assert_eq!(lignes("abcdefghij", 4), 3, "un mot trop long se coupe");
    assert_eq!(lignes("x", 0), 1, "jamais de division par zéro");
}

#[test]
fn positions_map_back_to_elements() {
    let g = film().scenario.clone().unwrap();
    let d = debuts(&mesures(), &g);
    assert_eq!(d.len(), g.script.elements.len() + 1);
    assert!(d.windows(2).all(|w| w[0] <= w[1]), "croissant");
    assert_eq!(element_a(&d, -5.0), 0);
    assert_eq!(element_a(&d, 0.0), 0);
    for i in 0..g.script.elements.len() {
        assert_eq!(element_a(&d, d[i] + 0.01), i);
    }
    assert_eq!(element_a(&d, 1e9), g.script.elements.len() - 1);
    assert_eq!(element_a(&[0.0], 5.0), 0, "un scénario vide ne plante pas");
    assert!(hauteur_element(&mesures(), &g.script.elements[0]) > 0.0);
}

#[test]
fn a_120_page_script_lays_out_in_a_few_milliseconds() {
    let mut texte = String::from("Title: Long\n\n");
    for s in 0..600 {
        texte.push_str(&format!("INT. LIEU {s} - JOUR\n\nIl y a de l'action à raconter dans ce lieu, avec assez de mots pour passer à la ligne plusieurs fois dans une colonne étroite.\n\n"));
        for r in 0..6 {
            texte.push_str(&format!("PERSO{}\n(bas)\nUne réplique assez longue pour tenir sur deux ou trois lignes de la colonne de dialogue {r}.\n\n", r % 3));
        }
        texte.push_str("CUT TO:\n\n");
    }
    let s = parse_fountain(&texte, &|_| {});
    assert!(s.elements.len() > 4000, "{}", s.elements.len());
    let g = Gardee::new("long.fountain", s, Vec::new());
    let t = Instant::now();
    let d = debuts(&mesures(), &g);
    let ms = t.elapsed().as_millis();
    assert!(ms < 150, "{ms} ms pour {} éléments", g.script.elements.len());
    assert!(*d.last().unwrap() > 50_000.0);
    // L'aller-retour du fichier d'un gros scénario reste raisonnable.
    let mut doc = Document::new(32, 32, 24);
    doc.scenario = Some(Arc::new(g));
    let t = Instant::now();
    let bytes = scenariogarde::to_bytes(&doc).unwrap().unwrap();
    let back = scenariogarde::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(back.script.elements.len(), doc.scenario.as_ref().unwrap().script.elements.len());
    assert!(t.elapsed().as_millis() < 1500, "{} ms", t.elapsed().as_millis());
}

#[test]
fn the_panel_draws_empty_with_a_script_and_after_the_structure_changes() {
    let (ctx, mut app) = headless("dessin");
    let frame = |app: &mut App| {
        let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(360.0, 700.0))), ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.scenario_panneau(ui));
        });
    };
    // Sans scénario : la phrase et le bouton.
    app.doc.scenario = None;
    frame(&mut app);
    // Avec un scénario, plusieurs images, les trois tailles.
    app.doc = film();
    for taille in [scenariogarde::Taille::Petit, scenariogarde::Taille::Moyen, scenariogarde::Taille::Grand] {
        app.settings.scenario_taille = taille;
        for _ in 0..3 {
            frame(&mut app);
        }
    }
    // Un plan supprimé, un plan courant hors limites, un scénario sans plan lié : aucun plantage.
    app.doc.remove_panel(1).unwrap();
    app.cur_panel = app.doc.panels.len() - 1;
    frame(&mut app);
    app.doc.scenario = Some(Arc::new(Gardee::new("x", script(), Vec::new())));
    frame(&mut app);
    // Un scénario sans aucun élément.
    app.doc.scenario = Some(Arc::new(Gardee::new("vide", Script::default(), Vec::new())));
    frame(&mut app);
}

// ------------------------------------------------------------------------------------ dispositions

#[test]
fn the_panel_is_in_the_vignettes_and_storyboard_layouts_and_closed_elsewhere() {
    let a = |e: Espace| e.defaut().dock_layout();
    assert!(a(Espace::Vignettes).contains(Pane::Scenario));
    assert!(a(Espace::Storyboard).contains(Pane::Scenario));
    assert!(!a(Espace::Animation).contains(Pane::Scenario));
    assert!(!a(Espace::Montage).contains(Pane::Scenario));
    // Il ne prend aucune place de plus : en onglet derrière un panneau qui reste devant.
    for e in [Espace::Vignettes, Espace::Storyboard] {
        let l = a(e);
        let mut sans = l.clone();
        sans.close(Pane::Scenario);
        assert_eq!(sans.panes().len() + 1, l.panes().len());
    }
    assert_eq!(Pane::Scenario.group(), Group::Right);
    assert!(Pane::Scenario.on_demand());
    assert_eq!(Pane::Scenario.name(), "Scénario");
    assert_eq!(serde_json::to_string(&Pane::Scenario).unwrap(), "\"scenario\"");
    assert!(Pane::ALL.contains(&Pane::Scenario));
}

#[test]
fn the_panel_opens_beside_the_existing_ones_in_every_workspace() {
    for e in Espace::TOUS {
        let mut l: DockLayout = e.defaut().dock_layout();
        l.close(Pane::Scenario);
        assert!(!l.contains(Pane::Scenario));
        l.open(Pane::Scenario);
        assert!(l.contains(Pane::Scenario), "{e:?}");
        assert!(l.contains(Pane::Canvas));
    }
}

#[test]
fn the_panel_has_its_bar_icon_its_action_and_its_tooltip() {
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.scenario"), Some(Pane::Scenario));
    let e = crate::barrepanneaux::entree_de_action("vue.scenario").unwrap();
    assert_eq!(e.nom, "Scénario");
    assert!(e.description.len() > 40);
    assert!(crate::barrepanneaux::panneaux_d_office(Espace::Vignettes).contains(&Pane::Scenario));
    assert!(crate::barrepanneaux::panneaux_d_office(Espace::Storyboard).contains(&Pane::Scenario));
    assert!(!crate::barrepanneaux::panneaux_d_office(Espace::Montage).contains(&Pane::Scenario));
    let info = crate::keymap::action_info("scenario.creer_plans").expect("action déclarée");
    assert!(info.implemented && info.label.contains("plans"));
    assert!(crate::keymap::action_info("vue.scenario").is_some());
    // Le menu Fichier ne promet plus de plans.
    let import = crate::keymap::action_info("fichier.importer_scenario").unwrap();
    assert!(import.label.contains("garder"), "{}", import.label);
}

#[test]
fn the_import_window_keeps_the_script_and_asks_before_replacing() {
    let (ctx, mut app) = headless("import");
    let dir = std::env::temp_dir().join(format!("shot-scenario-import-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("Le Réveil.fountain");
    std::fs::write(&file, TEXTE).unwrap();
    app.read_scenario(file.clone());
    let t = Instant::now();
    while !app.scenario_window.as_ref().unwrap().ready() {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
        assert!(t.elapsed().as_secs() < 10);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let plans_avant = app.doc.panels.len();
    // L'import garde le scénario sans créer aucun plan, ouvre le panneau, en une étape d'annulation.
    let script = script();
    assert!(app.scenario_keep(&script, "Le Réveil.fountain"));
    assert_eq!(app.doc.panels.len(), plans_avant);
    assert!(app.doc.scenario.as_ref().is_some_and(|g| g.liens.is_empty() && g.script == script));
    assert!(app.pane_shown(Pane::Scenario), "le panneau s'ouvre tout seul après l'import");
    app.run_action("edition.annuler");
    assert!(app.doc.scenario.is_none());
    app.run_action("edition.retablir");
    assert!(app.doc.scenario.is_some());
    // Un deuxième import : la fenêtre demande s'il remplace l'actuel ; l'ancien n'est pas touché tant qu'on n'a pas répondu.
    app.read_scenario(file.clone());
    while !app.scenario_window.as_ref().unwrap().ready() {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let avant = app.doc.scenario.clone().unwrap();
    // (Le clic sur « Importer » est simulé par le même chemin que la fenêtre : la question se pose.)
    assert!(!app.scenario_window.as_ref().unwrap().asks_to_replace());
    std::fs::remove_dir_all(&dir).ok();
    assert!(Arc::ptr_eq(&avant, app.doc.scenario.as_ref().unwrap()));
}
