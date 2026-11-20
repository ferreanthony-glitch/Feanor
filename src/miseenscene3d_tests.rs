//! Essais de la mise en scène 3D (partie 200, `miseenscene3d.rs`, `miseenscene3dui.rs`, `docs/INTERFACE.md`) : les lampes (ajouter,
//! régler, le plafond, l'enregistrement et un lecteur d'avant, l'annulation, la lumière qu'elles donnent au décor, la marque qu'on prend
//! et qu'on tire dans la vue), les clés d'un objet (les poser, la règle des caméras de plan avec zéro, une ou plusieurs clés,
//! l'adoucissement, les losanges de la timeline, l'enregistrement, la copie, la duplication), la clé de l'image du décor (immobile :
//! celle d'avant ; animé : une par image), l'export qui bouge, « Caché dans le plan », « Nouveau plan depuis cette vue ».

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::bulles::vus;
use crate::couleurs3d::{Rendu, Teinte};
use crate::decor3d::{self, Decor3d, Modele3d, Objet3d, Place3d};
use crate::decor3drendu::{Aspect, Prise};
use crate::dock::Pane;
use crate::document::{Document, Layer, LayerKind};
use crate::espace3d::{Monde, Xf};
use crate::espace3dmath::{self as m, Orbite};
use crate::espace3dvues::PointDeVue;
use crate::miseenscene3d::{self as ms, CleObjet, Lampe3d, MAX_LAMPES};
use crate::miseenscene3dui::{lampe_of, lampe_selection};
use crate::onglet3d::Ligne3d;
use crate::onglet3dui::{CLES_SEQ, LIGNES_S3D, TIMELINE_SEQ};
use crate::perf_tests::{banc_taille, Banc};
use crate::pistes_tests::{clic, glisser};
use crate::poignees3d::objet_selection;
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------------------ les aides

fn modele_du_carre() -> Arc<Modele3d> {
    let octets = crate::decor3d_tests::carre(true, None, |_| {});
    let (scene, _) = crate::decor3dgltf::lire(&octets, None).unwrap();
    Arc::new(Modele3d::new(Arc::new(scene), Arc::new(octets)))
}

fn objet(id: u64, nom: &str, modele: &Arc<Modele3d>, position: [f32; 3], echelle: f32) -> Objet3d {
    Objet3d { id, nom: nom.into(), source: "carre.glb".into(), allege: Vec::new(), place: Place3d { position, rotation_y: 0.0, echelle }, origine: [0.0; 3], aimante: false, modele: modele.clone(), couleur: None, cache: false, verrou: false, cles: Vec::new(), caches_dans: Vec::new() }
}

/// Un film : une séquence de trois plans de 24 images (deux calques chacun), puis une autre d'un plan ; la première a un décor de deux
/// carrés (« Maison », « Arbre »), sans ombres.
fn film() -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    doc.add_sequence_after(2);
    for (k, p) in doc.panels.iter_mut().enumerate() {
        p.layers = vec![Layer::new(&format!("Fond {k}"), LayerKind::Vector), Layer::new(&format!("Perso {k}"), LayerKind::Vector)];
        p.set_length(24);
    }
    let seq = doc.sequence_of(0);
    let modele = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ombres: false, ..Default::default() });
    for (nom, x) in [("Maison", -1.0), ("Arbre", 1.5)] {
        let id = doc.alloc();
        d.objets.push(objet(id, nom, &modele, [x, 0.0, -2.0], 1.0));
    }
    doc.decors3d.par_sequence.insert(seq, d);
    doc
}

/// Deux carrés qui remplissent chacun une moitié du cadre (`decor3d_tests::projet_a_deux_carres`), dans une séquence de trois plans de
/// 24 images.
fn deux_carres_trois_plans() -> Document {
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for p in doc.panels.iter_mut() {
        p.set_length(24);
    }
    doc
}

fn banc_3d(doc: Document) -> Banc {
    // (Haute : Propriétés montre tout, sans défiler.)
    let mut b = banc_taille(doc, false, 1600, 1800);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    b.app.set_espace(Espace::TroisD);
    if let Some(k) = b.app.colonne.rang_de(Pane::Scene3d) {
        b.app.colonne.panneaux[k].hauteur = Some(300.0);
    }
    b.garder_formes = true;
    b.chauffer(8);
    b
}

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

fn decor(b: &Banc) -> &Decor3d {
    b.app.doc.decor_du_plan(b.app.cur_panel).expect("le décor de la séquence")
}

fn ids(doc: &Document) -> Vec<u64> {
    doc.decor_du_plan(0).unwrap().objets.iter().map(|o| o.id).collect()
}

fn etapes(b: &Banc) -> Vec<String> {
    b.app.history.names().0.into_iter().map(String::from).collect()
}

fn annuler(b: &mut Banc) {
    b.app.run_action("edition.annuler");
    b.chauffer(2);
}

fn ligne(l: Ligne3d) -> Rect {
    LIGNES_S3D.with(|v| v.borrow().iter().find(|x| x.0 == l).map(|x| x.1)).unwrap_or_else(|| panic!("{l:?}"))
}

/// Où le point `p` du monde tombe à l'écran, dans la vue seule de l'onglet 3D.
fn a_l_ecran(b: &Banc, p: [f32; 3]) -> Pos2 {
    let draw = b.app.espace3d.rect;
    let view = b.app.e3_view(draw);
    let q = view.project(p, [draw.width() / 2.0, draw.height() / 2.0]).expect("devant l'œil");
    pos2(draw.left() + q[0], draw.top() + q[1])
}

/// L'image exportée du plan `pi`, à son image `f`.
fn exportee(doc: &Document, pi: usize, f: usize) -> Vec<eframe::egui::Color32> {
    use crate::exportrender::{snapshot_of, Papier, Rendu as R};
    let rendu = R::new(snapshot_of(doc, &[pi]), Papier::Aucun, false, Vec::new(), Arc::new(std::sync::atomic::AtomicBool::new(false)));
    (*rendu.image(pi, f).expect("image")).clone()
}

/// La clé de l'image du décor du plan `pi` à son image `f`.
fn cle_du_decor(doc: &Document, pi: usize, f: usize) -> decor3d::DecorCle {
    let plan = crate::render::panel_plan_at(doc, pi, f, None);
    match plan.items.iter().find_map(|it| it.fond.clone()) {
        Some(crate::render::FondPart::Decor(k)) => k,
        _ => panic!("pas de décor dans le plan {pi}"),
    }
}

/// L'empreinte d'un décor telle que la partie 199 la calculait (avant les clés, les objets cachés dans un plan et les lampes).
fn contenu_d_avant(d: &Decor3d) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let couleurs = d.aspect.rendu == Rendu::Couleurs;
    for o in d.vus() {
        o.modele.uid.hash(&mut h);
        o.matrice().map(f32::to_bits).hash(&mut h);
        if couleurs {
            o.couleur.hash(&mut h);
        }
    }
    h.finish()
}

fn la_seq(doc: &Document) -> u64 {
    doc.sequence_of(0)
}

fn objet_mut(doc: &mut Document, id: u64) -> &mut Objet3d {
    let seq = la_seq(doc);
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().objet_mut(id).unwrap()
}

fn place_x(x: f32) -> Place3d {
    Place3d { position: [x, 0.0, -2.0], rotation_y: 0.0, echelle: 1.0 }
}

fn images(b: &Banc, id: u64) -> Vec<usize> {
    b.app.e3_objet(id).unwrap().cles.iter().map(|k| k.image).collect()
}

fn images_et_x(b: &Banc, id: u64) -> Vec<(usize, f32)> {
    b.app.e3_objet(id).unwrap().cles.iter().map(|k| (k.image, k.place.position[0])).collect()
}

// ------------------------------------------------------------------------------------------------ les lampes

#[test]
fn a_lamp_is_added_from_the_tool_above_the_view_target_edited_capped_switched_off_and_undone() {
    let mut b = banc_3d(film());
    assert!(decor(&b).lampes.is_empty());
    let cible = b.app.o3_orbite_perspective().expect("la vue en perspective").target;
    clic(&mut b, ou("Ajouter une lampe").center(), Modifiers::NONE);
    let l = decor(&b).lampes.first().cloned().expect("une lampe");
    let sol = Monde::of(&b.app.doc).sol_y();
    assert_eq!((l.position[0], l.position[2]), (cible[0], cible[2]), "au-dessus du point que regarde la vue");
    assert!((l.position[1] - (sol + ms::HAUTEUR)).abs() < 1e-4);
    assert_eq!((l.nom.as_str(), l.couleur, l.intensite, l.portee, l.cache), ("Lampe 1", ms::TEINTES[0].1, ms::INTENSITE, ms::PORTEE, false), "Ampoule, d'origine");
    assert_eq!(b.app.espace3d.sel, vec![lampe_selection(l.id)], "choisie dans la vue");
    assert!(etapes(&b).iter().any(|n| n == "Ajouter une lampe"), "{:?}", etapes(&b));
    // Le sommaire la montre sous Lumières ; Propriétés > Lumières la montre ouverte : sa teinte.
    b.chauffer(3);
    ligne(Ligne3d::Lampe(l.id));
    clic(&mut b, ou("Teinte : Bleutée").center(), Modifiers::NONE);
    b.chauffer(2);
    assert_eq!(decor(&b).lampes[0].couleur, ms::TEINTES[2].1, "Bleutée");
    assert!(etapes(&b).last().is_some_and(|n| n == "Retoucher le décor 3D"), "{:?}", etapes(&b));
    // L'œil l'éteint (une étape) : elle n'éclaire plus.
    b.app.scene3d_basculer_oeil(Ligne3d::Lampe(l.id));
    assert!(decor(&b).lampes[0].cache);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Éteindre une lampe"));
    assert_eq!(ms::lampes_vues(decor(&b), &m::IDENT).n, 0);
    b.app.scene3d_basculer_oeil(Ligne3d::Lampe(l.id));
    // Le plafond : huit par séquence ; l'ajout le dit.
    for _ in 1..MAX_LAMPES {
        b.app.o3_ajouter_lampe();
    }
    assert_eq!(decor(&b).lampes.len(), MAX_LAMPES);
    b.app.o3_ajouter_lampe();
    assert_eq!(decor(&b).lampes.len(), MAX_LAMPES);
    assert!(b.app.status.contains("au plus 8"), "{}", b.app.status);
    // Annuler retire la dernière ajoutée ; enlever par le sommaire (une étape).
    annuler(&mut b);
    assert_eq!(decor(&b).lampes.len(), MAX_LAMPES - 1);
    b.chauffer(2);
    let r = ligne(Ligne3d::Lampe(l.id));
    let poubelle = vus::ou("Enlever la lampe").into_iter().find(|x| r.contains(x.center())).expect("enlever, dans la ligne de la lampe");
    clic(&mut b, poubelle.center(), Modifiers::NONE);
    assert!(decor(&b).lampe(l.id).is_none() && etapes(&b).last().is_some_and(|n| n == "Enlever une lampe"));
    annuler(&mut b);
    assert!(decor(&b).lampe(l.id).is_some(), "Ctrl+Z la rend");
}

#[test]
fn lamps_are_saved_with_the_decor_and_an_old_file_and_an_old_reader_still_work() {
    let dir = crate::decor3d_tests::dossier("lampes200");
    let manifeste = dir.join("Film.shot");
    let mut doc = film();
    let seq = la_seq(&doc);
    let id = doc.alloc();
    {
        let d = doc.decors3d.par_sequence.get_mut(&seq).unwrap();
        let mut l = Lampe3d::neuve(id, "Lampe de la table".into(), [0.3, 1.95, -0.4]);
        l.couleur = ms::TEINTES[3].1;
        l.portee = 3.5;
        d.lampes.push(l);
    }
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"lampes\"") && texte.contains("Lampe de la table") && texte.contains("\"version\": 2"), "{texte}");
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!(relu.doc.decor_du_plan(0).unwrap().lampes, doc.decor_du_plan(0).unwrap().lampes);
    // Un lecteur d'avant (la séquence sans lampes) lit le fichier d'aujourd'hui.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct SequenceAvant {
        sequence: u64,
        #[serde(default)]
        aspect: Aspect,
        objets: Vec<serde_json::Value>,
    }
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    for s in v["decors"].as_array().unwrap() {
        let _: SequenceAvant = serde_json::from_value(s.clone()).expect("une séquence lue par un lecteur d'avant");
    }
    // Un fichier d'avant (sans la clé) : aucune lampe.
    let mut ancien = v.clone();
    for s in ancien["decors"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("lampes");
    }
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&ancien).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.warnings.is_empty() && relu.doc.decor_du_plan(0).unwrap().lampes.is_empty());
    // Un fichier abîmé : la lampe revient dans ses bornes, et plus de huit sont ramenées à huit.
    let mut abime = v.clone();
    let l = abime["decors"][0]["lampes"][0].clone();
    let mut l2 = l.clone();
    l2["portee"] = serde_json::json!(1e9);
    abime["decors"][0]["lampes"] = serde_json::Value::Array((0..12).map(|k| if k == 0 { l2.clone() } else { l.clone() }).collect());
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&abime).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    let lampes = &relu.doc.decor_du_plan(0).unwrap().lampes;
    assert_eq!((lampes.len(), lampes[0].portee), (MAX_LAMPES, ms::PORTEE_MAX));
    std::fs::remove_dir_all(&dir).ok();
}

/// Le carré seul, rendu par le processeur dans l'aspect `rendu`, avec ces lampes : la couleur au centre (sRGB).
fn centre_avec(rendu: Rendu, lampes: &[Lampe3d]) -> [u8; 3] {
    let modele = modele_du_carre();
    let monde = Monde::new([320, 180], [160, 90]);
    let prise = Prise::de_la_feuille(&monde, monde.eye_reference()).unwrap();
    let place = Place3d { position: [0.0, -0.5, -0.5], rotation_y: 0.0, echelle: 2.0 };
    let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: false, rendu, ..Aspect::default() });
    d.lampes = lampes.to_vec();
    let aspect = Aspect { lampes: ms::lampes_vues(&d, &m::IDENT), ..d.aspect };
    let (rgba, _) = crate::decor3drendu::rendre_objets_prof_teintes(&[(&modele.scene, place.matrice())], &[Teinte::de(rendu, Some(5))], &prise, [320, 180], 1, 2, &aspect);
    let at = (90 * 320 + 160) * 4;
    assert_eq!(rgba[at + 3], 255, "le carré au centre");
    [rgba[at], rgba[at + 1], rgba[at + 2]]
}

#[test]
fn a_lamp_lights_the_decor_in_every_aspect_its_plan_images_and_their_cache_key() {
    let mut blanche = Lampe3d::neuve(1, "Blanche".into(), [0.0, 0.0, 0.0]);
    blanche.couleur = [255, 255, 255];
    blanche.intensite = 1.5;
    blanche.portee = 3.0;
    let somme = |c: [u8; 3]| c.iter().map(|&v| v as u32).sum::<u32>();
    for r in Rendu::TOUS {
        let (sans, avec) = (centre_avec(r, &[]), centre_avec(r, std::slice::from_ref(&blanche)));
        assert!(avec.iter().zip(&sans).all(|(a, s)| a >= s) && somme(avec) > somme(sans) + 60, "{r:?} : {sans:?} → {avec:?}");
        // Au-delà de sa portée, rien ; éteinte, rien.
        let mut loin = blanche.clone();
        loin.position = [0.0, 0.0, 4.0];
        assert_eq!(centre_avec(r, &[loin]), sans, "{r:?} : hors de portée");
        let mut eteinte = blanche.clone();
        eteinte.cache = true;
        assert_eq!(centre_avec(r, &[eteinte]), sans, "{r:?} : éteinte");
    }
    // Une lampe rouge rougit.
    let mut rouge = blanche.clone();
    rouge.couleur = ms::TEINTES[3].1;
    let (sans, avec) = (centre_avec(Rendu::Argile, &[]), centre_avec(Rendu::Argile, &[rouge]));
    assert!(avec[0] as i32 - sans[0] as i32 > avec[2] as i32 - sans[2] as i32 + 10, "{sans:?} → {avec:?}");
    // Dans l'image du plan : la clé change (une autre image, la même texture du décor), l'export s'éclaire.
    let mut doc = crate::decor3d_tests::projet_a_deux_carres();
    let seq = la_seq(&doc);
    let k0 = cle_du_decor(&doc, 0, 0);
    let [w, h] = doc.frame;
    let px0 = exportee(&doc, 0, 0)[(h / 2) * w + w / 4];
    let id = doc.alloc();
    let mut l = Lampe3d::neuve(id, "Lampe".into(), [-0.5, 0.0, 0.5]);
    l.intensite = 2.0;
    l.couleur = [255, 255, 255];
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().lampes.push(l);
    let k1 = cle_du_decor(&doc, 0, 0);
    assert_ne!(decor3d::id_of(&k1), decor3d::id_of(&k0), "la lampe est dans la clé de l'image");
    assert_eq!(k1.decor, k0.decor);
    let px1 = exportee(&doc, 0, 0)[(h / 2) * w + w / 4];
    let s = |c: eframe::egui::Color32| c.r() as u32 + c.g() as u32 + c.b() as u32;
    assert!(s(px1) > s(px0) + 40, "{px0:?} → {px1:?}");
    // La carte graphique fait de même, quand le banc d'essai en a une.
    if let Some(rs) = crate::gpu_tests::headless() {
        let objets = decor3d::objets_de(&doc, &k1).unwrap();
        let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
        let [fw, fh] = k1.feuille;
        let mut rendu = |a: Aspect| {
            let d = decor3d::dessins_gpu(&objets, a);
            crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &k1.prise().unwrap(), [fw as u32, fh as u32], false, crate::decor3dbudget::Qualite::Sans, 1).unwrap().0
        };
        let at = ((fh / 2) * fw + fw / 2 - doc.frame[0] / 4) * 4;
        let (sans, avec) = (rendu(Aspect { lampes: Default::default(), ..k1.aspect }), rendu(k1.aspect));
        assert!(avec[at] as u32 + avec[at + 1] as u32 + avec[at + 2] as u32 > sans[at] as u32 + sans[at + 1] as u32 + sans[at + 2] as u32 + 40, "sur la carte");
    }
}

#[test]
fn a_lamp_is_picked_and_dragged_in_the_view_in_one_undo_step() {
    let mut b = banc_3d(film());
    b.app.o3_ajouter_lampe();
    let l = decor(&b).lampes[0].clone();
    b.app.espace3d.sel.clear();
    b.chauffer(3);
    let p = a_l_ecran(&b, l.position);
    clic(&mut b, p, Modifiers::NONE);
    assert_eq!(b.app.espace3d.sel, vec![lampe_selection(l.id)], "un clic sur sa marque la choisit");
    assert_eq!(b.app.o3.sujet, Some(Pane::Lumieres3d), "Propriétés passe à Lumières");
    let n = etapes(&b).len();
    glisser(&mut b, p, p + vec2(80.0, 0.0));
    let apres = decor(&b).lampes[0].position;
    assert!(m::length(m::sub(apres, l.position)) > 0.05, "{:?} → {apres:?}", l.position);
    assert_eq!(etapes(&b).len(), n + 1, "une étape : {:?}", etapes(&b));
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Déplacer une lampe"));
    // Le manipulateur (l'outil Déplacer, les poignées) la prend aussi : sa place est son placement.
    assert_eq!(b.app.e3_xfs(&[lampe_selection(l.id)])[0].1.pos, apres);
    b.app.e3_set_xfs(&[(lampe_selection(l.id), Xf { pos: [1.0, 2.0, -1.0], rot: [0.0; 3], size: [1.0; 3] })]);
    assert_eq!(decor(&b).lampes[0].position, [1.0, 2.0, -1.0]);
    // Elle ne se tourne pas.
    assert!(b.app.e3_poignee_refusee(crate::espace3dgizmo::Mode::Rotate, crate::espace3dgizmo::Handle::Ring(1)).is_some());
    annuler(&mut b);
    assert_eq!(decor(&b).lampes[0].position, l.position, "Ctrl+Z la ramène");
    assert!(lampe_of(objet_selection(3)).is_none() && lampe_of(crate::espace3d::card_selection(3)).is_none() && lampe_of(crate::espace3dgizmo::CAMERA_ID).is_none());
}

// ------------------------------------------------------------------------------------------------ les clés d'un objet

#[test]
fn keys_ease_in_and_out_between_two_and_hold_before_the_first_and_after_the_last() {
    let a = CleObjet { image: 0, place: Place3d { position: [0.0, 0.0, 0.0], rotation_y: 0.0, echelle: 1.0 } };
    let z = CleObjet { image: 10, place: Place3d { position: [10.0, 2.0, -4.0], rotation_y: 90.0, echelle: 3.0 } };
    assert_eq!(ms::place_a(&[], 3), None);
    assert_eq!(ms::place_a(&[z], 3), Some(z.place), "une clé : partout la même");
    let cles = [a, z];
    let p = ms::place_a(&cles, 5).unwrap();
    assert_eq!((p.position, p.rotation_y, p.echelle), ([5.0, 1.0, -2.0], 45.0, 2.0), "au milieu");
    let p = ms::place_a(&cles, 2).unwrap();
    assert!((p.position[0] - 1.04).abs() < 1e-4, "un départ lent : {}", p.position[0]);
    let p = ms::place_a(&cles, 8).unwrap();
    assert!((p.position[0] - 8.96).abs() < 1e-4, "une arrivée lente : {}", p.position[0]);
    assert_eq!(ms::place_a(&cles, 0).unwrap(), a.place);
    assert_eq!(ms::place_a(&cles, 40).unwrap(), z.place, "après la dernière, elle tient");
    let mut c = vec![CleObjet { image: 0, place: a.place }];
    assert!(!ms::retoucher(&mut c, 7, z.place) && c.len() == 1 && c[0].place == z.place, "une clé : elle-même");
    c.push(CleObjet { image: 10, place: a.place });
    assert!(ms::retoucher(&mut c, 5, z.place) && c.iter().map(|k| k.image).collect::<Vec<_>>() == [0, 5, 10], "deux : une clé de plus, triée");
    assert!(!ms::retoucher(&mut c, 5, a.place) && c.len() == 3, "à une image qui a sa clé : celle-là");
    assert_eq!(ms::cles_valides(vec![CleObjet { image: 9, place: a.place }, CleObjet { image: 2, place: z.place }, CleObjet { image: 9, place: z.place }]).iter().map(|k| k.image).collect::<Vec<_>>(), vec![2, 9]);
}

#[test]
fn moving_an_object_follows_the_plan_camera_rule_with_none_one_or_several_keys() {
    let mut b = banc_3d(film());
    let id = ids(&b.app.doc)[0];
    let s = objet_selection(id);
    let bouger = |b: &mut Banc, x: f32| {
        b.app.e3_set_xfs(&[(s, Xf { pos: [x, 0.0, -2.0], rot: [0.0; 3], size: [1.0; 3] })]);
        b.chauffer(1);
    };
    // Sans clé : la place de l'objet.
    bouger(&mut b, -0.5);
    assert!(images(&b, id).is_empty() && b.app.e3_objet(id).unwrap().place.position[0] == -0.5);
    // « ◆ Clé » en tête de la bande, l'objet choisi : une clé à l'image 1.
    b.app.e3_choisir_objet(id, false);
    b.chauffer(3);
    clic(&mut b, ou("Poser une clé").center(), Modifiers::NONE);
    assert_eq!(images_et_x(&b, id), vec![(0, -0.5)]);
    assert!(etapes(&b).last().is_some_and(|n| n == "Clé d'un objet du décor"));
    // Une clé : déplacer l'objet, à n'importe quelle image, règle celle-là.
    b.app.goto_global(10);
    b.chauffer(2);
    bouger(&mut b, 0.25);
    assert_eq!(images_et_x(&b, id), vec![(0, 0.25)]);
    // Une deuxième clé (Propriétés > Objet, « ◆ Clé à l'image 11 ») ; puis, avec deux clés, déplacer l'objet à une image sans clé en crée
    // une à cette image.
    b.app.colonne.montrer(Pane::Objet3d);
    b.chauffer(3);
    clic(&mut b, ou("Clé à l'image courante").center(), Modifiers::NONE);
    assert_eq!(images(&b, id), vec![0, 10]);
    b.app.goto_global(20);
    b.chauffer(2);
    bouger(&mut b, 1.0);
    assert_eq!(images_et_x(&b, id), vec![(0, 0.25), (10, 0.25), (20, 1.0)]);
    // À une image qui a sa clé : celle-là, rien de plus.
    b.app.goto_global(10);
    b.chauffer(2);
    assert_eq!(b.app.e3_objet(id).unwrap().place.position[0], 0.25, "à l'image 11, l'objet est à sa clé");
    bouger(&mut b, -1.0);
    assert_eq!(images_et_x(&b, id), vec![(0, 0.25), (10, -1.0), (20, 1.0)]);
    // Le champ de Propriétés > Objet, par le chemin de la retouche du panneau : la même règle, à l'image 7 du deuxième plan (30).
    b.app.goto_global(30);
    b.chauffer(2);
    let seq = la_seq(&b.app.doc);
    let mut d = decor(&b).clone();
    d.objet_mut(id).unwrap().place.position[0] = 2.0;
    b.app.decor3d_retoucher(seq, d);
    b.app.decor3d_retouche_finir();
    b.chauffer(1);
    assert_eq!(images_et_x(&b, id), vec![(0, 0.25), (10, -1.0), (20, 1.0), (30, 2.0)]);
    // L'autre objet, sans clé, n'a rien gagné ; une retouche de couleur ne pose aucune clé.
    let autre = ids(&b.app.doc)[1];
    assert!(images(&b, autre).is_empty());
    b.app.couleur_d_identification(&[id], Some(2));
    assert_eq!(images(&b, id).len(), 4);
    // Changer l'origine ne bouge l'objet à aucune clé.
    let mut d = decor(&b).clone();
    let avant = [0, 10, 20, 30].map(|i| m::point(&d.objet(id).unwrap().matrice_a(i), [0.2, 0.3, 0.0]));
    d.objet_mut(id).unwrap().changer_origine([0.5, 0.5, 0.0]);
    let apres = [0, 10, 20, 30].map(|i| m::point(&d.objet(id).unwrap().matrice_a(i), [0.2, 0.3, 0.0]));
    for (p, q) in avant.iter().zip(&apres) {
        assert!(m::length(m::sub(*p, *q)) < 1e-4, "{p:?} / {q:?}");
    }
    // « Effacer les clés » : immobile, à sa place de l'image courante.
    b.app.o3_effacer_cles(id);
    assert!(images(&b, id).is_empty() && b.app.e3_objet(id).unwrap().place.position[0] == 2.0);
    annuler(&mut b);
    assert_eq!(images(&b, id).len(), 4, "Ctrl+Z rend les clés");
}

#[test]
fn the_sequence_timeline_shows_the_keys_a_click_goes_a_right_click_removes_a_drag_moves() {
    let mut b = banc_3d(film());
    let id = ids(&b.app.doc)[0];
    objet_mut(&mut b.app.doc, id).cles = vec![CleObjet { image: 0, place: place_x(-1.0) }, CleObjet { image: 12, place: place_x(0.0) }, CleObjet { image: 30, place: place_x(1.0) }];
    // Sans objet choisi : la rangée le demande.
    b.chauffer(3);
    assert!(CLES_SEQ.with(|c| c.borrow().is_empty()));
    b.app.e3_choisir_objet(id, false);
    b.chauffer(3);
    let rangee = || CLES_SEQ.with(|c| c.borrow().clone());
    assert_eq!(rangee().iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 12, 30]);
    let (piste, _) = TIMELINE_SEQ.with(|t| t.borrow().clone()).unwrap();
    // Un clic : la tête de lecture y va (l'image 7 du deuxième plan) ; l'objet est à sa clé.
    let r = rangee()[2].1;
    clic(&mut b, r.center(), Modifiers::NONE);
    assert_eq!((b.app.cur_panel, b.app.cur_frame), (1, 6));
    assert_eq!(b.app.e3_objet(id).unwrap().place.position[0], 1.0);
    // Un clic droit : la clé part (une étape).
    let r = rangee().into_iter().find(|x| x.0 == 12).unwrap().1;
    b.image(vec![Event::PointerMoved(r.center())], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: r.center(), button: PointerButton::Secondary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: r.center(), button: PointerButton::Secondary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(images(&b, id), vec![0, 30]);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Enlever une clé"));
    // Un glisser : la clé de l'image 31 va à l'image 41, en une étape.
    let n = etapes(&b).len();
    let x_de = |i: usize| piste.left() + i as f32 / 72.0 * piste.width() + 0.5;
    let depart = rangee().into_iter().find(|x| x.0 == 30).unwrap().1.center();
    glisser(&mut b, depart, pos2(x_de(40), depart.y));
    assert_eq!(images(&b, id), vec![0, 40]);
    assert_eq!(etapes(&b).len(), n + 1);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Déplacer une clé"));
    annuler(&mut b);
    assert_eq!(images(&b, id), vec![0, 30]);
}

#[test]
fn keys_and_hides_are_saved_kept_by_copy_and_duplicate_and_an_old_reader_still_works() {
    let dir = crate::decor3d_tests::dossier("cles200");
    let manifeste = dir.join("Film.shot");
    let mut doc = film();
    let seq = la_seq(&doc);
    let id = ids(&doc)[0];
    let plan1 = doc.panels[1].id;
    let cles = vec![CleObjet { image: 0, place: place_x(-1.0) }, CleObjet { image: 30, place: place_x(1.0) }];
    objet_mut(&mut doc, id).cles = cles.clone();
    objet_mut(&mut doc, id).caches_dans = vec![plan1];
    crate::project::save(&doc, &manifeste, &mut Default::default()).unwrap();
    let texte = std::fs::read_to_string(dir.join(decor3d::FILE)).unwrap();
    assert!(texte.contains("\"cles\"") && texte.contains("\"caches_dans\"") && texte.contains("\"version\": 2"), "{texte}");
    let relu = crate::project::load(&manifeste).unwrap();
    let o = relu.doc.decor_du_plan(0).unwrap().objet(id).unwrap().clone();
    assert_eq!((o.cles.clone(), o.caches_dans.clone()), (cles, vec![plan1]));
    // Un lecteur d'avant (un objet sans ces champs) lit le fichier d'aujourd'hui ; un fichier d'avant se lit immobile, partout.
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct ObjetAvant {
        id: u64,
        nom: String,
        fichier: String,
        #[serde(flatten)]
        place: Place3d,
        #[serde(default)]
        couleur: Option<u8>,
    }
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    for s in v["decors"].as_array().unwrap() {
        for o in s["objets"].as_array().unwrap() {
            let _: ObjetAvant = serde_json::from_value(o.clone()).expect("un objet lu par un lecteur d'avant");
        }
    }
    let mut ancien = v.clone();
    for s in ancien["decors"].as_array_mut().unwrap() {
        for o in s["objets"].as_array_mut().unwrap() {
            o.as_object_mut().unwrap().remove("cles");
            o.as_object_mut().unwrap().remove("caches_dans");
        }
    }
    std::fs::write(dir.join(decor3d::FILE), serde_json::to_vec_pretty(&ancien).unwrap()).unwrap();
    let relu = crate::project::load(&manifeste).unwrap();
    assert!(relu.doc.decor_du_plan(0).unwrap().objets.iter().all(|o| o.cles.is_empty() && o.caches_dans.is_empty()));
    std::fs::remove_dir_all(&dir).ok();
    // La copie (partie 190) garde les clés.
    let source = doc.decor_du_plan(0).unwrap().objets.clone();
    let aspect = doc.decor_du_plan(0).unwrap().aspect;
    let neufs = doc.poser_copies_3d(seq, &source[..1], aspect, 0.5);
    assert_eq!(doc.decor_du_plan(0).unwrap().objet(neufs[0]).unwrap().cles.len(), 2);
    // La séquence dupliquée (partie 191) : les clés, les lampes (à des numéros neufs), et l'objet caché dans la copie du plan 2.
    let lid = doc.alloc();
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().lampes.push(Lampe3d::neuve(lid, "Lampe".into(), [0.0, 2.0, 0.0]));
    let pi = doc.dupliquer_sequence(seq).expect("la copie");
    let neuve = doc.sequence_of(pi);
    assert_ne!(neuve, seq);
    let (a, _) = doc.sequence_run(pi);
    let d = doc.decors3d.par_sequence.get(&neuve).unwrap();
    let copie = d.objets.iter().find(|o| o.nom == "Maison" && o.cles.len() == 2 && !o.caches_dans.is_empty()).expect("la maison, ses clés, caché dans un plan");
    assert_eq!(copie.caches_dans, vec![doc.panels[a + 1].id], "cachée dans la copie du deuxième plan");
    assert_eq!(d.lampes.len(), 1);
    assert_ne!(d.lampes[0].id, lid);
}

// ------------------------------------------------------------------------------------------------ l'image du décor

#[test]
fn a_still_decor_keeps_todays_cache_key_and_an_animated_one_changes_per_frame_and_the_export_moves() {
    let mut doc = deux_carres_trois_plans();
    let d = doc.decor_du_plan(0).unwrap().clone();
    // Immobile : l'empreinte d'avant, la même clé à toutes les images de tous les plans.
    let k = cle_du_decor(&doc, 0, 0);
    assert_eq!(k.contenu, contenu_d_avant(&d), "l'empreinte de la partie 199");
    for (pi, f) in [(0, 10), (1, 0), (2, 23)] {
        assert_eq!(decor3d::id_of(&cle_du_decor(&doc, pi, f)), decor3d::id_of(&k), "plan {pi}, image {f}");
    }
    // Animé : le carré de gauche monte d'une demi-largeur de cadre de l'image 0 à l'image 30 (le plan 2, image 7) : il sort du cadre.
    let id = d.objets[0].id;
    let p0 = d.objets[0].place;
    let mut p1 = p0;
    p1.position[1] += p0.echelle * 5.0;
    objet_mut(&mut doc, id).cles = vec![CleObjet { image: 0, place: p0 }, CleObjet { image: 30, place: p1 }];
    let k0 = cle_du_decor(&doc, 0, 0);
    let k5 = cle_du_decor(&doc, 0, 5);
    let k30 = cle_du_decor(&doc, 1, 6);
    let k40 = cle_du_decor(&doc, 1, 16);
    assert_eq!(k0.contenu, contenu_d_avant(&d), "à l'image 0, le carré est à sa place d'avant");
    assert_ne!(decor3d::id_of(&k5), decor3d::id_of(&k0), "une autre image du décor à chaque image du mouvement");
    assert_ne!(decor3d::id_of(&k30), decor3d::id_of(&k5));
    assert_eq!(decor3d::id_of(&k40), decor3d::id_of(&k30), "après la dernière clé : la même image, gardée");
    assert_eq!(k30.decor, k0.decor, "la même texture du décor, redessinée");
    assert_eq!(decor3d::objets_de(&doc, &k30).unwrap().len(), 2);
    // L'export bouge : au milieu à gauche, le carré au début, plus là quand il est monté ; à mi-chemin, entre les deux.
    let [w, h] = doc.frame;
    let (gauche, droite) = ((h / 2) * w + w / 4, (h / 2) * w + 3 * w / 4);
    let (debut, fin) = (exportee(&doc, 0, 0), exportee(&doc, 1, 6));
    assert_eq!(debut[gauche].a(), 255, "le carré, au début");
    assert_eq!(fin[gauche].a(), 0, "le carré est monté");
    assert_eq!(fin[droite].a(), 255, "l'autre, immobile, reste");
    assert_ne!(exportee(&doc, 0, 15), debut, "en chemin, une autre image");
}

#[test]
fn hidden_in_the_plan_leaves_that_plan_image_and_export_but_the_3d_tab_views_still_show_it() {
    let mut doc = deux_carres_trois_plans();
    let id = doc.decor_du_plan(0).unwrap().objets[0].id;
    let plan0 = doc.panels[0].id;
    objet_mut(&mut doc, id).caches_dans = vec![plan0];
    let [w, h] = doc.frame;
    let (gauche, droite) = ((h / 2) * w + w / 4, (h / 2) * w + 3 * w / 4);
    let k0 = cle_du_decor(&doc, 0, 0);
    let k1 = cle_du_decor(&doc, 1, 0);
    assert_ne!(decor3d::id_of(&k0), decor3d::id_of(&k1));
    assert_eq!(decor3d::objets_de(&doc, &k0).unwrap().len(), 1, "le plan 1 ne le dessine pas");
    assert_eq!(decor3d::objets_de(&doc, &k1).unwrap().len(), 2);
    let (e0, e1) = (exportee(&doc, 0, 0), exportee(&doc, 1, 0));
    assert_eq!((e0[gauche].a(), e0[droite].a()), (0, 255), "l'export du plan 1 ne le montre pas");
    assert_eq!((e1[gauche].a(), e1[droite].a()), (255, 255), "les autres plans, si");
    // Tous ses objets cachés dans un plan : pas d'élément de décor du tout dans ce plan.
    let mut seul = crate::decor3d_tests::projet_a_deux_carres();
    let ids_seul: Vec<u64> = seul.decor_du_plan(0).unwrap().objets.iter().map(|o| o.id).collect();
    let p = seul.panels[0].id;
    for i in ids_seul {
        objet_mut(&mut seul, i).caches_dans = vec![p];
    }
    assert!(crate::render::panel_plan_at(&seul, 0, 0, None).items.iter().all(|it| it.fond.is_none()));
    // Dans l'onglet : la case de Propriétés > Objet ; la perspective le dessine encore (en pointillés), « Caméra du plan » non.
    let mut b = banc_3d(film());
    let id = ids(&b.app.doc)[0];
    b.app.e3_choisir_objet(id, false);
    b.app.colonne.montrer(Pane::Objet3d);
    b.chauffer(3);
    clic(&mut b, ou("Caché dans le plan").center(), Modifiers::NONE);
    assert_eq!(b.app.e3_objet(id).unwrap().caches_dans, vec![b.app.doc.panels[0].id]);
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Cacher un objet dans le plan"));
    crate::decor3dui::PEINTS.with(|v| v.borrow_mut().clear());
    b.chauffer(1);
    assert_eq!(crate::decor3dui::PEINTS.with(|v| v.borrow().last().copied()), Some((false, 2, 1)), "la perspective : les deux, l'un en pointillés");
    b.app.o3_choisir_point(PointDeVue::Camera);
    crate::decor3dui::PEINTS.with(|v| v.borrow_mut().clear());
    b.chauffer(1);
    assert_eq!(crate::decor3dui::PEINTS.with(|v| v.borrow().last().copied()), Some((true, 1, 0)), "« Caméra du plan » : seulement l'autre");
    // Le plan suivant le montre.
    b.app.select_panel(1);
    crate::decor3dui::PEINTS.with(|v| v.borrow_mut().clear());
    b.chauffer(2);
    assert_eq!(crate::decor3dui::PEINTS.with(|v| v.borrow().last().copied()), Some((true, 2, 0)));
}

// ------------------------------------------------------------------------------------------------ nouveau plan depuis la vue

#[test]
fn a_new_plan_from_the_view_films_what_the_perspective_shows_at_the_end_of_the_sequence_in_one_step() {
    let mut b = banc_3d(film());
    let orbite = Orbite { target: [0.5, 0.0, -2.0], azimuth: 25.0, elevation: 12.0, distance: 6.0, focal: 42.0 };
    b.app.espace3d.cam = Some(orbite);
    b.chauffer(2);
    let n = b.app.doc.panels.len();
    clic(&mut b, ou("Nouveau plan depuis cette vue").center(), Modifiers::NONE);
    assert_eq!(b.app.doc.panels.len(), n + 1);
    assert_eq!(b.app.cur_panel, 3, "à la fin de la première séquence, avant la seconde");
    assert_eq!(b.app.doc.sequence_run(3), (0, 4));
    assert!(b.app.espace3d.sel.contains(&crate::espace3dgizmo::CAMERA_ID), "sa caméra choisie");
    assert_eq!(etapes(&b).last().map(String::as_str), Some("Nouveau plan depuis la vue"));
    // Ce qu'un nouveau plan reçoit : son calque, sa durée (Préférences > Montage : 2 s d'origine).
    let p = &b.app.doc.panels[3];
    assert_eq!(p.layers.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["Dessin"]);
    assert_eq!(p.frame_count(), crate::dureeplans::images(b.app.settings.new_plan_secs, b.app.doc.fps));
    assert_eq!(p.frame_count(), 48);
    // Sa caméra : à l'œil de la vue, dans sa direction, sans rotation, 35 mm (la focale proposée la plus proche de 42).
    let vue = orbite.view(1000.0);
    let sc = Monde::du_plan(&b.app.doc, 3).scene_cam(&b.app.doc.panel_camera(3, 0));
    assert!(m::length(m::sub(sc.pos, vue.eye)) < 1e-2, "{:?} / {:?}", sc.pos, vue.eye);
    assert!(m::dot(sc.forward(), vue.forward) > 0.9999, "{:?} / {:?}", sc.forward(), vue.forward);
    assert!(sc.roll.abs() < 1e-3);
    assert!((sc.focal - 35.0).abs() < 1e-3, "{}", sc.focal);
    // Une étape : Ctrl+Z l'enlève.
    annuler(&mut b);
    assert_eq!(b.app.doc.panels.len(), n);
    // D'une vue orthographique : la perspective de l'onglet.
    b.app.o3_choisir_point(PointDeVue::Dessus);
    b.chauffer(2);
    assert!(b.app.e3_vues_multiples());
    let attendue = b.app.o3_orbite_perspective().expect("la perspective de l'onglet");
    b.app.o3_nouveau_plan_depuis_la_vue();
    assert_eq!(b.app.doc.panels.len(), n + 1);
    let pi = b.app.cur_panel;
    let sc = Monde::du_plan(&b.app.doc, pi).scene_cam(&b.app.doc.panel_camera(pi, 0));
    let vue = attendue.view(1000.0);
    assert!(m::length(m::sub(sc.pos, vue.eye)) < 1e-2 && m::dot(sc.forward(), vue.forward) > 0.9999);
}

// ------------------------------------------------------------------------------------------------ la mesure

/// La mesure de la partie 200 (dans le rapport) : ce que coûte un décor animé à la lecture, par image. La clé de l'image du décor (les
/// poses des objets à cette image et l'empreinte) sur le fil de l'interface ; et, quand le banc a une carte graphique, le dessin du décor
/// à la taille d'une feuille HD (une passe par image : la texture du décor est redessinée, jamais recréée).
#[test]
fn mesure_d_un_decor_anime_a_la_lecture() {
    let mut doc = Document::with_frame(1920, 1080, 24);
    doc.add_scene_after(0);
    for p in doc.panels.iter_mut() {
        p.set_length(48);
    }
    let seq = la_seq(&doc);
    let modele = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ombres: false, ..Default::default() });
    for k in 0..40 {
        let id = doc.alloc();
        let mut o = objet(id, "Carré", &modele, [(k % 8) as f32 - 4.0, 0.0, -2.0 - (k / 8) as f32], 0.4);
        if k % 2 == 0 {
            let mut p = o.place;
            p.position[0] += 1.0;
            o.cles = vec![CleObjet { image: 0, place: o.place }, CleObjet { image: 90, place: p }];
        }
        d.objets.push(o);
    }
    doc.decors3d.par_sequence.insert(seq, d);
    let t0 = std::time::Instant::now();
    let mut vues = std::collections::HashSet::new();
    for f in 0..48 {
        let k = cle_du_decor(&doc, 0, f);
        vues.insert(decor3d::id_of(&k));
    }
    let par_image = t0.elapsed().as_secs_f64() * 1000.0 / 48.0;
    assert_eq!(vues.len(), 48, "une image du décor par image du mouvement");
    eprintln!("MESURE 200 : clé du décor animé (40 objets, 20 animés), avec la composition du plan : {par_image:.3} ms par image");
    if let Some(rs) = crate::gpu_tests::headless() {
        let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
        let (mut temps, mut passes) = (Vec::new(), Vec::new());
        for f in 0..24 {
            let k = cle_du_decor(&doc, 0, f);
            let [fw, fh] = k.feuille;
            let objets = decor3d::objets_de(&doc, &k).unwrap();
            let dessins = decor3d::dessins_gpu(&objets, k.aspect);
            let t = std::time::Instant::now();
            crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &dessins, &k.prise().unwrap(), [fw as u32, fh as u32], false, crate::decor3dbudget::Qualite::Sans, 1).unwrap();
            temps.push(t.elapsed().as_secs_f64() * 1000.0);
            // (La passe seule, sans la relecture : ce que l'écran paie ; quatre images d'une soumission, la meilleure de trois séries.)
            passes.push(crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &dessins, &k.prise().unwrap(), [fw as u32, fh as u32], false, crate::decor3dbudget::Qualite::Sans, 4).unwrap().1 as f64);
        }
        temps.sort_by(f64::total_cmp);
        passes.sort_by(f64::total_cmp);
        eprintln!("MESURE 200 : dessin du décor animé par la carte, 1920 × 1080, avec la relecture : médiane {:.2} ms, pire {:.2} ms", temps[temps.len() / 2], temps[temps.len() - 1]);
        eprintln!("MESURE 200 : la passe seule (sans relecture) : médiane {:.3} ms, pire {:.3} ms ; budget d'une passe {:.2} ms", passes[passes.len() / 2], passes[passes.len() - 1], crate::decor3dbudget::BudgetMachine::ms_par_passe());
        // Avec huit lampes, le plafond.
        for k in 0..MAX_LAMPES {
            let id = doc.alloc();
            doc.decors3d.par_sequence.get_mut(&seq).unwrap().lampes.push(Lampe3d::neuve(id, format!("Lampe {k}"), [k as f32 - 4.0, 1.5, -3.0]));
        }
        let mut passes = Vec::new();
        for f in 0..24 {
            let k = cle_du_decor(&doc, 0, f);
            assert_eq!(k.aspect.lampes.n as usize, MAX_LAMPES);
            let [fw, fh] = k.feuille;
            let objets = decor3d::objets_de(&doc, &k).unwrap();
            let dessins = decor3d::dessins_gpu(&objets, k.aspect);
            passes.push(crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &dessins, &k.prise().unwrap(), [fw as u32, fh as u32], false, crate::decor3dbudget::Qualite::Sans, 4).unwrap().1 as f64);
        }
        passes.sort_by(f64::total_cmp);
        eprintln!("MESURE 200 : la passe seule avec huit lampes : médiane {:.3} ms, pire {:.3} ms", passes[passes.len() / 2], passes[passes.len() - 1]);
    } else {
        eprintln!("MESURE 200 : pas de carte graphique dans ce banc d'essai");
    }
}

// ------------------------------------------------------------------------------------------------ 200.7 : le plan qui devient gris

/// Un plan de 640 × 360 : deux carrés rouge-orangé dans le décor, et un calque peint d'un bloc vert, en bas au milieu de la feuille.
fn plan_decor_et_carte() -> Document {
    let mut doc = Document::with_frame(640, 360, 24);
    doc.add_scene_after(0);
    for p in doc.panels.iter_mut() {
        p.set_length(24);
    }
    let seq = la_seq(&doc);
    let monde = Monde::of(&doc);
    let k = -2.0 * monde.sol_y() / doc.frame[1] as f32;
    let largeur = doc.frame[0] as f32 * k;
    let modele = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: true, rendu: Rendu::Couleurs, ..Aspect::default() });
    let lid = doc.alloc();
    d.lampes.push(Lampe3d::neuve(lid, "Lampe".into(), [0.0, 0.5, 0.5]));
    for (i, x) in [-largeur / 4.0, largeur / 4.0].into_iter().enumerate() {
        let id = doc.alloc();
        let mut o = objet(id, &format!("Carré {i}"), &modele, [x, 0.0, -0.5], largeur / 8.0);
        o.couleur = Some(5);
        d.objets.push(o);
    }
    doc.decors3d.par_sequence.insert(seq, d);
    let (sw, sh) = (doc.width, doc.height);
    let mut rgba = vec![0u8; sw * sh * 4];
    let [fw, fh] = doc.frame;
    for y in sh / 2 + fh / 4..sh / 2 + fh / 4 + fh / 10 {
        for x in sw / 2 - fw / 10..sw / 2 + fw / 10 {
            rgba[(y * sw + x) * 4..(y * sw + x) * 4 + 4].copy_from_slice(&[30, 200, 60, 255]);
        }
    }
    for pi in 0..doc.panels.len() {
        let cel = doc.new_cel(LayerKind::Bitmap);
        doc.cels.get_mut(&cel).unwrap().raster = crate::raster::Raster::from_rgba(sw, sh, &rgba);
        let n = doc.panels[pi].frame_count();
        doc.panels[pi].layers = vec![Layer::new("Sol", LayerKind::Bitmap)];
        doc.panels[pi].layers[0].frames = vec![Some(cel); n];
    }
    doc
}

/// Dans la capture, la zone de la feuille (le dessin, à gauche de la vue à côté) : combien de pixels du décor (rouge-orangé) et de la
/// carte (vert).
fn decor_et_carte_a_l_ecran(b: &Banc, rgba: &[u8]) -> (usize, usize) {
    let w = b.taille.x as usize;
    let c = b.app.canvas_rect;
    let droite = if b.app.espace3d_a_cote_visible() { b.app.espace3d.rect.left().min(c.right()) } else { c.right() };
    let (mut rouge, mut vert) = (0, 0);
    for y in c.top().max(0.0) as usize..c.bottom() as usize {
        for x in c.left().max(0.0) as usize..droite as usize {
            let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
            let (r, g, bl) = (p[0] as i32, p[1] as i32, p[2] as i32);
            // (Le décor « dans les plans » en Couleurs : le bleu de ses objets.)
            if bl > r + 50 {
                rouge += 1;
            }
            if g > r + 80 && g > bl + 80 {
                vert += 1;
            }
        }
    }
    (rouge, vert)
}

/// Garde la capture (FEANOR_CAPTURE_DIR), pour la regarder.
fn capture(b: &Banc, rgba: &[u8], nom: &str) {
    if let Some(d) = std::env::var_os("FEANOR_CAPTURE_DIR") {
        let d = std::path::PathBuf::from(d);
        let _ = std::fs::create_dir_all(&d);
        let _ = image::save_buffer(d.join(format!("{nom}.png")), rgba, b.taille.x as u32, b.taille.y as u32, image::ColorType::Rgba8);
    }
}

/// 200.7, les hypothèses écartées : la vue à côté et l'image du plan ne se disputent pas la texture du décor (la vue dessine dans
/// ses propres cibles, `Moteur::prise`), et un nouveau rendu ne montre jamais une texture vide : la caméra qui bouge, la vue à côté
/// d'un autre aspect ou du même, les ombres, les bords doux et une lampe, le décor et la carte sont dans chaque image.
#[test]
fn moving_the_camera_with_the_side_view_open_never_empties_the_plan_image() {
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    // (Comme chez Anthony : les ombres et les bords doux du décor, une lampe.)
    decor3d::essai_bords(Some(true));
    decor3d::essai_poser(Some(crate::decor3dbudget::Qualite::Hautes), None);
    let mut b = banc_taille(plan_decor_et_carte(), true, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.app.renderer.invalidate_all();
    b.chauffer(30);
    let rgba = b.capturer().unwrap();
    capture(&b, &rgba, "200x-depart");
    let (r0, v0) = decor_et_carte_a_l_ecran(&b, &rgba);
    assert!(r0 > 50 && v0 > 50, "le plan seul : le décor et la carte ({r0}, {v0})");
    b.app.run_action("espace3d.a_cote");
    b.chauffer(10);
    assert!(b.app.espace3d_a_cote_visible(), "la vue à côté");
    for (aspect_vue, nom) in [(Rendu::Argile, "aspects différents"), (Rendu::Couleurs, "le même aspect")] {
        b.app.settings.aspect_vue_3d = aspect_vue;
        let mut vides = Vec::new();
        for k in 0..40 {
            let mut cam = b.app.current_camera();
            cam.x += if (k / 10) % 2 == 0 { 3.0 } else { -3.0 };
            b.app.set_current_camera(cam);
            b.image(Vec::new(), 1.0 / 60.0);
            let rgba = b.capturer().unwrap();
            let (r, v) = decor_et_carte_a_l_ecran(&b, &rgba);
            if r < 50 || v < 50 {
                if vides.is_empty() {
                    capture(&b, &rgba, &format!("200x-vide-{k}"));
                }
                vides.push((k, r, v));
            }
        }
        assert!(vides.is_empty(), "{nom} : des images du plan sans le décor ou sans la carte : {vides:?}");
    }
    // La caméra tirée à la souris dans la vue à côté, comme Anthony : appuyer sur elle, glisser deux secondes, en va-et-vient.
    b.app.settings.aspect_vue_3d = Rendu::Argile;
    let depart = b.app.current_camera();
    b.chauffer(4);
    let world = b.app.e3_world();
    let p = a_l_ecran(&b, world.scene_cam(&depart).pos);
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    let mut vides = Vec::new();
    let mut q = p;
    for k in 0..120 {
        q += vec2(if (k / 30) % 2 == 0 { 2.0 } else { -2.0 }, if (k / 20) % 2 == 0 { 0.7 } else { -0.7 });
        b.image(vec![Event::PointerMoved(q)], 1.0 / 60.0);
        let rgba = b.capturer().unwrap();
        let (r, v) = decor_et_carte_a_l_ecran(&b, &rgba);
        if r < 50 || v < 50 {
            if vides.is_empty() {
                capture(&b, &rgba, &format!("200x-souris-{k}"));
            }
            vides.push((k, r, v));
        }
    }
    b.image(vec![Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    assert!(b.app.current_camera() != depart, "la caméra a bougé");
    assert!(vides.is_empty(), "la caméra tirée dans la vue : des images du plan sans le décor ou sans la carte : {vides:?}");
    b.app.set_current_camera_libre(depart);
    decor3d::essai_bords(None);
    decor3d::essai_poser(None, None);
}


/// La caméra du « Plan 5 » d'Anthony (le 2026-11-20, rapportée à un cadre de 960 × 540) : posée haut dans la vue, presque contre le
/// plan de la feuille, elle plonge de 44° vers le décor, panoramique de −37°. Dans les anciennes limites (partie 159), mais son
/// cadre tombe bien au-dessus de la feuille.
fn camera_du_plan_5(doc: &Document) -> crate::camera::Camera {
    let [fw, _] = doc.frame;
    let (cx, cy) = (doc.width as f32 / 2.0, doc.height as f32 / 2.0);
    let d0 = crate::camera::ref_distance(doc.frame);
    crate::camera::Camera { x: cx + 0.1703 * fw as f32, y: cy - 1.96875 * fw as f32, zoom: 1.006267, angle: -5.854, dolly: 0.95 * d0, tilt: -44.1198, pan: -36.6579 }
}

/// Un décor de trois murs rouge-orangé, posés au sol là où plonge la caméra du « Plan 5 » ; le plan a cette caméra, libre.
fn plan_5() -> Document {
    let mut doc = Document::with_frame(960, 540, 24);
    let seq = la_seq(&doc);
    let sol = Monde::of(&doc).sol_y();
    let modele = modele_du_carre();
    let mut d = Decor3d::new(Aspect { ciel: false, brume: false, ombres: false, ..Aspect::default() });
    for (k, (x, z, r)) in [(-7.3, -10.9, 0.0), (-5.0, -12.5, 30.0), (-9.5, -9.5, -40.0), (-6.5, -8.5, 70.0)].into_iter().enumerate() {
        let id = doc.alloc();
        let mut o = objet(id, &format!("Mur {k}"), &modele, [x, sol + 1.5, z], 1.5);
        o.place.rotation_y = r;
        d.objets.push(o);
    }
    doc.decors3d.par_sequence.insert(seq, d);
    let cam = camera_du_plan_5(&doc);
    doc.panels[0].camera = vec![crate::camera::CameraKey::new(0, cam)];
    doc.panels[0].camera_libre = true;
    doc
}

#[test]
fn a_free_camera_whose_frame_leaves_the_sheet_still_films_the_decor_through_its_own_frame() {
    let doc = plan_5();
    let cam = doc.panel_camera_reelle(0, 0);
    // La cause : le chemin de toujours (le cadre sur la feuille) était pris, et le cadre n'y trouvait rien.
    assert!(doc.camera_dans_les_anciennes_limites(&cam), "dans les anciennes limites de la partie 159");
    assert!(!doc.cadre_sur_la_feuille(&cam), "son cadre sort de la feuille : {:?}", cam.corners(doc.frame));
    assert!(doc.repere_du_plan(0, 0).is_some(), "le plan passe par le repère de sa caméra");
    // L'export montre le décor (avant : rien du tout, pas un pixel).
    let img = exportee(&doc, 0, 0);
    let murs = img.iter().filter(|c| c.a() > 0 && c.r() as i32 > c.g() as i32 + 40).count();
    assert!(murs > img.len() / 50, "les murs du décor dans l'image : {murs} pixels sur {}", img.len());
    // La même caméra ramenée au-dessus de la feuille : le chemin de toujours, les mêmes pixels qu'avant.
    let mut doc2 = plan_5();
    doc2.panels[0].camera = vec![crate::camera::CameraKey::new(0, crate::camera::Camera { y: doc.height as f32 / 2.0, x: doc.width as f32 / 2.0, dolly: 0.0, tilt: -10.0, pan: 0.0, angle: 0.0, zoom: 1.0 })];
    let cam2 = doc2.panel_camera_reelle(0, 0);
    assert!(doc2.cadre_sur_la_feuille(&cam2) && doc2.repere_du_plan(0, 0).is_none());
}

#[test]
fn moving_the_plan_5_camera_in_the_side_view_never_greys_the_plan() {
    if crate::gpu_tests::headless().is_none() {
        return;
    }
    let mut b = banc_taille(plan_5(), true, 1600, 1000);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    b.app.run_action("espace3d.a_cote");
    b.chauffer(10);
    assert!(b.app.espace3d_a_cote_visible());
    let depart = b.app.current_camera();
    let mut vides = Vec::new();
    for k in 0..48 {
        // (Le va-et-vient d'un glisser dans la vue : de part et d'autre, plus haut, plus bas.)
        let t = (k as f32 * 0.4).sin();
        let cam = crate::camera::Camera { x: depart.x + 40.0 * t, y: depart.y + 25.0 * (k as f32 * 0.25).cos(), tilt: depart.tilt + 3.0 * t, ..depart };
        b.app.set_current_camera_libre(cam);
        b.image(Vec::new(), 1.0 / 60.0);
        let rgba = b.capturer().unwrap();
        let w = b.taille.x as usize;
        let c = b.app.canvas_rect;
        let droite = b.app.espace3d.rect.left().min(c.right());
        let mut murs = 0;
        for y in c.top().max(0.0) as usize..c.bottom() as usize {
            for x in c.left().max(0.0) as usize..droite as usize {
                let p = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
                if p[0] as i32 > p[1] as i32 + 50 && p[0] as i32 > p[2] as i32 + 50 {
                    murs += 1;
                }
            }
        }
        if murs < 50 {
            if vides.is_empty() {
                capture(&b, &rgba, &format!("200x-plan5-{k}"));
            }
            vides.push((k, murs));
        }
    }
    assert!(vides.is_empty(), "des images du plan sans le décor : {vides:?}");
}
