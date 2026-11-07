//! Journal des différences (`journal.rs`, `journaljob.rs`, `journalui.rs`) : ce qui a changé, ce qui est ajouté, retiré, déplacé, la part
//! de pixels, les notes traitées entre deux envois, la page HTML et le PDF écrits, l'arrêt et l'échec qui ne laissent aucun fichier, la
//! fenêtre dans l'application, le paquet de livraison.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{CelId, Document, Layer, LayerKind};
use crate::envois::{self, Copie, Demande, NouvelEnvoi, PlanCopie};
use crate::fiabilite_fixture::dossier;
use crate::fildecision::Status;
use crate::journal::{self, comparer, Genre, PlanVu};
use crate::journaljob::{Cote, Demande as DemandeJ, JournalJob, Rapport};
use crate::paquet::{PaquetJob, Reglages as PaquetReglages};
use crate::perf_tests::{banc, Banc};
use crate::project::{self, Tete};

// ---------------------------------------------------------------------------------------------- les plans comparés

fn pv(id: u64, empreinte: u64, images: usize) -> PlanVu {
    PlanVu { id, titre: format!("Plan {id}"), empreinte, version: 1, images }
}

fn sans_noms() -> HashMap<u64, String> {
    HashMap::new()
}

#[test]
fn only_the_plans_whose_content_changed_are_listed() {
    let a = [pv(1, 10, 24), pv(2, 20, 24), pv(3, 30, 24)];
    let b = [pv(1, 10, 24), pv(2, 21, 36), pv(3, 30, 24)];
    let d = comparer(&a, &b, &sans_noms());
    assert_eq!(d.lignes.len(), 1);
    assert_eq!(d.inchanges, 2);
    let l = &d.lignes[0];
    assert_eq!((l.id, l.genre, l.deplace), (2, Genre::Modifie, false));
    assert_eq!((l.rang_a, l.rang_b), (Some(2), Some(2)));
    assert_eq!((l.images_a, l.images_b), (Some(24), Some(36)));
    assert_eq!((d.modifies(), d.ajoutes(), d.retires(), d.deplaces()), (1, 0, 0, 0));
    // Aucun changement : un journal vide, qui le dit.
    let rien = comparer(&a, &a, &sans_noms());
    assert!(rien.est_vide() && rien.inchanges == 3);
}

#[test]
fn added_removed_and_moved_plans_are_told_apart() {
    // 3 est retiré, 4 est ajouté ; 1 et 2 gardent leur ordre.
    let a = [pv(1, 10, 4), pv(2, 20, 4), pv(3, 30, 4)];
    let b = [pv(1, 10, 4), pv(2, 20, 4), pv(4, 40, 4)];
    let d = comparer(&a, &b, &sans_noms());
    let genres: Vec<(u64, Genre)> = d.lignes.iter().map(|l| (l.id, l.genre)).collect();
    assert_eq!(genres, [(4, Genre::Ajoute), (3, Genre::Retire)]);
    assert_eq!(d.inchanges, 2);
    assert_eq!((d.lignes[0].rang_a, d.lignes[0].rang_b), (None, Some(3)));
    assert_eq!((d.lignes[1].rang_a, d.lignes[1].rang_b), (Some(3), None));
    // Le plan retiré garde son titre ; un plan qui reste prend le nom d'aujourd'hui.
    let noms: HashMap<u64, String> = [(4, "SQ01_PL004".to_string())].into_iter().collect();
    let d = comparer(&a, &b, &noms);
    assert_eq!((d.lignes[0].nom.as_str(), d.lignes[1].nom.as_str()), ("SQ01_PL004", "Plan 3"));
    // Un seul plan qui saute en fin de film : lui seul est déplacé, pas les quatre autres.
    let a = [pv(1, 1, 4), pv(2, 2, 4), pv(3, 3, 4), pv(4, 4, 4), pv(5, 5, 4)];
    let b = [pv(1, 1, 4), pv(3, 3, 4), pv(4, 4, 4), pv(5, 5, 4), pv(2, 2, 4)];
    let d = comparer(&a, &b, &sans_noms());
    assert_eq!(d.lignes.len(), 1);
    assert_eq!((d.lignes[0].id, d.lignes[0].genre, d.lignes[0].deplace), (2, Genre::Deplace, true));
    assert_eq!((d.lignes[0].rang_a, d.lignes[0].rang_b), (Some(2), Some(5)));
    assert_eq!((d.inchanges, d.deplaces()), (4, 1));
    // Un plan modifié ET déplacé reste « modifié » et porte l'étiquette « déplacé ».
    let b2 = [pv(1, 1, 4), pv(3, 3, 4), pv(4, 4, 4), pv(5, 5, 4), pv(2, 22, 4)];
    let d = comparer(&a, &b2, &sans_noms());
    assert_eq!((d.lignes[0].genre, d.lignes[0].deplace), (Genre::Modifie, true));
    // Deux plans échangés : un seul est déplacé (l'autre sert de repère).
    let c = [pv(1, 1, 4), pv(3, 3, 4), pv(2, 2, 4)];
    let d = comparer(&[pv(1, 1, 4), pv(2, 2, 4), pv(3, 3, 4)], &c, &sans_noms());
    assert_eq!(d.deplaces(), 1);
}

#[test]
fn the_summary_and_the_texts_read_well() {
    assert_eq!(journal::pct_texte(0.0), "0 %");
    assert_eq!(journal::pct_texte(0.00001), "< 0,1 %");
    assert_eq!(journal::pct_texte(0.1234), "12,3 %");
    assert_eq!(journal::durees_texte(Some(24), Some(36), 24, "→"), "1s 00 → 1s 12 (+12 i)");
    assert_eq!(journal::durees_texte(Some(30), Some(10), 24, "->"), "1s 06 -> 10 i (-20 i)");
    assert_eq!(journal::durees_texte(Some(4), Some(4), 24, "→"), "4 i (inchangée)");
    assert_eq!(journal::pct_phrase(0.0), "image identique : le changement est dans la durée, la caméra ou les réglages");
    assert_eq!(journal::pct_phrase(0.062), "6,2 % de l'image diffère");
    assert_eq!(journal::durees_texte(None, Some(48), 24, "→"), "2s 00");
}

// ---------------------------------------------------------------------------------------------- les notes

fn film_simple(n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    for k in 1..n {
        doc.add_scene_after(k - 1);
    }
    doc
}

#[test]
fn only_the_treated_notes_between_the_two_sends_are_kept() {
    let mut doc = film_simple(2);
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    let mut note = |plan: u64, creee: u64, statut: Status, raison: &str, traitee: u64| {
        let id = doc.add_comment(plan, 0, "Léa", creee);
        doc.comment_mut(id).unwrap().text = format!("note {id}");
        if statut != Status::Todo {
            doc.set_note_status(id, statut, raison, traitee).unwrap();
        }
        id
    };
    let fait = note(a, 100, Status::Done, "", 1_500);
    let refuse = note(b, 100, Status::Refused, "Hors budget", 2_000);
    let discussion = note(a, 100, Status::Discussion, "", 2_001);
    let a_faire = note(a, 1_500, Status::Todo, "", 0);
    let trop_tot = note(a, 100, Status::Done, "", 1_000);
    // Une note résolue d'avant le fil de décision : pas de date de statut, on prend celle de la note.
    let ancienne = note(b, 1_700, Status::Todo, "", 0);
    doc.comment_mut(ancienne).unwrap().done = true;
    let ids = |v: Vec<journal::NoteJ>| v.into_iter().map(|n| n.id).collect::<Vec<_>>();
    // Après 1000 (exclu), jusqu'à 2000 (compris) : le fait, l'ancienne, le refus, dans l'ordre des dates.
    assert_eq!(ids(journal::notes_entre(&doc, 1_000, Some(2_000))), [fait, ancienne, refuse]);
    // Jusqu'à aujourd'hui : la note en discussion s'ajoute ; une note « à faire » ne compte jamais.
    assert_eq!(ids(journal::notes_entre(&doc, 1_000, None)), [fait, ancienne, refuse, discussion]);
    let _ = (a_faire, trop_tot);
    let r = &journal::notes_entre(&doc, 1_000, Some(2_000))[2];
    assert_eq!((r.statut, r.raison.as_str(), r.auteur.as_str()), (Status::Refused, "Hors budget", "Léa"));
    // Le côté d'un envoi : l'instant où SHOT l'a noté, à défaut le jour choisi.
    let mut e = crate::envois::Envoi { id: 1, nom: "v1".into(), date: 5, note_le: 0, fichier: String::new(), plans: Vec::new(), variantes: Vec::new() };
    assert_eq!(journal::instant_de(&e), 5);
    e.note_le = 9;
    assert_eq!(journal::instant_de(&e), 9);
}

// ---------------------------------------------------------------------------------------------- un film réel, deux envois

/// Un rectangle plein sur le dessin `cel` : la feuille fait 640 x 360, le cadre de 320 x 180 est au centre (x 160..480, y 90..270).
fn rectangle(doc: &mut Document, cel: CelId, x: std::ops::Range<usize>, y: std::ops::Range<usize>, couleur: Color32) {
    let c = doc.cels.get_mut(&cel).unwrap();
    for yy in y {
        for xx in x.clone() {
            let t = c.raster.tile_index_of(xx, yy);
            c.raster.tile_mut(t)[(yy % 64) * 64 + xx % 64] = couleur;
        }
    }
    c.version += 1;
}

/// Un film de `n` plans de 4 images : un calque bitmap, deux dessins, chacun avec un rectangle dans le cadre dont la couleur dépend du plan.
fn film(n: usize) -> Document {
    let mut doc = film_simple(n);
    for i in 0..n {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        let ids = [doc.new_cel(LayerKind::Bitmap), doc.new_cel(LayerKind::Bitmap)];
        layer.frames = vec![Some(ids[0]), Some(ids[0]), Some(ids[1]), Some(ids[1])];
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 4;
        rectangle(&mut doc, ids[0], 170..250, 100..160, Color32::from_rgba_premultiplied(20 + (i * 30) as u8, 40, 90, 255));
        rectangle(&mut doc, ids[1], 260..330, 100..160, Color32::from_rgba_premultiplied(20 + (i * 30) as u8, 100, 90, 255));
    }
    doc
}

/// Un carré noir de 60 x 60 dans le cadre, sur le premier dessin du plan de rang `rang` : 6 % de l'image.
fn noircir(doc: &mut Document, rang: usize) {
    let id = doc.panels[rang].layers[0].frames[0].unwrap();
    rectangle(doc, id, 400..460, 190..250, Color32::from_rgba_premultiplied(0, 0, 0, 255));
}
fn demande_envois(doc: &Document, envoi: Option<NouvelEnvoi>) -> Demande {
    let plans: Vec<PlanCopie> = doc.panels.iter().enumerate().map(|(i, p)| PlanCopie { panel: p.clone(), titre: format!("Plan {}", i + 1), version: 1, nom: String::new(), sequence: String::new() }).collect();
    let mut cels = HashMap::new();
    for p in &doc.panels {
        for id in crate::empreinte::dessins_du_plan(p) {
            if let Some(c) = doc.cels.get(&id) {
                cels.insert(id, Copie { version: c.version, donnees: crate::empreinte::Donnees::de(c) });
            }
        }
    }
    Demande { plans, cels, connus: HashMap::new(), tete: Tete::de(doc), envoi }
}

/// Marque un envoi (comme l'application, sans l'interface), noté par SHOT à l'instant `note_le`.
fn marquer(doc: &mut Document, nom: &str, note_le: u64) -> u64 {
    let id = doc.envois.prochain_numero();
    let nouveau = NouvelEnvoi { id, nom: nom.into(), date: 1_760_000_000 + id * 86_400, note_le, unique: 0xAB00 + id as u32 };
    let r = envois::construire(demande_envois(doc, Some(nouveau))).expect("envoi");
    let (envoi, instantane) = r.envoi.expect("l'envoi");
    doc.envois.ajouter(envoi, instantane);
    id
}

fn manifeste(dir: &Path) -> PathBuf {
    dir.join("Film").join("Film.pelure")
}

fn enregistrer(doc: &Document, manifeste: &Path, written: &mut HashMap<CelId, u64>) {
    project::prepare_save(doc, manifeste, written, None).expect("préparation").run().expect("écriture");
}

fn nom_du_plan(doc: &Document, id: u64) -> String {
    doc.nom_de_plan(doc.panels.iter().position(|p| p.id == id).expect("le plan"))
}

/// Le film d'essai : 5 plans, un envoi, puis un plan retouché (≈ 7 % de l'image), un plan rallongé, un plan retiré, un plan ajouté et un
/// plan déplacé en fin de film, un second envoi. Rend aussi les numéros des plans touchés.
struct Scenario {
    doc: Document,
    dir: PathBuf,
    a: u64,
    b: u64,
    retouche: u64,
    rallonge: u64,
    retire: u64,
    ajoute: u64,
    deplace: u64,
    intact: u64,
}

fn scenario(nom: &str) -> Scenario {
    let travail = dossier(nom);
    let m = manifeste(&travail);
    let mut doc = film(5);
    let ids: Vec<u64> = doc.panels.iter().map(|p| p.id).collect();
    let mut written = HashMap::new();
    let a = marquer(&mut doc, "v1 au réal", 1_000);
    enregistrer(&doc, &m, &mut written);
    // Un plan retouché, un plan rallongé, un plan retiré, un plan ajouté, un plan déplacé.
    noircir(&mut doc, 1);
    doc.panels[2].set_length(6);
    doc.panels.remove(4);
    doc.add_scene_after(3);
    let ajoute = doc.panels[4].id;
    doc.panels.rotate_left(1);
    let b = marquer(&mut doc, "v2 au réal", 2_000);
    enregistrer(&doc, &m, &mut written);
    Scenario { dir: m.parent().unwrap().to_path_buf(), doc, a, b, retouche: ids[1], rallonge: ids[2], retire: ids[4], ajoute, deplace: ids[0], intact: ids[3] }
}

fn journal_entre(s: &Scenario) -> journal::Journal {
    let (ea, eb) = (s.doc.envois.trouve(s.a).unwrap(), s.doc.envois.trouve(s.b).unwrap());
    journal::faire(&s.doc, ea, Some(eb), Vec::new(), 1_761_000_000)
}

fn cote(s: &Scenario, id: u64) -> Cote {
    Cote::Envoi { dossier: s.dir.clone(), envoi: Box::new(s.doc.envois.trouve(id).unwrap().clone()) }
}

fn attendre(job: &mut JournalJob) -> Result<Rapport, String> {
    let t = Instant::now();
    while job.running() {
        job.poll();
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120), "le journal est trop long");
    }
    job.outcome.take().expect("compte rendu")
}

fn fichiers_sous(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}

#[test]
fn two_real_sends_give_the_changed_added_removed_and_moved_plans() {
    let s = scenario("journal-liste");
    let j = journal_entre(&s);
    let d = &j.diff;
    let genre = |id: u64| d.lignes.iter().find(|l| l.id == id).map(|l| (l.genre, l.deplace));
    assert_eq!(genre(s.retouche), Some((Genre::Modifie, false)));
    assert_eq!(genre(s.rallonge), Some((Genre::Modifie, false)));
    assert_eq!(genre(s.ajoute), Some((Genre::Ajoute, false)));
    assert_eq!(genre(s.retire), Some((Genre::Retire, false)));
    assert_eq!(genre(s.deplace), Some((Genre::Deplace, true)));
    assert_eq!(genre(s.intact), None, "un plan qui n'a pas bougé n'est pas dans le journal");
    assert_eq!((d.modifies(), d.ajoutes(), d.retires(), d.deplaces(), d.inchanges), (2, 1, 1, 1, 1));
    // Le plan rallongé : 4 images → 6, le plan retiré garde son titre d'alors, les autres le nom d'aujourd'hui.
    let l = d.lignes.iter().find(|l| l.id == s.rallonge).unwrap();
    assert_eq!((l.images_a, l.images_b), (Some(4), Some(6)));
    assert_eq!(l.nom, nom_du_plan(&s.doc, s.rallonge));
    assert_eq!(d.lignes.iter().find(|l| l.id == s.retire).unwrap().nom, "Plan 5");
    assert!(j.resume().contains("2 plans modifiés") && j.resume().contains("1 ajouté") && j.resume().contains("1 retiré") && j.resume().contains("1 déplacé"), "{}", j.resume());
    assert_eq!(j.de, format!("v1 au réal, {}", envois::date_courte(1_760_000_000 + 86_400)));
}

#[test]
fn the_page_and_the_pdf_are_written_with_the_percentage_the_durations_and_the_notes() {
    let mut s = scenario("journal-fichiers");
    // Notes : un refus (en discussion avant), un fait sur un plan qui n'est pas dans le journal, une note trop tôt.
    let (p1, p3) = (s.retouche, s.intact);
    let n1 = s.doc.add_comment(p1, 0, "Claire", 900);
    s.doc.comment_mut(n1).unwrap().text = "Trop sombre".into();
    s.doc.set_note_status(n1, Status::Refused, "Hors budget", 1_500).unwrap();
    let n2 = s.doc.add_comment(p3, 0, "Claire", 900);
    s.doc.set_note_status(n2, Status::Done, "", 1_600).unwrap();
    let n3 = s.doc.add_comment(p1, 1, "Léa", 900);
    s.doc.comment_mut(n3).unwrap().text = "Plus vieille".into();
    s.doc.set_note_status(n3, Status::Done, "", 500).unwrap();
    let mut j = journal_entre(&s);
    assert_eq!(j.notes.len(), 1, "seule la note du plan listé, traitée entre les deux");
    let sortie = dossier("journal-sortie");
    let (html, pdf) = (sortie.join("journal.html"), sortie.join("journal.pdf"));
    j.avertissements.clear();
    let d = DemandeJ { journal: j, a: cote(&s, s.a), b: cote(&s, s.b), html: Some(html.clone()), pdf: Some(pdf.clone()) };
    let mut job = JournalJob::start(d, None).unwrap();
    let r = attendre(&mut job).expect("journal");
    assert_eq!((r.modifies, r.ajoutes, r.retires, r.deplaces, r.notes), (2, 1, 1, 1, 1));
    assert_eq!(fichiers_sous(&sortie), ["journal.html", "journal.pdf"], "pas de .part qui traîne");
    // La page : le plan retouché avec sa part de pixels (≈ 7 %), le plan rallongé avec ses durées, les images, la note, pas le plan intact.
    let page = std::fs::read_to_string(&html).unwrap();
    let nom_retouche = nom_du_plan(&s.doc, s.retouche);
    assert!(page.starts_with("<!doctype html>"));
    assert!(page.contains(&format!("<h3>{nom_retouche}</h3>")), "le plan retouché");
    assert!(!page.contains(&format!("<h3>{}</h3>", nom_du_plan(&s.doc, s.intact))), "le plan intact n'y est pas");
    let apres = page.split("class=\"pct\">").nth(1).expect("une part de pixels");
    let pct: f32 = apres.split(" %").next().unwrap().replace(',', ".").parse().unwrap();
    assert!((4.0..9.0).contains(&pct), "part de pixels : {pct}");
    assert!(page.contains("4 i → 6 i (+2 i)"), "les durées avant / après et l'écart");
    assert!(page.contains("image identique"), "le plan rallongé a la même image : la page le dit");
    assert!(page.contains("data:image/jpeg;base64,"), "les images sont dans la page");
    assert_eq!(page.matches("<img ").count(), 2 + 2 + 1 + 1, "avant et après des deux modifiés, après de l'ajouté, avant du retiré");
    assert!(page.contains("Refusé") && page.contains("Raison : Hors budget") && page.contains("Trop sombre"));
    assert!(!page.contains("Plus vieille"));
    assert!(page.contains("déplacé") && page.contains("Plans retirés") && page.contains("Plans ajoutés"));
    // Le PDF : une vraie structure, le texte en clair, au moins une page de résumé et une page d'images.
    let octets = std::fs::read(&pdf).unwrap();
    assert!(octets.starts_with(b"%PDF-1.4") && octets.ends_with(b"%%EOF\n"));
    let texte = String::from_utf8_lossy(&octets);
    assert!(texte.contains("Journal des diff"), "le titre");
    assert!(texte.contains("Hors budget"), "la raison du refus");
    assert!(texte.contains("/Subtype /Image"), "les images");
    assert!(texte.matches("/Type /Page ").count() >= 2);
    assert!(r.octets > 1000);
}

#[test]
fn an_only_html_or_only_pdf_journal_is_possible_and_no_format_is_refused() {
    let s = scenario("journal-formats");
    let sortie = dossier("journal-formats-sortie");
    let faire = |html: Option<PathBuf>, pdf: Option<PathBuf>| DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html, pdf };
    assert!(JournalJob::start(faire(None, None), None).is_err());
    let mut job = JournalJob::start(faire(Some(sortie.join("seule.html")), None), None).unwrap();
    attendre(&mut job).expect("page");
    assert_eq!(fichiers_sous(&sortie), ["seule.html"]);
    let mut job = JournalJob::start(faire(None, Some(sortie.join("seul.pdf"))), None).unwrap();
    attendre(&mut job).expect("pdf");
    assert_eq!(fichiers_sous(&sortie), ["seul.pdf", "seule.html"]);
}

#[test]
fn a_missing_snapshot_still_gives_a_journal_without_that_side_s_images() {
    let s = scenario("journal-sans-instantane");
    // L'instantané du premier envoi disparaît (dossier « versions » déplacé).
    std::fs::remove_dir_all(s.dir.join("versions")).unwrap();
    let sortie = dossier("journal-sans-instantane-sortie");
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(sortie.join("j.html")), pdf: Some(sortie.join("j.pdf")) };
    let mut job = JournalJob::start(d, None).unwrap();
    attendre(&mut job).expect("journal malgré tout");
    let page = std::fs::read_to_string(sortie.join("j.html")).unwrap();
    assert!(page.contains("Pas d&#39;images pour l&#39;envoi") && page.contains("Image indisponible"), "l'absence est dite en clair");
    assert!(page.contains("<h3>"), "la liste des plans est là quand même");
}

// ---------------------------------------------------------------------------------------------- arrêt et échec

#[test]
fn a_stopped_journal_leaves_nothing_and_a_finished_one_leaves_both_files() {
    let s = scenario("journal-arret");
    let sortie = dossier("journal-arret-sortie");
    let (html, pdf) = (sortie.join("j.html"), sortie.join("j.pdf"));
    // L'arrêt demandé avant que le fil commence : rien n'est écrit, pas même un .part.
    let annule = std::sync::atomic::AtomicBool::new(true);
    let fait = std::sync::atomic::AtomicUsize::new(0);
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(html.clone()), pdf: Some(pdf.clone()) };
    let e = crate::journaljob::travailler(d, &annule, &fait).unwrap_err();
    assert!(e.contains("arrêté"), "{e}");
    assert!(fichiers_sous(&sortie).is_empty());
    // Pendant le travail : ou bien le journal est arrêté et il n'y a aucun fichier, ou bien il était déjà fini et les deux y sont.
    for essai in 0..6 {
        let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(html.clone()), pdf: Some(pdf.clone()) };
        let mut job = JournalJob::start(d, None).unwrap();
        if essai % 2 == 1 {
            std::thread::sleep(Duration::from_millis(40));
        }
        job.cancel();
        let r = attendre(&mut job);
        let fichiers = fichiers_sous(&sortie);
        match r {
            Err(e) => {
                assert!(e.contains("arrêté"), "{e}");
                assert!(fichiers.is_empty(), "arrêté : {fichiers:?}");
            }
            Ok(_) => assert_eq!(fichiers, ["j.html", "j.pdf"]),
        }
        let _ = std::fs::remove_file(&html);
        let _ = std::fs::remove_file(&pdf);
    }
    // Lâcher la tâche (fermer SHOT) pendant le calcul : rien ne traîne non plus.
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(html.clone()), pdf: Some(pdf.clone()) };
    drop(JournalJob::start(d, None).unwrap());
    let fichiers = fichiers_sous(&sortie);
    assert!(fichiers.iter().all(|f| !f.ends_with(".part")), "{fichiers:?}");
    assert!(fichiers.is_empty() || fichiers == ["j.html", "j.pdf"], "{fichiers:?}");
}

#[test]
fn a_failure_while_writing_leaves_no_file_behind() {
    let s = scenario("journal-echec");
    let sortie = dossier("journal-echec-sortie");
    // La page peut s'écrire, le PDF non (dossier absent) : la page ne reste pas seule.
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(sortie.join("j.html")), pdf: Some(sortie.join("absent").join("j.pdf")) };
    let mut job = JournalJob::start(d, None).unwrap();
    let e = attendre(&mut job).unwrap_err();
    assert!(e.contains("j.pdf"), "{e}");
    assert!(fichiers_sous(&sortie).is_empty(), "{:?}", fichiers_sous(&sortie));
    // Un journal déjà écrit sous le même nom n'est pas détruit par un échec.
    std::fs::write(sortie.join("j.html"), "ancien").unwrap();
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(sortie.join("j.html")), pdf: Some(sortie.join("absent").join("j.pdf")) };
    let mut job = JournalJob::start(d, None).unwrap();
    attendre(&mut job).unwrap_err();
    assert_eq!(std::fs::read_to_string(sortie.join("j.html")).unwrap(), "ancien");
    assert_eq!(fichiers_sous(&sortie), ["j.html"]);
}

#[test]
fn the_sampled_frames_cover_the_start_the_middle_and_the_end() {
    assert_eq!(crate::journaljob::echantillons(1, 1), [(0, 0)]);
    assert_eq!(crate::journaljob::echantillons(4, 4), [(0, 0), (2, 2), (3, 3)]);
    // Des durées différentes : début, milieu et fin de chacun.
    assert_eq!(crate::journaljob::echantillons(5, 9), [(0, 0), (2, 4), (4, 8)]);
}

// ---------------------------------------------------------------------------------------------- dans l'application

fn banc_projet(nom: &str, n: usize) -> Banc {
    let travail = dossier(nom);
    let m = project::manifeste_shot(&manifeste(&travail));
    let mut b = banc(film(n), false);
    b.app.path = Some(m);
    b.app.modified = true;
    b
}

fn pomper(b: &mut Banc) {
    for _ in crate::perf_tests::attente() {
        b.image(Vec::new(), 0.25);
        if b.app.envois_ui.au_repos() && !b.app.save_en_cours() {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("les envois ne se calment pas");
}

fn marquer_dans_l_appli(b: &mut Banc, nom: &str) {
    let ctx = b.ctx.clone();
    b.app.envois_marquer(&ctx, nom, envois::lire_date("20/10/2026").unwrap());
    pomper(b);
}

#[test]
fn the_window_proposes_the_last_send_against_today_and_exports_without_blocking() {
    let mut b = banc_projet("journal-appli", 3);
    marquer_dans_l_appli(&mut b, "v1 au réal");
    // Rien n'a changé : avec un seul envoi, le journal est vide mais possible.
    b.app.run_action("envoi.journal");
    let w = b.app.journal.as_ref().expect("la fenêtre s'ouvre");
    assert_eq!((w.a, w.b), (Some(1), None));
    let base = b.app.journal_base(1, None).expect("comparaison");
    assert!(base.journal.diff.est_vide() && base.journal.resume().starts_with("Aucune différence"));
    b.app.journal = None;
    // Un plan retouché : le journal le liste, avec « l'état actuel » comme second côté.
    noircir(&mut b.app.doc, 1);
    pomper(&mut b);
    b.app.run_action("envoi.journal");
    assert_eq!(b.app.journal.as_ref().map(|w| (w.a, w.b)), Some((Some(1), None)));
    let base = b.app.journal_base(1, None).unwrap();
    assert_eq!((base.journal.diff.modifies(), base.journal.diff.inchanges), (1, 2));
    assert!(base.journal.vers.starts_with("État actuel"));
    // Choisir deux fois le même envoi : refusé en clair ; deux envois dans le désordre : remis dans l'ordre.
    assert!(b.app.journal_base(1, Some(1)).err().expect("refusé").contains("deux états différents"));
    // L'exportation : la tâche tourne sur un autre fil, la fenêtre se redessine pendant ce temps.
    let sortie = dossier("journal-appli-sortie");
    let ctx = b.ctx.clone();
    b.app.journal_exporter_vers(&ctx, &sortie.join("Film_journal"));
    assert!(b.app.journal.as_ref().unwrap().busy() || b.app.journal.as_ref().unwrap().resultat.is_some());
    let t = Instant::now();
    while b.app.journal.as_ref().unwrap().resultat.is_none() {
        b.image(Vec::new(), 0.1);
        std::thread::sleep(Duration::from_millis(5));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    let r = b.app.journal.as_ref().unwrap().resultat.as_ref().unwrap().as_ref().expect("écrit");
    assert_eq!((r.modifies, r.ajoutes, r.retires), (1, 0, 0));
    assert_eq!(fichiers_sous(&sortie), ["Film_journal.html", "Film_journal.pdf"]);
    let page = std::fs::read_to_string(sortie.join("Film_journal.html")).unwrap();
    assert!(page.contains("État actuel") && page.contains("<img "));
    let pct: f32 = page.split("class=\"pct\">").nth(1).unwrap().split(" %").next().unwrap().replace(',', ".").parse().unwrap();
    assert!((4.0..9.0).contains(&pct), "{pct}");
    assert!(b.app.status.contains("Journal des différences écrit"), "{}", b.app.status);
    assert!(b.app.exports_en_cours().is_empty());
    // Depuis la fenêtre « Envois » : un envoi précis comme premier côté.
    b.app.journal = None;
    b.app.journal_ouvrir(Some(1));
    assert_eq!(b.app.journal.as_ref().map(|w| (w.a, w.b)), Some((Some(1), None)));
}

#[test]
fn the_default_sides_are_the_last_send_against_today_or_the_two_last_sends() {
    let mut doc = film(2);
    assert_eq!(crate::journalui::defauts(&doc.envois, false), (None, None));
    marquer(&mut doc, "v1", 10);
    assert_eq!(crate::journalui::defauts(&doc.envois, false), (Some(1), None));
    marquer(&mut doc, "v2", 20);
    marquer(&mut doc, "v3", 30);
    // Des plans ont changé depuis le dernier envoi : le dernier envoi contre aujourd'hui.
    assert_eq!(crate::journalui::defauts(&doc.envois, true), (Some(3), None));
    // Rien de neuf : l'avant-dernier contre le dernier.
    assert_eq!(crate::journalui::defauts(&doc.envois, false), (Some(2), Some(3)));
}

#[test]
fn a_journal_without_a_send_says_why_and_opens_nothing() {
    let mut b = banc(film(2), false);
    b.app.run_action("envoi.journal");
    assert!(b.app.journal.is_none());
    assert!(b.app.status.contains("Aucun envoi"), "{}", b.app.status);
}

// ---------------------------------------------------------------------------------------------- les entrées et le paquet

#[test]
fn the_command_is_in_the_palette_the_menus_the_hub_and_the_shortcut_file() {
    assert!(crate::keymap::action_info("envoi.journal").is_some_and(|a| a.implemented));
    assert!(crate::barres::Barre::Montage.cherche("envoi.journal").is_some());
    assert!(crate::prefsui::FAMILLES.iter().any(|(_, exports)| exports.iter().any(|e| e.3 == "envoi.journal")));
    assert!(include_str!("../keymap.toml").contains("action = \"envoi.journal\""));
}

#[test]
fn the_delivery_package_gets_the_journal_only_when_asked() {
    assert!(!PaquetReglages::default().journal, "décoché d'origine");
    // Le journal est un contenu à lui seul.
    let seul = PaquetReglages { videos: false, vignettes: false, pdf: false, animatique: false, liste_csv: false, liste_json: false, journal: true, ..PaquetReglages::default() };
    assert!(!seul.vide());
    let mut b = banc_projet("journal-paquet", 3);
    marquer_dans_l_appli(&mut b, "v1 au réal");
    noircir(&mut b.app.doc, 0);
    pomper(&mut b);
    let (a, c) = b.app.journal_defauts();
    let demande = b.app.journal_demande(a.unwrap(), c, None, None).expect("demande");
    let dest = dossier("journal-paquet-sortie");
    let reglages = PaquetReglages { modele_chemin: ".".into(), ..seul };
    let base = crate::paquet::base_du_film(&b.app.doc);
    // Sans la préparation de l'appelant : l'étape le dit et ne laisse rien.
    let mut sans = PaquetJob::start(&b.app.doc, &reglages, 0, None, &dest, Vec::new()).unwrap();
    let t = Instant::now();
    while sans.running() {
        sans.step(&b.app.doc, Duration::from_millis(8));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert!(sans.outcome.take().unwrap().unwrap_err().contains("journal"));
    assert_eq!(fichiers_sous(&dest).len(), 0);
    // Avec : le PDF et la page sont rangés, et le manifeste les cite.
    let mut job = PaquetJob::start(&b.app.doc, &reglages, 0, None, &dest, Vec::new()).unwrap().avec_journal(demande);
    let t = Instant::now();
    while job.running() {
        job.step(&b.app.doc, Duration::from_millis(8));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    job.outcome.take().unwrap().expect("paquet");
    assert!(dest.join(format!("{base}_journal.pdf")).is_file() && dest.join(format!("{base}_journal.html")).is_file(), "{:?}", fichiers_sous(&dest));
    let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dest.join("manifest.json")).unwrap()).unwrap();
    let sortes: Vec<&str> = m["fichiers"].as_array().unwrap().iter().map(|f| f["sorte"].as_str().unwrap()).collect();
    assert_eq!(sortes, ["journal", "journal"]);
    assert!(std::fs::read_dir(&dest).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with(".shot-paquet")));
    // Arrêter le paquet pendant le journal : rien ne reste.
    let dest2 = dossier("journal-paquet-arret");
    let demande = b.app.journal_demande(a.unwrap(), c, None, None).unwrap();
    let mut job = PaquetJob::start(&b.app.doc, &reglages, 0, None, &dest2, Vec::new()).unwrap().avec_journal(demande);
    for _ in 0..3 {
        job.step(&b.app.doc, Duration::from_millis(8));
    }
    job.cancel();
    assert_eq!(fichiers_sous(&dest2).len(), 0);
}

/// À la demande (`cargo test -- --ignored journal_echantillon`, avec `JOURNAL_ECHANTILLON` = un dossier) : écrit un journal d'exemple pour le regarder.
#[test]
#[ignore]
fn journal_echantillon() {
    let Some(sortie) = std::env::var_os("JOURNAL_ECHANTILLON").map(PathBuf::from) else { return };
    let mut s = scenario("journal-echantillon");
    let n1 = s.doc.add_comment(s.retouche, 0, "Claire", 900);
    s.doc.comment_mut(n1).unwrap().text = "Le ciel est trop sombre, on n'y voit plus rien.".into();
    s.doc.set_note_status(n1, Status::Refused, "Hors budget : le plan est déjà validé par la production.", 1_500).unwrap();
    let n2 = s.doc.add_comment(s.rallonge, 2, "Léa", 900);
    s.doc.comment_mut(n2).unwrap().text = "Laisser respirer l'entrée du personnage.".into();
    s.doc.set_note_status(n2, Status::Done, "", 1_600).unwrap();
    let n3 = s.doc.add_comment(s.ajoute, 0, "Léa", 900);
    s.doc.comment_mut(n3).unwrap().text = "Un plan de transition ?".into();
    s.doc.set_note_status(n3, Status::Discussion, "", 1_700).unwrap();
    std::fs::create_dir_all(&sortie).unwrap();
    let d = DemandeJ { journal: journal_entre(&s), a: cote(&s, s.a), b: cote(&s, s.b), html: Some(sortie.join("journal.html")), pdf: Some(sortie.join("journal.pdf")) };
    let mut job = JournalJob::start(d, None).unwrap();
    attendre(&mut job).expect("journal");
}
