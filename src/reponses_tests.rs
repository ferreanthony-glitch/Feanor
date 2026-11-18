//! Ses réponses (partie 177) : les neuf numéros restés sans réponse, puis « oui ». Trois transitions de plus (iris, volet en
//! horloge, zoom traversant), un sens pour le volet, la poussée et le raccord filé, un centre pour l'iris et le zoom ; six plans
//! types de plus (titre de séquence, plan manquant, ardoise, compte à rebours, mire, fin d'acte).

use eframe::egui::{pos2, Color32, Rect};

use crate::document::{Document, Layer, LayerKind};
use crate::planstypes::PlanType;
use crate::transitions::{blend_images, zones, Cote, Kind, Mix};

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

fn plein(c: [u8; 3], n: usize) -> Vec<Color32> {
    vec![Color32::from_rgb(c[0], c[1], c[2]); n]
}

const ROUGE: [u8; 3] = [200, 40, 0];
const BLEU: [u8; 3] = [100, 80, 250];

fn proche(got: Color32, want: [u8; 3], tol: i32) -> bool {
    let g = got.to_array();
    (0..3).all(|k| (g[k] as i32 - want[k] as i32).abs() <= tol)
}

fn app_avec(doc: Document) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("feanor-reponses-{}-{:?}", std::process::id(), std::thread::current().id()));
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

// ------------------------------------------------------------------------------------------ les transitions

#[test]
fn trois_transitions_de_plus_rangees_et_permises_ou_il_faut() {
    assert_eq!(Kind::ALL.len(), 13);
    for k in [Kind::Iris, Kind::Horloge, Kind::ZoomTraversant] {
        assert!(Kind::ALL.contains(&k) && k.de_ses_reponses() && k.allowed(true), "{k:?}");
    }
    assert!(Kind::Iris.allowed(false), "l'iris s'ouvre au début du film, se ferme à la fin");
    assert!(!Kind::Horloge.allowed(false) && !Kind::ZoomTraversant.allowed(false));
    assert!(Kind::Iris.a_un_centre() && Kind::ZoomTraversant.a_un_centre() && !Kind::Push.a_un_centre());
    assert_eq!((Kind::Wipe.cote_d_origine(), Kind::Push.cote_d_origine(), Kind::Whip.cote_d_origine(), Kind::Iris.cote_d_origine()), (Some(Cote::Gauche), Some(Cote::Droite), Some(Cote::Droite), None));
}

#[test]
fn l_iris_se_ferme_sur_son_centre_puis_se_rouvre_sur_le_plan_suivant() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    let id = doc.set_transition(1, Kind::Iris).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().centre = Some([0.25, 0.5]);
    let z = zones(&doc)[0];
    let mixes: Vec<Mix> = (z.start..z.end).map(|g| z.mix(g).unwrap()).collect();
    let rayons: Vec<f32> = mixes.iter().map(|m| m.iris.unwrap()[2]).collect();
    let milieu = mixes.len() / 2;
    assert!(rayons[..milieu].windows(2).all(|w| w[1] <= w[0]), "il se ferme : {rayons:?}");
    assert!(rayons[milieu..].windows(2).all(|w| w[1] >= w[0]), "il se rouvre : {rayons:?}");
    assert!(mixes[0].a.is_some() && mixes[0].b.is_none() && mixes.last().unwrap().b.is_some() && mixes.last().unwrap().a.is_none());
    assert!(mixes.iter().all(|m| m.iris.unwrap()[0] == 0.25 && m.iris.unwrap()[1] == 0.5), "sur son centre");
    assert!(rayons[milieu - 1].min(rayons[milieu]) < 0.1, "presque fermé au milieu : {rayons:?}");
    // Un rond ouvert sur le quart de la largeur : le plan dedans, du noir dehors.
    let m = Mix { iris: Some([0.25, 0.5, 0.2]), ..mixes[0] };
    let (w, h) = (40, 24);
    let img = blend_images(&m, Some(&plein(ROUGE, w * h)), None, [w, h]);
    assert!(proche(img[12 * w + 10], ROUGE, 0), "au centre");
    assert!(proche(img[12 * w + 38], [0, 0, 0], 0) && proche(img[w - 1], [0, 0, 0], 0), "dehors");
}

#[test]
fn l_iris_au_bord_du_film_s_ouvre_ou_se_ferme_seulement() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.set_transition(0, Kind::Iris).expect("au début");
    doc.set_transition(2, Kind::Iris).expect("à la fin");
    let zs = zones(&doc);
    let (debut, fin) = (zs[0], zs[1]);
    let r = |z: &crate::transitions::Zone, g: usize| z.mix(g).unwrap().iris.unwrap()[2];
    assert!(r(&debut, debut.start) < 0.05 && r(&debut, debut.end - 1) > r(&debut, debut.start), "il s'ouvre");
    assert!(r(&fin, fin.end - 1) == 0.0 && r(&fin, fin.start) > 0.5, "il se ferme, tout noir à la dernière image");
}

#[test]
fn le_volet_en_horloge_balaie_depuis_midi() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.set_transition(1, Kind::Horloge).unwrap();
    let z = zones(&doc)[0];
    let angles: Vec<f32> = (z.start..z.end).map(|g| z.mix(g).unwrap().horloge.unwrap()).collect();
    assert!(angles.windows(2).all(|w| w[1] > w[0]) && angles[0] > 0.0 && *angles.last().unwrap() < std::f32::consts::TAU);
    // Un quart de tour : B en haut à droite, A ailleurs.
    let m = Mix { horloge: Some(std::f32::consts::FRAC_PI_2), ..z.mix(z.start).unwrap() };
    let (w, h) = (40, 40);
    let img = blend_images(&m, Some(&plein(ROUGE, w * h)), Some(&plein(BLEU, w * h)), [w, h]);
    assert!(proche(img[5 * w + 35], BLEU, 0), "en haut à droite");
    assert!(proche(img[5 * w + 5], ROUGE, 0) && proche(img[35 * w + 35], ROUGE, 0), "en haut à gauche et en bas à droite");
}

#[test]
fn le_zoom_traversant_plonge_dans_le_plan_puis_ressort_du_suivant() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.set_transition(1, Kind::ZoomTraversant).unwrap();
    let z = zones(&doc)[0];
    let mixes: Vec<Mix> = (z.start..z.end).map(|g| z.mix(g).unwrap()).collect();
    let (premier, dernier) = (mixes[0], *mixes.last().unwrap());
    assert!(premier.zoom.unwrap()[0] < 1.2 && premier.m < 0.01, "il part du plan sortant presque tel quel");
    assert!(dernier.zoom.unwrap()[1] < 1.2 && dernier.m > 0.99, "il arrive sur le plan entrant presque tel quel");
    assert!(mixes.windows(2).all(|w| w[1].zoom.unwrap()[0] >= w[0].zoom.unwrap()[0]), "A grossit");
    // Grossi deux fois autour du centre : le coin du cadre montre ce qui était à mi-chemin du centre.
    let (w, h) = (40, 40);
    let mut a = plein(ROUGE, w * h);
    for y in 10..30 {
        for x in 10..30 {
            a[y * w + x] = Color32::from_rgb(0, 255, 0);
        }
    }
    let m = Mix { zoom: Some([2.0, 1.0]), zoom_centre: [0.5, 0.5], m: 0.0, ..premier };
    let img = blend_images(&m, Some(&a), Some(&plein(BLEU, w * h)), [w, h]);
    assert!(proche(img[w + 1], [0, 255, 0], 0), "le carré du milieu emplit le cadre");
}

#[test]
fn le_volet_et_la_poussee_viennent_de_chacun_des_quatre_cotes() {
    let (w, h) = (40, 40);
    let (a, b) = (plein(ROUGE, w * h), plein(BLEU, w * h));
    for (cote, cote_b, cote_a) in [(Cote::Gauche, (2, 20), (37, 20)), (Cote::Droite, (37, 20), (2, 20)), (Cote::Haut, (20, 2), (20, 37)), (Cote::Bas, (20, 37), (20, 2))] {
        let mut doc = film_uni(&[ROUGE, BLEU], 24);
        let id = doc.set_transition(1, Kind::Wipe).unwrap();
        doc.transitions.iter_mut().find(|t| t.id == id).unwrap().depuis = Some(cote);
        let z = zones(&doc)[0];
        let m = Mix { m: 0.4, ..z.mix(z.cut).unwrap() };
        let img = blend_images(&m, Some(&a), Some(&b), [w, h]);
        assert!(proche(img[cote_b.1 * w + cote_b.0], BLEU, 0), "volet {cote:?} : B de son côté");
        assert!(proche(img[cote_a.1 * w + cote_a.0], ROUGE, 0), "volet {cote:?} : A de l'autre");
        // La poussée, du même côté.
        let mut doc = film_uni(&[ROUGE, BLEU], 24);
        let id = doc.set_transition(1, Kind::Push).unwrap();
        doc.transitions.iter_mut().find(|t| t.id == id).unwrap().depuis = Some(cote);
        let z = zones(&doc)[0];
        let m = z.mix(z.cut).unwrap();
        assert_eq!(m.vertical, matches!(cote, Cote::Haut | Cote::Bas), "{cote:?}");
        let img = blend_images(&m, Some(&a), Some(&b), [w, h]);
        assert!(proche(img[cote_b.1 * w + cote_b.0], BLEU, 0), "poussée {cote:?} : B arrive de son côté");
        assert!(proche(img[cote_a.1 * w + cote_a.0], ROUGE, 0), "poussée {cote:?} : A sort par l'autre");
    }
    // Sans sens écrit, comme avant : le volet de la gauche, la poussée de la droite.
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.set_transition(1, Kind::Push).unwrap();
    let m = zones(&doc)[0].mix(zones(&doc)[0].cut).unwrap();
    assert!(!m.vertical && m.shift_b > 0.0);
}

#[test]
fn le_fichier_passe_en_version_3_seulement_quand_il_le_faut() {
    let mut doc = film_uni(&[ROUGE, BLEU, ROUGE], 24);
    doc.set_transition(1, Kind::Push);
    let texte = String::from_utf8(crate::transitions::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(texte.contains("\"version\": 2"), "{texte}");
    let id = doc.set_transition(2, Kind::Iris).unwrap();
    doc.transitions.iter_mut().find(|t| t.id == id).unwrap().centre = Some([0.3, 0.6]);
    doc.transitions[0].depuis = Some(Cote::Bas);
    let texte = String::from_utf8(crate::transitions::to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(texte.contains("\"version\": 3") && texte.contains("\"depuis\": \"bas\"") && texte.contains("\"centre\""), "{texte}");
    let mut relu = crate::transitions::parse(&texte).unwrap();
    relu.sort_by_key(|t| t.id);
    let mut attendu = doc.transitions.clone();
    attendu.sort_by_key(|t| t.id);
    assert_eq!(relu, attendu);
    // Un centre abîmé revient dans le cadre.
    let abime = r#"{"format": "shot-transitions", "version": 3, "transitions": [{"id": 4, "sortant": 1, "entrant": 2, "sorte": "iris", "images": 9, "centre": [3.0, -1.0]}]}"#;
    assert_eq!(crate::transitions::parse(abime).unwrap()[0].centre, Some([1.0, 0.0]));
    for k in Kind::ALL {
        assert_eq!(serde_json::from_str::<Kind>(&serde_json::to_string(&k).unwrap()).unwrap(), k);
    }
}

#[test]
fn les_trois_commandes_de_plus() {
    let mut app = app_avec(film_uni(&[ROUGE, BLEU], 24));
    app.cur_frame = 20;
    for (action, kind) in [("transition.iris", Kind::Iris), ("transition.horloge", Kind::Horloge), ("transition.zoom_traversant", Kind::ZoomTraversant)] {
        app.run_action(action);
        assert_eq!(app.doc.transitions[0].kind, kind, "{action}");
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == action));
    }
}

/// Ce que la carte graphique affiche d'une image de transition (la feuille entière, 1 pixel pour 1).
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
fn la_carte_graphique_fait_l_iris_l_horloge_le_zoom_et_les_quatre_sens() {
    let Some(rs) = crate::gpu_tests::headless() else {
        eprintln!("aucune carte graphique : essai sauté");
        return;
    };
    let doc = film_uni(&[ROUGE, BLEU], 4);
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    let at = |img: &[Color32], x: f32, y: f32| img[y as usize * doc.width + x as usize];
    let fr = doc.frame_rect();
    let p = |fx: f32, fy: f32| (fr.left() + fx * fr.width(), fr.top() + fy * fr.height());
    let deux = Mix { a: Some((0, 0)), b: Some((1, 0)), m: 1.0, ..Mix::default() };
    // L'iris : le plan dans le rond, du noir dehors.
    let iris = Mix { b: None, iris: Some([0.5, 0.5, 0.2]), color: [0, 0, 0], ..deux };
    let img = gpu_mix(&rs, &mut gpu, &doc, &iris);
    let (cx, cy) = p(0.5, 0.5);
    let (ex, ey) = p(0.05, 0.1);
    assert!(proche(at(&img, cx, cy), ROUGE, 2) && proche(at(&img, ex, ey), [0, 0, 0], 2), "{:?} {:?}", at(&img, cx, cy), at(&img, ex, ey));
    // L'horloge à un quart de tour : B en haut à droite.
    let horloge = Mix { horloge: Some(std::f32::consts::FRAC_PI_2), ..deux };
    let img = gpu_mix(&rs, &mut gpu, &doc, &horloge);
    let ((hx, hy), (gx, gy)) = (p(0.8, 0.2), p(0.2, 0.8));
    assert!(proche(at(&img, hx, hy), BLEU, 2) && proche(at(&img, gx, gy), ROUGE, 2), "{:?} {:?}", at(&img, hx, hy), at(&img, gx, gy));
    // Le volet depuis la droite, à moitié : B à droite.
    let volet = Mix { wipe: true, m: 0.5, wipe_depuis: Cote::Droite, ..deux };
    let img = gpu_mix(&rs, &mut gpu, &doc, &volet);
    let ((dx, dy), (lx, ly)) = (p(0.85, 0.5), p(0.15, 0.5));
    assert!(proche(at(&img, dx, dy), BLEU, 2) && proche(at(&img, lx, ly), ROUGE, 2));
    // La poussée depuis le bas, à moitié : B en bas.
    let pousse = Mix { glisse: true, vertical: true, shift_a: -0.5, shift_b: 0.5, ..deux };
    let img = gpu_mix(&rs, &mut gpu, &doc, &pousse);
    let ((bx, by), (tx, ty)) = (p(0.5, 0.85), p(0.5, 0.15));
    assert!(proche(at(&img, bx, by), BLEU, 2) && proche(at(&img, tx, ty), ROUGE, 2), "{:?} {:?}", at(&img, bx, by), at(&img, tx, ty));
    // Le zoom, B entier par-dessus.
    let zoom = Mix { zoom: Some([3.0, 1.0]), zoom_centre: [0.5, 0.5], m: 1.0, ..deux };
    let img = gpu_mix(&rs, &mut gpu, &doc, &zoom);
    assert!(proche(at(&img, cx, cy), BLEU, 2));
}

// ------------------------------------------------------------------------------------------ les plans types

#[test]
fn six_plans_types_de_plus_et_tous_ses_oui() {
    assert_eq!(PlanType::ALL.len(), 24);
    let numeros: std::collections::HashSet<&str> = PlanType::ALL.iter().map(|t| t.numero()).collect();
    for n in ["P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8", "P9", "P10", "P11", "P12", "P13", "P14", "P15", "P16", "P17", "P18"] {
        assert!(numeros.contains(n), "{n}");
    }
}

#[test]
fn le_titre_de_sequence_ouvre_la_sequence_d_apres_et_se_remplit_tout_seul() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.add_sequence_after(1);
    let seq_apres = doc.sequence_of(2);
    let i = doc.poser_plan_type(PlanType::TitreSequence, 2).unwrap();
    assert_eq!(i, 2);
    assert_eq!(doc.sequence_of(2), seq_apres, "dans la séquence qu'il ouvre");
    let l = doc.panels[2].layers.iter().find(|l| l.name == "Titre de séquence").unwrap();
    let inc = l.overlay().unwrap();
    assert_eq!(inc.source, crate::incrustation::Source::NomSequence);
    let texte = crate::incrustation::text_for(inc, &doc, 2, 0, 0);
    assert_eq!(texte, format!("Séquence {}", doc.sequence_label(seq_apres)));
}

#[test]
fn le_plan_manquant_dit_son_numero_et_sa_duree() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    let i = doc.poser_plan_type(PlanType::PlanManquant, 1).unwrap();
    let inc = doc.panels[i].layers.iter().find(|l| l.name == "Plan manquant").unwrap().overlay().unwrap().clone();
    let texte = |d: &Document| crate::incrustation::text_for(&inc, d, i, 0, 0);
    assert_eq!(texte(&doc), format!("{} — À VENIR\n3 s", crate::storyboard::panel_title(&doc, i)));
    // (Le film d'essai est à 12 images par seconde.)
    doc.panels[i].set_length(30);
    assert!(texte(&doc).ends_with("2 s 6 images"), "la durée suit le plan : {}", texte(&doc));
}

#[test]
fn l_ardoise_se_remplit_depuis_le_projet() {
    let mut doc = film_uni(&[ROUGE, BLEU], 48);
    (doc.properties.title, doc.properties.serie, doc.properties.episode) = ("Brawlidays".into(), "Les vacances".into(), "3".into());
    let i = doc.poser_plan_type(PlanType::Ardoise, 0).unwrap();
    let inc = doc.panels[i].layers.iter().find(|l| l.name == "Ardoise").unwrap().overlay().unwrap().clone();
    let texte = crate::incrustation::text_for(&inc, &doc, i, 0, 0);
    let total = doc.total_frames() / 12;
    for bout in ["Brawlidays", "Série : Les vacances", "Épisode : 3", "Version : 1", &format!("Durée du film : 0:{total:02}")] {
        assert!(texte.contains(bout), "{bout} dans « {texte} »");
    }
}

#[test]
fn le_compte_a_rebours_a_ses_chiffres_son_noir_et_son_bip_sur_le_2() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.fps = 24;
    let i = doc.poser_plan_type(PlanType::CompteARebours, 1).unwrap();
    let f = 24;
    let p = &doc.panels[i];
    assert_eq!(p.length, 8 * f);
    let chiffres = p.layers.iter().find(|l| l.name == "Chiffres").unwrap();
    let mut suite = Vec::new();
    for c in chiffres.frames.iter().flatten() {
        if suite.last() != Some(c) {
            suite.push(*c);
        }
    }
    assert_eq!(suite.len(), 7, "8, 7, 6, 5, 4, 3, puis 2");
    assert!(chiffres.frames[6 * f].is_some() && chiffres.frames.get(6 * f + 1).copied().flatten().is_none(), "le 2, une seule image");
    let noir = p.layers.iter().find(|l| l.name == "Noir").unwrap();
    assert!(noir.opacity_at(6 * f) < 0.01 && noir.opacity_at(6 * f + 1) > 0.99 && noir.opacity_at(8 * f - 1) > 0.99, "du noir après le 2");
    // Le bip : une image de son, sur le 2.
    let debut = doc.panel_start(i);
    let clips: Vec<_> = doc.audio_tracks.iter().flat_map(|t| t.clips.iter()).collect();
    assert_eq!(clips.len(), 1);
    assert_eq!((clips[0].start, clips[0].end()), (debut + 6 * f, debut + 6 * f + 1));
    let son = &doc.sounds[&clips[0].sound];
    assert!(son.bytes.starts_with(b"RIFF") && son.name.contains("1 kHz"));
}

#[test]
fn la_mire_a_son_son_de_1_khz_sur_tout_le_plan() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    let i = doc.poser_plan_type(PlanType::Mire, 2).unwrap();
    let n = doc.panels[i].length;
    let clips: Vec<_> = doc.audio_tracks.iter().flat_map(|t| t.clips.iter()).collect();
    assert_eq!(clips.len(), 1);
    assert_eq!((clips[0].start, clips[0].end()), (doc.panel_start(i), doc.panel_start(i) + n));
    // Le son lui-même : une sinusoïde à 1 kHz (vingt échantillons de 48 kHz par période ; elle repasse par zéro toutes les 24).
    let w = crate::planstypes::wav_1khz(0.01, 0.5);
    let ech = |k: usize| i16::from_le_bytes([w[44 + 2 * k], w[45 + 2 * k]]);
    assert_eq!(ech(0), 0);
    assert!(ech(12) > 16000, "le haut de la vague, au quart de la période : {}", ech(12));
    assert!(ech(24).abs() < 50, "la demi-période");
}

#[test]
fn la_fin_d_acte_compte_celles_d_avant() {
    let mut doc = film_uni(&[ROUGE, BLEU], 24);
    doc.poser_plan_type(PlanType::FinDActe, 1);
    let i = doc.poser_plan_type(PlanType::FinDActe, doc.panels.len()).unwrap();
    let l = doc.panels[i].layers.iter().find(|l| l.name == "Fin d'acte").unwrap();
    let id = l.frames[0].unwrap();
    let textes: Vec<String> = doc.cels[&id].vector.as_ref().unwrap().elements.iter().filter_map(|e| if let crate::vector::Element::Text { text, .. } = e { Some(text.clone()) } else { None }).collect();
    assert_eq!(textes, vec!["Fin de l'acte 2".to_string()]);
}

#[test]
fn annuler_le_compte_a_rebours_retire_aussi_son_bip() {
    let mut app = app_avec(film_uni(&[ROUGE, BLEU], 24));
    app.poser_plan_type_a_la_tete(PlanType::CompteARebours);
    assert_eq!(app.doc.panels.len(), 3);
    assert_eq!(app.doc.audio_tracks.iter().map(|t| t.clips.len()).sum::<usize>(), 1);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), 2);
    assert_eq!(app.doc.audio_tracks.iter().map(|t| t.clips.len()).sum::<usize>(), 0, "le bip part avec son plan");
}

/// Images de contrôle (partie 177), dans `FEANOR_CAPTURE_DIR` : l'iris, le volet en horloge, le zoom traversant, un volet du
/// haut et une poussée du bas à mi-chemin ; les six plans types neufs. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn reponses_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let mut doc = Document::with_frame(480, 270, 24);
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
    doc.properties.title = "Brawlidays".into();
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Montage);
    let sauver = |b: &crate::perf_tests::Banc, nom: &str| {
        if let Some(rgba) = b.capturer() {
            image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
            println!("REPONSES {nom}");
        }
    };
    for (kind, depuis, centre, decalage, nom) in [
        (Kind::Iris, None, Some([0.62, 0.4]), -3i64, "rep-1-iris.png"),
        (Kind::Horloge, None, None, 0, "rep-2-horloge.png"),
        (Kind::ZoomTraversant, None, Some([0.62, 0.4]), -2, "rep-3-zoom.png"),
        (Kind::Wipe, Some(Cote::Haut), None, 0, "rep-4-volet-du-haut.png"),
        (Kind::Push, Some(Cote::Bas), None, 0, "rep-5-poussee-du-bas.png"),
    ] {
        b.app.doc.transitions.clear();
        let id = b.app.doc.set_transition(1, kind).unwrap();
        if let Some(t) = b.app.doc.transitions.iter_mut().find(|t| t.id == id) {
            t.depuis = depuis;
            t.centre = centre;
        }
        let g = 48i64 + decalage;
        (b.app.cur_panel, b.app.cur_frame) = if g < 48 { (0, g as usize) } else { (1, (g - 48) as usize) };
        b.chauffer(6);
        sauver(&b, nom);
    }
    b.app.doc.transitions.clear();
    b.app.camera_view = true;
    for (k, t) in [PlanType::TitreSequence, PlanType::PlanManquant, PlanType::Ardoise, PlanType::CompteARebours, PlanType::Mire, PlanType::FinDActe].into_iter().enumerate() {
        b.app.poser_plan_type(t, 1);
        let n = b.app.doc.panels[b.app.cur_panel].frame_count();
        b.app.cur_frame = if t == PlanType::CompteARebours { 30 } else { n / 2 };
        b.chauffer(6);
        sauver(&b, &format!("rep-plan-{}-{}.png", k + 1, t.numero()));
        b.app.run_action("edition.annuler");
    }
}
