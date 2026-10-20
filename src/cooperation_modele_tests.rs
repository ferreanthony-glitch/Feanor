//! Travailler à plusieurs : le modèle pur (les prises, les noms de fichiers, le balayage du dépôt, l'équipier) et le fil de fond
//! pour de vrai (horloge réelle, aucun tour à la main).

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::cooperation::{Delais, Horloge, Session};
use crate::cooperation_tests::{confirmer, deux_postes, film, partager, Poste};
use crate::depot::{self, CopieEnConflit, Curseur, Depot, DossierPartage, Nom, OrigineCopie};
use crate::equipe::{Cle, Identite};
use crate::fiabilite_fixture as fx;
use crate::project::{self, Loaded};
use crate::verrous::{self, Cible, Equipier, Fiche, Observations, Prise, Remplace, Rendue};

// ------------------------------------------------------------------------------------------ les prises

fn fiche(id: &str, seance: &str, renouvele: u64, prises: Vec<Prise>, rendues: Vec<Rendue>) -> Fiche {
    Fiche {
        format: verrous::FORMAT_FICHE.into(),
        version: verrous::VERSION_FICHE,
        equipier: Equipier { id: id.into(), nom: id.to_uppercase(), couleur: [1, 2, 3], machine: "m".into(), prefixe: 1 },
        seance: seance.into(),
        renouvele_le: renouvele,
        sequence: 1,
        prises,
        rendues,
        presence: None,
    }
}

fn prise(plan: u64, pris_le: u64) -> Prise {
    Prise { cible: Cible::Plan(plan), pris_le, revision: 0, remplace: Vec::new() }
}

const BAIL: u64 = 600_000;

#[test]
fn the_oldest_live_prise_wins_and_ties_go_to_the_smallest_equipier_then_seance() {
    let obs = Observations::default();
    let fiches = vec![fiche("lea", "a", 1_000, vec![prise(5, 900)], vec![]), fiche("anthony", "b", 1_000, vec![prise(5, 800)], vec![])];
    let e = verrous::evaluer(&fiches, Cible::Plan(5), 1_000, BAIL, &obs);
    assert_eq!(e.titulaire.as_ref().map(|t| t.equipier.id.as_str()), Some("anthony"));
    assert_eq!(e.perdantes.len(), 1);
    // À égalité d'instant : le plus petit identifiant d'équipier, puis de séance.
    let egal = vec![fiche("lea", "a", 1_000, vec![prise(5, 800)], vec![]), fiche("anthony", "z", 1_000, vec![prise(5, 800)], vec![]), fiche("anthony", "b", 1_000, vec![prise(5, 800)], vec![])];
    let e = verrous::evaluer(&egal, Cible::Plan(5), 1_000, BAIL, &obs);
    let t = e.titulaire.expect("titulaire");
    assert_eq!((t.cle.equipier.as_str(), t.cle.seance.as_str()), ("anthony", "b"));
    // Le même jugement dans n'importe quel ordre de lecture.
    let mut renverse = egal.clone();
    renverse.reverse();
    assert_eq!(verrous::evaluer(&renverse, Cible::Plan(5), 1_000, BAIL, &obs).titulaire.map(|t| t.cle), Some(t.cle));
}

#[test]
fn a_released_prise_does_not_count_but_a_later_one_does() {
    let obs = Observations::default();
    let rendue = Rendue { cible: Cible::Plan(5), le: 900, revision: 4 };
    let f = fiche("anthony", "a", 1_000, vec![prise(5, 800)], vec![rendue.clone()]);
    let e = verrous::evaluer(std::slice::from_ref(&f), Cible::Plan(5), 1_000, BAIL, &obs);
    assert!(e.titulaire.is_none() && e.perimees.is_empty());
    assert_eq!(e.revision_annoncee, 4, "la révision rendue reste annoncée : c'est le fichier à attendre");
    // Reprise après le rendu : la prise recompte.
    let f = fiche("anthony", "a", 1_000, vec![prise(5, 950)], vec![rendue]);
    assert!(verrous::evaluer(&[f], Cible::Plan(5), 1_000, BAIL, &obs).titulaire.is_some());
}

#[test]
fn a_taken_over_prise_stays_out_even_if_its_owner_wakes_up() {
    let obs = Observations::default();
    let ancienne = prise(5, 100);
    let reprise = Prise { remplace: vec![Remplace { equipier: "anthony".into(), seance: "a".into(), pris_le: 100 }], ..prise(5, 700_000) };
    // Anthony se réveille bien plus tard et renouvelle son bail : sa vieille prise ne revient pas.
    let fiches = vec![fiche("anthony", "a", 800_000, vec![ancienne], vec![]), fiche("lea", "b", 800_000, vec![reprise], vec![])];
    let e = verrous::evaluer(&fiches, Cible::Plan(5), 800_000, BAIL, &obs);
    assert_eq!(e.titulaire.as_ref().map(|t| t.equipier.id.as_str()), Some("lea"));
    assert!(e.perdantes.is_empty() && e.perimees.is_empty(), "elle est écartée, pas seulement battue");
}

#[test]
fn the_lease_uses_the_authors_clock_or_what_we_saw_change() {
    let mut obs = Observations::default();
    let f = fiche("anthony", "a", 10_000, vec![prise(5, 5_000)], vec![]);
    let cle = f.cle();
    // Frais pour son horloge.
    assert!(verrous::evaluer(std::slice::from_ref(&f), Cible::Plan(5), 100_000, BAIL, &obs).titulaire.is_some());
    // Vieux pour son horloge, vu pour la première fois : périmé.
    assert!(verrous::evaluer(std::slice::from_ref(&f), Cible::Plan(5), 5_000_000, BAIL, &obs).titulaire.is_none());
    obs.voir(&cle, 1, 5_000_000);
    assert!(verrous::evaluer(std::slice::from_ref(&f), Cible::Plan(5), 5_000_100, BAIL, &obs).titulaire.is_none(), "le premier regard ne compte pas");
    // Vu changer il y a peu : vivant, même si son horloge dit autre chose.
    obs.voir(&cle, 2, 5_000_200);
    assert!(verrous::evaluer(std::slice::from_ref(&f), Cible::Plan(5), 5_100_000, BAIL, &obs).titulaire.is_some());
    // Plus de changement depuis plus que le bail : périmé de nouveau.
    assert!(verrous::evaluer(&[f], Cible::Plan(5), 5_000_200 + BAIL + 1, BAIL, &obs).titulaire.is_none());
}

#[test]
fn a_fiche_that_is_damaged_or_newer_is_refused_and_a_target_is_taken_once_per_fiche() {
    assert!(Fiche::lire("pas du json").is_err());
    let f = fiche("a", "s", 1, vec![prise(1, 10), prise(1, 20), prise(2, 5)], vec![]);
    let texte = serde_json::to_string(&f).expect("écriture");
    let lue = Fiche::lire(&texte).expect("lecture");
    assert_eq!(lue.prises.len(), 2, "une prise par cible : la plus récente");
    assert_eq!(lue.prises.iter().find(|p| p.cible == Cible::Plan(1)).map(|p| p.pris_le), Some(20));
    let futur = texte.replace("\"version\":1", "\"version\":9");
    assert!(Fiche::lire(&futur).expect_err("plus récente").contains("plus récente"));
    let autre = texte.replace("shot-equipier", "autre");
    assert!(Fiche::lire(&autre).is_err());
    // Les cibles s'écrivent « plan:5 » et « montage ».
    for c in [Cible::Plan(5), Cible::Montage] {
        assert_eq!(Cible::depuis_texte(&c.texte()), Some(c));
    }
    assert_eq!(Cible::depuis_texte("plan:x"), None);
}

#[test]
fn three_people_can_work_at_once_each_in_their_own_plans() {
    let racine = fx::dossier("coop-trois");
    let manifeste = partager(&film(6), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let mut postes: Vec<Poste> = ["Anthony", "Lea", "Maya"].iter().map(|n| Poste::ouvrir(&manifeste, n, &h)).collect();
    for (i, p) in postes.iter_mut().enumerate() {
        for k in 0..2 {
            p.prendre_plan(i * 2 + k).expect("libre");
        }
    }
    confirmer(&h);
    for p in postes.iter_mut() {
        p.tour();
    }
    for p in postes.iter_mut() {
        p.tour();
    }
    // Chacun voit les deux autres.
    for (i, p) in postes.iter().enumerate() {
        assert_eq!(p.s.vue().equipe.len(), 3, "poste {i}");
        for k in 0..6 {
            let moi = k / 2 == i;
            assert_eq!(p.s.peut_modifier_plan(p.plan_id(k)).is_ok(), moi, "poste {i}, plan {k}");
        }
    }
    // Ils dessinent et enregistrent chacun le sien ; chacun reçoit le travail des autres.
    for (i, p) in postes.iter_mut().enumerate() {
        p.dessiner(i * 2, 40 + i as u8);
        p.enregistrer();
    }
    for _ in 0..3 {
        for p in postes.iter_mut() {
            p.tour();
        }
    }
    for p in &postes {
        for i in 0..3 {
            assert_eq!(p.rouge(i * 2), 40 + i as u8);
        }
    }
    // Un seul peut avoir le jeton.
    assert!(postes[0].s.prendre(&[Cible::Montage], false).remove(0).1.is_ok());
    for p in postes.iter_mut() {
        p.tour();
    }
    for p in postes.iter_mut().skip(1) {
        assert!(p.s.peut_modifier_montage().is_err());
    }
}

// ------------------------------------------------------------------------------------------ les noms de fichiers

#[test]
fn file_names_made_by_sync_clients_are_told_from_the_ones_shot_writes() {
    assert_eq!(depot::nom_de_plan("12.json"), Nom::Canonique("12".into()));
    for copie in [
        "12 (Copie en conflit de Léa 2026-10-20).json",
        "12 (Lea's conflicted copy 2026-10-20).json",
        "12 (1).json",
        "12-DESKTOP-ABC123.json",
        "12_old.json",
        "12.copy.json",
    ] {
        assert_eq!(depot::nom_de_plan(copie), Nom::Copie("12".into()), "{copie}");
    }
    for autre in ["plan.json", "notes.txt", "12.png", ".json"] {
        assert_eq!(depot::nom_de_plan(autre), Nom::Autre, "{autre}");
    }
    assert_eq!(depot::nom_d_equipier("abc123.s1.json"), Nom::Canonique("abc123.s1".into()));
    assert!(matches!(depot::nom_d_equipier("abc123.s1 (Copie en conflit de x).json"), Nom::Copie(_)));
    assert!(depot::est_copie_de_structure("Film (Copie en conflit de Léa 2026-10-20).pelure", "Film.pelure"));
    assert!(depot::est_copie_de_structure("Film-POSTE2.pelure", "Film.pelure"));
    assert!(!depot::est_copie_de_structure("Film.pelure", "Film.pelure"));
    assert!(!depot::est_copie_de_structure("Filmographie.pelure", "Film.pelure"));
    assert!(!depot::est_copie_de_structure("Film (1).json", "Film.pelure"));
}

#[test]
fn the_scan_reports_only_what_changed_ignores_temporary_files_and_sees_each_copy_once() {
    let racine = fx::dossier("coop-balayage");
    let manifeste = partager(&film(5), &racine);
    let dossier = DossierPartage::nouveau(&manifeste);
    let base = fx::racine(&manifeste).to_path_buf();
    let mut c = Curseur::default();
    let r = dossier.surveiller(&mut c).expect("balayage");
    assert!(r.premier && r.structure && r.plans.len() == 5, "{r:?}");
    let r = dossier.surveiller(&mut c).expect("balayage");
    assert!(!r.premier && !r.structure && r.plans.is_empty() && !r.fiches && r.copies.is_empty(), "rien n'a changé : {r:?}");
    // Un plan réécrit ; un fichier temporaire à côté (Dropbox en train d'écrire) ; une fiche neuve.
    let doc = project::load(&manifeste).expect("projet").doc;
    let plan = doc.panels[2].id;
    let chemin = base.join("plans").join(format!("{plan}.json"));
    let mut v = fx::lire_json(&chemin);
    v["revision"] = serde_json::json!(7);
    fx::ecrire_json(&chemin, &v);
    std::fs::write(base.join("plans").join(format!("{plan}.json.tmp")), b"x").expect("tmp");
    std::fs::write(base.join("plans").join("9.part"), b"x").expect("part");
    let r = dossier.surveiller(&mut c).expect("balayage");
    assert_eq!(r.plans, vec![plan], "{r:?}");
    assert!(r.copies.is_empty());
    // Une copie en conflit : signalée une fois.
    let nom = format!("{plan} (Copie en conflit de Lea 2026-10-20).json");
    std::fs::copy(&chemin, base.join("plans").join(&nom)).expect("copie");
    std::fs::write(base.join("Film (Copie en conflit de Lea 2026-10-20).pelure"), b"{}").expect("structure en double");
    let r = dossier.surveiller(&mut c).expect("balayage");
    assert_eq!(r.copies.len(), 2, "{r:?}");
    assert!(r.copies.contains(&CopieEnConflit { chemin: std::path::Path::new("plans").join(&nom), origine: OrigineCopie::Plan(plan) }));
    assert!(r.copies.iter().any(|c| c.origine == OrigineCopie::Structure));
    assert!(dossier.surveiller(&mut c).expect("balayage").copies.is_empty(), "une seule fois");
    // Les noms que SHOT refuse d'écrire ou de lire hors du projet.
    assert!(dossier.ecrire_brouillon("../evasion.json", b"x").is_err());
    assert!(dossier.ecrire_brouillon("a/b.json", b"x").is_err());
    assert!(dossier.lire_copie(&CopieEnConflit { chemin: "..\\x".into(), origine: OrigineCopie::Fiche }).is_err());
}

#[test]
fn a_file_removed_from_the_folder_is_noticed() {
    let (a, _, _, racine) = deux_postes("balayage-retrait", 3);
    let dossier = DossierPartage::nouveau(&a.manifeste);
    let mut c = Curseur::default();
    dossier.surveiller(&mut c).expect("balayage");
    let fiches = racine.join("Film").join("partage").join("equipe");
    let un = std::fs::read_dir(&fiches).expect("dossier").flatten().next().expect("une fiche").path();
    std::fs::remove_file(un).expect("retrait");
    assert!(dossier.surveiller(&mut c).expect("balayage").fiches);
}

// ------------------------------------------------------------------------------------------ l'équipier

#[test]
fn initials_and_colours_are_stable_and_the_default_name_comes_from_windows() {
    assert_eq!(crate::equipe::initiales("Léa Martin"), "LM");
    assert_eq!(crate::equipe::initiales("Anthony"), "AN");
    assert_eq!(crate::equipe::initiales("  "), "?");
    assert_eq!(crate::equipe::initiales("jean de la Fontaine"), "JF");
    assert_eq!(crate::equipe::couleur_pour("abc"), crate::equipe::couleur_pour("abc"));
    let r = crate::equipe::ReglageEquipe { id: "x1".into(), nom: String::new(), ..Default::default() };
    let moi = crate::equipe::identite(&r, "Anthony F");
    assert_eq!(moi.nom, "Anthony F", "à défaut, le nom des commentaires");
    assert_eq!(moi.id, "x1");
    let sans = crate::equipe::identite(&crate::equipe::ReglageEquipe::default(), "");
    assert!(!sans.nom.is_empty() && sans.id.len() == 16);
    let a = crate::equipe::identite(&r, "");
    let b = crate::equipe::identite(&r, "");
    assert_ne!(a.seance, b.seance, "chaque fenêtre est une séance à elle");
    assert_eq!(Cle { equipier: "x1".into(), seance: a.seance.clone() }, a.cle());
}

// ------------------------------------------------------------------------------------------ le fil de fond, pour de vrai

/// Des durées courtes pour que le fil de fond joue son rôle en une seconde ou deux.
fn vite() -> Delais {
    Delais { bail: Duration::from_secs(5), balayage: Duration::from_millis(30), battement: Duration::from_millis(200), confirmation: Duration::from_millis(100), presence: Duration::from_millis(50), patience: Duration::from_secs(3) }
}

fn ouvrir_avec_fil(manifeste: &std::path::Path, nom: &str) -> (crate::document::Document, Session) {
    let Loaded { mut doc, partage, .. } = project::load(manifeste).expect("projet partagé");
    let moi = Identite { id: nom.to_lowercase(), nom: nom.into(), couleur: [9, 9, 9], machine: "m".into(), seance: "s1".into() };
    let depot = Arc::new(DossierPartage::nouveau(manifeste));
    let s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut doc, vite(), Horloge::reelle(), true, None).expect("session");
    (doc, s)
}

fn attendre(quoi: &str, mut ok: impl FnMut() -> bool) {
    let debut = Instant::now();
    while debut.elapsed() < Duration::from_secs(10) {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("trop long : {quoi}");
}

#[test]
fn the_background_thread_alone_keeps_the_team_in_touch() {
    let racine = fx::dossier("coop-fil");
    let manifeste = partager(&film(3), &racine);
    let (mut doc_a, mut a) = ouvrir_avec_fil(&manifeste, "Anthony");
    let (doc_b, mut b) = ouvrir_avec_fil(&manifeste, "Lea");
    let plan = doc_a.panels[0].id;
    // Anthony prend un plan : Léa le voit sans que personne ne pompe.
    assert!(a.prendre(&[Cible::Plan(plan)], false)[0].1.is_ok());
    attendre("Léa voit la prise d'Anthony", || matches!(b.vue().etat(Cible::Plan(plan)), verrous::EtatPrise::Autre(_)));
    assert!(b.peut_modifier_plan(plan).is_err());
    // Un dessin enregistré par Anthony arrive chez Léa tout seul.
    std::thread::sleep(Duration::from_millis(250));
    let id = doc_a.panels[0].layers[0].frames[0].expect("dessin");
    {
        let c = doc_a.cels.get_mut(&id).expect("dessin");
        c.raster.tile_mut(0)[0] = eframe::egui::Color32::from_rgba_premultiplied(55, 1, 2, 255);
        c.version += 1;
    }
    let ctx = a.contexte_ecriture(&doc_a, false);
    let mut ecrits = std::collections::HashMap::new();
    let job = project::partage_io::preparer(&doc_a, &manifeste, &mut ecrits, ctx).expect("préparation");
    let r = job.run_with(None, 2).expect("écriture").partage.expect("rapport");
    assert_eq!(r.plans_ecrits.len(), 1);
    a.enregistrement_fini(&r, &doc_a);
    let mut doc_b = doc_b;
    attendre("le plan d'Anthony arrive chez Léa", || {
        b.sonder(&mut doc_b);
        doc_b.cels[&id].raster.get(0, 0).r() == 55
    });
    // Anthony ferme : ses prises sont rendues, Léa peut prendre.
    a.fermer();
    attendre("Léa voit le plan libre", || matches!(b.vue().etat(Cible::Plan(plan)), verrous::EtatPrise::Libre));
    assert!(b.prendre(&[Cible::Plan(plan)], false)[0].1.is_ok());
    b.fermer();
    doc_a.panels.clear();
}

#[test]
fn the_thread_never_asks_for_a_repaint_when_nothing_changes() {
    let racine = fx::dossier("coop-fil-repos");
    let manifeste = partager(&film(3), &racine);
    let Loaded { mut doc, partage, .. } = project::load(&manifeste).expect("projet");
    let ctx = eframe::egui::Context::default();
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [1, 1, 1], machine: "m".into(), seance: "s1".into() };
    let depot = Arc::new(DossierPartage::nouveau(&manifeste));
    let mut s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut doc, vite(), Horloge::reelle(), true, Some(ctx.clone())).expect("session");
    // Le premier tour a publié la vue (une demande de repaint). Ensuite, plus rien tant que rien ne change.
    std::thread::sleep(Duration::from_millis(400));
    let avant = ctx.cumulative_pass_nr();
    let mut demandes = 0;
    let debut = Instant::now();
    while debut.elapsed() < Duration::from_millis(800) {
        let sortie = ctx.run(Default::default(), |_| {});
        if sortie.viewport_output.values().any(|v| v.repaint_delay < Duration::from_millis(100)) {
            demandes += 1;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    assert!(demandes <= 1, "le fil réveille l'écran {demandes} fois sans raison");
    let _ = avant;
    s.fermer();
}
