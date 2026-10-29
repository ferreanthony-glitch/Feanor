//! Toile infinie : un dessin fait en partie hors de la feuille ne doit être coupé nulle part (Anthony, 2026-10-28 :
//! « bug d'affichage quand on dessine trop loin sur le canevas, même quand on déplace la caméra ensuite »). Le trait
//! vectoriel ou bitmap, son aplat (le fond sous le trait), l'image que filme la caméra déplacée sur lui.

use eframe::egui::{self, pos2, Color32};

use crate::app::{App, Tool};
use crate::camera::Camera;
use crate::document::{Document, Layer, LayerKind};
use crate::fond::FondReglages;
use crate::render;

const ROSE: [u8; 3] = [240, 190, 200];

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-toile-coupee-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

/// Un plan 1920 × 1080 à un calque de ce genre (avec ou sans aplat rose sous le trait), pinceau sur le calque.
fn app_calque(name: &str, kind: LayerKind, aplat: bool) -> App {
    let mut app = app(name);
    let mut doc = Document::new(1920, 1080, 24);
    let mut layer = Layer::new("daniel", kind);
    if aplat {
        layer.fond = Some(FondReglages { couleur: ROSE, ..FondReglages::default() });
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = 4;
    app.doc = doc;
    (app.cur_panel, app.cur_layer, app.cur_frame) = (0, 0, 0);
    app.tool = Tool::Brush;
    app.color = [20, 20, 20];
    app
}

/// Un trait de pinceau qui suit les points `pts` (coordonnées du monde), d'un seul geste.
fn geste(app: &mut App, pts: &[(f32, f32)]) {
    app.begin_paint(pos2(pts[0].0, pts[0].1), 1.0, false);
    for w in pts.windows(2) {
        let n = 40;
        for k in 1..=n {
            let t = k as f32 / n as f32;
            app.stroke_to(pos2(w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t), 1.0);
        }
    }
    app.end_stroke();
}

/// Le rendu exact d'un trait vectoriel vient d'un fil de travail : on le laisse arriver.
fn attendre_rejeu(app: &mut App) {
    let ctx = egui::Context::default();
    for _ in 0..400 {
        app.poll_rejeu(&ctx);
        if !app.rejeu.has_pending() && app.rejeu_deferred.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Un anneau fermé de centre (`cx`, `cy`) et de rayon `r`, en une suite de points.
fn anneau(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..=24).map(|k| {
        let a = k as f32 / 24.0 * std::f32::consts::TAU;
        (cx + r * a.cos(), cy + r * a.sin())
    }).collect()
}

fn est_rose(c: Color32) -> bool {
    c.a() > 200 && (c.r() as i32 - ROSE[0] as i32).abs() < 12 && (c.g() as i32 - ROSE[1] as i32).abs() < 12
}

fn est_trait(c: Color32) -> bool {
    c.a() > 200 && c.r() < 80 && c.g() < 80
}

/// Une image d'écran entière de l'application (1280 × 720).
struct Ecran {
    ctx: egui::Context,
    temps: f64,
}

impl Ecran {
    fn new() -> Self {
        Ecran { ctx: egui::Context::default(), temps: 0.0 }
    }

    fn image(&mut self, app: &mut App) {
        self.temps += 1.0 / 60.0;
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(1280.0, 720.0))), time: Some(self.temps), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(1.0);
        let _ = self.ctx.run(input, |ctx| app.frame(ctx));
    }
}

/// Dessine en direct, comme Anthony : des anneaux de plus en plus loin à gauche et au-dessus de la feuille, une image
/// d'écran à chaque point (zoom arrière), puis le rendu exact. Rend les centres des anneaux.
fn dessiner_loin(app: &mut App, ecran: &mut Ecran) -> Vec<(f32, f32, f32)> {
    ecran.image(app);
    app.fit_pending = false;
    app.view.zoom = 0.1;
    app.view.pan = egui::vec2(0.0, 0.0);
    ecran.image(app);
    let anneaux = vec![(900.0, 500.0, 120.0), (-300.0, 400.0, 150.0), (-900.0, 450.0, 150.0), (-1500.0, 500.0, 150.0), (-2300.0, 450.0, 160.0), (-400.0, -500.0, 140.0), (-1200.0, -900.0, 140.0)];
    for &(cx, cy, r) in &anneaux {
        let pts = anneau(cx, cy, r);
        geste(app, &pts);
        for _ in 0..3 {
            ecran.image(app);
        }
    }
    attendre_rejeu(app);
    for _ in 0..40 {
        ecran.image(app);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    anneaux
}

#[test]
fn le_trait_vectoriel_dessine_loin_est_entier_a_l_ecran_et_dans_les_donnees() {
    let mut app = app_calque("vecto-loin", LayerKind::Vector, false);
    let mut ecran = Ecran::new();
    let anneaux = dessiner_loin(&mut app, &mut ecran);
    let cel = app.layers()[0].frames[0].expect("dessin");
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    let mut manque = Vec::new();
    for &(cx, cy, r) in &anneaux {
        let (x, y) = ((cx - r) as i64, cy as i64);
        let donnees = (-3..=3).any(|d| est_trait(app.doc.cels[&cel].raster.get_i(x + d, y)));
        let compose = (-3..=3).any(|d| est_trait(render::composite_pixel_i(&app.doc, &plan, x + d, y)));
        let hors_feuille = x < 0 || y < 0;
        let ecran_ok = !hors_feuille || (-3..=3).any(|d| app.monde.pixel(x + d, y).is_some_and(est_trait));
        if !(donnees && compose && ecran_ok) {
            manque.push((cx, cy, donnees, compose, ecran_ok));
        }
    }
    println!("zone du monde : {:?}", app.monde.coverage());
    assert!(manque.is_empty(), "anneaux coupés (centre, données, composé, écran) : {manque:?}");
}

#[test]
fn l_aplat_dessine_loin_couvre_toute_la_forme_a_l_ecran_et_dans_le_cadre_de_la_camera() {
    let mut app = app_calque("aplat-loin", LayerKind::Vector, true);
    let mut ecran = Ecran::new();
    let anneaux = dessiner_loin(&mut app, &mut ecran);
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    let mut manque = Vec::new();
    for &(cx, cy, _) in &anneaux {
        let (x, y) = (cx as i64, cy as i64);
        let compose = est_rose(render::composite_pixel_i(&app.doc, &plan, x, y));
        let hors_feuille = x < 0 || y < 0;
        let ecran_ok = !hors_feuille || app.monde.pixel(x, y).is_some_and(est_rose);
        if !(compose && ecran_ok) {
            manque.push((cx, cy, compose, ecran_ok));
        }
    }
    assert!(manque.is_empty(), "aplats coupés (centre, composé, écran) : {manque:?}");
    // La caméra déplacée sur le dessin lointain : l'image filmée contient le trait et l'aplat.
    let c = app.doc.default_camera();
    let cam = Camera { x: -1500.0, y: 500.0, ..c };
    let sortie = render::camera_image(&app.doc, &plan, &cam);
    let [fw, fh] = app.doc.frame;
    let (mut rose, mut trait_) = (0, 0);
    for p in &sortie {
        rose += est_rose(*p) as usize;
        trait_ += est_trait(*p) as usize;
    }
    assert!(rose > fw * fh / 40, "l'aplat est dans l'image de la caméra ({rose} pixels roses)");
    assert!(trait_ > 500, "le trait aussi ({trait_} pixels)");
    // Le trait des anneaux lointains est entier, à l'écran aussi (l'aplat ne lui coupe rien).
    let cel = app.layers()[0].frames[0].expect("dessin");
    for &(cx, cy, r) in &anneaux {
        let (x, y) = ((cx - r) as i64, cy as i64);
        assert!((-3..=3).any(|d| est_trait(app.doc.cels[&cel].raster.get_i(x + d, y))), "le trait de l'anneau ({cx}, {cy}) est dans les données");
        if x < 0 || y < 0 {
            assert!((-3..=3).any(|d| app.monde.pixel(x + d, y).is_some_and(est_trait)), "le trait de l'anneau ({cx}, {cy}) est à l'écran");
        }
    }
}

/// Le pixel (`x`, `y`) du monde, tel que le plan le compose (papier absent).
fn pixel(app: &App, x: i64, y: i64) -> Color32 {
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    render::composite_pixel_i(&app.doc, &plan, x, y)
}

#[test]
fn l_aplat_et_le_trait_ne_s_arretent_pas_au_bord_de_la_feuille() {
    for (nom, genre) in [("bord-vecto", LayerKind::Vector), ("bord-bitmap", LayerKind::Bitmap)] {
        let mut app = app_calque(nom, genre, true);
        // Une boucle fermée qui part de la feuille et s'étend de 2 000 px à gauche de son bord.
        geste(&mut app, &[(400.0, 300.0), (-2000.0, 300.0), (-2000.0, 700.0), (400.0, 700.0), (400.0, 300.0)]);
        attendre_rejeu(&mut app);
        // L'aplat, de part et d'autre de l'ancien bord (x = 0), sur toute la hauteur de la forme.
        let (gauche, droite) = ((330..670).filter(|&y| est_rose(pixel(&app, -3, y))).count(), (330..670).filter(|&y| est_rose(pixel(&app, 3, y))).count());
        assert!(gauche > 300 && droite > 300, "{nom} : l'aplat des deux côtés du bord de la feuille ({gauche} à gauche, {droite} à droite)");
        assert!(gauche.abs_diff(droite) <= 4, "{nom} : pas de bord vertical ({gauche} contre {droite})");
        // Le trait, lui aussi : le haut de la boucle traverse le bord sans s'y arrêter.
        let (t_gauche, t_droite) = ((-80..0).filter(|&x| (298..=302).any(|y| est_trait(pixel(&app, x, y)))).count(), (0..80).filter(|&x| (298..=302).any(|y| est_trait(pixel(&app, x, y)))).count());
        assert!(t_gauche >= 78 && t_droite >= 78, "{nom} : le trait traverse le bord ({t_gauche} contre {t_droite})");
        // Loin du bord : l'aplat de toute la forme, et rien autour.
        assert!(est_rose(pixel(&app, -1000, 500)) && est_rose(pixel(&app, -1950, 500)), "{nom} : l'aplat jusqu'au bout de la forme");
        assert_eq!(pixel(&app, -2100, 500).a(), 0, "{nom} : rien au-delà du trait");
        assert_eq!(pixel(&app, -1000, 800).a(), 0, "{nom} : rien sous la forme");
    }
}

/// Un projet enregistré dans un dossier neuf, puis relu.
fn aller_retour(doc: &Document, nom: &str) -> (crate::project::Loaded, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("shot-toile-coupee-save-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    (crate::project::load(&manifest).unwrap(), dir)
}

#[test]
fn le_trait_lointain_n_est_jamais_coupe_dans_les_donnees_apres_enregistrer_et_ouvrir() {
    for (nom, genre) in [("rt-vecto", LayerKind::Vector), ("rt-bitmap", LayerKind::Bitmap)] {
        let mut app = app_calque(nom, genre, true);
        geste(&mut app, &[(400.0, 300.0), (-2000.0, 300.0), (-2000.0, 700.0), (400.0, 700.0), (400.0, 300.0)]);
        attendre_rejeu(&mut app);
        let cel = app.layers()[0].frames[0].expect("dessin");
        let avant = app.doc.cels[&cel].raster.clone();
        let points = app.doc.cels[&cel].vector.as_ref().map(|v| v.elements.iter().map(|e| e.bounds()).collect::<Vec<_>>());
        let (relu, dir) = aller_retour(&app.doc, nom);
        assert!(relu.warnings.is_empty(), "{nom} : {:?}", relu.warnings);
        let cel2 = relu.doc.panels[0].layers[0].frames[0].expect("dessin relu");
        let apres = &relu.doc.cels[&cel2].raster;
        // Le trait, pixel pour pixel, tout le long (jusqu'au bout de la boucle à gauche).
        // (Les coins de la boucle sont arrondis par le lissage du geste : on lit le trait loin d'eux.)
        for (x, y) in [(-1900, 300), (-1500, 300), (-1000, 300), (-500, 300), (-60, 300), (60, 300), (-2000, 500), (-1900, 700), (-1000, 700)] {
            assert!(est_trait(apres.get_i(x, y)) || est_trait(apres.get_i(x, y + 1)) || est_trait(apres.get_i(x + 1, y)), "{nom} : le trait relu en ({x}, {y}) : {:?}", apres.get_i(x, y));
            let (a, b) = (avant.get_i(x, y), apres.get_i(x, y));
            assert!(a.a().abs_diff(b.a()) <= 2, "{nom} : ({x}, {y}) avant {a:?}, après {b:?}");
        }
        if let Some(boites) = points {
            let relus: Vec<_> = relu.doc.cels[&cel2].vector.as_ref().expect("vecteur").elements.iter().map(|e| e.bounds()).collect();
            assert_eq!(boites, relus, "{nom} : les points du trait sont tous là");
        }
        // L'aplat, lui, se refait du trait relu : toute la forme.
        let plan = render::panel_plan_at(&relu.doc, 0, 0, None);
        for x in [-1950, -1000, -100, 100] {
            assert!(est_rose(render::composite_pixel_i(&relu.doc, &plan, x, 500)), "{nom} : l'aplat relu en ({x}, 500)");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}

#[test]
fn la_camera_deplacee_sur_la_partie_lointaine_filme_le_trait_et_l_aplat_en_vecto_et_en_bitmap() {
    for (nom, genre) in [("cam-vecto", LayerKind::Vector), ("cam-bitmap", LayerKind::Bitmap)] {
        let mut app = app_calque(nom, genre, true);
        geste(&mut app, &[(400.0, 300.0), (-2000.0, 300.0), (-2000.0, 700.0), (400.0, 700.0), (400.0, 300.0)]);
        attendre_rejeu(&mut app);
        let plan = render::panel_plan_at(&app.doc, 0, 0, None);
        let c = app.doc.default_camera();
        // Le cadre sur la partie qui était coupée (de -2 000 à -1 000, loin à gauche de la feuille).
        let cam = Camera { x: -1500.0, y: 500.0, ..c };
        let sortie = render::camera_image(&app.doc, &plan, &cam);
        let [fw, _] = app.doc.frame;
        // (Le pixel du monde (x, y) est au pixel (x - gauche, y - haut) du cadre : gauche = -1500 - 960, haut = 500 - 540.)
        let au = |x: i64, y: i64| sortie[((y - (500 - 540)) as usize) * fw + (x - (-1500 - 960)) as usize];
        assert!(est_rose(au(-1000, 500)) && est_rose(au(-1800, 500)) && est_rose(au(-1950, 450)), "{nom} : l'aplat dans le cadre de la caméra");
        assert!((296..=304).any(|y| est_trait(au(-1500, y))) && (296..=304).any(|y| est_trait(au(-1900, y))), "{nom} : le trait dans le cadre de la caméra");
        assert_eq!(au(-2300, 500).a(), 0, "{nom} : rien hors de la forme");
        // Pas de bord vertical dans l'image filmée : la caméra à cheval sur le bord de la feuille.
        let cam = Camera { x: 0.0, y: 500.0, ..c };
        let sortie = render::camera_image(&app.doc, &plan, &cam);
        let au = |x: i64, y: i64| sortie[((y - (500 - 540)) as usize) * fw + (x - (0 - 960)) as usize];
        let (g, d) = ((330..670).filter(|&y| est_rose(au(-3, y))).count(), (330..670).filter(|&y| est_rose(au(3, y))).count());
        assert!(g > 300 && g.abs_diff(d) <= 4, "{nom} : pas de bord vertical dans l'image filmée ({g} contre {d})");
    }
}

#[test]
fn un_dessin_trop_etale_pour_etre_calcule_en_entier_garde_au_moins_sa_feuille() {
    // Plus de 16 millions de pixels de rectangle de travail : l'aplat se limite à la feuille (rien ne plante, rien ne se perd
    // dans les données).
    let mut app = app_calque("etale", LayerKind::Vector, true);
    geste(&mut app, &[(400.0, 300.0), (-20000.0, 300.0), (-20000.0, 1300.0), (400.0, 1300.0), (400.0, 300.0), (200.0, 320.0), (200.0, 700.0), (60.0, 700.0), (60.0, 320.0), (200.0, 320.0)]);
    attendre_rejeu(&mut app);
    let cel = app.layers()[0].frames[0].expect("dessin");
    assert!(est_trait(app.doc.cels[&cel].raster.get_i(-19000, 300)) || est_trait(app.doc.cels[&cel].raster.get_i(-19000, 301)), "le trait est entier dans les données");
    assert!(est_rose(pixel(&app, 130, 500)), "la petite forme de la feuille a son aplat");
}

#[test]
fn le_rendu_approche_et_la_feuille_flottante_gardent_ce_qui_est_hors_de_la_feuille() {
    let mut app = app_calque("approche", LayerKind::Vector, false);
    geste(&mut app, &[(-1500.0, 300.0), (-1200.0, 320.0), (-900.0, 300.0)]);
    geste(&mut app, &[(100.0, 600.0), (300.0, 650.0)]);
    attendre_rejeu(&mut app);
    let cel = app.layers()[0].frames[0].expect("dessin");
    let mut d = app.doc.cels[&cel].vector.clone().expect("vecteur");
    assert!(d.elements.iter().all(|e| matches!(e, crate::vector::Element::Stroke { brush: Some(_), .. })), "des traits au réglage complet");
    for e in &mut d.elements {
        e.invalidate_cache();
    }
    // Le rendu approché d'un projet qui s'ouvre : le trait lointain y est.
    let vite = d.render_quick(1920, 1080);
    assert!(est_trait(vite.get_i(-1200, 320)) || est_trait(vite.get_i(-1200, 319)) || est_trait(vite.get_i(-1200, 321)), "rendu approché : {:?}", vite.get_i(-1200, 320));
    // La feuille flottante d'une sélection (le dernier trait) : le reste du dessin garde le trait lointain.
    let (reste, choisi, _) = d.render_split_extended(&[1], 1920, 1080);
    assert!(est_trait(reste.get_i(-1200, 320)) || est_trait(reste.get_i(-1200, 319)) || est_trait(reste.get_i(-1200, 321)), "le reste du dessin pendant le geste : {:?}", reste.get_i(-1200, 320));
    assert!(choisi.tiles.iter().any(Option::is_some), "et la sélection a sa feuille");
}

#[test]
fn le_rendu_exact_d_un_trait_lointain_previent_l_ecran_de_ses_tuiles_hors_de_la_feuille() {
    let mut app = app_calque("tuiles", LayerKind::Vector, false);
    let mut ecran = Ecran::new();
    ecran.image(&mut app);
    geste(&mut app, &[(-1500.0, 300.0), (-1200.0, 320.0), (-900.0, 300.0)]);
    let _ = app.renderer.take_world_dirty();
    attendre_rejeu(&mut app);
    let (_, ids) = app.renderer.take_world_dirty();
    assert!(ids.iter().any(|&i| crate::raster::is_outer_id(i)), "tuiles prévenues : {ids:?}");
}
