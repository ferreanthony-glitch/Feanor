//! Retour de terrain d'Anthony (2026-10-28, branche `aplat-trous`) : après une retouche du dessin, des zones blanches entre le trait et
//! l'aplat brun. Ces zones FONT PARTIE de l'aplat : c'est la « Couleur de l'aplat » du calque, restée blanche, alors que la pastille
//! (personnage, couleur neutre) peignait son aplat brun dans la strate. Tout morceau d'aplat gagné autrement que par un trait de la
//! pastille (gomme, transformation, Cutter) prenait la couleur du calque. Ces essais refont les gestes d'Anthony sur un calque vectoriel
//! dont l'aplat est blanc, avec une pastille à aplat brun, et comptent les pixels de l'aplat qui ne sont pas bruns.

use eframe::egui::{pos2, vec2, Color32, Pos2};

use crate::app::{App, Tool};
use crate::document::{Document, Layer, LayerKind};
use crate::fond::{self, FondReglages};
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::personnages::{self, Neutre};
use crate::personnagesui::Choix;
use crate::render;
use crate::vtools::VSelection;

const BRUN: [u8; 3] = [140, 90, 70];
const TRAIT: [u8; 3] = [30, 20, 20];
const BLANC: [u8; 3] = [255, 255, 255];
const VERT: [u8; 3] = [60, 230, 60];

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Un calque vectoriel « Dessin » dont l'aplat est allumé, blanc (celui d'un aplat neuf), débord 3, fermer les trous 6, le bord de
/// l'image ferme la forme : les réglages d'Anthony. Une pastille « Lou » : trait sombre, aplat brun.
fn banc_dessin() -> (Banc, u64) {
    crate::fxcache::monde::ecrire();
    let mut doc = Document::new(400, 300, 24);
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.fond = Some(FondReglages { couleur: BLANC, debord: 3, trous: 6, bord: true, intelligent: false });
    doc.panels[0].layers[0] = l;
    let mut b = banc(doc, false);
    b.app.tool = Tool::Brush;
    let lou = b.app.perso_ajouter().expect("personnage");
    b.app.perso_ecrire(Choix::Personnage(lou), "Lou".into(), TRAIT, Some(BRUN));
    b.app.perso_choisir(Choix::Personnage(lou));
    (b, lou)
}

/// Un trait de pinceau le long de `points` (une main : un point tous les 4 pixels).
fn polyligne(app: &mut App, points: &[(f32, f32)]) {
    let mut pts: Vec<Pos2> = Vec::new();
    for w in points.windows(2) {
        let (a, z) = (pos2(w[0].0, w[0].1), pos2(w[1].0, w[1].1));
        let n = (a.distance(z) / 4.0).ceil().max(1.0) as usize;
        for k in 0..n {
            pts.push(a + (z - a) * (k as f32 / n as f32));
        }
    }
    pts.push(pos2(points[points.len() - 1].0, points[points.len() - 1].1));
    app.begin_paint(pts[0], 1.0, false);
    for p in &pts[1..] {
        app.stroke_to(*p, 1.0);
    }
    app.end_stroke();
}

/// Un anneau carré fermé de `x0, y0` à `x1, y1`, d'un seul trait.
fn anneau(app: &mut App, x0: f32, y0: f32, x1: f32, y1: f32) {
    polyligne(app, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0 - 8.0)]);
}

fn attendre(b: &mut Banc) {
    b.app.perso_attendre();
    se_reposer(b, 60);
    b.app.perso_attendre();
}

fn etapes(app: &App) -> usize {
    app.history.names().0.len()
}

fn couleur_du_calque(app: &App) -> [u8; 3] {
    app.layers()[0].fond.expect("l'aplat du calque").couleur
}

/// Les pixels de l'aplat (couverture pleine, hors du trait) vus comme à l'écran et à l'export (`composite_pixel`) : (combien,
/// combien n'ont pas la couleur `attendue`).
fn aplat_hors_couleur(app: &App, attendue: [u8; 3]) -> (usize, usize) {
    let cel = app.doc.panels[0].layers[0].frames.iter().flatten().next().copied().expect("le dessin");
    let c = &app.doc.cels[&cel];
    let r = app.layers()[0].fond.expect("aplat");
    let couverture = fond::silhouette(&c.raster, &r);
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    let (w, h) = (c.raster.width, c.raster.height);
    let (mut dedans, mut mauvais) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            // (Le trait et ce qu'il touche, même faiblement, se mêlent au fond : on ne compte que l'aplat seul.)
            if couverture[y * w + x] == 255 && c.raster.get(x, y).a() == 0 {
                dedans += 1;
                if render::composite_pixel(&app.doc, &plan, x, y) != rgb(attendue) {
                    mauvais += 1;
                }
            }
        }
    }
    (dedans, mauvais)
}

/// L'aplat est partout de la couleur attendue, et il y en a.
fn aucun_pixel_d_une_autre_couleur(app: &App, attendue: [u8; 3], quand: &str) {
    let (dedans, mauvais) = aplat_hors_couleur(app, attendue);
    assert!(dedans > 500, "{quand} : il y a de l'aplat ({dedans} pixels)");
    assert_eq!(mauvais, 0, "{quand} : {mauvais} pixels de l'aplat sur {dedans} ne sont pas de la couleur de la pastille");
}

// ------------------------------------------------------------------------------------ la cause : la couleur du calque

#[test]
fn a_pastille_stroke_gives_its_aplat_colour_to_the_layer_in_the_same_undo_step() {
    let (mut b, _) = banc_dessin();
    assert_eq!(couleur_du_calque(&b.app), BLANC, "l'aplat du calque est blanc au départ");
    let avant = etapes(&b.app);
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    attendre(&mut b);
    assert_eq!(couleur_du_calque(&b.app), BRUN, "la couleur d'aplat de la pastille est celle du calque");
    assert_eq!(etapes(&b.app), avant + 1, "le trait, sa peinture d'aplat et la couleur du calque : une seule étape");
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après le trait");
    // Une seule annulation remet tout : le trait, l'aplat et le blanc du calque.
    b.app.undo();
    assert_eq!(etapes(&b.app), avant);
    assert_eq!(couleur_du_calque(&b.app), BLANC, "la couleur du calque est revenue");
    let vide = b.app.doc.panels[0].layers[0].frames.iter().flatten().all(|c| b.app.doc.cels.get(c).is_none_or(|c| c.is_empty()));
    assert!(vide, "le trait n'est plus là");
    // Rétablir rend tout.
    b.app.redo();
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après rétablir");
    assert_eq!(couleur_du_calque(&b.app), BRUN);
}

#[test]
fn a_brush_without_aplat_colour_leaves_the_layer_colour_alone() {
    let (mut b, _) = banc_dessin();
    // Un calque d'aplat vert fluo (réglé à la main) : la pastille « classique » et une pastille sans aplat n'y touchent pas.
    b.app.layers_mut()[0].fond.as_mut().unwrap().couleur = VERT;
    b.app.perso_classique();
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    assert_eq!(couleur_du_calque(&b.app), VERT, "la pastille classique ne change rien");
    let sans = b.app.perso_ajouter().expect("personnage sans aplat");
    b.app.perso_ecrire(Choix::Personnage(sans), "Fantôme".into(), TRAIT, None);
    b.app.perso_choisir(Choix::Personnage(sans));
    anneau(&mut b.app, 200.0, 60.0, 300.0, 160.0);
    attendre(&mut b);
    assert_eq!(couleur_du_calque(&b.app), VERT, "un personnage sans aplat non plus");
    // La gomme, avec une pastille à aplat choisie, ne change pas non plus la couleur.
    let (mut b, _) = banc_dessin();
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    attendre(&mut b);
    b.app.layers_mut()[0].fond.as_mut().unwrap().couleur = VERT;
    b.app.tool = Tool::Eraser;
    polyligne(&mut b.app, &[(60.0, 100.0), (60.0, 120.0)]);
    assert_eq!(couleur_du_calque(&b.app), VERT, "la gomme ne change pas la couleur d'aplat du calque");
}

// ------------------------------------------------------------------------------------ les gestes d'Anthony

#[test]
fn drawing_over_or_extending_the_outline_with_the_brush_leaves_no_white() {
    let (mut b, _) = banc_dessin();
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    attendre(&mut b);
    // Un trait qui prolonge le contour vers l'extérieur et le referme : un lobe, collé au côté droit.
    polyligne(&mut b.app, &[(160.0, 90.0), (230.0, 90.0), (230.0, 140.0), (160.0, 140.0)]);
    attendre(&mut b);
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après le lobe au pinceau");
    assert_eq!(couleur_du_calque(&b.app), BRUN);
}

#[test]
fn erasing_a_piece_of_the_outline_then_redrawing_it_elsewhere_leaves_no_white() {
    let (mut b, _) = banc_dessin();
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    attendre(&mut b);
    // La gomme enlève un bout du côté gauche...
    b.app.tool = Tool::Eraser;
    polyligne(&mut b.app, &[(60.0, 90.0), (60.0, 135.0)]);
    attendre(&mut b);
    // ...et le pinceau le refait en l'écartant vers la gauche.
    b.app.tool = Tool::Brush;
    polyligne(&mut b.app, &[(60.0, 80.0), (30.0, 90.0), (30.0, 140.0), (60.0, 150.0)]);
    attendre(&mut b);
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après la gomme et le trait refait");
    assert_eq!(couleur_du_calque(&b.app), BRUN);
}

#[test]
fn moving_a_selected_part_of_the_drawing_leaves_no_white_and_is_one_undo_step() {
    let (mut b, _) = banc_dessin();
    anneau(&mut b.app, 40.0, 60.0, 140.0, 160.0);
    anneau(&mut b.app, 200.0, 150.0, 300.0, 250.0);
    attendre(&mut b);
    let cel = b.app.cel_here().expect("le dessin");
    assert_eq!(b.app.doc.cels[&cel].vector.as_ref().map(|v| v.elements.len()), Some(2), "deux traits");
    // La sélection du second anneau, déplacée vers le haut à droite : sa forme neuve n'a jamais été peinte par la pastille.
    b.app.tool = Tool::Select;
    b.app.vsel = Some(VSelection { cel, items: vec![1] });
    let avant = etapes(&b.app);
    b.app.nudge_selection(vec2(40.0, -70.0));
    attendre(&mut b);
    assert_eq!(etapes(&b.app), avant + 1, "le déplacement est une étape");
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après le déplacement");
    assert_eq!(couleur_du_calque(&b.app), BRUN);
    b.app.undo();
    assert_eq!(etapes(&b.app), avant);
}

#[test]
fn cutting_a_piece_that_sticks_out_leaves_no_white() {
    let (mut b, _) = banc_dessin();
    anneau(&mut b.app, 60.0, 60.0, 160.0, 160.0);
    // Un trait qui dépasse du contour, à droite.
    polyligne(&mut b.app, &[(110.0, 110.0), (210.0, 110.0)]);
    attendre(&mut b);
    b.app.set_tool(Tool::Cutter);
    let avant = etapes(&b.app);
    b.app.cutter_press(pos2(190.0, 90.0));
    for y in [95.0, 100.0, 105.0, 110.0, 115.0, 120.0, 125.0] {
        b.app.cutter_drag(pos2(190.0, y));
    }
    b.app.end_stroke();
    attendre(&mut b);
    assert_eq!(etapes(&b.app), avant + 1, "le cutter coupe en une étape");
    aucun_pixel_d_une_autre_couleur(&b.app, BRUN, "après le cutter");
    assert_eq!(couleur_du_calque(&b.app), BRUN);
}


// ------------------------------------------------------------------------------------ une couleur neutre, dans le calque courant

fn neutre_a_aplat(b: &Banc) -> (usize, [u8; 3]) {
    let i = b.app.settings.neutral_colors.iter().position(|n| n.fill.is_some()).expect("une couleur neutre à aplat");
    let fond = b.app.settings.neutral_colors[i].fill.expect("aplat");
    (i, fond)
}

#[test]
fn a_neutral_colour_with_an_aplat_gives_it_to_the_current_layer_and_leaves_no_white_after_the_gestures() {
    let (mut b, _) = banc_dessin();
    let (i, gris) = neutre_a_aplat(&b);
    let couches = b.app.layers().len();
    b.app.perso_choisir(Choix::Neutre(i));
    anneau(&mut b.app, 40.0, 60.0, 140.0, 160.0);
    anneau(&mut b.app, 200.0, 150.0, 300.0, 250.0);
    attendre(&mut b);
    assert_eq!(b.app.layers().len(), couches, "une couleur neutre ne crée pas de calque : elle dessine dans le calque courant");
    assert_eq!(couleur_du_calque(&b.app), gris, "la couleur d'aplat de la couleur neutre est celle du calque courant");
    let cel = b.app.cel_here().expect("le dessin");
    b.app.tool = Tool::Select;
    b.app.vsel = Some(VSelection { cel, items: vec![1] });
    b.app.nudge_selection(vec2(40.0, -70.0));
    attendre(&mut b);
    aucun_pixel_d_une_autre_couleur(&b.app, gris, "après le déplacement");
}

#[test]
fn a_neutral_colour_on_a_layer_without_aplat_starts_it_in_its_own_colour() {
    // Le calque n'a pas d'aplat : la couleur neutre à aplat l'allume (« Aplat du personnage », étape à part), à SA couleur, pas au blanc.
    crate::fxcache::monde::ecrire();
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers[0] = Layer::new("Dessin", LayerKind::Vector);
    let mut b = banc(doc, false);
    b.app.tool = Tool::Brush;
    let (i, gris) = neutre_a_aplat(&b);
    b.app.perso_choisir(Choix::Neutre(i));
    anneau(&mut b.app, 40.0, 60.0, 140.0, 160.0);
    attendre(&mut b);
    assert_eq!(couleur_du_calque(&b.app), gris);
    aucun_pixel_d_une_autre_couleur(&b.app, gris, "sur un calque sans aplat");
}

// ------------------------------------------------------------------------------------ les préréglages d'origine

#[test]
fn the_original_neutral_colours_are_three_greys_darker_line_on_lighter_aplat_getting_lighter() {
    let d = personnages::neutres_par_defaut();
    assert_eq!(d.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(), ["Gris 1", "Gris 2", "Gris 3"]);
    assert_eq!(d[0].line, [0x1E; 3]);
    assert_eq!(d[0].fill, Some([0x8C; 3]));
    assert_eq!((d[1].line, d[1].fill), ([0x4A; 3], Some([0xB4; 3])));
    assert_eq!((d[2].line, d[2].fill), ([0x7A; 3], Some([0xDA; 3])));
    for n in &d {
        assert!(n.line[0] < n.fill.expect("aplat")[0], "le trait est plus foncé que l'aplat : {n:?}");
    }
    for w in d.windows(2) {
        assert!(w[0].line[0] < w[1].line[0] && w[0].fill.unwrap()[0] < w[1].fill.unwrap()[0], "de plus en plus clairs");
    }
}

#[test]
fn the_reset_button_of_the_neutral_colours_gives_the_three_greys_and_leaves_the_project_alone() {
    let (mut b, lou) = banc_dessin();
    b.app.settings.neutral_colors = vec![Neutre { name: "Sépia".into(), line: [100, 60, 30], fill: None }];
    b.app.perso_choisir(Choix::Neutre(0));
    let personnages = b.app.doc.personnages.clone();
    b.app.perso_neutres_d_origine();
    assert_eq!(b.app.settings.neutral_colors, personnages::neutres_par_defaut());
    assert!(b.app.settings_a_ecrire, "le réglage de l'application s'écrit");
    assert_eq!(b.app.perso.actif, None, "la couleur choisie n'existe plus");
    assert_eq!(b.app.doc.personnages, personnages, "les personnages du projet ne bougent pas");
    assert!(b.app.doc.personnages.iter().any(|p| p.id == lou));
}

#[test]
fn a_new_project_has_three_original_characters_and_an_existing_one_is_left_alone() {
    let doc = crate::nouveau::build_document(&crate::nouveau::Values::default(), "Neuf");
    let noms: Vec<&str> = doc.personnages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(noms, ["Perso 1", "Perso 2", "Perso 3"]);
    assert_eq!((doc.personnages[0].line, doc.personnages[0].fill), ([39, 2, 2], Some([150, 97, 97])));
    assert_eq!((doc.personnages[1].line, doc.personnages[1].fill), ([0, 28, 46], Some([78, 110, 145])));
    assert_eq!((doc.personnages[2].line, doc.personnages[2].fill), ([19, 39, 17], Some([93, 114, 96])));
    let mut ids: Vec<u64> = doc.personnages.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3, "trois numéros différents");
    // Un projet qui existe (ici : un document vide, comme à l'ouverture d'un ancien) n'en reçoit pas.
    assert!(Document::new(400, 300, 24).personnages.is_empty());
    // Les personnages d'un projet neuf s'enregistrent et se relisent.
    let dir = std::env::temp_dir().join(format!("shot-test-aplat-trous-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    let manifeste = dir.join("P.pelure");
    crate::project::save(&doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    assert_eq!(relu.doc.personnages, doc.personnages);
    std::fs::remove_dir_all(&dir).ok();
}