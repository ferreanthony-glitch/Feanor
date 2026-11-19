//! Les transitions du lot 4 des préréglages (partie 175) : poussée, raccord filé, flash de coupe, fondu par une couleur, volet
//! par un objet ; leur fichier ; leurs vignettes et leur dépôt sur une coupe du montage.

use std::collections::HashSet;

use eframe::egui::{pos2, Color32, Event, Rect};

use crate::document::{Document, Layer, LayerKind};
use crate::transitions::{self, blend_images, zones, Kind, Mix, Transition, COULEUR_DEFAUT, OBJET_COULEUR};

/// Un film de plans d'une couleur chacun, de `len` images (feuille 64 × 48, cadre 32 × 24 au milieu).
fn film_uni(colors: &[[u8; 3]], len: usize) -> Document {
    let mut doc = Document::with_frame(32, 24, 12);
    for _ in 1..colors.len() {
        let n = doc.panels.len() - 1;
        doc.add_scene_after(n);
    }
    for (p, c) in colors.iter().enumerate() {
        let id = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&id).unwrap().raster;
        for y in 0..doc.height {
            for x in 0..doc.width {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgba_premultiplied(c[0], c[1], c[2], 255);
            }
        }
        let mut l = Layer::new("Couleur", LayerKind::Bitmap);
        l.frames = vec![Some(id); len];
        doc.panels[p].layers = vec![l];
        doc.panels[p].length = len;
    }
    doc
}

fn app_avec(doc: Document) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("feanor-transitions-lot4-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_frame = 0;
    app
}

fn plein(c: [u8; 3], n: usize) -> Vec<Color32> {
    vec![Color32::from_rgb(c[0], c[1], c[2]); n]
}

const ROUGE: [u8; 3] = [200, 40, 0];
const BLEU: [u8; 3] = [100, 80, 250];

fn proche(got: Color32, want: [u8; 3], tol: i32) -> bool {
    let g = got.to_array();
    (0..3).all(|k| (g[k] as i32 - want[k] as i32).abs() <= tol)
}

#[test]
fn les_cinq_sortes_du_lot_4_sont_rangees_et_permises_ou_il_faut() {
    assert_eq!(Kind::ALL.len(), 13, "dix, puis treize (partie 177)");
    let noms: HashSet<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(noms.len(), 13, "des noms différents");
    for k in [Kind::Push, Kind::Whip, Kind::Flash, Kind::Colour, Kind::Objet] {
        assert!(Kind::ALL.contains(&k) && k.du_lot_4() && k.allowed(true), "{k:?}");
    }
    for k in [Kind::Dissolve, Kind::Black, Kind::White, Kind::Wipe, Kind::CutBlack] {
        assert!(!k.du_lot_4(), "{k:?}");
    }
    // Au bord du film, le fondu par une couleur s'ajoute aux fondus d'avant (et l'iris de la partie 177).
    assert!(Kind::Colour.allowed(false));
    for k in [Kind::Push, Kind::Whip, Kind::Flash, Kind::Objet] {
        assert!(!k.allowed(false), "{k:?}");
    }
    // Le flash et le raccord filé sont courts.
    assert_eq!((Kind::Flash.default_frames(), Kind::Whip.default_frames(), Kind::Push.default_frames()), (4, 8, transitions::DEFAULT_FRAMES));
}

#[test]
fn la_poussee_fait_sortir_le_plan_par_la_gauche_et_entrer_le_suivant_par_la_droite() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    doc.set_transition(1, Kind::Push).expect("une poussée sur la coupe");
    assert_eq!(doc.transitions[0].frames, transitions::DEFAULT_FRAMES);
    let z = zones(&doc)[0];
    let mut avant = 0.0_f32;
    for g in z.start..z.end {
        let m = z.mix(g).unwrap();
        assert!(m.glisse && m.a.is_some() && m.b.is_some() && m.blur == 0.0, "{m:?}");
        assert!((m.shift_b - (1.0 + m.shift_a)).abs() < 1e-6, "les deux plans se touchent : {m:?}");
        assert!(m.shift_a < avant, "A sort de plus en plus : {m:?}");
        assert!(m.shift_a > -1.0 && m.shift_a < 0.0);
        avant = m.shift_a;
    }
    // À la coupe, A à gauche de la jointure, B à droite.
    let m = z.mix(z.cut).unwrap();
    let (w, h) = (40, 3);
    let img = blend_images(&m, Some(&plein(ROUGE, w * h)), Some(&plein(BLEU, w * h)), [w, h]);
    let joint = ((1.0 + m.shift_a) * w as f32) as usize;
    assert!(joint > 2 && joint < w - 2, "{joint}");
    assert!(proche(img[w + joint - 2], ROUGE, 0) && proche(img[w + joint + 2], BLEU, 0));
    assert!(proche(img[w], ROUGE, 0) && proche(img[2 * w - 1], BLEU, 0));
    // Un plan sortant qui a un bord ne laisse pas voir au-delà : ce qui glisse est son propre cadre.
    let mut a = plein(ROUGE, w * h);
    for y in 0..h {
        a[y * w + w - 1] = Color32::from_rgb(0, 255, 0);
    }
    let img = blend_images(&m, Some(&a), Some(&plein(BLEU, w * h)), [w, h]);
    let premier_b = (0..w).find(|&x| proche(img[w + x], BLEU, 0)).expect("B se voit");
    assert!(proche(img[w + premier_b - 1], [0, 255, 0], 0), "le bord droit de A arrive à la jointure");
}

#[test]
fn le_raccord_file_va_tres_vite_au_milieu_et_y_est_flou() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    doc.set_transition(1, Kind::Whip).expect("un raccord filé");
    let z = zones(&doc)[0];
    assert_eq!(z.len(), 8);
    let mixes: Vec<Mix> = (z.start..z.end).map(|g| z.mix(g).unwrap()).collect();
    let pas = |i: usize| (mixes[i + 1].shift_a - mixes[i].shift_a).abs();
    assert!(pas(3) > 3.0 * pas(0), "plus vite au milieu qu'au début : {} / {}", pas(3), pas(0));
    let (debut, milieu) = (mixes[0].blur, mixes[3].blur.max(mixes[4].blur));
    assert!(milieu > 0.08 && milieu <= transitions::FLOU_FILE + 1e-6 && debut < 0.03, "{debut} {milieu}");
    // Le flou mêle les pixels voisins : une image moitié blanche, moitié noire a du gris près de sa limite.
    let (w, h) = (60, 2);
    let mut a = plein([0, 0, 0], w * h);
    for y in 0..h {
        for x in 0..w / 2 {
            a[y * w + x] = Color32::WHITE;
        }
    }
    let m = Mix { shift_a: 0.0, shift_b: 1.0, ..mixes[3] };
    let img = blend_images(&m, Some(&a), Some(&a), [w, h]);
    let gris = img[w / 2].to_array()[0];
    assert!(gris > 40 && gris < 215, "gris au milieu de la traînée : {gris}");
    assert_eq!(img[0].to_array()[0], 255, "loin de la limite, blanc");
}

#[test]
fn le_flash_blanchit_les_images_qui_touchent_la_coupe() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    doc.set_transition(1, Kind::Flash).expect("un flash");
    let z = zones(&doc)[0];
    assert_eq!((z.start, z.cut, z.end), (18, 20, 22));
    let at = |g: usize| z.mix(g).unwrap();
    assert!(at(19).a.is_some() && at(19).b.is_none() && at(19).k == 1.0, "la dernière image de A : blanche");
    assert!(at(20).b.is_some() && at(20).a.is_none() && at(20).k == 1.0, "la première de B : blanche");
    assert_eq!((at(18).k, at(21).k), (0.5, 0.5));
    assert_eq!(at(20).color, [255, 255, 255]);
    let img = blend_images(&at(20), None, Some(&plein(BLEU, 12)), [4, 3]);
    assert!(img.iter().all(|p| proche(*p, [255, 255, 255], 0)));
}

#[test]
fn le_fondu_par_une_couleur_passe_par_sa_couleur_et_se_pose_au_bord_du_film() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    let id = doc.set_transition(1, Kind::Colour).expect("un fondu par une couleur");
    assert_eq!(zones(&doc)[0].couleur, COULEUR_DEFAUT, "le bleu nuit d'origine");
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().couleur = Some([200, 120, 30]);
    let z = zones(&doc)[0];
    let plus_fort = (z.start..z.end).map(|g| z.mix(g).unwrap()).max_by(|a, b| a.k.total_cmp(&b.k)).unwrap();
    assert!(plus_fort.k > 0.85 && plus_fort.color == [200, 120, 30], "{plus_fort:?}");
    let img = blend_images(&Mix { k: 1.0, ..plus_fort }, Some(&plein(ROUGE, 12)), Some(&plein(BLEU, 12)), [4, 3]);
    assert!(proche(img[5], [200, 120, 30], 1));
    // Au début et à la fin du film aussi (depuis la couleur, vers elle).
    assert!(doc.set_transition(0, Kind::Colour).is_some());
    assert!(doc.set_transition(2, Kind::Colour).is_some());
    // La poussée, non.
    assert!(doc.set_transition(0, Kind::Push).is_none());
}

#[test]
fn le_volet_par_un_objet_cache_la_coupe_derriere_un_tronc() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    doc.set_transition(1, Kind::Objet).expect("un volet par un objet");
    let z = zones(&doc)[0];
    let milieux: Vec<f32> = (z.start..z.end).map(|g| z.mix(g).unwrap().objet.unwrap()).collect();
    assert!(milieux.windows(2).all(|w| w[1] < w[0]), "l'objet va de droite à gauche");
    assert!(milieux[0] > 1.0 && *milieux.last().unwrap() < 0.0, "il entre et sort du cadre : {milieux:?}");
    // L'objet au milieu du cadre : A à gauche, B à droite, le tronc entre les deux.
    let m = Mix { objet: Some(0.5), ..z.mix(z.cut).unwrap() };
    let (w, h) = (100, 10);
    let img = blend_images(&m, Some(&plein(ROUGE, w * h)), Some(&plein(BLEU, w * h)), [w, h]);
    let y = 5 * w;
    assert!(proche(img[y + 5], ROUGE, 0) && proche(img[y + 95], BLEU, 0));
    assert!(proche(img[y + 50], OBJET_COULEUR, 0) && proche(img[y + 44], OBJET_COULEUR, 0), "le tronc");
    // Plus large en bas qu'en haut.
    let large = |row: usize| (0..w).filter(|&x| proche(img[row * w + x], OBJET_COULEUR, 2)).count();
    assert!(large(9) > large(0), "{} {}", large(9), large(0));
}

#[test]
fn le_fichier_reste_en_version_1_sans_le_lot_4_et_garde_tout_avec_lui() {
    let mut doc = film_uni(&[ROUGE, BLEU, ROUGE], 20);
    doc.set_transition(1, Kind::Dissolve);
    let texte = String::from_utf8(transitions::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(texte.contains("\"version\": 1"), "{texte}");
    let id = doc.set_transition(2, Kind::Colour).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().couleur = Some([10, 20, 30]);
    doc.set_transition(0, Kind::White);
    let texte = String::from_utf8(transitions::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(texte.contains("\"version\": 2") && texte.contains("\"couleur\""), "{texte}");
    let relu = transitions::parse(&texte).unwrap();
    let mut attendu = doc.transitions.clone();
    attendu.sort_by_key(|t| t.id);
    let mut relu_trie = relu.clone();
    relu_trie.sort_by_key(|t| t.id);
    assert_eq!(relu_trie, attendu);
    // Une sorte qui ne se pose qu'entre deux plans, trouvée au bord du film dans un fichier : un fondu au noir.
    let abime = r#"{"format": "shot-transitions", "version": 2, "transitions": [{"id": 4, "entrant": 7, "sorte": "poussee", "images": 9}]}"#;
    let t: Vec<Transition> = transitions::parse(abime).unwrap();
    assert_eq!((t[0].kind, t[0].frames), (Kind::Black, 9));
    // Toutes les sortes passent par leur nom.
    for k in Kind::ALL {
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(serde_json::from_str::<Kind>(&json).unwrap(), k);
    }
}

#[test]
fn chaque_nouvelle_sorte_a_sa_commande() {
    let mut doc = film_uni(&[ROUGE, BLEU], 20);
    doc.set_transition(1, Kind::Dissolve);
    let mut app = app_avec(doc);
    app.cur_panel = 0;
    app.cur_frame = 18;
    for (action, kind) in [
        ("transition.poussee", Kind::Push),
        ("transition.raccord_file", Kind::Whip),
        ("transition.flash", Kind::Flash),
        ("transition.couleur", Kind::Colour),
        ("transition.objet", Kind::Objet),
    ] {
        app.run_action(action);
        assert_eq!(app.doc.transitions.len(), 1);
        assert_eq!(app.doc.transitions[0].kind, kind, "{action}");
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == action), "{action} dans la palette");
    }
}

#[test]
fn un_clic_sur_une_vignette_pose_la_transition_sur_la_coupe_la_plus_proche() {
    let doc = film_uni(&[ROUGE, BLEU, ROUGE], 20);
    let mut app = app_avec(doc);
    (app.cur_panel, app.cur_frame) = (1, 17);
    let etapes = app.history.undo_len();
    app.poser_transition_a_la_tete(Kind::Objet);
    let t = &app.doc.transitions[0];
    assert_eq!((t.kind, t.out, t.inn), (Kind::Objet, Some(app.doc.panels[1].id), Some(app.doc.panels[2].id)));
    assert_eq!(app.history.undo_len(), etapes + 1, "une étape d'annulation");
    // Au bord du film, une sorte qui n'y est pas permise ne pose rien, et le dit.
    (app.cur_panel, app.cur_frame) = (0, 0);
    app.trans.sel = None;
    app.poser_transition_a_la_tete(Kind::Push);
    assert_eq!(app.doc.transitions.len(), 1, "{}", app.status);
    assert!(app.status.contains("entre deux plans"), "{}", app.status);
}

#[test]
fn la_coupe_la_plus_proche_du_pointeur() {
    let blocs = [Rect::from_min_max(pos2(10.0, 0.0), pos2(50.0, 10.0)), Rect::from_min_max(pos2(52.0, 0.0), pos2(90.0, 10.0))];
    let c = |x: f32| crate::transitionsui::coupe_la_plus_proche(&blocs, x).map(|(c, _)| c);
    assert_eq!((c(0.0), c(40.0), c(55.0), c(80.0), c(200.0)), (Some(0), Some(1), Some(1), Some(2), Some(2)));
    assert_eq!(crate::transitionsui::coupe_la_plus_proche(&[], 5.0), None);
}

/// Ce que la carte graphique affiche d'une image de transition (la feuille entière, 1 pixel pour 1), le cadre filmé étant
/// celui du document.
fn gpu_mix(rs: &eframe::egui_wgpu::RenderState, gpu: &mut crate::gpu::GpuCanvas, doc: &Document, mix: &Mix) -> Vec<Color32> {
    use crate::gpu::{DisplayMix, MixLayer};
    use eframe::egui::vec2;
    let plan_of = |at: (usize, usize)| crate::render::panel_plan_at(doc, at.0, at.1, None);
    let first = plan_of(mix.a.or(mix.b).unwrap());
    let second = mix.a.and(mix.b).map(plan_of);
    let mut both = first.clone();
    if let Some(s) = &second {
        both.items.extend(s.items.iter().cloned());
    }
    let resolved = gpu.sync(doc, &both, true, &[], &[], 0);
    let n = first.items.len();
    let (w, h) = (doc.width as u32, doc.height as u32);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32));
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    let fr = doc.frame_rect();
    let sheet = doc.width as f32;
    let display = DisplayMix {
        wipe: mix.wipe,
        m: mix.m,
        couleur: mix.color,
        k: mix.k,
        wipe_u: [fr.left() / sheet, fr.right() / sheet],
        glisse: mix.glisse,
        shift_a: mix.shift_a,
        shift_b: mix.shift_b,
        blur: mix.blur,
        objet: mix.objet,
        vertical: mix.vertical,
        wipe_depuis: mix.wipe_depuis,
        iris: mix.iris,
        horloge: mix.horloge,
        zoom: mix.zoom,
        zoom_centre: mix.zoom_centre,
        cadre: [fr.left_top(), fr.right_top(), fr.right_bottom(), fr.left_bottom()],
    };
    let a = MixLayer { plan: &first, resolved: &resolved[..n], corners, ws: None };
    let b = second.as_ref().map(|p| MixLayer { plan: p, resolved: &resolved[n..], corners, ws: None });
    let callback = gpu.paint_mix(a, b, Some(&display), rect, [doc.width, doc.height]);
    crate::gpu_tests::read_callback(rs, w, h, rect, callback)
}

#[test]
fn la_carte_graphique_pousse_cache_et_colore_comme_le_processeur() {
    let Some(rs) = crate::gpu_tests::headless() else {
        eprintln!("aucune carte graphique : essai sauté");
        return;
    };
    let doc = film_uni(&[ROUGE, BLEU], 4);
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    let at = |img: &[Color32], x: usize, y: usize| img[y * doc.width + x];
    let fr = doc.frame_rect();
    let (x0, xm, x1, ym) = (fr.left() as usize, fr.center().x as usize, fr.right() as usize, fr.center().y as usize);
    // La poussée à mi-chemin : A dans la moitié gauche du cadre, B dans la droite.
    let pousse = Mix { a: Some((0, 0)), b: Some((1, 0)), m: 1.0, glisse: true, shift_a: -0.5, shift_b: 0.5, ..Mix::default() };
    let img = gpu_mix(&rs, &mut gpu, &doc, &pousse);
    assert!(proche(at(&img, x0 + 4, ym), ROUGE, 2), "{:?}", at(&img, x0 + 4, ym));
    assert!(proche(at(&img, x1 - 4, ym), BLEU, 2), "{:?}", at(&img, x1 - 4, ym));
    // Le raccord filé (flou) : les mêmes deux moitiés, loin de la jointure.
    let file = Mix { blur: 0.05, ..pousse };
    let img = gpu_mix(&rs, &mut gpu, &doc, &file);
    assert!(proche(at(&img, xm - 10, ym), ROUGE, 3) && proche(at(&img, xm + 10, ym), BLEU, 3), "{:?} {:?}", at(&img, xm - 10, ym), at(&img, xm + 10, ym));
    // Le volet par un objet au milieu : le tronc sur la jointure.
    let objet = Mix { a: Some((0, 0)), b: Some((1, 0)), m: 1.0, objet: Some(0.5), ..Mix::default() };
    let img = gpu_mix(&rs, &mut gpu, &doc, &objet);
    assert!(proche(at(&img, xm, ym), OBJET_COULEUR, 2), "{:?}", at(&img, xm, ym));
    assert!(proche(at(&img, x0 + 2, ym), ROUGE, 2) && proche(at(&img, x1 - 2, ym), BLEU, 2));
    // La couche d'une couleur choisie (mode 3) : la couleur même.
    let couleur = Mix { a: Some((0, 0)), k: 1.0, color: [200, 120, 30], ..Mix::default() };
    let img = gpu_mix(&rs, &mut gpu, &doc, &couleur);
    assert!(proche(at(&img, xm, ym), [200, 120, 30], 1), "{:?}", at(&img, xm, ym));
}

#[test]
fn les_vignettes_des_transitions_se_peignent_dans_le_panneau() {
    let doc = film_uni(&[ROUGE, BLEU], 20);
    let mut b = crate::perf_tests::banc_taille(doc, false, 1600, 2600);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Montage);
    b.garder_formes = true;
    b.chauffer(6);
    let textes = crate::blocscamera_tests::textes_peints(&b);
    for k in Kind::ALL {
        assert!(textes.iter().any(|(t, _)| t == k.name()), "la vignette « {} » dans le panneau", k.name());
    }
    // Survolée, une vignette joue sa transition sans rien casser.
    let (_, r) = textes.iter().find(|(t, _)| t == Kind::Objet.name()).unwrap().clone();
    let p = pos2(r.left() + 60.0, r.center().y - 46.0);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    for _ in 0..30 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
}

/// Un film de deux plans dessinés, assez grands pour se lire à l'écran : A, des bandes rouges en biais sur du blanc ; B, un rond
/// blanc sur du bleu.
fn film_dessine() -> Document {
    let mut doc = Document::with_frame(480, 270, 40);
    doc.add_scene_after(0);
    let (w, h) = (doc.width, doc.height);
    for p in 0..2 {
        let id = doc.new_cel(LayerKind::Bitmap);
        let r = &mut doc.cels.get_mut(&id).unwrap().raster;
        for y in 0..h {
            for x in 0..w {
                let c = if p == 0 {
                    if (x + y) / 24 % 2 == 0 { [220, 50, 30] } else { [250, 248, 240] }
                } else {
                    let (dx, dy) = (x as f32 - w as f32 / 2.0, y as f32 - h as f32 / 2.0);
                    if dx.hypot(dy) < 80.0 { [250, 250, 250] } else { [40, 70, 200] }
                };
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = Color32::from_rgb(c[0], c[1], c[2]);
            }
        }
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        l.frames = vec![Some(id); 48];
        doc.panels[p].layers = vec![l];
        doc.panels[p].length = 48;
    }
    doc
}

/// Images de contrôle (partie 175), dans `FEANOR_CAPTURE_DIR` : à la coupe, la poussée, le raccord filé, le flash, le fondu par
/// une couleur et le volet par un objet, tels que l'écran les montre ; le panneau Préréglages ouvert sur le Montage ; une
/// transition glissée au-dessus d'une coupe. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn transitions_lot4_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let sauver = |b: &crate::perf_tests::Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("LOT4 pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("LOT4 {nom}");
    };
    let mut b = crate::perf_tests::banc(film_dessine(), true);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Montage);
    for (kind, decalage, nom) in [
        (Kind::Push, 0i64, "lot4-1-poussee.png"),
        (Kind::Whip, 0, "lot4-2-raccord-file.png"),
        (Kind::Flash, 0, "lot4-3-flash.png"),
        (Kind::Colour, -1, "lot4-4-couleur.png"),
        (Kind::Objet, 0, "lot4-5-objet.png"),
    ] {
        b.app.doc.transitions.clear();
        b.app.doc.set_transition(1, kind);
        let cut = 48i64 + decalage;
        let (p, f) = if cut < 48 { (0, cut as usize) } else { (1, (cut - 48) as usize) };
        (b.app.cur_panel, b.app.cur_frame) = (p, f);
        b.garder_formes = true;
        b.chauffer(6);
        sauver(&b, nom);
    }
    // Une transition glissée au-dessus de la coupe du montage.
    let blocs = crate::storyboard::BLOCS_DU_MONTAGE.with(|v| v.borrow().clone());
    if let Some(bloc) = blocs.get(1) {
        let p = pos2(bloc.left() + 6.0, bloc.center().y);
        b.image_avec(|ctx| eframe::egui::DragAndDrop::set_payload(ctx, crate::transitionsui::TransitionDrag { kind: Kind::Objet }));
        b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
        b.image(vec![Event::PointerMoved(p + eframe::egui::vec2(1.0, 0.0))], 1.0 / 60.0);
        sauver(&b, "lot4-6-glisse-sur-la-coupe.png");
    }
}
