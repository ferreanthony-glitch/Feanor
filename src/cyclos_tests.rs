//! Essais des cyclos (partie 202, `cyclos.rs`, `cyclosui.rs`, `docs/INTERFACE.md` partie 202) : les choix, leur image (faite une
//! fois, gardée), le cyclo derrière les cartes et le décor dans l'image du plan (le processeur, l'export, la carte graphique), sa
//! lumière sur les cartes (Jour clair les laisse, la Nuit les assombrit, une lampe les éclaire), la case des cartes, un décor sans
//! objet, l'enregistrement (et un lecteur d'avant), la copie et la duplication, l'annulation, la galerie et la mesure.

use std::sync::Arc;

use eframe::egui::{pos2, Color32, Modifiers};

use crate::bulles::vus;
use crate::cyclos::{self, Cyclo, Preset};
use crate::decor3d::{self, Decor3d};
use crate::decor3drendu::Aspect;
use crate::dock::Pane;
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------------------ les aides

/// L'image du cyclo `c`, prête (faite tout de suite hors du fil de l'interface, attendue sinon).
fn pret(c: &Cyclo) -> Arc<cyclos::Equirect> {
    cyclos::assurer(c);
    cyclos::attendre();
    cyclos::texture(c.cle()).expect("l'image du cyclo")
}

/// Le cyclo `c` (rien : aucun) dans la séquence du plan 0, comme le choisit l'interface (`cyclosui::poser`), son image prête.
fn avec_cyclo(mut doc: Document, c: Option<Cyclo>) -> Document {
    let seq = doc.sequence_of(0);
    let d = doc.decors3d.par_sequence.entry(seq).or_insert_with(|| Decor3d::new(Aspect::default()));
    crate::cyclosui::poser(d, c);
    if let Some(c) = c {
        pret(&c);
    }
    doc
}

/// Le projet aux deux carrés (`decor3d_tests::projet_a_deux_carres`), le ciel du décor allumé (son aspect d'origine), une carte
/// peinte d'un bloc ocre au milieu de la feuille (« Prend la lumière du cyclo » : `prend`), et ces lampes.
fn carte(prend: bool, lampes: &[crate::miseenscene3d::Lampe3d]) -> Document {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let seq = doc.sequence_of(0);
    {
        let d = doc.decors3d.par_sequence.get_mut(&seq).unwrap();
        d.aspect.ciel = true;
        d.lampes = lampes.to_vec();
    }
    let cel = doc.new_cel(LayerKind::Bitmap);
    let (sw, sh) = (doc.width, doc.height);
    let mut rgba = vec![0u8; sw * sh * 4];
    for y in sh / 2 - 8..sh / 2 + 8 {
        for x in sw / 2 - 8..sw / 2 + 8 {
            rgba[(y * sw + x) * 4..(y * sw + x) * 4 + 4].copy_from_slice(&[180, 150, 90, 255]);
        }
    }
    doc.cels.get_mut(&cel).unwrap().raster = crate::raster::Raster::from_rgba(sw, sh, &rgba);
    let n = doc.panels[0].frame_count();
    let mut l = Layer::new("Carte", LayerKind::Bitmap);
    l.frames = vec![Some(cel); n];
    l.prend_cyclo = prend;
    doc.panels[0].layers = vec![l];
    doc
}

fn lampe(position: [f32; 3]) -> crate::miseenscene3d::Lampe3d {
    let mut l = crate::miseenscene3d::Lampe3d::neuve(9_100, "Lampe".into(), position);
    l.intensite = 1.5;
    l.portee = 3.0;
    l
}

fn exporter(doc: &Document) -> Vec<Color32> {
    crate::decor3d_tests::image_exportee(doc)
}

/// Le pixel au milieu de l'image (la carte), en haut à gauche (le ciel), au milieu du carré de gauche (le décor).
fn trois(img: &[Color32], doc: &Document) -> (Color32, Color32, Color32) {
    let [w, h] = doc.frame;
    (img[(h / 2) * w + w / 2], img[3 * w + 3], img[(h / 2) * w + w / 4])
}

fn ecart(a: Color32, b: Color32) -> i32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).abs()).max().unwrap()
}

fn somme(c: Color32) -> i32 {
    c.r() as i32 + c.g() as i32 + c.b() as i32
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(2);
}

/// La couleur moyenne (sRGB, 0 à 255) d'une bande d'élévation de l'image (`e0` à `e1`, en degrés).
fn bande(img: &cyclos::Equirect, e0: f32, e1: f32) -> [f32; 3] {
    let (y0, y1) = (((90.0 - e1) / 180.0 * img.hauteur as f32) as usize, ((90.0 - e0) / 180.0 * img.hauteur as f32) as usize);
    let mut s = [0.0f64; 3];
    let mut n = 0.0;
    for y in y0..y1.max(y0 + 1) {
        for x in 0..img.largeur {
            let i = (y * img.largeur + x) * 4;
            for k in 0..3 {
                s[k] += img.rgba[i + k] as f64;
            }
            n += 1.0;
        }
    }
    s.map(|v| (v / n) as f32)
}

// ------------------------------------------------------------------------------------------------ 1. les choix

#[test]
fn every_preset_is_deterministic_per_seed_and_the_presets_look_different() {
    let (w, h) = (256, 128);
    let mut signes: Vec<(Preset, [f32; 9])> = Vec::new();
    for p in Preset::TOUS {
        let c = p.cyclo();
        let a = cyclos::generer(&c, w, h);
        assert!(a == cyclos::generer(&c, w, h), "{} : la même image, à la même graine", p.nom());
        assert_eq!(a.rgba.len(), w * h * 4);
        assert!(a.rgba.chunks(4).all(|px| px[3] == 255), "{} : opaque", p.nom());
        if !matches!(p, Preset::Studio) {
            let autre = cyclos::generer(&Cyclo { graine: 7, ..c }, w, h);
            assert!(autre != a, "{} : une autre graine, une autre image", p.nom());
        }
        let (haut, horizon, bas) = (bande(&a, 40.0, 90.0), bande(&a, -2.0, 6.0), bande(&a, -60.0, -10.0));
        signes.push((p, [haut[0], haut[1], haut[2], horizon[0], horizon[1], horizon[2], bas[0], bas[1], bas[2]]));
    }
    // Neuf allures : deux choix ne se ressemblent pas (le haut du ciel, la bande de l'horizon, le sol au loin).
    for (i, (p, a)) in signes.iter().enumerate() {
        for (q, b) in &signes[i + 1..] {
            let d = (0..9).map(|k| (a[k] - b[k]).abs()).fold(0.0, f32::max);
            assert!(d > 8.0, "{} et {} se ressemblent trop : {a:?} / {b:?}", p.nom(), q.nom());
        }
    }
    let s = |p: Preset| signes.iter().find(|x| x.0 == p).unwrap().1;
    let lum = |v: [f32; 9], k: usize| v[k] + v[k + 1] + v[k + 2];
    // Le Jour clair : un ciel bleu en haut ; la Nuit, bien plus sombre et bleue ; le Coucher de soleil, un horizon chaud.
    assert!(s(Preset::Jour)[2] > s(Preset::Jour)[0] + 30.0, "{:?}", s(Preset::Jour));
    assert!(lum(s(Preset::Nuit), 0) < lum(s(Preset::Jour), 0) * 0.35 && s(Preset::Nuit)[2] > s(Preset::Nuit)[0], "{:?}", s(Preset::Nuit));
    assert!(s(Preset::Coucher)[3] > s(Preset::Coucher)[5] + 40.0, "l'horizon du soir est chaud : {:?}", s(Preset::Coucher));
    // Le Studio : un gris neutre, sans horizon marqué.
    let st = s(Preset::Studio);
    assert!(st.iter().all(|v| (v - st[0]).abs() < 30.0), "{st:?}");
    assert!((st[0] - st[1]).abs() < 2.0 && (st[1] - st[2]).abs() < 2.0, "neutre : {st:?}");
    // Le Brouillard : le ciel et l'horizon presque pareils.
    let b = s(Preset::Brouillard);
    assert!((lum(b, 0) - lum(b, 3)).abs() < 80.0, "{b:?}");
}

#[test]
fn the_night_has_stars_the_city_has_lit_windows_at_night_and_the_mountains_have_ridges() {
    let (w, h) = (1024, 512);
    let nuit = cyclos::generer(&Preset::Nuit.cyclo(), w, h);
    // Des étoiles : des points clairs dans le haut du ciel sombre.
    let haut = &nuit.rgba[..w * (h / 4) * 4];
    let etoiles = haut.chunks(4).filter(|px| px[0] as u32 + px[1] as u32 + px[2] as u32 > 330).count();
    assert!(etoiles > 20, "des étoiles : {etoiles}");
    // La ville de nuit : des fenêtres allumées (chaudes) sous l'horizon des immeubles ; de jour, presque aucune.
    let fenetres = |c: &Cyclo| {
        let img = cyclos::generer(c, w, h);
        let (y0, y1) = (h / 2 - h / 10, h / 2);
        (y0..y1).flat_map(|y| (0..w).map(move |x| (y * w + x) * 4)).filter(|&i| img.rgba[i] > 170 && img.rgba[i + 1] > 120 && img.rgba[i] as i32 - img.rgba[i + 2] as i32 > 60).count()
    };
    let ville_nuit = fenetres(&Cyclo { heure: 23.0, ..Preset::Ville.cyclo() });
    let ville_jour = fenetres(&Preset::Ville.cyclo());
    assert!(ville_nuit > 30 && ville_nuit > ville_jour * 3, "des fenêtres allumées la nuit : {ville_nuit} (jour : {ville_jour})");
    // La montagne : la ligne des crêtes varie d'une colonne à l'autre (pas un horizon plat, comme la Mer).
    let crete = |p: Preset| {
        // (Sans nuages ni brume : rien d'autre que l'horizon et ses silhouettes.)
        let img = cyclos::generer(&Cyclo { nuages: 0.0, brume: 0.0, ..p.cyclo() }, w, h);
        let ligne: Vec<f32> = (0..w)
            .step_by(8)
            .map(|x| {
                // (La première ligne, en descendant du haut de la bande du ciel, qui s'écarte du ciel juste au-dessus d'elle.)
                (h / 4..h / 2)
                    .find(|&y| {
                        let (i, j) = ((y * w + x) * 4, ((y - 6) * w + x) * 4);
                        (0..3).map(|k| (img.rgba[i + k] as i32 - img.rgba[j + k] as i32).abs()).sum::<i32>() > 40
                    })
                    .unwrap_or(h / 2) as f32
            })
            .collect();
        let m = ligne.iter().sum::<f32>() / ligne.len() as f32;
        (ligne.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / ligne.len() as f32).sqrt()
    };
    let (montagne, mer) = (crete(Preset::Montagne), crete(Preset::Mer));
    assert!(montagne > mer + 2.0, "des crêtes : {montagne} (la mer : {mer})");
}

// ------------------------------------------------------------------------------------------------ 2. l'image, gardée

#[test]
fn the_cyclo_image_is_made_once_and_neither_the_camera_nor_the_rotation_remakes_it() {
    let c = Cyclo { graine: 202_001, ..Preset::Montagne.cyclo() };
    let doc = avec_cyclo(carte(true, &[]), Some(c));
    let cle = c.cle();
    assert_eq!(cyclos::faites_de(cle), 1, "faite une fois");
    let avant = cyclos::texture(cle).unwrap();
    // La caméra du plan bouge (de côté, en avant) : chaque image du plan se refait, le cyclo non.
    let base = doc.camera_seen(0, 0);
    let mut images = Vec::new();
    for k in 0..6 {
        let mut cam = base;
        cam.offset.x += k as f32 * 7.0;
        cam.dolly += k as f32 * 0.03;
        let item = decor3d::item(&doc, 0, 0, &cam, None).expect("l'élément du décor");
        let Some(crate::render::FondPart::Decor(key)) = item.fond else { panic!() };
        let objets = decor3d::objets_de(&doc, &key).unwrap();
        let (r, _, _) = decor3d::calculer(&objets, &key, 2);
        images.push(r.get(4, 4));
    }
    // (La rotation tourne à la lecture.)
    let mut tourne = doc;
    let seq = tourne.sequence_of(0);
    tourne.decors3d.par_sequence.get_mut(&seq).unwrap().cyclo.as_mut().unwrap().rotation = 90.0;
    let _ = exporter(&tourne);
    assert_eq!(cyclos::faites_de(cle), 1, "ni la caméra ni la rotation ne la refont");
    assert!(Arc::ptr_eq(&avant, &cyclos::texture(cle).unwrap()), "la même image, gardée");
    assert!(images.iter().all(|p| p.a() == 255), "le ciel derrière, à chaque pose de la caméra : {images:?}");
    // Un réglage qui la change (l'heure) : une autre image ; la rotation, non.
    assert_ne!(Cyclo { heure: 18.5, ..c }.cle(), cle);
    assert_eq!(Cyclo { rotation: 45.0, ..c }.cle(), cle);
}

// ------------------------------------------------------------------------------------------------ 3. l'image du plan

#[test]
fn the_plan_image_shows_the_cyclo_behind_the_cards_and_the_decor_and_aucun_gives_back_todays_pixels() {
    let aujourd_hui = carte(true, &[]);
    let img0 = exporter(&aujourd_hui);
    let (carte0, ciel0, _) = trois(&img0, &aujourd_hui);
    let doc = avec_cyclo(carte(true, &[]), Some(Preset::Jour.cyclo()));
    let img = exporter(&doc);
    let (carte1, ciel1, decor1) = trois(&img, &doc);
    // Le ciel : celui du cyclo, plus le dégradé d'avant ; opaque.
    assert_eq!(ciel1.a(), 255);
    assert!(ecart(ciel0, ciel1) > 6, "le cyclo remplace le ciel d'avant : {ciel0:?} → {ciel1:?}");
    // La carte passe devant (Jour clair : ses couleurs), le décor aussi (le carré, devant le ciel).
    assert!(ecart(carte0, carte1) <= 2, "Jour clair : la carte telle qu'elle est dessinée : {carte0:?} → {carte1:?}");
    assert!(decor1.a() == 255 && ecart(decor1, ciel1) > 10, "le décor devant le cyclo : {decor1:?} / {ciel1:?}");
    // « Aucun » : l'image d'aujourd'hui, au bit près.
    let aucun = avec_cyclo(doc, None);
    assert!(exporter(&aucun) == img0, "sans cyclo, les pixels d'aujourd'hui");
    // Et la case des cartes, sans cyclo, ne change rien.
    assert!(exporter(&carte(false, &[])) == img0);
}

#[test]
fn a_sequence_without_any_3d_object_has_its_cyclo_in_the_plan_image() {
    let mut doc = carte(true, &[]);
    doc.decors3d.par_sequence.clear();
    let img0 = exporter(&doc);
    assert!(doc.decor_du_plan(0).is_none());
    let doc = avec_cyclo(doc, Some(Preset::Coucher.cyclo()));
    assert!(doc.decor_du_plan(0).is_some_and(|d| d.objets.is_empty() && d.cyclo.is_some()), "un décor : son cyclo seul");
    let img = exporter(&doc);
    let (c0, s0, _) = trois(&img0, &doc);
    let (c1, s1, _) = trois(&img, &doc);
    assert!(s0.a() < 255 && s1.a() == 255, "le ciel du cyclo derrière la carte : {s0:?} → {s1:?}");
    // Le coucher réchauffe la carte (plus de rouge que de bleu qu'avant).
    assert!(c1.r() as i32 - c1.b() as i32 > c0.r() as i32 - c0.b() as i32, "réchauffée : {c0:?} → {c1:?}");
}

// ------------------------------------------------------------------------------------------------ 4. la lumière des cartes

#[test]
fn the_night_darkens_and_blues_the_cards_a_lamp_lights_them_and_the_card_checkbox_leaves_them_as_drawn() {
    let jour = avec_cyclo(carte(true, &[]), Some(Preset::Jour.cyclo()));
    let (cj, _, _) = trois(&exporter(&jour), &jour);
    let nuit = avec_cyclo(carte(true, &[]), Some(Preset::Nuit.cyclo()));
    let (cn, ciel_nuit, _) = trois(&exporter(&nuit), &nuit);
    assert!(somme(cn) * 2 < somme(cj), "la nuit assombrit la carte : {cj:?} → {cn:?}");
    assert!((cn.b() as f32 / cn.r().max(1) as f32) > (cj.b() as f32 / cj.r().max(1) as f32) * 1.4, "et la bleuit : {cj:?} → {cn:?}");
    assert!(somme(ciel_nuit) < 200, "un ciel de nuit : {ciel_nuit:?}");
    // Une lampe tout près de la carte, la nuit : elle l'éclaire.
    let lampe_nuit = avec_cyclo(carte(true, &[lampe([0.0, 0.0, 0.6])]), Some(Preset::Nuit.cyclo()));
    let (cl, _, _) = trois(&exporter(&lampe_nuit), &lampe_nuit);
    assert!(somme(cl) > somme(cn) + 60, "la lampe éclaire la carte la nuit : {cn:?} → {cl:?}");
    // « Prend la lumière du cyclo » décochée : la carte telle qu'elle est dessinée (le ciel reste celui du cyclo).
    let decochee = avec_cyclo(carte(false, &[]), Some(Preset::Nuit.cyclo()));
    let sans = carte(false, &[]);
    let (cd, sd, _) = trois(&exporter(&decochee), &decochee);
    let (c0, _, _) = trois(&exporter(&sans), &sans);
    assert_eq!(cd, c0, "décochée : pas de lumière du cyclo");
    assert_eq!(sd, ciel_nuit);
    // Le coucher : la carte plus chaude ; le studio : à peine changée.
    let soir = avec_cyclo(carte(true, &[]), Some(Preset::Coucher.cyclo()));
    let (cs, _, _) = trois(&exporter(&soir), &soir);
    assert!(cs.r() as i32 - cs.b() as i32 > cj.r() as i32 - cj.b() as i32 + 10, "réchauffée : {cj:?} → {cs:?}");
    let studio = avec_cyclo(carte(true, &[]), Some(Preset::Studio.cyclo()));
    let (ct, _, _) = trois(&exporter(&studio), &studio);
    assert!(ecart(ct, cj) <= 3, "{cj:?} / {ct:?}");
    // La lumière d'un pixel de carte : `c × (ambiante + ambiante_soleil × n·soleil)`, en lumière linéaire.
    let plan = crate::render::panel_plan_at(&nuit, 0, 0, None);
    let Some(crate::render::FondPart::Decor(k)) = plan.items.iter().find_map(|i| i.fond.clone()) else { panic!() };
    let cy = k.aspect.cyclo;
    assert!(cy.actif);
    let lum = crate::ombres::Lumiere::des_lampes(&k.aspect, &k.prise().unwrap());
    let px = Color32::from_rgb(180, 150, 90);
    let e = lum.eclairer_carte_avec(px, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0, false, true);
    let s = cy.soleil_dir[2].max(0.0);
    let lin = crate::couleurs3d::lineaire([px.r(), px.g(), px.b()]);
    let att: [u8; 3] = std::array::from_fn(|i| (crate::decor3drendu::vers_srgb(lin[i] * (cy.ambiante[i] + cy.ambiante_soleil[i] * s)) * 255.0).round() as u8);
    assert!(ecart(e, Color32::from_rgb(att[0], att[1], att[2])) <= 1, "{e:?} / {att:?}");
    assert_eq!(lum.eclairer_carte_avec(px, [0.0, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0, false, false), px, "sans la case : telle quelle");
}

#[test]
fn jour_clair_keeps_the_card_pixels_within_a_tight_tolerance_everywhere_on_the_card() {
    let sans = carte(true, &[]);
    let jour = avec_cyclo(carte(true, &[]), Some(Preset::Jour.cyclo()));
    let (a, b) = (exporter(&sans), exporter(&jour));
    let [w, h] = sans.frame;
    let mut pire = 0;
    for y in h / 2 - 4..h / 2 + 4 {
        for x in w / 2 - 4..w / 2 + 4 {
            pire = pire.max(ecart(a[y * w + x], b[y * w + x]));
        }
    }
    assert!(pire <= 2, "Jour clair : au plus {pire} sur la carte");
    // (Sa lumière : 1, sans soleil.)
    let vu = Preset::Jour.cyclo().vu();
    assert_eq!((vu.ambiante, vu.ambiante_soleil), ([1.0; 3], [0.0; 3]));
}

#[test]
fn on_the_gpu_the_cyclo_is_behind_the_card_the_night_darkens_it_and_the_checkbox_leaves_it() {
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    let capture = |doc: Document| {
        let mut b = banc_taille(doc, true, 1280, 800);
        b.app.set_espace(Espace::Storyboard);
        b.app.renderer.invalidate_all();
        b.chauffer(12);
        decor3d::wait_idle();
        cyclos::attendre();
        b.chauffer(6);
        let (dw, dh) = (b.app.doc.width as f32, b.app.doc.height as f32);
        let [fw, fh] = b.app.doc.frame;
        let p = b.app.layer_to_screen(pos2(dw / 2.0, dh / 2.0));
        let q = b.app.layer_to_screen(pos2(dw / 2.0 - fw as f32 / 2.0 + 4.0, dh / 2.0 - fh as f32 / 2.0 + 4.0));
        let rgba = b.capturer().unwrap();
        let w = b.taille.x as usize;
        let at = |p: eframe::egui::Pos2| {
            let i = (p.y as usize * w + p.x as usize) * 4;
            Color32::from_rgb(rgba[i], rgba[i + 1], rgba[i + 2])
        };
        (at(p), at(q), rgba)
    };
    let (c0, s0, img0) = capture(carte(true, &[]));
    let (cj, sj, _) = capture(avec_cyclo(carte(true, &[]), Some(Preset::Jour.cyclo())));
    assert!(ecart(c0, cj) <= 2, "Jour clair sur la carte graphique : {c0:?} → {cj:?}");
    assert!(ecart(s0, sj) > 6, "le cyclo derrière : {s0:?} → {sj:?}");
    let (cn, _, _) = capture(avec_cyclo(carte(true, &[]), Some(Preset::Nuit.cyclo())));
    assert!(somme(cn) * 2 < somme(c0), "la nuit assombrit la carte : {c0:?} → {cn:?}");
    let (cl, _, _) = capture(avec_cyclo(carte(true, &[lampe([0.0, 0.0, 0.6])]), Some(Preset::Nuit.cyclo())));
    assert!(somme(cl) > somme(cn) + 60, "la lampe éclaire la carte la nuit : {cn:?} → {cl:?}");
    let (cd, _, _) = capture(avec_cyclo(carte(false, &[]), Some(Preset::Nuit.cyclo())));
    let (cd0, _, _) = capture(carte(false, &[]));
    assert!(ecart(cd, cd0) <= 1, "décochée : la carte telle qu'elle est dessinée : {cd0:?} → {cd:?}");
    // « Aucun » : l'écran d'aujourd'hui, au pixel près.
    let (_, _, img_aucun) = capture(avec_cyclo(avec_cyclo(carte(true, &[]), Some(Preset::Nuit.cyclo())), None));
    assert!(img_aucun == img0, "sans cyclo, l'écran d'aujourd'hui");
}

// ------------------------------------------------------------------------------------------------ 5. l'enregistrement

#[test]
fn a_sequence_without_objects_keeps_its_cyclo_and_lamps_through_save_and_load_and_an_old_reader_still_works() {
    let dir = crate::decor3d_tests::dossier("cyclos202");
    let manifeste = dir.join("Film.shot");
    let mut doc = carte(false, &[]);
    doc.decors3d.par_sequence.clear();
    let c = Cyclo { heure: 21.5, nuages: 0.4, couleur: Some([20, 40, 200]), graine: 17, rotation: 30.0, ..Preset::Ville.cyclo() };
    let mut doc = avec_cyclo(doc, Some(c));
    let seq = doc.sequence_of(0);
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().lampes.push(lampe([0.5, 1.0, 0.0]));
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"cyclo\"") && texte.contains("\"ville\"") && texte.contains("\"lampes\""), "{texte}");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = relu.doc.decors3d.par_sequence.get(&seq).expect("le décor sans objet");
    assert_eq!(d.cyclo, Some(c));
    assert_eq!(d.lampes.len(), 1);
    assert!(relu.doc.decor_du_plan(0).is_some());
    assert!(!relu.doc.panels[0].layers[0].prend_cyclo, "la case décochée se garde");
    // Un lecteur d'avant (la séquence sans cyclo) lit le fichier d'aujourd'hui.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct SequenceAvant {
        sequence: u64,
        #[serde(default)]
        aspect: Aspect,
        objets: Vec<serde_json::Value>,
        #[serde(default)]
        lampes: Vec<serde_json::Value>,
    }
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(v["version"], serde_json::json!(2), "la version du fichier ne change pas");
    for s in v["decors"].as_array().unwrap() {
        let _: SequenceAvant = serde_json::from_value(s.clone()).expect("une séquence lue par un lecteur d'avant");
    }
    // Un fichier d'avant (sans la clé) : aucun cyclo, les lampes seules restent.
    let mut ancien = v.clone();
    for s in ancien["decors"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("cyclo");
    }
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&ancien).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert!(relu.doc.decors3d.par_sequence.get(&seq).is_some_and(|d| d.cyclo.is_none() && d.lampes.len() == 1), "les lampes seules restent");
    assert!(relu.doc.decor_du_plan(0).is_none(), "des lampes seules ne font pas un décor (comme avant)");
    // Un fichier abîmé : le cyclo revient dans ses bornes.
    let mut abime = v.clone();
    abime["decors"][0]["cyclo"]["heure"] = serde_json::json!(1e9);
    abime["decors"][0]["cyclo"]["nuages"] = serde_json::json!(-4.0);
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&abime).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    let c2 = relu.doc.decors3d.par_sequence.get(&seq).unwrap().cyclo.unwrap();
    assert_eq!((c2.heure, c2.nuages), (24.0, 0.0));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_layer_from_an_old_file_takes_the_cyclo_light_and_only_an_unchecked_box_is_written() {
    let dir = crate::decor3d_tests::dossier("cyclos202-calque");
    let manifeste = dir.join("Film.shot");
    let doc = carte(true, &[]);
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let tout: String = walk(&dir);
    assert!(!tout.contains("prend_cyclo"), "cochée (d'office) : rien d'écrit, un fichier d'avant");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.doc.panels[0].layers[0].prend_cyclo, "un calque d'avant prend la lumière du cyclo");
    let decochee = carte(false, &[]);
    crate::project::save(&decochee, &manifeste, &mut Default::default()).unwrap();
    assert!(walk(&dir).contains("prend_cyclo"));
    assert!(!crate::project::load(&manifeste).unwrap().doc.panels[0].layers[0].prend_cyclo);
    std::fs::remove_dir_all(&dir).ok();
}

/// Tout le texte des fichiers JSON d'un dossier (et de ses sous-dossiers).
fn walk(dir: &std::path::Path) -> String {
    let mut s = String::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            s.push_str(&walk(&p));
        } else if let Ok(t) = std::fs::read_to_string(&p) {
            s.push_str(&t);
        }
    }
    s
}

#[test]
fn copying_the_decor_and_duplicating_the_sequence_carry_the_cyclo() {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_sequence_after(0);
    let (a, b) = (doc.sequence_order()[0], doc.sequence_order()[1]);
    let c = Cyclo { graine: 3, ..Preset::Nuit.cyclo() };
    doc = avec_cyclo(doc, Some(c));
    // Un cyclo seul se copie vers une séquence sans décor.
    assert!(doc.copier_decor_de_sequence(a, b));
    assert_eq!(doc.decors3d.par_sequence.get(&b).and_then(|d| d.cyclo), Some(c));
    assert!(!doc.copier_decor_de_sequence(a, b), "pas par-dessus un décor");
    // La séquence dupliquée a le cyclo de l'originale.
    let n = doc.panels.len();
    doc.dupliquer_sequence(a).expect("la copie");
    assert!(doc.panels.len() > n);
    let neuve = *doc.sequence_order().iter().find(|s| **s != a && **s != b).unwrap();
    assert_eq!(doc.decors3d.par_sequence.get(&neuve).and_then(|d| d.cyclo), Some(c));
    // Les décors à objets copient aussi le leur.
    let mut deux = avec_cyclo(crate::decor3d_tests::projet_a_deux_carres(), Some(Preset::Montagne.cyclo()));
    deux.add_sequence_after(0);
    let (a2, b2) = (deux.sequence_order()[0], deux.sequence_order()[1]);
    assert!(deux.copier_decor_de_sequence(a2, b2));
    let d = deux.decors3d.par_sequence.get(&b2).unwrap();
    assert!(d.objets.len() == 2 && d.cyclo.map(|c| c.preset) == Some(Preset::Montagne));
}

// ------------------------------------------------------------------------------------------------ 6. l'interface

fn banc_3d(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1600, 1800);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.set_espace(Espace::TroisD);
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn ou(nom: &str) -> eframe::egui::Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

#[test]
fn choosing_a_cyclo_is_one_undo_step_a_slider_is_one_step_at_release_and_aucun_gives_back_the_decor() {
    let mut b = banc_3d(carte(true, &[]));
    let seq = b.app.doc.sequence_of(0);
    let avant = b.app.doc.decor_du_plan(0).unwrap().aspect;
    b.app.changer_cyclo("Cyclo", Some(Preset::Nuit.cyclo()));
    assert_eq!(b.app.cyclo_courant().map(|c| c.preset), Some(Preset::Nuit));
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Cyclo"));
    // Le soleil du décor suit celui du cyclo (son heure), sans toucher au sien.
    let d = b.app.doc.decors3d.par_sequence.get(&seq).unwrap();
    let vu = decor3d::aspect_vu(d, &crate::espace3dmath::IDENT);
    assert_ne!(vu.soleil(), avant.soleil());
    assert_eq!(d.aspect, avant);
    // Un réglage tiré : une étape au lâcher.
    let n = etapes(&b).len();
    for h in [22.0, 21.0, 20.5] {
        b.app.retoucher_cyclo(Cyclo { heure: h, ..b.app.cyclo_courant().unwrap() });
    }
    b.app.decor3d_retouche_finir();
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(b.app.cyclo_courant().map(|c| c.heure), Some(20.5));
    annuler(&mut b);
    assert_eq!(b.app.cyclo_courant().map(|c| c.heure), Some(Preset::Nuit.cyclo().heure));
    annuler(&mut b);
    assert_eq!(b.app.cyclo_courant(), None, "Ctrl+Z : plus de cyclo");
    // « Aucun » : le décor tel qu'il était.
    b.app.changer_cyclo("Cyclo", Some(Preset::Coucher.cyclo()));
    b.app.changer_cyclo("Cyclo", None);
    let d = b.app.doc.decors3d.par_sequence.get(&seq).unwrap();
    assert!(d.cyclo.is_none() && d.aspect == avant, "Aucun : l'aspect d'avant");
    assert!(b.app.modified);
}

#[test]
fn the_gallery_shows_every_preset_with_its_thumbnail_and_a_click_chooses_it_and_the_outliner_line_leads_there() {
    let mut b = banc_3d(carte(true, &[]));
    // La ligne « Cyclo » de Scène 3D mène à la section, au bas de Propriétés > Monde.
    assert_eq!(crate::onglet3d::nom_de(&b.app.doc, 0, crate::onglet3d::Ligne3d::Ciel), "Cyclo");
    b.app.colonne.montrer(Pane::Lumieres3d);
    b.chauffer(2);
    b.app.scene3d_choisir(crate::onglet3d::Ligne3d::Ciel, false);
    b.chauffer(4);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Monde3d));
    for p in Preset::TOUS {
        ou(&format!("Cyclo : {}", p.nom()));
    }
    ou("Cyclo : Aucun");
    // Les vignettes : faites par le code, gardées, différentes.
    let v: Vec<_> = Preset::TOUS.iter().map(|p| cyclos::vignette(*p)).collect();
    assert!(Arc::ptr_eq(&v[0], &cyclos::vignette(Preset::TOUS[0])), "gardée");
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            assert!(v[i].rgba != v[j].rgba);
        }
    }
    let r = ou("Cyclo : Nuit");
    assert!(r.max.x <= b.taille.x && r.max.y <= b.taille.y, "à l'écran : {r:?}");
    clic(&mut b, r.center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.cyclo_courant().map(|c| c.preset), Some(Preset::Nuit));
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Cyclo"));
    // Ses réglages paraissent.
    for nom in ["Heure du cyclo", "Nuages du cyclo", "Brume du cyclo", "Rotation du cyclo", "Autre graine", "Couleur dominante du cyclo"] {
        ou(nom);
    }
    let g = b.app.cyclo_courant().unwrap().graine;
    clic(&mut b, ou("Autre graine").center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.cyclo_courant().unwrap().graine, g + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Retoucher le décor 3D"));
    clic(&mut b, ou("Cyclo : Aucun").center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.cyclo_courant(), None);
}

#[test]
fn the_storyboard_bar_has_the_cyclo_picker_and_the_card_has_its_checkbox_with_undo() {
    let mut b = banc_3d(carte(true, &[]));
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    ou("Cyclo de la séquence");
    b.app.changer_cyclo("Cyclo", Some(Preset::Ville.cyclo()));
    b.app.basculer_prend_cyclo(0);
    assert!(!b.app.doc.panels[0].layers[0].prend_cyclo);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Ne prend pas la lumière du cyclo"));
    annuler(&mut b);
    assert!(b.app.doc.panels[0].layers[0].prend_cyclo);
    // Dans l'onglet 3D, la carte choisie montre la case (la séquence a un cyclo) ; la barre de la vue, « Cyclo », allumé d'office.
    b.app.set_espace(Espace::TroisD);
    b.app.espace3d.sel = vec![crate::espace3d::card_selection(b.app.doc.panels[0].layers[0].id)];
    b.app.colonne.montrer(Pane::Objet3d);
    b.chauffer(6);
    ou(crate::cyclosui::NOM_PREND);
    assert!(b.app.o3.montrer_cyclo);
    ou("Cyclo");
}

// ------------------------------------------------------------------------------------------------ 7. la mesure

/// La mesure de la partie 202 : le temps de faire l'image d'un cyclo (2048 × 1024, chaque choix), et ce que coûte une image d'écran
/// avec un cyclo (la vue Espace 3D qui tourne ; l'export par le processeur), avec et sans.
#[test]
fn mesure_des_cyclos() {
    let mediane = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() - 1])
    };
    for p in Preset::TOUS {
        let t0 = std::time::Instant::now();
        let _ = cyclos::generer(&Cyclo { graine: 99, ..p.cyclo() }, cyclos::LARGEUR, cyclos::HAUTEUR);
        eprintln!("MESURE 202 : l'image du cyclo « {} » (2048 × 1024) : {:.0} ms", p.nom(), t0.elapsed().as_secs_f64() * 1000.0);
    }
    let t0 = std::time::Instant::now();
    for p in Preset::TOUS {
        let _ = cyclos::vignette(p);
    }
    eprintln!("MESURE 202 : les neuf vignettes : {:.0} ms", t0.elapsed().as_secs_f64() * 1000.0);
    // L'export (le processeur) : avec et sans cyclo.
    for cy in [None, Some(Preset::Montagne.cyclo())] {
        let doc = avec_cyclo(carte(true, &[]), cy);
        let _ = exporter(&doc);
        let mut t = Vec::new();
        for _ in 0..5 {
            let t0 = std::time::Instant::now();
            let _ = exporter(&doc);
            t.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let (m, p) = mediane(t);
        eprintln!("MESURE 202 : processeur (export 160 × 90), {} : médiane {m:.2} ms, pire {p:.2} ms", if cy.is_some() { "avec cyclo" } else { "sans" });
    }
    // L'export en 1920 × 1080 d'un plan sans objet 3D : sans cyclo, avec (le ciel lu pixel par pixel, la lumière des cartes).
    for cy in [None, Some(Preset::Ville.cyclo())] {
        let mut doc = Document::with_frame(1920, 1080, 24);
        doc.frame = [1920, 1080];
        let doc = avec_cyclo(doc, cy);
        let _ = exporter(&doc);
        let mut t = Vec::new();
        for _ in 0..5 {
            let t0 = std::time::Instant::now();
            let _ = exporter(&doc);
            t.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        let (m, p) = mediane(t);
        eprintln!("MESURE 202 : processeur (export 1920 × 1080), {} : médiane {m:.1} ms, pire {p:.1} ms", if cy.is_some() { "avec cyclo" } else { "sans" });
    }
    // La carte graphique : la vue Espace 3D tourne (une image d'écran par pas), avec et sans cyclo ; le cyclo ne se refait pas.
    if crate::gpu_tests::headless().is_some() {
        for cy in [None, Some(Cyclo { graine: 202_777, ..Preset::Ville.cyclo() })] {
            let mut b = banc_taille(avec_cyclo(carte(true, &[]), cy), true, 1600, 1000);
            b.app.set_espace(Espace::TroisD);
            b.chauffer(10);
            decor3d::wait_idle();
            cyclos::attendre();
            b.chauffer(4);
            let mut t = Vec::new();
            for k in 0..30 {
                if let Some(c) = b.app.espace3d.cam.as_mut() {
                    c.azimuth += if k % 2 == 0 { 2.0 } else { -1.0 };
                }
                let m = b.image(Vec::new(), 1.0 / 60.0);
                t.push(m.carte);
            }
            if let Some(c) = cy {
                assert_eq!(cyclos::faites_de(c.cle()), 1, "la vue tourne : le cyclo ne se refait pas");
            }
            let (m, p) = mediane(t);
            eprintln!("MESURE 202 : carte graphique, la vue Espace 3D qui tourne, {} : médiane {m:.2} ms, pire {p:.2} ms", if cy.is_some() { "avec cyclo" } else { "sans" });
        }
    }
}

/// (Pour regarder : `FEANOR_CYCLOS_PNG=<dossier>` écrit la planche des choix, de jour et à 23 h.)
#[test]
fn ecrire_les_cyclos_en_png() {
    let Ok(dir) = std::env::var("FEANOR_CYCLOS_PNG") else { return };
    let (w, h) = (640, 320);
    let (cols, rows) = (2, Preset::TOUS.len());
    let mut planche = vec![255u8; cols * w * rows * h * 4];
    for (r, p) in Preset::TOUS.iter().enumerate() {
        for (k, c) in [p.cyclo(), Cyclo { heure: 23.0, ..p.cyclo() }].into_iter().enumerate() {
            let img = cyclos::generer(&c, w, h);
            for y in 0..h {
                let o = ((r * h + y) * cols * w + k * w) * 4;
                planche[o..o + w * 4].copy_from_slice(&img.rgba[y * w * 4..(y + 1) * w * 4]);
            }
        }
    }
    image::save_buffer(format!("{dir}/planche.png"), &planche, (cols * w) as u32, (rows * h) as u32, image::ColorType::Rgba8).unwrap();
}

/// (Pour regarder : `FEANOR_CYCLOS_PNG=<dossier>` écrit l'écran de l'onglet 3D, Propriétés > Monde ouvert, et celui du Storyboard,
/// avec le cyclo Montagne.)
#[test]
fn ecrire_les_ecrans_en_png() {
    let Ok(dir) = std::env::var("FEANOR_CYCLOS_PNG") else { return };
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    let mut b = banc_taille(avec_cyclo(carte(true, &[]), Some(Preset::Montagne.cyclo())), true, 1600, 1000);
    b.app.set_espace(Espace::TroisD);
    b.app.colonne.montrer(Pane::Monde3d);
    b.chauffer(10);
    decor3d::wait_idle();
    cyclos::attendre();
    b.chauffer(6);
    let (w, h) = (b.taille.x as u32, b.taille.y as u32);
    image::save_buffer(format!("{dir}/ecran-3d.png"), &b.capturer().unwrap(), w, h, image::ColorType::Rgba8).unwrap();
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(10);
    decor3d::wait_idle();
    b.chauffer(4);
    image::save_buffer(format!("{dir}/ecran-storyboard.png"), &b.capturer().unwrap(), w, h, image::ColorType::Rgba8).unwrap();
}
