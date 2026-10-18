//! Aides au cadrage : lignes de la grille d'or, spirale qui remplit le cadre (16:9 comme 9:16, dans
//! ses huit orientations), diagonales réciproques perpendiculaires, réglages par défaut et retenus.

use eframe::egui::{pos2, Rect};

use crate::cadrage::{self, build, center_cross, diagonals, phi_grid, phi_positions, spiral, thirds, Cache, FramingAids, SafeZones, PHI};
use crate::settings::Settings;

fn near(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn the_golden_lines_split_at_1_0618_1() {
    for len in [100.0_f32, 1080.0, 1920.0] {
        let [a, b] = phi_positions(len);
        // 1 / 2,618 et 1,618 / 2,618 de la longueur.
        assert!(near(a, len / 2.618, len * 1e-4) && near(b, len * 1.618 / 2.618, len * 1e-4), "{len}");
        // Trois parts dans les proportions 1 : 0,618 : 1.
        let parts = [a, b - a, len - b];
        assert!(near(parts[0], parts[2], len * 1e-5), "les parts du bord sont égales");
        assert!(near(parts[1] / parts[0], 0.618, 1e-3), "la part du milieu vaut 0,618 fois celle du bord");
    }
    let (lines, dots) = phi_grid(1920.0, 1080.0);
    assert_eq!(lines.len(), 4);
    assert_eq!(dots.len(), 4, "un point à chaque intersection");
    let [x0, x1] = phi_positions(1920.0);
    let [y0, y1] = phi_positions(1080.0);
    for (dx, dy) in [(x0, y0), (x0, y1), (x1, y0), (x1, y1)] {
        assert!(dots.iter().any(|d| near(d.x, dx, 1e-3) && near(d.y, dy, 1e-3)), "point ({dx}, {dy})");
    }
    // Deux lignes verticales (sur toute la hauteur) et deux horizontales (sur toute la largeur).
    assert_eq!(lines.iter().filter(|l| l[0].x == l[1].x && l[0].y == 0.0 && l[1].y == 1080.0).count(), 2);
    assert_eq!(lines.iter().filter(|l| l[0].y == l[1].y && l[0].x == 0.0 && l[1].x == 1920.0).count(), 2);
}

#[test]
fn the_thirds_are_a_three_by_three_grid() {
    let lines = thirds(300.0, 90.0);
    assert_eq!(lines.len(), 4);
    let mut xs: Vec<f32> = lines.iter().filter(|l| l[0].x == l[1].x).map(|l| l[0].x).collect();
    let mut ys: Vec<f32> = lines.iter().filter(|l| l[0].y == l[1].y).map(|l| l[0].y).collect();
    xs.sort_by(f32::total_cmp);
    ys.sort_by(f32::total_cmp);
    assert!(xs.len() == 2 && ys.len() == 2);
    assert!(xs.iter().zip([100.0, 200.0]).all(|(a, b)| near(*a, b, 1e-3)));
    assert!(ys.iter().zip([30.0, 60.0]).all(|(a, b)| near(*a, b, 1e-3)));
}

fn area(r: &Rect) -> f32 {
    r.width() * r.height()
}

/// Le recouvrement de deux rectangles (aire de leur intersection).
fn overlap(a: &Rect, b: &Rect) -> f32 {
    let i = a.intersect(*b);
    if i.is_positive() {
        area(&i)
    } else {
        0.0
    }
}

#[test]
fn the_spiral_squares_fill_the_frame_in_every_orientation_and_aspect() {
    for (w, h) in [(1920.0_f32, 1080.0_f32), (1080.0, 1920.0), (2390.0, 1000.0), (1000.0, 1000.0), (PHI * 1000.0, 1000.0)] {
        let frame = Rect::from_min_size(pos2(0.0, 0.0), eframe::egui::vec2(w, h)).expand(0.01);
        let mut arcs = Vec::new();
        for variant in 0..8u8 {
            let s = spiral(w, h, variant);
            assert_eq!(s.squares.len(), cadrage::SPIRAL_SQUARES);
            // Tous les carrés sont dans le cadre, sans se recouvrir.
            for (i, a) in s.squares.iter().enumerate() {
                assert!(frame.contains_rect(*a), "{w}×{h} orientation {variant} : carré {i} {a:?} hors du cadre");
                for b in &s.squares[i + 1..] {
                    assert!(overlap(a, b) < area(a) * 1e-3, "{w}×{h} orientation {variant} : deux carrés se recouvrent");
                }
            }
            // Ils couvrent presque tout le cadre (il reste le tout petit rectangle du centre de la spirale).
            let covered: f32 = s.squares.iter().map(area).sum();
            assert!(covered / (w * h) > 0.99 && covered / (w * h) <= 1.0 + 1e-4, "{w}×{h} : {} du cadre", covered / (w * h));
            // La spirale reste dans le cadre, d'un seul tenant (de petits pas, sans saut).
            assert!(s.arc.iter().all(|p| frame.contains(*p)), "{w}×{h} orientation {variant} : la spirale sort du cadre");
            let longest = s.arc.windows(2).map(|p| p[0].distance(p[1])).fold(0.0, f32::max);
            assert!(longest < 0.1 * w.max(h), "{w}×{h} orientation {variant} : saut de {longest}");
            // Elle part d'un coin du cadre.
            let start = s.arc[0];
            assert!([(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].iter().any(|c| near(start.x, c.0, 1e-2) && near(start.y, c.1, 1e-2)), "{w}×{h} orientation {variant} : départ {start:?}");
            // Les carrés se suivent en rapetissant.
            assert!(s.squares.windows(2).all(|p| area(&p[1]) < area(&p[0])));
            arcs.push(s.arc);
        }
        if w != h {
            // Huit orientations différentes (sauf un cadre carré, où des retournements se confondent).
            for i in 0..8 {
                for j in i + 1..8 {
                    assert_ne!(arcs[i], arcs[j], "{w}×{h} : orientations {i} et {j} identiques");
                }
            }
        }
    }
}

#[test]
fn the_spiral_of_a_golden_frame_is_made_of_true_squares_and_follows_phi() {
    let s = spiral(PHI * 1000.0, 1000.0, 0);
    for r in &s.squares {
        assert!(near(r.width(), r.height(), 0.05), "{r:?} est un vrai carré");
    }
    // Le premier carré fait toute la hauteur, les suivants rapetissent de 1 / phi.
    assert!(near(s.squares[0].width(), 1000.0, 0.05));
    assert!(s.squares.windows(2).all(|p| near(p[1].width() / p[0].width(), 1.0 / PHI, 2e-3)));
    // Dans un 16:9, le grand carré est étiré : il couvre 1 / phi de la largeur et toute la hauteur.
    let w = spiral(1920.0, 1080.0, 0);
    assert!(near(w.squares[0].width(), 1920.0 / PHI, 0.1) && near(w.squares[0].height(), 1080.0, 0.1));
    // En 9:16 (variante 0 aussi), c'est la même forme étirée à la verticale.
    let t = spiral(1080.0, 1920.0, 0);
    assert!(near(t.squares[0].width(), 1080.0 / PHI, 0.1) && near(t.squares[0].height(), 1920.0, 0.1));
}

#[test]
fn the_reciprocal_diagonals_are_perpendicular_to_a_main_diagonal() {
    for (w, h) in [(1920.0_f32, 1080.0_f32), (1080.0, 1920.0), (2390.0, 1000.0), (500.0, 500.0)] {
        let d = diagonals(w, h);
        assert_eq!(d.len(), 6, "deux diagonales et quatre réciproques");
        let dir = |s: &[eframe::egui::Pos2; 2]| s[1] - s[0];
        let (a, b) = (dir(&d[0]), dir(&d[1]));
        assert_eq!((d[0][0], d[0][1]), (pos2(0.0, 0.0), pos2(w, h)));
        assert_eq!((d[1][0], d[1][1]), (pos2(w, 0.0), pos2(0.0, h)));
        // Les réciproques des coins haut-droite et bas-gauche tombent sur la diagonale haut-gauche / bas-droite…
        for r in &d[2..4] {
            assert!(dir(r).dot(a).abs() < 1e-2 * dir(r).length() * a.length(), "{w}×{h} : {r:?} n'est pas perpendiculaire à la diagonale");
            let foot = r[1] - d[0][0];
            assert!((foot.x * a.y - foot.y * a.x).abs() < 1e-2 * foot.length().max(1.0) * a.length(), "le pied est sur la diagonale");
        }
        // … celles des deux autres coins sur l'autre diagonale.
        for r in &d[4..6] {
            assert!(dir(r).dot(b).abs() < 1e-2 * dir(r).length() * b.length(), "{w}×{h} : {r:?} n'est pas perpendiculaire à l'autre diagonale");
            let foot = r[1] - d[1][0];
            assert!((foot.x * b.y - foot.y * b.x).abs() < 1e-2 * foot.length().max(1.0) * b.length());
        }
        // Chacune part d'un coin du cadre, et les quatre coins sont servis.
        let corners = [pos2(0.0, 0.0), pos2(w, 0.0), pos2(w, h), pos2(0.0, h)];
        let mut served: Vec<_> = d[2..].iter().map(|r| r[0]).collect();
        served.sort_by(|p, q| (p.x, p.y).partial_cmp(&(q.x, q.y)).unwrap());
        let mut all = corners.to_vec();
        all.sort_by(|p, q| (p.x, p.y).partial_cmp(&(q.x, q.y)).unwrap());
        assert_eq!(served, all);
    }
    // Dans un carré, la réciproque d'un coin tombe au centre (les diagonales se coupent à angle droit).
    let d = diagonals(100.0, 100.0);
    assert!(near(d[2][1].x, 50.0, 1e-3) && near(d[2][1].y, 50.0, 1e-3));
}

#[test]
fn the_center_cross_is_two_short_strokes_through_the_center() {
    let c = center_cross(1920.0, 1080.0);
    assert_eq!(c.len(), 2);
    for l in &c {
        assert!(near((l[0].x + l[1].x) / 2.0, 960.0, 1e-3) && near((l[0].y + l[1].y) / 2.0, 540.0, 1e-3));
    }
    assert!(near(c[0][1].x - c[0][0].x, 2.0 * 0.025 * 1080.0, 1e-3));
}

#[test]
fn only_the_chosen_aids_are_built() {
    let none = FramingAids::default();
    assert_eq!(build(1920.0, 1080.0, &none), cadrage::Geometry::default(), "rien de choisi : rien de tracé");
    let thirds_only = FramingAids { thirds: true, ..none.clone() };
    let g = build(1920.0, 1080.0, &thirds_only);
    assert_eq!((g.lines.len(), g.faint.len(), g.dots.len()), (4, 0, 0));
    let phi = FramingAids { phi_grid: true, ..none.clone() };
    let g = build(1920.0, 1080.0, &phi);
    assert_eq!((g.lines.len(), g.dots.len()), (4, 4));
    let spiral = FramingAids { spiral: true, ..none.clone() };
    let g = build(1920.0, 1080.0, &spiral);
    assert_eq!((g.lines.len(), g.faint.len()), (1, cadrage::SPIRAL_SQUARES), "la spirale et ses carrés (discrets)");
    let diag = FramingAids { diagonals: true, ..none.clone() };
    assert_eq!(build(1920.0, 1080.0, &diag).lines.len(), 6);
    let cross = FramingAids { cross: true, ..none.clone() };
    assert_eq!(build(1920.0, 1080.0, &cross).lines.len(), 2);
    let all = FramingAids { thirds: true, phi_grid: true, spiral: true, diagonals: true, cross: true, ..none };
    assert_eq!(build(1920.0, 1080.0, &all).lines.len(), 4 + 4 + 1 + 6 + 2, "chaque aide s'ajoute aux autres");
    assert!(all.any_on());
}

#[test]
fn the_geometry_is_computed_once_per_frame_size_and_choice() {
    let mut cache = Cache::default();
    let mut s = FramingAids { thirds: true, spiral: true, ..Default::default() };
    cache.get(1920.0, 1080.0, &s);
    cache.get(1920.0, 1080.0, &s);
    assert_eq!(cache.builds, 1, "même cadre, mêmes repères : le jeu est gardé");
    // L'aspect (couleur, opacité, épaisseur) ne change pas la géométrie.
    s.color = [255, 0, 0];
    s.opacity = 0.3;
    s.width = 3.0;
    cache.get(1920.0, 1080.0, &s);
    assert_eq!(cache.builds, 1);
    // Une autre taille, une autre orientation de la spirale, un autre repère : on recalcule.
    cache.get(1280.0, 720.0, &s);
    assert_eq!(cache.builds, 2);
    s.next_variant();
    cache.get(1280.0, 720.0, &s);
    assert_eq!(cache.builds, 3);
    s.diagonals = true;
    cache.get(1280.0, 720.0, &s);
    assert_eq!(cache.builds, 4);
    // Spirale éteinte : son orientation ne compte plus.
    s.spiral = false;
    cache.get(1280.0, 720.0, &s);
    let b = cache.builds;
    s.next_variant();
    cache.get(1280.0, 720.0, &s);
    assert_eq!(cache.builds, b);
}

#[test]
fn the_spiral_orientation_cycles_through_eight() {
    let mut s = FramingAids::default();
    let seen: Vec<u8> = (0..9)
        .map(|_| {
            let v = s.spiral_variant;
            s.next_variant();
            v
        })
        .collect();
    assert_eq!(seen, vec![0, 1, 2, 3, 4, 5, 6, 7, 0]);
    assert_eq!(cadrage::variant_label(0), "1 / 8");
    assert_eq!(cadrage::variant_label(7), "8 / 8");
}

#[test]
fn defaults_are_all_off_and_settings_are_kept() {
    let d = FramingAids::default();
    assert!(!d.any_on() && !d.thirds && !d.phi_grid && !d.spiral && !d.diagonals && !d.cross);
    assert_eq!(d.spiral_variant, 0);
    assert_eq!(d.safe, SafeZones::Ebu);
    assert!((d.opacity - cadrage::DEFAULT_OPACITY).abs() < 1e-6 && d.width > 0.0);
    // Un fichier de réglages sans la table des aides au cadrage : les valeurs par défaut.
    let old: Settings = toml::from_str("").expect("réglages vides");
    assert_eq!(old.framing, FramingAids::default());
    let partial: Settings = toml::from_str("[aides_cadrage]\ntiers = true\n").expect("table partielle");
    assert!(partial.framing.thirds && !partial.framing.spiral && partial.framing.opacity == d.opacity);
    // Écrire puis relire.
    let mut s = Settings::default();
    s.framing = FramingAids { thirds: true, phi_grid: true, spiral: true, spiral_variant: 5, diagonals: true, cross: true, safe: SafeZones::Large, color: [1, 2, 3], opacity: 0.5, width: 2.5, ..Default::default() };
    let text = toml::to_string(&s).expect("écriture");
    assert!(text.contains("[aides_cadrage]") && text.contains("grille_or = true") && text.contains("spirale_orientation = 5"), "{text}");
    let back: Settings = toml::from_str(&text).expect("relecture");
    assert_eq!(back.framing, s.framing);
    // Des valeurs absurdes sont ramenées dans les bornes.
    let wild = FramingAids { spiral_variant: 11, opacity: f32::NAN, width: -4.0, ..Default::default() }.sanitized();
    assert_eq!(wild.spiral_variant, 3);
    assert!((wild.opacity - cadrage::DEFAULT_OPACITY).abs() < 1e-6 && wild.width >= 0.5);
    assert!((FramingAids { opacity: 9.0, ..Default::default() }.sanitized().opacity - 1.0).abs() < 1e-6);
}

#[test]
fn the_default_guides_are_thin_and_discreet() {
    let d = FramingAids::default();
    assert!(d.width <= 1.0, "épaisseur par défaut {} px", d.width);
    assert!(d.opacity < 0.85);
}

#[test]
fn an_old_settings_file_with_the_old_default_look_gets_the_new_default() {
    // Un fichier d'avant : l'épaisseur (1,3) et l'opacité (0,85) de l'ancien défaut, sans version d'aspect.
    let old = "[aides_cadrage]\ntiers = true\nepaisseur = 1.3\nopacite = 0.85\n";
    let s: Settings = toml::from_str(old).expect("ancien fichier");
    assert_eq!(s.framing.look_version, 0, "sans version : un fichier d'avant");
    let m = s.framing.sanitized();
    assert!(m.thirds, "les choix sont gardés");
    assert_eq!((m.width, m.opacity), (cadrage::DEFAULT_WIDTH, cadrage::DEFAULT_OPACITY));
    assert_eq!(m.look_version, 1);
    // Un fichier d'avant où l'on avait choisi autre chose : on le garde.
    let custom: Settings = toml::from_str("[aides_cadrage]\nepaisseur = 2.5\nopacite = 0.4\n").expect("réglages choisis");
    let m = custom.framing.sanitized();
    assert_eq!((m.width, m.opacity), (2.5, 0.4));
    // Un fichier déjà migré qui garde 1,3 px par choix : on ne le retouche plus.
    let chosen: Settings = toml::from_str("[aides_cadrage]\nepaisseur = 1.3\nversion_aspect = 1\n").expect("choix");
    assert_eq!(chosen.framing.sanitized().width, 1.3);
    // Les réglages écrits portent la version, et sans table du tout c'est le nouveau défaut.
    let text = toml::to_string(&Settings::default()).expect("écriture");
    assert!(text.contains("version_aspect = 1"), "{text}");
    let none: Settings = toml::from_str("").expect("vide");
    assert_eq!(none.framing.sanitized(), FramingAids::default());
}

#[test]
fn the_color_carries_the_opacity_and_safe_zones_have_two_pairs() {
    let c = FramingAids { color: [10, 20, 30], opacity: 0.5, ..Default::default() }.stroke_color();
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    assert!((r as i32 - 10).abs() <= 1 && (g as i32 - 20).abs() <= 1 && (b as i32 - 30).abs() <= 1, "{r} {g} {b}");
    assert!((a as i32 - 128).abs() <= 1);
    assert_eq!(SafeZones::Ebu.parts(), (0.93, 0.90));
    assert_eq!(SafeZones::Large.parts(), (0.90, 0.80));
    assert!(SafeZones::Large.label().contains("à vérifier"));
    // Le 2,39:1 (anamorphique), le 1,85:1 et le 4:3 sont des formats proposés.
    for r in [[2.39, 1.0], [1.85, 1.0], [4.0, 3.0], [16.0, 9.0], [9.0, 16.0], [1.0, 1.0], [4.0, 5.0]] {
        assert!(crate::frames::is_preset(r), "{r:?}");
    }
    // Éteindre les repères ne touche pas aux zones de sécurité.
    let mut s = FramingAids { thirds: true, safe: SafeZones::Large, ..Default::default() };
    s.clear();
    assert!(!s.any_on());
    assert_eq!(s.safe, SafeZones::Large);
}
