//! Partie 127 : un trait **bitmap** tracé à travers les cases, au vrai geste sur le `Banc`, vu en pixels comme les parties 111, 122 et 125. Hors des
//! cases, ses pixels restent sur la couche bitmap de la planche ; la sélection des Vignettes prend le trait entier ; rien ne se perd.
//!
//! Le premier essai (`moving_a_bitmap_stroke_selected_in_one_case_moves_the_whole_stroke`) ne passe que par l'écran (Ctrl + lasso, Ctrl + glisser, pixels
//! lus) : il a été lancé tel quel sur le code d'avant (6bcade3), rouge.

use eframe::egui::{pos2, vec2, Modifiers, Pos2};

use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer, Releve};
use crate::vignettes_selection_tests::{absent, glisser_avec, glisser_droit, lasso, present, sur_le_trait, trait_a_travers};

fn voir(b: &mut Banc, nom: &str, de: Pos2, a: Pos2) -> Releve {
    repos(b);
    // (La boîte du choix est orange : on la lève le temps de la capture, pour ne voir que le dessin.)
    let choix = b.app.vignettes.selb.take();
    b.chauffer(2);
    let rgba = b.capturer().unwrap();
    b.app.vignettes.selb = choix;
    b.chauffer(1);
    sauver(&rgba, nom);
    let r = relever(&rgba, de, a);
    println!("{nom} : {r:?}");
    r
}

/// Enregistre et rouvre : la planche relue, au même zoom.
fn rouvrir(b: &Banc, nom: &str) -> Banc {
    let dossier = std::env::temp_dir().join(format!("shot-test-vignettes-bitmap-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    std::fs::remove_dir_all(&dossier).ok();
    let mut c = planche(relu.doc, 6, 1.0).expect("la planche");
    c.app.board_zoom = b.app.board_zoom;
    c.app.board_pan = b.app.board_pan;
    c.chauffer(4);
    c.attendre_vignettes();
    c
}

/// Le point où l'on prend la boîte pour la déplacer : sur le trait, à côté de son milieu (le pivot).
fn prise(m: Pos2, a: Pos2) -> Pos2 {
    m + (a - m).normalized() * 12.0
}

#[test]
fn moving_a_bitmap_stroke_selected_in_one_case_moves_the_whole_stroke() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    present(&voir(&mut b, "bmp-0-avant", de, a), "avant le geste");
    let pas0 = b.app.history.undo_len();
    // Ctrl + lasso autour du trait, dans la case du milieu, puis Ctrl + glisser : 40 pixels à droite.
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 30.0);
    println!("après le lasso : {}", b.app.status);
    assert_eq!(b.app.history.undo_len(), pas0, "choisir ne fait aucune étape");
    let d = vec2(40.0, 0.0);
    let rejeux = crate::vector::replay_count();
    glisser_droit(&mut b, prise(m, a), d);
    let rejeux = crate::vector::replay_count() - rejeux;
    println!("après le geste : {}", b.app.status);
    present(&voir(&mut b, "bmp-1-deplace", de + d, a + d), "le trait déplacé, entier");
    absent(&voir(&mut b, "bmp-1b-ancienne", de, a), "l'ancienne place");
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "une seule étape d'annulation pour le geste");
    assert_eq!(rejeux, 0, "aucun rejeu du moteur sur le fil de l'interface, du geste au lâcher");
    // Annuler, rétablir.
    b.app.undo();
    present(&voir(&mut b, "bmp-2-annule", de, a), "annulé : le trait revient");
    absent(&relever(&b.capturer().unwrap(), de + d, a + d), "annulé : plus rien à la place d'après");
    b.app.redo();
    present(&voir(&mut b, "bmp-3-retabli", de + d, a + d), "rétabli");
    // Enregistrer, rouvrir : le trait est là, et le lasso le reprend encore entier.
    let mut c = rouvrir(&b, "deplace");
    present(&voir(&mut c, "bmp-4-rouvert", de + d, a + d), "rouvert");
    let d2 = vec2(0.0, 30.0);
    lasso(&mut c, m + d, 30.0);
    println!("rouvert, après le lasso : actif {} ; pièces {:?} ; {}", c.app.selb_active(), c.app.vignettes.selb.as_ref().map(|s| s.pieces.len()), c.app.status);
    let pas1 = c.app.history.undo_len();
    glisser_droit(&mut c, prise(m + d, a + d), d2);
    println!("rouvert, après le geste : étapes {} ; {}", c.app.history.undo_len() - pas1, c.app.status);
    present(&voir(&mut c, "bmp-5-rouvert-deplace", de + d + d2, a + d + d2), "rouvert : le lasso reprend le trait entier");
    absent(&voir(&mut c, "bmp-5b-rouvert-ancienne", de + d, a + d), "rouvert : l'ancienne place");
}

// ------------------------------------------------------------------------------ la couche bitmap de la planche

/// Le nombre de pixels d'encre de la couche bitmap de la planche.
fn pixels_planche(b: &Banc) -> usize {
    let mut n = 0;
    for m in &b.app.doc.annotations.bitmap.morceaux {
        crate::planchebitmap::chaque_pixel(&m.pixels, |_, _, _| n += 1);
    }
    n
}

#[test]
fn outside_the_cases_a_bitmap_stroke_keeps_its_pixels_on_the_board() {
    let Some((mut b, de, a, _, _)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    let bm = &b.app.doc.annotations.bitmap;
    println!("morceaux : {:?} ; empreintes : {}", bm.morceaux, bm.empreintes.len());
    assert!(!bm.morceaux.is_empty(), "les pixels hors des cases sont sur la couche bitmap de la planche");
    assert!(b.app.doc.annotations.traces.is_empty(), "et plus en trait vectoriel");
    assert!(bm.empreintes.len() >= 3, "le geste a ses empreintes dans les trois cases ({})", bm.empreintes.len());
    let lien = bm.morceaux[0].lien;
    assert!(lien.is_some() && bm.empreintes.iter().all(|e| Some(e.lien) == lien), "un même numéro pour toutes les parts du geste");
    present(&voir(&mut b, "bmp-planche", de, a), "le trait est continu, cases et planche");
    // (Avant de relire les pixels : la peinture de l'écran les relit sans les garder ouverts.)
    let bm = &b.app.doc.annotations.bitmap;
    let (px, masques): (usize, usize) = (bm.morceaux.iter().map(|m| m.octets()).sum(), bm.empreintes.iter().map(|e| e.masque.octets()).sum());
    println!("rangée : pixels {px} octets, empreintes {masques} octets");
    assert!(px < 64 << 10, "les tuiles du morceau restent rangées après l'affichage ({px} octets)");
    // Les pixels du papier sont exactement ceux du pinceau : sous le bout du trait au-dessus de la case 1, l'encre est pleine (pas une ligne refaite).
    assert!(pixels_planche(&b) > 200, "des pixels sur la planche ({})", pixels_planche(&b));
    // Ni gonflement ni double : la couche ne pèse que ses tuiles.
    let octets = b.app.doc.annotations.bitmap.octets();
    println!("la couche pèse {octets} octets");
    assert!(octets < 2 << 20, "quelques tuiles, pas une image pleine ({octets} octets)");
}

#[test]
fn the_board_export_contains_the_bitmap_layer() {
    let Some((mut b, de, a, _, _)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    let xf = b.app.vignettes.xf;
    let opt = crate::planche::Options::default();
    let img = crate::traits_continus_tests::exporter(&mut b, &opt, "bitmap-planche");
    let o = b.app.planche_options(&opt);
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let p = crate::planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &o, &b.app.doc.annotations);
    let (s, ..) = crate::planche::dimensions(p.cadre, o.resolution);
    // Le bout du trait sur le papier, au-dessus de la case 1 : de l'encre rouge à l'export.
    let bout = xf.to_board(de + (a - de).normalized() * 10.0);
    let (x, y) = (((bout.x - p.cadre.left()) * s) as i32, ((bout.y - p.cadre.top()) * s) as i32);
    let mut rouge = 0;
    for dy in -6..=6 {
        for dx in -6..=6 {
            let (u, v) = (x + dx, y + dy);
            if u >= 0 && v >= 0 && (u as u32) < img.width() && (v as u32) < img.height() {
                let q = img.get_pixel(u as u32, v as u32);
                if q[0] as i32 - q[1] as i32 > 60 {
                    rouge += 1;
                }
            }
        }
    }
    println!("export {}x{} ; pixels rouges au bout du trait : {rouge}", img.width(), img.height());
    assert!(rouge > 10, "l'export de la planche contient la couche bitmap");
}

#[test]
fn an_old_project_opens_unchanged_and_writes_no_folder() {
    // Un trait bitmap tout entier dans une case : rien sur la planche, aucun dossier `planche_bitmap/`.
    let Some(mut b) = planche(des_plans(6, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let t2 = b.app.vignettes.xf.rect(l.cards[2].thumb);
    tracer(&mut b, t2.center() + vec2(-30.0, 0.0), t2.center() + vec2(30.0, 0.0));
    repos(&mut b);
    assert!(b.app.doc.annotations.bitmap.est_vide(), "un trait dans sa case ne laisse rien sur la planche ni d'empreinte");
    let dossier = std::env::temp_dir().join(format!("shot-test-vignettes-bitmap-ancien-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dossier).ok();
    std::fs::create_dir_all(&dossier).unwrap();
    let manifeste = dossier.join("P.shot");
    crate::project::save(&b.app.doc, &manifeste, &mut Default::default()).expect("enregistrement");
    assert!(!dossier.join(crate::planchebitmap::DOSSIER).exists(), "aucun dossier de plus pour un projet sans dessin hors des cases");
    let relu = crate::project::load(&manifeste).expect("réouverture");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!(relu.doc.annotations, b.app.doc.annotations, "les annotations relues sont les mêmes");
    std::fs::remove_dir_all(&dossier).ok();
}

#[test]
fn a_case_that_cannot_receive_keeps_the_part_visible_over_it() {
    // Partie 122 : le calque de la case 3 est verrouillé ; la part du trait sur sa case reste par-dessus elle, sur la planche, et rien n'est écrit dans son plan.
    let Some(mut b) = planche(des_plans(12, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let p3 = l.cards[3].panel;
    for layer in &mut b.app.doc.panels[p3].layers {
        layer.locked = true;
    }
    b.chauffer(2);
    let xf = b.app.vignettes.xf;
    let (t2, t3) = (xf.rect(l.cards[2].thumb), xf.rect(l.cards[3].thumb));
    let (de, a) = (t2.center(), pos2(t3.right() + 40.0, t3.center().y));
    tracer(&mut b, de, a);
    present(&voir(&mut b, "bmp-122", de, a), "rien ne disparaît au lever");
    assert!(!crate::vignettes_tests::encre(&b.app.doc, p3), "rien dans le plan verrouillé");
    let dessus = b.app.doc.annotations.bitmap.morceaux.iter().filter(|m| m.dessus).count();
    assert!(dessus >= 1, "la part sur la case verrouillée est posée par-dessus elle ({dessus}) ; état : {}", b.app.status);
    assert!(b.app.status.contains("verrouillé"), "la ligne d'état dit pourquoi : {}", b.app.status);
    // La gomme posée dessus l'enlève, elle seule.
    let avant = b.app.doc.annotations.bitmap.morceaux.len();
    b.app.set_tool(crate::app::Tool::Eraser);
    let p = t3.center();
    glisser_avec(&mut b, &[p, p + vec2(2.0, 0.0), p + vec2(4.0, 0.0)], Modifiers::NONE);
    let apres = b.app.doc.annotations.bitmap.morceaux.len();
    assert_eq!(apres + dessus, avant, "la gomme enlève la part posée par-dessus la case, elle seule");
}

#[test]
fn painting_behind_puts_the_board_part_under_the_others() {
    let Some(mut b) = planche(des_plans(12, true), 6, 1.0) else { return println!("pas de carte graphique") };
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (t1, t2) = (xf.rect(l.cards[1].thumb), xf.rect(l.cards[2].thumb));
    // Un premier trait de la case 1 au papier au-dessus d'elle, puis un second, peint derrière, sur le papier, qui le croise.
    let y = t1.top() - 30.0;
    tracer(&mut b, pos2(t1.center().x, t1.center().y), pos2(t1.center().x, t1.top() - 50.0));
    repos(&mut b);
    let avant: Vec<Option<u64>> = b.app.doc.annotations.bitmap.morceaux.iter().map(|m| m.lien).collect();
    b.app.paint_behind = true;
    b.app.color = [30, 30, 200];
    tracer(&mut b, pos2(t1.left() - 20.0, y), pos2(t2.center().x, y));
    repos(&mut b);
    println!("morceaux : {:?}", b.app.doc.annotations.bitmap.morceaux);
    let bm = &b.app.doc.annotations.bitmap;
    assert!(bm.morceaux.len() >= 2, "deux morceaux ({})", bm.morceaux.len());
    let neuf = bm.morceaux.iter().map(|m| m.lien).find(|l| !avant.contains(l));
    assert!(neuf.is_some(), "le second trait a son morceau");
    assert_eq!(bm.morceaux[0].lien, neuf.flatten(), "le trait peint derrière est le premier morceau : sous l'autre");
    // Au croisement, le rouge (dessus) se voit, pas le bleu.
    let rgba = b.capturer().unwrap();
    let c = crate::traits_continus_tests::px(&rgba, t1.center().x as i32, y as i32);
    println!("au croisement : {c:?}");
    assert!(c[0] > c[2], "le trait d'avant reste dessus : {c:?}");
}

#[test]
fn scaling_and_rotating_a_bitmap_stroke_take_it_whole() {
    for (nom, geste) in [("echelle", 0), ("rotation", 1)] {
        let Some((mut b, de, a, _, t2)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
        let m = sur_le_trait(de, a, t2.center().x);
        lasso(&mut b, m, 30.0);
        let l = b.app.board_layout();
        let _ = l;
        let xf = b.app.vignettes.xf;
        let s = b.app.vignettes.selb.as_ref().expect("un choix");
        let r = xf.rect(s.bornes.expand(2.0 / xf.zoom.max(0.05)));
        let q = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        let pas0 = b.app.history.undo_len();
        let (handle, depart, vers) = if geste == 0 {
            (crate::selbox::Handle::Corner(0), q[0], q[0] + vec2(-30.0, -20.0))
        } else {
            let c = q[1] + (q[1] - q[3]).normalized() * 10.0;
            (crate::selbox::Handle::Rotate(1), c, c + vec2(0.0, 45.0))
        };
        let frame = crate::selbox::SelFrame { rect0: s.bornes.expand(2.0 / xf.zoom.max(0.05)), m: crate::vector::Affine::IDENTITY, pivot: None };
        let mods = crate::selbox::Mods::with(false, false, b.app.keep_proportions, b.app.settings.point_fixe, b.app.settings.rotation_autour);
        let aff = crate::selbox::gesture(handle, &frame, xf.to_board(depart), xf.to_board(vers), mods);
        let f = |p: Pos2| xf.to_screen(aff.apply(xf.to_board(p)));
        let rejeux = crate::vector::replay_count();
        glisser_droit(&mut b, depart, vers - depart);
        assert_eq!(crate::vector::replay_count(), rejeux, "{nom} : aucun rejeu");
        let (de2, a2) = (f(de), f(a));
        println!("{nom} : {de:?}->{a:?} devient {de2:?}->{a2:?} ; {}", b.app.status);
        present(&voir(&mut b, &format!("bmp-{nom}"), de2, a2), nom);
        assert_eq!(b.app.history.undo_len(), pas0 + 1, "{nom} : une seule étape");
    }
}

#[test]
fn copy_cut_paste_and_delete_a_bitmap_stroke_whole() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 30.0);
    let pas0 = b.app.history.undo_len();
    b.app.run_action("edition.copier");
    assert!(b.app.vignettes.selb_presse.is_some(), "copié : {}", b.app.status);
    b.app.run_action("edition.supprimer");
    absent(&voir(&mut b, "bmp-supprime", de, a), "supprimé : toutes les parts");
    assert!(b.app.doc.annotations.bitmap.morceaux.is_empty(), "plus rien sur la planche");
    assert_eq!(b.app.history.undo_len(), pas0 + 1, "supprimer : une étape");
    b.app.run_action("edition.coller");
    present(&voir(&mut b, "bmp-colle", de, a), "collé : le trait revient à sa place, réparti");
    assert!(!b.app.doc.annotations.bitmap.morceaux.is_empty(), "sa part hors des cases est de nouveau sur la planche");
    assert_eq!(b.app.history.undo_len(), pas0 + 2, "coller : une étape");
    lasso(&mut b, m, 30.0);
    b.app.run_action("edition.couper");
    absent(&voir(&mut b, "bmp-coupe", de, a), "coupé");
    b.app.undo();
    present(&voir(&mut b, "bmp-coupe-annule", de, a), "coupé puis annulé");
}

#[test]
fn a_full_history_still_undoes_the_gesture_in_one_step() {
    let Some((mut b, de, a, _, t2)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    // L'historique plein : chaque étape de plus en oublie une vieille.
    let l = b.app.board_layout();
    let t5 = b.app.vignettes.xf.rect(l.cards[5].thumb);
    for i in 0..(crate::history::LIMIT + 5) {
        let y = t5.top() + 5.0 + (i % 40) as f32;
        let _ = y;
        b.app.record_step();
    }
    let m = sur_le_trait(de, a, t2.center().x);
    lasso(&mut b, m, 30.0);
    let d = vec2(40.0, 0.0);
    glisser_droit(&mut b, prise(m, a), d);
    present(&voir(&mut b, "bmp-plein-deplace", de + d, a + d), "déplacé");
    b.app.undo();
    present(&voir(&mut b, "bmp-plein-annule", de, a), "un seul Ctrl+Z rend tout");
    absent(&relever(&b.capturer().unwrap(), de + d, a + d), "et rien ne reste à la place d'après");
}

// ------------------------------------------------------------------------------ la limite

/// Un projet de 30 plans bitmap dont chaque dessin est plein d'encre (1920 × 1080), et un trait réparti à travers les cases 1 à 3.
fn trente_pleines() -> Option<(Banc, Pos2, Pos2, eframe::egui::Rect)> {
    let mut doc = des_plans(30, true);
    let (w, h) = (doc.width, doc.height);
    let mut plein = crate::raster::Raster::new(w, h);
    let c = eframe::egui::Color32::from_rgba_premultiplied(120, 120, 120, 255);
    for id in 0..plein.tiles.len() {
        // (Des tuiles toutes différentes : aucune ne se partage, le pire cas.)
        let mut t = [c; crate::raster::TILE_PIXELS];
        t[0] = eframe::egui::Color32::from_rgba_premultiplied((id % 251) as u8, 0, 0, 255);
        plein.tiles[id] = Some(crate::raster::Tile::new(t));
    }
    for p in 0..doc.panels.len() {
        let id = doc.new_cel(crate::document::LayerKind::Bitmap);
        doc.cels.get_mut(&id).unwrap().raster = plein.clone();
        let n = doc.panels[p].frame_count();
        for f in 0..n {
            doc.panels[p].layers[0].set(f, Some(id));
        }
    }
    let mut b = planche(doc, 6, 1.0)?;
    b.attendre_vignettes();
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let t2 = xf.rect(l.cards[2].thumb);
    Some((b, t2.center() + vec2(-40.0, 0.0), t2.center() + vec2(40.0, 0.0), xf.rect(l.cards.iter().fold(eframe::egui::Rect::NOTHING, |r, c| r.union(c.thumb)))))
}

#[test]
#[ignore]
fn measure_the_memory_of_a_huge_selection() {
    // Ce n'est pas un essai de réussite : la mesure de la limite (partie 127). Lancée seule : `--test-threads=1`, pour que le pic soit le sien.
    let Some((mut b, _, _, tout)) = trente_pleines() else { return println!("pas de carte graphique") };
    let (ws0, prive0) = crate::lourd_tests::memoire();
    let pic0 = crate::lourd_tests::memoire_pic();
    // Un lasso qui entoure les 30 cases : bien au-delà de la limite. Pour mesurer ce que coûterait une sélection de cette taille, la limite est levée le
    // temps de l'essai (`selb_prendre_sans_limite`).
    let poly = [tout.left_top() + vec2(-5.0, -5.0), tout.right_top() + vec2(5.0, -5.0), tout.right_bottom() + vec2(5.0, 5.0), tout.left_bottom() + vec2(-5.0, 5.0)];
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let poly: Vec<Pos2> = poly.iter().map(|p| xf.to_board(*p)).collect();
    let debut = std::time::Instant::now();
    let pieces = b.app.selb_prendre_sans_limite(&l, &poly);
    let t_choix = debut.elapsed();
    let tuiles: usize = pieces.iter().map(|p| crate::planchebitmap::tuiles_de(&p.pixels)).sum();
    let (ws1, prive1) = crate::lourd_tests::memoire();
    // Le lâcher d'un déplacement de 40 pixels de la planche (même échelle partout : recopie au pixel près).
    let debut = std::time::Instant::now();
    b.app.selb_mesurer_un_lacher(&l, pieces.clone(), &crate::planchebitmap::A64::echelle_decalage(1.0, 40.0 / xf.zoom as f64, 0.0));
    let t_lacher = debut.elapsed();
    let (ws2, prive2) = crate::lourd_tests::memoire();
    let pic = crate::lourd_tests::memoire_pic();
    println!(
        "MESURE 30 cases 1920x1080 pleines : {tuiles} tuiles prises ({:.1} Mpx) ; choix {:.0} ms, lâcher (déplacer) {:.0} ms ; ensemble de travail {ws0:.0} -> {ws1:.0} -> {ws2:.0} Mo, privé {prive0:.0} -> {prive1:.0} -> {prive2:.0} Mo, pic {pic0:.0} -> {pic:.0} Mo",
        tuiles as f64 * 4096.0 / 1e6,
        t_choix.as_secs_f64() * 1000.0,
        t_lacher.as_secs_f64() * 1000.0
    );
    // Tourner : le rééchantillonnage (`Raster::transformee`), le cas le plus lent. Sur le dessin d'avant (annulé).
    b.app.undo();
    let c = tout.center();
    let c = xf.to_board(c);
    let (s, co) = (10.0_f64.to_radians().sin(), 10.0_f64.to_radians().cos());
    let rot = crate::planchebitmap::A64 { a: co, b: -s, c: s, d: co, tx: c.x as f64 - co * c.x as f64 + s * c.y as f64, ty: c.y as f64 - s * c.x as f64 - co * c.y as f64 };
    let (ws3, _) = crate::lourd_tests::memoire();
    let debut = std::time::Instant::now();
    b.app.selb_mesurer_un_lacher(&l, pieces, &rot);
    let t_rot = debut.elapsed();
    let (ws4, _) = crate::lourd_tests::memoire();
    println!("MESURE tourner de 10° : lâcher {:.0} ms ; ensemble de travail {ws3:.0} -> {ws4:.0} Mo, pic {:.0} Mo", t_rot.as_secs_f64() * 1000.0, crate::lourd_tests::memoire_pic());
}

#[test]
fn just_below_and_just_above_the_limit() {
    // Un geste dont l'empreinte couvre exactement `LIMITE_TUILES` tuiles se prend entier ; une de plus : la sélection reste celle de la case, rien ne bouge.
    let Some((mut b, de, a, _, t2)) = trait_a_travers(true) else { return println!("pas de carte graphique") };
    let lien = b.app.doc.annotations.bitmap.empreintes[0].lien;
    let deja: usize = b.app.doc.annotations.bitmap.empreintes.iter().map(|e| e.masque.tuiles.len()).sum::<usize>()
        + b.app.doc.annotations.bitmap.morceaux.iter().map(|m| crate::planchebitmap::tuiles_de(&m.pixels)).sum::<usize>();
    let l = b.app.board_layout();
    let cel = b.app.doc.annotations.bitmap.empreintes[0].cel;
    for (n, attendu) in [(crate::vignetteselbmp::LIMITE_TUILES - deja, true), (crate::vignetteselbmp::LIMITE_TUILES - deja + 1, false)] {
        let mut c = b.app.doc.annotations.bitmap.clone();
        // Une empreinte de plus, dans le même dessin, loin de la feuille : `n` tuiles.
        let mut m = crate::planchebitmap::Masque::default();
        for i in 0..n as i64 {
            m.poser(-20_000 + (i % 300) * 64, -20_000 + (i / 300) * 64);
        }
        c.noter_empreinte(lien, cel, m);
        let garde = std::mem::replace(&mut b.app.doc.annotations.bitmap, c);
        let pris = b.app.selb_du_lien_pour_essai(&l, lien);
        b.app.doc.annotations.bitmap = garde;
        println!("{n} tuiles de plus : {}", if pris { "pris entier" } else { "refusé" });
        assert_eq!(pris, attendu, "limite : {n} tuiles de plus");
    }
    // Au-dessus de la limite, au vrai geste : la sélection reste celle de la case, et la ligne d'état le dit en une phrase.
    let Some((mut g, _, _, tout)) = trente_pleines() else { return };
    let xf = g.app.vignettes.xf;
    let t1 = xf.rect(g.app.board_layout().cards[1].thumb);
    let _ = (de, a, t2);
    crate::vignettes_selection_tests::lasso_par(&mut g, &[t1.center(), tout.right_top() + vec2(5.0, -5.0), tout.right_bottom() + vec2(5.0, 5.0), tout.left_bottom() + vec2(-5.0, 5.0), t1.center()]);
    println!("au-dessus : {}", g.app.status);
    assert!(g.app.status.starts_with("Sélection trop grande pour suivre tout le trait"), "{}", g.app.status);
    let s = g.app.vignettes.selb.as_ref().expect("un choix");
    assert!(s.limite_atteinte);
    let cases: std::collections::BTreeSet<u64> = s.pieces.iter().filter_map(|p| match p.source {
        crate::vignetteselbmp::Source::Dessin { case, .. } => Some(case),
        _ => None,
    }).collect();
    assert_eq!(cases.len(), 1, "seule la part d'une case est prise");
}
