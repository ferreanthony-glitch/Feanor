//! Les effets et ce qui dépasse de la feuille (partie 119), **vus en pixels** : le trait qui passe le bord de la feuille ne doit se casser avec aucun effet.
//!
//! La mesure de la cassure (`couture`) : deux colonnes voisines, la dernière de la feuille et la première au-delà. On y relève le centre de l'encre, sa quantité et sa couleur.
//! Un trait droit, passé par un effet continu, ne saute pas d'une colonne à la suivante. Pour les effets du trait, on compare à la même image sur une feuille assez grande.

use eframe::egui::Color32;

use crate::fx::{Effect, EffectKind, FxStep};
use crate::raster::{outer_id, Raster, TILE};

pub(crate) const ROUGE: Color32 = Color32::from_rgb(200, 30, 40);
const W: usize = 640;
/// (La hauteur n'est pas un multiple de la tuile : la dernière rangée de tuiles est à cheval sur le bord.)
const H: usize = 328;

fn poser(r: &mut Raster, x: i64, y: i64, c: Color32) {
    let t = TILE as i64;
    let id = r.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
    r.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = c;
}

fn pave(r: &mut Raster, x0: i64, x1: i64, y0: i64, y1: i64, c: Color32) {
    for y in y0..y1 {
        for x in x0..x1 {
            poser(r, x, y, c);
        }
    }
}

/// Le bord de la feuille que le trait passe.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Bord {
    Droite,
    Bas,
    Gauche,
}

const BORDS: [Bord; 3] = [Bord::Droite, Bord::Bas, Bord::Gauche];

/// Une bande de 12 pixels qui passe le bord (`w` × `h`), de 300 pixels en deçà à 200 au-delà.
fn bande(w: usize, h: usize, bord: Bord) -> Raster {
    let mut r = Raster::new(w, h);
    let (w, h) = (w as i64, h as i64);
    match bord {
        Bord::Droite => pave(&mut r, w - 300, w + 200, 100, 112, ROUGE),
        Bord::Bas => pave(&mut r, 100, 112, h - 300, h + 200, ROUGE),
        Bord::Gauche => pave(&mut r, -200, 300, 100, 112, ROUGE),
    }
    r
}

/// Les réglages qui montrent l'effet au point où le trait passe le bord.
fn reglages(kind: EffectKind, bord: Bord) -> Vec<f32> {
    let (cx, cy) = match bord {
        Bord::Droite => (100.0, 0.0),
        Bord::Bas => (0.0, 100.0),
        Bord::Gauche => (0.0, 0.0),
    };
    let mut v = Effect::new(kind).values.clone();
    match kind {
        EffectKind::Wave => v = vec![10.0, 120.0, 30.0],
        EffectKind::Ripple => v = vec![8.0, 60.0, cx, cy],
        EffectKind::Twirl => v = vec![90.0, 150.0, cx, cy],
        EffectKind::Pinch => v = vec![50.0, 150.0, cx, cy],
        EffectKind::Shake => v = vec![30.0, 14.0, 2.0, 110.0, 0.0],
        EffectKind::Vignette => v[1] = 40.0,
        EffectKind::Aberration => v = vec![30.0, cx, cy],
        EffectKind::Flash => v[0] = 50.0,
        EffectKind::Boil => {
            v[crate::tremblement::INTENSITY] = 6.0;
            v.push(1.0);
        }
        EffectKind::Grain => v.push(3.0),
        _ => {}
    }
    v
}

fn pas(kind: EffectKind, bord: Bord) -> [FxStep; 1] {
    [FxStep { kind, values: reglages(kind, bord) }]
}

/// Ce qu'on relève sur une colonne (ou une rangée) perpendiculaire au trait.
#[derive(Clone, Copy, Debug, Default)]
struct Coupe {
    centre: f32,
    encre: f32,
    couleur: [f32; 3],
}

/// La coupe `k` le long du trait (`k` : abscisse pour un trait horizontal, ordonnée pour un trait vertical). `large` : ne lire que le milieu du trait (un effet qui remplit la feuille).
fn coupe(r: &Raster, bord: Bord, k: i64, large: bool) -> Coupe {
    let (mut somme, mut pond, mut c) = (0.0f32, 0.0f32, [0.0f32; 3]);
    let etendue = if large { 20..220i64 } else { 103..109i64 };
    for t in etendue {
        let p = if bord == Bord::Bas { r.get_i(t, k) } else { r.get_i(k, t) };
        let a = p.a() as f32;
        somme += a;
        pond += a * t as f32;
        c[0] += p.r() as f32;
        c[1] += p.g() as f32;
        c[2] += p.b() as f32;
    }
    if somme <= 0.0 {
        return Coupe::default();
    }
    Coupe { centre: pond / somme, encre: somme, couleur: c.map(|v| v * 255.0 / somme) }
}

/// (saut du centre de l'encre en pixels, écart de la quantité d'encre en %, écart de couleur sur 255) de part et d'autre de `bord`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Couture {
    saut: f32,
    encre: f32,
    couleur: f32,
}

fn couture(r: &Raster, bord: Bord, large: bool) -> Couture {
    let k = match bord {
        Bord::Droite => W as i64,
        Bord::Bas => H as i64,
        Bord::Gauche => 0,
    };
    let (a, b) = (coupe(r, bord, k - 1, large), coupe(r, bord, k, large));
    let plus = a.encre.max(b.encre).max(1.0);
    Couture {
        saut: if a.encre > 0.0 && b.encre > 0.0 { (a.centre - b.centre).abs() } else { 0.0 },
        encre: (a.encre - b.encre).abs() * 100.0 / plus,
        couleur: (0..3).map(|i| (a.couleur[i] - b.couleur[i]).abs()).fold(0.0, f32::max),
    }
}

/// L'effet remplit la feuille là où rien n'est dessiné : seul le milieu du trait se compare.
fn remplit(kind: EffectKind) -> bool {
    matches!(kind, EffectKind::Flash | EffectKind::Impact)
}

/// Les effets sur les pixels, sauf ceux réservés au dessin vectoriel.
fn effets_des_pixels() -> Vec<EffectKind> {
    let mut v: Vec<EffectKind> = EffectKind::ALL.into_iter().filter(|k| !k.vector_only() && !k.vector_stage() || *k == EffectKind::Boil).collect();
    v.dedup();
    v
}

// ------------------------------------------------------------------------------ la couture, effet par effet

/// Point 1 et 3 : d'un côté et de l'autre du bord, le même trait, avec chaque effet sur les pixels.
#[test]
fn no_effect_breaks_a_stroke_at_the_edge_of_the_sheet() {
    let mut echecs = Vec::new();
    for kind in effets_des_pixels() {
        for bord in BORDS {
            let mut etapes = pas(kind, bord);
            // (La lueur diffuse ne prend que ce qui brille : un seuil que le rouge passe.)
            if kind == EffectKind::Bloom {
                etapes[0].values[2] = 40.0;
            }
            let out = crate::fx::apply(&bande(W, H, bord), &etapes);
            let m = couture(&out, bord, !remplit(kind));
            println!("COUTURE {kind:?} {bord:?} : saut {:.2} px, encre {:.1} %, couleur {:.1}", m.saut, m.encre, m.couleur);
            if m.saut > 1.5 || m.encre > 25.0 || m.couleur > 20.0 {
                echecs.push(format!("{kind:?} {bord:?} : saut {:.2} px, encre {:.1} %, couleur {:.1}", m.saut, m.encre, m.couleur));
            }
        }
    }
    assert!(echecs.is_empty(), "des cassures au bord de la feuille :\n{}", echecs.join("\n"));
}

// ------------------------------------------------------------------------------ le motif et la grille à la couture

/// La bande de `bande` sur une feuille assez grande pour tout tenir (même origine : même grille, même hasard).
fn grande_bande(bord: Bord) -> Raster {
    let mut r = Raster::new(W * 2, H * 2);
    let (w, h) = (W as i64, H as i64);
    match bord {
        Bord::Droite => pave(&mut r, w - 300, w + 200, 100, 112, ROUGE),
        Bord::Bas => pave(&mut r, 100, 112, h - 300, h + 200, ROUGE),
        Bord::Gauche => unreachable!("une feuille ne s'étend pas à gauche de son origine"),
    }
    r
}

/// Combien de pixels diffèrent de `reference` sur l'étendue donnée, et de combien au plus (sur 255) ?
fn ecart(a: &Raster, reference: &Raster, xs: std::ops::Range<i64>, ys: std::ops::Range<i64>) -> (usize, i32) {
    let (mut n, mut pire) = (0, 0);
    for y in ys {
        for x in xs.clone() {
            let (p, q) = (a.get_i(x, y).to_array(), reference.get_i(x, y).to_array());
            let d = (0..4).map(|i| (p[i] as i32 - q[i] as i32).abs()).max().unwrap_or(0);
            if d > 2 {
                n += 1;
            }
            pire = pire.max(d);
        }
    }
    (n, pire)
}

/// Point 3 : la grille de la mosaïque et le hasard du bruit et du grain se suivent sans décalage à travers le bord, comme sur une feuille plus grande.
#[test]
fn the_pattern_of_mosaic_noise_and_grain_is_aligned_at_the_seam() {
    for kind in [EffectKind::Mosaic, EffectKind::Noise, EffectKind::Grain] {
        let reglages = |_| pas(kind, Bord::Droite);
        for bord in [Bord::Droite, Bord::Bas] {
            let (a, reference) = (crate::fx::apply(&bande(W, H, bord), &reglages(bord)), crate::fx::apply(&grande_bande(bord), &reglages(bord)));
            let (xs, ys) = match bord {
                Bord::Droite => ((W as i64 - 80)..(W as i64 + 190), 90..122),
                Bord::Bas => (90..122, (H as i64 - 80)..(H as i64 + 190)),
                Bord::Gauche => unreachable!(),
            };
            let (n, pire) = ecart(&a, &reference, xs, ys);
            println!("MOTIF {kind:?} {bord:?} : {n} pixels différents d'une feuille plus grande, {pire} au plus");
            assert_eq!(n, 0, "{kind:?} {bord:?} : le motif est décalé à la couture ({n} pixels, jusqu'à {pire})");
        }
    }
}

/// Le motif continue aussi à gauche de la feuille, là où les coordonnées sont négatives : une image à l'origine décalée donne la même chose.
#[test]
fn the_pattern_has_no_seam_on_the_negative_side_either() {
    for kind in [EffectKind::Mosaic, EffectKind::Noise, EffectKind::Grain] {
        let out = crate::fx::apply(&bande(W, H, Bord::Gauche), &pas(kind, Bord::Gauche));
        if kind == EffectKind::Mosaic {
            // Des carrés de 10 pixels de la grille du monde : le carré est uni, et ses bords tombent sur des multiples de 10, côté négatif compris.
            let carre = |x: i64, y: i64| out.get_i(x, y);
            for y in 96..116 {
                for x in -40i64..-30 {
                    assert_eq!(carre(x, y), carre(-40, y), "({x}, {y}) : la mosaïque suit la grille du monde");
                }
            }
        } else {
            let touches = (-150i64..-50).flat_map(|x| (100..112).map(move |y| (x, y))).filter(|&(x, y)| out.get_i(x, y) != ROUGE).count();
            assert!(touches > 100, "{kind:?} : l'effet s'applique à gauche de la feuille ({touches} pixels changés)");
        }
    }
}

// ------------------------------------------------------------------------------ les effets du trait

fn trait_long(w: usize, h: usize) -> crate::vector::VectorDrawing {
    use crate::vector::{Element, Trace, VPoint, VectorDrawing};
    let _ = (w, h);
    let points: Vec<VPoint> = (0..240).map(|k| VPoint { x: 300.0 + k as f32 * 3.0, y: 150.0 + (k as f32 * 0.02).sin() * 12.0, r: 4.0 }).collect();
    VectorDrawing { elements: vec![Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: Trace::default() }] }
}

const EFFETS_DU_TRAIT: [EffectKind; 8] = [EffectKind::Write, EffectKind::Sketch, EffectKind::Breath, EffectKind::StrokeWave, EffectKind::Dashes, EffectKind::Dust, EffectKind::Dry, EffectKind::StrokeGradient];

fn pas_du_trait(kind: EffectKind) -> Vec<FxStep> {
    let mut e = Effect::new(kind);
    if kind == EffectKind::Dust {
        e.values[crate::fxtrait::DUST_SPREAD] = 30.0;
        e.values[crate::fxtrait::DUST_DRIFT] = 10.0;
    }
    crate::fx::steps_at_with(&[e], 5, None, true)
}

/// Un effet du trait sur une feuille à passer et sur une feuille assez grande : (pixels différents autour de la couture, encre au-delà, encre attendue au-delà).
fn trait_et_reference(kind: EffectKind) -> ((usize, i32), usize, usize, Raster) {
    let v = trait_long(W, H);
    let steps = pas_du_trait(kind);
    assert!(!steps.is_empty(), "{kind:?} change quelque chose");
    let out = crate::fx::apply_with(&v.render(W, H), Some(&v), &steps, 0);
    let (gw, gh) = (W * 3, H);
    let reference = crate::fx::apply_with(&v.render(gw, gh), Some(&v), &steps, 0);
    let bord = W as i64;
    let e = ecart(&out, &reference, (bord - 40)..(bord + 40), 40..260);
    let encre = |r: &Raster| (bord..(bord + 400)).flat_map(|x| (20..300).map(move |y| (x, y))).filter(|&(x, y)| r.get_i(x, y).a() > 32).count();
    (e, encre(&out), encre(&reference), out)
}

/// Point 1 : l'effet du trait continue de l'autre côté du bord, comme sur une feuille plus grande.
#[test]
fn stroke_effects_continue_across_the_edge() {
    let mut echecs = Vec::new();
    for kind in EFFETS_DU_TRAIT {
        let ((n, pire), encre, attendu, _) = trait_et_reference(kind);
        println!("TRAIT {kind:?} : {n} pixels différents d'une feuille plus grande autour du bord (jusqu'à {pire}), encre au-delà {encre} pour {attendu} attendus");
        if n > 40 || (encre as i64 - attendu as i64).unsigned_abs() as usize * 10 > attendu.max(1) {
            echecs.push(format!("{kind:?} : {n} pixels (jusqu'à {pire}), encre {encre} pour {attendu}"));
        }
    }
    assert!(echecs.is_empty(), "des cassures sur les effets du trait :\n{}", echecs.join("\n"));
}

/// Point 2 : Écriture et Poussière ne font plus disparaître le trait au-delà de la feuille.
#[test]
fn write_and_dust_show_the_stroke_beyond_the_sheet() {
    for kind in [EffectKind::Write, EffectKind::Dust] {
        let (_, encre, attendu, out) = trait_et_reference(kind);
        println!("MONTRE {kind:?} : encre au-delà de la feuille {encre}, attendu {attendu}");
        assert!(attendu > 500, "{kind:?} : la référence montre le trait au-delà");
        assert!(encre * 10 >= attendu * 9, "{kind:?} : le trait a disparu au-delà de la feuille ({encre} pour {attendu})");
        assert!(out.has_beyond(), "{kind:?}");
    }
}

/// L'écriture cache ce qu'elle n'a pas encore écrit : la fin du trait, au-delà de la feuille, reste cachée tant que l'avancement n'y est pas ; elle apparaît à 100 %.
#[test]
fn the_write_effect_reveals_the_end_of_the_stroke_in_its_turn() {
    let v = trait_long(W, H);
    let raster = v.render(W, H);
    let encre = |avancement: f32| {
        let mut e = Effect::new(EffectKind::Write);
        e.values[0] = avancement;
        let out = crate::fx::apply_with(&raster, Some(&v), &crate::fx::steps_at_with(&[e], 0, None, true), 0);
        (W as i64..1100).flat_map(|x| (20..300).map(move |y| (x, y))).filter(|&(x, y)| out.get_i(x, y).a() > 32).count()
    };
    let (a, b, c) = (encre(10.0), encre(70.0), encre(100.0));
    println!("ECRITURE au-delà de la feuille : à 10 % {a}, à 70 % {b}, à 100 % {c}");
    assert_eq!(a, 0, "à 10 %, le trait n'est pas encore passé le bord");
    assert!(b > 200 && b < c, "à 70 %, il l'a passé : {b} (à 100 % : {c})");
    assert!(c > 1000);
}

// ------------------------------------------------------------------------------ sans débord, rien ne change

/// Une encre qui touche les bords sans les passer, de quoi faire travailler chaque effet.
fn sans_debord(w: usize, h: usize) -> Raster {
    let mut r = Raster::new(w, h);
    let (w, h) = (w as i64, h as i64);
    pave(&mut r, 0, w, 100, 112, ROUGE);
    pave(&mut r, 100, 112, 0, h, Color32::from_rgba_premultiplied(20, 90, 60, 160));
    pave(&mut r, w - 40, w, h - 40, h, Color32::from_rgb(30, 60, 200));
    r
}

fn empreinte(r: &Raster) -> u64 {
    let mut h = 0xcbf2_9ce4_8423_2325u64;
    let mut mele = |v: u8| {
        h ^= v as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    for y in 0..r.height {
        for x in 0..r.width {
            let c = r.get(x, y).to_array();
            for v in c {
                mele(v);
            }
        }
    }
    assert!(!r.has_beyond(), "rien au-delà de la feuille");
    h
}

/// Les empreintes de ces images, sur `main` avant ce chantier (97722ec). Un dessin qui reste dans la feuille ne change pas d'un pixel.
const EMPREINTES_DE_MAIN: &[(&str, u64)] = &[
    ("GaussianBlur Droite", 0x33a754fc97c4a61b),
    ("GaussianBlur Bas", 0x33a754fc97c4a61b),
    ("GaussianBlur Gauche", 0x33a754fc97c4a61b),
    ("MotionBlur Droite", 0x530a7fa7213be2d5),
    ("MotionBlur Bas", 0x530a7fa7213be2d5),
    ("MotionBlur Gauche", 0x530a7fa7213be2d5),
    ("BrightnessContrast Droite", 0xf4b9050d1dec025),
    ("BrightnessContrast Bas", 0xf4b9050d1dec025),
    ("BrightnessContrast Gauche", 0xf4b9050d1dec025),
    ("Levels Droite", 0xf4b9050d1dec025),
    ("Levels Bas", 0xf4b9050d1dec025),
    ("Levels Gauche", 0xf4b9050d1dec025),
    ("HueSaturation Droite", 0xf4b9050d1dec025),
    ("HueSaturation Bas", 0xf4b9050d1dec025),
    ("HueSaturation Gauche", 0xf4b9050d1dec025),
    ("BlackWhite Droite", 0x65f9193c04af0265),
    ("BlackWhite Bas", 0x65f9193c04af0265),
    ("BlackWhite Gauche", 0x65f9193c04af0265),
    ("Invert Droite", 0xf3103368ad307d25),
    ("Invert Bas", 0xf3103368ad307d25),
    ("Invert Gauche", 0xf3103368ad307d25),
    ("Posterize Droite", 0x332a4e73ea52945),
    ("Posterize Bas", 0x332a4e73ea52945),
    ("Posterize Gauche", 0x332a4e73ea52945),
    ("Threshold Droite", 0x6fcbc9cca1633fa5),
    ("Threshold Bas", 0x6fcbc9cca1633fa5),
    ("Threshold Gauche", 0x6fcbc9cca1633fa5),
    ("Sharpen Droite", 0x30cf689a202c552d),
    ("Sharpen Bas", 0x30cf689a202c552d),
    ("Sharpen Gauche", 0x30cf689a202c552d),
    ("Noise Droite", 0xe9e2f0a93980f5b0),
    ("Noise Bas", 0xe9e2f0a93980f5b0),
    ("Noise Gauche", 0xe9e2f0a93980f5b0),
    ("Mosaic Droite", 0xbc43e7ff43d882b5),
    ("Mosaic Bas", 0xbc43e7ff43d882b5),
    ("Mosaic Gauche", 0xbc43e7ff43d882b5),
    ("DropShadow Droite", 0xc2ac26cfb1fd5d39),
    ("DropShadow Bas", 0xc2ac26cfb1fd5d39),
    ("DropShadow Gauche", 0xc2ac26cfb1fd5d39),
    ("Glow Droite", 0x38f2d054c1038c40),
    ("Glow Bas", 0x38f2d054c1038c40),
    ("Glow Gauche", 0x38f2d054c1038c40),
    ("Wave Droite", 0xc37452722bd8c427),
    ("Wave Bas", 0xc37452722bd8c427),
    ("Wave Gauche", 0xc37452722bd8c427),
    ("Ripple Droite", 0x79f2cb68315716b7),
    ("Ripple Bas", 0x710a22c8098869ae),
    ("Ripple Gauche", 0xdf91b9fda5e3e4a2),
    ("Twirl Droite", 0xc97a424725bb9864),
    ("Twirl Bas", 0xc5f11b4615952671),
    ("Twirl Gauche", 0x906624bdb63c55c3),
    ("Pinch Droite", 0x663cba9563c89207),
    ("Pinch Bas", 0x32e0eb6d0f98b596),
    ("Pinch Gauche", 0x1c73b1987b5450ca),
    ("Clouds Droite", 0xb8707625f33223d5),
    ("Clouds Bas", 0xb8707625f33223d5),
    ("Clouds Gauche", 0xb8707625f33223d5),
    ("Lighting Droite", 0xb7564f28596def4d),
    ("Lighting Bas", 0xb7564f28596def4d),
    ("Lighting Gauche", 0xb7564f28596def4d),
    ("Gradient Droite", 0xe6fd48269e0e85),
    ("Gradient Bas", 0xe6fd48269e0e85),
    ("Gradient Gauche", 0xe6fd48269e0e85),
    ("Boil Droite", 0x9b79956a1866d903),
    ("Boil Bas", 0x9b79956a1866d903),
    ("Boil Gauche", 0x9b79956a1866d903),
    ("Flash Droite", 0xc3d676925c19b1a5),
    ("Flash Bas", 0xc3d676925c19b1a5),
    ("Flash Gauche", 0xc3d676925c19b1a5),
    ("Impact Droite", 0x5c1224a21081fe25),
    ("Impact Bas", 0x5c1224a21081fe25),
    ("Impact Gauche", 0x5c1224a21081fe25),
    ("Bloom Droite", 0xf4b9050d1dec025),
    ("Bloom Bas", 0xf4b9050d1dec025),
    ("Bloom Gauche", 0xf4b9050d1dec025),
    ("Shake Droite", 0xe199df7aec499e2e),
    ("Shake Bas", 0xe199df7aec499e2e),
    ("Shake Gauche", 0xe199df7aec499e2e),
    ("Vignette Droite", 0xadb12c087cf1c055),
    ("Vignette Bas", 0xadb12c087cf1c055),
    ("Vignette Gauche", 0xadb12c087cf1c055),
    ("Grain Droite", 0x6ca0b0526c08ebb7),
    ("Grain Bas", 0x6ca0b0526c08ebb7),
    ("Grain Gauche", 0x6ca0b0526c08ebb7),
    ("Aberration Droite", 0xd041f83e7e50e4f9),
    ("Aberration Bas", 0xb251b58744732df),
    ("Aberration Gauche", 0x3db64a4f32e0e829),
    ("Outline Droite", 0x2330f7ca658d9ad9),
    ("Outline Bas", 0x2330f7ca658d9ad9),
    ("Outline Gauche", 0x2330f7ca658d9ad9),
    ("Bleed Droite", 0xe5e53e4ec134132c),
    ("Bleed Bas", 0xe5e53e4ec134132c),
    ("Bleed Gauche", 0xe5e53e4ec134132c),
    ("Write trait", 0x69b6f903fe7464b1),
    ("Sketch trait", 0x1d5f1ffa5be2ca7a),
    ("Breath trait", 0xd473e5c0c81efe6f),
    ("StrokeWave trait", 0xd26f773fee007604),
    ("Dashes trait", 0x983bd0632fb58ef0),
    ("Dust trait", 0x159252231817b3d3),
    ("Dry trait", 0x13062a6dde245bcb),
    ("StrokeGradient trait", 0x108b9f6e10bce4ff),
    ("pile", 0xe7fdbdc79d9464c8),
    ("GaussianBlur aperçu", 0x50ecf6e860fa3adf),
    ("Noise aperçu", 0x532112151bcf694c),
    ("Boil aperçu", 0x7458e7f99cd39aa2),
    ("Vignette aperçu", 0xef1f92dafc82c88b),
];

/// Le trait qui reste dans la feuille, avec les effets du trait.
fn trait_dans_la_feuille() -> crate::vector::VectorDrawing {
    use crate::vector::{Element, Trace, VPoint, VectorDrawing};
    let points: Vec<VPoint> = (0..60).map(|k| VPoint { x: 40.0 + k as f32 * 9.0, y: 150.0 + (k as f32 * 0.1).sin() * 12.0, r: 4.0 }).collect();
    VectorDrawing { elements: vec![Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: Trace::default() }] }
}

fn images_sans_debord() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let r = sans_debord(W, H);
    for kind in effets_des_pixels() {
        for bord in BORDS {
            let image = crate::fx::apply(&r, &pas(kind, bord));
            out.push((format!("{kind:?} {bord:?}"), empreinte(&image)));
        }
    }
    let v = trait_dans_la_feuille();
    let dessin = v.render(W, H);
    assert!(!dessin.has_beyond());
    for kind in EFFETS_DU_TRAIT {
        out.push((format!("{kind:?} trait"), empreinte(&crate::fx::apply_with(&dessin, Some(&v), &pas_du_trait(kind), 0))));
    }
    // Une pile : le tremblement, puis un flou, puis une ombre, puis un réglage de couleur.
    let pile: Vec<FxStep> = [EffectKind::Boil, EffectKind::GaussianBlur, EffectKind::DropShadow, EffectKind::BlackWhite].into_iter().flat_map(|k| pas(k, Bord::Droite)).collect();
    out.push(("pile".into(), empreinte(&crate::fx::apply(&r, &pile))));
    // L'aperçu réduit.
    for kind in [EffectKind::GaussianBlur, EffectKind::Noise, EffectKind::Boil, EffectKind::Vignette] {
        let petit = crate::fxproxy::downscale(&r, 1);
        out.push((format!("{kind:?} aperçu"), empreinte(&crate::fx::apply_scaled(&petit, &pas(kind, Bord::Droite), 1))));
    }
    out
}

/// Un dessin sans débord ne change pas d'un pixel, effet par effet (empreintes relevées sur `main`).
#[test]
fn a_drawing_without_overflow_does_not_change_by_one_pixel() {
    let images = images_sans_debord();
    if EMPREINTES_DE_MAIN.is_empty() {
        for (nom, e) in &images {
            println!("    (\"{nom}\", {e:#x}),");
        }
        panic!("les empreintes de main ne sont pas encore relevées");
    }
    let mut diff = Vec::new();
    assert_eq!(images.len(), EMPREINTES_DE_MAIN.len());
    for ((nom, e), (attendu, ref_e)) in images.iter().zip(EMPREINTES_DE_MAIN) {
        assert_eq!(nom, attendu);
        if e != ref_e {
            diff.push(nom.clone());
        }
    }
    assert!(diff.is_empty(), "ces effets ont changé un dessin sans débord : {diff:?}");
}

// ------------------------------------------------------------------------------ le grand débord

/// Un trait épais du coin de la feuille 4K jusqu'à une feuille plus loin, à droite et en bas : son rectangle dépasse les 33 millions de pixels.
fn grand_debord() -> Raster {
    let (w, h) = (3840usize, 2160usize);
    let mut r = Raster::new(w, h);
    for k in 0..7600i64 {
        let (x, y) = (100 + k, 100 + k * 43 / 76);
        pave(&mut r, x, x + 2, y, y + 14, ROUGE);
    }
    r
}

/// Point 4 : un débord d'une feuille à droite et en bas garde son flou et sa lueur hors de la feuille.
#[test]
fn a_big_overflow_keeps_its_blur_and_glow_beyond_the_sheet() {
    let r = grand_debord();
    let (ux, uy) = (100 + 7000i64, 100 + 7000 * 43 / 76);
    assert!(ux > 3840 * 7 / 4 && uy > 2160 * 3 / 2);
    for kind in [EffectKind::GaussianBlur, EffectKind::Glow, EffectKind::DropShadow, EffectKind::Sharpen, EffectKind::MotionBlur] {
        let e = Effect::new(kind);
        let t = std::time::Instant::now();
        let out = crate::fx::apply(&r, &[FxStep { kind, values: e.values.clone() }]);
        println!("GRAND DEBORD {kind:?} : {} ms", t.elapsed().as_millis());
        if kind == EffectKind::Sharpen {
            continue;
        }
        // À 25 pixels du trait, là où le dessin n'a rien : le halo, le bougé ou l'ombre (le trait a changé d'aspect).
        let halo = (4..40i64).any(|d| out.get_i(ux, uy - d).a() > 0 || out.get_i(ux, uy + 14 + d).a() > 0 || out.get_i(ux - d, uy - d).a() > 0);
        assert!(halo, "{kind:?} : l'effet ne s'applique pas hors de la feuille");
    }
}

/// Le flou d'un dessin découpé en zones : les mêmes pixels que tout le dessin d'un seul tenant (une feuille assez grande pour tout tenir).
#[test]
fn computing_by_zones_gives_the_same_pixels_as_the_whole_drawing() {
    let (w, h) = (640usize, 320usize);
    let mut debord = Raster::new(w, h);
    let mut tout = Raster::new(2400, 1400);
    for k in 0..1500i64 {
        let (x, y) = (500 + k, 200 + k * 7 / 10);
        pave(&mut debord, x, x + 2, y, y + 14, ROUGE);
        pave(&mut tout, x, x + 2, y, y + 14, ROUGE);
    }
    for (kind, valeurs) in [
        (EffectKind::GaussianBlur, vec![40.0]),
        (EffectKind::GaussianBlur, vec![8.0]),
        (EffectKind::Glow, vec![12.0, 100.0]),
        (EffectKind::DropShadow, vec![10.0, 10.0, 6.0, 60.0]),
        (EffectKind::MotionBlur, vec![30.0, 20.0]),
        (EffectKind::Sharpen, vec![100.0, 2.0]),
        (EffectKind::Mosaic, vec![10.0]),
    ] {
        let steps = [FxStep { kind, values: valeurs.clone() }];
        let (a, b) = (crate::fx::apply(&debord, &steps), crate::fx::apply(&tout, &steps));
        // (Hors de la feuille et au-delà de la portée de l'effet depuis le bord : là où la feuille n'a pas son calcul à elle.)
        let (mut n, mut pire, mut encre) = (0, 0, 0);
        for y in 180..1380i64 {
            for x in 700..2380i64 {
                let (p, q) = (a.get_i(x, y).to_array(), b.get_i(x, y).to_array());
                let d = (0..4).map(|i| (p[i] as i32 - q[i] as i32).abs()).max().unwrap_or(0);
                encre += usize::from(q[3] > 0);
                if d > 0 {
                    n += 1;
                    pire = pire.max(d);
                }
            }
        }
        println!("ZONES {kind:?} {valeurs:?} : {n} pixels différents sur {encre}, {pire} au plus");
        assert!(encre > 10_000, "{kind:?}");
        assert_eq!(n, 0, "{kind:?} {valeurs:?} : les zones se voient ({n} pixels, jusqu'à {pire})");
    }
}

// ------------------------------------------------------------------------------ le fil de l'interface

/// Un plan dont le calque porte `effet`, un gros dessin qui passe le bord de la feuille.
pub(crate) fn plan_lourd(effet: EffectKind) -> crate::document::Document {
    let mut doc = crate::traits_continus_tests::des_plans(1, true);
    // (Chaque essai a ses numéros de dessin : le cache des effets les retrouve pour tout le processus.)
    static NUMEROS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(100);
    for _ in 0..NUMEROS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) {
        doc.new_cel(crate::document::LayerKind::Bitmap);
    }
    let id = doc.new_cel(crate::document::LayerKind::Bitmap);
    let (w, h) = (doc.width as i64, doc.height as i64);
    let c = doc.cels.get_mut(&id).unwrap();
    // De l'encre sur toute la feuille, et une feuille de plus à droite.
    for k in 0..48i64 {
        let y = k * h / 48;
        pave(&mut c.raster, 0, w * 2, y, y + 18, ROUGE);
    }
    c.version += 1;
    let f = doc.panels[0].frame_count().max(1);
    let l = &mut doc.panels[0].layers[0];
    l.frames.resize(f, None);
    l.frames[0] = Some(id);
    l.fx.push(Effect::new(effet));
    doc
}

/// Point 6 : ce que `Monde` fige sur le fil de l'interface ne calcule pas l'effet : un dessin sans résultat prêt se montre sans son effet, le fil de fond calcule.
#[test]
fn freezing_the_world_on_the_ui_thread_does_not_compute_the_effect() {
    let doc = plan_lourd(EffectKind::GaussianBlur);
    let plan = crate::render::panel_plan_at(&doc, 0, 0, Some(doc.paper));
    assert!(!plan.items.is_empty() && !plan.items[0].fx.is_empty());
    let (ms, couches) = crate::monde::mesure_de_l_instantane(&doc, &plan);
    println!("INSTANTANE gros dessin : {ms:.1} ms sur le fil de l'interface, {couches} couches");
    assert!(ms < 50.0, "{ms} ms sur le fil de l'interface");
}

/// (À regarder : `FEANOR_CAPTURE_DIR`.) La couture de chaque effet, grossie deux fois sur papier blanc : un trait oblique qui passe le bord, la verticale du bord marquée en haut et en bas.
#[test]
#[ignore]
fn captures_de_la_couture() {
    let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") else { return };
    let d = std::path::PathBuf::from(d);
    let _ = std::fs::create_dir_all(&d);
    use crate::vector::{Element, Trace, VPoint, VectorDrawing};
    let points: Vec<VPoint> = (0..260).map(|k| VPoint { x: 300.0 + k as f32 * 3.0, y: 90.0 + k as f32 * 0.6, r: 5.0 }).collect();
    let v = VectorDrawing { elements: vec![Element::Stroke { points, color: [200, 30, 40], opacity: 1.0, hardness: 1.0, brush: None, trace: Trace::default() }] };
    let dessin = v.render(W, H);
    let mut piles: Vec<(String, Vec<FxStep>)> = EFFETS_DU_TRAIT.iter().map(|&k| (format!("{k:?}"), pas_du_trait(k))).collect();
    for k in effets_des_pixels() {
        piles.push((format!("{k:?}"), pas(k, Bord::Droite).to_vec()));
    }
    for (nom, steps) in piles {
        let out = crate::fx::apply_with(&dessin, Some(&v), &steps, 0);
        let (x0, y0, cw, ch) = (W as i64 - 200, 60i64, 400usize, 240usize);
        let mut rgba = vec![255u8; cw * ch * 4 * 4];
        for y in 0..ch * 2 {
            for x in 0..cw * 2 {
                let (wx, wy) = (x0 + x as i64 / 2, y0 + y as i64 / 2);
                let c = out.get_i(wx, wy).to_array();
                let k = 255 - c[3] as u32;
                let mut p = [c[0] as u32 + k, c[1] as u32 + k, c[2] as u32 + k];
                if wx == W as i64 && !(14..ch * 2 - 14).contains(&y) {
                    p = [0, 120, 255];
                }
                let at = (y * cw * 2 + x) * 4;
                rgba[at..at + 3].copy_from_slice(&p.map(|v| v.min(255) as u8));
            }
        }
        let _ = image::save_buffer(d.join(format!("couture-{nom}.png")), &rgba, cw as u32 * 2, ch as u32 * 2, image::ColorType::Rgba8);
    }
}

/// (À la main : `FEANOR_PROJET_ESSAI`, le fichier d'une **copie** d'un vrai projet.) Ouvre le projet, passe chaque plan à l'écran avec le tremblement du réalisateur, relève ce qui passe le bord.
#[test]
#[ignore]
fn essai_sur_un_vrai_projet() {
    let Some(chemin) = std::env::var_os("FEANOR_PROJET_ESSAI") else { return println!("FEANOR_PROJET_ESSAI n'est pas posé") };
    let t = std::time::Instant::now();
    let lu = crate::project::load(std::path::Path::new(&chemin)).expect("ouverture");
    println!("PROJET ouvert en {} ms : {} plans, {} dessins", t.elapsed().as_millis(), lu.doc.panels.len(), lu.doc.cels.len());
    let qui_debordent = lu.doc.cels.values().filter(|c| c.raster.has_beyond()).count();
    println!("PROJET dessins qui passent le bord de la feuille : {qui_debordent}");
    let mut b = crate::perf_tests::banc(lu.doc, true);
    if b.capturer().is_none() {
        return println!("pas de carte graphique");
    }
    b.app.settings.tremblement_defaut = crate::tremblement::Defaut { actif: true, intensite: 0.5, vitesse: 3, ..Default::default() };
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.app.board_view = crate::app::BoardView::Scene;
    let n = b.app.doc.panels.len();
    let (mut pire, mut captures) = (0.0f64, 0);
    for p in 0..n {
        b.app.select_panel(p);
        let t = std::time::Instant::now();
        b.chauffer(3);
        pire = pire.max(t.elapsed().as_secs_f64() * 1000.0 / 3.0);
        let plan = crate::render::panel_plan_at(&b.app.doc, p, 0, Some(b.app.doc.paper));
        let deborde = plan.items.iter().any(|i| !i.fx.is_empty() && b.app.doc.cels.get(&i.cel).is_some_and(|c| c.raster.has_beyond()));
        if deborde && captures < 4 {
            let r = b.app.canvas_rect;
            let [w, h] = [b.app.doc.width as f32, b.app.doc.height as f32];
            b.app.view.fit(r, eframe::egui::vec2(w, h) * 2.0);
            for _ in 0..120 {
                b.image(Vec::new(), 1.0 / 60.0);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            crate::traits_continus_tests::sauver(&b.capturer().unwrap(), &format!("projet-plan-{p}"));
            captures += 1;
        }
    }
    println!("PROJET {n} plans passés, image la plus lente {pire:.1} ms en moyenne sur trois, {captures} captures, plantages notés {}", crate::lourd_tests::nb_plantages());
}

/// (Mesure.) Temps et mémoire d'un grand débord, et d'un point perdu très loin.
#[test]
#[ignore]
fn mesure_grand_debord() {
    let r = grand_debord();
    let mut perdu = crate::raster::Raster::new(3840, 2160);
    pave(&mut perdu, 3000, 4000, 1000, 1012, ROUGE);
    perdu.tile_mut(outer_id(30_000 / TILE as i32, 30_000 / TILE as i32))[0] = ROUGE;
    println!("MESURE tuiles du dessin : {} ; pic du processus au départ {:.0} Mo", r.drawn_ids().len(), crate::lourd_tests::memoire_pic());
    for kind in [EffectKind::Boil, EffectKind::GaussianBlur, EffectKind::Glow, EffectKind::DropShadow, EffectKind::Wave, EffectKind::Shake] {
        let e = Effect::new(kind);
        let mut values = e.values.clone();
        if kind == EffectKind::Boil {
            values[0] = 0.5;
            values.push(0.0);
        }
        if kind == EffectKind::Shake {
            values[0] = 20.0;
        }
        let steps = [FxStep { kind, values }];
        let mut sans = grand_debord();
        sans.outer.clear();
        let t = std::time::Instant::now();
        let _ = crate::fx::apply(&sans, &steps);
        let feuille = t.elapsed().as_millis();
        let t = std::time::Instant::now();
        let out = crate::fx::apply(&r, &steps);
        let (ms, pic) = (t.elapsed().as_millis(), crate::lourd_tests::memoire_pic());
        let t = std::time::Instant::now();
        let loin = crate::fx::apply(&perdu, &steps);
        println!(
            "MESURE {kind:?} : feuille seule {feuille} ms ; grand débord {ms} ms, pic du processus {pic:.0} Mo, {} tuiles hors feuille, {:.0} Mo dans le résultat ; point perdu à 30 000 : {} ms, {} tuiles hors feuille",
            out.outer.len(),
            out.octets_en_memoire() as f64 / 1e6,
            t.elapsed().as_millis(),
            loin.outer.len()
        );
    }
    println!("MESURE pic du processus à la fin : {:.0} Mo", crate::lourd_tests::memoire_pic());
}
