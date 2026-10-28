//! La couleur qui se mélange (`melange.rs`, la case « La couleur se mélange » des réglages de la brosse) : le mélange
//! soustractif (bleu plus jaune donnent un vert), la brosse de pigment, d'eau et de doigt, le réglage enregistré, la case
//! grisée sur un calque vectoriel. L'outil Mélangeur et la palette de mélange : `melangeur_tests.rs`.

use eframe::egui::{pos2, Color32};

use crate::brush::{BrushSettings, MixKind, StrokeMode, StrokeState};
use crate::melange::{melanger, melanger_u8};
use crate::raster::{Raster, TILE};

const BLEU: [u8; 3] = [0, 0, 255];
const JAUNE: [u8; 3] = [255, 255, 0];

fn put(r: &mut Raster, x: usize, y: usize, c: Color32) {
    let idx = r.tile_index_of(x, y);
    r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
}

/// Un rectangle de couleur pleine.
fn remplir(r: &mut Raster, x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 3]) {
    for y in y0..y1 {
        for x in x0..x1 {
            put(r, x, y, Color32::from_rgb(c[0], c[1], c[2]));
        }
    }
}

/// Une brosse ronde nette de taille `size`, sans lissage ni appui : un trait simple à comparer.
fn simple(size: f32) -> BrushSettings {
    BrushSettings { size, hardness: 1.0, spacing: 0.08, smoothing: 0.0, pressure_size: false, ..BrushSettings::brush() }
}

/// Trait de `a` à `b` en `n` points, dans la couleur `color` ; renvoie ce que le trait rend de l'annulation (les tuiles d'avant).
fn tracer(raster: &mut Raster, s: BrushSettings, color: [u8; 3], a: (f32, f32), b: (f32, f32), n: usize) -> Option<crate::history::Edit> {
    let mut st = StrokeState::new(0, StrokeMode::Paint(color), s, None, true);
    let mut dirty = Vec::new();
    for k in 0..=n {
        let t = k as f32 / n as f32;
        st.add_point_at(raster, pos2(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), 1.0, k as f64 * 0.01, &mut dirty);
    }
    st.finish(raster, &mut dirty)
}

// ------------------------------------------------------------------------------------ le mélange soustractif

#[test]
fn blue_and_yellow_make_a_green_not_a_grey() {
    let m = melanger_u8(BLEU, JAUNE, 0.5);
    assert!(m[1] as i32 - m[0] as i32 > 25 && m[1] as i32 - m[2] as i32 > 8, "vert : {m:?}");
    // Le mélange additif (la moyenne) ferait un gris : les trois canaux se tiennent.
    let moyenne = [(BLEU[0] as i32 + JAUNE[0] as i32) / 2, (BLEU[1] as i32 + JAUNE[1] as i32) / 2, (BLEU[2] as i32 + JAUNE[2] as i32) / 2];
    assert!((moyenne[0] - moyenne[2]).abs() < 5, "la moyenne est bien un gris : {moyenne:?}");
    // Plus de jaune, plus de jaune-vert ; plus de bleu, un vert bleuté.
    let (bleute, jaune) = (melanger_u8(BLEU, JAUNE, 0.25), melanger_u8(BLEU, JAUNE, 0.75));
    assert!(bleute[2] > m[2] && jaune[2] < m[2] && jaune[0] > m[0], "{bleute:?} {m:?} {jaune:?}");
    // Rouge plus jaune : un orange ; rouge plus bleu : un violet ; cyan plus magenta : un bleu.
    let orange = melanger_u8([255, 0, 0], JAUNE, 0.5);
    assert!(orange[0] > 220 && orange[1] > 90 && orange[1] < 200 && orange[2] < 40, "{orange:?}");
    let violet = melanger_u8([255, 0, 0], BLEU, 0.5);
    assert!(violet[0] > 80 && violet[2] > 80 && violet[1] < 40, "{violet:?}");
}

#[test]
fn mixing_a_colour_with_itself_or_with_nothing_gives_it_back() {
    for c in [[120.0, 80.0, 200.0], [0.0, 0.0, 0.0], [255.0, 255.0, 255.0], [12.0, 200.0, 90.0]] {
        assert_eq!(melanger(c, [10.0, 20.0, 30.0], 0.0), c);
        assert_eq!(melanger([10.0, 20.0, 30.0], c, 1.0), c);
        let m = melanger(c, c, 0.37);
        assert!((0..3).all(|k| (m[k] - c[k]).abs() < 0.6), "{c:?} -> {m:?}");
    }
    // Un poids qui n'est pas un nombre ne casse rien.
    assert_eq!(melanger([1.0, 2.0, 3.0], [9.0, 9.0, 9.0], f32::NAN), [1.0, 2.0, 3.0]);
    // Blanc et noir : un gris, entre les deux.
    let g = melanger_u8([255; 3], [0; 3], 0.5);
    assert!(g[0] == g[1] && g[1] == g[2] && g[0] > 40 && g[0] < 200, "{g:?}");
}

// ------------------------------------------------------------------------------------ la brosse de pigment

#[test]
fn a_blue_brush_over_yellow_paints_a_green() {
    let mut r = Raster::new(320, 160);
    remplir(&mut r, 0, 0, 320, 160, JAUNE);
    let mut s = simple(20.0);
    s.mix = 0.15;
    tracer(&mut r, s, BLEU, (40.0, 80.0), (280.0, 80.0), 60);
    // Le long du trait, la brosse bleue prend le jaune qu'elle traverse : elle passe par du vert (jamais par un gris : sur
    // le bleu et le jaune, un mélange additif donnerait le rouge égal au vert).
    let verts = (45..280).filter(|&x| {
        let p = r.get(x, 80);
        p.a() == 255 && p.g() as i32 - p.r() as i32 > 12 && p.g() as i32 - p.b() as i32 > 12
    });
    assert!(verts.count() >= 5, "du vert sur le trait");
    // Le jaune alentour n'a pas bougé.
    assert_eq!(r.get(160, 20), Color32::from_rgb(255, 255, 0));
    // Sans mélange, la même brosse pose du bleu pur.
    let mut r2 = Raster::new(320, 160);
    remplir(&mut r2, 0, 0, 320, 160, JAUNE);
    tracer(&mut r2, simple(20.0), BLEU, (40.0, 80.0), (280.0, 80.0), 60);
    assert_eq!(r2.get(160, 80), Color32::from_rgb(0, 0, 255));
}

#[test]
fn the_charge_of_a_pigment_brush_runs_out_along_the_stroke() {
    let mut r = Raster::new(320, 160);
    let mut s = simple(16.0);
    s.mix_kind = MixKind::Pigment;
    s.charge = 0.6;
    tracer(&mut r, s, [200, 30, 30], (30.0, 80.0), (290.0, 80.0), 80);
    let (debut, milieu, fin) = (r.get(45, 80).a(), r.get(160, 80).a(), r.get(285, 80).a());
    assert!(debut > 200 && fin < debut / 2 && milieu < debut && milieu > fin, "début {debut}, milieu {milieu}, fin {fin}");
    // Une charge pleine ne s'épuise pas.
    let mut r2 = Raster::new(320, 160);
    let mut s = simple(16.0);
    s.mix = 0.3;
    s.charge = 1.0;
    tracer(&mut r2, s, [200, 30, 30], (30.0, 80.0), (290.0, 80.0), 80);
    assert!(r2.get(285, 80).a() > 240);
}

// ------------------------------------------------------------------------------------ l'eau

fn eau(water: f32, hardness: f32) -> BrushSettings {
    BrushSettings { mix_kind: MixKind::Water, water, hardness, ..simple(24.0) }
}

#[test]
fn watercolour_is_more_transparent_with_more_water_and_with_none_it_is_a_plain_stroke() {
    let alpha = |water: f32| {
        let mut r = Raster::new(320, 160);
        tracer(&mut r, eau(water, 1.0), [30, 60, 200], (40.0, 80.0), (280.0, 80.0), 60);
        r.get(160, 80).a() as i32
    };
    let (peu, beaucoup) = (alpha(0.1), alpha(0.9));
    assert!(peu > beaucoup + 80, "peu d'eau {peu}, beaucoup {beaucoup}");
    assert!(beaucoup > 20 && beaucoup < 140, "reste visible, transparent : {beaucoup}");
    // L'eau ne passe pas à zéro d'un coup : sans eau (0), le mélange est éteint, la brosse pose son trait ordinaire.
    assert!(!eau(0.0, 1.0).mixes());
    let mut r = Raster::new(320, 160);
    tracer(&mut r, eau(0.0, 1.0), [30, 60, 200], (40.0, 80.0), (280.0, 80.0), 60);
    assert_eq!(r.get(160, 80), Color32::from_rgb(30, 60, 200));
}

#[test]
fn watercolour_mixes_with_the_wet_colours_below_and_leaves_a_slightly_denser_edge() {
    // Du bleu qui coule sur du jaune : vert, comme avec le pigment.
    let mut r = Raster::new(320, 160);
    remplir(&mut r, 0, 0, 320, 160, JAUNE);
    tracer(&mut r, eau(0.5, 1.0), BLEU, (40.0, 80.0), (280.0, 80.0), 60);
    // Le bleu dilué qui coule sur le jaune passe par du vert (le rouge reste en dessous du vert : un mélange additif les
    // aurait égaux), et le jaune dessous garde la feuille opaque.
    let verts = (45..280).filter(|&x| {
        let p = r.get(x, 80);
        p.a() == 255 && p.g() as i32 - p.r() as i32 > 12
    });
    assert!(verts.count() >= 5, "du vert sur le trait");
    // Le bord : un pinceau doux d'eau est un peu plus dense au bord que le même trait ordinaire de même transparence.
    let eau_douce = eau(0.5, 0.2);
    let mut a = Raster::new(320, 160);
    tracer(&mut a, eau_douce, [30, 60, 200], (40.0, 80.0), (280.0, 80.0), 60);
    let mut b = Raster::new(320, 160);
    let ordinaire = BrushSettings { opacity: 1.0 - 0.75 * 0.5, mix_kind: MixKind::Water, mix_off: true, ..eau_douce };
    tracer(&mut b, ordinaire, [30, 60, 200], (40.0, 80.0), (280.0, 80.0), 60);
    let (mut plus, mut moins) = (0, 0);
    for y in 60..100 {
        let (ea, eb) = (a.get(160, y).a(), b.get(160, y).a());
        match ea.cmp(&eb) {
            std::cmp::Ordering::Greater => plus += 1,
            std::cmp::Ordering::Less => moins += 1,
            _ => {}
        }
    }
    assert!(plus >= 3 && moins == 0, "bord plus dense : {plus} pixels plus denses, {moins} moins denses");
    // Au centre plein, jamais plus dense que la transparence demandée.
    assert_eq!(a.get(160, 80).a(), b.get(160, 80).a());
}

// ------------------------------------------------------------------------------------ le doigt

fn doigt(smudge: f32) -> BrushSettings {
    BrushSettings { mix_kind: MixKind::Smudge, smudge, ..simple(18.0) }
}

#[test]
fn the_finger_drags_what_it_crosses_and_puts_down_no_colour_of_its_own() {
    // Moitié rouge, moitié bleue ; le doigt est réglé en vert.
    let mut r = Raster::new(320, 160);
    remplir(&mut r, 0, 0, 160, 160, [220, 0, 0]);
    remplir(&mut r, 160, 0, 320, 160, [0, 0, 220]);
    let avant = r.to_rgba();
    tracer(&mut r, doigt(0.6), [0, 255, 0], (100.0, 80.0), (260.0, 80.0), 60);
    assert_ne!(r.to_rgba(), avant, "le doigt a étalé quelque chose");
    // Passé la frontière, du rouge a été entraîné dans le bleu.
    let p = r.get(172, 80);
    assert!(p.r() > 40, "du rouge entraîné : {p:?}");
    // Nulle part le vert du doigt : la couleur ne peut venir que du rouge, du bleu, ou d'un mélange des deux.
    for y in 0..160 {
        for x in 0..320 {
            let p = r.get(x, y);
            assert!(p.g() < 40, "pas de vert posé en ({x}, {y}) : {p:?}");
            assert_eq!(p.a(), 255, "rien n'est devenu transparent");
        }
    }
}

#[test]
fn the_finger_on_an_empty_sheet_or_a_flat_colour_changes_nothing() {
    let mut vide = Raster::new(320, 160);
    let edit = tracer(&mut vide, doigt(0.8), [255, 0, 0], (40.0, 80.0), (280.0, 80.0), 60);
    assert!(vide.is_blank(), "aucune couleur neuve sur une feuille vide");
    assert!(edit.is_none(), "pas de tuile touchée, pas d'étape d'annulation inutile");
    let mut plat = Raster::new(320, 160);
    remplir(&mut plat, 0, 0, 320, 160, [90, 140, 60]);
    let avant = plat.to_rgba();
    tracer(&mut plat, doigt(0.8), [255, 0, 0], (40.0, 80.0), (280.0, 80.0), 60);
    assert_eq!(plat.to_rgba(), avant, "une couleur unie reste unie");
}

#[test]
fn a_higher_drag_carries_the_colour_further() {
    let loin = |smudge: f32| {
        let mut r = Raster::new(400, 160);
        remplir(&mut r, 0, 0, 120, 160, [220, 0, 0]);
        remplir(&mut r, 120, 0, 400, 160, [0, 0, 220]);
        tracer(&mut r, doigt(smudge), [0, 0, 0], (60.0, 80.0), (380.0, 80.0), 100);
        (120..400).rev().find(|&x| r.get(x, 80).r() > 30).unwrap_or(0)
    };
    assert!(loin(0.95) > loin(0.2) + 20, "{} contre {}", loin(0.95), loin(0.2));
}

// ------------------------------------------------------------------------------------ une étape d'annulation

#[test]
fn a_mixing_stroke_is_one_undo_step_that_restores_the_sheet() {
    for kind in MixKind::ALL {
        let mut r = Raster::new(320, 160);
        remplir(&mut r, 0, 0, 160, 160, [220, 0, 0]);
        remplir(&mut r, 160, 0, 320, 160, JAUNE);
        let avant = r.to_rgba();
        let s = BrushSettings { mix_kind: kind, mix: 0.5, water: 0.5, smudge: 0.6, ..simple(20.0) };
        let edit = tracer(&mut r, s, BLEU, (60.0, 80.0), (260.0, 80.0), 50);
        assert_ne!(r.to_rgba(), avant, "{kind:?} a changé la feuille");
        // Un seul `Edit` pour tout le trait : on rend les tuiles d'avant, la feuille est celle d'avant.
        let Some(crate::history::Edit::Tiles { tiles, .. }) = edit else { panic!("{kind:?} : une étape de tuiles") };
        for (i, t) in tiles {
            r.replace_slot(i, t);
        }
        assert_eq!(r.to_rgba(), avant, "{kind:?} : annulé");
    }
}

// ------------------------------------------------------------------------------------ le réglage

#[test]
fn the_mixing_setting_is_saved_with_the_brush_and_read_back() {
    let mut s = BrushSettings { mix_kind: MixKind::Water, water: 0.8, smudge: 0.3, mix: 0.45, charge: 0.7, ..BrushSettings::brush() };
    s.mix_off = true;
    let text = toml::to_string(&s).expect("écrit");
    let back: BrushSettings = toml::from_str(&text).expect("relu");
    assert_eq!(back, s, "{text}");
    // Dans « Mes brosses », sur le disque, relu au démarrage suivant.
    use crate::brushtex::{BrushLibrary, Target, MINE};
    let dir = std::env::temp_dir().join(format!("shot-melange-brosses-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut lib = BrushLibrary::load(None);
    let i = lib.save_mine("Aquarelle", Target::Brush, s);
    assert_eq!(lib.presets[i].settings, s);
    lib.save(&dir).expect("écrit");
    let relue = BrushLibrary::load(Some(&dir));
    let p = relue.presets.iter().find(|p| p.source == MINE && p.name == "Aquarelle").expect("relue");
    assert_eq!(p.settings, s);
    let _ = std::fs::remove_dir_all(&dir);
    // Une brosse sans mélange ne change pas le fichier (rien de neuf dans le texte), et les vieux fichiers se relisent.
    let plain = toml::to_string(&BrushSettings::brush()).expect("écrit");
    for cle in ["melange_type", "eau", "entrainement", "melange_coupe"] {
        assert!(!plain.contains(cle), "{cle} dans le texte d'une brosse sans mélange");
    }
    let vieux = "melange = 0.4\ncharge = 0.8\n";
    let b: BrushSettings = toml::from_str(vieux).expect("vieux réglage");
    assert_eq!((b.mix, b.charge, b.mix_kind, b.mix_off), (0.4, 0.8, MixKind::Pigment, false));
    assert!(b.mixes(), "un vieux réglage avec du mélange mélange encore");
}

#[test]
fn the_checkbox_keeps_the_values_and_the_category_follows() {
    use crate::pinceaux::Categorie;
    let mut s = BrushSettings { mix: 0.5, charge: 0.8, ..BrushSettings::brush() };
    assert!(s.mixes() && Categorie::Couleur.allumee(&s));
    // Décochée : le mélange est coupé, les valeurs restent.
    s.mix_off = true;
    assert!(!s.mixes() && !Categorie::Couleur.allumee(&s));
    assert_eq!((s.mix, s.charge), (0.5, 0.8));
    // Recochée : tout revient comme avant.
    s.turn_mix_on();
    assert!(s.mixes() && (s.mix, s.charge) == (0.5, 0.8));
    // Une brosse sans rien à mêler prend une valeur de départ du type choisi.
    for (kind, ok) in [(MixKind::Pigment, true), (MixKind::Water, true), (MixKind::Smudge, true)] {
        let mut p = BrushSettings { mix_kind: kind, mix: 0.0, water: 0.0, smudge: 0.0, ..BrushSettings::brush() };
        assert!(!p.mixes());
        p.turn_mix_on();
        assert_eq!(p.mixes(), ok, "{kind:?}");
    }
    // Chaque type ne dépend que de son curseur.
    let eau = BrushSettings { mix_kind: MixKind::Water, mix: 0.0, water: 0.4, ..BrushSettings::brush() };
    assert!(eau.mixes());
    let doigt = BrushSettings { mix_kind: MixKind::Smudge, mix: 0.9, smudge: 0.0, ..BrushSettings::brush() };
    assert!(!doigt.mixes());
}

#[test]
fn the_checkbox_is_greyed_on_a_vector_layer_with_its_tooltip() {
    use eframe::egui::{self, vec2, Rect};
    let run = |vector: bool, s: &mut BrushSettings| -> Vec<String> {
        let ctx = egui::Context::default();
        let mut textes = Vec::new();
        for _ in 0..2 {
            textes.clear();
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0))), ..Default::default() };
            let out = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| crate::dynui::wet_ui(ui, s, vector, false));
            });
            fn collecter(s: &egui::Shape, out: &mut Vec<String>) {
                match s {
                    egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                    egui::Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
                    _ => {}
                }
            }
            out.shapes.iter().for_each(|c| collecter(&c.shape, &mut textes));
        }
        textes
    };
    // Bitmap, mélange allumé : la case, les trois types et le curseur du type.
    let mut s = BrushSettings { mix: 0.5, ..BrushSettings::brush() };
    let t = run(false, &mut s);
    let tout = t.join(" | ");
    for mot in ["La couleur se mélange", "Pigment", "Eau", "Lisser", "Mélange", "Charge"] {
        assert!(tout.contains(mot), "{mot} absent : {tout}");
    }
    // Éteint : la case seule.
    let mut off = BrushSettings::brush();
    let t = run(false, &mut off).join(" | ");
    assert!(t.contains("La couleur se mélange") && !t.contains("Pigment"), "{t}");
    // Vectoriel : la case, rien d'autre ; elle est grisée (l'état d'activation de la case).
    let mut v = BrushSettings { mix: 0.5, ..BrushSettings::brush() };
    let t = run(true, &mut v).join(" | ");
    assert!(t.contains("La couleur se mélange") && !t.contains("Pigment") && !t.contains("Charge"), "{t}");
    assert_eq!(v, BrushSettings { mix: 0.5, ..BrushSettings::brush() }, "la case grisée ne change rien");
}
