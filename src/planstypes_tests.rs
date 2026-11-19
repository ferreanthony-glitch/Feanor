//! Les plans types (lot 4 des préréglages, partie 176) : chacun pose un plan neuf tout prêt sur une coupe (ou des calques sur
//! un plan), fait de calques ordinaires ; une étape d'annulation ; ils passent par le projet ; leurs vignettes se peignent.

use std::collections::HashSet;

use eframe::egui::Color32;

use crate::document::{Document, Layer, LayerKind};
use crate::planstypes::PlanType;
use crate::vector::Element;

/// Un film de plans d'une couleur chacun, de `len` images.
fn film_uni(colors: &[[u8; 3]], len: usize) -> Document {
    film_uni_de(colors, len, [64, 36])
}

fn film_uni_de(colors: &[[u8; 3]], len: usize, cadre: [usize; 2]) -> Document {
    let mut doc = Document::with_frame(cadre[0], cadre[1], 24);
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

/// Les textes des dessins du calque `l`.
fn textes(doc: &Document, l: &Layer) -> Vec<String> {
    let mut out = Vec::new();
    let mut vus = HashSet::new();
    for id in l.frames.iter().flatten() {
        if !vus.insert(*id) {
            continue;
        }
        if let Some(v) = doc.cels.get(id).and_then(|c| c.vector.as_ref()) {
            for e in &v.elements {
                if let Element::Text { text, .. } = e {
                    out.push(text.clone());
                }
            }
        }
    }
    out
}

fn calque<'a>(doc: &'a Document, plan: usize, nom: &str) -> &'a Layer {
    doc.panels[plan].layers.iter().find(|l| l.name == nom).unwrap_or_else(|| panic!("le calque « {nom} » du plan {plan}"))
}

#[test]
fn les_dix_huit_plans_types_sont_ceux_de_sa_liste() {
    let noms: HashSet<&str> = PlanType::ALL.iter().map(|t| t.label()).collect();
    assert_eq!(noms.len(), 24, "dix-huit, puis vingt-quatre (partie 177)");
    let retenus: HashSet<String> = (1..=18).map(|k| format!("P{k}")).collect();
    let faits: HashSet<String> = PlanType::ALL.iter().map(|t| t.numero().to_string()).collect();
    assert_eq!(faits, retenus, "ses douze « Oui », puis ses six réponses : P1 à P18");
    let sur_un_plan: Vec<PlanType> = PlanType::ALL.into_iter().filter(|t| t.sur_un_plan()).collect();
    assert_eq!(sur_un_plan, vec![PlanType::LieuEtDate, PlanType::GeneriqueDebut]);
}

#[test]
fn chaque_plan_type_pose_un_plan_neuf_tout_pret_sur_la_coupe() {
    for t in PlanType::ALL.into_iter().filter(|t| !t.sur_un_plan()) {
        let mut doc = film_uni(&[[200, 40, 0], [100, 80, 250], [20, 160, 60]], 24);
        let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
        let i = doc.poser_plan_type(t, 2).unwrap_or_else(|| panic!("{t:?}"));
        assert_eq!(i, 2, "{t:?} : sur la coupe d'entre les plans 2 et 3");
        assert_eq!(doc.panels.len(), 4);
        let autres: Vec<u64> = doc.panels.iter().enumerate().filter(|(k, _)| *k != i).map(|(_, p)| p.id).collect();
        assert_eq!(autres, ids, "{t:?} : les autres plans ne bougent pas");
        let p = &doc.panels[i];
        assert_eq!(p.length, (t.secondes() * 24.0).round() as usize, "{t:?}");
        assert!(!p.layers.is_empty(), "{t:?}");
        for l in &p.layers {
            assert!(l.frames.len() <= p.length, "{t:?} : {} ne dépasse pas le plan", l.name);
            for id in l.frames.iter().flatten() {
                assert!(doc.cels.contains_key(id), "{t:?} : {} montre des dessins qui existent", l.name);
            }
        }
        // L'image du plan se calcule, et elle n'est pas vide (un carton, un titre, des cases…).
        let plan = crate::render::panel_plan_at(&doc, i, p.length / 2, doc.render_paper());
        let img = crate::render::camera_image(&doc, &plan, &doc.panel_camera(i, p.length / 2));
        assert_eq!(img.len(), doc.frame[0] * doc.frame[1]);
    }
    // Au début et à la fin du film aussi.
    let mut doc = film_uni(&[[200, 40, 0], [100, 80, 250], [20, 160, 60]], 24);
    assert_eq!(doc.poser_plan_type(PlanType::CartonNoir, 0), Some(0));
    let n = doc.panels.len();
    assert_eq!(doc.poser_plan_type(PlanType::CartonBlanc, n), Some(n));
    assert_eq!(doc.poser_plan_type(PlanType::LieuEtDate, 1), None, "le lieu et la date ne font pas un plan neuf");
}

#[test]
fn le_titre_reprend_celui_du_projet_et_s_allume_en_fondu() {
    let mut doc = film_uni(&[[200, 40, 0]], 24);
    doc.properties.title = "Brawlidays".into();
    let i = doc.poser_plan_type(PlanType::Titre, 0).unwrap();
    let l = calque(&doc, i, "Titre");
    assert_eq!(textes(&doc, l), vec!["Brawlidays".to_string()]);
    let n = doc.panels[i].length;
    assert!(l.opacity_at(0) < 0.01 && l.opacity_at(n / 2) > 0.99 && l.opacity_at(n - 1) < 0.01, "fondu d'entrée et de sortie");
    assert_eq!(calque(&doc, i, "Carton").unie, Some([8, 8, 10]), "sur un carton noir");
    // Sans titre dans les propriétés : « Titre du film », à retoucher.
    let mut doc = film_uni(&[[200, 40, 0]], 24);
    let i = doc.poser_plan_type(PlanType::Titre, 1).unwrap();
    assert_eq!(textes(&doc, calque(&doc, i, "Titre")), vec!["Titre du film".to_string()]);
}

#[test]
fn le_generique_de_fin_monte_du_bas_du_cadre_jusqu_a_sortir_par_le_haut() {
    let mut doc = film_uni(&[[200, 40, 0]], 24);
    doc.properties.author = "Anthony".into();
    let i = doc.poser_plan_type(PlanType::GeneriqueFin, 1).unwrap();
    let l = calque(&doc, i, "Générique");
    assert!(textes(&doc, l).iter().any(|t| t == "Anthony"), "le nom de l'auteur du projet");
    let n = doc.panels[i].length;
    assert_eq!(l.pos_keys.len(), 2);
    let fh = doc.frame_rect().height();
    assert_eq!((l.pos_keys[0].frame, l.pos_keys[0].t.y), (0, fh), "il part sous le cadre");
    assert_eq!(l.pos_keys[1].frame, n - 1);
    assert!(l.pos_keys[1].t.y < -fh * 0.5, "il sort par le haut");
}

#[test]
fn l_ecran_partage_a_une_case_par_calque_et_ses_separations() {
    for (t, cases) in [(PlanType::Partage2, 2), (PlanType::Partage3, 3), (PlanType::Partage4, 4)] {
        let mut doc = film_uni(&[[200, 40, 0]], 24);
        let i = doc.poser_plan_type(t, 1).unwrap();
        let noms: Vec<&str> = doc.panels[i].layers.iter().map(|l| l.name.as_str()).collect();
        let mut attendu: Vec<String> = (1..=cases).map(|k| format!("Case {k}")).collect();
        attendu.push("Séparations".into());
        assert_eq!(noms, attendu.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        // Chaque case montre un dessin vide sur tout le plan : ce qu'on y dessine tient le plan.
        let n = doc.panels[i].length;
        for l in &doc.panels[i].layers[..cases] {
            assert_eq!(l.frames.len(), n);
            assert!(l.frames.iter().all(|f| *f == l.frames[0] && f.is_some()));
        }
    }
}

#[test]
fn precedemment_et_a_suivre_reprennent_des_images_des_plans_d_a_cote() {
    let couleurs = [[200, 40, 0], [100, 80, 250], [20, 160, 60]];
    let mut doc = film_uni(&couleurs, 24);
    let i = doc.poser_plan_type(PlanType::Precedemment, 3).unwrap();
    let l = calque(&doc, i, "Images du film");
    let cels: Vec<_> = {
        let mut v = Vec::new();
        for id in l.frames.iter().flatten() {
            if v.last() != Some(id) {
                v.push(*id);
            }
        }
        v
    };
    assert_eq!(cels.len(), 3, "les trois plans d'avant, l'un après l'autre");
    let fr = doc.frame_rect();
    for (id, c) in cels.iter().zip(couleurs) {
        let r = &doc.cels[id].raster;
        let (x, y) = (fr.center().x as usize, fr.center().y as usize);
        let px = crate::render::sample(r, x as f32 + 0.5, y as f32 + 0.5).to_array();
        assert!((0..3).all(|k| (px[k] as i32 - c[k] as i32).abs() <= 2), "{px:?} / {c:?}");
    }
    assert_eq!(textes(&doc, calque(&doc, i, "Carton de série")), vec!["PRÉCÉDEMMENT…".to_string()]);
    // « À suivre » au début du film : les plans d'après.
    let mut doc = film_uni(&couleurs, 24);
    let i = doc.poser_plan_type(PlanType::ASuivre, 0).unwrap();
    assert_eq!(i, 0);
    assert!(doc.panels[0].layers.iter().any(|l| l.name == "Images du film"));
    // Sans plan d'à côté : le carton seul.
    let mut doc = film_uni(&couleurs, 24);
    let i = doc.poser_plan_type(PlanType::Precedemment, 0).unwrap();
    assert!(!doc.panels[i].layers.iter().any(|l| l.name == "Images du film"));
}

#[test]
fn la_carte_dessine_sa_ligne_puis_la_garde() {
    let mut doc = film_uni(&[[200, 40, 0]], 24);
    let i = doc.poser_plan_type(PlanType::Carte, 1).unwrap();
    let l = calque(&doc, i, "Trajet");
    let n = doc.panels[i].length;
    let traits = |f: usize| doc.cels[&l.frames[f].unwrap()].vector.as_ref().map_or(0, |v| v.elements.len());
    assert!(traits(0) < traits(n / 3) && traits(n / 3) < traits(2 * n / 3), "{} {} {}", traits(0), traits(n / 3), traits(2 * n / 3));
    assert_eq!(traits(2 * n / 3), traits(n - 1), "la ligne reste, finie");
}

#[test]
fn le_telephone_recoit_ses_messages_l_un_apres_l_autre() {
    let mut doc = film_uni(&[[200, 40, 0]], 24);
    let i = doc.poser_plan_type(PlanType::Telephone, 1).unwrap();
    let arrivees: Vec<usize> = (1..=3).map(|k| (0..doc.panels[i].length).find(|&f| calque(&doc, i, &format!("Message {k}")).opacity_at(f) > 0.5).unwrap()).collect();
    assert!(arrivees.windows(2).all(|w| w[1] > w[0]), "{arrivees:?}");
}

#[test]
fn le_lieu_et_la_date_s_ecrivent_lettre_a_lettre_en_bas_du_plan() {
    let mut doc = film_uni(&[[200, 40, 0], [100, 80, 250]], 48);
    let li = doc.poser_sur_le_plan(PlanType::LieuEtDate, 1).unwrap();
    assert_eq!(doc.panels.len(), 2, "pas de plan neuf");
    let l = &doc.panels[1].layers[li];
    assert_eq!(l.name, "Lieu et date");
    let inc = l.overlay().expect("une incrustation").clone();
    assert_eq!(inc.ancre, crate::incrustation::Ancre::BasGauche);
    assert_eq!(inc.lettre_a_lettre, 2, "à 24 images par seconde, douze lettres par seconde");
    assert_eq!(l.frames.len(), 48, "visible sur tout le plan");
    let texte = |f: usize| crate::incrustation::text_for(&inc, &doc, 1, f, 0);
    assert_eq!((texte(0), texte(1), texte(2), texte(5)), ("P".to_string(), "P".to_string(), "Pa".to_string(), "Par".to_string()));
    assert_eq!(texte(40), "Paris, 1962");
    // Sans lettre à lettre, tout d'un coup (comme avant).
    let d = crate::incrustation::Incrustation { lettre_a_lettre: 0, ..inc };
    assert_eq!(crate::incrustation::text_for(&d, &doc, 1, 0, 0), "Paris, 1962");
}

#[test]
fn le_generique_de_debut_montre_trois_noms_l_un_apres_l_autre() {
    let mut doc = film_uni(&[[200, 40, 0]], 72);
    doc.properties.author = "Anthony".into();
    let li = doc.poser_sur_le_plan(PlanType::GeneriqueDebut, 0).unwrap();
    let l = &doc.panels[0].layers[li];
    let mut suite = Vec::new();
    for id in l.frames.iter().flatten() {
        if suite.last() != Some(id) {
            suite.push(*id);
        }
    }
    assert_eq!(suite.len(), 3, "trois dessins, l'un après l'autre");
    assert!(textes(&doc, l).contains(&"Anthony".to_string()));
    // Chaque nom s'allume et s'éteint : l'opacité retombe à zéro entre deux noms.
    assert!(l.opacity_at(0) < 0.01 && l.opacity_at(12) > 0.99 && l.opacity_at(23) < 0.01 && l.opacity_at(36) > 0.99);
}

/// Une application sur le film, sans fenêtre.
fn app_avec(doc: Document) -> crate::app::App {
    let dir = std::env::temp_dir().join(format!("feanor-plans-types-{}-{:?}", std::process::id(), std::thread::current().id()));
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

#[test]
fn poser_un_plan_type_est_une_etape_d_annulation() {
    let mut app = app_avec(film_uni(&[[200, 40, 0], [100, 80, 250]], 24));
    let etapes = app.history.undo_len();
    app.poser_plan_type_a_la_tete(PlanType::Intertitre);
    assert_eq!(app.doc.panels.len(), 3);
    assert_eq!(app.cur_panel, 1, "juste après le plan courant, et il devient le plan courant");
    assert_eq!(app.layer().name, "Intertitre", "son texte est le calque courant");
    assert_eq!(app.history.undo_len(), etapes + 1);
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), 2, "Ctrl+Z le retire");
    // Sur le plan courant, pour le lieu et la date.
    app.cur_panel = 1;
    app.poser_plan_type_a_la_tete(PlanType::LieuEtDate);
    assert_eq!(app.doc.panels.len(), 2);
    assert_eq!(app.layer().name, "Lieu et date");
    app.run_action("edition.annuler");
    assert!(!app.doc.panels[1].layers.iter().any(|l| l.name == "Lieu et date"));
}

#[test]
fn le_format_60_garde_le_lettre_a_lettre_et_les_plans_types() {
    let dir = std::env::temp_dir().join(format!("feanor-plans-types-format-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("essai.shot");
    let mut doc = film_uni(&[[200, 40, 0], [100, 80, 250]], 48);
    doc.poser_plan_type(PlanType::GeneriqueFin, 2);
    doc.poser_plan_type(PlanType::Titre, 0);
    doc.poser_sur_le_plan(PlanType::LieuEtDate, 1);
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(&manifest).unwrap();
    assert!(texte.contains("\"version\": 61") && texte.contains("\"lettre_a_lettre\": 2"), "format 61");
    let lu = crate::project::load(&manifest).unwrap().doc;
    assert_eq!(lu.panels.len(), 4);
    let inc = |d: &Document| d.panels[2].layers.iter().find(|l| l.name == "Lieu et date").and_then(|l| l.overlay().cloned());
    assert_eq!(inc(&lu), inc(&doc), "l'incrustation revient avec son lettre à lettre");
    let generique = |d: &Document| d.panels[3].layers.iter().find(|l| l.name == "Générique").map(|l| (l.pos_keys.clone(), l.frames.len()));
    assert_eq!(generique(&lu), generique(&doc), "le générique garde sa montée");
    let titre = |d: &Document| d.panels[0].layers.iter().find(|l| l.name == "Titre").map(|l| l.opacity_keys.clone());
    assert_eq!(titre(&lu), titre(&doc), "le titre garde son fondu");
    // Une incrustation sans lettre à lettre ne l'écrit pas.
    let mut vieux = film_uni(&[[200, 40, 0]], 24);
    let pose = vieux.new_cel(LayerKind::Bitmap);
    let mut l = Layer::new_effect("Timecode", crate::calqueffet::CalqueEffet::incrustation(crate::incrustation::Incrustation::default(), "Timecode"));
    l.frames = vec![Some(pose); 24];
    vieux.panels[0].layers.push(l);
    crate::project::save(&vieux, &manifest, &mut Default::default()).unwrap();
    assert!(!std::fs::read_to_string(&manifest).unwrap().contains("lettre_a_lettre"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn les_vignettes_des_plans_types_se_peignent_dans_le_panneau() {
    let doc = film_uni(&[[200, 40, 0], [100, 80, 250]], 24);
    let mut b = crate::perf_tests::banc_taille(doc, false, 1600, 4000);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Montage);
    b.garder_formes = true;
    b.chauffer(6);
    let textes = crate::blocscamera_tests::textes_peints(&b);
    for t in PlanType::ALL {
        assert!(textes.iter().any(|(x, _)| x == t.label()), "la vignette « {} » dans le panneau", t.label());
    }
    // Chacune joue au survol sans rien casser.
    for t in PlanType::ALL {
        if let Some((_, r)) = textes.iter().find(|(x, _)| x == t.label()) {
            b.image(vec![eframe::egui::Event::PointerMoved(eframe::egui::pos2(r.left() + 60.0, r.center().y - 46.0))], 1.0 / 60.0);
            for _ in 0..6 {
                b.image(Vec::new(), 1.0 / 10.0);
            }
        }
    }
}

/// Images de contrôle (partie 176), dans `FEANOR_CAPTURE_DIR` : chaque plan type posé, à son image du milieu, en vue caméra ; le
/// panneau Préréglages ouvert sur les plans types. Sans carte graphique : rien.
#[test]
#[ignore = "écrit des captures : FEANOR_CAPTURE_DIR"]
fn plans_types_captures() {
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    let mut doc = film_uni_de(&[[214, 196, 160], [150, 175, 205]], 48, [960, 540]);
    doc.properties.title = "Brawlidays".into();
    doc.properties.author = "Anthony".into();
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.show_pane(crate::dock::Pane::Prereglages);
    crate::panneauprereglages::choisir_onglet(&b.app.ctx, crate::panneauprereglages::Onglet::Montage);
    b.app.camera_view = true;
    for (k, t) in PlanType::ALL.into_iter().enumerate() {
        let cible = if t.sur_un_plan() { 0 } else { 1 };
        b.app.poser_plan_type(t, cible);
        let n = b.app.doc.panels[b.app.cur_panel].frame_count();
        b.app.cur_frame = if t == PlanType::LieuEtDate { n / 3 } else { n / 2 };
        b.garder_formes = true;
        b.chauffer(6);
        let Some(rgba) = b.capturer() else { return println!("PLANS pas de carte graphique") };
        let nom = format!("plans-{:02}-{}.png", k + 1, t.numero());
        image::save_buffer(dossier.join(&nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("PLANS {nom} {}", t.label());
        b.app.run_action("edition.annuler");
    }
}
