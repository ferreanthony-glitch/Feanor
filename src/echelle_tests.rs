//! La taille de l'interface (`echelle.rs`) : 90, 100, 125 ou 150 %, choisie dans Préférences > Apparence, appliquée tout de suite,
//! retenue dans les réglages ; et le dessin reste juste à chaque taille : un point du stylet ou de la souris reste un point du
//! dessin, les pixels par point de l'écran sont ceux de l'interface, et le dépôt de fichiers lit le curseur au bon endroit.

use eframe::egui::{epaint::Shape, pos2, vec2, Event, Modifiers, PointerButton, Pos2};

use crate::document::{Document, Layer, LayerKind};
use crate::echelle::{echelle_valide, facteur, ECHELLES, ECHELLE_DEFAUT};
use crate::perf_tests::{banc_taille, Banc};
use crate::prefsonglets::Onglet;
use crate::settings::Settings;

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(320, 180, 24), false, 1600, 900);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

fn tous_les_textes(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

#[test]
fn the_four_sizes_are_offered_and_anything_else_snaps_to_the_nearest() {
    assert_eq!(ECHELLES, [90, 100, 125, 150]);
    assert_eq!(ECHELLE_DEFAUT, 100);
    assert_eq!([facteur(90), facteur(100), facteur(125), facteur(150)], [0.9, 1.0, 1.25, 1.5]);
    for (ecrit, valide) in [(0, 90), (89, 90), (95, 90), (101, 100), (110, 100), (120, 125), (140, 150), (150, 150), (400, 150), (u16::MAX, 150)] {
        assert_eq!(echelle_valide(ecrit), valide, "{ecrit} %");
    }
}

#[test]
fn the_size_is_remembered_in_the_settings_file_and_the_default_leaves_no_trace() {
    let mut s = Settings::default();
    assert_eq!(s.echelle, 100);
    assert!(!toml::to_string(&s).unwrap().contains("echelle_interface"), "100 % : la clé n'est pas écrite");
    for e in [90, 125, 150] {
        s.echelle = e;
        let ecrit = toml::to_string(&s).unwrap();
        assert!(ecrit.contains(&format!("echelle_interface = {e}")), "{ecrit}");
        let relu: Settings = toml::from_str(&ecrit).unwrap();
        assert_eq!(relu.echelle, e);
    }
    // Des réglages d'avant : 100 %. Une valeur écrite à la main hors liste est ramenée à la plus proche au chargement.
    let ancien: Settings = toml::from_str("").unwrap();
    assert_eq!(ancien.echelle, 100);
}

#[test]
fn the_chosen_size_is_applied_live_to_the_interface_and_written_to_disk() {
    let mut b = banc();
    assert!((b.ctx.pixels_per_point() - 1.0).abs() < 1e-4);
    b.app.settings.prefs_onglet = Onglet::Apparence;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    // Les quatre choix sont dans la fenêtre, 100 % est allumé.
    // (« 100 % » est aussi un bouton de la vue sous le dessin : on prend celui de la ligne « Échelle ».)
    let pos = |b: &Banc, t: &str| {
        let ligne = tous_les_textes(b).into_iter().find(|(x, _)| x == "Échelle").expect("la ligne Échelle").1.y;
        tous_les_textes(b).into_iter().find(|(x, p)| x == t && (p.y - ligne).abs() < 4.0).unwrap_or_else(|| panic!("« {t} » manque")).1
    };
    for e in ECHELLES {
        pos(&b, &format!("{e} %"));
    }
    let p = pos(&b, "150 %");
    clic(&mut b, p);
    assert_eq!(b.app.settings.echelle, 150);
    b.chauffer(3);
    assert!((b.ctx.zoom_factor() - 1.5).abs() < 1e-4 && (b.ctx.pixels_per_point() - 1.5).abs() < 1e-4, "egui est à 150 % : {} {}", b.ctx.zoom_factor(), b.ctx.pixels_per_point());
    assert!((b.app.ppp - 1.5).abs() < 1e-4, "l'application lit les mêmes pixels par point");
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(disque.contains("echelle_interface = 150"), "{disque}");
    // Un autre choix, tout de suite aussi ; et le retour à 100 % défait la clé du fichier.
    let p = pos(&b, "90 %");
    clic(&mut b, p);
    b.chauffer(3);
    assert!((b.ctx.pixels_per_point() - 0.9).abs() < 1e-4);
    let p = pos(&b, "100 %");
    clic(&mut b, p);
    b.chauffer(3);
    assert!((b.ctx.pixels_per_point() - 1.0).abs() < 1e-4);
    let disque = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(!disque.contains("echelle_interface"), "{disque}");
}

#[test]
fn ctrl_plus_and_ctrl_zero_no_longer_change_the_interface_size_behind_the_setting() {
    let mut b = banc();
    b.app.settings.echelle = 125;
    b.chauffer(4);
    assert!((b.ctx.pixels_per_point() - 1.25).abs() < 1e-4);
    // Ctrl+0 est « Ajuster à la fenêtre » : il ne remet pas l'interface à 100 %.
    let ctrl = Modifiers::COMMAND;
    b.image(vec![Event::Key { key: eframe::egui::Key::Num0, physical_key: Some(eframe::egui::Key::Num0), pressed: true, repeat: false, modifiers: ctrl }], 1.0 / 60.0);
    b.chauffer(3);
    assert!((b.ctx.pixels_per_point() - 1.25).abs() < 1e-4, "{}", b.ctx.pixels_per_point());
    b.image(vec![Event::Key { key: eframe::egui::Key::Plus, physical_key: Some(eframe::egui::Key::Plus), pressed: true, repeat: false, modifiers: ctrl }], 1.0 / 60.0);
    b.chauffer(3);
    assert!((b.ctx.pixels_per_point() - 1.25).abs() < 1e-4, "{}", b.ctx.pixels_per_point());
}

/// Un projet dont le premier calque est bitmap : le trait y laisse des pixels, faciles à mesurer.
fn banc_bitmap(echelle: u16) -> Banc {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = vec![Layer::new("Calque", LayerKind::Bitmap)];
    let mut b = banc_taille(doc, false, 1600, 900);
    b.app.settings.echelle = echelle;
    b.chauffer(10);
    // (Recadré une fois la mise en page posée : la première image d'un banc ne la connaît pas encore.)
    b.app.fit_pending = true;
    b.chauffer(4);
    b
}

/// La boîte (en pixels du dessin) de tout ce qui est peint dans le dessin courant.
fn boite_de_l_encre(b: &Banc) -> Option<(usize, usize, usize, usize)> {
    let cel = b.app.cel_here().and_then(|c| b.app.doc.cels.get(&c))?;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..cel.raster.height {
        for x in 0..cel.raster.width {
            if cel.raster.get(x, y).a() > 40 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    (x0 != usize::MAX).then_some((x0, y0, x1, y1))
}

#[test]
fn a_stroke_lands_where_the_view_says_at_every_interface_size() {
    // Le trait est tracé en points de la fenêtre, comme les événements du stylet et de la souris les donnent à egui. Il doit arriver
    // dans le dessin là où la vue (`View::to_canvas`) place ces points, à 90, 100, 125 et 150 %.
    let mut vus = Vec::new();
    for e in ECHELLES {
        let mut b = banc_bitmap(e);
        assert!((b.ctx.pixels_per_point() - f32::from(e) / 100.0).abs() < 1e-4, "{e} % : egui a la bonne taille");
        let rect = b.app.canvas_rect;
        let (size, view) = (b.app.canvas_size(), b.app.view.clone());
        // Un trait horizontal à gauche du centre, un autre vertical à droite : la boîte de l'encre dit l'échelle et la place.
        let (a, c) = (rect.center() + vec2(-120.0, -40.0), rect.center() + vec2(120.0, 60.0));
        let chemin: Vec<Pos2> = (0..=16).map(|k| a + (c - a) * (k as f32 / 16.0)).collect();
        b.image(vec![Event::PointerMoved(chemin[0])], 1.0 / 60.0);
        b.image(vec![Event::PointerButton { pos: chemin[0], button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        for q in &chemin[1..] {
            b.image(vec![Event::PointerMoved(*q)], 1.0 / 120.0);
        }
        b.image(vec![Event::PointerButton { pos: c, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.chauffer(6);
        let (x0, y0, x1, y1) = boite_de_l_encre(&b).unwrap_or_else(|| panic!("{e} % : le trait est dans le dessin"));
        // Où la vue place le début et la fin du trait.
        let (da, dc) = (view.to_canvas(rect, size, a), view.to_canvas(rect, size, c));
        let marge = 8.0 / view.zoom.max(0.1) + 3.0;
        let (attendu_x0, attendu_x1, attendu_y0, attendu_y1) = (da.x.min(dc.x), da.x.max(dc.x), da.y.min(dc.y), da.y.max(dc.y));
        assert!(
            (x0 as f32 - attendu_x0).abs() < marge && (x1 as f32 - attendu_x1).abs() < marge && (y0 as f32 - attendu_y0).abs() < marge && (y1 as f32 - attendu_y1).abs() < marge,
            "{e} % : encre ({x0},{y0})-({x1},{y1}) attendue ({attendu_x0:.1},{attendu_y0:.1})-({attendu_x1:.1},{attendu_y1:.1}), marge {marge:.1}"
        );
        // La vue reste la vue du dessin : le trait fait en points la même longueur à l'écran, quelle que soit la taille.
        vus.push((e, view.zoom, ((x1 - x0) as f32 * view.zoom, (y1 - y0) as f32 * view.zoom)));
    }
    for (e, _, (l, h)) in &vus {
        assert!((l - 240.0).abs() < 14.0 && (h - 100.0).abs() < 14.0, "{e} % : le trait fait 240 × 100 points à l'écran : {l:.1} × {h:.1}");
    }
}

#[test]
fn the_status_bar_zoom_counts_screen_pixels_at_every_size() {
    // « Zoom % » = zoom de la vue × pixels par point : 100 % veut dire un pixel du dessin pour un pixel de l'écran.
    for e in ECHELLES {
        let mut b = banc_bitmap(e);
        b.app.zoom_actual();
        b.chauffer(3);
        let ppp = b.ctx.pixels_per_point();
        assert!((b.app.view.zoom * ppp - 1.0).abs() < 1e-3, "{e} % : un pixel du dessin pour un pixel de l'écran : zoom {} ppp {}", b.app.view.zoom, ppp);
        assert!(b.app.ppp == ppp);
    }
}

#[test]
fn the_cursor_of_a_dropped_file_uses_the_interface_pixels_per_point() {
    use crate::deposer::curseur_en_points;
    for e in ECHELLES {
        let ppp = facteur(e);
        // Un curseur à 1000 px de gauche, 600 px du haut de l'écran ; la fenêtre commence à (100, 50) pixels.
        let inner_min = vec2(100.0 / ppp, 50.0 / ppp);
        let p = curseur_en_points((1000, 600), inner_min, ppp);
        assert!((p - pos2(900.0 / ppp, 550.0 / ppp)).length() < 1e-3, "{e} %");
    }
}
