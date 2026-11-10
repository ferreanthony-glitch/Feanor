//! « Peindre derrière » et « Peint sur l'aplat » (deux interrupteurs, exclusifs), puis les outils de l'aplat : sélection et
//! transformation (une clé ou plusieurs), peinture et pot de peinture sur plusieurs clés, portée du changement de couleur.
//! (Anthony, 2026-10-28.)

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::document::Document;
use crate::perf_tests::{banc, Banc};

// ------------------------------------------------------------------------------------ l'outillage

/// Un plan de 400 × 300, un calque bitmap et un anneau carré tracé au pinceau (60..160).
fn banc_anneau() -> Banc {
    let mut b = banc(Document::new(400, 300, 24), false);
    b.app.tool = Tool::Brush;
    trait_carre(&mut b.app, 60.0, 60.0, 160.0, 160.0);
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

fn noms(app: &App) -> Vec<String> {
    app.history.names().0.iter().map(|n| n.to_string()).collect()
}

fn peint(r: &[Color32]) -> usize {
    r.iter().filter(|c| c.a() > 0).count()
}

/// Le trait du calque `li` à l'image `f`.
fn trait_de(app: &App, li: usize, f: usize) -> Vec<Color32> {
    let cel = app.layers()[li].frames[f].expect("dessin");
    app.doc.cels[&cel].raster.to_flat()
}

/// La peinture de la strate d'aplat du dessin du calque `li` à l'image `f`.
fn strate_de(app: &App, li: usize, f: usize) -> Vec<Color32> {
    let cel = app.layers()[li].frames[f].expect("dessin");
    app.doc.cels[&cel].fond.as_ref().map(|s| s.peinture.raster.to_flat()).unwrap_or_default()
}

// ------------------------------------------------------------------------------------ les deux interrupteurs

#[test]
fn the_two_toggles_are_separate_and_exclusive() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    assert!(!app.paint_behind && !app.paint_on_aplat);
    app.run_action("pinceau.derriere");
    assert!(app.paint_behind && !app.paint_on_aplat);
    // L'autre l'éteint (et inversement).
    app.run_action("pinceau.sur_aplat");
    assert!(!app.paint_behind && app.paint_on_aplat, "{}", app.status);
    app.run_action("pinceau.derriere");
    assert!(app.paint_behind && !app.paint_on_aplat);
    app.run_action("pinceau.derriere");
    assert!(!app.paint_behind && !app.paint_on_aplat);
    // Chacun a son action, son nom et sa phrase dans la liste des raccourcis ; « Peint sur l'aplat » n'a pas de touche d'office.
    let derriere = crate::keymap::ACTIONS.iter().find(|a| a.id == "pinceau.derriere").expect("pinceau.derriere");
    let sur = crate::keymap::ACTIONS.iter().find(|a| a.id == "pinceau.sur_aplat").expect("pinceau.sur_aplat");
    assert!(derriere.label.to_lowercase().contains("derrière"), "{}", derriere.label);
    assert!(sur.label.to_lowercase().contains("sur l'aplat"), "{}", sur.label);
    assert_eq!(app.raccourci("pinceau.sur_aplat"), "", "pas de touche par défaut");
}

#[test]
fn old_settings_with_peint_derriere_give_paint_behind() {
    use crate::settings::Settings;
    // Un ancien fichier : la clé d'avant est « Peindre derrière », « Peint sur l'aplat » est éteint.
    let old: Settings = toml::from_str("peint_derriere = true").unwrap();
    assert!(old.paint_behind && !old.paint_on_aplat);
    // La clé de « Peint sur l'aplat » est à elle, et fait l'aller-retour.
    let mut s = Settings::default();
    s.paint_on_aplat = true;
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("peint_sur_aplat = true") && text.contains("peint_derriere = false"), "{text}");
    let back: Settings = toml::from_str(&text).unwrap();
    assert!(back.paint_on_aplat && !back.paint_behind);
    // L'application les lit au lancement ; un fichier qui aurait les deux garde « Peindre derrière ».
    let dir = std::env::temp_dir().join(format!("shot-aplatsur-reglages-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let fichier = crate::donnees::roaming().expect("dossier des réglages").join("reglages.toml");
    let _ = std::fs::create_dir_all(fichier.parent().unwrap());
    let ctx = egui::Context::default();
    for (texte, derriere, sur) in [("peint_derriere = true", true, false), ("peint_sur_aplat = true", false, true), ("peint_derriere = true\npeint_sur_aplat = true", true, false)] {
        std::fs::write(&fichier, texte).unwrap();
        let app = App::sans_fenetre(&ctx, None);
        assert_eq!((app.paint_behind, app.paint_on_aplat), (derriere, sur), "{texte}");
    }
}

#[test]
fn both_toggles_are_buttons_above_more_settings_with_their_own_tooltips() {
    use egui_phosphor::regular as ph;
    let mut b = banc_anneau();
    b.app.set_tool(Tool::Brush);
    let ctx = egui::Context::default();
    let mut textes: Vec<String> = Vec::new();
    for _ in 0..3 {
        let out = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(400.0, 900.0))), ..Default::default() }, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| b.app.tool_options_panel(ui));
        });
        textes.clear();
        fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                _ => {}
            }
        }
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
    }
    assert!(textes.iter().any(|t| t.contains(ph::SELECTION_BACKGROUND)), "« Peindre derrière » : {textes:?}");
    assert!(textes.iter().any(|t| t.contains(ph::SQUARE_HALF_BOTTOM)), "« Peint sur l'aplat » : {textes:?}");
    assert_ne!(ph::SELECTION_BACKGROUND, ph::SQUARE_HALF_BOTTOM, "chacun son icône");
    // Les deux textes de leur bulle d'une seconde sont dans `panels.rs`, reliés à `bulles::bulle` par `interrupteur_a`.
    let source = include_str!("panels.rs");
    assert!(source.contains("\"Peindre derrière\"") && source.contains("\"Peint sur l'aplat\""));
}

// ------------------------------------------------------------------------------------ peint sur l'aplat

#[test]
fn paint_on_aplat_creates_the_stratum_and_never_touches_the_line() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    assert!(app.layers()[0].fond.is_none());
    let trait0 = trait_de(app, 0, 0);
    app.paint_on_aplat = true;
    app.color = [10, 200, 10];
    let n0 = noms(app).len();
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    // L'aplat a été créé (comme le bouton du calque), la peinture est dedans, le trait n'a pas bougé.
    assert!(app.layers()[0].fond.is_some(), "{}", app.status);
    assert_eq!(trait_de(app, 0, 0), trait0, "le trait n'est pas touché");
    assert!(peint(&strate_de(app, 0, 0)) > 0, "la peinture est dans l'aplat");
    assert!(app.status.to_lowercase().contains("aplat"), "la barre d'état le dit : {}", app.status);
    let apres = noms(app);
    assert!(apres.len() >= n0 + 2 && apres.contains(&"Aplat : ajouter".to_string()) && apres.last().map(String::as_str) == Some("Aplat : peinture"), "{apres:?}");
    // Un deuxième trait : l'aplat existe, plus de création.
    let n1 = noms(app).len();
    gliss(app, pos2(90.0, 130.0), pos2(130.0, 130.0));
    assert_eq!(noms(app).len(), n1 + 1, "un geste, une étape : {:?}", noms(app));
    assert_eq!(trait_de(app, 0, 0), trait0);
    // Tout s'annule, jusqu'au trait seul.
    app.undo();
    app.undo();
    app.undo();
    assert!(app.layers()[0].fond.is_none(), "l'aplat créé s'annule aussi");
    assert_eq!(trait_de(app, 0, 0), trait0);
    // Éteint, le pinceau dessine de nouveau le trait.
    app.paint_on_aplat = false;
    gliss(app, pos2(200.0, 200.0), pos2(260.0, 200.0));
    assert_ne!(trait_de(app, 0, 0), trait0);
}

#[test]
fn paint_on_aplat_goes_into_the_colour_layer_of_a_line_layer() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    app.cur_layer = 0;
    app.new_colour_layer();
    assert_eq!(app.layers().len(), 2);
    let (couleur, trait_) = (0usize, 1usize);
    assert!(app.layers()[couleur].couleur.is_some());
    app.cur_layer = trait_;
    app.paint_on_aplat = true;
    app.color = [200, 30, 30];
    let trait0 = trait_de(app, trait_, 0);
    gliss(app, pos2(90.0, 110.0), pos2(130.0, 110.0));
    assert_eq!(app.cur_layer, trait_, "le calque courant n'a pas changé");
    assert_eq!(trait_de(app, trait_, 0), trait0, "le trait n'est pas touché");
    assert!(app.layers()[trait_].fond.is_none(), "pas d'aplat sous le trait : le calque de couleur a servi");
    let c = app.layers()[couleur].frames[0].expect("le dessin de couleur de cette image");
    assert!(peint(&app.doc.cels[&c].raster.to_flat()) > 0, "la peinture est dans le calque de couleur");
    // Un geste, une étape, qui s'annule.
    let avant = noms(app).len();
    app.undo();
    assert_eq!(noms(app).len() + 1, avant, "{:?}", noms(app));
    // (Annuler le trait de couleur, pas autre chose.)
    let c = app.layers()[couleur].frames[0].expect("dessin");
    assert_eq!(peint(&app.doc.cels[&c].raster.to_flat()), 0, "la couleur est revenue à vide");
    assert_eq!(trait_de(app, trait_, 0), trait0);
}

#[test]
fn paint_on_aplat_does_not_divert_the_eraser_or_a_colour_layer() {
    let mut b = banc_anneau();
    let app = &mut b.app;
    app.paint_on_aplat = true;
    // La gomme n'est pas détournée : elle efface du trait, et ne crée pas d'aplat.
    let trait0 = trait_de(app, 0, 0);
    app.set_tool(Tool::Eraser);
    gliss(app, pos2(55.0, 110.0), pos2(65.0, 110.0));
    assert_ne!(trait_de(app, 0, 0), trait0, "la gomme efface du trait");
    assert!(app.layers()[0].fond.is_none());
    // Sur un calque de couleur, rien n'est détourné : on peint la couleur, dans la zone du trait.
    app.set_tool(Tool::Brush);
    app.cur_layer = 0;
    app.new_colour_layer();
    let colour = app.cur_layer;
    assert!(app.layers()[colour].couleur.is_some());
    assert_eq!(app.sur_aplat_cible(), None, "un calque de couleur est déjà l'aplat");
    // La sous-ligne de l'aplat déjà choisie : elle reste la cible, rien à détourner.
    app.cur_layer = 1;
    app.toggle_fond(1);
    app.select_aplat(1);
    assert_eq!(app.sur_aplat_cible(), None);
    // Le pot de peinture ne l'est pas non plus.
    app.paint_on_aplat = true;
    app.set_tool(Tool::Fill);
    assert_eq!(app.sur_aplat_cible(), None);
}

// ------------------------------------------------------------------------------------ sélectionner et transformer l'aplat

use crate::document::{CelId, Layer, LayerKind, Strate};
use crate::multisel::{self, PartKind};
use crate::fond::FondReglages;
use crate::montagekeys::KeyRef;
use crate::raster::Raster;
use eframe::egui::vec2;

const VERT: Color32 = Color32::from_rgb(10, 200, 10);

fn remplir(r: &mut Raster, x0: usize, y0: usize, x1: usize, y1: usize, c: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            let i = r.tile_index_of(x, y);
            r.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = c;
        }
    }
}

/// Un calque « A » (bitmap) aux dessins des images 0 à 4, 5 à 9 et 10 à 19 (trois clés). Chaque dessin a un anneau noir (le
/// trait), un aplat dont la peinture est un rectangle vert décalé selon la clé. La sous-ligne de l'aplat est choisie.
struct AplatFix {
    app: App,
    panel: u64,
    layer: u64,
    cels: Vec<CelId>,
}

fn aplat_fix(name: &str) -> AplatFix {
    let _ = name;
    let mut doc = Document::with_frame(200, 150, 24);
    // (La feuille n'est pas le cadre : les dessins de la strate ont la taille de la feuille.)
    let (dw, dh) = (doc.width, doc.height);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(20);
    let mut layer = Layer::new("A", LayerKind::Bitmap);
    layer.fond = Some(FondReglages::default());
    let mut cels = Vec::new();
    for (k, (from, to)) in [(0usize, 5usize), (5, 10), (10, 20)].into_iter().enumerate() {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let c = doc.cels.get_mut(&cel).unwrap();
        let black = Color32::from_rgb(0, 0, 0);
        remplir(&mut c.raster, 20, 20, 100, 23, black);
        remplir(&mut c.raster, 20, 77, 100, 80, black);
        remplir(&mut c.raster, 20, 20, 23, 80, black);
        remplir(&mut c.raster, 97, 20, 100, 80, black);
        let mut s = Strate::new(dw, dh);
        remplir(&mut s.peinture.raster, 40 + k * 6, 35, 70 + k * 6, 55, VERT);
        c.fond = Some(Box::new(s));
        for f in from..to {
            layer.set(f, Some(cel));
        }
        cels.push(cel);
    }
    let id = layer.id;
    doc.panels[0].layers.push(layer);
    let panel = doc.panels[0].id;
    // Un banc : l'application a tourné quelques images (bibliothèque de pinceaux lue, vue ajustée), comme une vraie.
    let Banc { mut app, .. } = banc(doc, false);
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.tool = Tool::Select;
    // (Dessiner se fait en mode Animation : en Storyboard, le dessin passe par les plans.)
    app.mode = crate::app::Mode::Animation;
    app.aplat_cible = Some(id);
    AplatFix { app, panel, layer: id, cels }
}

impl AplatFix {
    fn choose(&mut self, starts: &[usize]) {
        self.app.key_sel = starts.iter().map(|&s| KeyRef { panel: self.panel, layer: self.layer, start: s }).collect();
    }

    fn peinture(&self, k: usize) -> &Raster {
        &self.app.doc.cels[&self.cels[k]].fond.as_ref().expect("aplat").peinture.raster
    }

    fn vert(&self, k: usize, x: usize, y: usize) -> bool {
        self.peinture(k).get(x, y) == VERT
    }

    fn trait_(&self, k: usize) -> Vec<Color32> {
        self.app.doc.cels[&self.cels[k]].raster.to_flat()
    }
}

#[test]
fn the_selection_tool_takes_the_pixels_of_the_aplat_when_its_sub_row_is_the_target() {
    let mut f = aplat_fix("un");
    assert!(f.app.aplat_en_cible() && f.app.aplat_strate_selection() && f.app.multi_wanted());
    assert!(f.app.aplat_indication().contains("pixels de l'aplat"), "la ligne du bas le dit : {}", f.app.aplat_indication());
    // Cliquer la sous-ligne avec l'outil Sélection garde l'outil.
    f.app.aplat_cible = None;
    f.app.select_aplat(0);
    assert_eq!(f.app.tool, Tool::Select);
    assert!(f.app.aplat_strate_selection());
    // Un lasso qui n'entoure que le trait (pas la peinture de l'aplat) ne prend rien : le trait n'est pas la cible.
    f.app.multi_select_polygon(&[pos2(10.0, 10.0), pos2(30.0, 10.0), pos2(30.0, 90.0), pos2(10.0, 90.0)], false);
    assert!(f.app.multi.sel.is_none(), "le trait n'est pas pris : {}", f.app.status);
    // Un lasso autour de la peinture la prend.
    f.app.multi_select_polygon(&[pos2(30.0, 30.0), pos2(80.0, 30.0), pos2(80.0, 60.0), pos2(30.0, 60.0)], false);
    let sel = f.app.multi.sel.as_ref().expect("la peinture de l'aplat");
    assert_eq!(sel.parts.len(), 1);
    assert!(sel.parts[0].aplat && matches!(sel.parts[0].kind, PartKind::Bitmap { .. }));
    assert!(sel.summary().contains("aplat"), "{}", sel.summary());
    // Le rectangle fait pareil.
    f.app.multi.sel = None;
    f.app.select_rect = true;
    f.app.multi.drag = Some(crate::multisel::MDrag::Rect { from: pos2(30.0, 30.0), to: pos2(80.0, 60.0), add: false });
    f.app.multi_finish_drag();
    assert!(f.app.multi.sel.as_ref().is_some_and(|s| s.parts.len() == 1 && s.parts[0].aplat), "{}", f.app.status);
    // Un clic sur la peinture choisit sa forme ; un clic sur le trait ne choisit rien.
    f.app.multi.sel = None;
    assert!(f.app.multi_pick_object(pos2(50.0, 40.0), false));
    assert!(f.app.multi.sel.as_ref().is_some_and(|s| s.parts[0].aplat));
    f.app.multi.sel = None;
    assert!(!f.app.multi_pick_object(pos2(21.0, 40.0), false), "le trait n'est pas un objet de l'aplat");
    // Tout sélectionner : le rectangle de la peinture.
    f.app.multi.sel = None;
    f.app.select_all();
    assert!(f.app.multi.sel.as_ref().is_some_and(|s| s.parts.len() == 1 && s.parts[0].aplat));
    // Sans aplat (ou si l'on revient au trait) la sélection n'est plus la bonne.
    f.app.aplat_cible = None;
    f.app.multi_sync();
    assert!(f.app.multi.sel.is_none(), "revenir au trait lâche la sélection de l'aplat");
}

#[test]
fn moving_the_aplat_selection_never_touches_the_line_and_one_undo_restores_it() {
    let mut f = aplat_fix("deplacer");
    f.app.select_all();
    let (trait0, replays) = (f.trait_(0), crate::vector::replay_count());
    let n0 = noms(&f.app).len();
    f.app.multi_nudge(vec2(20.0, 10.0));
    assert!(f.vert(0, 70, 50) && !f.vert(0, 50, 40), "la peinture de l'aplat a bougé de (20, 10)");
    assert_eq!(f.trait_(0), trait0, "le trait n'est pas touché");
    assert_eq!(noms(&f.app).len(), n0 + 1, "une étape : {:?}", noms(&f.app));
    assert_eq!(noms(&f.app).last().map(String::as_str), Some("Déplacer"));
    assert_eq!(crate::vector::replay_count(), replays, "aucun rejeu du moteur des brosses");
    // La version de la strate a changé : l'aplat affiché est refait.
    f.app.run_action("edition.annuler");
    assert!(f.vert(0, 50, 40) && !f.vert(0, 70, 50), "l'annulation rend la peinture");
    assert_eq!(f.trait_(0), trait0);
    f.app.run_action("edition.retablir");
    assert!(f.vert(0, 70, 50), "rétablir la remet");
}

#[test]
fn a_gesture_of_the_box_scales_and_rotates_the_aplat_with_no_replay_and_one_step() {
    let mut f = aplat_fix("boite");
    f.app.select_all();
    let (trait0, replays) = (f.trait_(0), crate::vector::replay_count());
    let before = f.peinture(0).to_flat();
    let pivot = pos2(55.0, 45.0);
    let a = crate::vector::Affine::rotate_about(pivot, 90f32.to_radians()).then(&crate::vector::Affine::scale_about(pivot, 1.0, 1.0));
    // Le geste en deux temps, comme la souris : la pose à chaque image (sans rien rejouer), puis le lâcher.
    let n0 = noms(&f.app).len();
    let sel = f.app.multi.sel.as_mut().unwrap();
    let mut floats = Vec::new();
    let snaps = multisel::begin_gesture(&mut f.app.doc, sel, &mut floats);
    assert!(floats.is_empty(), "des pixels, pas de feuille vectorielle");
    let dirty = multisel::set_gesture(&mut f.app.doc, sel, &mut floats, &snaps, &a, false);
    assert!(!dirty.is_empty());
    let edits = multisel::commit_gesture(&mut f.app.doc, sel, &mut floats, snaps, &a);
    assert_eq!(edits.len(), 1);
    assert!(matches!(edits[0], crate::history::Edit::FondTiles { coupe: false, .. }), "l'étape est celle de l'aplat");
    f.app.history.push(crate::history::Edit::Group(edits));
    assert_eq!(noms(&f.app).len(), n0 + 1);
    assert_ne!(f.peinture(0).to_flat(), before, "la peinture a tourné");
    assert_eq!(f.trait_(0), trait0);
    assert_eq!(crate::vector::replay_count(), replays);
    // Le rectangle de 30 × 20 tourné de 90° autour de son centre : 20 × 30.
    assert!(f.vert(0, 55, 33) && f.vert(0, 55, 56) && !f.vert(0, 42, 45), "tourné de 90° autour de son centre");
}

#[test]
fn several_chosen_keys_move_the_aplat_of_every_key_in_one_undo_step() {
    let mut f = aplat_fix("cles");
    f.choose(&[0, 5, 10]);
    assert!(f.app.keys_mode());
    f.app.select_all();
    let sel = f.app.multi.sel.as_ref().expect("une sélection sur les trois aplats");
    assert_eq!(sel.parts.len(), 3);
    assert!(sel.parts.iter().all(|p| p.aplat) && sel.by_keys);
    assert_eq!(sel.summary(), "3 aplats, 3 zones", "{}", sel.summary());
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    let n0 = noms(&f.app).len();
    f.app.multi_nudge(vec2(15.0, 5.0));
    for k in 0..3 {
        assert!(f.vert(k, 50 + k * 6 + 15, 40 + 5) && !f.vert(k, 50 + k * 6, 40), "l'aplat de la clé {k} a bougé de (15, 5)");
        assert_eq!(f.trait_(k), traits[k], "le trait de la clé {k} n'est pas touché");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape pour les trois : {:?}", noms(&f.app));
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        assert!(f.vert(k, 50 + k * 6, 40) && !f.vert(k, 80 + k * 6, 45), "l'annulation rend l'aplat de la clé {k}");
    }
    // Un seul dessin sans aplat : écarté, avec sa raison.
    f.app.doc.cels.get_mut(&f.cels[2]).unwrap().fond = None;
    let t = f.app.multi_targets();
    assert_eq!(t.list.len(), 2);
    assert!(t.skipped_text().is_some_and(|s| s.contains("sans aplat")), "{:?}", t.skipped_text());
}

#[test]
fn deleting_the_aplat_selection_empties_the_aplat_not_the_line() {
    let mut f = aplat_fix("suppr");
    f.choose(&[0, 5]);
    f.app.select_all();
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    f.app.multi_delete();
    assert_eq!(peint(&f.peinture(0).to_flat()), 0);
    assert_eq!(peint(&f.peinture(1).to_flat()), 0);
    assert!(peint(&f.peinture(2).to_flat()) > 0, "la clé 10 n'était pas choisie");
    for k in 0..3 {
        assert_eq!(f.trait_(k), traits[k]);
    }
    f.app.run_action("edition.annuler");
    assert!(f.vert(0, 50, 40) && f.vert(1, 56, 40));
}

#[test]
fn the_keys_of_a_colour_layer_select_and_move_together_with_the_colour_layer_as_target() {
    let mut f = aplat_fix("couleur");
    // Un calque de couleur sous le calque « A » ; l'aplat de strate n'est plus visé.
    f.app.aplat_cible = None;
    f.app.cur_layer = 0;
    f.app.new_colour_layer();
    f.app.sync_couleurs();
    let colour = f.app.cur_layer;
    assert!(f.app.layers()[colour].couleur.is_some());
    let colour_id = f.app.layers()[colour].id;
    let mut cels = Vec::new();
    for fr in [0usize, 5, 10] {
        let c = f.app.layers()[colour].frames[fr].expect("la couleur suit le trait");
        remplir(&mut f.app.doc.cels.get_mut(&c).unwrap().raster, 30, 30, 50, 45, VERT);
        cels.push(c);
    }
    // Les clés choisies sont celles du calque de trait : la couleur est visée, ce sont ses dessins qui bougent.
    f.choose(&[0, 5, 10]);
    f.app.tool = Tool::Select;
    assert!(f.app.keys_mode());
    f.app.select_all();
    let sel = f.app.multi.sel.as_ref().expect("la sélection sur les trois dessins de couleur");
    assert_eq!(sel.parts.len(), 3);
    assert!(sel.parts.iter().all(|p| p.layer == colour_id && !p.aplat), "ce sont les dessins du calque de couleur");
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    let n0 = noms(&f.app).len();
    f.app.multi_nudge(vec2(10.0, 10.0));
    for c in &cels {
        let r = &f.app.doc.cels[c].raster;
        assert!(r.get(45, 45) == VERT && r.get(32, 32).a() == 0, "la couleur a bougé de (10, 10)");
    }
    for k in 0..3 {
        assert_eq!(f.trait_(k), traits[k], "le trait n'est pas touché");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "une étape : {:?}", noms(&f.app));
    f.app.run_action("edition.annuler");
    for c in &cels {
        assert!(f.app.doc.cels[c].raster.get(32, 32) == VERT);
    }
}

// ------------------------------------------------------------------------------------ plusieurs clés : peindre, remplir

const ROUGE: Color32 = Color32::from_rgb(200, 10, 10);

fn coupe_de(f: &AplatFix, k: usize) -> &Raster {
    &f.app.doc.cels[&f.cels[k]].fond.as_ref().expect("aplat").coupe
}

/// Un calque de couleur sous le calque « A » de `aplat_fix`, le calque de couleur courant ; ses dessins (images 0, 5, 10).
fn avec_calque_de_couleur(f: &mut AplatFix) -> (usize, Vec<CelId>) {
    f.app.aplat_cible = None;
    f.app.cur_layer = 0;
    f.app.new_colour_layer();
    f.app.sync_couleurs();
    let colour = f.app.cur_layer;
    let cels = [0usize, 5, 10].iter().map(|&fr| f.app.layers()[colour].frames[fr].expect("la couleur suit le trait")).collect();
    (colour, cels)
}

/// Enregistrer juste après un pot sur plusieurs clés : le fichier écrit a l'aplat de toutes les clés (l'enregistrement attend
/// le travail en route), rien n'arrive après lui.
#[test]
fn saving_right_after_a_multi_key_fill_waits_for_the_other_keys() {
    let mut f = aplat_fix("pot-cles-enregistrer");
    f.app.set_tool(Tool::Fill);
    f.choose(&[0, 5, 10]);
    f.app.color = [200, 10, 10];
    f.app.fill_at(pos2(30.0, 70.0));
    let dossier = std::env::temp_dir().join(format!("shot-aplat-enregistrer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    assert!(f.app.save_to_blocking(dossier.join("essai.shot")), "{}", f.app.status);
    assert_eq!(f.app.aplat_travaux.en_route, 0, "plus rien en route après l'enregistrement");
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(30, 70), ROUGE, "l'aplat de la clé {k} est rempli avant l'écriture");
    }
    assert!(!f.app.modified, "rien n'est arrivé après l'enregistrement");
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn the_bucket_fills_the_same_zone_of_the_aplat_of_every_chosen_key_in_one_step() {
    let mut f = aplat_fix("pot-cles");
    f.app.set_tool(Tool::Fill);
    f.choose(&[0, 5, 10]);
    f.app.color = [200, 10, 10];
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    let n0 = noms(&f.app).len();
    f.app.fill_at(pos2(30.0, 70.0));
    // La clé qu'on voit est remplie tout de suite ; les autres sur un fil de travail, sans attendre.
    assert_eq!(f.peinture(0).get(30, 70), ROUGE, "la clé qu'on voit : {}", f.app.status);
    assert!(f.app.aplat_travaux.en_route > 0, "les autres se calculent en arrière-plan : {}", f.app.status);
    f.app.attendre_aplat_multi();
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(30, 70), ROUGE, "l'aplat de la clé {k} est rempli");
        assert_eq!(f.trait_(k), traits[k], "le trait de la clé {k} n'est pas touché");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape pour les trois : {:?}", noms(&f.app));
    assert_eq!(noms(&f.app).last().map(String::as_str), Some("Aplat : remplir"));
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(30, 70).a(), 0, "l'annulation rend l'aplat de la clé {k}");
    }
    // Une seule clé choisie : rien d'autre ne change.
    f.choose(&[0]);
    f.app.fill_at(pos2(30.0, 70.0));
    assert!(f.app.aplat_travaux.en_route == 0);
    f.app.attendre_aplat_multi();
    assert_eq!(f.peinture(0).get(30, 70), ROUGE);
    assert_eq!(f.peinture(1).get(30, 70).a(), 0);
    // La clé qu'on voit n'est pas parmi les clés choisies : rien d'autre non plus.
    f.app.run_action("edition.annuler");
    f.choose(&[5, 10]);
    f.app.fill_at(pos2(30.0, 70.0));
    f.app.attendre_aplat_multi();
    assert_eq!(f.peinture(1).get(30, 70).a(), 0, "les clés choisies sans celle qu'on voit ne sont pas touchées");
}

#[test]
fn the_cutter_removes_the_zone_from_the_aplat_of_every_chosen_key() {
    let mut f = aplat_fix("cutter-cles");
    f.app.set_tool(Tool::Cutter);
    f.choose(&[0, 5, 10]);
    let n0 = noms(&f.app).len();
    f.app.cutter_press(pos2(30.0, 70.0));
    f.app.attendre_aplat_multi();
    for k in 0..3 {
        assert!(coupe_de(&f, k).get(30, 70).a() > 0, "la zone est retirée de l'aplat de la clé {k}");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape : {:?}", noms(&f.app));
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        assert_eq!(coupe_de(&f, k).get(30, 70).a(), 0);
    }
}

#[test]
fn a_brush_stroke_on_the_aplat_is_made_on_every_chosen_key_in_one_step_without_replay() {
    let mut f = aplat_fix("trait-cles");
    f.app.set_tool(Tool::Brush);
    f.choose(&[0, 5, 10]);
    f.app.color = [10, 10, 220];
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    let (replays, n0) = (crate::vector::replay_count(), noms(&f.app).len());
    gliss(&mut f.app, pos2(30.0, 70.0), pos2(80.0, 70.0));
    for k in 0..3 {
        let bleu = (30..80).filter(|&x| f.peinture(k).get(x, 70).b() > 150 && f.peinture(k).get(x, 70).r() < 80).count();
        assert!(bleu > 20, "l'aplat de la clé {k} a reçu le trait ({bleu} pixels bleus)");
        assert_eq!(f.trait_(k), traits[k], "le trait de la clé {k} n'est pas touché");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "un geste, une étape pour les trois : {:?}", noms(&f.app));
    assert_eq!(noms(&f.app).last().map(String::as_str), Some("Aplat : peinture"));
    assert_eq!(crate::vector::replay_count(), replays, "aucun rejeu du moteur sur le fil de l'interface");
    f.app.run_action("edition.annuler");
    for k in 0..3 {
        assert!((30..80).all(|x| f.peinture(k).get(x, 70).b() < 150), "l'annulation rend l'aplat de la clé {k}");
    }
    assert!(f.vert(1, 56, 40), "et la peinture d'avant est toujours là");
    // La gomme coupe l'aplat de chaque clé choisie.
    f.app.set_tool(Tool::Eraser);
    gliss(&mut f.app, pos2(30.0, 100.0), pos2(80.0, 100.0));
    for k in 0..3 {
        assert!((30..80).any(|x| coupe_de(&f, k).get(x, 100).a() > 0), "la gomme a coupé l'aplat de la clé {k}");
    }
}

#[test]
fn painting_on_the_aplat_toggle_also_works_on_every_chosen_key() {
    let mut f = aplat_fix("sur-aplat-cles");
    // Pas de sous-ligne choisie : c'est « Peint sur l'aplat » qui vise l'aplat, depuis le trait.
    f.app.aplat_cible = None;
    f.app.set_tool(Tool::Brush);
    f.app.paint_on_aplat = true;
    f.choose(&[0, 5, 10]);
    f.app.color = [10, 10, 220];
    let n0 = noms(&f.app).len();
    let traits: Vec<Vec<Color32>> = (0..3).map(|k| f.trait_(k)).collect();
    gliss(&mut f.app, pos2(30.0, 70.0), pos2(80.0, 70.0));
    for k in 0..3 {
        assert!((30..80).any(|x| f.peinture(k).get(x, 70).b() > 150), "l'aplat de la clé {k} a reçu le trait");
        assert_eq!(f.trait_(k), traits[k]);
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "{:?}", noms(&f.app));
}

#[test]
fn a_colour_layer_gets_the_same_stroke_and_the_same_fill_on_every_chosen_key() {
    let mut f = aplat_fix("couleur-cles");
    let (colour, cels) = avec_calque_de_couleur(&mut f);
    // Les clés choisies sont celles du calque de trait : la couleur est visée.
    f.choose(&[0, 5, 10]);
    f.app.set_tool(Tool::Brush);
    f.app.color = [10, 10, 220];
    let (n0, replays) = (noms(&f.app).len(), crate::vector::replay_count());
    gliss(&mut f.app, pos2(30.0, 70.0), pos2(80.0, 70.0));
    for c in &cels {
        let r = &f.app.doc.cels[c].raster;
        assert!((30..80).any(|x| r.get(x, 70).b() > 150), "le trait de couleur est sur chaque clé");
    }
    assert_eq!(f.app.cur_layer, colour);
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape : {:?}", noms(&f.app));
    assert_eq!(crate::vector::replay_count(), replays);
    f.app.run_action("edition.annuler");
    for c in &cels {
        assert!(f.app.doc.cels[c].raster.is_blank(), "annulé sur chaque clé");
    }
    // Le pot de peinture : la zone fermée par le trait de chaque clé.
    f.app.set_tool(Tool::Fill);
    f.app.color = [200, 10, 10];
    let n1 = noms(&f.app).len();
    f.app.fill_at(pos2(30.0, 90.0));
    f.app.attendre_aplat_multi();
    // (Le point (30, 90) est hors de l'anneau : le dehors se remplit ; (30, 70) est dedans.)
    f.app.fill_at(pos2(30.0, 70.0));
    f.app.attendre_aplat_multi();
    for c in &cels {
        assert_eq!(f.app.doc.cels[c].raster.get(30, 70), ROUGE, "la zone est remplie sur chaque clé");
    }
    assert_eq!(noms(&f.app).len(), n1 + 2, "une étape par clic : {:?}", noms(&f.app));
}

#[test]
fn a_drawing_touched_while_the_fill_is_computed_is_left_alone() {
    let mut f = aplat_fix("retouche");
    f.app.set_tool(Tool::Fill);
    f.choose(&[0, 5]);
    f.app.color = [200, 10, 10];
    f.app.fill_at(pos2(30.0, 70.0));
    // La clé 5 est retouchée avant l'arrivée du résultat : il est abandonné pour elle, avec un mot dans la barre d'état.
    f.app.doc.cels.get_mut(&f.cels[1]).unwrap().version += 1;
    f.app.attendre_aplat_multi();
    assert_eq!(f.peinture(1).get(30, 70).a(), 0, "le dessin retouché n'est pas écrasé");
    assert_eq!(f.peinture(0).get(30, 70), ROUGE);
    assert!(f.app.status.contains("retouchée"), "{}", f.app.status);
}

#[test]
fn a_fill_result_waits_while_its_drawing_is_being_drawn_on() {
    // Partie 132, relecture : un résultat qui arrive pendant un trait sur son dessin l'écraserait (la version ne monte qu'au lâcher).
    let mut f = aplat_fix("trait-en-cours");
    f.app.set_tool(Tool::Fill);
    f.choose(&[0, 5]);
    f.app.color = [200, 10, 10];
    f.app.fill_at(pos2(30.0, 70.0));
    let s = f.app.brush;
    f.app.stroke = Some(crate::brush::StrokeState::new(f.cels[1], crate::brush::StrokeMode::Erase, s, None, false));
    let fin = std::time::Instant::now() + std::time::Duration::from_millis(400);
    while std::time::Instant::now() < fin {
        f.app.poll_aplat_multi();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(f.peinture(1).get(30, 70).a(), 0, "pas posé pendant le trait sur ce dessin");
    f.app.stroke = None;
    f.app.attendre_aplat_multi();
    assert_eq!(f.peinture(1).get(30, 70), ROUGE, "posé à la fin du trait");
}

#[test]
fn a_bucket_on_a_single_key_never_starts_a_worker() {
    let mut f = aplat_fix("un-seul");
    f.app.set_tool(Tool::Fill);
    f.app.fill_at(pos2(30.0, 70.0));
    assert_eq!(f.app.aplat_travaux.en_route, 0);
    assert!(f.app.aplat_miroirs.est_vide());
    gliss(&mut f.app, pos2(30.0, 70.0), pos2(80.0, 70.0));
    assert!(f.app.aplat_miroirs.est_vide());
}

#[test]
fn a_colour_stroke_is_kept_inside_the_closed_zone_of_every_chosen_key() {
    let mut f = aplat_fix("zone-cles");
    let (_, cels) = avec_calque_de_couleur(&mut f);
    f.choose(&[0, 5, 10]);
    f.app.set_tool(Tool::Brush);
    f.app.color = [10, 10, 220];
    let n0 = noms(&f.app).len();
    // Un trait qui part de l'intérieur de l'anneau (x de 20 à 100) et sort largement au-delà.
    gliss(&mut f.app, pos2(30.0, 50.0), pos2(160.0, 50.0));
    f.app.attendre_aplat_multi();
    for (k, c) in cels.iter().enumerate() {
        let r = &f.app.doc.cels[c].raster;
        assert!(r.get(60, 50).b() > 150, "la clé {k} est peinte dans la zone");
        assert_eq!(r.get(130, 50).a(), 0, "la clé {k} : le trait ne sort pas de la zone fermée par son trait");
    }
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape : {:?}", noms(&f.app));
    f.app.run_action("edition.annuler");
    for c in &cels {
        assert!(f.app.doc.cels[c].raster.is_blank(), "une annulation rend toutes les clés");
    }
    // Annulé avant l'arrivée de la zone : rien n'est ressuscité.
    gliss(&mut f.app, pos2(30.0, 50.0), pos2(160.0, 50.0));
    f.app.run_action("edition.annuler");
    f.app.attendre_aplat_multi();
    for c in &cels {
        assert!(f.app.doc.cels[c].raster.is_blank(), "le résultat d'un geste annulé est abandonné");
    }
}

// ------------------------------------------------------------------------------------ la portée du changement de couleur

use crate::aplatcouleur::AplatPortee;

const NEUF: [u8; 3] = [200, 10, 10];

/// Comme `aplat_fix`, avec un aplat vert (la couleur du calque, et celle de la peinture de chaque dessin).
fn fix_vert(name: &str) -> AplatFix {
    let mut f = aplat_fix(name);
    f.app.aplat_cible = None;
    f.app.layers_mut()[0].fond = Some(FondReglages { couleur: [10, 200, 10], ..FondReglages::default() });
    f
}

fn change(f: &mut AplatFix, portee: AplatPortee) -> usize {
    f.app.aplat_portee = portee;
    f.app.choisir_couleur_aplat(0, NEUF, 1);
    assert_eq!(f.app.couleur_attente(), Some(NEUF), "la couleur est retenue jusqu'au lâcher");
    let n = f.app.appliquer_couleur_aplat();
    f.app.attendre_aplat_multi();
    n
}

#[test]
fn the_colour_scope_defaults_to_the_key_and_recolours_that_key_only() {
    let mut f = fix_vert("portee-cle");
    assert_eq!(f.app.aplat_portee, AplatPortee::Cle);
    let n0 = noms(&f.app).len();
    assert_eq!(change(&mut f, AplatPortee::Cle), 1);
    // La clé qu'on voit : sa peinture a pris la nouvelle couleur, et sa forme aussi ; les autres n'ont pas bougé.
    assert_eq!(f.peinture(0).get(50, 40), ROUGE, "la peinture de la clé a changé de couleur");
    assert!(f.peinture(0).get(30, 70).a() > 0 && f.peinture(0).get(30, 70).r() > 150, "sa forme prend la nouvelle couleur");
    for k in 1..3 {
        assert_eq!(f.peinture(k).get(50 + 6 * k, 40), VERT, "la clé {k} garde sa couleur");
        assert_eq!(f.peinture(k).get(30, 70).a(), 0, "et sa forme");
    }
    assert_eq!(f.app.layers()[0].fond.unwrap().couleur, [10, 200, 10], "la couleur du calque ne change pas : les autres clés gardent la leur");
    assert!(f.app.status.contains("1 dessin") && f.app.status.contains("la clé"), "{}", f.app.status);
    assert_eq!(noms(&f.app).len(), n0 + 1, "une seule étape : {:?}", noms(&f.app));
    assert_eq!(noms(&f.app).last().map(String::as_str), Some("Aplat : couleur"));
    f.app.run_action("edition.annuler");
    assert_eq!(f.peinture(0).get(50, 40), VERT);
    assert_eq!(f.peinture(0).get(30, 70).a(), 0, "l'annulation rend aussi la forme");
}

#[test]
fn the_key_scope_follows_the_chosen_keys() {
    let mut f = fix_vert("portee-cles");
    f.choose(&[0, 5, 10]);
    assert_eq!(change(&mut f, AplatPortee::Cle), 3);
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(50 + 6 * k, 40).r(), 200, "la clé {k}");
    }
    assert!(f.app.status.contains("3 dessins"), "{}", f.app.status);
}

#[test]
fn the_plan_scope_recolours_every_drawing_of_the_plan_and_the_colour_of_the_layer() {
    let mut f = fix_vert("portee-plan");
    let n0 = noms(&f.app).len();
    assert_eq!(change(&mut f, AplatPortee::Plan), 3);
    assert_eq!(f.app.layers()[0].fond.unwrap().couleur, NEUF, "la couleur du calque");
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(50 + 6 * k, 40).r(), 200, "la peinture de la clé {k}");
    }
    assert!(f.app.status.contains("3 dessins") && f.app.status.contains("le plan"), "{}", f.app.status);
    assert_eq!(noms(&f.app).len(), n0 + 1, "{:?}", noms(&f.app));
    f.app.run_action("edition.annuler");
    assert_eq!(f.app.layers()[0].fond.unwrap().couleur, [10, 200, 10]);
    for k in 0..3 {
        assert_eq!(f.peinture(k).get(50 + 6 * k, 40), VERT);
    }
    // Une autre couleur d'aplat n'est pas touchée : seul ce qui avait l'ancienne couleur change.
    f.app.layers_mut()[0].fond = Some(FondReglages { couleur: [0, 0, 255], ..FondReglages::default() });
    f.app.choisir_couleur_aplat(0, NEUF, 2);
    f.app.aplat_couleur_attente.as_mut().unwrap().set_old_for_test([10, 200, 10]);
    f.app.aplat_portee = AplatPortee::Plan;
    f.app.appliquer_couleur_aplat();
    assert_eq!(f.app.layers()[0].fond.unwrap().couleur, [0, 0, 255], "un aplat d'une autre couleur ne change pas");
}

#[test]
fn the_storyboard_scope_reaches_every_plan_and_counts_the_drawings() {
    let mut f = fix_vert("portee-storyboard");
    f.app.doc.duplicate_panel_after(0);
    assert_eq!(f.app.doc.panels.len(), 2);
    let n = change(&mut f, AplatPortee::Storyboard);
    assert_eq!(n, 6, "trois dessins dans chacun des deux plans : {}", f.app.status);
    for pi in 0..2 {
        assert_eq!(f.app.doc.panels[pi].layers[0].fond.unwrap().couleur, NEUF, "plan {pi}");
    }
    assert!(f.app.status.contains("6 dessins") && f.app.status.contains("tout le storyboard"), "{}", f.app.status);
    f.app.run_action("edition.annuler");
    for pi in 0..2 {
        assert_eq!(f.app.doc.panels[pi].layers[0].fond.unwrap().couleur, [10, 200, 10]);
    }
    // Le plan seul ne touche pas l'autre plan.
    f.app.aplat_portee = AplatPortee::Plan;
    f.app.choisir_couleur_aplat(0, NEUF, 3);
    assert_eq!(f.app.appliquer_couleur_aplat(), 3);
    assert_eq!(f.app.doc.panels[1].layers[0].fond.unwrap().couleur, [10, 200, 10], "l'autre plan n'a pas bougé");
}

#[test]
fn a_colour_layer_is_recoloured_by_the_plan_scope_and_nothing_changes_without_the_old_colour() {
    let mut f = fix_vert("portee-couleur");
    let (_, cels) = avec_calque_de_couleur(&mut f);
    for c in &cels {
        remplir(&mut f.app.doc.cels.get_mut(c).unwrap().raster, 30, 30, 50, 45, VERT);
    }
    f.app.cur_layer = 1;
    assert_eq!(change_layer(&mut f, 1, AplatPortee::Plan), 6, "trois dessins de couleur et trois strates : {}", f.app.status);
    for c in &cels {
        assert_eq!(f.app.doc.cels[c].raster.get(40, 40).r(), 200, "le calque de couleur est recoloré, opacité gardée");
        assert_eq!(f.app.doc.cels[c].raster.get(40, 40).a(), 255);
    }
    // Plus rien n'a l'ancienne couleur : aucun dessin, aucune étape de plus.
    let n0 = noms(&f.app).len();
    f.app.aplat_portee = AplatPortee::Plan;
    f.app.choisir_couleur_aplat(1, [1, 2, 3], 9);
    f.app.aplat_couleur_attente.as_mut().unwrap().set_old_for_test([99, 99, 99]);
    assert_eq!(f.app.appliquer_couleur_aplat(), 0);
    assert_eq!(noms(&f.app).len(), n0, "pas d'étape pour rien");
    assert!(f.app.status.contains("aucun dessin"), "{}", f.app.status);
}

fn change_layer(f: &mut AplatFix, li: usize, portee: AplatPortee) -> usize {
    f.app.aplat_portee = portee;
    f.app.choisir_couleur_aplat(li, NEUF, 1);
    let n = f.app.appliquer_couleur_aplat();
    f.app.attendre_aplat_multi();
    n
}

#[test]
fn the_colour_choice_is_held_while_dragging_and_laid_down_when_released() {
    let mut f = fix_vert("portee-glisse");
    let ctx = egui::Context::default();
    f.app.choisir_couleur_aplat(0, [1, 1, 1], 5);
    f.app.choisir_couleur_aplat(0, NEUF, 6);
    assert_eq!(f.app.layers()[0].fond.unwrap().couleur, [10, 200, 10], "rien n'est posé pendant le glissé");
    // (Une image plus tard, bouton relâché : posé.)
    for _ in 0..8 {
        let _ = ctx.run(Default::default(), |_| {});
    }
    f.app.tick_aplat_couleur(&ctx);
    assert!(f.app.couleur_attente().is_none(), "posée au lâcher");
    assert!(f.app.status.contains("dessin"), "{}", f.app.status);
    f.app.attendre_aplat_multi();
    assert_eq!(f.peinture(0).get(50, 40).r(), 200);
}
