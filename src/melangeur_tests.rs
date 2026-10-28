//! L'outil Mélangeur et la palette de mélange (`melangeur.rs`) : l'outil dans la case du pinceau, avec son icône, sa bulle et
//! son action sans touche ; il étale ce qui est peint sur un calque bitmap en une étape d'annulation, ne pose aucune couleur
//! et refuse un calque vectoriel ; la palette se peint, se lit au Alt + clic, se vide, et se garde en PNG dans le dossier des
//! réglages (un dossier temporaire dans les essais, jamais le vrai) ; les touches de brosse restent rapides.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

use crate::app::{App, Mode, Tool};
use crate::brush::{BrushSettings, MixKind, StrokeMode, StrokeState};
use crate::document::{Document, Layer, LayerKind};
use crate::melangeur::{reglage_de_palette, PaletteMelange, HAUTEUR, LARGEUR};
use crate::perf_tests::{banc_taille, Banc};
use crate::raster::{Raster, TILE};

fn app(name: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-melangeur-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    let debut = Instant::now();
    while app.brushes_job.is_some() && debut.elapsed() < Duration::from_secs(60) {
        std::thread::sleep(Duration::from_millis(2));
        app.poll_brushes();
    }
    app
}

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

fn remplir(r: &mut Raster, x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 3]) {
    for y in y0..y1 {
        for x in x0..x1 {
            put(r, x, y, Color32::from_rgb(c[0], c[1], c[2]));
        }
    }
}

fn etapes(app: &App) -> usize {
    app.history.names().0.len()
}

// ------------------------------------------------------------------------------------ un plan à l'écran

/// Un plan 400 × 300 à un calque de cette sorte, à l'écran ; un bitmap a un dessin moitié rouge, moitié bleu.
fn banc_melangeur(kind: LayerKind) -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", kind)];
    doc.panels[0].length = 4;
    if kind == LayerKind::Bitmap {
        let id = doc.new_cel(LayerKind::Bitmap);
        for k in 0..4 {
            doc.panels[0].layers[0].set(k, Some(id));
        }
        let r = &mut doc.cels.get_mut(&id).expect("le dessin").raster;
        remplir(r, 0, 0, 200, 300, [220, 0, 0]);
        remplir(r, 200, 0, 400, 300, [0, 0, 220]);
    }
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.tool = Tool::Brush;
    b.app.color = [0, 255, 0];
    b.app.brush.size = 20.0;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn bouton(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

fn glisser(b: &mut Banc, pts: &[(f32, f32)]) {
    let p0 = ecran(b, pts[0].0, pts[0].1);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![bouton(p0, true)], 1.0 / 60.0);
    for &(x, y) in &pts[1..] {
        let p = ecran(b, x, y);
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    }
    b.image(Vec::new(), 1.0 / 60.0);
    let fin = ecran(b, pts[pts.len() - 1].0, pts[pts.len() - 1].1);
    b.image(vec![bouton(fin, false)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

fn pixel(b: &Banc, x: usize, y: usize) -> Color32 {
    match b.app.layers()[0].frames.first().copied().flatten() {
        Some(cel) => b.app.doc.cels[&cel].raster.get(x, y),
        None => Color32::TRANSPARENT,
    }
}

// ------------------------------------------------------------------------------------ l'outil

#[test]
fn the_mixer_is_in_the_brush_case_with_its_icon_tooltip_and_an_action_without_a_key() {
    let case = crate::toolbars::TOOL_GROUPS.iter().flat_map(|g| g.iter()).find(|s| s.contains(&Tool::Brush)).expect("la case du pinceau");
    assert!(case.contains(&Tool::Mixer), "dans la case du pinceau, pas une case de plus");
    assert!(Tool::ALL.contains(&Tool::Mixer));
    assert_eq!((Tool::Mixer.label(), Tool::Mixer.action()), ("Mélangeur", "outil.melangeur"));
    assert!(Tool::Mixer.description().len() > 40, "une bulle qui dit ce qu'il fait");
    // Une icône à lui : aucun autre outil ne la prend.
    assert!(!Tool::Mixer.icon().is_empty());
    for t in Tool::ALL {
        if t != Tool::Mixer {
            assert_ne!(t.icon(), Tool::Mixer.icon(), "{t:?}");
        }
    }
    // L'action est dans le fichier de raccourcis, sans touche, et elle prend l'outil.
    let mut app = app("action");
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "outil.melangeur"));
    assert_eq!(app.keymap.keys_text("outil.melangeur"), "");
    app.run_action("outil.melangeur");
    assert_eq!(app.tool, Tool::Mixer);
    assert!(!crate::viewbar::tool_hint(Tool::Mixer).is_empty());
    assert!(crate::aidetouches::commandes_de_l_outil(Tool::Mixer).len() >= 2);
}

#[test]
fn the_mixer_uses_the_current_brush_as_a_finger() {
    let mut app = app("doigt");
    // Un pinceau qui ne mêle rien, en Mélangeur : le doigt, qui entraîne.
    app.brush = BrushSettings { size: 33.0, hardness: 0.4, ..BrushSettings::brush() };
    assert!(!app.brush.mixes());
    app.set_tool(Tool::Mixer);
    let s = app.current_brush().expect("une brosse");
    assert_eq!((s.mix_kind, s.size, s.hardness), (MixKind::Smudge, 33.0, 0.4));
    assert!(s.mixes());
    // Le réglage du pinceau n'a pas bougé : le Mélangeur n'y touche pas.
    assert!(!app.brush.mixes() && app.brush.mix_kind == MixKind::Pigment);
    // Le curseur de taille des deux outils est le même.
    app.set_tool(Tool::Brush);
    assert_eq!(app.current_brush().map(|s| s.mix_kind), Some(MixKind::Pigment));
}

#[test]
fn a_mixer_stroke_smears_a_bitmap_layer_in_one_undo_step_and_leaves_no_colour_of_its_own() {
    let mut b = banc_melangeur(LayerKind::Bitmap);
    b.app.set_tool(Tool::Mixer);
    let avant: Vec<Color32> = (0..300).flat_map(|y| (0..400).map(move |x| (x, y))).map(|(x, y)| pixel(&b, x, y)).collect();
    let etapes_avant = etapes(&b.app);
    let recentes = b.app.couleurs.prefs.recent.clone();
    glisser(&mut b, &[(120.0, 150.0), (160.0, 150.0), (200.0, 150.0), (240.0, 150.0), (280.0, 150.0)]);
    assert_eq!(etapes(&b.app), etapes_avant + 1, "une étape d'annulation pour le trait");
    // Du rouge entraîné dans le bleu, et sans le vert de l'avant-plan.
    let p = pixel(&b, 215, 150);
    assert!(p.r() > 30, "du rouge entraîné au-delà de la frontière : {p:?}");
    for y in 0..300 {
        for x in 0..400 {
            let p = pixel(&b, x, y);
            assert!(p.g() < 40 && p.a() == 255, "pas de vert posé en ({x}, {y}) : {p:?}");
        }
    }
    assert_eq!(b.app.couleurs.prefs.recent, recentes, "le Mélangeur ne pose aucune couleur : pas de couleur récente");
    // Annuler rend le dessin d'avant.
    b.app.undo();
    let apres: Vec<Color32> = (0..300).flat_map(|y| (0..400).map(move |x| (x, y))).map(|(x, y)| pixel(&b, x, y)).collect();
    assert_eq!(apres, avant);
}

#[test]
fn the_mixer_refuses_a_vector_layer_and_says_why() {
    let mut b = banc_melangeur(LayerKind::Vector);
    b.app.set_tool(Tool::Mixer);
    let etapes_avant = etapes(&b.app);
    glisser(&mut b, &[(120.0, 150.0), (200.0, 150.0), (280.0, 150.0)]);
    assert!(b.app.stroke.is_none(), "aucun trait commencé");
    assert_eq!(etapes(&b.app), etapes_avant, "rien à annuler");
    assert!(b.app.status.contains("bitmap"), "{}", b.app.status);
}

#[test]
fn a_brush_that_mixes_does_not_mix_on_a_vector_layer() {
    let mut b = banc_melangeur(LayerKind::Vector);
    b.app.brush.mix = 0.6;
    b.app.brush.charge = 0.3;
    assert!(b.app.brush.mixes());
    glisser(&mut b, &[(60.0, 150.0), (200.0, 150.0), (340.0, 150.0)]);
    let cel = b.app.layers()[0].frames[0].expect("un dessin");
    let dessin = b.app.drawing(cel).expect("vectoriel");
    let trait_ = dessin.elements.iter().find_map(|e| match e {
        crate::vector::Element::Stroke { brush, .. } => brush.as_ref(),
        _ => None,
    });
    let s = trait_.expect("le trait vectoriel garde son réglage");
    assert!(!s.mixes(), "la peinture qui se mêle ne vaut que sur un calque bitmap : {s:?}");
}

// ------------------------------------------------------------------------------------ la palette

#[test]
fn the_palette_brush_always_mixes_and_keeps_the_size_in_the_zone() {
    let plain = BrushSettings { size: 90.0, ..BrushSettings::brush() };
    let s = reglage_de_palette(plain);
    assert!(s.mixes() && s.size <= 40.0 && s.mix_kind == MixKind::Pigment);
    // Une brosse qui mêle déjà garde son type.
    let eau = BrushSettings { mix_kind: MixKind::Water, water: 0.7, size: 2.0, ..BrushSettings::brush() };
    let s = reglage_de_palette(eau);
    assert_eq!((s.mix_kind, s.water, s.size), (MixKind::Water, 0.7, 4.0));
}

#[test]
fn the_palette_is_painted_mixed_cleared_and_kept_as_a_png() {
    let dir = std::env::temp_dir().join(format!("shot-palette-png-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let fichier = dir.join("palette_melange.png");
    let mut p = PaletteMelange::vide();
    assert!(p.raster.is_blank());
    // Du jaune, puis du bleu qui le traverse en mélange : du vert entre les deux.
    let reglage = reglage_de_palette(BrushSettings { size: 20.0, hardness: 1.0, smoothing: 0.0, mix: 0.15, ..BrushSettings::brush() });
    let toucher = crate::brush::Toucher::default();
    p.commencer(pos2(30.0, 60.0), [255, 255, 0], reglage, Default::default(), toucher);
    for k in 1..=60 {
        p.glisser(pos2(30.0 + 4.0 * k as f32, 60.0));
    }
    assert!(p.finir() && !p.finir(), "un trait fini une fois");
    p.commencer(pos2(30.0, 60.0), [0, 0, 255], reglage, Default::default(), toucher);
    for k in 1..=60 {
        p.glisser(pos2(30.0 + 4.0 * k as f32, 60.0));
    }
    p.finir();
    let vert = (40..270).any(|x| {
        let c = p.raster.get(x, 60);
        c.a() == 255 && c.g() as i32 - c.r() as i32 > 12 && c.g() as i32 - c.b() as i32 > 12
    });
    assert!(vert, "du vert sur la palette");
    assert!(p.couleur_en(pos2(100.0, 60.0)).is_some(), "la pipette lit la peinture");
    assert!(p.couleur_en(pos2(100.0, 150.0)).is_none(), "et rien sur une place vide");
    assert!(p.couleur_en(pos2(-5.0, 10.0)).is_none() && p.couleur_en(pos2(LARGEUR as f32 + 1.0, 10.0)).is_none());
    // Gardée en PNG, relue pixel pour pixel (à un cran près : le PNG n'est pas prémultiplié).
    p.ecrire(&fichier).expect("écrite");
    assert!(fichier.exists());
    let relue = PaletteMelange::lire(&fichier);
    let mut ecart = 0;
    for y in 0..HAUTEUR {
        for x in 0..LARGEUR {
            let (a, b) = (p.raster.get(x, y), relue.raster.get(x, y));
            ecart = ecart.max((a.r() as i32 - b.r() as i32).abs()).max((a.a() as i32 - b.a() as i32).abs());
        }
    }
    assert!(ecart <= 2, "relue à {ecart} près");
    // Un fichier qui n'est pas une image, ou d'une autre taille : une palette vide, sans erreur.
    std::fs::write(dir.join("autre.png"), b"pas une image").unwrap();
    assert!(PaletteMelange::lire(&dir.join("autre.png")).raster.is_blank());
    assert!(PaletteMelange::lire(&dir.join("absent.png")).raster.is_blank());
    // Vidée : plus rien, et le fichier est retiré.
    p.vider();
    assert!(p.raster.is_blank());
    p.ecrire(&fichier).expect("vidée");
    assert!(!fichier.exists(), "une palette vide ne laisse pas de fichier");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Une image d'egui avec ces événements, la palette dessinée dans un panneau.
fn image_palette(app: &mut App, ctx: &egui::Context, temps: &mut f64, events: Vec<Event>, modifiers: Modifiers) {
    *temps += 1.0 / 60.0;
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 500.0))), time: Some(*temps), events, modifiers, ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.palette_melange_ui(ui));
    });
}

#[test]
fn the_palette_panel_is_folded_by_default_paints_with_the_mouse_picks_with_alt_and_is_kept_on_disk() {
    let mut app = app("panneau");
    let ctx = egui::Context::default();
    let mut t = 0.0;
    // Repliée d'office : rien n'est chargé ni dessiné, que son titre.
    assert!(!app.confort.palette_ouverte && app.confort.palette.is_none());
    image_palette(&mut app, &ctx, &mut t, Vec::new(), Modifiers::NONE);
    assert!(app.confort.palette.is_none(), "rien à lire tant que le volet est fermé");
    app.confort.palette_ouverte = true;
    app.color = [200, 30, 30];
    app.brush = BrushSettings { size: 16.0, hardness: 1.0, smoothing: 0.0, ..BrushSettings::brush() };
    image_palette(&mut app, &ctx, &mut t, Vec::new(), Modifiers::NONE);
    image_palette(&mut app, &ctx, &mut t, Vec::new(), Modifiers::NONE);
    let zone = app.confort.palette.as_ref().expect("chargée à l'ouverture").zone;
    assert!(zone.width() > 100.0 && zone.height() > 50.0, "{zone:?}");
    let en = |fx: f32, fy: f32| pos2(zone.left() + fx * zone.width(), zone.top() + fy * zone.height());
    // Un glisser dans la zone : de la peinture, brosse courante en mélange.
    let (a, c) = (en(0.2, 0.5), en(0.6, 0.5));
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerMoved(a)], Modifiers::NONE);
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerButton { pos: a, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], Modifiers::NONE);
    for k in 1..=8 {
        let p = a + (c - a) * (k as f32 / 8.0);
        image_palette(&mut app, &ctx, &mut t, vec![Event::PointerMoved(p)], Modifiers::NONE);
    }
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerButton { pos: c, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    let pal = app.confort.palette.as_ref().expect("la palette");
    assert!(!pal.raster.is_blank(), "la palette est peinte : version {}, zone {zone:?}, a {a:?}, c {c:?}, état : {}", pal.version, app.status);
    assert!(!pal.en_cours(), "le trait est fini au relâché");
    let fichier = crate::melangeur::chemin().expect("un chemin");
    assert!(fichier.exists(), "gardée sur le disque des réglages (de l'essai) au relâché : {}", fichier.display());
    // Alt + clic sur la peinture : la couleur d'avant-plan la prend.
    app.color = [1, 2, 3];
    let milieu = en(0.4, 0.5);
    let alt = Modifiers::ALT;
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerMoved(milieu)], alt);
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerButton { pos: milieu, button: PointerButton::Primary, pressed: true, modifiers: alt }], alt);
    image_palette(&mut app, &ctx, &mut t, vec![Event::PointerButton { pos: milieu, button: PointerButton::Primary, pressed: false, modifiers: alt }], alt);
    assert!(app.color[0] > 100 && app.color[1] < 100, "la couleur de la palette : {:?}", app.color);
    // Le palette n'a pas bougé sous la pipette.
    assert!(app.confort.palette.as_ref().is_some_and(|p| !p.en_cours()));
    // Un autre poste (une autre instance) la retrouve dans le dossier des réglages.
    let relue = PaletteMelange::lire(&fichier);
    assert!(!relue.raster.is_blank());
    // Le bouton la vide, et le fichier part.
    app.palette_de_melange().vider();
    app.enregistrer_palette();
    assert!(!fichier.exists());
}

// ------------------------------------------------------------------------------------ la vitesse

/// Un long trait de `n` points sur une feuille 1200 × 400 déjà peinte, avec ce réglage ; renvoie les millisecondes.
fn chronometrer(s: BrushSettings, n: usize) -> f64 {
    let mut r = Raster::new(1200, 400);
    remplir(&mut r, 0, 0, 600, 400, [220, 180, 20]);
    remplir(&mut r, 600, 0, 1200, 400, [20, 60, 200]);
    let mut st = StrokeState::new(0, StrokeMode::Paint([200, 40, 40]), s, None, true);
    let mut dirty = Vec::new();
    let t = Instant::now();
    for k in 0..=n {
        let f = k as f32 / n as f32;
        st.add_point_at(&mut r, pos2(40.0 + 1120.0 * f, 200.0 + 80.0 * (f * 9.0).sin()), 1.0, k as f64 * 0.004, &mut dirty);
        dirty.clear();
    }
    st.finish(&mut r, &mut dirty);
    t.elapsed().as_secs_f64() * 1e3
}

/// Le meilleur de cinq essais (la machine fait d'autres choses : le plus rapide est le plus juste).
fn meilleur(s: BrushSettings, n: usize) -> f64 {
    (0..5).map(|_| chronometrer(s, n)).fold(f64::INFINITY, f64::min)
}

/// Les touches de brosse : le chemin ordinaire n'est pas touché par le mélange (même vitesse qu'avant : une comparaison à
/// `ignore` avec la version d'avant est dans `docs/INTERFACE.md`, partie 58) ; les trois types de mélange ne coûtent que
/// quelques fois une touche ordinaire, et un long trait à la souris ne demande pas plus de quelques centaines de millisecondes.
#[test]
fn mixing_dabs_stay_fast() {
    let base = BrushSettings { size: 24.0, hardness: 0.8, spacing: 0.08, smoothing: 0.0, pressure_size: false, ..BrushSettings::brush() };
    let plat = meilleur(base, 300);
    let pigment = meilleur(BrushSettings { mix: 0.3, charge: 0.9, ..base }, 300);
    let eau = meilleur(BrushSettings { mix_kind: MixKind::Water, water: 0.5, ..base }, 300);
    let doigt = meilleur(BrushSettings { mix_kind: MixKind::Smudge, smudge: 0.6, ..base }, 300);
    println!("TOUCHES 300 points, pinceau de 24 px : ordinaire {plat:.1} ms, pigment {pigment:.1} ms, eau {eau:.1} ms, doigt {doigt:.1} ms");
    for (nom, ms) in [("pigment", pigment), ("eau", eau), ("doigt", doigt)] {
        assert!(ms < plat * 6.0 + 20.0, "{nom} : {ms:.1} ms contre {plat:.1} ms pour une touche ordinaire");
        assert!(ms < 1500.0, "{nom} : {ms:.1} ms pour un long trait");
    }
}
