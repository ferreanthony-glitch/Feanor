//! Essais des retouches des Vignettes (2026-10-21) : le trait qui déborde de sa case pendant le geste (`debord.rs`),
//! et le déplacement d'un plan par le cadre d'une de ses vignettes, Maj + glisser (`boardmove.rs`).

use eframe::egui::{epaint::ColorMode, pos2, vec2, CursorIcon, Event, Key, Modifiers, Pos2, Rect, Shape};

use crate::app::Tool;
use crate::board::{self, Hit};
use crate::boardui::Press;
use crate::perf_tests::{se_reposer, Banc};
use crate::vignettes_tests::{appui, barre, dernier_nom, encre, ouvert, pas_d_annulation, quatre_plans, vignette};

const MAGENTA: [u8; 3] = [255, 0, 255];

fn plans(b: &Banc) -> Vec<u64> {
    b.app.doc.panels.iter().map(|p| p.id).collect()
}

/// Les lignes que la planche a peintes dans la couleur du trait (magenta) à la dernière image : leur fenêtre de coupe
/// et leurs points.
fn lignes_magenta(b: &Banc) -> Vec<(Rect, Vec<Pos2>)> {
    b.formes
        .iter()
        .filter_map(|s| match &s.shape {
            Shape::Path(p) => match p.stroke.color {
                ColorMode::Solid(c) => {
                    // (Le trait du dehors est pâle, translucide : la couleur est prémultipliée par son opacité.)
                    let [r, g, bl, a] = c.to_array();
                    (a > 0 && (g as u32) * 8 < a as u32 && (r as u32) * 10 >= a as u32 * 9 && (bl as u32) * 10 >= a as u32 * 9).then(|| (s.clip_rect, p.points.clone()))
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Celles qui sont peintes dehors : leur fenêtre de coupe ne touche pas l'intérieur de la case.
fn debord(b: &Banc, case: Rect) -> Vec<(Rect, Vec<Pos2>)> {
    let dedans = case.shrink(1.5);
    lignes_magenta(b).into_iter().filter(|(clip, _)| !clip.intersects(dedans)).collect()
}

fn echap(pressed: bool) -> Event {
    Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed, repeat: false, modifiers: Modifiers::NONE }
}

macro_rules! maj_glisser {
    ($b:ident, $de:expr, [$($q:expr),*], $a:expr, $lacher:expr) => {{
        let chemin = [$($q),*];
        glisser_maj(&mut $b, $de, &chemin, $a, $lacher)
    }};
}

/// Un geste Maj : appui sur `de`, le pointeur passe par `chemin`, lâcher à `a`. Rend rien ; l'état se lit après.
fn glisser_maj(b: &mut Banc, de: Pos2, chemin: &[Pos2], a: Pos2, lacher: bool) {
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true, Modifiers::SHIFT)], 1.0 / 60.0);
    for q in chemin {
        b.image(vec![Event::PointerMoved(*q)], 1.0 / 60.0);
    }
    b.image(vec![Event::PointerMoved(a)], 1.0 / 60.0);
    if lacher {
        b.image(vec![appui(a, false, Modifiers::SHIFT)], 1.0 / 60.0);
        b.touches = Modifiers::NONE;
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

/// Le point du cadre gauche (deux points dedans) de la vignette `k`, à l'écran.
fn cadre(b: &Banc, k: usize) -> Pos2 {
    let v = vignette(b, k);
    pos2(v.left() + 2.0, v.center().y)
}

/// L'écart entre la vignette `k` et la suivante (même ligne), à l'écran : son milieu.
fn ecart(b: &Banc, k: usize) -> Pos2 {
    let (a, c) = (vignette(b, k), vignette(b, k + 1));
    pos2((a.right() + c.left()) * 0.5, a.center().y)
}

/// Le point juste devant la vignette `k`, là où se pose un plan : dans l'écart qui la précède, ou, si elle commence une
/// ligne, un peu à sa gauche.
fn devant(b: &Banc, k: usize) -> Pos2 {
    let l = b.app.board_layout();
    let v = vignette(b, k);
    if k > 0 && l.cards[k - 1].row == l.cards[k].row {
        ecart(b, k - 1)
    } else {
        pos2(v.left() - 14.0, v.center().y)
    }
}

// ------------------------------------------------------------------------------------ le trait qui déborde

#[test]
fn the_stroke_overflows_its_box_while_drawing_and_the_box_clips_it_after_but_the_data_keeps_it_all() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.color = MAGENTA;
    b.garder_formes = true;
    let (case, voisine) = (vignette(&b, 1), vignette(&b, 2));
    let depart = case.center();
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![appui(depart, true, Modifiers::NONE)], 1.0 / 60.0);
    // Dans la case : rien ne déborde, c'est le dessin lui-même qui est peint (et coupé au bord).
    b.image(vec![Event::PointerMoved(depart + vec2(10.0, 2.0))], 1.0 / 120.0);
    assert!(debord(&b, case).is_empty(), "un trait qui reste dans sa case ne produit aucune forme dehors");
    // Il sort à droite, traverse l'écart et la barre, et entre dans la case voisine.
    let mut fin = depart;
    for k in 1..=26 {
        fin = depart + vec2(k as f32 * 7.0, (k as f32 * 0.4).sin() * 5.0);
        b.image(vec![Event::PointerMoved(fin)], 1.0 / 120.0);
    }
    assert!(fin.x > voisine.left() + 20.0, "le trait est allé dans la case voisine");
    let dehors = debord(&b, case);
    assert!(!dehors.is_empty(), "le trait se voit dehors pendant le geste");
    assert!(
        dehors.iter().any(|(clip, pts)| clip.intersects(voisine) && pts.iter().any(|p| voisine.contains(*p))),
        "par-dessus la case voisine : {dehors:?}"
    );
    assert!(
        dehors.iter().any(|(_, pts)| pts.iter().any(|p| p.x <= case.right() + 8.0) && pts.iter().any(|p| p.x >= voisine.left())),
        "d'un seul tenant, par-dessus l'écart et sa barre"
    );
    // La pointe suit la croix sans retard : le dernier point du tracé dehors est à la pointe.
    let bout = dehors.iter().flat_map(|(_, pts)| pts.iter().copied()).fold(0.0_f32, |m, p| m.max(p.x));
    assert!((bout - fin.x).abs() < 9.0, "le trait va jusqu'à la pointe : {bout} contre {}", fin.x);
    // Et rien n'est peint dehors qui recouvre la case elle-même (le dedans est exact, il n'est pas redoublé).
    assert!(lignes_magenta(&b).iter().all(|(clip, _)| !clip.contains_rect(case.shrink(2.0))), "aucune forme du débord ne couvre l'intérieur de la case");

    // Au lâcher : le trait tracé reste peint le temps que ses parts soient rendues en fond (partie 111), puis plus rien du débord.
    b.image(vec![appui(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    crate::perf_tests::se_reposer(&mut b, 2000);
    assert!(debord(&b, case).is_empty(), "une fois les parts rendues, le débord ne peint plus rien");
    // … mais les données gardent tout le trait (toile infinie).
    assert!(encre(&b.app.doc, 0));
    // (Depuis le 2026-10-30, ce qui est hors du champ de la case n'est plus dans son dessin : la part dans la case voisine est dans le plan voisin, le reste sur la planche, `horschamp_tests.rs`.)
    // (La case voisine est du même plan : sa part est dans le dessin de son image.)
    assert!(b.app.vignettes.debord.is_none());
}

#[test]
fn a_shape_preview_also_overflows_while_dragged() {
    let mut b = ouvert(quatre_plans(), 4);
    b.app.tool = Tool::Line;
    b.app.color = MAGENTA;
    b.garder_formes = true;
    let (case, voisine) = (vignette(&b, 1), vignette(&b, 2));
    let depart = case.center();
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![appui(depart, true, Modifiers::NONE)], 1.0 / 60.0);
    let fin = voisine.center();
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(depart + (fin - depart) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    let dehors = debord(&b, case);
    assert!(dehors.iter().any(|(_, pts)| pts.iter().any(|p| (p.x - fin.x).abs() < 3.0)), "la ligne tirée se voit jusqu'au bout, dehors : {dehors:?}");
    b.image(vec![appui(fin, false, Modifiers::NONE)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    crate::perf_tests::se_reposer(&mut b, 2000);
    assert!(debord(&b, case).is_empty());
}

#[test]
fn the_overflow_costs_nothing_to_a_stroke_that_stays_in_its_box_and_little_to_one_that_leaves() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 1);
    let run = |b: &mut Banc, path: &[Pos2]| -> Vec<f64> {
        let mut times = Vec::new();
        b.image(vec![Event::PointerMoved(path[0])], 1.0 / 60.0);
        times.push(b.image(vec![appui(path[0], true, Modifiers::NONE)], 1.0 / 60.0).construction);
        for chunk in path[1..].chunks(4) {
            times.push(b.image(chunk.iter().map(|q| Event::PointerMoved(*q)).collect(), 1.0 / 60.0).construction);
        }
        b.image(vec![appui(*path.last().unwrap(), false, Modifiers::NONE)], 1.0 / 60.0);
        b.image(Vec::new(), 1.0 / 60.0);
        times
    };
    let stats = |v: &[f64]| (v.iter().sum::<f64>() / v.len() as f64, v.iter().cloned().fold(0.0, f64::max));
    // Deux traits de 240 points : l'un tourne dans la case, l'autre file vers la droite, hors de la case (et revient).
    let dedans: Vec<Pos2> = (0..240).map(|k| v.center() + vec2((k as f32 * 0.17).sin() * 50.0, (k as f32 * 0.23).cos() * 30.0)).collect();
    let dehors: Vec<Pos2> = (0..240).map(|k| v.center() + vec2((k as f32 * 0.05).sin() * 260.0, (k as f32 * 0.23).cos() * 30.0)).collect();
    run(&mut b, &dedans);
    let ((dm, dp), (om, op)) = (stats(&run(&mut b, &dedans)), stats(&run(&mut b, &dehors)));
    println!("DEBORD : trait dans la case : image moyenne {dm:.2} ms, pire {dp:.2} ms | trait qui sort : moyenne {om:.2} ms, pire {op:.2} ms");
    assert!(om < dm * 2.0 + 25.0, "image moyenne {om:.1} ms en débordant contre {dm:.1} ms dans la case");
    assert!(op < 3000.0, "pire image {op:.1} ms");

    // Le calcul du débord seul, sur un tracé de 4 000 points qui sort (la vue de l'application n'est pas celle de la
    // vignette ici : seul le coût compte).
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 1);
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(vec![appui(v.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 0..500 {
        let q = v.center() + vec2((k as f32 * 0.02).sin() * 300.0, (k as f32 * 0.31).cos() * 40.0);
        b.image(vec![Event::PointerMoved(q), Event::PointerMoved(q + vec2(1.0, 1.0))], 1.0 / 120.0);
    }
    let n = b.app.stroke.as_ref().map_or(0, |s| s.path.len());
    assert!(n > 300, "le tracé est long ({n} points)");
    let t0 = std::time::Instant::now();
    let mut formes = 0;
    for _ in 0..100 {
        formes += b.app.debord_collect(v, None, false).map_or(0, |d| d.shapes.len());
    }
    let par_image = t0.elapsed().as_secs_f64() * 10.0;
    println!("DEBORD : calcul seul sur {n} points : {par_image:.3} ms par image ({} formes)", formes / 100);
    assert!(par_image < 3.0, "{par_image:.2} ms par image pour {n} points");
    b.image(vec![appui(v.center(), false, Modifiers::NONE)], 1.0 / 60.0);
}

// ------------------------------------------------------------------------------------ le cadre d'une vignette

#[test]
fn the_frame_is_the_border_of_a_box_with_a_few_points_of_tolerance_on_both_sides() {
    let b = ouvert(quatre_plans(), 4);
    let layout = b.app.board_layout();
    let t = layout.cards[1].thumb;
    // Le filet, à un point dedans comme dehors, au milieu de chaque côté : le cadre.
    for p in [pos2(t.left() + 1.0, t.center().y), pos2(t.left() - 2.0, t.center().y), pos2(t.center().x, t.top() + 2.0), pos2(t.center().x, t.bottom() + 3.0), pos2(t.right() - 3.0, t.center().y)] {
        assert_eq!(board::frame_hit(&layout, p, 5.0), Some(1), "{p:?}");
    }
    // Le centre et le dedans plus loin que la tolérance, ou le dehors au-delà : pas le cadre.
    for p in [t.center(), pos2(t.left() + 12.0, t.center().y), pos2(t.center().x, t.top() - 9.0)] {
        assert_eq!(board::frame_hit(&layout, p, 5.0), None, "{p:?}");
    }
    // L'écart de dix points entre deux vignettes d'un plan est tout entier cadre (cinq points de chaque côté).
    let milieu = pos2(t.left() - 5.0, t.center().y);
    assert!(matches!(board::frame_hit(&layout, milieu, 5.0), Some(0 | 1)));
    // Une petite vignette garde son centre : la bande ne mange pas plus d'un quart.
    let mut mince = layout.clone();
    mince.cards[1].thumb = Rect::from_min_size(t.min, vec2(16.0, 16.0));
    assert_eq!(board::frame_hit(&mince, mince.cards[1].thumb.center(), 5.0), None);
    // Entre deux cadres qui se touchent, le plus proche l'emporte.
    let mut serre = layout.clone();
    serre.cards[2].thumb = Rect::from_min_size(pos2(t.right() + 4.0, t.top()), t.size());
    assert_eq!(board::frame_hit(&serre, pos2(t.right() + 0.5, t.center().y), 5.0), Some(1));
    assert_eq!(board::frame_hit(&serre, pos2(t.right() + 3.5, t.center().y), 5.0), Some(2));
}

#[test]
fn shift_press_anywhere_in_a_case_takes_the_plan_and_a_shift_click_chooses_it() {
    let mut b = ouvert(quatre_plans(), 4);
    let ids = plans(&b);
    let avant = pas_d_annulation(&b);
    // Dedans (au-delà de la bande) : le plan est pris (partie 113, proposition A), et un clic sans mouvement le choisit, au lâcher.
    let v = vignette(&b, 4);
    let dedans = pos2(v.left() + 30.0, v.center().y);
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(dedans)], 1.0 / 60.0);
    b.image(vec![appui(dedans, true, Modifiers::SHIFT)], 1.0 / 60.0);
    assert!(b.app.vignettes.moving.is_some() && b.app.vignettes.press == Press::Move);
    b.image(vec![appui(dedans, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.board_selection().contains(&ids[2]), "Maj + clic dedans choisit le plan");
    assert!(b.app.vignettes.moving.is_none());
    // Sur le cadre : pareil, le plan est pris ; un clic sans glissement ne déplace rien.
    b.app.board_sel.clear();
    b.app.vignettes.libre = true;
    let cadre = cadre(&b, 4);
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(cadre)], 1.0 / 60.0);
    b.image(vec![appui(cadre, true, Modifiers::SHIFT)], 1.0 / 60.0);
    assert_eq!(b.app.vignettes.press, Press::Move);
    assert_eq!(b.app.vignettes.moving.as_ref().map(|m| m.ids.clone()), Some(vec![ids[2]]));
    b.image(vec![appui(cadre, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(plans(&b), ids, "un simple clic sur le cadre (aucun glissement) ne déplace rien");
    assert_eq!(pas_d_annulation(&b), avant);
    assert!(b.app.vignettes.moving.is_none());
    for i in 0..4 {
        assert!(!encre(&b.app.doc, i), "et ne dessine rien");
    }
    // Sans Maj, un appui au même endroit dessine (la vignette est dessinable jusqu'au bord).
    b.app.board_sel.clear();
    let tracer = vignette(&b, 3);
    b.image(vec![Event::PointerMoved(tracer.center())], 1.0 / 60.0);
    b.image(vec![appui(tracer.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    assert_eq!(b.app.vignettes.press, Press::Card);
    b.image(vec![appui(tracer.center(), false, Modifiers::NONE)], 1.0 / 60.0);
}

#[test]
fn the_cursor_is_a_hand_on_the_frame_with_shift_held_even_on_the_current_box() {
    let mut b = ouvert(quatre_plans(), 4);
    // Sans Maj : le curseur ordinaire.
    let (courante, autre) = (cadre(&b, 0), cadre(&b, 4));
    b.image(vec![Event::PointerMoved(autre)], 1.0 / 60.0);
    assert_ne!(b.curseur, CursorIcon::Grab);
    // Avec Maj : une main sur le cadre d'une autre vignette comme sur celle de la vignette courante (dont le dessin
    // demande sinon son pinceau).
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(autre)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.curseur, CursorIcon::Grab, "cadre d'une autre vignette");
    assert!(b.app.vignettes.frame_hot);
    b.image(vec![Event::PointerMoved(courante)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.cur_panel, 0);
    assert_eq!(b.curseur, CursorIcon::Grab, "cadre de la vignette courante");
    // Dedans, avec Maj : la main aussi (partie 113, proposition A : Maj prend la case n'importe où) ; à un coin, la caméra (pas de main).
    let v = vignette(&b, 4);
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.curseur, CursorIcon::Grab, "dedans aussi");
    assert!(b.app.vignettes.frame_hot);
    b.image(vec![Event::PointerMoved(v.right_bottom() - vec2(1.0, 1.0))], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.app.vignettes.frame_hot, "un coin : la caméra, pas la main");
    b.touches = Modifiers::NONE;
    b.chauffer(2);
}

// ------------------------------------------------------------------------------------ déplacer un plan

#[test]
fn the_drop_marker_is_only_between_plans_never_inside_one() {
    for colonnes in [3, 4, 6] {
        let b = ouvert(quatre_plans(), colonnes);
        let layout = b.app.board_layout();
        let ordre = plans(&b);
        let slots = board::drop_slots(&layout);
        // Un repère devant chaque plan et un après le dernier : cinq pour quatre plans (pas un par vignette).
        assert_eq!(slots.len(), ordre.len() + 1);
        assert!(slots.iter().filter_map(|s| s.before).all(|id| ordre.contains(&id)), "jamais devant une marque d'image");
        // Où que soit le pointeur sur la planche, le repère choisi n'est jamais dans un plan.
        for p in [0.0, 17.0, 83.0, 140.0] {
            for y in (0..layout.size.y as i32).step_by(23) {
                for x in (0..layout.size.x as i32).step_by(19) {
                    let moving = [ordre[(x as usize + y as usize) % ordre.len()]];
                    let q = pos2(x as f32 + p, y as f32);
                    if let Some(s) = board::nearest_slot(&slots, &ordre, &moving, q) {
                        assert!(s.before.is_none_or(|id| ordre.contains(&id) && !moving.contains(&id)), "{s:?}");
                        assert!(board::drop_changes_order(&ordre, &moving, s.before), "un repère qui ne ferait rien n'est pas proposé");
                    }
                }
            }
        }
    }
    // Les repères se placent dans l'écart avant la première vignette du plan, et au début de la ligne si c'est la première.
    let b = ouvert(quatre_plans(), 3);
    let layout = b.app.board_layout();
    let slots = board::drop_slots(&layout);
    let devant = |plan: u64| slots.iter().find(|s| s.before == Some(plan)).copied().unwrap();
    let ordre = plans(&b);
    let (s1, s2) = (devant(ordre[1]), devant(ordre[2]));
    assert!(s1.x < layout.cards[3].thumb.left(), "devant la première vignette de la ligne : à sa gauche");
    assert!(s2.x < layout.cards[4].thumb.left() && s2.x > layout.cards[3].thumb.right());
}

#[test]
fn dragging_a_box_of_a_plan_moves_the_whole_plan_as_one_block_between_two_plans() {
    let mut b = ouvert(quatre_plans(), 4);
    let ordre = plans(&b);
    // Le plan 1 a trois vignettes ; on tire la deuxième (une marque d'image) par son cadre, vers l'écart avant le plan 3
    // (début de la deuxième ligne), en passant au-dessus de ses propres vignettes et d'une case à l'autre.
    b.app.doc.panels[0].notes = "Plan large".into();
    b.app.doc.panels[0].marks[0].notes = "Il entre".into();
    b.app.doc.panels[0].camera.push(crate::camera::CameraKey::new(3, crate::camera::Camera::centered(160, 90)));
    let marques_avant = b.app.doc.panels[0].marks.clone();
    let notes_avant = (b.app.doc.panels[0].notes.clone(), b.app.doc.panels[0].camera.len(), b.app.doc.panels[0].layers.len(), b.app.doc.panels[0].length);
    b.chauffer(3);
    let (prise, cible) = (cadre(&b, 1), devant(&b, 4));
    let avant = pas_d_annulation(&b);
    maj_glisser!(b, prise, [vignette(&b, 1).center(), vignette(&b, 2).center(), ecart(&b, 2)], cible, false);
    // Pendant le geste : un seul repère, entre deux plans, et le plan est entier dans le fantôme.
    let mv = b.app.vignettes.moving.as_ref().expect("le plan est pris");
    assert!(mv.active);
    assert_eq!(mv.ids, vec![ordre[0]], "le plan entier, quelle que soit la vignette tirée");
    let slot = mv.target.expect("un repère de pose");
    assert_eq!(slot.before, Some(ordre[2]), "devant le plan 3 : entre deux plans");
    assert_eq!(plans(&b), ordre, "rien n'a bougé avant le lâcher");
    b.image(vec![appui(cible, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    // Après : le plan 1 est entre le plan 2 et le plan 3, avec toutes ses vignettes côte à côte.
    assert_eq!(plans(&b), vec![ordre[1], ordre[0], ordre[2], ordre[3]]);
    let l = b.app.board_layout();
    assert_eq!(l.cards.iter().map(|c| c.plan).collect::<Vec<_>>(), vec![ordre[1], ordre[0], ordre[0], ordre[0], ordre[2], ordre[3]], "les trois vignettes du plan restent ensemble");
    // Tout ce qui est au plan l'a suivi.
    let p = &b.app.doc.panels[1];
    assert_eq!(p.marks, marques_avant);
    assert_eq!((p.notes.clone(), p.camera.len(), p.layers.len(), p.length), notes_avant);
    assert_eq!(p.marks[0].notes, "Il entre");
    assert_eq!(pas_d_annulation(&b), avant + 1, "une seule étape d'annulation");
    assert_eq!(dernier_nom(&b), "Déplacer le plan");
    assert_eq!(b.app.cur_panel, 1, "le plan déplacé est courant");
    assert!(b.app.status.contains("déplacé"), "{}", b.app.status);
    // Annuler remet tout en place, rétablir le déplace de nouveau.
    b.app.run_action("edition.annuler");
    assert_eq!(plans(&b), ordre);
    assert_eq!(b.app.doc.panels[0].marks, marques_avant);
    b.chauffer(2);
    b.app.run_action("edition.retablir");
    assert_eq!(plans(&b), vec![ordre[1], ordre[0], ordre[2], ordre[3]]);
}

#[test]
fn a_single_box_plan_moves_and_a_cut_bar_lets_a_box_leave_its_plan_first() {
    let mut b = ouvert(quatre_plans(), 6);
    let ordre = plans(&b);
    // Le plan 3 (une vignette) passe devant le plan 1 : repère au début de la ligne.
    let (prise, cible) = (cadre(&b, 4), devant(&b, 0));
    maj_glisser!(b, prise, [ecart(&b, 3)], cible, true);
    assert_eq!(plans(&b), vec![ordre[2], ordre[0], ordre[1], ordre[3]]);
    assert_eq!(dernier_nom(&b), "Déplacer le plan");
    b.app.run_action("edition.annuler");
    assert_eq!(plans(&b), ordre);
    // Pour sortir une vignette d'un plan : on coupe sa barre (un clic), puis on la déplace (elle est devenue un plan).
    let zone = barre(&b, 1).center();
    b.image(vec![Event::PointerMoved(zone)], 1.0 / 60.0);
    b.image(vec![appui(zone, true, Modifiers::NONE)], 1.0 / 60.0);
    b.image(vec![appui(zone, false, Modifiers::NONE)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.doc.panels.len(), 5, "la barre coupée fait un plan de la vignette de droite");
    let ordre5 = plans(&b);
    let prise = cadre(&b, 2);
    let fin = pos2(vignette(&b, 5).right() + 12.0, vignette(&b, 5).center().y);
    maj_glisser!(b, prise, [vignette(&b, 4).center()], fin, true);
    assert_eq!(plans(&b), vec![ordre5[0], ordre5[2], ordre5[3], ordre5[4], ordre5[1]], "la vignette sortie du plan va tout à la fin");
}

#[test]
fn escape_cancels_the_move_and_nothing_changes() {
    let mut b = ouvert(quatre_plans(), 4);
    let ordre = plans(&b);
    let avant = pas_d_annulation(&b);
    let (prise, cible) = (cadre(&b, 4), devant(&b, 3));
    maj_glisser!(b, prise, [vignette(&b, 3).center()], cible, false);
    assert!(b.app.vignettes.moving.as_ref().is_some_and(|m| m.active && m.target.is_some()));
    b.image(vec![echap(true)], 1.0 / 60.0);
    b.image(vec![echap(false)], 1.0 / 60.0);
    assert!(b.app.vignettes.moving.as_ref().is_some_and(|m| m.cancelled && m.target.is_none()), "plus de repère");
    b.image(vec![appui(cible, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(b.app.vignettes.moving.is_none());
    assert_eq!(plans(&b), ordre, "le lâcher après Échap ne déplace rien");
    assert_eq!(pas_d_annulation(&b), avant);
    assert!(b.app.status.contains("abandonné"), "{}", b.app.status);
}

#[test]
fn a_plan_dropped_where_it_already_is_does_not_move_and_leaves_no_undo_step() {
    let mut b = ouvert(quatre_plans(), 4);
    let ordre = plans(&b);
    let avant = pas_d_annulation(&b);
    // Le plan 2 (carte 3) lâché devant lui-même, ou devant le plan suivant (qui est sa place) : rien.
    let prise = cadre(&b, 3);
    let v3 = vignette(&b, 3);
    glisser_maj(&mut b, prise, &[v3.center()], pos2(v3.center().x + 3.0, v3.center().y + 3.0), true);
    assert_eq!(plans(&b), ordre);
    assert_eq!(pas_d_annulation(&b), avant);
}

#[test]
fn a_plan_can_move_to_another_sequence_like_in_the_montage_and_a_locked_storyboard_refuses() {
    let mut b = ouvert(crate::vignettes_tests::deux_sequences(), 4);
    // (Deux séquences l'une sous l'autre : le zoom les montre toutes les deux dans la vue.)
    b.app.board_zoom = 0.6;
    b.app.board_pan = eframe::egui::Vec2::ZERO;
    b.chauffer(3);
    let ordre = plans(&b);
    let sequences: Vec<u64> = (0..4).map(|i| b.app.doc.sequence_of(i)).collect();
    assert_ne!(sequences[0], sequences[3]);
    // Le plan 4 (autre séquence) est posé entre les plans 1 et 2 : il rejoint leur séquence, la barre d'état le dit.
    let (prise, cible) = (cadre(&b, 5), devant(&b, 3));
    maj_glisser!(b, prise, [vignette(&b, 4).center()], cible, true);
    assert_eq!(plans(&b), vec![ordre[0], ordre[3], ordre[1], ordre[2]]);
    assert_eq!(b.app.doc.sequence_of(1), sequences[0], "il a pris la séquence du plan devant lequel on le pose");
    assert!(b.app.status.to_lowercase().contains("séquence"), "{}", b.app.status);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.sequence_of(3), sequences[3], "annuler lui rend aussi sa séquence");
    // Storyboard verrouillé : refusé tout de suite, avec un message, rien n'est pris.
    b.app.doc.storyboard_locked = true;
    let avant = pas_d_annulation(&b);
    let prise = cadre(&b, 4);
    b.touches = Modifiers::SHIFT;
    b.image(vec![Event::PointerMoved(prise)], 1.0 / 60.0);
    b.image(vec![appui(prise, true, Modifiers::SHIFT)], 1.0 / 60.0);
    assert!(b.app.vignettes.moving.is_none() && b.app.status.to_lowercase().contains("verrouill"), "{}", b.app.status);
    b.image(vec![appui(prise, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!((pas_d_annulation(&b), plans(&b).len()), (avant, 4));
}

#[test]
fn chosen_plans_move_together_when_one_of_them_is_taken_by_its_frame() {
    let mut b = ouvert(quatre_plans(), 4);
    let ordre = plans(&b);
    b.app.board_sel = vec![ordre[0], ordre[2]];
    // On prend le plan 3 : le plan 1 (choisi avec lui) le suit ; ils se posent entre les plans 2 et 4.
    let (prise, cible) = (cadre(&b, 4), devant(&b, 5));
    maj_glisser!(b, prise, [vignette(&b, 5).center()], cible, true);
    assert_eq!(plans(&b), vec![ordre[1], ordre[0], ordre[2], ordre[3]]);
    assert_eq!(dernier_nom(&b), "Déplacer les plans");
    b.app.run_action("edition.annuler");
    assert_eq!(plans(&b), ordre);
}

#[test]
fn the_ghost_and_the_marker_are_painted_during_the_move_and_the_hit_test_is_unchanged_elsewhere() {
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_formes = true;
    let (prise, cible) = (cadre(&b, 0), devant(&b, 4));
    // (Les formes d'une image ordinaire : pas de repère.)
    b.image(Vec::new(), 1.0 / 60.0);
    let accent = crate::theme::accent();
    let traits_accent = |b: &Banc| b.formes.iter().filter(|s| matches!(&s.shape, Shape::LineSegment { stroke, .. } if stroke.color == accent && (stroke.width - 3.5).abs() < 0.01)).count();
    assert_eq!(traits_accent(&b), 0);
    maj_glisser!(b, prise, [vignette(&b, 1).center(), ecart(&b, 2)], cible, false);
    assert_eq!(traits_accent(&b), 1, "un repère de pose, d'accent");
    // Le fantôme : un cadre d'accent par vignette du plan (trois), qui suit le pointeur.
    let cadres = b.formes.iter().filter(|s| matches!(&s.shape, Shape::Rect(r) if r.stroke.color == accent && r.stroke.width >= 2.0)).count();
    assert!(cadres >= 3, "le fantôme des trois vignettes du plan : {cadres}");
    assert_eq!(b.curseur, CursorIcon::Grabbing);
    // Le reste de la planche répond comme avant : la barre sous le pointeur est toujours une barre.
    let z = barre(&b, 0).center();
    assert_eq!(board::hit(&b.app.board_layout(), b.app.vignettes.xf.to_board(z), 14.0), Hit::Bar(0));
    b.image(vec![appui(cible, false, Modifiers::SHIFT)], 1.0 / 60.0);
    b.touches = Modifiers::NONE;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(traits_accent(&b), 0, "plus de repère après le lâcher");
}

#[test]
fn the_vignettes_view_is_idle_after_a_stroke_that_overflowed_and_after_a_move() {
    crate::fxcache::monde::ecrire();
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 1);
    b.image(vec![Event::PointerMoved(v.center())], 1.0 / 60.0);
    b.image(vec![appui(v.center(), true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=20 {
        b.image(vec![Event::PointerMoved(v.center() + vec2(k as f32 * 9.0, 0.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(v.center() + vec2(180.0, 0.0), false, Modifiers::NONE)], 1.0 / 60.0);
    let (prise, cible) = (cadre(&b, 4), devant(&b, 3));
    maj_glisser!(b, prise, [vignette(&b, 3).center()], cible, true);
    let n = se_reposer(&mut b, 120);
    assert!(n < 120, "la vue Vignettes se repose ({n} images){}", b.raison_du_reveil());
    let m = b.image(Vec::new(), 0.5);
    assert!(m.reveil >= std::time::Duration::from_secs(1), "{:?}", m.reveil);
}
