//! Essais du lissage des trajectoires (`trajectoirelisse.rs`, `Trajectoire::lisser_a`, `docs/INTERFACE.md` partie 151) :
//! le modèle (bouts fixes, non destructif, marques, tirer, enregistrer), le vrai curseur des réglages sur la feuille (une
//! étape d'annulation, les clés suivent, les prochains tracés), et dans l'Espace 3D sur tout ce qui est choisi.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::trajectoire::{distance, lissage_de, Cles, Support, Trajectoire};
use crate::trajectoire3d::Sujet;
use crate::trajectoireui_tests::banc;

/// Un zigzag de 300 pixels autour de la droite y = 150 : des dents de 15 pixels tous les 20 pixels, les deux bouts sur la
/// droite. Un point tous les 2 pixels, comme un vrai trait (le petit lissage du tracé ne l'efface pas).
fn zigzag() -> Vec<[f32; 2]> {
    let sommet = |k: usize| [50.0 + 20.0 * k as f32, 150.0 + if k == 0 || k == 15 { 0.0 } else if k % 2 == 0 { -15.0 } else { 15.0 }];
    let mut out = vec![sommet(0)];
    for k in 0..15 {
        let (a, b) = (sommet(k), sommet(k + 1));
        out.extend((1..=10).map(|j| {
            let w = j as f32 / 10.0;
            [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w]
        }));
    }
    out
}

/// L'écart le plus grand du chemin à la droite y = 150.
fn ecart(points: &[[f32; 2]]) -> f32 {
    points.iter().map(|p| (p[1] - 150.0).abs()).fold(0.0, f32::max)
}

#[test]
fn le_lissage_adoucit_le_chemin_garde_ses_bouts_et_se_defait() {
    let mut t = Trajectoire::nouvelle(&zigzag()).expect("un chemin");
    let brut = t.points.clone();
    let (debut, fin) = (brut[0], *brut.last().unwrap());
    t.ajouter_marque(0.4);
    t.lisser_a(1.0);
    assert_eq!(t.lissage, 1.0);
    assert_eq!(t.brut.as_deref(), Some(&brut[..]), "le chemin brut est gardé");
    assert!(distance(t.points[0], debut) < 1e-4 && distance(*t.points.last().unwrap(), fin) < 1e-4, "les bouts restent fixes");
    let milieu = &t.points[t.points.len() / 5..t.points.len() * 4 / 5];
    assert!(ecart(milieu) < ecart(&brut) * 0.35, "les dents s'adoucissent : {} contre {}", ecart(milieu), ecart(&brut));
    assert_eq!(t.marques, vec![0.4], "les marques gardent leur place relative");
    // Plus le curseur monte, plus c'est lisse ; on remonte, on redescend : le même chemin qu'en y allant tout droit.
    let mut moitie = t.clone();
    moitie.lisser_a(0.3);
    assert!(ecart(&moitie.points[moitie.points.len() / 5..moitie.points.len() * 4 / 5]) > ecart(milieu));
    let mut direct = Trajectoire::nouvelle(&zigzag()).unwrap();
    direct.lisser_a(0.3);
    assert_eq!(moitie.points, direct.points, "le lissage se recalcule depuis le trait brut, il ne s'empile pas");
    // À zéro : le trait tel qu'il a été tracé, rien de plus dans le projet.
    t.lisser_a(0.0);
    assert_eq!((t.points.clone(), t.brut.clone(), t.lissage), (brut.clone(), None, 0.0));
    // Le calcul tient sur un long chemin, et un chemin de deux points n'a rien à lisser.
    assert_eq!(lissage_de(&[[0.0, 0.0], [10.0, 0.0]], 1.0), vec![[0.0, 0.0], [10.0, 0.0]]);
}

#[test]
fn un_chemin_lisse_se_tire_se_deplace_et_s_enregistre_avec_son_trait_brut() {
    let mut t = Trajectoire::nouvelle(&zigzag()).unwrap();
    t.lisser_a(0.6);
    // Tiré : c'est le trait brut qui est tiré, et le chemin reste ce trait lissé.
    t.tirer(0.5, 0.0, 40.0, t.rayon_de_tirage());
    let brut = t.brut.clone().expect("toujours lissé");
    assert!(brut.iter().any(|p| p[1] > 180.0), "le trait brut a suivi le tirage");
    assert_eq!(t.points, lissage_de(&brut, 0.6));
    // Déplacé : les deux ensemble.
    t.deplacer(10.0, -5.0);
    assert_eq!(t.brut.as_ref().unwrap()[0], [brut[0][0] + 10.0, brut[0][1] - 5.0]);
    assert!(distance(t.points[0], t.brut.as_ref().unwrap()[0]) < 1e-4);
    // Enregistré, relu, assaini : tel quel.
    let texte = serde_json::to_string(&t).unwrap();
    assert!(texte.contains("\"lissage\"") && texte.contains("\"brut\""), "{texte}");
    let relue: Trajectoire = serde_json::from_str(&texte).unwrap();
    assert_eq!(relue.assainie().as_ref(), Some(&t));
    // Sans lissage, rien de plus qu'avant dans le fichier.
    let simple = Trajectoire::nouvelle(&zigzag()).unwrap();
    let texte = serde_json::to_string(&simple).unwrap();
    assert!(!texte.contains("lissage") && !texte.contains("brut"), "{texte}");
    // Un lissage abîmé, un trait brut abîmé : le chemin lissé devient le trait brut.
    let mut abimee = t.clone();
    abimee.brut = Some(vec![[f32::NAN, 0.0]]);
    let propre = abimee.assainie().unwrap();
    assert_eq!((propre.lissage, propre.brut.is_none(), propre.points == t.points), (0.0, true, true));
    let mut abimee = t.clone();
    abimee.lissage = f32::NAN;
    assert_eq!(abimee.assainie().unwrap().lissage, 0.0);
}

// ------------------------------------------------------------------------------------------------ le curseur

fn ecran(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

/// Un trait de l'outil par ces points de la feuille.
fn tracer(b: &mut Banc, pts: &[[f32; 2]]) {
    let p0 = ecran(b, pts[0][0], pts[0][1]);
    b.image(vec![Event::PointerMoved(p0)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p0, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    for p in &pts[1..] {
        let q = ecran(b, p[0], p[1]);
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
    }
    let fin = ecran(b, pts[pts.len() - 1][0], pts[pts.len() - 1][1]);
    b.image(vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
}

/// Le curseur « Lissage » seul dans un panneau, une image : sa place à l'écran.
fn curseur(b: &mut Banc, events: Vec<Event>, espace: bool) -> Rect {
    let ctx = b.ctx.clone();
    let app = &mut b.app;
    let time = Some(ctx.input(|i| i.time) + 1.0 / 60.0);
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 300.0))), events, time, ..Default::default() };
    let mut rect = Rect::NOTHING;
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| rect = app.trajectoire_lissage_ui(ui, espace));
    });
    rect
}

/// Le curseur tiré jusqu'à la fraction `vers` de sa course (au-delà de 1 : jusqu'au bout), en quelques images, puis lâché.
fn tirer_le_curseur(b: &mut Banc, vers: f32, espace: bool) {
    // (Un clic dans le vide du panneau d'abord : ce qu'un geste précédent du banc aurait laissé en cours est fini.)
    let vide = pos2(450.0, 280.0);
    curseur(b, vec![Event::PointerMoved(vide)], espace);
    curseur(b, vec![Event::PointerButton { pos: vide, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], espace);
    curseur(b, vec![Event::PointerButton { pos: vide, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], espace);
    let r = curseur(b, Vec::new(), espace);
    // (La glissière occupe les 100 premiers pixels du curseur, sa poignée à quelques pixels des bords ; son texte, la
    // droite.)
    let x = |f: f32| r.left() + 6.0 + 88.0 * f;
    let (y, de) = (r.center().y, pos2(x(0.0), r.center().y));
    curseur(b, vec![Event::PointerMoved(de)], espace);
    curseur(b, vec![Event::PointerButton { pos: de, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], espace);
    for k in 1..=6 {
        curseur(b, vec![Event::PointerMoved(pos2(x(vers * k as f32 / 6.0), y))], espace);
    }
    let fin = pos2(x(vers), y);
    curseur(b, vec![Event::PointerButton { pos: fin, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], espace);
    curseur(b, Vec::new(), espace);
}

#[test]
fn le_curseur_lisse_le_chemin_des_calques_choisis_en_une_etape_et_les_cles_suivent() {
    let mut b = banc();
    tracer(&mut b, &zigzag());
    let avant = b.app.layers()[0].trajectoire.clone().expect("un chemin");
    let cles_avant = b.app.layers()[0].pos_keys.clone();
    assert_eq!(avant.lissage, 0.0);
    let etapes = b.app.history.names().0.len();
    tirer_le_curseur(&mut b, 1.2, false);
    b.chauffer(2);
    let t = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!(t.lissage > 0.9, "le curseur est allé au bout : {}", t.lissage);
    assert_eq!(t.brut.as_deref(), Some(&avant.points[..]));
    assert!(ecart(&t.points[t.points.len() / 5..t.points.len() * 4 / 5]) < ecart(&avant.points) * 0.5);
    // Les clés ont suivi le chemin lissé ; la première et la dernière restent.
    let cles = &b.app.layers()[0].pos_keys;
    assert_eq!(cles.len(), cles_avant.len());
    assert!(cles.iter().zip(&cles_avant).any(|(a, c)| (a.t.y - c.t.y).abs() > 1.0), "les clés du milieu ont bougé");
    assert!((cles[0].t.x - cles_avant[0].t.x).abs() < 1e-3 && (cles.last().unwrap().t.y - cles_avant.last().unwrap().t.y).abs() < 1e-3);
    // Un geste sur le curseur : une étape d'annulation, qui rend le trait tel que tracé.
    let noms = b.app.history.names().0;
    assert_eq!(noms.len(), etapes + 1, "{noms:?}");
    assert_eq!(noms.last().map(|s| s.to_string()).as_deref(), Some("Lisser la trajectoire"));
    b.app.undo();
    assert_eq!(b.app.layers()[0].trajectoire.as_ref(), Some(&avant));
    assert_eq!(b.app.layers()[0].pos_keys, cles_avant);
    b.app.redo();
    // La valeur du curseur vaut aussi pour le prochain tracé.
    assert!(b.app.traj.lissage > 0.9);
    b.app.layers_mut()[0].trajectoire = None;
    tracer(&mut b, &zigzag());
    let neuve = b.app.layers()[0].trajectoire.clone().unwrap();
    assert!(neuve.lissage > 0.9 && neuve.brut.is_some(), "le nouveau tracé est lissé");
}

#[test]
fn le_curseur_agit_sur_tous_les_calques_choisis_et_sans_trajectoire_regle_le_prochain_trace() {
    let mut b = banc();
    // Rien à lisser : le curseur ne règle que les prochains tracés, sans étape d'annulation.
    let etapes = b.app.history.names().0.len();
    tirer_le_curseur(&mut b, 0.5, false);
    assert!((b.app.traj.lissage - 0.5).abs() < 0.08, "{}", b.app.traj.lissage);
    assert_eq!(b.app.history.names().0.len(), etapes);
    // Deux calques avec chacun son chemin, les deux choisis.
    b.app.traj.lissage = 0.0;
    b.app.layers_mut().push(Layer::new("Autre", LayerKind::Vector));
    for li in [0, 1] {
        b.app.cur_layer = li;
        tracer(&mut b, &zigzag());
    }
    b.app.cur_layer = 0;
    b.app.layer_sel = vec![b.app.layers()[1].id];
    assert_eq!(b.app.trajectoire_calques_lissables(), vec![0, 1]);
    tirer_le_curseur(&mut b, 0.8, false);
    for li in [0, 1] {
        let t = b.app.layers()[li].trajectoire.as_ref().unwrap();
        assert!((t.lissage - 0.8).abs() < 0.08, "calque {li} : {}", t.lissage);
    }
    b.app.undo();
    assert!([0, 1].iter().all(|&li| b.app.layers()[li].trajectoire.as_ref().unwrap().lissage == 0.0), "une seule étape pour les deux");
}

#[test]
fn dans_l_espace_le_lissage_agit_sur_les_cartes_choisies_et_la_camera() {
    let mut doc = Document::with_frame(320, 180, 24);
    doc.panels[0].layers = vec![Layer::new("Carte 0", LayerKind::Bitmap), Layer::new("Carte 1", LayerKind::Bitmap)];
    doc.panels[0].length = 24;
    let mut b = crate::perf_tests::banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.app.run_action("espace3d.plan_en_espace");
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    let world = b.app.e3_world();
    let sol = Support::nouveau([0.0, world.sol_y(), 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], world.k).unwrap();
    let mut t = Trajectoire::nouvelle_dans_l_espace(&zigzag(), sol).unwrap();
    (t.cles, t.fin) = (Cles::Interpolation, Some(23));
    for li in [0, 1] {
        b.app.layers_mut()[li].trajectoire = Some(t.clone());
        b.app.trajectoire_recalculer(li);
    }
    b.app.panel_mut().trajectoire_camera = Some(t.clone());
    let ids: Vec<u64> = b.app.layers().iter().map(|l| crate::espace3d::card_selection(l.id)).collect();
    b.app.espace3d.sel = ids.into_iter().chain([crate::espace3dgizmo::CAMERA_ID]).collect();
    b.chauffer(2);
    let sujets = b.app.traj_sujets_des_reglages();
    assert_eq!(sujets.len(), 3, "{sujets:?}");
    let fin_avant = b.app.e3_world().carte(&b.app.layers()[0], 23).pos;
    let milieu_avant = b.app.e3_world().carte(&b.app.layers()[0], 11).pos;
    tirer_le_curseur(&mut b, 1.2, true);
    for s in [Sujet::Carte(b.app.layers()[0].id), Sujet::Carte(b.app.layers()[1].id), Sujet::Camera] {
        let t = b.app.e3_traj_de(s).unwrap();
        assert!(t.lissage > 0.9 && t.brut.is_some(), "{s:?} : {}", t.lissage);
    }
    // La carte suit le chemin lissé : le bout ne bouge pas, le milieu si.
    let w = b.app.e3_world();
    let (fin, milieu) = (w.carte(&b.app.layers()[0], 23).pos, w.carte(&b.app.layers()[0], 11).pos);
    assert!(crate::espace3dmath::length(crate::espace3dmath::sub(fin, fin_avant)) < 1e-3, "{fin:?} {fin_avant:?}");
    assert!(crate::espace3dmath::length(crate::espace3dmath::sub(milieu, milieu_avant)) > 1e-3, "le milieu a suivi le lissage");
    b.app.undo();
    assert!(sujets.iter().all(|&s| b.app.e3_traj_de(s).unwrap().lissage == 0.0), "une étape pour les trois");
}
