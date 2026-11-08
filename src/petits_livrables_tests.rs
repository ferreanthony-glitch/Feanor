//! Essais des « petits livrables » (`docs/INTERFACE.md` partie 91) : budget de durée, rapport de numérotation, PDF de studio (en-tête, plans modifiés, notes de
//! révision), planche des vignettes avec titre et légendes, fenêtres de fichiers dans le projet, nom de bobine de l'EDL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::budget::{self, Budget};
use crate::document::Document;
use crate::editorial::{self, Options};
use crate::editorial_tests::film;
use crate::entetepdf::{self, Entete, Studio};
use crate::envois::{Envoi, Etat, PlanEnvoye};
use crate::exports_v1_tests::dossier_neuf;
use crate::fiche::{self, Colonne, Entree};
use crate::fildecision::Status;
use crate::pdf::{Board, BoardImage};
use crate::pdffont::Fonts;
use crate::pdfmise::{self, PdfLayout};
use crate::renumerotation;

fn sequence(doc: &Document, rang: usize) -> u64 {
    doc.sequence_order()[rang]
}

// ------------------------------------------------------------------------------------------------ budget de durée

#[test]
fn a_target_duration_is_read_and_written_in_minutes_and_seconds() {
    assert_eq!(budget::lire_duree("90"), Some(90_000));
    assert_eq!(budget::lire_duree("1:30"), Some(90_000));
    assert_eq!(budget::lire_duree(" 11:30 "), Some(690_000));
    assert_eq!(budget::lire_duree("1:02:30"), Some(3_750_000));
    assert_eq!(budget::lire_duree("12,5"), Some(12_500));
    assert_eq!(budget::lire_duree("1:30.5"), Some(90_500));
    assert_eq!(budget::lire_duree("45 s"), Some(45_000));
    assert_eq!(budget::lire_duree(""), Some(0), "vide : pas de cible");
    assert_eq!(budget::lire_duree("aucune"), Some(0));
    for mauvais in ["abc", "1:2:3:4", "1.5:30", "-5", "1::2", "99999999"] {
        assert_eq!(budget::lire_duree(mauvais), None, "« {mauvais} »");
    }
    for ms in [0, 1_000, 90_000, 690_000, 3_750_000, 12_500, 59_999 + 1] {
        assert_eq!(budget::lire_duree(&budget::texte_duree(ms)), Some(ms), "{ms} ms : {}", budget::texte_duree(ms));
    }
    assert_eq!(budget::texte_duree(90_000), "1:30");
    assert_eq!(budget::texte_duree(3_750_000), "1:02:30");
    assert_eq!(budget::texte_duree(0), "aucune");
}

#[test]
fn the_gap_is_signed_in_frames_and_over_budget_is_a_positive_gap() {
    let b = Budget { duree: 72, cible: 48 };
    assert_eq!(b.ecart(), 24);
    assert!(b.depasse());
    assert_eq!(b.texte(24), "0:03 / 0:02 (+1s 00)");
    let b = Budget { duree: 40, cible: 48 };
    assert_eq!(b.ecart(), -8);
    assert!(!b.depasse());
    assert_eq!(b.texte(24), "0:01+16 / 0:02 (-8 i)");
    assert!(!Budget { duree: 48, cible: 48 }.depasse(), "pile à la cible : pas dépassé");
    assert_eq!(budget::texte_ecart(0, 24), "0");
}

#[test]
fn the_budget_of_the_film_and_of_each_sequence_is_live() {
    // film(24) : la séquence 1 a trois plans (24 + 12 + 36 = 72 images), la séquence 2 un plan de 24 images.
    let mut doc = film(24);
    assert!(doc.budget_du_film().is_none() && budget::budgets_des_sequences(&doc).is_empty(), "aucune cible : rien à montrer");
    doc.properties.budget_ms = 5_000;
    let b = doc.budget_du_film().expect("cible du film");
    assert_eq!((b.duree, b.cible, b.ecart()), (96, 120, -24));
    let (s1, s2) = (sequence(&doc, 0), sequence(&doc, 1));
    doc.poser_cible_de_sequence(s1, 2_000);
    let b1 = doc.budget_de_sequence(s1).expect("cible de la séquence 1");
    assert_eq!((b1.duree, b1.cible, b1.depasse()), (72, 48, true));
    assert!(doc.budget_de_sequence(s2).is_none());
    let tous = budget::budgets_des_sequences(&doc);
    assert_eq!(tous.len(), 1);
    assert_eq!(tous[&s1], b1);
    // Allonger un plan de la séquence 1 : l'écart suit tout de suite.
    doc.panels[0].set_length(30);
    assert_eq!(doc.budget_de_sequence(s1).unwrap().ecart(), 30);
    // Retirer la cible (0) l'efface ; une séquence qui n'existe plus n'a plus de cible.
    doc.poser_cible_de_sequence(s1, 0);
    assert!(doc.properties.budgets_sequences.is_empty());
    doc.properties.budgets_sequences.insert(9999, 3_000);
    assert!(doc.cible_de_sequence(9999).is_none());
}

#[test]
fn the_cumulative_gap_is_written_on_the_last_plan_of_each_sequence() {
    let mut doc = film(24);
    assert!(budget::ecarts_cumules(&doc).iter().all(Option::is_none), "sans budget, rien");
    // Cible du film seule : l'écart est sur le dernier plan du film.
    doc.properties.budget_ms = 5_000;
    assert_eq!(budget::ecarts_cumules(&doc), [None, None, None, Some(-24)]);
    // Cibles de séquences : la séquence 1 dure 72 images pour 48 permises ; la séquence 2 n'a pas de cible (elle compte pour ce qu'elle dure).
    let s1 = sequence(&doc, 0);
    doc.poser_cible_de_sequence(s1, 2_000);
    assert_eq!(budget::ecarts_cumules(&doc), [None, None, Some(24), Some(24)]);
    // Les deux séquences ont une cible : 48 + 48 permis pour 96 images, écart nul à la fin.
    let s2 = sequence(&doc, 1);
    doc.poser_cible_de_sequence(s2, 2_000);
    assert_eq!(budget::ecarts_cumules(&doc), [None, None, Some(24), Some(-24 + 24 + 24 - 24 + 0)]);
    assert_eq!(budget::ecarts_cumules(&doc)[3], Some(0));
    assert_eq!(budget::ecart_en_secondes(36, 24), 1.5);
}

#[test]
fn the_cumulative_gap_column_is_off_by_default_and_does_not_change_the_old_sheet() {
    assert_eq!(Colonne::TOUTES.len(), 31);
    assert!(!Colonne::EcartCumule.par_defaut());
    assert!(!fiche::colonnes_par_defaut().contains(&Colonne::EcartCumule));
    assert_eq!(Colonne::depuis_id("ecart_cumule"), Some(Colonne::EcartCumule));
    let mut doc = film(24);
    let s1 = sequence(&doc, 0);
    doc.poser_cible_de_sequence(s1, 2_000);
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    let defaut = fiche::colonnes_par_defaut();
    let f = fiche::build(&doc, &Entree { plans: &plans, colonnes: &defaut, par_image: false, start_tc: 0, etats: None, envoi: None }).unwrap();
    assert!(!f.titres().contains(&"Écart cumulé (s)"), "la fiche d'avant est la même, même avec un budget");
    // La colonne demandée : seulement le dernier plan de chaque séquence.
    let colonnes = [Colonne::Numero, Colonne::DureeSecondes, Colonne::EcartCumule];
    let f = fiche::build(&doc, &Entree { plans: &plans, colonnes: &colonnes, par_image: false, start_tc: 0, etats: None, envoi: None }).unwrap();
    let ecarts: Vec<fiche::Cellule> = f.lignes.iter().map(|l| l.cellules[2].clone()).collect();
    assert_eq!(ecarts, [fiche::Cellule::Vide, fiche::Cellule::Vide, fiche::Cellule::Decimal(1.0), fiche::Cellule::Decimal(1.0)]);
    assert_eq!(f.titres(), ["N° de plan", "Durée (s)", "Écart cumulé (s)"]);
    // Sans budget, la colonne est vide partout.
    let doc = film(24);
    let f = fiche::build(&doc, &Entree { plans: &plans, colonnes: &colonnes, par_image: false, start_tc: 0, etats: None, envoi: None }).unwrap();
    assert!(f.lignes.iter().all(|l| l.cellules[2] == fiche::Cellule::Vide));
}

#[test]
fn the_budget_is_saved_only_when_set_and_old_projects_open_the_same() {
    let dir = dossier_neuf("budget-fichier");
    let manifeste = dir.join("Film").join("Film.shot");
    let mut written = HashMap::new();
    let mut doc = film(24);
    crate::project::save(&doc, &manifeste, &mut written).unwrap();
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(!texte.contains("budget"), "sans cible, rien n'est écrit : {texte}");
    assert_eq!(crate::project::VERSION, 52);
    // Un projet de version 45 s'ouvre sans budget.
    let ancien = texte.replace(&format!("\"version\": {}", crate::project::VERSION), "\"version\": 45");
    assert_ne!(ancien, texte);
    std::fs::write(&manifeste, ancien).unwrap();
    let l = crate::project::load(&manifeste).unwrap();
    assert_eq!(l.doc.properties.budget_ms, 0);
    assert!(l.doc.properties.budgets_sequences.is_empty());
    // Avec des cibles : elles reviennent, et une séquence retirée n'y figure plus.
    doc.properties.budget_ms = 690_000;
    let s1 = sequence(&doc, 0);
    doc.poser_cible_de_sequence(s1, 90_500);
    crate::project::save(&doc, &manifeste, &mut written).unwrap();
    let texte = std::fs::read_to_string(&manifeste).unwrap();
    assert!(texte.contains("budget_ms") && texte.contains("budgets_sequences"));
    let l = crate::project::load(&manifeste).unwrap();
    assert_eq!(l.doc.properties.budget_ms, 690_000);
    assert_eq!(l.doc.properties.budgets_sequences.get(&s1), Some(&90_500));
    assert_eq!(l.doc.budget_de_sequence(s1).map(|b| b.cible), Some(budget::ms_en_images(90_500, 24)));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn changing_a_target_is_one_undo_step() {
    let (_ctx, mut app, dir) = app_de(film(24), "budget-annuler");
    app.record_step();
    app.doc.properties.budget_ms = 5_000;
    let s1 = sequence(&app.doc, 0);
    app.doc.poser_cible_de_sequence(s1, 2_000);
    app.undo();
    assert_eq!(app.doc.properties.budget_ms, 0);
    assert!(app.doc.properties.budgets_sequences.is_empty());
    app.redo();
    assert_eq!(app.doc.properties.budget_ms, 5_000);
    assert_eq!(app.doc.properties.budgets_sequences.get(&s1), Some(&2_000));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_project_settings_window_draws_with_targets() {
    let mut doc = film(24);
    doc.properties.budget_ms = 3_000;
    let s1 = sequence(&doc, 0);
    doc.poser_cible_de_sequence(s1, 2_000);
    let (ctx, mut app, dir) = app_de(doc, "budget-fenetre");
    app.properties_window = true;
    for _ in 0..3 {
        let input = eframe::egui::RawInput { screen_rect: Some(eframe::egui::Rect::from_min_size(eframe::egui::pos2(0.0, 0.0), eframe::egui::vec2(1280.0, 900.0))), time: Some(0.0), ..Default::default() };
        let _ = ctx.run(input, |ctx| app.properties_window_ui(ctx));
    }
    assert_eq!(app.doc.properties.budget_ms, 3_000, "dessiner ne change rien");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ noms de tous les plans

#[test]
fn every_plan_name_in_one_pass_is_the_name_of_each_plan() {
    for verrouille in [true, false] {
        for modele in ["", "{projet}_EP{episode:02}_SQ{sequence:03}_SH{plan:04}_v{version:03}", "{sequence:02}_{plan:03}"] {
            let mut doc = film(24);
            doc.properties.serie = "Course".into();
            doc.properties.episode = "3".into();
            doc.properties.modele_noms = modele.into();
            if !verrouille {
                doc.numbering_locked = false;
            }
            let un_coup = doc.noms_de_tous_les_plans();
            let un_par_un: Vec<String> = (0..doc.panels.len()).map(|p| doc.nom_de_plan(p)).collect();
            assert_eq!(un_coup, un_par_un, "verrouillée {verrouille}, modèle « {modele} »");
        }
    }
}

// ------------------------------------------------------------------------------------------------ rapport de numérotation

fn envoi_de(doc: &Document, nom: &str, avec_noms: bool) -> Envoi {
    let noms = doc.noms_de_tous_les_plans();
    Envoi {
        id: 1,
        nom: nom.into(),
        date: 1_760_000_000,
        note_le: 1_760_000_100,
        fichier: String::new(),
        plans: doc
            .panels
            .iter()
            .enumerate()
            .map(|(i, p)| PlanEnvoye {
                plan: p.id,
                titre: format!("Plan {}", doc.scene_label(p.scene)),
                empreinte: 0,
                version: 1,
                images: p.frame_count(),
                nom: if avec_noms { noms[i].clone() } else { String::new() },
                sequence: if avec_noms { doc.sequence_label(doc.sequence_of(i)) } else { String::new() },
            })
            .collect(),
        variantes: Vec::new(),
    }
}

#[test]
fn the_longest_ordered_run_tells_which_plans_moved() {
    assert!(renumerotation::deplaces(&[]).is_empty());
    assert!(renumerotation::deplaces(&[0, 1, 2, 3]).is_empty());
    assert_eq!(renumerotation::deplaces(&[1, 2, 0, 3]), vec![2], "un plan reculé de deux places : lui seul a bougé");
    assert_eq!(renumerotation::deplaces(&[3, 0, 1, 2]), vec![0], "un plan envoyé tout à la fin : les autres n'ont pas bougé");
    assert_eq!(renumerotation::deplaces(&[0, 4, 1, 2, 3]), vec![1]);
    assert_eq!(renumerotation::deplaces(&[2, 1, 0]).len(), 2);
    // Des trous (plans ajoutés ou supprimés entre eux) ne comptent pas : seuls les rangs relatifs comptent.
    assert!(renumerotation::deplaces(&[0, 2, 5, 9]).is_empty());
}

#[test]
fn nothing_changed_since_the_send_gives_an_empty_report() {
    let doc = film(24);
    let envoi = envoi_de(&doc, "v1", true);
    let r = renumerotation::construire(&doc, &envoi);
    assert!(r.est_vide(), "{r:?}");
    assert_eq!((r.plans_avant, r.plans_apres, r.inchanges), (4, 4, 4));
    assert!(!r.noms_anciens_inconnus);
    let t = renumerotation::texte(&r, &doc, "2026-10-30");
    assert!(t.contains("Rien n'a changé"), "{t}");
    // Le CSV n'a que ses en-têtes.
    let csv = renumerotation::csv(&r);
    assert_eq!(csv.trim_start_matches('\u{feff}').lines().count(), 1);
}

#[test]
fn added_removed_moved_and_renumbered_plans_are_told_apart() {
    // Un plan inséré après le deuxième : « ajouté », et aucun autre plan ne change (la numérotation est verrouillée).
    let mut doc2 = film(24);
    let envoi2 = envoi_de(&doc2, "v2 au réal", true);
    let i2 = doc2.add_scene_after(1);
    let r = renumerotation::construire(&doc2, &envoi2);
    assert_eq!((r.ajoutes(), r.supprimes(), r.deplaces(), r.renumerotes()), (1, 0, 0, 0), "{r:?}");
    assert_eq!(r.inchanges, 4);
    let ligne = &r.lignes[0];
    assert!(ligne.ajoute && ligne.ancien_numero.is_empty());
    assert_eq!(ligne.nouveau_numero, doc2.scene_label(doc2.panels[i2].scene));
    assert_eq!(ligne.nouveau_nom, doc2.nom_de_plan(i2));
    assert_eq!(ligne.nouveau_rang, Some(i2 + 1));
    assert_eq!(ligne.statut(), "Ajouté");
    // Un plan retiré : « supprimé », avec son ancien nom.
    let mut doc = film(24);
    let envoi = envoi_de(&doc, "v1", true);
    let ancien_nom = doc.nom_de_plan(1);
    doc.remove_panel(1).unwrap();
    let r = renumerotation::construire(&doc, &envoi);
    assert_eq!((r.ajoutes(), r.supprimes(), r.deplaces(), r.renumerotes()), (0, 1, 0, 0), "{r:?}");
    let l = &r.lignes[0];
    assert!(l.supprime && l.ancien_nom == ancien_nom && l.ancien_rang == Some(2) && l.nouveau_rang.is_none());
    assert_eq!((r.plans_avant, r.plans_apres), (4, 3));

    // « Renuméroter » après le retrait : les plans 3 et 4 deviennent 2 et 3, le tableau dit ancien → nouveau.
    let n = doc.renumber();
    assert!(n >= 2);
    let r = renumerotation::construire(&doc, &envoi);
    assert_eq!((r.ajoutes(), r.supprimes(), r.deplaces(), r.renumerotes()), (0, 1, 0, 2), "{r:?}");
    let renumerotes: Vec<(&str, &str)> = r.lignes.iter().filter(|l| l.renumerote).map(|l| (l.ancien_numero.as_str(), l.nouveau_numero.as_str())).collect();
    assert_eq!(renumerotes, [("3", "2"), ("4", "3")]);
    assert_eq!(r.lignes.iter().find(|l| l.renumerote).unwrap().statut(), "Renuméroté");

    // Deux plans de place échangée : un seul est dit déplacé (l'autre n'a pas bougé par rapport aux suivants).
    let mut doc = film(24);
    let envoi = envoi_de(&doc, "v1", true);
    doc.panels.swap(0, 1);
    let r = renumerotation::construire(&doc, &envoi);
    assert_eq!((r.ajoutes(), r.supprimes(), r.deplaces(), r.renumerotes()), (0, 0, 1, 0), "{r:?}");
    let l = r.lignes.iter().find(|l| l.deplace).unwrap();
    assert_eq!(l.statut(), "Déplacé");
    assert_ne!(l.ancien_rang, l.nouveau_rang);
}

#[test]
fn a_changed_sequence_name_or_number_counts_as_renumbering_when_the_send_kept_it() {
    let mut doc = film(24);
    let envoi = envoi_de(&doc, "v1", true);
    let s2 = sequence(&doc, 1);
    doc.sequence_mut(s2).unwrap().name = "Poursuite".into();
    let r = renumerotation::construire(&doc, &envoi);
    assert_eq!(r.renumerotes(), 1, "le plan 4 est maintenant dans « Poursuite » : {r:?}");
    let l = &r.lignes[0];
    assert_eq!((l.ancienne_sequence.as_str(), l.nouvelle_sequence.as_str()), ("2", "Poursuite"));
    assert_eq!(l.ancien_numero, l.nouveau_numero);
    // Un envoi d'avant le rapport n'a pas gardé la séquence : seul le numéro de plan est comparé, et le rapport le dit.
    let ancien = envoi_de(&film(24), "v0", false);
    let mut doc = film(24);
    let ancien = Envoi { plans: ancien.plans.iter().zip(&doc.panels).map(|(p, panel)| PlanEnvoye { plan: panel.id, ..p.clone() }).collect(), ..ancien };
    let s2 = sequence(&doc, 1);
    doc.sequence_mut(s2).unwrap().name = "Poursuite".into();
    let r = renumerotation::construire(&doc, &ancien);
    assert!(r.est_vide() && r.noms_anciens_inconnus, "{r:?}");
    assert!(renumerotation::texte(&r, &doc, "2026-10-30").contains("seul le numéro de plan est comparé"));
}

#[test]
fn the_report_files_are_a_semicolon_csv_with_a_byte_order_mark_and_a_readable_page() {
    let mut doc = film(24);
    doc.properties.serie = "Course".into();
    doc.properties.episode = "3".into();
    let envoi = envoi_de(&doc, "v2 au réal", true);
    doc.remove_panel(1).unwrap();
    doc.renumber();
    let i = doc.add_scene_after(0);
    doc.panels[i].set_length(8);
    let r = renumerotation::construire(&doc, &envoi);
    assert!(r.ajoutes() == 1 && r.supprimes() == 1 && r.renumerotes() >= 2, "{r:?}");
    let csv = renumerotation::csv(&r);
    assert!(csv.starts_with('\u{feff}') && csv.ends_with("\r\n"));
    let lignes: Vec<&str> = csv.trim_start_matches('\u{feff}').split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(lignes[0], "statut;ancien_numero;nouveau_numero;ancien_nom;nouveau_nom;ancienne_sequence;nouvelle_sequence;ancien_rang;nouveau_rang;images_avant;images_apres");
    assert_eq!(lignes.len(), 1 + r.lignes.len());
    assert!(lignes.iter().any(|l| l.starts_with("Ajouté;")) && lignes.iter().any(|l| l.starts_with("Supprimé;")) && lignes.iter().any(|l| l.starts_with("Renuméroté;")));
    assert!(lignes.iter().all(|l| l.split(';').count() == 11));
    let t = renumerotation::texte(&r, &doc, "2026-10-30");
    assert!(t.contains("Rapport de numérotation — Course") && t.contains("Épisode 3") && t.contains("« v2 au réal »"), "{t}");
    for titre in ["AJOUTÉS (1)", "SUPPRIMÉS (1)", "RENUMÉROTÉS (ancien → nouveau)"] {
        assert!(t.contains(titre), "{titre} manque dans :\n{t}");
    }
    // Les deux fichiers s'écrivent côte à côte, sous le nom choisi.
    let dir = dossier_neuf("rapport-fichiers");
    let (c, p) = renumerotation::ecrire(&r, &doc, &dir, "Mon film : EP3").unwrap();
    assert_eq!(c.file_name().unwrap().to_string_lossy(), "Mon_film___EP3_renumerotation.csv", "un nom sûr pour Windows");
    assert_eq!(p.extension().unwrap(), "txt");
    assert_eq!(std::fs::read_to_string(&c).unwrap(), csv);
    assert!(std::fs::read_to_string(&p).unwrap().contains("AJOUTÉS"));
    let choisi = dir.join("rapport.csv");
    let (c2, p2) = renumerotation::ecrire_vers(&r, &doc, &choisi).unwrap();
    assert_eq!((c2, p2), (choisi, dir.join("rapport.txt")));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_new_send_keeps_the_name_and_the_sequence_of_each_plan() {
    use crate::envois::{Copie, Demande, NouvelEnvoi, PlanCopie};
    let mut doc = film(24);
    doc.properties.serie = "Course".into();
    doc.properties.modele_noms = "{projet}_SQ{sequence:02}_SH{plan:03}".into();
    let noms = doc.noms_de_tous_les_plans();
    let plans: Vec<PlanCopie> = doc
        .panels
        .iter()
        .enumerate()
        .map(|(i, p)| PlanCopie { panel: p.clone(), titre: crate::storyboard::panel_title(&doc, i), version: 1, nom: noms[i].clone(), sequence: doc.sequence_label(doc.sequence_of(i)) })
        .collect();
    let demande = Demande { plans, cels: HashMap::<u64, Copie>::new(), connus: HashMap::new(), tete: crate::project::Tete::de(&doc), envoi: Some(NouvelEnvoi { id: 1, nom: "v1".into(), date: 1_760_000_000, note_le: 1_760_000_100, unique: 1 }) };
    let (envoi, _) = crate::envois::construire(demande).unwrap().envoi.unwrap();
    assert_eq!(envoi.plans[0].nom, "Course_SQ01_SH001");
    assert_eq!(envoi.plans[3].nom, "Course_SQ02_SH004");
    assert_eq!((envoi.plans[0].sequence.as_str(), envoi.plans[3].sequence.as_str()), ("1", "2"));
    doc.envois.list.push(envoi);
    // Le fichier des envois les écrit, et un envoi d'avant (sans eux) se relit identique.
    let octets = crate::envois::to_bytes(&doc).unwrap().unwrap();
    let texte = String::from_utf8(octets).unwrap();
    assert!(texte.contains("\"nom\": \"Course_SQ01_SH001\"") && texte.contains("\"sequence\": \"2\""), "{texte}");
    let relus = crate::envois::parse(&texte).unwrap();
    assert_eq!(relus[0].plans[0].nom, "Course_SQ01_SH001");
    let ancien = crate::envois::to_bytes(&{
        let mut d = film(24);
        d.envois.list.push(envoi_de(&d, "v0", false));
        d
    })
    .unwrap()
    .unwrap();
    let ancien = String::from_utf8(ancien).unwrap();
    assert!(!ancien.contains("\"nom\": \"S") && !ancien.contains("\"sequence\""), "rien d'écrit quand c'est vide : {ancien}");
    assert!(crate::envois::parse(&ancien).unwrap()[0].plans.iter().all(|p| p.nom.is_empty() && p.sequence.is_empty()));
}

#[test]
fn the_report_command_writes_both_files_and_says_what_changed() {
    let mut doc = film(24);
    let envoi = envoi_de(&doc, "v1", true);
    doc.envois.list.push(envoi.clone());
    doc.remove_panel(2).unwrap();
    let (_ctx, app, dir) = app_de(doc, "rapport-commande");
    // Sans envoi : la commande dit quoi faire (et n'ouvre aucune fenêtre de fichier).
    let (_c2, mut sans, d2) = app_de(film(24), "rapport-sans-envoi");
    sans.run_action("fichier.exporter_renumerotation");
    assert!(sans.status.contains("aucun envoi"), "{}", sans.status);
    std::fs::remove_dir_all(&d2).ok();
    // Avec un envoi : le rapport se calcule et s'écrit.
    let rapport = renumerotation::depuis_le_dernier_envoi(&app.doc).unwrap();
    let csv = dir.join("essai.csv");
    let m = app.renumerotation_ecrire(&rapport, &csv).unwrap();
    assert!(m.contains("« v1 »") && m.contains("1 supprimé(s)"), "{m}");
    assert!(csv.is_file() && dir.join("essai.txt").is_file());
    let rien = renumerotation::construire(&app.doc, &Envoi { plans: envoi_de(&app.doc, "v2", true).plans, ..envoi });
    assert!(crate::renumerotationui::resume(&rien, &csv, &dir.join("essai.txt")).contains("rien n'a changé"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_hub_lists_the_report_with_its_command() {
    let entree = crate::prefsui::FAMILLES.iter().flat_map(|f| f.1.iter()).find(|e| e.3 == "fichier.exporter_renumerotation").expect("entrée du hub");
    assert_eq!(entree.1, "Rapport de numérotation");
    assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == "fichier.exporter_renumerotation"));
}

// ------------------------------------------------------------------------------------------------ nom de bobine de l'EDL

#[test]
fn the_edl_reel_carries_the_sequence_and_stays_within_eight_characters() {
    let doc = film(24);
    let o = Options { sound: false, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film").unwrap();
    let reels: Vec<&str> = plan.clips.iter().map(|c| c.reel.as_str()).collect();
    assert_eq!(reels, ["S01P001", "S01P002", "S01P003", "S02P004"], "la séquence est dans la bobine");
    assert!(reels.iter().all(|r| r.len() <= 8));
    // La séquence 2 exportée à part n'a pas la bobine d'un plan de la séquence 1, même si le plan portait le même numéro.
    let seul = editorial::build_plan(&doc, &Options { whole_film: false, ..o.clone() }, 3, "Mon film").unwrap();
    assert_eq!(seul.clips.iter().map(|c| c.reel.as_str()).collect::<Vec<_>>(), ["S02P004"]);
    // Le nom complet du plan, avec l'épisode si le modèle de noms le porte, est dans l'EDL, en commentaire.
    let mut doc = film(24);
    doc.properties.serie = "Course".into();
    doc.properties.episode = "3".into();
    doc.properties.modele_noms = "{projet}_EP{episode:02}_SQ{sequence:03}_SH{plan:04}".into();
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film").unwrap();
    assert_eq!(plan.clips[0].name, "Course_EP03_SQ001_SH0001");
    assert_eq!(plan.clips[0].reel, "S01P001", "la bobine ne dépend pas du modèle : 8 caractères au plus");
    let edl = editorial::write_edl(&plan);
    assert!(edl.contains("001  S01P001") && edl.contains("* FROM CLIP NAME: Course_EP03_SQ001_SH0001"), "{edl}");
}

#[test]
fn a_plan_number_too_long_for_a_reel_falls_back_to_the_rank_in_the_film_and_reels_never_repeat() {
    let mut doc = film(24);
    // Un numéro de plan trop long pour huit caractères (« S01P001AAA ») : la bobine de secours est le rang dans le film.
    let scene = doc.panels[1].scene;
    doc.scene_mut(scene).unwrap().number = "2ABC".into();
    let o = Options { sound: false, ..Options::default() };
    let plan = editorial::build_plan(&doc, &o, 0, "Mon film").unwrap();
    let reels: Vec<&str> = plan.clips.iter().map(|c| c.reel.as_str()).collect();
    assert_eq!(reels, ["S01P001", "R00002", "S01P003", "S02P004"], "{reels:?}");
    assert!(reels.iter().all(|r| r.len() <= 8 && r.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')));
    let uniques: std::collections::HashSet<&&str> = reels.iter().collect();
    assert_eq!(uniques.len(), reels.len());
    // La même bobine de secours, que la séquence soit exportée seule ou avec le film : elle suit le rang dans le film.
    let seule = editorial::build_plan(&doc, &Options { whole_film: false, ..o }, 1, "Mon film").unwrap();
    assert_eq!(seule.clips[1].reel, "R00002");
}

// ------------------------------------------------------------------------------------------------ PDF de studio

fn une_image() -> BoardImage {
    let mut jpeg = Vec::new();
    let rgb = vec![200u8; 16 * 9 * 3];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 80).encode(&rgb, 16, 9, image::ExtendedColorType::Rgb8).unwrap();
    BoardImage { width: 16, height: 9, jpeg }
}

fn planches(n: usize) -> (Vec<Board>, Vec<BoardImage>) {
    let boards = (0..n).map(|i| Board { title: format!("Plan {}", i + 1), duration: "1s 00".into(), sequence: "1".into(), action: "Elle court.".into(), dialogue: String::new(), notes: String::new(), comments: Vec::new() }).collect();
    (boards, (0..n).map(|_| une_image()).collect())
}

fn pages(pdf: &[u8]) -> usize {
    String::from_utf8_lossy(pdf).matches("/Type /Page /Parent").count()
}

fn texte(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

fn studio_d_essai(modifies: Vec<bool>, revision: Option<entetepdf::Revision>) -> Studio {
    Studio { entete: Entete { projet: "Course".into(), episode: "3".into(), sequence: String::new(), version: "v2 final".into(), date: "2026-10-30".into() }, modifies, revision }
}

#[test]
fn the_title_block_lines_say_only_what_exists() {
    let e = Entete { projet: "Course".into(), episode: "3".into(), sequence: String::new(), version: "v2 au réal".into(), date: "2026-10-30".into() };
    assert_eq!(e.ligne1(), "Course");
    assert_eq!(e.ligne2(), "Épisode 3 · Version v2 au réal · 2026-10-30");
    assert_eq!(e.pour_sequence("Poursuite").ligne2(), "Épisode 3 · Séquence Poursuite · Version v2 au réal · 2026-10-30");
    assert_eq!(Entete { projet: "Film".into(), date: "2026-10-30".into(), ..Default::default() }.ligne2(), "2026-10-30");
    // Du projet : la série (sinon le titre), l'épisode, le dernier envoi.
    let mut doc = film(24);
    doc.properties.title = "Titre du projet".into();
    assert_eq!(Entete::de_projet(&doc, None, "2026-10-30").projet, "Titre du projet");
    doc.properties.serie = "Brawlidays".into();
    doc.properties.episode = " 7 ".into();
    let envoi = envoi_de(&doc, "v3 pour le réal", true);
    let e = Entete::de_projet(&doc, Some(&envoi), "2026-10-30");
    assert_eq!((e.projet.as_str(), e.episode.as_str(), e.version.as_str()), ("Brawlidays", "7", "v3 pour le réal"));
}

#[test]
fn the_studio_pdf_has_a_header_with_page_numbers_and_marks_the_modified_plans() {
    let (boards, images) = planches(8);
    let l = PdfLayout { studio: true, ..PdfLayout::default() };
    let modifies = vec![false, true, false, false, false, false, false, false];
    let studio = studio_d_essai(modifies, None);
    let pdf = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &l, Fonts::Std, Some(&studio));
    assert_eq!(pages(&pdf), 2, "huit cases à six par page");
    let t = texte(&pdf);
    assert!(t.contains("Version v2 final") && t.contains("Page 1 / 2") && t.contains("Page 2 / 2"), "l'en-tête de studio dit la version et la page");
    assert!(t.contains("MODIFI"), "le plan modifié est marqué");
    assert_eq!(t.matches("0.85 0.12 0.12 rg").count(), 1, "une seule case marquée");
    // Pas de second numéro de page en bas (il est dans l'en-tête).
    assert_eq!(t.matches("Page 1 / 2").count(), 1);
    // Sans la case « marquer », rien n'est marqué.
    let sans = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout { marquer_modifies: false, ..l }, Fonts::Std, Some(&studio));
    assert!(!texte(&sans).contains("0.85 0.12 0.12 rg"));
}

#[test]
fn the_default_studio_pdf_is_the_one_from_before() {
    let (boards, images) = planches(8);
    let avant = pdfmise::storyboard_layout_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout::default(), Fonts::Std);
    // Même avec un projet qui a des envois et des plans modifiés, les choix d'origine (en-tête de studio décoché, page de révision décochée) ne changent rien
    // tant qu'aucun plan n'est marqué.
    let studio = studio_d_essai(vec![false; 8], Some(entetepdf::Revision { envoi: "v1".into(), plans: vec![], notes: vec![] }));
    let apres = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout::default(), Fonts::Std, Some(&studio));
    assert_eq!(avant, apres);
    assert!(!PdfLayout::default().studio && !PdfLayout::default().notes_revision && PdfLayout::default().marquer_modifies);
    assert_eq!(avant, pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout::default(), Fonts::Std, None));
    // Un plan marqué change l'image, mais pas le nombre de pages ni les textes.
    let marque = studio_d_essai(vec![true; 8], None);
    let m = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout::default(), Fonts::Std, Some(&marque));
    assert_ne!(avant, m);
    assert_eq!(pages(&avant), pages(&m));
}

#[test]
fn the_revision_notes_page_lists_the_modified_plans_and_the_notes_dealt_with() {
    let (boards, images) = planches(3);
    let rev = entetepdf::Revision {
        envoi: "v2 final, 30/10/2026".into(),
        plans: vec![("Séquence 1 — plan 2".into(), "Modifié".into()), ("Séquence 1 — plan 2A".into(), "Ajouté".into())],
        notes: vec![
            entetepdf::NoteTraitee { plan: "Séquence 1 — plan 2".into(), statut: "Fait".into(), auteur: "Léa".into(), texte: "Plus de flou".into(), raison: String::new() },
            entetepdf::NoteTraitee { plan: "Séquence 1 — plan 3".into(), statut: "Refusé".into(), auteur: String::new(), texte: "Tout en rouge".into(), raison: "Hors charte".into() },
        ],
    };
    let studio = studio_d_essai(vec![false, true, false], Some(rev));
    let l = PdfLayout { studio: true, notes_revision: true, ..PdfLayout::default() };
    let pdf = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &l, Fonts::Std, Some(&studio));
    assert_eq!(pages(&pdf), 2, "une page de planches, une page de révision");
    let t = texte(&pdf);
    assert!(t.contains("Notes de r"), "titre de la page");
    assert!(t.contains("Plus de flou") && t.contains("Tout en rouge") && t.contains("Hors charte"), "les notes et la raison du refus");
    assert!(t.contains("Page 2 / 2"));
    // Décochée : pas de page de plus.
    let sans = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &PdfLayout { notes_revision: false, ..l }, Fonts::Std, Some(&studio));
    assert_eq!(pages(&sans), 1);
    assert!(!texte(&sans).contains("Plus de flou"));
    // Beaucoup de notes : la page se prolonge, et le « N » de chaque page en tient compte.
    let beaucoup = entetepdf::Revision { envoi: "v1".into(), plans: vec![], notes: (0..200).map(|k| entetepdf::NoteTraitee { plan: format!("Plan {k}"), statut: "Fait".into(), auteur: String::new(), texte: format!("Note numéro {k}"), raison: String::new() }).collect() };
    let long = pdfmise::storyboard_layout_studio_with("Course", "2026-10-30", &boards, &images, 16.0 / 9.0, &l, Fonts::Std, Some(&studio_d_essai(vec![false; 3], Some(beaucoup))));
    let n = pages(&long);
    assert!(n > 2, "{n} pages");
    assert!(texte(&long).contains(&format!("Page {n} / {n}")) && texte(&long).contains("Note num"));
    // Avec les vraies polices de la machine (accents embarqués) : le fichier se fabrique aussi.
    let vrai = pdfmise::storyboard_layout_studio("Course é", "2026-10-30", &boards, &images, 16.0 / 9.0, &l, Some(&studio));
    assert!(vrai.starts_with(b"%PDF") && pages(&vrai) == 2);
}

#[test]
fn the_notes_dealt_with_since_the_send_come_from_the_decision_thread() {
    let mut doc = film(24);
    let envoi = envoi_de(&doc, "v2 final", true);
    let (p0, p1, p2) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    let a = doc.add_comment(p0, 2, "Léa", 1_760_000_150);
    let b = doc.add_comment(p0, 3, "Léa", 1_760_000_150);
    let c = doc.add_comment(p1, 1, "Marc", 1_760_000_150);
    let d = doc.add_comment(p2, 1, "Marc", 1_760_000_150);
    let e = doc.add_comment(p2, 2, "Marc", 1_760_000_150);
    for (id, texte) in [(a, "Plus de flou"), (b, "Trop tôt"), (c, "Tout en rouge"), (d, "Ouvrir plus"), (e, "Déjà traitée avant l'envoi")] {
        doc.comment_mut(id).unwrap().text = texte.into();
    }
    doc.comment_mut(a).unwrap().appliquer_statut(Status::Done, "", 1_760_000_200);
    doc.comment_mut(c).unwrap().appliquer_statut(Status::Refused, "Hors charte", 1_760_000_300);
    doc.comment_mut(e).unwrap().appliquer_statut(Status::Done, "", 1_760_000_050);
    // b est à faire, d est en discussion : ni l'une ni l'autre n'est « traitée ».
    doc.comment_mut(d).unwrap().appliquer_statut(Status::Discussion, "", 1_760_000_400);
    let notes = entetepdf::notes_traitees(&doc, envoi.note_le);
    assert_eq!(notes.iter().map(|n| (n.texte.as_str(), n.statut.as_str(), n.raison.as_str())).collect::<Vec<_>>(), [("Plus de flou", "Fait", ""), ("Tout en rouge", "Refusé", "Hors charte")]);
    assert_eq!(notes[0].plan, crate::storyboard::panel_long_title(&doc, 0));
    // Les plans modifiés : ceux que le relevé dit modifiés ou ajoutés, dans l'ordre du film.
    let mut etats = HashMap::new();
    etats.insert(p1, Etat::Modifie);
    etats.insert(p2, Etat::Nouveau);
    let rev = entetepdf::revision(&doc, &envoi, Some(&etats));
    assert_eq!(rev.plans.iter().map(|p| p.1.as_str()).collect::<Vec<_>>(), ["Modifié", "Ajouté"]);
    assert!(rev.envoi.contains("v2 final"));
    assert!(entetepdf::revision(&doc, &envoi, None).plans.is_empty(), "relevé pas prêt : pas de liste (la page le dit « Aucun »)");
    // Le PDF de studio du projet : les cases modifiées suivent les plans de chaque case.
    doc.envois.list.push(envoi);
    let s = entetepdf::studio(&doc, &[0, 1, 1, 2], Some(&etats), true, "2026-10-30");
    assert_eq!(s.modifies, [false, true, true, true]);
    assert_eq!(s.entete.version, "v2 final");
    assert!(s.revision.is_some());
    assert!(entetepdf::studio(&doc, &[0], Some(&etats), false, "2026-10-30").revision.is_none(), "case décochée : pas de page");
}

// ------------------------------------------------------------------------------------------------ planche des vignettes

fn reglages() -> crate::board::BoardSettings {
    crate::board::BoardSettings { card_w: 200.0, columns: 3, rows: 0 }
}

fn vignettes_unies(p: &crate::planche::Planche, s: f32) -> Vec<image::RgbImage> {
    (0..p.cartes.len())
        .map(|k| {
            let [w, h] = crate::planche::taille_vignette(p, k, s);
            image::RgbImage::from_pixel(w, h, image::Rgb([60, 120, 200]))
        })
        .collect()
}

#[test]
fn the_board_legends_name_each_case_with_the_naming_template_and_its_duration() {
    use crate::planche::{construire, Options};
    let mut doc = crate::vignettes_tests::quatre_plans();
    doc.properties.serie = "Course".into();
    doc.properties.modele_noms = "{projet}_SQ{sequence:02}_SH{plan:03}".into();
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    // Sans l'option : aucune légende (la planche d'avant).
    let sans = construire(&doc, &plans, &reglages(), &Options::default(), &doc.annotations);
    assert!(sans.cartes.iter().all(|c| c.legende.is_none()));
    let avec = construire(&doc, &plans, &reglages(), &Options { legendes: true, ..Options::default() }, &doc.annotations);
    assert_eq!(avec.cartes.len(), 6);
    for c in &avec.cartes {
        let (nom, duree) = c.legende.as_ref().expect("une légende par case");
        assert!(nom.starts_with("Course_SQ01_SH"), "{nom}");
        assert!(!duree.is_empty());
    }
    // La première case d'un plan porte le nom du plan ; les suivantes, « _C02 » (règle d'avant : le modèle n'a pas {case}).
    assert_eq!(avec.cartes[0].legende.as_ref().unwrap().0, doc.nom_de_plan(0));
    assert_eq!(avec.cartes[1].legende.as_ref().unwrap().0, format!("{}_C02", doc.nom_de_plan(0)));
    assert_eq!(avec.cartes[3].legende.as_ref().unwrap().0, doc.nom_de_plan(1));
    // Elle s'écrit dans l'image, à la place du numéro seul.
    let s = 2.0;
    let v = vignettes_unies(&avec, s);
    let a = crate::planche::composer(&avec, &v, &Options { legendes: true, ..Options::default() }, s).unwrap();
    let b = crate::planche::composer(&sans, &vignettes_unies(&sans, s), &Options::default(), s).unwrap();
    assert_eq!(a.dimensions(), b.dimensions(), "les légendes ne changent pas la disposition");
    assert_ne!(a, b);
    // Les noms trop longs sont coupés par « … ».
    let court = crate::planche::tronquer("Course_SQ01_SH001_C02", 10.0, 40.0);
    assert!(court.ends_with('…') && court.chars().count() < 21, "{court}");
    assert_eq!(crate::planche::tronquer("SH001", 10.0, 200.0), "SH001");
}

#[test]
fn the_board_pdf_has_a_title_block_on_every_page_and_the_old_pdf_is_unchanged_without_it() {
    use crate::planche::{construire, composer, encoder, encoder_avec, Options, Sortie};
    let doc = crate::vignettes_tests::quatre_plans();
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    let o = Options::default();
    let p = construire(&doc, &plans, &reglages(), &o, &doc.annotations);
    let s = 1.0;
    let img = composer(&p, &vignettes_unies(&p, s), &o, s).unwrap();
    let bloc = Entete { projet: "Course".into(), episode: "3".into(), sequence: "1".into(), version: "v2 final".into(), date: "2026-10-30".into() };
    for sortie in [Sortie::PdfUnePage, Sortie::PdfA4, Sortie::PdfA3] {
        let avant = encoder(&p, &img, sortie, "T", s).unwrap();
        // Sans bloc : exactement le fichier d'avant.
        assert_eq!(avant, encoder_avec(&p, &img, sortie, "T", s, None).unwrap(), "{sortie:?}");
        let avec = encoder_avec(&p, &img, sortie, "T", s, Some(&bloc)).unwrap();
        assert!(avec.starts_with(b"%PDF") && pages(&avec) == pages(&avant), "{sortie:?} : même nombre de pages");
        // (Avec les polices embarquées de la machine, le texte est écrit en numéros de glyphes : on le lit dans `bloc_de_titre`, plus bas.)
        assert_ne!(avec, avant, "{sortie:?} : le bloc change la page");
        assert!(avec.len() > avant.len());
    }
    let mut std = Fonts::Std;
    let ops = crate::planche::bloc_de_titre(&mut std, 600.0, 842.0, (20.0, 20.0), 1.0, &bloc, "Page 1 / 3");
    assert!(ops.contains("Course") && ops.contains("Version v2 final") && ops.contains("Page 1 / 3"), "{ops}");
    // Une grande page : elle grandit de la hauteur du bloc.
    let une = |b: Option<&Entete>| texte(&encoder_avec(&p, &img, Sortie::PdfUnePage, "T", s, b).unwrap());
    let hauteur = |t: &str| {
        let i = t.find("/MediaBox [0 0 ").unwrap() + 15;
        t[i..].split([' ', ']']).nth(1).unwrap().parse::<f32>().unwrap()
    };
    assert!(hauteur(&une(Some(&bloc))) > hauteur(&une(None)) + 30.0);
    // Une image (PNG, JPEG) n'a pas de bloc : le fichier ne change pas.
    assert_eq!(encoder(&p, &img, Sortie::Png, "T", s).unwrap(), encoder_avec(&p, &img, Sortie::Png, "T", s, Some(&bloc)).unwrap());
    // Les pages A4 et A3 : tous les morceaux de la planche tiennent encore, le nombre de pages ne dépend que de la place restante.
    let a4 = |b| pages(&encoder_avec(&p, &img, Sortie::PdfA4, "T", s, b).unwrap());
    assert!(a4(Some(&bloc)) >= a4(None));
}

#[test]
fn the_board_window_options_default_to_legends_and_title_block_but_the_library_defaults_do_not() {
    let o = crate::planche::Options::default();
    assert!(!o.legendes && !o.bloc_titre, "les essais d'avant lisent les mêmes défauts");
    let e = crate::plancheui::Etat::default();
    assert!(e.opt.legendes && e.opt.bloc_titre, "la fenêtre les coche");
}

#[test]
fn the_board_title_block_names_the_sequence_only_when_the_plans_are_in_one_sequence() {
    let (_ctx, app, dir) = app_de(film(24), "planche-entete");
    let tous = app.planche_entete(&[0, 1, 2, 3]);
    assert_eq!(tous.sequence, "");
    let une = app.planche_entete(&[0, 1]);
    assert_eq!(une.sequence, "1");
    let autre = app.planche_entete(&[3]);
    assert_eq!(autre.sequence, "2");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------------------ fenêtres de fichiers

fn dossier_vrai(base: &Path, nom: &str) -> PathBuf {
    let d = base.join(nom);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_file_dialogs_open_in_the_project_folder_or_the_last_folder_chosen_inside_it() {
    use crate::dialogues::dossier_de_depart;
    let base = dossier_neuf("dialogues");
    let projet = dossier_vrai(&base, "Film");
    let dedans = dossier_vrai(&projet, "exports");
    let dehors = dossier_vrai(&base, "ailleurs");
    // Aucun projet ouvert : on ne propose rien (le système choisit).
    assert_eq!(dossier_de_depart(None, None), None);
    assert_eq!(dossier_de_depart(None, Some(&dedans)), None);
    // Un projet, rien de retenu : le dossier du projet.
    assert_eq!(dossier_de_depart(Some(&projet), None), Some(projet.clone()));
    // Le dernier dossier de cette sorte, s'il est dans le projet ; sinon le projet.
    assert_eq!(dossier_de_depart(Some(&projet), Some(&dedans)), Some(dedans.clone()));
    assert_eq!(dossier_de_depart(Some(&projet), Some(&dehors)), Some(projet.clone()));
    assert_eq!(dossier_de_depart(Some(&projet), Some(&projet.join("disparu"))), Some(projet.clone()), "un dossier qui n'existe plus n'est pas proposé");
    // Un projet dont le dossier a disparu : rien.
    assert_eq!(dossier_de_depart(Some(&base.join("perdu")), None), None);
    // Un dernier dossier par sorte, retenu pour la séance.
    crate::dialogues::retenir("essai_a", &dedans);
    crate::dialogues::retenir("essai_b", &dehors);
    assert_eq!(crate::dialogues::dernier("essai_a"), Some(dedans.clone()));
    assert_eq!(crate::dialogues::dernier("essai_b"), Some(dehors.clone()));
    assert_eq!(crate::dialogues::dernier("essai_c"), None);
    std::fs::remove_dir_all(&base).ok();
}

#[test]
fn the_app_finds_the_project_folder_and_saves_next_to_it_not_inside() {
    let base = dossier_neuf("dialogues-app");
    let projet = dossier_vrai(&base, "Film");
    let (_ctx, mut app, dir) = app_de(film(24), "dialogues-app-b");
    assert_eq!(app.dossier_du_projet(), None);
    assert_eq!(app.dossier_pour("fiche"), None, "rien d'enregistré : le système choisit");
    assert_eq!(app.dossier_pour_enregistrer_sous(), None);
    app.path = Some(projet.join("Film.shot"));
    assert_eq!(app.dossier_du_projet(), Some(projet.clone()));
    assert_eq!(app.dossier_pour("fiche"), Some(projet.clone()));
    // « Enregistrer sous » crée un dossier à ce nom : la fenêtre s'ouvre à côté du dossier du projet.
    assert_eq!(app.dossier_pour_enregistrer_sous(), Some(base.clone()));
    crate::dialogues::retenir("fiche_app", &projet.join("exports"));
    std::fs::remove_dir_all(&base).ok();
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn every_export_dialog_goes_through_the_central_helpers() {
    for (fichier, source) in [
        ("exportui.rs", include_str!("exportui.rs")),
        ("ficheui.rs", include_str!("ficheui.rs")),
        ("listeplansui.rs", include_str!("listeplansui.rs")),
        ("plancheui.rs", include_str!("plancheui.rs")),
        ("colorscriptui.rs", include_str!("colorscriptui.rs")),
        ("editorialui.rs", include_str!("editorialui.rs")),
        ("paquetui.rs", include_str!("paquetui.rs")),
        ("renumerotationui.rs", include_str!("renumerotationui.rs")),
    ] {
        assert!(!source.contains(".save_file()") && !source.contains(".pick_folder()"), "{fichier} ouvre une fenêtre « où écrire » sans passer par dialogues.rs");
    }
    // « Enregistrer sous » garde sa fenêtre à lui, mais elle part du dossier du projet.
    assert!(include_str!("enregistrement.rs").contains("dossier_pour_enregistrer_sous"));
}

/// Écrit des exemples de chaque livrable dans le dossier `SHOT_LIVRABLES_DIR` (rien si la variable n'est pas posée).
#[test]
fn write_the_sample_files_when_asked() {
    let Some(dir) = std::env::var_os("SHOT_LIVRABLES_DIR").map(PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let mut doc = film(24);
    doc.properties.serie = "Course-poursuite".into();
    doc.properties.episode = "3".into();
    doc.properties.title = "Course-poursuite".into();
    doc.properties.budget_ms = 5_000;
    let s1 = sequence(&doc, 0);
    doc.poser_cible_de_sequence(s1, 2_000);
    doc.panels[0].action = "Léa court dans la ruelle.".into();
    doc.panels[1].dialogue = "Léa : « Plus vite ! »".into();
    let envoi = envoi_de(&doc, "v2 au réal", true);
    doc.envois.list.push(envoi.clone());
    doc.remove_panel(1).unwrap();
    doc.renumber();
    let i = doc.add_scene_after(0);
    doc.panels[i].set_length(8);
    doc.panels[i].action = "Gros plan sur la clé.".into();
    let r = renumerotation::construire(&doc, &envoi);
    renumerotation::ecrire_vers(&r, &doc, &dir.join("rapport_renumerotation.csv")).unwrap();
    // La fiche avec la colonne d'écart cumulé.
    let plans: Vec<usize> = (0..doc.panels.len()).collect();
    let colonnes = [Colonne::Sequence, Colonne::Numero, Colonne::Reference, Colonne::DureeSecondes, Colonne::EcartCumule];
    let f = fiche::build(&doc, &Entree { plans: &plans, colonnes: &colonnes, par_image: false, start_tc: 0, etats: None, envoi: None }).unwrap();
    std::fs::write(dir.join("fiche_ecart_cumule.csv"), crate::ficheformats::csv(&f, crate::ficheformats::Separateur::PointVirgule)).unwrap();
    // L'EDL.
    let plan = editorial::build_plan(&doc, &Options { sound: false, ..Options::default() }, 0, "Course").unwrap();
    std::fs::write(dir.join("montage.edl"), editorial::write_edl(&plan)).unwrap();
    // Le storyboard PDF de studio.
    let (boards, images) = planches(5);
    let mut etats = HashMap::new();
    etats.insert(doc.panels[i].id, Etat::Nouveau);
    let studio = entetepdf::studio(&doc, &[0, 1, i, 2, 3], Some(&etats), true, "2026-10-31");
    let l = PdfLayout { studio: true, notes_revision: true, ..PdfLayout::default() };
    std::fs::write(dir.join("storyboard_studio.pdf"), pdfmise::storyboard_layout_studio("Course-poursuite", "2026-10-31", &boards, &images, 16.0 / 9.0, &l, Some(&studio))).unwrap();
    // La planche PDF légendée.
    use crate::planche::{composer, construire, encoder_avec, Options as PO, Sortie};
    let docp = crate::vignettes_tests::quatre_plans();
    let po = PO { legendes: true, ..PO::default() };
    let tous: Vec<usize> = (0..docp.panels.len()).collect();
    let p = construire(&docp, &tous, &reglages(), &po, &docp.annotations);
    let img = composer(&p, &vignettes_unies(&p, 1.5), &po, 1.5).unwrap();
    let bloc = Entete { projet: "Course-poursuite".into(), episode: "3".into(), sequence: String::new(), version: "v2 au réal".into(), date: "2026-10-31".into() };
    std::fs::write(dir.join("planche_legendee.pdf"), encoder_avec(&p, &img, Sortie::PdfA4, "Course-poursuite", 1.5, Some(&bloc)).unwrap()).unwrap();
}
// ------------------------------------------------------------------------------------------------ outils

fn app_de(doc: Document, nom: &str) -> (eframe::egui::Context, crate::app::App, PathBuf) {
    let dir = dossier_neuf(nom);
    crate::donnees::bac_a_sable(&dir);
    let ctx = eframe::egui::Context::default();
    let mut app = crate::app::App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    (ctx, app, dir)
}
