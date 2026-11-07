//! Lissage de la caméra à la main (`lissage.rs`) : les poignées de lissage de la piste de caméra.
//!
//! Les essais de la logique sont sur des clés seules. Ceux du geste passent par la vraie fenêtre sans
//! écran (`Banc`) : on retrouve où les losanges et les ronds creux sont tombés à l'écran d'après les
//! formes peintes, puis on clique et on glisse avec le pointeur, dans la timeline et dans le montage.

use eframe::egui::{self, epaint::Shape, pos2, vec2, Color32, Event, Modifiers, PointerButton, Pos2};

use crate::camera::{self, Camera, CameraKey};
use crate::camkeys::{copy, delete, paste, paste_insert, shift, CamKeyRef};
use crate::document::Document;
use crate::lissage::{self, Adouci, Cote};
use crate::perf_tests::{banc_taille, Banc};

const DT: f64 = 1.0 / 60.0;

fn cam(x: f32) -> Camera {
    Camera { x, ..Camera::centered(64, 48) }
}

/// Des clés aux images `frames`, la caméra de chacune à x = 10 × son image.
fn cles(frames: &[usize]) -> Vec<CameraKey> {
    frames.iter().map(|&f| CameraKey::new(f, cam(f as f32 * 10.0))).collect()
}

/// Le x de la caméra à chaque image, de la première clé à la dernière : le mouvement tel qu'il se voit.
fn trajet(keys: &[CameraKey]) -> Vec<f32> {
    let fin = keys.last().map_or(0, |k| k.frame);
    (0..=fin).map(|f| camera::camera_at(keys, f, cam(0.0)).x).collect()
}

fn presque(a: &[f32], b: &[f32], ctx: &str) {
    assert_eq!(a.len(), b.len(), "{ctx}");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!((x - y).abs() < 1e-3, "{ctx} : image {i} : {x} contre {y}");
    }
}

fn poignee(keys: &[CameraKey], i: usize, c: Cote) -> Option<f32> {
    lissage::poignees(keys).into_iter().find(|&(k, cote, _)| k == i && cote == c).map(|(_, _, v)| v)
}

fn proche(a: Option<f32>, b: f32) -> bool {
    a.is_some_and(|a| (a - b).abs() < 1e-3)
}

// ------------------------------------------------------------------------------ la logique

#[test]
fn ce_qui_est_montre_est_ce_que_fait_ease() {
    // `shared_eases` rend ce que `ease` applique : mêmes avancements avec ou sans le partage.
    for (span, out, inn) in [(10.0_f32, 8.0, 8.0), (10.0, 3.0, 4.0), (10.0, 20.0, 5.0), (7.0, 0.0, 9.0), (1.0, 4.0, 4.0)] {
        let (a, b) = camera::shared_eases(span, out, inn);
        assert!(a + b <= span + 1e-4, "{span} {out} {inn} : {a} + {b}");
        for k in 0..=20 {
            let t = k as f32 / 20.0;
            let (x, y) = (camera::ease(t, span, out, inn), camera::ease(t, span, a, b));
            assert!((x - y).abs() < 1e-5, "{span} {out} {inn} t={t} : {x} contre {y}");
        }
    }
}

#[test]
fn une_poignee_n_existe_que_la_ou_il_y_a_un_mouvement_et_un_adouci() {
    let mut k = cles(&[0, 10, 30]);
    assert!(lissage::poignees(&k).is_empty(), "sans adouci, aucune poignée");
    k[1].ease_in = 4.0;
    k[1].ease_out = 6.0;
    assert_eq!(poignee(&k, 1, Cote::Entree), Some(4.0));
    assert_eq!(poignee(&k, 1, Cote::Sortie), Some(6.0));
    // Première clé : pas de poignée à gauche ; dernière : pas à droite (même avec une valeur gardée).
    k[0].ease_in = 5.0;
    k[2].ease_out = 5.0;
    assert_eq!(poignee(&k, 0, Cote::Entree), None);
    assert_eq!(poignee(&k, 2, Cote::Sortie), None);
    // Une seule clé : rien.
    let mut seule = cles(&[7]);
    seule[0].ease_in = 3.0;
    seule[0].ease_out = 3.0;
    seule[0].smooth = true;
    assert!(lissage::poignees(&seule).is_empty());
    // Ramener à 0 : la poignée disparaît.
    k[1].ease_out = 0.0;
    assert_eq!(poignee(&k, 1, Cote::Sortie), None);
}

#[test]
fn adoucir_pose_des_valeurs_reelles_en_demi_intervalle_puis_retire_tout() {
    let mut k = cles(&[0, 12, 30]);
    assert_eq!(lissage::adoucir(&mut k, 1), Adouci::Cree(6.0, 9.0));
    assert_eq!((k[1].ease_in, k[1].ease_out, k[1].smooth), (6.0, 9.0, false), "des valeurs réelles, pas de `smooth` nouveau");
    assert_eq!((poignee(&k, 1, Cote::Entree), poignee(&k, 1, Cote::Sortie)), (Some(6.0), Some(9.0)));
    // Même mouvement que l'ancien réglage.
    let mut ancien = cles(&[0, 12, 30]);
    ancien[1].smooth = true;
    presque(&trajet(&k), &trajet(&ancien), "demi-intervalle comme l'ancien réglage");
    // Sur une clé déjà adoucie : tout part.
    assert_eq!(lissage::adoucir(&mut k, 1), Adouci::Retire);
    assert_eq!((k[1].ease_in, k[1].ease_out, k[1].smooth), (0.0, 0.0, false));
    assert!(lissage::poignees(&k).is_empty());
    // Un côté seulement sur la première et la dernière clé.
    assert_eq!(lissage::adoucir(&mut k, 0), Adouci::Cree(0.0, 6.0));
    assert_eq!(lissage::adoucir(&mut k, 2), Adouci::Cree(9.0, 0.0));
    // Une seule clé : pas de mouvement.
    let mut seule = cles(&[5]);
    assert_eq!(lissage::adoucir(&mut seule, 0), Adouci::SansMouvement);
    // Une ancienne clé adoucie : le bouton retire aussi son `smooth`.
    let mut vieille = cles(&[0, 10, 20]);
    vieille[1].smooth = true;
    assert_eq!(lissage::adoucir(&mut vieille, 1), Adouci::Retire);
    assert!(!vieille[1].smooth);
}

#[test]
fn adoucir_ne_prend_pas_la_place_de_la_poignee_d_en_face() {
    let mut k = cles(&[0, 10, 20]);
    k[2].ease_in = 8.0; // la clé d'après a déjà pris 8 images sur 10
    assert_eq!(lissage::adoucir(&mut k, 1), Adouci::Cree(5.0, 2.0), "la sortie ne garde que les 2 images restantes");
    let somme = poignee(&k, 1, Cote::Sortie).unwrap() + poignee(&k, 2, Cote::Entree).unwrap();
    assert!(somme <= 10.0 + 1e-4, "ce qu'on voit tient dans l'intervalle : {somme}");
}

#[test]
fn une_poignee_ne_depasse_ni_la_cle_voisine_ni_la_poignee_d_en_face() {
    let mut k = cles(&[0, 10, 30]);
    k[2].ease_in = 6.0;
    // Sortie de la clé 10 : l'intervalle fait 20, la poignée d'en face en prend 6.
    assert_eq!(lissage::regler(&mut k, 1, Cote::Sortie, 100.0), 14.0);
    assert_eq!(k[1].ease_out, 14.0);
    assert_eq!(poignee(&k, 2, Cote::Entree), Some(6.0), "la poignée d'en face n'a pas bougé");
    // Dans l'autre sens, la poignée d'en face laisse 6 images sur 20 à l'entrée de la clé 30.
    assert_eq!(lissage::regler(&mut k, 2, Cote::Entree, 100.0), 6.0, "elle ne dépasse pas la poignée qui vient en face");
    // Sortie de la clé 0 : l'intervalle de 10 images, sans autre poignée.
    assert_eq!(lissage::regler(&mut k, 0, Cote::Sortie, 100.0), 10.0);
    // Arrondi à l'image, jamais négatif.
    assert_eq!(lissage::regler(&mut k, 0, Cote::Sortie, 3.4), 3.0);
    assert_eq!(lissage::regler(&mut k, 0, Cote::Sortie, -5.0), 0.0);
    assert_eq!(poignee(&k, 0, Cote::Sortie), None, "ramenée sur sa clé : la poignée disparaît");
}

#[test]
fn ramener_une_poignee_sur_sa_cle_rend_ce_cote_a_vitesse_constante() {
    let mut k = cles(&[0, 10, 20]);
    lissage::adoucir(&mut k, 1);
    lissage::regler(&mut k, 1, Cote::Sortie, 0.0);
    // Entre les clés 10 et 20, la caméra va en ligne droite : 10 de x par image.
    let t = trajet(&k);
    for f in 10..=20 {
        assert!((t[f] - (100.0 + 10.0 * (f - 10) as f32)).abs() < 1e-3, "image {f} : {}", t[f]);
    }
    assert_eq!(poignee(&k, 1, Cote::Sortie), None);
    assert!(poignee(&k, 1, Cote::Entree).is_some(), "l'autre côté garde son lissage");
}

#[test]
fn la_conversion_d_une_ancienne_cle_ne_change_aucune_image() {
    for voisines in [false, true] {
        let mut k = cles(&[0, 8, 20, 26]);
        k[1].smooth = true;
        if voisines {
            k[0].ease_out = 7.0; // la clé d'avant prend 7 images sur 8 : partage avec l'entrée de la clé 8
            k[2].ease_in = 9.0;
            k[2].ease_out = 2.0;
        }
        let avant = trajet(&k);
        let mut c = k.clone();
        lissage::convertir(&mut c, 1);
        assert!(!c[1].smooth && c[1].ease_in > 0.0 && c[1].ease_out > 0.0, "valeurs explicites : {:?}", c[1]);
        presque(&trajet(&c), &avant, &format!("convertir, voisines={voisines}"));
        // Les poignées montrent les mêmes valeurs avant et après.
        for cote in [Cote::Entree, Cote::Sortie] {
            let (a, b) = (poignee(&k, 1, cote), poignee(&c, 1, cote));
            assert!(a.is_some() && proche(b, a.unwrap()), "voisines={voisines} {cote:?} : {a:?} {b:?}");
        }
    }
}

#[test]
fn regler_un_cote_d_une_ancienne_cle_ne_change_pas_le_mouvement_de_l_autre() {
    let mut k = cles(&[0, 10, 30]);
    k[1].smooth = true; // entrée 5, sortie 10
    let avant = trajet(&k);
    let mut c = k.clone();
    assert_eq!(lissage::regler(&mut c, 1, Cote::Entree, 2.0), 2.0);
    assert!(!c[1].smooth);
    assert_eq!(c[1].ease_out, 10.0, "l'autre côté devient explicite, avec sa valeur d'avant");
    let apres = trajet(&c);
    // Entre 10 et 30, rien n'a bougé ; entre 0 et 10 seulement.
    presque(&apres[10..], &avant[10..], "le côté non touché");
    assert!(apres[..10].iter().zip(&avant[..10]).any(|(a, b)| (a - b).abs() > 1e-3), "le côté touché a changé");
}

#[test]
fn deux_adoucis_trop_longs_sont_montres_partages_et_la_poignee_d_en_face_les_prend() {
    let mut k = cles(&[0, 10, 20]);
    k[0].ease_out = 8.0;
    k[1].ease_in = 8.0; // 16 sur 10 : `ease` les partage en proportion, 5 et 5
    assert!(proche(poignee(&k, 0, Cote::Sortie), 5.0));
    assert!(proche(poignee(&k, 1, Cote::Entree), 5.0), "ce qu'on voit est ce qui se passe");
    let avant = trajet(&k);
    // Tirer l'une écrit l'autre en valeur effective : elle ne bouge pas.
    assert_eq!(lissage::regler(&mut k, 1, Cote::Entree, 5.0), 5.0);
    assert_eq!(k[0].ease_out, 5.0);
    presque(&trajet(&k), &avant, "même mouvement");
    assert_eq!(lissage::regler(&mut k, 1, Cote::Entree, 9.0), 5.0, "au plus ce qu'il reste");
}

#[test]
fn une_cle_deplacee_garde_ses_adoucis_et_la_poignee_dit_la_verite() {
    let mut k = cles(&[0, 10, 30]);
    lissage::regler(&mut k, 1, Cote::Sortie, 8.0);
    lissage::regler(&mut k, 2, Cote::Entree, 8.0);
    // L'intervalle 10 → 30 raccourcit à 12 : 16 images d'adoucis y sont partagées.
    shift(&mut k, &[30], -8);
    assert_eq!((k[1].ease_out, k[2].ease_in), (8.0, 8.0), "les données ne sont pas détruites");
    let vus = (poignee(&k, 1, Cote::Sortie).unwrap(), poignee(&k, 2, Cote::Entree).unwrap());
    assert!((vus.0 + vus.1 - 12.0).abs() < 1e-3, "la poignée montre la valeur effective : {vus:?}");
    // Ramenée à la longueur d'avant, la poignée reprend ses 8 images.
    shift(&mut k, &[22], 8);
    assert_eq!((poignee(&k, 1, Cote::Sortie), poignee(&k, 2, Cote::Entree)), (Some(8.0), Some(8.0)));
}

#[test]
fn retirer_une_cle_laisse_aux_voisines_leurs_valeurs() {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].set_length(40);
    let mut k = cles(&[0, 10, 20, 30]);
    lissage::adoucir(&mut k, 1);
    lissage::adoucir(&mut k, 2);
    doc.panels[0].camera = k.clone();
    let id = doc.panels[0].id;
    assert_eq!(delete(&mut doc, &[CamKeyRef { panel: id, frame: 10 }]), 1);
    let rest = &doc.panels[0].camera;
    assert_eq!(rest.len(), 3);
    assert_eq!((rest[1].frame, rest[1].ease_in, rest[1].ease_out), (20, 5.0, 5.0), "la clé 20 garde ses valeurs");
    // Les poignées sont celles de ce qui se passe maintenant (intervalle 0 → 20).
    assert_eq!((poignee(rest, 1, Cote::Entree), poignee(rest, 1, Cote::Sortie)), (Some(5.0), Some(5.0)));
}

#[test]
fn copier_et_coller_emportent_les_adoucis_avec_la_cle() {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].set_length(60);
    let mut k = cles(&[0, 10, 20]);
    lissage::adoucir(&mut k, 1);
    doc.panels[0].camera = k;
    let id = doc.panels[0].id;
    let clip = copy(&doc, &[CamKeyRef { panel: id, frame: 10 }]).unwrap();
    // Collée sur la tête : une clé de plus, mêmes adoucis, ses poignées montrées.
    let r = paste(&mut doc, &clip, 40, None);
    assert_eq!(r.placed.len(), 1);
    let keys = &doc.panels[0].camera;
    let neuve = keys.iter().find(|k| k.frame == 40).unwrap();
    assert_eq!((neuve.ease_in, neuve.ease_out, neuve.smooth), (5.0, 5.0, false));
    let i = keys.iter().position(|k| k.frame == 40).unwrap();
    assert!(poignee(keys, i, Cote::Entree).is_some(), "la clé collée a sa poignée d'entrée");
    assert_eq!(poignee(keys, i, Cote::Sortie), None, "dernière clé : rien à droite");
    // Insérée après la clé 10.
    doc.panels[0].camera = {
        let mut k = cles(&[0, 10, 20]);
        lissage::adoucir(&mut k, 1);
        k
    };
    let ok = paste_insert(&mut doc, &clip, 10, None).unwrap();
    assert_eq!(ok.pasted.placed.len(), 1);
    let ins = doc.panels[0].camera.iter().find(|k| k.frame == 11).unwrap();
    assert_eq!((ins.ease_in, ins.ease_out), (5.0, 5.0));
}

#[test]
fn la_priorite_est_a_la_cle_et_la_poignee_a_une_prise_plus_large_que_son_dessin() {
    let mut k = cles(&[0, 10, 20]);
    lissage::regler(&mut k, 1, Cote::Sortie, 1.0);
    let cell = 4.0;
    let x_of = |f: usize| 100.0 + (f as f32 + 0.5) * cell;
    let xk = x_of(10);
    let tol = cell * 0.5 + 2.0;
    // Sur la clé, ou à moins de sa prise : la clé, même si la poignée (à une image) est là aussi.
    assert_eq!(lissage::sous_pointeur(&k, xk, &x_of, cell, tol), None);
    assert_eq!(lissage::sous_pointeur(&k, xk + 3.0, &x_of, cell, tol), None, "les deux se recouvrent : la clé passe avant");
    // Une poignée séparée de sa clé s'attrape, avec une prise plus large que son rayon de 3.
    let mut k2 = cles(&[0, 10, 20]);
    lissage::regler(&mut k2, 1, Cote::Sortie, 4.0);
    let xp2 = lissage::x_poignee(x_of(10), Cote::Sortie, 4.0, cell);
    assert_eq!(lissage::sous_pointeur(&k2, xp2, &x_of, cell, tol), Some((1, Cote::Sortie)));
    assert_eq!(lissage::sous_pointeur(&k2, xp2 + 6.0, &x_of, cell, tol), Some((1, Cote::Sortie)), "prise de 7 points, dessin de 3");
    assert_eq!(lissage::sous_pointeur(&k2, xp2 + 9.0, &x_of, cell, tol), None);
}

#[test]
fn l_enregistrement_garde_les_adoucis_et_l_ancien_reglage() {
    let mut doc = Document::new(64, 48, 24);
    doc.panels[0].set_length(40);
    let mut k = cles(&[0, 10, 20, 30]);
    lissage::adoucir(&mut k, 1);
    lissage::regler(&mut k, 2, Cote::Sortie, 3.0);
    k[2].ease_in = 4.0;
    k[3].smooth = true;
    doc.panels[0].camera = k.clone();
    let dir = std::env::temp_dir().join(format!("shot-lissage-cam-save-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("L.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let lu = crate::project::load(&manifest).unwrap().doc;
    let relu = &lu.panels[0].camera;
    assert_eq!(relu.len(), k.len());
    for (a, b) in relu.iter().zip(&k) {
        assert_eq!((a.frame, a.ease_in, a.ease_out, a.smooth), (b.frame, b.ease_in, b.ease_out, b.smooth));
    }
    presque(&trajet(relu), &trajet(&k), "le même mouvement après l'aller-retour");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn les_champs_adoucir_l_entree_et_la_sortie_suivent_les_memes_regles() {
    let mut k = cles(&[0, 10, 20]);
    k[2].ease_in = 6.0;
    // Les valeurs du champ sont celles des poignées.
    lissage::adoucir(&mut k, 1);
    let (e, o) = lissage::valeurs(&k, 1);
    assert_eq!((Some(e), Some(o)), (poignee(&k, 1, Cote::Entree), poignee(&k, 1, Cote::Sortie)));
    // Un chiffre tapé trop grand est ramené à ce qui tient.
    assert_eq!(lissage::regler(&mut k, 1, Cote::Sortie, 50.0), 4.0);
    // Une ancienne clé : taper un côté ne perd pas l'autre (avant : l'autre tombait à 0).
    let mut a = cles(&[0, 10, 20]);
    a[1].smooth = true;
    lissage::regler(&mut a, 1, Cote::Entree, 8.0);
    assert_eq!((a[1].ease_in, a[1].ease_out, a[1].smooth), (8.0, 5.0, false));
}

// ------------------------------------------------------------------------------ le banc

fn bouton(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Un plan de 40 images, des clés de caméra aux images 0, 12 et 30.
fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].set_length(40);
    doc.panels[0].camera = cles(&[0, 12, 30]);
    doc
}

/// La timeline, à 24 points par image.
fn banc_timeline() -> Banc {
    let mut b = banc_taille(film(), false, 1920, 1080);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 24.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn chercher(shape: &Shape, out: &mut Vec<(Pos2, bool)>, couleur: Color32, fond: Color32) {
    match shape {
        Shape::Path(p) if p.fill == couleur && p.points.len() == 4 => out.push((egui::Rect::from_points(&p.points).center(), true)),
        Shape::Circle(c) if c.fill == fond && ((c.radius - 3.0).abs() < 0.01 || (c.radius - 4.5).abs() < 0.01) => out.push((c.center, false)),
        Shape::Vec(v) => v.iter().for_each(|s| chercher(s, out, couleur, fond)),
        _ => {}
    }
}

/// Les centres des losanges de clés de caméra et des ronds creux des poignées peints par la dernière image,
/// de gauche à droite.
fn vu(b: &Banc) -> (Vec<Pos2>, Vec<Pos2>) {
    let mut tout = Vec::new();
    for s in &b.formes {
        chercher(&s.shape, &mut tout, crate::theme::camera(), crate::theme::rangee_sombre());
    }
    let mut cles: Vec<Pos2> = tout.iter().filter(|(_, d)| *d).map(|(p, _)| *p).collect();
    let mut poignees: Vec<Pos2> = tout.iter().filter(|(_, d)| !*d).map(|(p, _)| *p).collect();
    cles.sort_by(|a, b| a.x.total_cmp(&b.x));
    poignees.sort_by(|a, b| a.x.total_cmp(&b.x));
    (cles, poignees)
}

fn glisser(b: &mut Banc, de: Pos2, vers: Pos2) {
    b.image(vec![Event::PointerMoved(de)], DT);
    b.image(vec![bouton(de, true)], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(de + (vers - de) * (k as f32 / 6.0))], DT);
    }
    b.image(vec![bouton(vers, false)], DT);
    b.chauffer(3);
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, true)], DT);
    b.image(vec![bouton(p, false)], DT);
    b.chauffer(3);
}

fn cles_du_plan(b: &Banc) -> Vec<CameraKey> {
    b.app.doc.panels[0].camera.clone()
}

/// Adoucit la clé de l'image 12 par le bouton (l'action `camera.cle_adoucir`).
fn adoucir_12(b: &mut Banc) {
    b.app.cur_frame = 12;
    b.app.run_action("camera.cle_adoucir");
    b.chauffer(3);
}

fn a_une_etape_nommee(b: &Banc, nom: &str) -> bool {
    b.app.history.names().0.iter().any(|n| *n == nom)
}

#[test]
fn le_bouton_adoucir_fait_apparaitre_les_poignees_et_les_retire() {
    let mut b = banc_timeline();
    let (cles0, poignees0) = vu(&b);
    assert_eq!(cles0.len(), 3, "trois losanges : {cles0:?}");
    assert!(poignees0.is_empty(), "sans lissage, pas de poignée : {poignees0:?}");
    let cell = (cles0[1].x - cles0[0].x) / 12.0;
    assert!((cell - 24.0).abs() < 0.6, "24 points par image : {cell}");
    let etapes = b.app.history.undo_len();
    adoucir_12(&mut b);
    let k = cles_du_plan(&b);
    assert_eq!((k[1].ease_in, k[1].ease_out, k[1].smooth), (6.0, 9.0, false));
    let (cles1, poignees1) = vu(&b);
    assert_eq!(cles1.len(), 3, "les poignées ne sont pas des clés");
    assert_eq!(poignees1.len(), 2, "une à gauche, une à droite : {poignees1:?}");
    assert!((poignees1[0].x - (cles1[1].x - 6.0 * cell)).abs() < 0.6, "à gauche, 6 images avant la clé");
    assert!((poignees1[1].x - (cles1[1].x + 9.0 * cell)).abs() < 0.6, "à droite, 9 images après la clé");
    assert!((poignees1[0].y - cles1[1].y).abs() < 0.6 && (poignees1[1].y - cles1[1].y).abs() < 0.6, "sur la piste de caméra");
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    // Une seconde fois : tout est retiré.
    adoucir_12(&mut b);
    let k = cles_du_plan(&b);
    assert_eq!((k[1].ease_in, k[1].ease_out), (0.0, 0.0));
    assert!(vu(&b).1.is_empty());
}

#[test]
fn la_premiere_et_la_derniere_cle_n_ont_qu_une_poignee_et_une_cle_seule_aucune() {
    let mut b = banc_timeline();
    b.app.cur_frame = 0;
    b.app.run_action("camera.cle_adoucir");
    b.app.cur_frame = 30;
    b.app.run_action("camera.cle_adoucir");
    b.chauffer(3);
    let (cles, poignees) = vu(&b);
    let cell = (cles[1].x - cles[0].x) / 12.0;
    assert_eq!(poignees.len(), 2, "{poignees:?}");
    assert!((poignees[0].x - (cles[0].x + 6.0 * cell)).abs() < 0.6, "première clé : seulement à droite");
    assert!((poignees[1].x - (cles[2].x - 9.0 * cell)).abs() < 0.6, "dernière clé : seulement à gauche");
    // Plan à une seule clé : le bouton ne fait rien, et la barre d'état le dit.
    b.app.doc.panels[0].camera = cles_seules();
    b.app.cur_frame = 5;
    b.app.run_action("camera.cle_adoucir");
    b.chauffer(3);
    assert!(vu(&b).1.is_empty());
    assert!(b.app.status.contains("seule"), "{}", b.app.status);
    assert_eq!(b.app.doc.panels[0].camera[0].ease_out, 0.0);
}

fn cles_seules() -> Vec<CameraKey> {
    cles(&[5])
}

#[test]
fn tirer_la_poignee_de_droite_regle_la_sortie_en_une_etape() {
    let mut b = banc_timeline();
    adoucir_12(&mut b);
    let avant = trajet(&cles_du_plan(&b));
    let (cles, poignees) = vu(&b);
    let cell = 24.0;
    let etapes = b.app.history.undo_len();
    // De 9 images à 4.
    glisser(&mut b, poignees[1], pos2(cles[1].x + 4.0 * cell, cles[1].y));
    let k = cles_du_plan(&b);
    assert_eq!((k[1].ease_out, k[1].ease_in), (4.0, 6.0), "la sortie change, pas l'entrée");
    assert!(!k[1].smooth);
    assert_ne!(trajet(&k), avant, "le mouvement a changé");
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une seule étape d'annulation");
    assert!(a_une_etape_nommee(&b, "Régler le lissage de la caméra"), "{:?}", b.app.history.names());
    assert!(b.app.status.contains("4 images"), "la barre d'état donne la durée : {}", b.app.status);
    assert!(b.app.cam_key_sel.is_empty() && b.app.cam_keys_drag.is_none() && b.app.ease_drag.is_none(), "ni choix de clé ni glissé de clé");
    assert_eq!(b.app.cur_frame, 12, "la tête de lecture n'a pas bougé");
    assert_eq!(cles_du_plan(&b).iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12, 30], "aucune clé n'a bougé");
    // Annuler rend tout.
    b.app.run_action("edition.annuler");
    b.chauffer(3);
    assert_eq!(trajet(&cles_du_plan(&b)), avant);
}

#[test]
fn le_mouvement_se_voit_pendant_le_glisse_pas_seulement_au_lache() {
    let mut b = banc_timeline();
    adoucir_12(&mut b);
    let (_, poignees) = vu(&b);
    b.app.cur_frame = 14;
    let avant = b.app.doc.panel_camera(0, 14).x;
    b.image(vec![Event::PointerMoved(poignees[1])], DT);
    b.image(vec![bouton(poignees[1], true)], DT);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(poignees[1] - vec2(k as f32 * 20.0, 0.0))], DT);
    }
    assert!(b.app.ease_drag.is_some(), "le geste est en cours");
    let pendant = b.app.doc.panel_camera(0, 14).x;
    assert!((pendant - avant).abs() > 0.01, "la caméra de l'image 14 a déjà changé avant le lâcher : {avant} -> {pendant}");
    // Échap : tout revient comme avant, rien n'est enregistré.
    let etapes = b.app.history.undo_len();
    b.app.cancel_drags();
    assert!(b.app.ease_drag.is_none());
    assert_eq!(b.app.doc.panel_camera(0, 14).x, avant);
    assert_eq!(b.app.history.undo_len(), etapes);
    b.image(vec![bouton(poignees[1], false)], DT);
}

#[test]
fn tirer_la_poignee_de_gauche_et_les_limites() {
    let mut b = banc_timeline();
    adoucir_12(&mut b);
    // La clé 30 a pris 6 images d'entrée : la sortie de la clé 12 ne peut pas dépasser 18 - 6 = 12.
    b.app.doc.panels[0].camera[2].ease_in = 6.0;
    b.chauffer(3);
    let (cles, poignees) = vu(&b);
    let cell = 24.0;
    assert_eq!(poignees.len(), 3, "{poignees:?}");
    // Poignée de gauche de la clé 12 (6 images) : de 6 à 2.
    glisser(&mut b, poignees[0], pos2(cles[1].x - 2.0 * cell, cles[1].y));
    let k = cles_du_plan(&b);
    assert_eq!((k[1].ease_in, k[1].ease_out), (2.0, 9.0));
    // Tirée bien plus loin que la clé voisine : elle s'arrête à la clé d'avant (12 images).
    let (cles, poignees) = vu(&b);
    glisser(&mut b, poignees[0], pos2(cles[1].x - 14.0 * cell, cles[1].y));
    assert_eq!(cles_du_plan(&b)[1].ease_in, 12.0, "pas plus que l'intervalle");
    // La sortie de la clé 12 (poignée à 9 images) tirée trop loin à droite : 18 − 6 = 12 images au plus.
    let (cles, poignees) = vu(&b);
    let droite = poignees.iter().copied().find(|p| p.x > cles[1].x && p.x < cles[2].x - 7.0 * cell).expect("poignée de sortie");
    glisser(&mut b, droite, pos2(cles[1].x + 40.0 * cell, cles[1].y));
    assert_eq!(cles_du_plan(&b)[1].ease_out, 12.0, "elle ne dépasse pas la poignée d'en face");
    assert!(b.app.status.contains("12 images"), "{}", b.app.status);
    assert_eq!(cles_du_plan(&b).iter().map(|k| k.frame).collect::<Vec<_>>(), vec![0, 12, 30]);
    // Ramenée sur sa clé : ce côté est à 0 et la poignée disparaît.
    let (cles, poignees) = vu(&b);
    let droite = poignees.iter().copied().find(|p| p.x > cles[1].x).unwrap();
    glisser(&mut b, droite, pos2(cles[1].x - 5.0, cles[1].y));
    assert_eq!(cles_du_plan(&b)[1].ease_out, 0.0);
    let (cles, poignees) = vu(&b);
    assert_eq!(poignees.iter().filter(|p| p.x > cles[1].x + 1.0).count(), 1, "il ne reste à droite que l'entrée de la clé 30 : {poignees:?}");
}

#[test]
fn une_ancienne_cle_smooth_montre_ses_poignees_et_le_premier_glisse_la_convertit() {
    let mut b = banc_timeline();
    b.app.doc.panels[0].camera[1].smooth = true;
    b.chauffer(3);
    let avant = cles_du_plan(&b);
    let (cles, poignees) = vu(&b);
    assert_eq!(poignees.len(), 2, "les valeurs effectives : {poignees:?}");
    let cell = 24.0;
    assert!((poignees[0].x - (cles[1].x - 6.0 * cell)).abs() < 0.6 && (poignees[1].x - (cles[1].x + 9.0 * cell)).abs() < 0.6);
    // Le glisser de la poignée de gauche convertit les deux côtés.
    glisser(&mut b, poignees[0], pos2(cles[1].x - 3.0 * cell, cles[1].y));
    let k = cles_du_plan(&b);
    assert!(!k[1].smooth);
    assert_eq!((k[1].ease_in, k[1].ease_out), (3.0, 9.0), "le côté non touché garde sa valeur effective");
    // Le côté non touché (de 12 à 30) a exactement le même mouvement.
    presque(&trajet(&k)[12..], &trajet(&avant)[12..], "côté droit inchangé");
}

#[test]
fn une_cle_deplacee_emmene_ses_poignees() {
    let mut b = banc_timeline();
    adoucir_12(&mut b);
    let (cles, _) = vu(&b);
    // Glisser le losange de 12 à 16 (Maj tenue : seule la clé, les autres restent).
    b.touches = Modifiers::SHIFT;
    glisser(&mut b, cles[1], cles[1] + vec2(4.0 * 24.0, 0.0));
    b.touches = Modifiers::NONE;
    let k = cles_du_plan(&b);
    assert_eq!(k.iter().map(|c| c.frame).collect::<Vec<_>>(), vec![0, 16, 30]);
    assert_eq!((k[1].ease_in, k[1].ease_out), (6.0, 9.0), "durées conservées");
    let (cles, poignees) = vu(&b);
    assert_eq!(poignees.len(), 2);
    assert!((poignees[0].x - (cles[1].x - 6.0 * 24.0)).abs() < 0.6 && (poignees[1].x - (cles[1].x + 9.0 * 24.0)).abs() < 0.6);
}

#[test]
fn la_cle_passe_avant_la_poignee_quand_elles_se_recouvrent() {
    let mut b = banc_timeline();
    b.app.timeline_cell = crate::timeline::CELL_MIN;
    b.chauffer(4);
    b.app.doc.panels[0].camera[1].ease_out = 1.0;
    b.chauffer(3);
    let (cles, poignees) = vu(&b);
    assert_eq!(poignees.len(), 1, "{poignees:?}");
    // Appui sur la clé : la clé est prise, pas la poignée (même si la poignée est à une image).
    b.image(vec![Event::PointerMoved(cles[1])], DT);
    b.image(vec![bouton(cles[1], true)], DT);
    b.image(vec![Event::PointerMoved(cles[1] + vec2(30.0, 0.0))], DT);
    assert!(b.app.ease_drag.is_none(), "pas de poignée");
    assert!(b.app.cam_keys_drag.is_some(), "la clé est prise");
    b.image(vec![bouton(cles[1] + vec2(30.0, 0.0), false)], DT);
}

#[test]
fn un_clic_sur_une_poignee_ne_choisit_rien_et_ne_deplace_pas_la_tete() {
    let mut b = banc_timeline();
    adoucir_12(&mut b);
    let (cles, poignees) = vu(&b);
    clic(&mut b, cles[2]);
    let choisie = b.app.cam_key_sel.clone();
    assert_eq!(choisie.len(), 1);
    let (frame, etapes) = (b.app.cur_frame, b.app.history.undo_len());
    clic(&mut b, poignees[1]);
    assert_eq!(b.app.cam_key_sel, choisie, "le choix de clés n'est pas touché");
    assert_eq!(b.app.cur_frame, frame, "la tête de lecture reste");
    assert_eq!(b.app.history.undo_len(), etapes, "rien n'est enregistré");
}

// ------------------------------------------------------------------------------ le montage

fn banc_montage() -> Banc {
    use crate::workspace::Espace;
    let mut doc = film();
    doc.add_scene_after(0);
    doc.panels[1].set_length(20);
    let mut b = banc_taille(doc, false, 1920, 1080);
    b.app.set_espace(Espace::Montage);
    b.app.montage_zoom = 12.0;
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

#[test]
fn le_montage_montre_et_tire_les_memes_poignees() {
    let mut b = banc_montage();
    b.app.doc.panels[0].camera[1].ease_in = 4.0;
    b.app.doc.panels[0].camera[1].ease_out = 6.0;
    b.chauffer(3);
    let (cles, poignees) = vu(&b);
    assert_eq!(cles.len(), 3, "{cles:?}");
    assert_eq!(poignees.len(), 2, "{poignees:?}");
    let ppf = (cles[1].x - cles[0].x) / 12.0;
    assert!((poignees[1].x - (cles[1].x + 6.0 * ppf)).abs() < 0.6);
    let etapes = b.app.history.undo_len();
    glisser(&mut b, poignees[1], pos2(cles[1].x + 2.0 * ppf, cles[1].y));
    let k = cles_du_plan(&b);
    assert_eq!((k[1].ease_in, k[1].ease_out), (4.0, 2.0));
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert!(a_une_etape_nommee(&b, "Régler le lissage de la caméra"));
    assert!(b.app.cam_key_sel.is_empty());
    // La poignée de gauche, dans le montage aussi.
    let (cles, poignees) = vu(&b);
    glisser(&mut b, poignees[0], pos2(cles[1].x - 1.0 * ppf, cles[1].y));
    assert_eq!(cles_du_plan(&b)[1].ease_in, 1.0);
}

// ------------------------------------------------------------------------------ la capture

/// Pour voir la piste de caméra et ses poignées (hors de la suite) :
/// `FEANOR_CAPTURE_DIR=… cargo test --release --offline --locked --target-dir target\msvc --lib lissage_capture -- --ignored --nocapture`
#[test]
#[ignore]
fn lissage_capture() {
    let dir = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).unwrap();
    let sauver = |b: &Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("LISSAGE pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dir.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("LISSAGE {nom}");
    };
    let mut doc = film();
    doc.panels[0].camera = {
        let mut k = cles(&[0, 12, 30, 38]);
        lissage::adoucir(&mut k, 1);
        lissage::adoucir(&mut k, 2);
        k
    };
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_mode(crate::app::Mode::Animation);
    b.app.timeline_cell = 30.0;
    b.app.cur_frame = 14;
    b.garder_formes = true;
    b.chauffer(10);
    sauver(&b, "lissage-1-poignees.png");
    // Une poignée tenue, éclairée, et la barre d'état.
    let (cles, poignees) = vu(&b);
    let p = poignees.iter().copied().find(|p| p.x > cles[1].x).unwrap();
    b.image(vec![Event::PointerMoved(p)], DT);
    b.image(vec![bouton(p, true)], DT);
    for k in 1..=3 {
        b.image(vec![Event::PointerMoved(p - vec2(k as f32 * 20.0, 0.0))], DT);
    }
    b.chauffer(2);
    sauver(&b, "lissage-2-poignee-tenue.png");
    b.image(vec![bouton(p - vec2(60.0, 0.0), false)], DT);
    b.chauffer(3);
    // À faible zoom, avec les clés qui se recouvrent presque.
    b.app.timeline_cell = crate::timeline::CELL_MIN;
    b.chauffer(6);
    sauver(&b, "lissage-3-faible-zoom.png");
}
