//! Le trait tracé à travers les cases de la planche, **vu à l'écran** (partie 111) : ces essais peignent vraiment la planche sur la carte
//! graphique, lisent ses pixels après le lâcher et le repos, et suivent le trait pixel par pixel. Les essais du canevas libre (partie 101)
//! n'avaient vérifié que les données : le trait était bien réparti, et il avait des trous à l'écran.

use eframe::egui::{pos2, vec2, Pos2};

use crate::app::Tool;
use crate::document::{Document, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::vignettes_tests::glisser;

const ROUGE: [u8; 3] = [200, 30, 40];
/// Au-dessus de cette différence rouge moins vert, le pixel est de l'encre (le gris des filets et des barres n'en est pas).
const ENCRE: i32 = 40;

pub(crate) fn des_plans(n: usize, bitmap: bool) -> Document {
    let mut doc = Document::with_frame(1920, 1080, 24);
    let mut p = 0;
    for _ in 1..n {
        p = doc.add_scene_after(p);
    }
    doc.into_plans();
    assert_eq!(doc.panels.len(), n);
    if bitmap {
        for p in &mut doc.panels {
            for l in &mut p.layers {
                l.kind = LayerKind::Bitmap;
            }
        }
    }
    doc
}

/// La planche sur la carte graphique, `colonnes` cases par rangée, au zoom d'ajustement fois `zoom`.
pub(crate) fn planche(doc: Document, colonnes: usize, zoom: f32) -> Option<Banc> {
    let mut b = banc_taille(doc, true, 1920, 1080);
    b.capturer()?;
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.layout.board_columns = colonnes;
    b.app.layout.board_rows = 0;
    b.app.layout.card_size = 170.0;
    b.app.layout.board_sens = crate::boardsens::Sens::Lignes;
    // (Sans texte sous les cases, comme sur la capture du réalisateur.)
    b.app.layout.board_textes = crate::boardsens::Textes { action: false, dialogue: false, notes: false };
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = false;
    b.chauffer(6);
    b.app.board_zoom *= zoom;
    b.app.settings.vignettes.numeros_visibles = false;
    b.chauffer(6);
    b.app.set_tool(Tool::Brush);
    b.app.color = ROUGE;
    b.app.brush.size = 14.0;
    b.app.brush.opacity = 1.0;
    Some(b)
}

/// Jusqu'au repos : plus aucun rendu en fond depuis dix images. (Patient : les autres essais occupent les fils de fond.) Le réveil lui-même a son
/// essai, `thirty_drawn_plans…` : `fx::draft_level` est commun à tout le processus, les autres essais qui tournent en même temps le font réagir.
pub(crate) fn repos(b: &mut Banc) {
    let debut = std::time::Instant::now();
    let mut calmes = 0;
    while calmes < 10 {
        b.image(Vec::new(), 0.5);
        let rendu = b.app.vignettes_en_cours() || b.app.rejeu.has_pending() || b.app.vignettes.debord_tenu.is_some();
        calmes = if rendu { 0 } else { calmes + 1 };
        if rendu {
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        assert!(debut.elapsed() < std::time::Duration::from_secs(180), "les rendus en fond finissent après le lâcher{}", b.raison_du_reveil());
    }
    b.chauffer(2);
}

/// Un trait droit à la souris.
pub(crate) fn tracer(b: &mut Banc, de: Pos2, a: Pos2) {
    let n = ((a - de).length() / 6.0).ceil().max(2.0) as usize;
    let chemin: Vec<Pos2> = (0..=n).map(|k| de + (a - de) * (k as f32 / n as f32)).collect();
    glisser(b, &chemin);
}

pub(crate) fn px(rgba: &[u8], x: i32, y: i32) -> [i32; 3] {
    if !(0..1920).contains(&x) || !(0..1080).contains(&y) {
        return [255, 255, 255];
    }
    let k = (y as usize * 1920 + x as usize) * 4;
    [rgba[k] as i32, rgba[k + 1] as i32, rgba[k + 2] as i32]
}

/// Ce qu'on voit en suivant le trait de `de` à `a`, un pixel après l'autre (le maximum d'encre en travers, à 5 pixels près).
#[derive(Debug)]
pub(crate) struct Releve {
    /// Le plus long trou (pixels) et où il commence.
    pub trou: usize,
    pub ou: Pos2,
    pub longueur: usize,

    /// Largeur en pixels d'encre, en travers : la plus basse et la plus haute (mêmes 3 %).
    pub largeur: (usize, usize),
    /// L'encre en travers du trait, en lumière linéaire, en centièmes de pixel d'encre pleine (sa couleur fois son épaisseur : l'anticrénelage
    /// ne la change pas, ni la place du trait sur les pixels), moyennée sur 9 pixels le long du trait : la plus basse, la plus haute, hors 3 %
    /// des valeurs de chaque bout (les filets passent dessus).
    pub poids: (i32, i32),
}

pub(crate) fn relever(rgba: &[u8], de: Pos2, a: Pos2) -> Releve {
    relever_a(rgba, de, a, ENCRE)
}

pub(crate) fn relever_a(rgba: &[u8], de: Pos2, a: Pos2, seuil: i32) -> Releve {
    let d = (a - de).normalized();
    let n = vec2(-d.y, d.x);
    let longueur = (a - de).length() as usize;
    // (Les bouts du trait s'amincissent : on les laisse.)
    let (mut trou, mut courant, mut ou, mut debut) = (0, 0, de, de);
    let mut largeurs = Vec::new();
    let mut poids = Vec::new();
    for k in 8..longueur.saturating_sub(8) {
        let c = de + d * k as f32;
        let (mut haut, mut large, mut tout) = (0, 0, 0);
        for w in -6..=6 {
            let q = c + n * w as f32;
            let v = px(rgba, q.x.round() as i32, q.y.round() as i32);
            let e = v[0] - v[1];
            haut = haut.max(e);
            // (Le vert va du blanc à l'encre : 30 sur 255, presque noir en lumière linéaire. Les gris des filets et des barres ne comptent pas.)
            if e > 3 {
                tout += ((1.0 - eframe::egui::ecolor::linear_f32_from_gamma_u8(v[1].clamp(0, 255) as u8)) * 100.0) as i32;
            }
            if e >= seuil {
                large += 1;
            }
        }
        // (Un filet de case, peint par-dessus le trait, n'est pas un trou : le trait y passe dessous.)
        let sur_un_filet = (-1..=1).any(|w| {
            let q = c + n * w as f32;
            let v = px(rgba, q.x.round() as i32, q.y.round() as i32);
            (v[0] - v[1]).abs() < 12 && (v[1] - v[2]).abs() < 12 && v[0] < 225
        });
        if sur_un_filet && haut < seuil {
            continue;
        }
        if haut >= seuil {
            largeurs.push(large);
            poids.push(tout);
            courant = 0;
        } else {
            if courant == 0 {
                debut = c;
            }
            courant += 1;
            if courant > trou {
                trou = courant;
                ou = debut;
            }
        }
    }
    largeurs.sort_unstable();
    let mut poids: Vec<i32> = poids.windows(9.min(poids.len().max(1))).map(|w| w.iter().sum::<i32>() / w.len() as i32).collect();
    poids.sort_unstable();
    let coupe = poids.len() * 3 / 100;
    let bout = |v: &[i32]| if v.is_empty() { (0, 0) } else { (v[coupe.min(v.len() - 1)], v[v.len() - 1 - coupe.min(v.len() - 1)]) };
    let l = if largeurs.is_empty() { (0, 0) } else { (largeurs[coupe.min(largeurs.len() - 1)], largeurs[largeurs.len() - 1 - coupe.min(largeurs.len() - 1)]) };
    Releve { trou, ou, longueur, largeur: l, poids: bout(&poids) }
}

pub(crate) fn sauver(rgba: &[u8], nom: &str) {
    if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
        let d = std::path::PathBuf::from(d);
        let _ = std::fs::create_dir_all(&d);
        let _ = image::save_buffer(d.join(format!("{nom}.png")), rgba, 1920, 1080, image::ColorType::Rgba8);
    }
}

/// Le trait de `de` à `a` est continu, de couleur et d'épaisseur stables.
pub(crate) fn continu(rgba: &[u8], de: Pos2, a: Pos2, quoi: &str) {
    let r = relever(rgba, de, a);
    println!("TRAIT {quoi} : {r:?}");
    assert!(r.trou <= 2, "{quoi} : un trou de {} pixels à {:?} (trait de {} pixels) : {r:?}", r.trou, r.ou, r.longueur);
    assert!(r.poids.0 * 100 >= r.poids.1 * 60, "{quoi} : la couleur ou l'épaisseur change le long du trait : {r:?}");
    assert!(r.largeur.1 <= r.largeur.0 + 3, "{quoi} : l'épaisseur change le long du trait : {r:?}");
}

fn zooms() -> [f32; 2] {
    [1.0, 1.4]
}

#[test]
fn a_horizontal_stroke_across_three_cases_stays_continuous_at_rest() {
    for bitmap in [false, true] {
        for zoom in zooms() {
            let Some(mut b) = planche(des_plans(18, bitmap), 6, zoom) else { return println!("pas de carte graphique") };
            let l = b.app.board_layout();
            let xf = b.app.vignettes.xf;
            let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[3].thumb));
            let y = v0.top() + v0.height() * 0.2;
            let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
            tracer(&mut b, de, a);
            repos(&mut b);
            let rgba = b.capturer().unwrap();
            let nom = format!("rangee-{}-{zoom}", if bitmap { "bitmap" } else { "vecteur" });
            sauver(&rgba, &nom);
            continu(&rgba, de, a, &nom);
        }
    }
}

#[test]
fn a_diagonal_stroke_across_three_cases_stays_continuous_at_rest() {
    for bitmap in [false, true] {
        for zoom in zooms() {
            let Some(mut b) = planche(des_plans(18, bitmap), 6, zoom) else { return println!("pas de carte graphique") };
            let l = b.app.board_layout();
            let xf = b.app.vignettes.xf;
            let (v0, v2) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[14].thumb));
            let (de, a) = (v0.center() + vec2(10.0, 11.0), v2.center() - vec2(13.0, 7.0));
            tracer(&mut b, de, a);
            repos(&mut b);
            let rgba = b.capturer().unwrap();
            let nom = format!("diagonale-{}-{zoom}", if bitmap { "bitmap" } else { "vecteur" });
            sauver(&rgba, &nom);
            continu(&rgba, de, a, &nom);
        }
    }
}

#[test]
fn a_vertical_stroke_down_a_column_and_a_stroke_on_the_japanese_board_stay_continuous_at_rest() {
    for bitmap in [false, true] {
        for zoom in zooms() {
            let Some(mut b) = planche(des_plans(18, bitmap), 6, zoom) else { return println!("pas de carte graphique") };
            let l = b.app.board_layout();
            let xf = b.app.vignettes.xf;
            let (v0, v2) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[13].thumb));
            let x = v0.left() + v0.width() * 0.3;
            let (de, a) = (pos2(x, v0.center().y), pos2(x, v2.center().y));
            tracer(&mut b, de, a);
            repos(&mut b);
            let rgba = b.capturer().unwrap();
            let nom = format!("colonne-{}-{zoom}", if bitmap { "bitmap" } else { "vecteur" });
            sauver(&rgba, &nom);
            continu(&rgba, de, a, &nom);
        }
        // À la japonaise : les cases se suivent de haut en bas.
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        b.app.layout.board_sens = crate::boardsens::Sens::Japonais;
        b.app.board_zoom = 0.0;
        b.chauffer(8);
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v2) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[2].thumb));
        let x = v0.left() + v0.width() * 0.3;
        let (de, a) = (pos2(x, v0.center().y), pos2(x, v2.center().y));
        tracer(&mut b, de, a);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        let nom = format!("japonais-{}", if bitmap { "bitmap" } else { "vecteur" });
        sauver(&rgba, &nom);
        continu(&rgba, de, a, &nom);
    }
}

#[test]
fn a_stroke_from_the_last_plan_into_empty_cases_stays_continuous_at_rest_and_makes_new_plans() {
    for bitmap in [false, true] {
        for zoom in zooms() {
            let Some(mut b) = planche(des_plans(2, bitmap), 6, zoom) else { return println!("pas de carte graphique") };
            let l = b.app.board_layout();
            let xf = b.app.vignettes.xf;
            let v = xf.rect(l.cards[1].thumb);
            let w = v.width() + (xf.rect(l.cards[1].thumb).left() - xf.rect(l.cards[0].thumb).left() - v.width());
            let y = v.top() + v.height() * 0.2;
            let (de, a) = (pos2(v.center().x, y), pos2(v.center().x + w * 2.0, y));
            tracer(&mut b, de, a);
            repos(&mut b);
            if !bitmap {
                assert!(b.app.doc.panels.len() >= 3, "la case vide touchée fait son plan");
            }
            let rgba = b.capturer().unwrap();
            let nom = format!("vide-{}-{zoom}", if bitmap { "bitmap" } else { "vecteur" });
            sauver(&rgba, &nom);
            continu(&rgba, de, a, &nom);
        }
    }
}

/// Encre d'un plan : les tronçons de son dessin, en points de la planche.
fn points_du_plan(b: &Banc, i: usize) -> Vec<Pos2> {
    let l = b.app.board_layout();
    let Some(k) = l.cards.iter().position(|c| c.panel == i) else { return Vec::new() };
    let aff = crate::horschamp::aff_de_la_case(&b.app.doc, &l, k).expect("caméra droite");
    b.app.doc.panels[i]
        .layers
        .iter()
        .flat_map(|l| l.frames.iter().flatten())
        .filter_map(|id| b.app.doc.cels.get(id).and_then(|c| c.vector.as_ref()))
        .flat_map(|v| v.elements.iter())
        .flat_map(|e| match e {
            crate::vector::Element::Stroke { points, .. } => points.iter().map(|p| aff.applique(p.pos())).collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn a_stroke_going_down_into_the_top_of_the_next_row_enters_that_plan_and_shows_inside_its_frame() {
    // Le débord d'origine et de grands débords (jusqu'à 40 % : la marge de la case du dessus couvre bien plus que l'écart des rangées).
    for bitmap in [false, true] {
        for debord in [12u8, 25, 40] {
            let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
            b.app.settings.vignettes.debord = debord;
            // De grandes cases vues de loin, comme sur la capture du réalisateur : l'écart des rangées est fixe, la marge suit la case et le dépasse.
            b.app.layout.card_size = 800.0;
            b.app.board_zoom = 0.0;
            b.chauffer(8);
            b.attendre_vignettes();
            let l = b.app.board_layout();
            let (ecart, marge) = (l.cards[6].thumb.top() - l.cards[0].thumb.bottom(), l.cards[0].thumb.width() * f32::from(debord) / 100.0);
            assert!(marge > ecart, "la marge ({marge}) dépasse l'écart des rangées ({ecart})");
            let l = b.app.board_layout();
            let xf = b.app.vignettes.xf;
            let (haut, bas) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[7].thumb));
            let x = haut.left() + haut.width() * 0.4;
            // Le trait part de la case du haut et s'arrête au tiers de celle du dessous.
            let (de, a) = (pos2(x, haut.center().y), pos2(x, bas.top() + bas.height() * 0.35));
            tracer(&mut b, de, a);
            repos(&mut b);
            let rgba = b.capturer().unwrap();
            let nom = format!("descend-{}-{debord}", if bitmap { "bitmap" } else { "vecteur" });
            sauver(&rgba, &nom);
            continu(&rgba, de, a, &nom);
            if !bitmap {
                let dedans = points_du_plan(&b, 7);
                let bord = bas.top();
                assert!(dedans.iter().any(|p| b.app.vignettes.xf.to_screen(*p).y > bord + bas.height() * 0.25), "{nom} : la part est dans le plan du dessous : {dedans:?}");
                let haut_b = l.cards[7].thumb;
                let origine = points_du_plan(&b, 1);
                assert!(origine.iter().all(|p| !haut_b.shrink(0.5).contains(*p)), "{nom} : rien de la case du dessous n'est resté au plan du dessus");
            } else {
                assert!(crate::vignettes_tests::encre(&b.app.doc, 7), "{nom} : la part est dans le plan du dessous");
            }
        }
    }
}

#[test]
fn a_stroke_into_a_plan_with_a_camera_move_enters_its_sheet_and_stays_continuous() {
    let Some(mut b) = planche(crate::vignettes_camera_tests::plan_court(), 6, 1.0) else { return println!("pas de carte graphique") };
    crate::vignettes_camera_tests::deux_cadres(&mut b.app.doc, 1, 160.0, 0.0);
    b.chauffer(4);
    b.attendre_vignettes();
    let k = crate::vignettes_camera_tests::K;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (avant, m) = (xf.rect(l.cards[k - 1].thumb), xf.rect(l.move_of(k).expect("mouvement").tuile.rect));
    let y = m.top() + m.height() * 0.3;
    let (de, a) = (pos2(avant.center().x, y), pos2(m.center().x, y));
    let n = crate::vignettes_tests::pas_d_annulation(&b);
    tracer(&mut b, de, a);
    repos(&mut b);
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "mouvement");
    continu(&rgba, de, a, "mouvement");
    assert!(crate::vignettes_tests::encre(&b.app.doc, 1), "la part dans l'enveloppe est dans le plan à mouvement");
    assert_eq!(crate::vignettes_tests::pas_d_annulation(&b), n + 1);
}

#[test]
fn every_screen_image_after_the_release_shows_the_same_stroke_and_the_ui_thread_replays_nothing() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[3].thumb));
        let y = v0.top() + v0.height() * 0.2;
        let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
        let rejeux = crate::vector::replay_count();
        let n = ((a - de).length() / 6.0).ceil() as usize;
        let chemin: Vec<Pos2> = (0..=n).map(|k| de + (a - de) * (k as f32 / n as f32)).collect();
        // Le geste, sans le lâcher : l'image qu'on voit la main en bas.
        b.image(vec![eframe::egui::Event::PointerMoved(chemin[0])], 1.0 / 60.0);
        b.image(vec![crate::vignettes_tests::appui(chemin[0], true, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
        for q in &chemin[1..] {
            b.image(vec![eframe::egui::Event::PointerMoved(*q)], 1.0 / 120.0);
        }
        let geste = b.capturer().unwrap();
        sauver(&geste, &format!("geste-{bitmap}"));
        let en_geste = relever(&geste, de, a);
        println!("TRAIT geste bitmap {bitmap} : {en_geste:?}");
        assert!(en_geste.trou <= 2, "pendant le geste : {en_geste:?}");
        b.image(vec![crate::vignettes_tests::appui(*chemin.last().unwrap(), false, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
        // Chaque image d'écran depuis le lâcher, jusqu'au repos : le trait reste entier.
        let mut pire = 0;
        for k in 0..240 {
            b.image(Vec::new(), 1.0 / 60.0);
            let rgba = b.capturer().unwrap();
            let r = relever(&rgba, de, a);
            if r.trou > pire {
                pire = r.trou;
                sauver(&rgba, &format!("apres-lacher-{bitmap}-{k}"));
            }
            assert!(r.trou <= 2, "bitmap {bitmap} : image {k} après le lâcher : {r:?}");
            std::thread::sleep(std::time::Duration::from_millis(2));
            if k > 30 && !b.app.vignettes_en_cours() && !b.app.rejeu.has_pending() {
                break;
            }
        }
        assert_eq!(crate::vector::replay_count(), rejeux, "bitmap {bitmap} : aucun rejeu du moteur sur le fil de l'interface, ni au geste, ni au lâcher");
        repos(&mut b);
        let repos_img = b.capturer().unwrap();
        let r = relever(&repos_img, de, a);
        println!("TRAIT repos bitmap {bitmap} : {r:?}");
        assert!(r.trou <= 2, "{r:?}");
    }
}
#[test]
fn a_stroke_that_starts_on_the_paper_or_between_two_rows_stays_continuous_at_rest() {
    for bitmap in [false, true] {
        // Au-dessus de la planche, à travers la première rangée, jusqu'à la seconde (la capture : des traits partis du haut de la planche).
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v1) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[8].thumb));
        let (de, a) = (pos2(v0.left() + 6.0, v0.top() - 26.0), v1.center() - vec2(5.0, 3.0));
        tracer(&mut b, de, a);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        let nom = format!("papier-{}", if bitmap { "bitmap" } else { "vecteur" });
        sauver(&rgba, &nom);
        continu(&rgba, de, a, &nom);
        // Dans l'écart entre deux rangées, jusque dans une case plus loin.
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return };
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (u, v) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[9].thumb));
        let (de, a) = (pos2(u.center().x + 11.0, u.bottom() + 12.0), v.center() - vec2(8.0, 6.0));
        tracer(&mut b, de, a);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        let nom = format!("ecart-{}", if bitmap { "bitmap" } else { "vecteur" });
        sauver(&rgba, &nom);
        continu(&rgba, de, a, &nom);
    }
}

// ------------------------------------------------------------------------------ couleur, pression, bouts de tronçons

#[test]
fn the_ends_of_the_pieces_keep_the_colour_and_thickness_with_a_soft_brush_a_translucent_one_and_a_light_pen() {
    for (nom, taille, opacite, durete, stylet) in [("large-doux", 60.0, 1.0, 0.3, None), ("translucide", 40.0, 0.55, 1.0, None), ("stylet-leger", 40.0, 1.0, 0.8, Some(0.35_f32))] {
        let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
        b.app.brush.size = taille;
        b.app.brush.opacity = opacite;
        b.app.brush.hardness = durete;
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[3].thumb));
        let y = v0.top() + v0.height() * 0.2;
        let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
        if let Some(f) = stylet {
            let n = ((a - de).length() / 6.0).ceil() as usize;
            let toucher = |pos: Pos2, phase| eframe::egui::Event::Touch { device_id: eframe::egui::TouchDeviceId(1), id: eframe::egui::TouchId(1), phase, pos, force: Some(f) };
            b.image(vec![eframe::egui::Event::PointerMoved(de)], 1.0 / 60.0);
            b.image(vec![toucher(de, eframe::egui::TouchPhase::Start), crate::vignettes_tests::appui(de, true, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
            for k in 1..=n {
                let q = de + (a - de) * (k as f32 / n as f32);
                b.image(vec![toucher(q, eframe::egui::TouchPhase::Move), eframe::egui::Event::PointerMoved(q)], 1.0 / 120.0);
            }
            b.image(vec![toucher(a, eframe::egui::TouchPhase::End), crate::vignettes_tests::appui(a, false, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
        } else {
            tracer(&mut b, de, a);
        }
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("brosse-{nom}"));
        continu(&rgba, de, a, nom);
    }
}

/// L'encre totale du trait dans le cadre de la case `k` : de quoi comparer la case vive aux autres.
fn poids_dans(rgba: &[u8], r: eframe::egui::Rect) -> i64 {
    let mut t = 0_i64;
    for y in (r.top() as i32 + 2)..(r.bottom() as i32 - 2) {
        for x in (r.left() as i32 + 2)..(r.right() as i32 - 2) {
            let v = px(rgba, x, y);
            if v[0] - v[1] > 3 {
                t += ((1.0 - eframe::egui::ecolor::linear_f32_from_gamma_u8(v[1].clamp(0, 255) as u8)) * 100.0) as i64;
            }
        }
    }
    t
}

#[test]
fn the_same_stroke_is_as_dark_in_the_case_being_drawn_as_in_the_others() {
    // La capture du réalisateur : un « M » pâle dans la dernière case dessinée, plus foncé dès qu'il est dans une autre. Un trait fin.
    for taille in [4.0_f32, 8.0, 14.0] {
        let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
        b.app.brush.size = taille;
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let v = xf.rect(l.cards[2].thumb);
        tracer(&mut b, v.left_center() + vec2(14.0, -9.0), v.right_center() + vec2(-14.0, 7.0));
        repos(&mut b);
        assert_eq!(b.app.cur_panel, 2, "la case dessinée est vive");
        let vive = poids_dans(&b.capturer().unwrap(), v);
        b.app.select_panel(0);
        b.chauffer(4);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("pale-{taille}"));
        let autre = poids_dans(&rgba, v);
        println!("TRAIT taille {taille} : encre {vive} dans la case vive, {autre} en vignette");
        assert!(vive > 0 && autre > 0);
        let rapport = autre as f32 / vive as f32;
        assert!((0.8..1.25).contains(&rapport), "taille {taille} : même trait, {vive} vif, {autre} en vignette");
    }
}

#[test]
fn a_thin_stroke_does_not_change_at_the_release() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.brush.size = 5.0;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[3].thumb));
    let y = v0.top() + v0.height() * 0.2;
    let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
    let n = ((a - de).length() / 6.0).ceil() as usize;
    b.image(vec![eframe::egui::Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![crate::vignettes_tests::appui(de, true, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=n {
        b.image(vec![eframe::egui::Event::PointerMoved(de + (a - de) * (k as f32 / n as f32))], 1.0 / 120.0);
    }
    let geste = relever_a(&b.capturer().unwrap(), de, a, 12);
    b.image(vec![crate::vignettes_tests::appui(a, false, eframe::egui::Modifiers::NONE)], 1.0 / 60.0);
    repos(&mut b);
    let fini = relever_a(&b.capturer().unwrap(), de, a, 12);
    println!("TRAIT fin : geste {geste:?} ; repos {fini:?}");
    assert!(geste.trou <= 2 && fini.trou <= 2);
    assert!((fini.poids.1 as f32) < geste.poids.1 as f32 * 1.4 && (fini.poids.1 as f32) > geste.poids.1 as f32 * 0.6, "la couleur ne change pas au lâcher : {geste:?} {fini:?}");
}

// ------------------------------------------------------------------------------ la règle de la marge

/// Un trait dans la marge seule de la case 1 : il part du cadre, sort à droite et s'arrête dans la marge.
fn trait_dans_la_marge(b: &mut Banc) -> (Pos2, Pos2) {
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let v = xf.rect(l.cards[1].thumb);
    let y = v.top() + v.height() * 0.2;
    let (de, a) = (pos2(v.right() - 12.0, y), pos2(v.right() + 14.0, y));
    tracer(b, de, a);
    repos(b);
    (pos2(v.right() + 3.0, y), a - vec2(3.0, 0.0))
}

fn encre_entre(rgba: &[u8], de: Pos2, a: Pos2) -> i32 {
    let (mut t, n) = (0, (a - de).length().ceil() as i32);
    for k in 0..=n {
        let q = de + (a - de) * (k as f32 / n.max(1) as f32);
        for w in -4..=4 {
            let v = px(rgba, q.x as i32, q.y as i32 + w);
            t += (v[0] - v[1]).max(0);
        }
    }
    t
}

#[test]
fn the_margin_shows_on_the_paper_for_every_plan_and_the_passe_partout_hides_it() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let (de, a) = trait_dans_la_marge(&mut b);
        assert!(encre_entre(&b.capturer().unwrap(), de, a) > 200, "bitmap {bitmap} : la marge du plan courant se voit");
        // Un autre plan courant : elle se voit toujours.
        b.app.select_panel(4);
        b.chauffer(4);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        assert!(encre_entre(&rgba, de, a) > 200, "bitmap {bitmap} : la marge d'un plan qui n'est pas courant se voit");
        // Le passe-partout allumé : cachée comme tout ce que la caméra ne montre pas.
        b.app.vignettes.annot.marie = true;
        b.chauffer(4);
        let rgba = b.capturer().unwrap();
        assert_eq!(encre_entre(&rgba, de, a), 0, "bitmap {bitmap} : le passe-partout cache la marge");
        b.app.vignettes.annot.marie = false;
        b.chauffer(2);
    }
}

#[test]
fn the_margin_never_covers_the_image_of_another_case() {
    // Un grand débord : le trait monte dans la marge de la case 7, dont la marge couvre l'image de la case du dessus. Rien ne vient par-dessus elle.
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.settings.vignettes.debord = 40;
    b.app.layout.card_size = 800.0;
    b.app.board_zoom = 0.0;
    b.chauffer(8);
    b.attendre_vignettes();
    // Dans le cadre de la case 1, un trait la traverse ; une case voisine en reçoit une part dans sa marge, sous l'image.
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (haut, bas) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[7].thumb));
    let x = bas.left() + 20.0;
    tracer(&mut b, pos2(x, bas.center().y), pos2(x, bas.top() - 2.0));
    repos(&mut b);
    let rgba = b.capturer().unwrap();
    assert_eq!(poids_dans(&rgba, haut), 0, "l'image de la case du dessus reste nette");
    assert!(poids_dans(&rgba, bas) > 0);
}

#[test]
fn only_the_board_shows_the_margin_the_film_and_the_storyboard_show_the_camera_frame() {
    let Some(mut b) = planche(des_plans(18, false), 6, 1.0) else { return println!("pas de carte graphique") };
    // Un trait qui entre dans le cadre par son bord haut, au tout début : presque tout dans la marge, à droite de la case.
    let (de, a) = trait_dans_la_marge(&mut b);
    assert!(encre_entre(&b.capturer().unwrap(), de, a) > 200, "la planche montre la marge");
    let dessin: Vec<_> = points_du_plan(&b, 1);
    assert!(dessin.iter().any(|p| p.x > b.app.board_layout().cards[1].thumb.right() + 2.0), "le plan a bien du dessin hors de son cadre");
    let plan = crate::render::panel_plan_at(&b.app.doc, 1, 0, Some(b.app.doc.paper));
    let cam = b.app.doc.panel_camera(1, 0);
    let image = crate::render::thumbnail(&b.app.doc, &plan, &cam, 192, 108);
    assert_eq!(image.size, [192, 108]);
    // Le trait quitte le cadre à droite : seule la dernière colonne ou presque en a. Ce qui dépasse n'a pris aucune place de plus dans l'image.
    let encre_a = |x: usize| (0..108).any(|y| image.pixels[y * 192 + x].r() as i32 - image.pixels[y * 192 + x].g() as i32 > 8);
    assert!(encre_a(191), "le trait va jusqu'au bord du cadre");
    assert!(!encre_a(150), "et rien d'autre : la vignette de la barre, le film et l'animatique n'ont que le cadre de la caméra");
}

// ------------------------------------------------------------------------------ l'export de la planche

pub(crate) fn exporter(b: &mut Banc, opt: &crate::planche::Options, nom: &str) -> image::RgbImage {
    let dir = std::env::temp_dir().join(format!("shot-test-traits-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier");
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let sortie = dir.join("planche.png");
    b.app.export_job = None;
    let job = b.app.planche_job(&plans, opt, &sortie).expect("tâche");
    b.app.start_job(job);
    let debut = std::time::Instant::now();
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < std::time::Duration::from_secs(120) {
        b.image(Vec::new(), 1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    b.chauffer(2);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.is_ok(), "{fin:?}");
    let img = image::open(&sortie).expect("PNG").to_rgb8();
    std::fs::remove_dir_all(&dir).ok();
    img
}

#[test]
fn the_export_of_the_board_applies_the_same_rule() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[3].thumb));
        let y = v0.top() + v0.height() * 0.2;
        let (de, a) = (pos2(v0.center().x, y), pos2(v3.center().x, y));
        tracer(&mut b, de, a);
        repos(&mut b);
        let (bd, ba) = (xf.to_board(de), xf.to_board(a));
        let mesurer = |b: &Banc, img: &image::RgbImage, opt: &crate::planche::Options| {
            let opt = b.app.planche_options(opt);
            let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
            let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
            let (s, ..) = crate::planche::dimensions(p.cadre, opt.resolution);
            let vers = |q: Pos2| pos2((q.x - p.cadre.left()) * s, (q.y - p.cadre.top()) * s);
            let (i0, i1) = (vers(bd), vers(ba));
            let long = (i1 - i0).length() as i32;
            let (mut trou, mut courant, mut encre) = (0, 0, 0);
            for k in (8.0 * s) as i32..long - (8.0 * s) as i32 {
                let c = i0 + (i1 - i0) * (k as f32 / long as f32);
                let haut = (-6..=6).map(|w| {
                    let (x, y) = (c.x as i32, c.y as i32 + (w as f32 * s.max(1.0)) as i32);
                    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
                        let v = img.get_pixel(x as u32, y as u32);
                        v[0] as i32 - v[1] as i32
                    } else {
                        0
                    }
                });
                if haut.max().unwrap_or(0) >= ENCRE {
                    courant = 0;
                    encre += 1;
                } else {
                    courant += 1;
                    trou = trou.max(courant);
                }
            }
            (trou, encre, s)
        };
        let opt = crate::planche::Options::default();
        let img = exporter(&mut b, &opt, &format!("sans-{bitmap}"));
        if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
            let _ = img.save(std::path::PathBuf::from(d).join(format!("export-{bitmap}.png")));
        }
        let (trou, _, s) = mesurer(&b, &img, &opt);
        println!("TRAIT export bitmap {bitmap} : trou {trou} pixels, échelle {s}");
        // (Le filet d'une case passe sur le trait : son épaisseur à cette échelle.)
        assert!(trou as f32 <= 2.0 + 2.0 * s.max(1.0).round(), "bitmap {bitmap} : l'export a un trou de {trou} pixels (échelle {s})");
        // Avec le passe-partout : seules les images se voient, la marge ne sort pas.
        let opt = crate::planche::Options { marie: true, ..Default::default() };
        let img = exporter(&mut b, &opt, &format!("avec-{bitmap}"));
        let (trou, encre, _) = mesurer(&b, &img, &opt);
        println!("TRAIT export avec passe-partout bitmap {bitmap} : trou {trou}, {encre} pixels d'encre");
        assert!(trou as f32 >= b.app.vignettes.xf.zoom * 5.0, "bitmap {bitmap} : le passe-partout cache ce qui est entre les cases");
    }
}

// ------------------------------------------------------------------------------ annuler, enregistrer, rouvrir

/// Combien de pixels de la planche diffèrent franchement.
fn ecarts(a: &[u8], b: &[u8], vue: eframe::egui::Rect) -> usize {
    let mut n = 0;
    for y in vue.top() as i32..vue.bottom() as i32 {
        for x in vue.left() as i32..vue.right() as i32 {
            let (p, q) = (px(a, x, y), px(b, x, y));
            if (0..3).any(|c| (p[c] - q[c]).abs() > 48) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn undo_redo_saving_and_reopening_give_the_same_image() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(18, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        repos(&mut b);
        let vue = b.app.vignettes.view;
        let avant = b.capturer().unwrap();
        let l = b.app.board_layout();
        let xf = b.app.vignettes.xf;
        let (v0, v3) = (xf.rect(l.cards[0].thumb), xf.rect(l.cards[8].thumb));
        let (de, a) = (v0.center() + vec2(10.0, 7.0), v3.center() - vec2(9.0, 5.0));
        tracer(&mut b, de, a);
        repos(&mut b);
        let apres = b.capturer().unwrap();
        assert!(ecarts(&avant, &apres, vue) > 50, "le trait se voit");
        continu(&apres, de, a, "avant d'annuler");
        b.app.run_action("edition.annuler");
        repos(&mut b);
        let annule = b.capturer().unwrap();
        // (La case vive est revenue : on compare à l'image d'avant, le cadre courant compris.)
        let e = ecarts(&avant, &annule, vue);
        assert!(e <= 8, "bitmap {bitmap} : annuler rend l'image d'avant : {e} pixels diffèrent");
        b.app.run_action("edition.retablir");
        repos(&mut b);
        let refait = b.capturer().unwrap();
        let e = ecarts(&apres, &refait, vue);
        assert!(e <= 8, "bitmap {bitmap} : rétablir rend l'image d'après : {e} pixels diffèrent");
        continu(&refait, de, a, "après rétablir");
        // Enregistrer et rouvrir.
        let dossier = std::env::temp_dir().join(format!("shot-test-traits-relu-{bitmap}-{}-{:?}", std::process::id(), std::thread::current().id()));
        std::fs::remove_dir_all(&dossier).ok();
        std::fs::create_dir_all(&dossier).unwrap();
        let manifeste = dossier.join("P.shot");
        crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
        let relu = crate::project::load(&manifeste).expect("réouverture");
        let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
        let Some(mut c) = planche(relu.doc, 6, 1.0) else { return };
        c.app.board_zoom = zoom;
        c.app.board_pan = pan;
        c.chauffer(4);
        repos(&mut c);
        let rouvert = c.capturer().unwrap();
        sauver(&rouvert, &format!("rouvert-{bitmap}"));
        sauver(&refait, &format!("refait-{bitmap}"));
        let (r1, r2) = (relever(&refait, de, a), relever(&rouvert, de, a));
        println!("TRAIT rouvert bitmap {bitmap} : {r1:?} {r2:?}");
        continu(&rouvert, de, a, "après réouverture");
        std::fs::remove_dir_all(&dossier).ok();
    }
}

// ------------------------------------------------------------------------------ aucune latence

#[test]
fn thirty_drawn_plans_rest_without_repainting_and_a_frame_stays_cheap() {
    let Some(mut b) = planche(des_plans(30, false), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let rejeux_avant = crate::vector::replay_count();
    let n = l.cards.len();
    for k in 0..n.saturating_sub(1) {
        if k % 6 == 5 {
            continue;
        }
        let (u, v) = (xf.rect(l.cards[k].thumb), xf.rect(l.cards[k + 1].thumb));
        tracer(&mut b, u.center() - vec2(0.0, 8.0), v.center() + vec2(0.0, 6.0));
        b.chauffer(1);
    }
    assert_eq!(crate::vector::replay_count(), rejeux_avant, "aucun rejeu du moteur sur le fil de l'interface, pour aucun trait");
    repos(&mut b);
    // Au repos : plus aucune image demandée, sauf par `fx::draft_level`, commun au processus (seul, cet essai n'en voit aucune).
    for _ in 0..4 {
        b.image(Vec::new(), 0.5);
    }
    let reveil = b.raison_du_reveil();
    let derniers: Vec<&str> = reveil.lines().rev().take(3).collect();
    println!("COUT repos : {derniers:?}");
    assert!(derniers.iter().all(|l| l.contains("demandée par rien") || l.contains("fx.rs")), "la planche ne redessine pas en continu : {reveil}");
    // Le coût d'une image, la souris qui se promène sur la planche.
    let vue = b.app.vignettes.view;
    let mut mesures: Vec<(f64, f64, f64, usize)> = Vec::new();
    for k in 0..120 {
        let q = vue.center() + vec2((k as f32 * 0.21).sin() * vue.width() * 0.4, (k as f32 * 0.13).cos() * vue.height() * 0.4);
        let m = b.image(vec![eframe::egui::Event::PointerMoved(q)], 1.0 / 60.0);
        mesures.push((m.construction, m.decoupage, m.carte, m.sommets));
    }
    let mediane = |f: &dyn Fn(&(f64, f64, f64, usize)) -> f64| {
        let mut v: Vec<f64> = mesures.iter().map(f).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let (c, d, g, s) = (mediane(&|m| m.0), mediane(&|m| m.1), mediane(&|m| m.2), mediane(&|m| m.3 as f64));
    println!("COUT image, {n} cases dessinées : construction {c:.2} ms, découpage {d:.2} ms, carte {g:.2} ms, {s:.0} sommets");
    assert!(c < 16.0, "construction d'une image : {c:.2} ms");
    // Le coût d'une vignette, sur un fil de fond : celle de la case d'aujourd'hui, celle d'avant (256 pixels, 4 lectures).
    let plan = crate::render::panel_plan_at(&b.app.doc, 0, 0, Some(b.app.doc.paper));
    let cam = b.app.doc.panel_camera(0, 0);
    let t = std::time::Instant::now();
    let _ = crate::render::thumbnail_marge_until(&b.app.doc, &plan, &cam, 192, 108, 23, None, &|| false);
    let maintenant = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    let _ = crate::render::thumbnail_marge_until(&b.app.doc, &plan, &cam, 318, 180, 0, None, &|| false);
    let avant = t.elapsed().as_secs_f64() * 1e3;
    println!("COUT vignette d'une case : {maintenant:.1} ms ; avant (4 lectures, 256 pixels et sa marge) : {avant:.1} ms");
}