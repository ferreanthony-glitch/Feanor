//! Essais des couleurs d'identification du décor 3D et de ses aspects (partie 199, `couleurs3d.rs`, `couleurs3dui.rs`,
//! `docs/INTERFACE.md`) : la palette, ses pastilles dans le sommaire et dans Objet, l'annulation, l'enregistrement (aller et retour,
//! un fichier d'avant, un lecteur d'avant), les copies, l'aspect de la vue et celui des plans (les couleurs rendues, la clé de
//! l'image, l'export), les cartes jamais teintes, l'œil et le cadenas gardés, les maillages jamais refaits.

use eframe::egui::{Modifiers, Rect};

use crate::bulles::vus;
use crate::couleurs3d::{self, Rendu, Teinte};
use crate::decor3d::{self, Place3d};
use crate::decor3drendu::{Aspect, Prise};
use crate::dock::Pane;
use crate::document::Document;
use crate::espace3d::Monde;
use crate::onglet3d::Ligne3d;
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::clic;
use crate::workspace::Espace;

fn ids(doc: &Document) -> Vec<u64> {
    doc.decor_du_plan(0).unwrap().objets.iter().map(|o| o.id).collect()
}

fn objet(doc: &Document, id: u64) -> &decor3d::Objet3d {
    doc.decor_du_plan(0).unwrap().objet(id).unwrap()
}

fn noms_des_etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn banc_3d() -> Banc {
    let mut b = banc_taille(crate::decor3d_tests::projet_a_deux_carres(), false, 1600, 1000);
    b.app.set_espace(Espace::TroisD);
    if let Some(k) = b.app.colonne.rang_de(Pane::Scene3d) {
        b.app.colonne.panneaux[k].hauteur = Some(330.0);
    }
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn dans(nom: &str, r: Rect) -> Rect {
    let v: Vec<Rect> = vus::ou(nom).into_iter().filter(|x| r.contains(x.center())).collect();
    assert_eq!(v.len(), 1, "« {nom} » dans {r:?} : {:?}", vus::ou(nom));
    v[0]
}

fn ligne(l: Ligne3d) -> Rect {
    crate::onglet3dui::LIGNES_S3D.with(|v| v.borrow().iter().find(|x| x.0 == l).map(|x| x.1)).unwrap_or_else(|| panic!("{l:?}"))
}

// ------------------------------------------------------------------------------------------------ la palette et ses pastilles

#[test]
fn the_palette_has_ten_named_colours_spread_in_lightness_and_none() {
    assert_eq!(couleurs3d::PALETTE.len(), 10);
    assert_eq!(couleurs3d::NOMS, ["Rouge", "Orange", "Jaune", "Vert", "Sarcelle", "Bleu", "Violet", "Rose", "Brun", "Gris clair"]);
    assert_eq!(couleurs3d::nom(None), "Aucune");
    assert_eq!(couleurs3d::nom(Some(5)), "Bleu");
    assert!(couleurs3d::couleur(10).is_none());
    // Des clartés étalées, du sombre au clair : on les distingue aussi par elle.
    let clarte = |c: [u8; 3]| {
        let l = couleurs3d::lineaire(c);
        0.2126 * l[0] + 0.7152 * l[1] + 0.0722 * l[2]
    };
    let mut l: Vec<f32> = couleurs3d::PALETTE.iter().map(|c| clarte(*c)).collect();
    l.sort_by(f32::total_cmp);
    assert!(l[9] - l[0] > 0.3, "{l:?}");
    for (i, a) in couleurs3d::PALETTE.iter().enumerate() {
        for b in &couleurs3d::PALETTE[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

#[test]
fn the_chips_in_the_outliner_and_in_object_set_the_colour_in_one_named_undo_step() {
    let mut b = banc_3d();
    let id = ids(&b.app.doc);
    assert_eq!(objet(&b.app.doc, id[0]).couleur, None, "un objet neuf n'a pas de couleur");
    // La puce de l'objet dans le sommaire : la palette ; « Bleu ».
    let r = ligne(Ligne3d::Objet(id[0]));
    clic(&mut b, dans("Couleur d'identification", r).center(), Modifiers::NONE);
    let bleu = *vus::ou("Couleur : Bleu").last().expect("la palette ouverte");
    let etapes = b.app.history.undo_len();
    clic(&mut b, bleu.center(), Modifiers::NONE);
    assert_eq!(objet(&b.app.doc, id[0]).couleur, Some(5));
    assert_eq!(b.app.history.undo_len(), etapes + 1, "une étape");
    assert!(noms_des_etapes(&b).iter().any(|n| n.contains("Couleur d'identification")), "{:?}", noms_des_etapes(&b));
    // Annuler la rend.
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(objet(&b.app.doc, id[0]).couleur, None);
    // Propriétés > Objet : la rangée « Couleur d'identification », en haut ; « Rouge ».
    b.app.e3_choisir_objet(id[1], false);
    b.app.colonne.montrer(Pane::Objet3d);
    b.chauffer(4);
    let rouge = vus::ou("Couleur : Rouge").into_iter().next().expect("la rangée de l'onglet Objet");
    clic(&mut b, rouge.center(), Modifiers::NONE);
    assert_eq!(objet(&b.app.doc, id[1]).couleur, Some(0));
    // « Aucune » l'enlève.
    let aucune = vus::ou("Couleur : Aucune").into_iter().next().unwrap();
    clic(&mut b, aucune.center(), Modifiers::NONE);
    assert_eq!(objet(&b.app.doc, id[1]).couleur, None);
}

// ------------------------------------------------------------------------------------------------ le projet

#[test]
fn colours_eye_and_padlock_are_saved_and_an_old_file_and_an_old_reader_still_work() {
    let dir = crate::decor3d_tests::dossier("couleurs199");
    let manifeste = dir.join("Film.shot");
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let id = ids(&doc);
    let seq = doc.sequence_of(0);
    {
        let d = doc.decors3d.par_sequence.get_mut(&seq).unwrap();
        d.objets[0].couleur = Some(3);
        d.objets[1].cache = true;
        d.objets[1].verrou = true;
        d.aspect.rendu = Rendu::Couleurs;
    }
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"couleur\": 3") && texte.contains("\"cache\": true") && texte.contains("\"verrou\": true") && texte.contains("\"rendu\": \"couleurs\""), "{texte}");
    assert!(texte.contains("\"version\": 2"), "le fichier des décors garde sa version");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = relu.doc.decor_du_plan(0).unwrap();
    assert_eq!((d.objet(id[0]).unwrap().couleur, d.objet(id[1]).unwrap().cache, d.objet(id[1]).unwrap().verrou, d.aspect.rendu), (Some(3), true, true, Rendu::Couleurs));
    // Un lecteur d'avant (sans ces champs, comme la partie 198) lit le fichier d'aujourd'hui : rien n'interdit les clés inconnues.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct ObjetAvant {
        id: u64,
        nom: String,
        #[serde(default)]
        fichier_d_origine: String,
        fichier: String,
        #[serde(flatten)]
        place: Place3d,
        #[serde(default)]
        origine: Option<[f32; 3]>,
        #[serde(default)]
        aimante: bool,
        #[serde(default)]
        allege: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct AspectAvant {
        facettes: bool,
        ciel: bool,
        brume: bool,
        ombres: bool,
        soleil_azimut: f32,
        soleil_hauteur: f32,
    }
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    for s in v["decors"].as_array().unwrap() {
        let _: AspectAvant = serde_json::from_value(s["aspect"].clone()).expect("l'aspect, lu par un lecteur d'avant");
        for o in s["objets"].as_array().unwrap() {
            let _: ObjetAvant = serde_json::from_value(o.clone()).expect("l'objet, lu par un lecteur d'avant");
        }
    }
    // Un fichier d'avant (sans ces clés) se lit : aucune couleur, montré, libre, les matières.
    let mut ancien = v.clone();
    for s in ancien["decors"].as_array_mut().unwrap() {
        s["aspect"].as_object_mut().unwrap().remove("rendu");
        for o in s["objets"].as_array_mut().unwrap() {
            for k in ["couleur", "cache", "verrou"] {
                o.as_object_mut().unwrap().remove(k);
            }
        }
    }
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&ancien).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    let d = relu.doc.decor_du_plan(0).unwrap();
    assert!(d.objets.iter().all(|o| o.couleur.is_none() && !o.cache && !o.verrou) && d.aspect.rendu == Rendu::Matieres);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn copying_objects_and_duplicating_a_sequence_keep_the_colours() {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let seq = doc.sequence_of(0);
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().objets[0].couleur = Some(7);
    let source = doc.decor_du_plan(0).unwrap().objets.clone();
    let aspect = doc.decor_du_plan(0).unwrap().aspect;
    let neufs = doc.poser_copies_3d(seq, &source, aspect, 0.5);
    let d = doc.decor_du_plan(0).unwrap();
    assert_eq!(d.objet(neufs[0]).unwrap().couleur, Some(7), "la copie (partie 190) garde la couleur");
    // Une séquence neuve, son décor copié (la duplication de la partie 191).
    let p = doc.add_sequence_after(0);
    let vers = doc.sequence_of(p);
    assert!(doc.copier_decor_de_sequence(seq, vers));
    let copie = doc.decors3d.par_sequence.get(&vers).unwrap();
    assert!(copie.objets.iter().any(|o| o.couleur == Some(7)), "la séquence dupliquée aussi");
}

// ------------------------------------------------------------------------------------------------ les aspects rendus

/// Le carré rouge-orangé (son matériau) seul, rendu par le processeur, dans une teinte : la couleur au centre (sRGB).
fn centre_cpu(teinte: Teinte) -> [u8; 3] {
    let octets = crate::decor3d_tests::carre(true, None, |_| {});
    let (scene, _) = crate::decor3dgltf::lire(&octets, None).unwrap();
    let monde = Monde::new([320, 180], [160, 90]);
    let prise = Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
    let place = Place3d { position: [0.0, -0.5, -0.5], rotation_y: 0.0, echelle: 2.0 };
    let aspect = Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() };
    let (rgba, _) = crate::decor3drendu::rendre_objets_prof_teintes(&[(&scene, place.matrice())], &[teinte], &prise, [320, 180], 1, 2, &aspect);
    let at = rgba.chunks(4).position(|p| p[3] == 255).expect("le carré est rendu") * 4;
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

#[test]
fn the_aspects_change_the_rendered_colours_of_the_objects() {
    let matieres = centre_cpu(Teinte::Matieres);
    assert!(matieres[0] > matieres[2] + 40, "ses matières : rouge-orangé {matieres:?}");
    let bleu = centre_cpu(Teinte::de(Rendu::Couleurs, Some(5)));
    assert!(bleu[2] > bleu[0] + 40, "Couleurs, Bleu : {bleu:?}");
    let sans = centre_cpu(Teinte::de(Rendu::Couleurs, None));
    assert!(sans.iter().max().unwrap() - sans.iter().min().unwrap() < 40, "sans couleur, un gris neutre (sous la lumière) : {sans:?}");
    let argile = centre_cpu(Teinte::de(Rendu::Argile, Some(5)));
    assert!(argile[2] < argile[0] + 10, "l'argile, la couleur n'y fait rien : {argile:?}");
    assert_ne!(argile, bleu);
    // La carte graphique fait de même, quand le banc d'essai en a une.
    if let Some(rs) = crate::gpu_tests::headless() {
        let doc = crate::decor3d_tests::projet_a_deux_carres();
        let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
        let Some(crate::render::FondPart::Decor(key)) = plan.items.iter().find_map(|it| it.fond.clone()) else { panic!() };
        let objets: Vec<decor3d::Pose3d> = decor3d::objets_de(&doc, &key).unwrap().into_iter().map(|(m, x, _)| (m, x, Some(5))).collect();
        let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
        let [w, h] = key.feuille;
        let mut rendu = |r: Rendu| {
            let d = decor3d::dessins_gpu(&objets, Aspect { rendu: r, ..key.aspect });
            crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &key.prise().unwrap(), [w as u32, h as u32], false, crate::decor3dbudget::Qualite::Sans, 1).unwrap().0
        };
        let (cx, cy, fw) = (w / 2, h / 2, doc.frame[0]);
        let px = |v: &[u8]| {
            let at = (cy * w + cx - fw / 4) * 4;
            [v[at], v[at + 1], v[at + 2]]
        };
        let (m, c) = (px(&rendu(Rendu::Matieres)), px(&rendu(Rendu::Couleurs)));
        assert!(m[0] > m[2] && c[2] > c[0], "sur la carte : {m:?} → {c:?}");
    }
}

#[test]
fn in_the_plans_the_aspect_changes_the_image_its_cache_key_and_the_export_and_never_the_cards() {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let seq = doc.sequence_of(0);
    let cle = |doc: &Document| {
        let plan = crate::render::panel_plan_at(doc, 0, 0, None);
        let Some(crate::render::FondPart::Decor(k)) = plan.items.iter().find_map(|it| it.fond.clone()) else { panic!() };
        k
    };
    let k0 = cle(&doc);
    let modele = doc.decor_du_plan(0).unwrap().objets[0].modele.clone();
    // Une couleur, dans les matières : la même image (rien à redessiner).
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().objets[0].couleur = Some(5);
    assert_eq!(decor3d::id_of(&cle(&doc)), decor3d::id_of(&k0));
    // « Dans les plans » : Couleurs : une autre image ; changer la couleur, encore une autre ; les maillages restent les mêmes.
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.rendu = Rendu::Couleurs;
    let k1 = cle(&doc);
    assert_ne!(decor3d::id_of(&k1), decor3d::id_of(&k0), "l'aspect est dans la clé de l'image (et des vignettes)");
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().objets[0].couleur = Some(3);
    let k2 = cle(&doc);
    assert_ne!(decor3d::id_of(&k2), decor3d::id_of(&k1), "la couleur aussi, dans l'aspect Couleurs");
    assert_eq!(k2.decor, k0.decor, "la même texture du décor sur la carte : redessinée, pas recréée");
    assert!(std::sync::Arc::ptr_eq(&doc.decor_du_plan(0).unwrap().objets[0].modele, &modele), "le modèle n'est pas refait");
    // L'export (par le processeur, sans carte graphique ici) : le carré de gauche passe du rouge-orangé au vert (rang 3).
    let [w, h] = doc.frame;
    let px = |img: &[eframe::egui::Color32]| img[(h / 2) * w + w / 4];
    let couleurs = px(&crate::decor3d_tests::image_exportee(&doc));
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.rendu = Rendu::Matieres;
    let matieres = px(&crate::decor3d_tests::image_exportee(&doc));
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.rendu = Rendu::Argile;
    let argile = px(&crate::decor3d_tests::image_exportee(&doc));
    assert!(matieres.r() > matieres.g() + 40, "matières : {matieres:?}");
    assert!(couleurs.g() > couleurs.r(), "couleurs, Vert : {couleurs:?}");
    assert!(argile.b() < argile.r() + 10 && argile != matieres && argile != couleurs, "argile : {argile:?}");
    // Les cartes ne sont jamais teintes : un calque peint au milieu du cadre garde ses pixels dans les trois aspects.
    let cel = doc.new_cel(crate::document::LayerKind::Bitmap);
    let (sw, sh) = (doc.width, doc.height);
    let mut rgba = vec![0u8; sw * sh * 4];
    for y in sh / 2 - 6..sh / 2 + 6 {
        for x in sw / 2 - 6..sw / 2 + 6 {
            rgba[(y * sw + x) * 4..(y * sw + x) * 4 + 4].copy_from_slice(&[30, 160, 200, 255]);
        }
    }
    doc.cels.get_mut(&cel).unwrap().raster = crate::raster::Raster::from_rgba(sw, sh, &rgba);
    let n = doc.panels[0].frame_count();
    doc.panels[0].layers[0].frames = vec![Some(cel); n];
    let mut vus_carte = Vec::new();
    for r in Rendu::TOUS {
        doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.rendu = r;
        vus_carte.push(crate::decor3d_tests::image_exportee(&doc)[(h / 2) * w + w / 2]);
    }
    assert!(vus_carte.windows(2).all(|p| p[0] == p[1]) && vus_carte[0].b() > 150, "la carte, la même dans les trois : {vus_carte:?}");
}

// ------------------------------------------------------------------------------------------------ l'aspect de la vue, le Storyboard

#[test]
fn the_view_aspect_is_a_setting_and_in_the_plans_has_a_quick_switch_in_the_storyboard() {
    let mut b = banc_3d();
    for r in Rendu::TOUS {
        assert_eq!(vus::ou(&format!("Aspect : {}", r.nom())).len(), 1, "{r:?} dans la barre de la vue");
    }
    clic(&mut b, vus::ou("Aspect : Argile")[0].center(), Modifiers::NONE);
    assert_eq!(b.app.settings.aspect_vue_3d, Rendu::Argile);
    assert_eq!(crate::settings::Settings::load().aspect_vue_3d, Rendu::Argile, "retenu dans les réglages");
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().aspect.rendu, Rendu::Matieres, "la vue ne change pas les plans");
    // Propriétés > Décor : « Dans les plans ».
    b.app.colonne.montrer(Pane::Decor3d);
    b.chauffer(4);
    clic(&mut b, vus::ou("Dans les plans : Couleurs")[0].center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().aspect.rendu, Rendu::Couleurs);
    // Le Storyboard : la barre au-dessus du dessin a « Décor » et les trois aspects dans les plans ; une étape nommée.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    let etapes = b.app.history.undo_len();
    clic(&mut b, vus::ou("Décor dans les plans : Argile")[0].center(), Modifiers::NONE);
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().aspect.rendu, Rendu::Argile);
    assert_eq!(b.app.history.undo_len(), etapes + 1);
    assert!(noms_des_etapes(&b).iter().any(|n| n.contains("Aspect du décor dans les plans")));
}

#[test]
fn the_eye_and_the_padlock_of_an_object_are_undoable_project_data() {
    let mut b = banc_3d();
    let id = ids(&b.app.doc);
    let etapes = b.app.history.undo_len();
    b.app.scene3d_basculer_oeil(Ligne3d::Objet(id[0]));
    b.app.scene3d_basculer_verrou(id[1]);
    assert!(objet(&b.app.doc, id[0]).cache && objet(&b.app.doc, id[1]).verrou);
    assert_eq!(b.app.history.undo_len(), etapes + 2, "une étape chacun");
    assert_eq!(b.app.doc.decor_du_plan(0).unwrap().vus().count(), 1, "caché : ni la vue ni les plans ne le dessinent");
    b.app.run_action("edition.annuler");
    b.chauffer(1);
    assert!(!objet(&b.app.doc, id[1]).verrou);
}
