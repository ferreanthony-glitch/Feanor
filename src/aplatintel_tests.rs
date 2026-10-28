//! L'aplat intelligent (Anthony, 2026-10-28) : les régions du trait, leur correspondance d'une clé à l'autre, et ce que
//! l'application en fait (retouche d'une clé coloriée, clé qui suit une clé coloriée, commande en chaîne, enregistrement).
//! Dessins de synthèse : un visage rond (la peau) avec un œil fermé et une zone d'ombre fermée, coloriés.

use eframe::egui::{pos2, Color32, Pos2};

use crate::app::{App, Mode, Tool};
use crate::aplatregions::{self as reg, Entree};
use crate::document::{CelId, Document, Layer, LayerKind, Strate};
use crate::fond::{self, FondReglages};
use crate::montagekeys::KeyRef;
use crate::perf_tests::{banc, Banc};
use crate::raster::Raster;

const PEAU: Color32 = Color32::from_rgb(230, 180, 140);
const OEIL: Color32 = Color32::from_rgb(120, 170, 230);
const OMBRE: Color32 = Color32::from_rgb(150, 110, 90);
const ROSE: Color32 = Color32::from_rgb(240, 120, 150);
const W: usize = 400;
const H: usize = 300;

// ------------------------------------------------------------------------------------ dessins de synthèse

/// Un anneau noir de centre (`cx`, `cy`), de rayon `rayon` et d'épaisseur `ep`.
fn anneau(r: &mut Raster, cx: f32, cy: f32, rayon: f32, ep: f32) {
    let m = rayon + ep;
    for y in ((cy - m).max(0.0) as usize)..((cy + m) as usize + 1).min(r.height) {
        for x in ((cx - m).max(0.0) as usize)..((cx + m) as usize + 1).min(r.width) {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            if (d - rayon).abs() <= ep / 2.0 {
                let i = r.tile_index_of(x, y);
                r.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::BLACK;
            }
        }
    }
}

/// Un disque plein de la couleur `c` (peinture à la main).
fn disque(r: &mut Raster, cx: f32, cy: f32, rayon: f32, c: Color32) {
    for y in ((cy - rayon).max(0.0) as usize)..((cy + rayon) as usize + 1).min(r.height) {
        for x in ((cx - rayon).max(0.0) as usize)..((cx + rayon) as usize + 1).min(r.width) {
            if ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt() <= rayon {
                let i = r.tile_index_of(x, y);
                r.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = c;
            }
        }
    }
}

/// Un point de la scène (`cx`, `cy`, échelle `s`) à la position (`dx`, `dy`) du dessin d'origine (centre en 0, échelle 1).
fn pt(cx: f32, cy: f32, s: f32, dx: f32, dy: f32) -> (f32, f32) {
    (cx + dx * s, cy + dy * s)
}

const VISAGE: (f32, f32) = (-55.0, 5.0);
const OEIL_C: (f32, f32) = (-25.0, -15.0);
const OMBRE_C: (f32, f32) = (30.0, 35.0);
const JOUE: (f32, f32) = (-45.0, 40.0);

/// Le visage : un cercle de rayon 80 s, un œil fermé et une zone d'ombre fermée dedans.
fn scene(cx: f32, cy: f32, s: f32) -> Raster {
    let mut r = Raster::new(W, H);
    anneau(&mut r, cx, cy, 80.0 * s, 4.0);
    let (ox, oy) = pt(cx, cy, s, OEIL_C.0, OEIL_C.1);
    anneau(&mut r, ox, oy, 14.0 * s, 3.0);
    let (mx, my) = pt(cx, cy, s, OMBRE_C.0, OMBRE_C.1);
    anneau(&mut r, mx, my, 18.0 * s, 3.0);
    r
}

fn lire(r: &Raster, p: (f32, f32)) -> Color32 {
    r.get(p.0 as usize, p.1 as usize)
}

/// La peinture de l'aplat de `scene(cx, cy, s)` : peau, œil, ombre, par le pot de peinture de l'aplat (zones fermées), et un reflet
/// rose peint à la main sur la joue si `joue`.
fn peindre_scene(ligne: &Raster, cx: f32, cy: f32, s: f32, joue: bool) -> Raster {
    let mut p = Raster::new(W, H);
    for (c, (dx, dy)) in [(PEAU, VISAGE), (OEIL, OEIL_C), (OMBRE, OMBRE_C)] {
        let (x, y) = pt(cx, cy, s, dx, dy);
        let m = fond::zone(ligne, x as usize, y as usize, fond::TROUS).expect("zone fermée");
        fond::paint_mask(&mut p, &m, Some(c));
    }
    if joue {
        let (x, y) = pt(cx, cy, s, JOUE.0, JOUE.1);
        disque(&mut p, x, y, 6.0 * s, ROSE);
    }
    p
}

fn vide() -> Raster {
    Raster::new(W, H)
}

fn reglages() -> FondReglages {
    FondReglages { intelligent: true, ..FondReglages::default() }
}

fn entree<'a>(ligne: &'a Raster, peinture: &'a Raster, coupe: &'a Raster) -> Entree<'a> {
    Entree { trait_: ligne, peinture, coupe }
}

/// Le rang de la région qui contient le point `p` du dessin.
fn region_en(r: &reg::Regions, p: (f32, f32)) -> usize {
    let l = r.region_en(p.0 as i64, p.1 as i64);
    assert!(l > 0, "aucune région en {p:?}");
    l as usize - 1
}

// ------------------------------------------------------------------------------------ les régions

#[test]
fn a_round_face_with_a_closed_eye_and_a_shadow_has_three_regions_and_their_inclusion() {
    let ligne = scene(150.0, 150.0, 1.0);
    let r = reg::calculer(&ligne, &reglages()).expect("des régions");
    assert_eq!(r.regs.len(), 3, "la peau, l'œil, l'ombre : {:?}", r.regs.iter().map(|g| g.aire).collect::<Vec<_>>());
    let (peau, oeil, ombre) = (region_en(&r, pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1)), region_en(&r, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), region_en(&r, pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1)));
    assert!(peau != oeil && peau != ombre && oeil != ombre);
    assert_eq!(r.regs[peau].parent, None, "la peau touche le dehors par son contour : aucune région ne l'entoure");
    assert_eq!(r.regs[oeil].parent, Some(peau), "l'œil est dans la peau");
    assert_eq!(r.regs[ombre].parent, Some(peau), "l'ombre est dans la peau");
    assert!(r.regs[peau].aire > r.regs[ombre].aire && r.regs[ombre].aire > r.regs[oeil].aire);
    // Le dehors n'est pas une région.
    assert_eq!(r.region_en(5, 5), 0);
    // Un trait qui ne ferme rien (un arc) : aucune région.
    let mut arc = Raster::new(W, H);
    for x in 100..200 {
        disque(&mut arc, x as f32, 100.0 + ((x - 100) as f32 / 8.0).sin() * 10.0, 2.0, Color32::BLACK);
    }
    assert!(reg::calculer(&arc, &reglages()).is_none(), "un trait ouvert ne ferme aucune région");
    assert!(reg::calculer(&Raster::new(W, H), &reglages()).is_none(), "pas de trait, pas de région");
}

#[test]
fn the_colour_of_a_region_is_what_is_painted_in_it() {
    let ligne = scene(150.0, 150.0, 1.0);
    let peinture = peindre_scene(&ligne, 150.0, 150.0, 1.0, false);
    let r = reg::calculer(&ligne, &reglages()).unwrap();
    let c = r.couleurs(&peinture);
    assert_eq!(c[region_en(&r, pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1))], Some(PEAU));
    assert_eq!(c[region_en(&r, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1))], Some(OEIL));
    assert_eq!(c[region_en(&r, pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1))], Some(OMBRE));
    // Sans peinture : aucune couleur (celle du calque).
    assert!(r.couleurs(&vide()).iter().all(Option::is_none));
    // Un reflet peint à la main (une petite part de la peau) ne change pas la couleur de la région.
    let avec_joue = peindre_scene(&ligne, 150.0, 150.0, 1.0, true);
    assert_eq!(r.couleurs(&avec_joue)[region_en(&r, pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1))], Some(PEAU));
}

// ------------------------------------------------------------------------------------ la correspondance

#[test]
fn matching_finds_the_three_regions_of_a_moved_and_enlarged_drawing() {
    let (a, b) = (scene(150.0, 150.0, 1.0), scene(190.0, 170.0, 1.1));
    let (ra, rb) = (reg::calculer(&a, &reglages()).unwrap(), reg::calculer(&b, &reglages()).unwrap());
    let ap = reg::apparier(&ra, &rb, false);
    for (dx, dy) in [VISAGE, OEIL_C, OMBRE_C] {
        let (ia, ib) = (region_en(&ra, pt(150.0, 150.0, 1.0, dx, dy)), region_en(&rb, pt(190.0, 170.0, 1.1, dx, dy)));
        assert_eq!(ap.de_cible[ib], Some(ia), "la région en ({dx}, {dy}) se retrouve");
        assert_eq!(ap.de_source[ia], Some(ib));
    }
    assert_ne!(ap.recalage, reg::Recalage::IDENTITE, "le dessin a bougé : un recalage a été trouvé");
    // Le même dessin, une retouche : l'identité, aucun recalage.
    let ap = reg::apparier(&ra, &ra, true);
    assert_eq!(ap.recalage, reg::Recalage::IDENTITE);
    assert!(ap.de_cible.iter().all(Option::is_some));
}

#[test]
fn the_correspondence_is_deterministic() {
    let (a, b) = (scene(150.0, 150.0, 1.0), scene(190.0, 170.0, 1.1));
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, true);
    let (c, d) = (vide(), vide());
    let un = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&b, &d, &c), &reglages(), false).expect("une mise en couleur");
    let deux = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&b, &d, &c), &reglages(), false).expect("une mise en couleur");
    assert_eq!(un.0.to_flat(), deux.0.to_flat());
    assert_eq!(un.1.to_flat(), deux.1.to_flat());
    assert_eq!(un.2, deux.2);
}

// ------------------------------------------------------------------------------------ suivre une autre clé

#[test]
fn the_next_key_moved_and_enlarged_takes_the_same_colours_in_the_right_regions() {
    let (a, b) = (scene(150.0, 150.0, 1.0), scene(190.0, 170.0, 1.1));
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, true);
    let (c, d) = (vide(), vide());
    let (peinture, _coupe, bilan) = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&b, &d, &c), &reglages(), false).expect("une mise en couleur");
    assert_eq!((bilan.regions_cible, bilan.appariees, bilan.laissees), (3, 3, 0), "{bilan:?}");
    assert_eq!(lire(&peinture, pt(190.0, 170.0, 1.1, VISAGE.0, VISAGE.1)), PEAU);
    assert_eq!(lire(&peinture, pt(190.0, 170.0, 1.1, OEIL_C.0, OEIL_C.1)), OEIL, "le blanc de l'œil suit l'œil");
    assert_eq!(lire(&peinture, pt(190.0, 170.0, 1.1, OMBRE_C.0, OMBRE_C.1)), OMBRE, "la zone d'ombre suit l'ombre");
    // Ce que la main a peint dans la peau (le reflet) suit, à sa place recalée.
    assert_eq!(lire(&peinture, pt(190.0, 170.0, 1.1, JOUE.0, JOUE.1)), ROSE, "le détail peint suit le dessin");
    // Le dehors reste vide.
    assert_eq!(lire(&peinture, (20.0, 20.0)).a(), 0);
    assert_eq!(lire(&peinture, (380.0, 280.0)).a(), 0);
}

#[test]
fn a_new_region_inside_a_coloured_one_takes_its_colour_but_only_when_following_another_key() {
    // La clé suivante a, en plus, une petite zone fermée dans la peau (une narine) : sans correspondance.
    let a = scene(150.0, 150.0, 1.0);
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, false);
    let mut b = scene(150.0, 150.0, 1.0);
    let narine = pt(150.0, 150.0, 1.0, 30.0, -35.0);
    anneau(&mut b, narine.0, narine.1, 12.0, 3.0);
    let (c, d) = (vide(), vide());
    // En suivant une autre clé : la narine prend la peau (sa région englobante est appariée).
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&b, &d, &c), &reglages(), false).unwrap();
    assert_eq!((bilan.appariees, bilan.englobees, bilan.laissees), (3, 1, 0), "{bilan:?}");
    assert_eq!(lire(&peinture, narine), PEAU, "une zone neuve dans la peau : la peau");
    // Une zone neuve qui n'est dans rien (une boucle hors du visage) : laissée vide.
    let mut e = scene(150.0, 150.0, 1.0);
    anneau(&mut e, 330.0, 60.0, 20.0, 3.0);
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&e, &d, &c), &reglages(), false).unwrap();
    assert_eq!((bilan.appariees, bilan.englobees, bilan.laissees), (3, 0, 1), "{bilan:?}");
    assert_eq!(lire(&peinture, (330.0, 60.0)).a(), 0, "sans correspondance et sans région autour : vide");
}

#[test]
fn the_chain_of_four_keys_each_following_the_previous_keeps_every_colour() {
    let scenes = [(150.0, 150.0, 1.0), (165.0, 157.0, 1.05), (180.0, 164.0, 1.10), (195.0, 171.0, 1.15)];
    let lignes: Vec<Raster> = scenes.iter().map(|&(x, y, s)| scene(x, y, s)).collect();
    let coupe = vide();
    let mut peinture = peindre_scene(&lignes[0], scenes[0].0, scenes[0].1, scenes[0].2, true);
    for k in 1..4 {
        let vierge = vide();
        let (p, _, bilan) = reg::colorier_comme(&entree(&lignes[k - 1], &peinture, &coupe), &entree(&lignes[k], &vierge, &coupe), &reglages(), false).expect("la clé suit");
        assert_eq!((bilan.appariees, bilan.laissees), (3, 0), "clé {k} : {bilan:?}");
        let (x, y, s) = scenes[k];
        assert_eq!(lire(&p, pt(x, y, s, VISAGE.0, VISAGE.1)), PEAU, "clé {k} : la peau");
        assert_eq!(lire(&p, pt(x, y, s, OEIL_C.0, OEIL_C.1)), OEIL, "clé {k} : l'œil");
        assert_eq!(lire(&p, pt(x, y, s, OMBRE_C.0, OMBRE_C.1)), OMBRE, "clé {k} : l'ombre");
        assert_eq!(lire(&p, pt(x, y, s, JOUE.0, JOUE.1)), ROSE, "clé {k} : le reflet");
        peinture = p;
    }
}

// ------------------------------------------------------------------------------------ retoucher le trait d'une clé coloriée

#[test]
fn a_retouch_that_moves_the_outline_gives_the_gained_pixels_the_colour_of_their_region() {
    let a = scene(150.0, 150.0, 1.0);
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, false);
    // Le contour du visage est repris plus loin (rayon 100 au lieu de 80) : l'œil et l'ombre n'ont pas bougé.
    let mut b = Raster::new(W, H);
    anneau(&mut b, 150.0, 150.0, 100.0, 4.0);
    let (ox, oy) = pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1);
    anneau(&mut b, ox, oy, 14.0, 3.0);
    let (mx, my) = pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1);
    anneau(&mut b, mx, my, 18.0, 3.0);
    let coupe = vide();
    // Ce que l'aplat montrait sans l'aplat intelligent : la peinture n'a pas bougé, la bande gagnée est vide (couleur du calque).
    let gagne = (150.0 + 88.0, 150.0);
    assert_eq!(lire(&pa, gagne).a(), 0, "la bande gagnée n'a pas de peinture");
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &coupe), &entree(&b, &pa, &coupe), &reglages(), true).expect("retouche");
    assert_eq!((bilan.appariees, bilan.laissees), (3, 0), "{bilan:?}");
    assert_eq!(lire(&peinture, gagne), PEAU, "la bande gagnée a la peau");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), OEIL, "l'œil garde sa couleur");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1)), OMBRE);
}

#[test]
fn a_retouch_that_closes_a_new_small_region_keeps_the_colour_it_had() {
    let a = scene(150.0, 150.0, 1.0);
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, true);
    let mut b = scene(150.0, 150.0, 1.0);
    let narine = pt(150.0, 150.0, 1.0, 30.0, -35.0);
    anneau(&mut b, narine.0, narine.1, 12.0, 3.0);
    let coupe = vide();
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &coupe), &entree(&b, &pa, &coupe), &reglages(), true).unwrap();
    assert_eq!((bilan.appariees, bilan.englobees, bilan.laissees), (3, 0, 1), "la nouvelle zone n'a pas de correspondance : laissée comme elle est");
    assert_eq!(lire(&peinture, narine), PEAU, "elle a la couleur qu'avait cet endroit");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1)), PEAU);
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, JOUE.0, JOUE.1)), ROSE, "ce qu'on avait peint reste");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), OEIL);
}

#[test]
fn a_region_that_disappears_is_absorbed_by_the_region_that_takes_its_place() {
    // On efface l'anneau de l'œil : il n'y a plus qu'une région, la peau (qui prend la place de l'œil).
    let a = scene(150.0, 150.0, 1.0);
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, false);
    let mut b = Raster::new(W, H);
    anneau(&mut b, 150.0, 150.0, 80.0, 4.0);
    let (mx, my) = pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1);
    anneau(&mut b, mx, my, 18.0, 3.0);
    let coupe = vide();
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &coupe), &entree(&b, &pa, &coupe), &reglages(), true).unwrap();
    assert_eq!((bilan.regions_cible, bilan.appariees), (2, 2), "{bilan:?}");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), PEAU, "là où était l'œil : la peau");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1)), OMBRE);
}

#[test]
fn an_outline_opened_by_the_eraser_loses_its_region_but_breaks_nothing_else() {
    let a = scene(150.0, 150.0, 1.0);
    let pa = peindre_scene(&a, 150.0, 150.0, 1.0, false);
    // Une brèche de 40 pixels dans le contour du visage : la peau fuit vers le dehors, l'œil et l'ombre restent fermés.
    let mut b = scene(150.0, 150.0, 1.0);
    for y in 60..82 {
        for x in 130..170 {
            let i = b.tile_index_of(x, y);
            b.tile_mut(i)[(y % crate::raster::TILE) * crate::raster::TILE + x % crate::raster::TILE] = Color32::TRANSPARENT;
        }
    }
    let rb = reg::calculer(&b, &reglages()).unwrap();
    assert_eq!(rb.regs.len(), 2, "la peau n'est plus fermée");
    let coupe = vide();
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &coupe), &entree(&b, &pa, &coupe), &reglages(), true).unwrap();
    assert_eq!((bilan.regions_cible, bilan.appariees), (2, 2), "{bilan:?}");
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), OEIL);
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, OMBRE_C.0, OMBRE_C.1)), OMBRE);
    // La peinture de la peau n'est pas effacée : elle reste là pour quand le contour sera refermé.
    assert_eq!(lire(&peinture, pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1)), PEAU);
}

// ------------------------------------------------------------------------------------ le réglage

#[test]
fn the_setting_is_written_only_when_on_and_old_files_read_it_off() {
    let off = serde_json::to_string(&FondReglages::default()).unwrap();
    assert!(!off.contains("intelligent"), "éteint, rien n'est écrit : {off}");
    let on = serde_json::to_string(&reglages()).unwrap();
    assert!(on.contains("\"intelligent\":true"), "{on}");
    let back: FondReglages = serde_json::from_str(&on).unwrap();
    assert!(back.intelligent);
    let ancien: FondReglages = serde_json::from_str(r#"{"couleur":[250,240,200],"debord":4,"trous":7,"bord":false}"#).unwrap();
    assert!(!ancien.intelligent && ancien.couleur == [250, 240, 200]);
    assert!(crate::project::VERSION >= 41, "41 : l'aplat intelligent");
}

// ------------------------------------------------------------------------------------ dans l'application

struct Fix {
    app: App,
    panel: u64,
    layer: u64,
    cels: Vec<CelId>,
}

/// Un calque « A » (bitmap) dont les clés commencent aux images 0, 5, 10… : `scenes[k]` est le dessin de la clé k, `colorie[k]` :
/// son aplat est peint (peau, œil, ombre). La clé 0 est choisie, c'est l'outil pinceau (trait noir).
fn fix(intelligent: bool, scenes: &[(f32, f32, f32)], colorie: &[bool]) -> Fix {
    let mut doc = Document::with_frame(200, 150, 24);
    let (dw, dh) = (doc.width, doc.height);
    assert_eq!((dw, dh), (W, H));
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(5 * scenes.len());
    let mut layer = Layer::new("A", LayerKind::Bitmap);
    layer.fond = Some(FondReglages { intelligent, ..FondReglages::default() });
    let mut cels = Vec::new();
    for (k, &(cx, cy, s)) in scenes.iter().enumerate() {
        let cel = doc.new_cel(LayerKind::Bitmap);
        let c = doc.cels.get_mut(&cel).unwrap();
        c.raster = scene(cx, cy, s);
        if colorie[k] {
            let mut st = Strate::new(dw, dh);
            st.peinture.raster = peindre_scene(&c.raster, cx, cy, s, false);
            c.fond = Some(Box::new(st));
        }
        for f in 5 * k..5 * k + 5 {
            layer.set(f, Some(cel));
        }
        cels.push(cel);
    }
    let id = layer.id;
    doc.panels[0].layers.push(layer);
    let panel = doc.panels[0].id;
    let Banc { mut app, .. } = banc(doc, false);
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.tool = Tool::Brush;
    app.mode = Mode::Animation;
    app.color = [0, 0, 0];
    app.brush.size = 8.0;
    Fix { app, panel, layer: id, cels }
}

impl Fix {
    fn peinture(&self, k: usize) -> Raster {
        self.app.doc.cels[&self.cels[k]].fond.as_ref().map_or_else(vide, |s| s.peinture.raster.clone())
    }

    fn ligne(&self, k: usize) -> Raster {
        self.app.doc.cels[&self.cels[k]].raster.clone()
    }

    fn noms(&self) -> Vec<String> {
        self.app.history.names().0.iter().map(|n| n.to_string()).collect()
    }
}

/// Un trait de la main : du point `a` au point `b`, droit.
fn gliss(app: &mut App, a: Pos2, b: Pos2) {
    app.begin_paint(a, 1.0, false);
    for i in 1..=12 {
        app.stroke_to(a + (b - a) * (i as f32 / 12.0), 1.0);
    }
    app.end_stroke();
}

/// Un trait qui passe par ces points.
fn trait_par(app: &mut App, pts: &[Pos2]) {
    app.begin_paint(pts[0], 1.0, false);
    for w in pts.windows(2) {
        for i in 1..=8 {
            app.stroke_to(w[0] + (w[1] - w[0]) * (i as f32 / 8.0), 1.0);
        }
    }
    app.end_stroke();
}

fn p2(p: (f32, f32)) -> Pos2 {
    pos2(p.0, p.1)
}

#[test]
fn with_the_option_off_a_retouch_changes_nothing_in_the_aplat() {
    let mut f = fix(false, &[(150.0, 150.0, 1.0)], &[true]);
    let avant = f.peinture(0).to_flat();
    let n0 = f.noms().len();
    // Une narine fermée, tracée à la main dans la peau.
    let n = pt(150.0, 150.0, 1.0, 10.0, 10.0);
    trait_par(&mut f.app, &[pos2(n.0 - 12.0, n.1 - 12.0), pos2(n.0 + 12.0, n.1 - 12.0), pos2(n.0 + 12.0, n.1 + 12.0), pos2(n.0 - 12.0, n.1 + 12.0), pos2(n.0 - 12.0, n.1 - 16.0)]);
    f.app.attendre_aplat_multi();
    assert!(!f.app.intel_occupe(), "rien n'est parti");
    assert_eq!(f.peinture(0).to_flat(), avant, "la peinture de l'aplat n'a pas bougé d'un pixel");
    assert_eq!(f.noms().len(), n0 + 1, "une seule étape, celle du trait : {:?}", f.noms());
    assert!(!f.noms().iter().any(|n| n.contains("intelligent")));
}

#[test]
fn with_the_option_on_a_retouch_follows_in_the_same_undo_step_as_the_stroke() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0)], &[true]);
    let (ligne0, avant) = (f.ligne(0), f.peinture(0));
    let n0 = f.noms().len();
    // Une corde qui coupe le côté gauche du visage : une zone fermée de plus.
    trait_par(&mut f.app, &[pos2(100.0, 75.0), pos2(100.0, 225.0)]);
    f.app.attendre_aplat_multi();
    assert!(!f.app.intel_occupe());
    let rb = reg::calculer(&f.ligne(0), &reglages()).unwrap();
    assert_eq!(rb.regs.len(), 4, "le côté gauche est une zone de plus");
    assert_eq!(f.peinture(0).get(84, 150), PEAU, "la zone coupée garde la peau");
    assert_eq!(f.peinture(0).get(150 - 55, 155), PEAU);
    assert_eq!(f.peinture(0).get(150 - 25, 135), OEIL, "l'œil garde son blanc");
    // Une seule étape pour le trait et la couleur : un Ctrl+Z rend les deux.
    assert_eq!(f.noms().len(), n0 + 1, "{:?}", f.noms());
    f.app.undo();
    assert_eq!(f.ligne(0).to_flat(), ligne0.to_flat(), "le trait est revenu");
    assert_eq!(f.peinture(0).to_flat(), avant.to_flat(), "la peinture aussi");
}

#[test]
fn a_gap_made_by_the_eraser_and_closed_again_further_out_keeps_the_skin() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0)], &[true]);
    // 1. La gomme fait une brèche dans le haut du contour : la peau n'est plus fermée.
    f.app.set_tool(Tool::Eraser);
    f.app.eraser.size = 34.0;
    gliss(&mut f.app, pos2(150.0, 70.0), pos2(152.0, 70.0));
    f.app.attendre_aplat_multi();
    let ouvert = reg::calculer(&f.ligne(0), &reglages()).unwrap();
    assert_eq!(ouvert.regs.len(), 2, "la brèche a ouvert la peau");
    // 2. Le pinceau referme plus haut, avec un renflement.
    f.app.set_tool(Tool::Brush);
    trait_par(&mut f.app, &[pos2(130.0, 71.0), pos2(120.0, 45.0), pos2(150.0, 35.0), pos2(180.0, 45.0), pos2(170.0, 71.0)]);
    f.app.attendre_aplat_multi();
    let ferme = reg::calculer(&f.ligne(0), &reglages()).unwrap();
    assert_eq!(ferme.regs.len(), 3, "le contour est refermé");
    assert_eq!(f.peinture(0).get(150, 52), PEAU, "le renflement a la peau, retrouvée dans l'état d'avant la brèche");
    assert_eq!(f.peinture(0).get(125, 135), OEIL, "l'œil garde son blanc");
}

#[test]
fn a_new_key_without_colours_following_a_coloured_key_takes_its_colours_at_the_end_of_a_stroke() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0), (190.0, 170.0, 1.1)], &[true, false]);
    assert!(f.peinture(1).is_blank());
    f.app.cur_frame = 5;
    // Un petit trait fait dans la clé 2 (un pli sans fermer de zone).
    let (a, b) = (pt(190.0, 170.0, 1.1, -20.0, 50.0), pt(190.0, 170.0, 1.1, -5.0, 50.0));
    gliss(&mut f.app, p2(a), p2(b));
    f.app.attendre_aplat_multi();
    let p = f.peinture(1);
    assert_eq!(lire(&p, pt(190.0, 170.0, 1.1, VISAGE.0, VISAGE.1)), PEAU);
    assert_eq!(lire(&p, pt(190.0, 170.0, 1.1, OEIL_C.0, OEIL_C.1)), OEIL);
    assert_eq!(lire(&p, pt(190.0, 170.0, 1.1, OMBRE_C.0, OMBRE_C.1)), OMBRE);
    // La clé d'avant n'a pas bougé.
    assert_eq!(lire(&f.peinture(0), pt(150.0, 150.0, 1.0, OEIL_C.0, OEIL_C.1)), OEIL);
    // Un Ctrl+Z rend le trait et la couleur ensemble.
    f.app.undo();
    assert!(f.peinture(1).is_blank(), "la clé est revenue sans aplat peint");
}

#[test]
fn colour_like_the_previous_key_follows_in_a_chain_for_the_chosen_keys_in_one_undo_step() {
    let scenes = [(150.0, 150.0, 1.0), (165.0, 157.0, 1.05), (180.0, 164.0, 1.10), (195.0, 171.0, 1.15)];
    let mut f = fix(true, &scenes, &[true, false, false, false]);
    f.app.key_sel = [5usize, 10, 15].iter().map(|&s| KeyRef { panel: f.panel, layer: f.layer, start: s }).collect();
    let n0 = f.noms().len();
    f.app.run_action("aplat.colorier_comme_precedente");
    assert!(f.app.intel_occupe(), "le calcul part en arrière-plan : {}", f.app.status);
    f.app.attendre_aplat_multi();
    assert!(!f.app.intel_occupe());
    for k in 1..4 {
        let (x, y, s) = scenes[k];
        let p = f.peinture(k);
        assert_eq!(lire(&p, pt(x, y, s, VISAGE.0, VISAGE.1)), PEAU, "clé {k}");
        assert_eq!(lire(&p, pt(x, y, s, OEIL_C.0, OEIL_C.1)), OEIL, "clé {k}");
        assert_eq!(lire(&p, pt(x, y, s, OMBRE_C.0, OMBRE_C.1)), OMBRE, "clé {k}");
    }
    let noms = f.noms();
    assert_eq!(noms.len(), n0 + 1, "une seule étape : {noms:?}");
    assert!(noms.last().is_some_and(|n| n.contains("colorier comme la clé précédente")), "{noms:?}");
    assert!(f.app.status.contains("3 clés"), "{}", f.app.status);
    f.app.undo();
    for k in 1..4 {
        assert!(f.peinture(k).is_blank(), "clé {k} : tout s'annule d'un coup");
    }
    assert!(!f.peinture(0).is_blank(), "la clé coloriée n'a pas bougé");
}

#[test]
fn colour_like_the_previous_key_needs_the_option_and_a_key_before() {
    let mut f = fix(false, &[(150.0, 150.0, 1.0), (165.0, 157.0, 1.05)], &[true, false]);
    f.app.key_sel = vec![KeyRef { panel: f.panel, layer: f.layer, start: 5 }];
    f.app.run_action("aplat.colorier_comme_precedente");
    assert!(!f.app.intel_occupe() && f.app.status.contains("Aplat intelligent"), "option éteinte : {}", f.app.status);
    assert!(f.peinture(1).is_blank());
    // Option allumée, mais la clé choisie est la première : rien avant elle.
    f.app.layers_mut()[0].fond = Some(reglages());
    f.app.key_sel = vec![KeyRef { panel: f.panel, layer: f.layer, start: 0 }];
    f.app.run_action("aplat.colorier_comme_precedente");
    assert!(!f.app.intel_occupe(), "{}", f.app.status);
    assert!(f.app.status.contains("rien à suivre"), "{}", f.app.status);
}

#[test]
fn saving_right_after_a_stroke_waits_for_the_colours() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0), (190.0, 170.0, 1.1)], &[true, false]);
    f.app.cur_frame = 5;
    let (a, b) = (pt(190.0, 170.0, 1.1, -20.0, 50.0), pt(190.0, 170.0, 1.1, -5.0, 50.0));
    gliss(&mut f.app, p2(a), p2(b));
    assert!(f.app.intel_occupe() || !f.peinture(1).is_blank(), "le calcul est parti");
    let dossier = std::env::temp_dir().join(format!("shot-aplatintel-enregistrer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    assert!(f.app.save_to_blocking(dossier.join("essai.shot")), "{}", f.app.status);
    assert!(!f.app.intel_occupe(), "plus rien en route après l'enregistrement");
    assert_eq!(lire(&f.peinture(1), pt(190.0, 170.0, 1.1, OEIL_C.0, OEIL_C.1)), OEIL, "tout est posé avant l'écriture");
    let manifeste = std::fs::read_to_string(dossier.join("essai.shot")).unwrap();
    assert!(manifeste.contains("\"intelligent\": true"), "le réglage est dans le projet");
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn a_project_without_the_option_does_not_mention_it() {
    let mut f = fix(false, &[(150.0, 150.0, 1.0)], &[true]);
    let dossier = std::env::temp_dir().join(format!("shot-aplatintel-sans-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    assert!(f.app.save_to_blocking(dossier.join("essai.shot")), "{}", f.app.status);
    let manifeste = std::fs::read_to_string(dossier.join("essai.shot")).unwrap();
    assert!(!manifeste.contains("intelligent"), "le format n'a rien de plus sans l'option");
    assert!(manifeste.contains(&format!("\"version\": {}", crate::project::VERSION)));
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn a_stroke_in_progress_holds_the_result_until_it_ends() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0), (190.0, 170.0, 1.1)], &[true, false]);
    f.app.cur_frame = 5;
    let (a, b) = (pt(190.0, 170.0, 1.1, -20.0, 50.0), pt(190.0, 170.0, 1.1, -5.0, 50.0));
    gliss(&mut f.app, p2(a), p2(b));
    // Un deuxième trait commence avant l'arrivée du premier résultat : rien n'est posé pendant le geste.
    let c = pt(190.0, 170.0, 1.1, 10.0, 60.0);
    f.app.begin_paint(p2(c), 1.0, false);
    let debut = std::time::Instant::now();
    while f.app.aplat_intel_en_route() && debut.elapsed() < std::time::Duration::from_secs(20) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        f.app.poll_aplat_multi();
    }
    assert!(f.peinture(1).is_blank(), "pendant le geste, le résultat attend");
    f.app.stroke_to(pos2(c.0 + 10.0, c.1), 1.0);
    f.app.end_stroke();
    f.app.attendre_aplat_multi();
    assert_eq!(lire(&f.peinture(1), pt(190.0, 170.0, 1.1, OEIL_C.0, OEIL_C.1)), OEIL, "puis il est posé");
}

#[test]
fn painting_a_region_by_hand_makes_that_colour_the_reference_for_the_next_key() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0), (165.0, 157.0, 1.05)], &[true, false]);
    // L'aplat est la cible : on repeint la peau d'une autre couleur au pot de peinture. Rien ne part (c'est la main).
    f.app.select_aplat(0);
    f.app.set_tool(Tool::Fill);
    f.app.color = [240, 120, 150];
    let (x, y) = pt(150.0, 150.0, 1.0, VISAGE.0, VISAGE.1);
    f.app.fill_at(pos2(x, y));
    f.app.attendre_aplat_multi();
    assert!(!f.app.intel_occupe(), "repeindre à la main ne lance aucun calcul");
    assert_eq!(f.peinture(0).get(x as usize, y as usize), ROSE, "la peau est repeinte");
    // La clé suivante suit la peau repeinte, pas l'ancienne : c'est la nouvelle référence.
    f.app.aplat_cible = None;
    f.app.key_sel = vec![KeyRef { panel: f.panel, layer: f.layer, start: 5 }];
    f.app.run_action("aplat.colorier_comme_precedente");
    f.app.attendre_aplat_multi();
    let (a, b, s) = (165.0, 157.0, 1.05);
    assert_eq!(lire(&f.peinture(1), pt(a, b, s, VISAGE.0, VISAGE.1)), ROSE, "la peau repeinte est suivie");
    assert_eq!(lire(&f.peinture(1), pt(a, b, s, OEIL_C.0, OEIL_C.1)), OEIL, "l'œil garde son blanc");
}

#[test]
fn a_stroke_painted_into_the_aplat_by_hand_starts_no_calculation() {
    let mut f = fix(true, &[(150.0, 150.0, 1.0)], &[true]);
    f.app.select_aplat(0);
    f.app.color = [10, 200, 10];
    gliss(&mut f.app, pos2(100.0, 200.0), pos2(130.0, 200.0));
    f.app.attendre_aplat_multi();
    assert!(!f.app.intel_occupe());
    assert!(f.noms().iter().any(|n| n == "Aplat : peinture"), "{:?}", f.noms());
}

#[test]
fn on_a_vector_layer_a_new_key_follows_a_coloured_key() {
    // Un calque vectoriel : un anneau tracé au pinceau, colorié au pot de peinture de l'aplat ; puis, à une autre image, un autre
    // anneau, plus grand et plus loin : à la fin du trait, il prend la couleur.
    let mut doc = Document::with_frame(200, 150, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(10);
    let mut layer = Layer::new("V", LayerKind::Vector);
    layer.fond = Some(reglages());
    let id = layer.id;
    doc.panels[0].layers.push(layer);
    let Banc { mut app, .. } = banc(doc, false);
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app.tool = Tool::Brush;
    app.mode = Mode::Animation;
    app.color = [0, 0, 0];
    app.brush.size = 8.0;
    let rond = |cx: f32, cy: f32, r: f32| -> Vec<Pos2> { (0..=52).map(|i| { let a = i as f32 / 48.0 * std::f32::consts::TAU; pos2(cx + r * a.cos(), cy + r * a.sin()) }).collect() };
    trait_par(&mut app, &rond(150.0, 150.0, 70.0));
    app.attendre_aplat_multi();
    let k0 = app.layers()[0].frames[0].expect("le dessin de la clé 1");
    assert!(app.doc.cels[&k0].vector.is_some(), "un dessin vectoriel");
    // Colorier la zone fermée de l'aplat de cette clé.
    app.aplat_cible = Some(id);
    app.set_tool(Tool::Fill);
    app.color = [230, 180, 140];
    app.fill_at(pos2(150.0, 150.0));
    app.aplat_cible = None;
    assert!(app.doc.cels[&k0].fond.as_ref().is_some_and(|s| !s.peinture.raster.is_blank()), "{}", app.status);
    // L'autre clé : un anneau plus grand, ailleurs.
    app.set_tool(Tool::Brush);
    app.color = [0, 0, 0];
    app.cur_frame = 5;
    trait_par(&mut app, &rond(210.0, 170.0, 85.0));
    app.attendre_aplat_multi();
    let k1 = app.layers()[0].frames[5].expect("le dessin de la clé 2");
    assert_ne!(k0, k1);
    let p = app.doc.cels[&k1].fond.as_ref().map(|s| s.peinture.raster.clone()).expect("l'aplat de la clé 2 est peint");
    assert_eq!(p.get(210, 170), PEAU, "l'anneau vectoriel suit la clé coloriée");
}
#[test]
fn a_full_hd_drawing_is_matched_and_coloured_in_a_fraction_of_a_second() {
    // Un visage de plan poitrine en 1920 × 1080 : la peau, un œil, une ombre, puis la même chose déplacée et agrandie.
    let grand = |cx: f32, cy: f32, s: f32| {
        let mut r = Raster::new(1920, 1080);
        anneau(&mut r, cx, cy, 380.0 * s, 6.0);
        anneau(&mut r, cx - 120.0 * s, cy - 70.0 * s, 60.0 * s, 5.0);
        anneau(&mut r, cx + 140.0 * s, cy + 160.0 * s, 80.0 * s, 5.0);
        r
    };
    let (a, b) = (grand(900.0, 520.0, 1.0), grand(1000.0, 560.0, 1.1));
    let mut pa = Raster::new(1920, 1080);
    for (c, p) in [(PEAU, (900.0 - 300.0, 520.0 + 20.0)), (OEIL, (900.0 - 120.0, 520.0 - 70.0)), (OMBRE, (900.0 + 140.0, 520.0 + 160.0))] {
        let m = fond::zone(&a, p.0 as usize, p.1 as usize, fond::TROUS).expect("zone fermée");
        fond::paint_mask(&mut pa, &m, Some(c));
    }
    let (c, d) = (Raster::new(1920, 1080), Raster::new(1920, 1080));
    let debut = std::time::Instant::now();
    let (peinture, _, bilan) = reg::colorier_comme(&entree(&a, &pa, &c), &entree(&b, &d, &c), &reglages(), false).expect("une mise en couleur");
    let duree = debut.elapsed();
    eprintln!("aplat intelligent, 1920 × 1080, 3 zones : {duree:?}");
    assert_eq!((bilan.appariees, bilan.laissees), (3, 0), "{bilan:?}");
    assert_eq!(peinture.get(1000 - 120 * 11 / 10, 560 - 70 * 11 / 10), OEIL);
    assert!(duree < std::time::Duration::from_secs(10), "{duree:?}");
}