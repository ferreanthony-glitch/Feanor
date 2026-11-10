//! On n'enregistre plus (`continu.rs`, `jalonsui.rs`) : un changement s'écrit tout seul et sans rien dire, jamais au milieu
//! d'un geste ; fermer et quitter ne demandent rien ; un projet sans dossier garde la question ; un échec met l'écriture en
//! pause ; Ctrl+S pose un jalon ; l'ouverture en pose un avant la première écriture ; revenir à un jalon rend le projet.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::app::Pending;
use crate::document::{Cel, Document, Layer, LayerKind};
use crate::fiabilite_fixture::dossier;
use crate::jalons;
use crate::perf_tests::{banc, Banc};
use crate::project;

fn petit_projet(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(200, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

/// Des images d'écran jusqu'à ce que `fini` soit vrai (ou `limite`).
fn jusqu_a(b: &mut Banc, limite: Duration, fini: impl Fn(&Banc) -> bool) -> bool {
    let t = Instant::now();
    while t.elapsed() < limite {
        if fini(b) {
            return true;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    fini(b)
}

/// Tout est écrit : rien de modifié, aucune écriture ni aucun jalon en cours.
fn au_repos(b: &Banc) -> bool {
    !b.app.modified && !b.app.save_en_cours() && b.app.continu.jalon.is_none() && !b.app.continu.jalon_demande
}

/// L'écriture continue allumée, avec des délais d'essai (attente, calme, plafond) au lieu de vraies secondes.
fn allumer(b: &mut Banc) {
    b.app.settings.enregistrer_en_continu = true;
    plafond(Duration::from_millis(600));
}

/// Les délais d'essai, avec ce plafond.
fn plafond(p: Duration) {
    crate::continu::delais_d_essai(Some((Duration::from_millis(40), Duration::from_millis(20), p)));
}

/// Un petit projet déjà sur le disque, ouvert dans un banc où FEANOR écrit en continu : `(banc, dossier de travail, manifeste)`.
fn banc_continu(nom: &str) -> (Banc, PathBuf, PathBuf) {
    let dir = dossier(nom);
    let manifeste = dir.join("Essai").join("Essai.shot");
    let mut b = banc(petit_projet(4), false);
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| !b.app.save_en_cours()));
    assert!(!b.app.modified, "enregistré : {}", b.app.status);
    allumer(&mut b);
    (b, dir, manifeste)
}

/// Le dessin de la première image, modifié comme un trait : pixel changé, version et projet modifié.
fn modifier(b: &mut Banc, rouge: u8) {
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    let cel = b.app.doc.cels.get_mut(&id).expect("dessin");
    cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(rouge, 10, 10, 255);
    cel.version += 1;
    b.app.modified = true;
}

/// Le rouge du premier pixel du premier dessin tel que le fichier le garde.
fn rouge_sur_disque(manifeste: &Path) -> u8 {
    let loaded = project::load(manifeste).expect("projet lisible");
    let id = loaded.doc.panels[0].layers[0].frames[0].expect("dessin");
    loaded.doc.cels[&id].raster.get(0, 0).r()
}

/// Le rouge du même pixel dans le projet ouvert.
fn rouge_a_l_ecran(b: &Banc) -> u8 {
    let id = b.app.doc.panels[0].layers[0].frames[0].expect("dessin");
    b.app.doc.cels[&id].raster.get(0, 0).r()
}

#[test]
fn a_change_is_written_by_itself_and_says_nothing() {
    let (mut b, dir, manifeste) = banc_continu("continu-seul");
    assert!(b.app.continu_actif());
    b.app.status = "Le message du dessin.".into();
    modifier(&mut b, 77);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "le changement s'écrit tout seul : {}", b.app.status);
    assert_eq!(rouge_sur_disque(&manifeste), 77);
    assert_eq!(b.app.status, "Le message du dessin.", "l'écriture continue ne touche pas à la barre d'état");
    assert!(b.app.pending.is_none() && b.app.save_error.is_none());
    // L'onglet ne montre pas de pastille « à enregistrer », même pendant l'instant où le projet attend d'être écrit.
    modifier(&mut b, 78);
    let info = b.app.infos_onglets().remove(0);
    assert!(info.a_perdre() && info.continu && !info.pastille());
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    assert_eq!(rouge_sur_disque(&manifeste), 78);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_written_while_the_film_plays_until_the_ceiling() {
    let (mut b, dir, manifeste) = banc_continu("continu-geste");
    // Pendant la lecture, on n'écrit pas tout de suite (une image de film ne doit pas sauter pour un cliché).
    plafond(Duration::from_secs(30));
    b.app.playing = true;
    modifier(&mut b, 61);
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(900) {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
        b.app.playing = true;
        assert!(!b.app.save_en_cours(), "une écriture est partie pendant la lecture");
    }
    assert!(b.app.modified && rouge_sur_disque(&manifeste) == 200);
    // Mais une lecture laissée en boucle ne retient pas l'écriture au-delà du plafond.
    plafond(Duration::from_millis(300));
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(90) && (b.app.modified || b.app.save_en_cours()) {
        b.app.playing = true;
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    assert_eq!(rouge_sur_disque(&manifeste), 61, "passé le plafond, le projet s'écrit même pendant la lecture");
    b.app.playing = false;
    plafond(Duration::from_millis(600));
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    // Le réglage décoché : comme avant, rien ne s'écrit tout seul.
    b.app.settings.enregistrer_en_continu = false;
    modifier(&mut b, 62);
    assert!(!jusqu_a(&mut b, Duration::from_millis(600), |b| !b.app.modified), "réglage décoché : seul « Enregistrer » écrit");
    assert_eq!(rouge_sur_disque(&manifeste), 61);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nothing_is_written_while_the_pen_is_down_and_feanor_rests_once_it_is_written() {
    use eframe::egui::{pos2, Event, Modifiers, PointerButton};
    let (mut b, dir, manifeste) = banc_continu("continu-stylet");
    b.chauffer(3);
    // Le stylet se pose (un appui tenu, où que ce soit) : tant qu'il est posé, rien ne s'écrit.
    let p = pos2(960.0, 540.0);
    b.image(vec![Event::PointerMoved(p)], 0.016);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 0.016);
    modifier(&mut b, 21);
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(900) {
        b.image(vec![Event::PointerMoved(p)], 0.016);
        std::thread::sleep(Duration::from_millis(4));
        assert!(!b.app.save_en_cours(), "une écriture est partie pendant que le stylet était posé");
    }
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    // Il se lève : le projet s'écrit.
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 0.016);
    b.image(vec![Event::PointerGone], 0.016);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "{}", b.app.status);
    assert_eq!(rouge_sur_disque(&manifeste), 21);
    // Tout est écrit : FEANOR ne demande plus à être repeint (l'écriture continue ne tourne pas à vide).
    for _ in 0..12 {
        b.image(Vec::new(), 0.5);
    }
    let pire = (0..4).map(|_| b.image(Vec::new(), 0.5).reveil).min().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "au repos, FEANOR demande à être repeint dans {pire:?}{}", b.raison_du_reveil());
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_continuous_writing_is_for_projects_with_a_folder_that_are_neither_shared_nor_a_milestone() {
    let c = crate::continu::Continu::default();
    let projet = Path::new("C:/Films/Bus/Bus.shot");
    assert!(c.vaut(true, Some(projet), false));
    assert!(!c.vaut(false, Some(projet), false), "réglage décoché");
    assert!(!c.vaut(true, None, false), "projet sans dossier");
    assert!(!c.vaut(true, Some(projet), true), "film partagé : enregistrer y publie le travail, c'est un geste voulu");
    assert!(!c.vaut(true, Some(Path::new("C:/Films/Bus/jalons/2026-11-10_19-42-07/Bus.shot")), false), "un jalon ouvert");
    let mut en_pause = crate::continu::Continu::default();
    en_pause.pause = Some("disque plein".into());
    assert!(!en_pause.vaut(true, Some(projet), false), "en pause après un échec");
}

#[test]
fn an_old_format_project_is_kept_as_it_was_in_the_opening_milestone() {
    let dir = dossier("continu-ancien");
    let ancien = dir.join("Ancien").join("Ancien.pelure");
    project::save(&petit_projet(3), &ancien, &mut Default::default()).expect("enregistrement");
    // Le même projet, tel qu'une version d'avant l'aurait écrit (format 3, sans cadre) : l'ouvrir le convertit.
    let mut json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&ancien).expect("manifeste")).expect("json");
    json["version"] = 3.into();
    json.as_object_mut().expect("objet").remove("cadre");
    std::fs::write(&ancien, serde_json::to_vec(&json).expect("json")).expect("manifeste");
    let avant = crate::fiabilite_fixture::arbre(ancien.parent().expect("dossier"));
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    allumer(&mut b);
    b.app.open_path(ancien.clone());
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.open_job.is_none()), "{}", b.app.status);
    assert!(b.app.status.contains("gardé dans un jalon"), "{}", b.app.status);
    // Le projet se réécrit tout seul au nouveau format, sous son nouveau nom…
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "{}", b.app.status);
    let projet = ancien.parent().expect("dossier").to_path_buf();
    let neuf = projet.join("Ancien.shot");
    assert!(neuf.is_file() && !ancien.exists(), "le manifeste a pris son nom d'aujourd'hui");
    assert!(!project::load(&neuf).expect("projet").converted);
    // … et le jalon de l'ouverture le garde tel qu'il était, octet pour octet : une version d'avant peut encore l'ouvrir.
    let liste = jalons::lister(&projet);
    assert_eq!(liste.len(), 1);
    let garde: std::collections::BTreeMap<String, u64> = crate::fiabilite_fixture::arbre(&liste[0].dossier).into_iter().filter(|(k, _)| k != "jalon.json").collect();
    assert_eq!(garde, avant, "le jalon est l'ancien projet");
    assert!(project::load(&liste[0].manifeste().expect("manifeste du jalon")).expect("jalon").converted);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_folder_follows_the_work_through_undo_and_redo() {
    let (mut b, dir, manifeste) = banc_continu("continu-annuler");
    let sur_disque = |m: &Path| {
        let l = project::load(m).expect("le projet reste lisible à chaque écriture");
        assert!(l.warnings.is_empty(), "{:?}", l.warnings);
        l.doc.panels.len()
    };
    // Un plan de plus, annulé, rétabli : le dossier suit chaque état, et reste un projet sans dessin manquant.
    b.app.run_action("panneau.nouveau");
    assert_eq!(b.app.doc.panels.len(), 2);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "{}", b.app.status);
    assert_eq!(sur_disque(&manifeste), 2);
    b.app.run_action("edition.annuler");
    assert_eq!(b.app.doc.panels.len(), 1);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "{}", b.app.status);
    assert_eq!(sur_disque(&manifeste), 1);
    b.app.run_action("edition.retablir");
    assert_eq!(b.app.doc.panels.len(), 2);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos), "{}", b.app.status);
    assert_eq!(sur_disque(&manifeste), 2);
    // Ce que le dossier contient est ce qu'on voit : relu, c'est le même projet.
    let relu = project::load(&manifeste).expect("projet");
    let vide = std::collections::BTreeSet::new();
    crate::fiabilite_fixture::meme_projet(&crate::fiabilite_fixture::instantane(&b.app.doc, &vide), &crate::fiabilite_fixture::instantane(&relu.doc, &vide)).expect("le dossier est le projet ouvert");
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_asks_nothing_and_writes_first() {
    let (mut b, dir, manifeste) = banc_continu("continu-quitter");
    modifier(&mut b, 55);
    b.app.request(Pending::Quit);
    assert!(b.app.pending.is_none(), "aucune question « Modifications non enregistrées »");
    assert!(!b.app.allow_close || !b.app.modified, "on ne quitte pas avant d'avoir écrit");
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| b.app.allow_close), "Quitter a lieu à la fin de l'écriture : {}", b.app.status);
    assert!(!b.app.modified);
    assert_eq!(rouge_sur_disque(&manifeste), 55);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_project_without_a_folder_keeps_the_question() {
    let mut b = banc(petit_projet(2), false);
    allumer(&mut b);
    assert!(!b.app.continu_actif(), "sans dossier, FEANOR n'a nulle part où écrire");
    modifier(&mut b, 44);
    assert!(!jusqu_a(&mut b, Duration::from_millis(400), |b| !b.app.modified));
    b.app.request(Pending::Quit);
    assert!(b.app.pending.is_some() && !b.app.allow_close, "la question de toujours");
    let info = b.app.infos_onglets().remove(0);
    assert!(info.pastille() && !info.continu);
    crate::continu::delais_d_essai(None);
}

#[test]
fn a_failed_write_pauses_the_continuous_writing_until_a_save_succeeds() {
    let (mut b, dir, manifeste) = banc_continu("continu-echec");
    // Le manifeste ne peut plus être écrit (son fichier provisoire est occupé par un dossier).
    let obstacle = manifeste.parent().expect("dossier").join("Essai.shot.tmp");
    std::fs::create_dir_all(&obstacle).expect("obstacle");
    modifier(&mut b, 33);
    b.app.doc.panels[0].notes = "Plan retouché.".into();
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| b.app.save_error.is_some()), "l'échec se dit : {}", b.app.status);
    assert!(b.app.modified, "rien n'est marqué écrit à tort");
    assert!(b.app.continu.pause.is_some() && !b.app.continu_actif(), "l'écriture continue est en pause");
    // La fenêtre d'erreur fermée : elle ne revient pas chaque seconde, rien ne repart tout seul.
    b.app.save_error = None;
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(500) {
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
        assert!(!b.app.save_en_cours() && b.app.save_error.is_none(), "l'écriture en pause est repartie");
    }
    // En pause, fermer redemande, comme pour un projet qu'on enregistre à la main.
    b.app.request(Pending::Quit);
    assert!(b.app.pending.take().is_some() && !b.app.allow_close);
    // L'obstacle levé, un enregistrement réussi relance l'écriture continue.
    std::fs::remove_dir_all(&obstacle).expect("obstacle levé");
    b.app.run_action("fichier.enregistrer");
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| !b.app.modified && !b.app.save_en_cours()), "{}", b.app.status);
    assert!(b.app.continu.pause.is_none() && b.app.continu_actif());
    assert_eq!(rouge_sur_disque(&manifeste), 33);
    modifier(&mut b, 34);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    assert_eq!(rouge_sur_disque(&manifeste), 34);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ctrl_s_writes_what_is_left_then_poses_a_milestone() {
    let (mut b, dir, manifeste) = banc_continu("continu-jalon");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    modifier(&mut b, 91);
    b.app.run_action("fichier.enregistrer");
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| au_repos(b) && !jalons::lister(&projet).is_empty()), "le jalon se pose : {}", b.app.status);
    let liste = jalons::lister(&projet);
    assert_eq!(liste.len(), 1);
    assert!(!liste[0].fiche.auto, "un jalon demandé reste jusqu'à ce qu'on le supprime");
    assert!(b.app.status.starts_with("Jalon posé à "), "{}", b.app.status);
    assert_eq!(b.app.action_du_message(), Some(crate::messages::ActionMessage::NommerJalon(liste[0].dossier.clone())), "la barre d'état propose de le nommer");
    // Le jalon a ce qui venait d'être dessiné : l'écriture est passée avant lui.
    assert_eq!(rouge_sur_disque(&liste[0].manifeste().expect("manifeste du jalon")), 91);
    // La suite du travail ne le change pas.
    modifier(&mut b, 92);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    assert_eq!(rouge_sur_disque(&manifeste), 92);
    assert_eq!(rouge_sur_disque(&liste[0].manifeste().expect("manifeste du jalon")), 91);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ouvre `manifeste` dans un banc neuf où FEANOR écrit en continu, comme Fichier > Ouvrir, et attend le projet et son jalon d'ouverture.
fn banc_ouvert(manifeste: &Path) -> Banc {
    let mut b = banc(Document::with_frame(320, 180, 24), false);
    allumer(&mut b);
    b.app.open_path(manifeste.to_path_buf());
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.open_job.is_none()), "l'ouverture finit : {}", b.app.status);
    assert!(b.app.path.as_deref().is_some_and(|p| crate::onglets::meme_chemin(p, manifeste)), "le projet est ouvert : {:?}", b.app.path);
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.continu.jalon.is_none()), "le jalon d'ouverture finit");
    b
}

#[test]
fn opening_poses_a_milestone_before_the_first_write_and_returning_to_it_brings_the_project_back() {
    let (b, dir, manifeste) = banc_continu("continu-ouverture");
    drop(b);
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    let mut b = banc_ouvert(&manifeste);
    let liste = jalons::lister(&projet);
    assert_eq!(liste.len(), 1, "un jalon à l'ouverture");
    assert!(liste[0].fiche.auto && liste[0].fiche.nom == "À l'ouverture");
    assert!(b.app.continu.pause.is_none());
    // Une séance de travail : tout s'écrit tout seul, le jalon garde le projet tel qu'on l'a ouvert.
    modifier(&mut b, 12);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    assert_eq!(rouge_sur_disque(&manifeste), 12);
    assert_eq!(rouge_sur_disque(&liste[0].manifeste().expect("manifeste du jalon")), 200);
    // Un dernier trait, pas encore écrit, puis on revient au jalon : rien ne se perd, l'état d'aujourd'hui est gardé.
    modifier(&mut b, 13);
    b.app.jalon_revenir(&liste[0].dossier);
    assert!(jusqu_a(&mut b, Duration::from_secs(120), |b| b.app.open_job.is_none()), "le projet se rouvre : {}", b.app.status);
    assert!(b.app.path.as_deref().is_some_and(|p| crate::onglets::meme_chemin(p, &manifeste)), "{:?} — {}", b.app.path, b.app.status);
    assert_eq!(rouge_a_l_ecran(&b), 200, "le projet est revenu à son état de l'ouverture");
    assert_eq!(rouge_sur_disque(&manifeste), 200);
    assert!(!b.app.modified && b.app.history.is_empty());
    let apres = jalons::lister(&projet);
    let avant_retour = apres.iter().find(|j| j.fiche.nom.starts_with("Avant le retour")).expect("le jalon de l'état d'aujourd'hui");
    assert_eq!(rouge_sur_disque(&avant_retour.manifeste().expect("manifeste du jalon")), 13, "même le trait pas encore écrit y est");
    // Et l'écriture continue reprend sur le projet rouvert.
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.continu.jalon.is_none()));
    modifier(&mut b, 14);
    assert!(jusqu_a(&mut b, Duration::from_secs(90), au_repos));
    assert_eq!(rouge_sur_disque(&manifeste), 14);
    assert_eq!(rouge_sur_disque(&avant_retour.manifeste().expect("manifeste du jalon")), 13);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopening_an_untouched_project_does_not_pile_up_milestones() {
    let (b, dir, manifeste) = banc_continu("continu-rouvrir");
    drop(b);
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    drop(banc_ouvert(&manifeste));
    assert_eq!(jalons::lister(&projet).len(), 1);
    drop(banc_ouvert(&manifeste));
    assert_eq!(jalons::lister(&projet).len(), 1, "rien n'a changé depuis le dernier jalon : pas de second");
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn leaving_a_tab_writes_its_project() {
    let (mut b, dir, manifeste) = banc_continu("continu-onglet");
    modifier(&mut b, 71);
    assert!(b.app.nouvel_onglet(), "un second onglet");
    // Le projet rangé s'écrit pendant qu'il est caché ; son onglet n'a rien à perdre ensuite.
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| !b.app.infos_onglets()[0].a_perdre()), "{}", b.app.status);
    assert_eq!(rouge_sur_disque(&manifeste), 71);
    assert!(!b.app.status.contains("Enregistré"), "l'écriture d'un onglet caché ne dit rien non plus : {}", b.app.status);
    // Quitter avec un onglet caché déjà écrit : aucune question.
    b.app.request(Pending::Quit);
    assert!(b.app.pending.is_none() && b.app.groupe.is_none() && b.app.allow_close);
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_milestone_opened_to_look_at_it_is_never_written() {
    let (mut b, dir, manifeste) = banc_continu("continu-jalon-ouvert");
    let projet = manifeste.parent().expect("dossier").to_path_buf();
    b.app.run_action("fichier.enregistrer");
    assert!(jusqu_a(&mut b, Duration::from_secs(90), |b| au_repos(b) && !jalons::lister(&projet).is_empty()));
    let jalon = jalons::lister(&projet).remove(0);
    let dans_le_jalon = jalon.manifeste().expect("manifeste du jalon");
    drop(b);
    let mut c = banc(Document::with_frame(320, 180, 24), false);
    allumer(&mut c);
    c.app.open_path(dans_le_jalon.clone());
    assert!(jusqu_a(&mut c, Duration::from_secs(120), |b| b.app.open_job.is_none()));
    assert!(c.app.path.as_deref() == Some(dans_le_jalon.as_path()));
    assert!(!c.app.continu_actif(), "un jalon ouvert n'est pas écrit en continu");
    assert!(jalons::lister(dans_le_jalon.parent().expect("dossier")).is_empty(), "et il n'a pas de jalons à lui");
    modifier(&mut c, 5);
    assert!(!jusqu_a(&mut c, Duration::from_millis(400), |b| !b.app.modified));
    assert!(!c.app.save(), "« Enregistrer » y est refusé");
    assert!(c.app.save_error.take().is_some_and(|e| e.contains("jalon")));
    assert_eq!(rouge_sur_disque(&dans_le_jalon), 200, "le jalon est resté tel qu'il était");
    crate::continu::delais_d_essai(None);
    let _ = std::fs::remove_dir_all(&dir);
}
