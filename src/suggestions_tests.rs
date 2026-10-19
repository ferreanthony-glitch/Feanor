//! Tests des suggestions sonores dans l'application, **sans réseau ni modèle** (fournisseurs simulés
//! de `fournisseurs_tests.rs`) : lire le storyboard par règles, la portée, proposer → générer →
//! accepter (le son se pose à la bonne image sur la piste « Sons proposés », avec annulation), annuler,
//! la confirmation d'envoi en ligne, les vignettes, les clés, l'essai de connexion, le panneau.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::clessecretes::{Cle, Coffre, CoffreMemoire, Service};
use crate::dock::Pane;
use crate::document::Document;
use crate::fournisseurs::ErreurGen;
use crate::fournisseurs_tests::{FabriqueSimulee, LecteurSimule};
use crate::fournisseursapi::TransportABrancher;
use crate::generation::FabriqueReelle;
use crate::genreglages::{Mode, Role};
use crate::indications::{Genre, IndicationBrute, Statut, PISTE};
use crate::lecturesb::{entree_plan, plans_de_la_portee, vignette_png, Portee};
use crate::suggestionsui::{Action, ChoixPortee, Edition};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-suggestions-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Un son par plan : à l'image 5, 12 images.
fn un_son_par_plan() -> LecteurSimule {
    LecteurSimule::new(|p| {
        vec![IndicationBrute { start: 5, frames: 12, kind: Some(Genre::Bruitage), description: format!("Son du {}", p.titre), prompt: "a sound".into(), ..Default::default() }]
    })
}

/// Une application avec trois plans de 24 images et des fournisseurs simulés.
fn application(name: &str, lecteur: LecteurSimule) -> (App, Arc<FabriqueSimulee>, PathBuf) {
    let dir = temp(name);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc.add_scene_after(0);
    app.doc.add_scene_after(1);
    for p in app.doc.panels.iter_mut() {
        p.set_length(24);
    }
    let fab = Arc::new(FabriqueSimulee::new(lecteur));
    app.sug.fabrique = fab.clone();
    app.sug.coffre = Arc::new(CoffreMemoire::default());
    app.sug.dossier = dir.join("cache");
    (app, fab, dir)
}

/// Fait tourner l'interface (ses reprises, image après image) jusqu'à ce que plus rien ne travaille.
fn pomper(app: &mut App) {
    let ctx = egui::Context::default();
    let t = Instant::now();
    loop {
        app.poll_suggestions(&ctx);
        if !app.sug.occupe() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(30), "les suggestions ne finissent pas");
        std::thread::sleep(Duration::from_millis(2));
    }
    app.poll_suggestions(&ctx);
}

fn ids(app: &App) -> Vec<u64> {
    app.indications_en_ordre()
}

/// Lit le film avec le lecteur simulé et génère tout : les propositions sont prêtes.
fn proposes_et_pretes(app: &mut App) -> Vec<u64> {
    app.lire_storyboard(Portee::Film);
    pomper(app);
    let tous = ids(app);
    app.generer_indications(tous.clone());
    pomper(app);
    tous
}

// ================================================================================== lire (règles)

#[test]
fn reading_the_storyboard_by_rules_needs_no_model_and_fills_the_list() {
    let (mut app, _fab, dir) = application("regles", un_son_par_plan());
    // Le vrai fournisseur, avec le choix d'origine (règles) : aucun serveur, aucune clé.
    app.sug.fabrique = Arc::new(FabriqueReelle { transport: Arc::new(TransportABrancher), coffre: Arc::new(CoffreMemoire::default()), serveur: crate::serveurlocal::GestionnaireServeur::inactif() });
    assert_eq!(app.settings.generation_son.lecture, Mode::Regles);
    app.doc.panels[0].action = "La porte claque. Il pleut dehors.".into();
    app.doc.panels[0].dialogue = "MARIE : Viens ici.".into();
    app.doc.panels[1].action = "Un klaxon retentit.".into();
    app.modified = false;
    app.lire_storyboard(Portee::Film);
    assert!(app.sug.lecture_en_cours());
    assert!(app.status.contains("Lecture du storyboard"), "{}", app.status);
    pomper(&mut app);
    let liste = ids(&app);
    assert_eq!(liste.len(), 4, "{:#?}", app.doc.sons_proposes.indications);
    let titres: Vec<String> = liste.iter().map(|&i| app.doc.sons_proposes.get(i).unwrap().titre()).collect();
    assert!(titres.iter().any(|t| t == "Porte qui claque") && titres.iter().any(|t| t == "Pluie") && titres.iter().any(|t| t.starts_with("MARIE")) && titres.iter().any(|t| t == "Klaxon"), "{titres:?}");
    // Dans l'ordre du film : les trois du plan 1, puis celui du plan 2.
    let plans: Vec<u64> = liste.iter().map(|&i| app.doc.sons_proposes.get(i).unwrap().plan).collect();
    assert_eq!(plans, vec![app.doc.panels[0].id; 3].into_iter().chain([app.doc.panels[1].id]).collect::<Vec<_>>());
    assert!(liste.iter().all(|&i| app.doc.sons_proposes.get(i).unwrap().status == Statut::Proposee));
    assert!(app.modified, "le projet est à enregistrer");
    assert!(app.status.contains("4 propositions ajoutées"), "{}", app.status);
    // Relire ne double rien.
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(ids(&app).len(), 4);
    assert!(app.status.contains("0 proposition ajoutée") && app.status.contains("4 déjà connues"), "{}", app.status);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_scope_picks_the_whole_film_a_sequence_or_chosen_plans() {
    let (mut app, fab, dir) = application("portee", un_son_par_plan());
    // Une deuxième séquence, avec un quatrième plan.
    let s2 = app.doc.new_sequence();
    let scene = app.doc.new_scene(s2);
    let p = app.doc.make_panel(scene);
    app.doc.panels.push(p);
    let (a, b, c, d) = (app.doc.panels[0].id, app.doc.panels[1].id, app.doc.panels[2].id, app.doc.panels[3].id);
    assert_eq!(plans_de_la_portee(&app.doc, &Portee::Film), vec![0, 1, 2, 3]);
    assert_eq!(plans_de_la_portee(&app.doc, &Portee::Sequence(s2)), vec![3]);
    assert_eq!(plans_de_la_portee(&app.doc, &Portee::Plans(vec![c, a])), vec![0, 2], "dans l'ordre du film");
    assert!(plans_de_la_portee(&app.doc, &Portee::Plans(vec![987])).is_empty());
    // Ce que le panneau propose.
    app.cur_panel = 3;
    app.sug.portee = ChoixPortee::Sequence;
    assert_eq!(app.portee_choisie(), Portee::Sequence(s2));
    app.sug.portee = ChoixPortee::Plan;
    assert_eq!(app.portee_choisie(), Portee::Plans(vec![d]));
    app.sug.portee = ChoixPortee::Plage;
    app.sug.plage = (3, 2);
    assert_eq!(app.portee_choisie(), Portee::Plans(vec![b, c]), "les bornes se remettent dans l'ordre");
    app.sug.plage = (0, 99);
    assert_eq!(app.portee_choisie(), Portee::Plans(vec![a, b, c, d]), "et dans le film");
    // Lire seulement le plan 2 et le plan 3.
    app.lire_storyboard(Portee::Plans(vec![b, c]));
    pomper(&mut app);
    assert_eq!(ids(&app).len(), 2);
    assert_eq!(fab.lecteur.appels.lock().unwrap().iter().map(|a| a.0).collect::<Vec<_>>(), vec![0, 1], "le lecteur ne voit que les plans de la portée");
    // Rien à lire.
    app.lire_storyboard(Portee::Plans(vec![987]));
    assert_eq!(app.status, "Aucun plan à lire.");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn what_a_reader_receives_is_the_same_for_every_provider() {
    let (mut app, _f, dir) = application("entree", un_son_par_plan());
    app.doc.panels[1].action = "Il court.".into();
    app.doc.panels[1].dialogue = "PAUL : Attends !".into();
    app.doc.panels[1].notes = "SFX : pas".into();
    app.doc.panels[1].marks.push(crate::document::ImageMark { id: 987_001, frame: 10, color: 0, action: "Il tombe.".into(), dialogue: String::new(), notes: String::new() });
    let (p0, p1) = (app.doc.panels[0].id, app.doc.panels[1].id);
    app.doc.set_transition(1, crate::transitions::Kind::Dissolve).unwrap();
    let c = app.doc.add_comment(p1, 3, "Léa", 1);
    app.doc.comment_mut(c).unwrap().text = "Trop long.".into();
    let r = app.doc.add_comment(p1, 4, "Léa", 2);
    app.doc.comment_mut(r).unwrap().text = "Déjà réglé.".into();
    app.doc.comment_mut(r).unwrap().done = true;
    let e = entree_plan(&app.doc, 1);
    assert_eq!((e.plan, e.titre.as_str(), e.frames, e.debut), (p1, "Plan 2", 24, 24));
    assert_eq!((e.action.as_str(), e.dialogue.as_str(), e.notes.as_str()), ("Il court.", "PAUL : Attends !", "SFX : pas"));
    assert_eq!(e.marques.len(), 1);
    assert_eq!((e.marques[0].frame, e.marques[0].action.as_str()), (10, "Il tombe."));
    assert!(e.transition_entrante.as_deref().is_some_and(|t| t.starts_with("Fondu enchaîné")), "{:?}", e.transition_entrante);
    assert_eq!(e.transition_sortante, None);
    assert_eq!(e.commentaires, vec!["Trop long.".to_string()], "les commentaires résolus n'y sont pas");
    assert!(entree_plan(&app.doc, 0).transition_sortante.is_some());
    assert_eq!(entree_plan(&app.doc, 0).plan, p0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_thumbnail_is_a_small_png_with_the_shape_of_the_frame() {
    let (app, _f, dir) = application("vignette", un_son_par_plan());
    let png = vignette_png(&app.doc, 0, 96).expect("vignette");
    assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    let img = image::load_from_memory(&png).expect("un PNG lisible");
    let [fw, fh] = app.doc.frame;
    assert_eq!(img.width(), 96);
    assert_eq!(img.height(), ((96.0 * fh as f32 / fw as f32).round() as u32).max(8));
    assert!(png.len() < 200_000);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn thumbnails_go_to_a_model_reader_only_when_asked_and_never_to_the_rules() {
    let (mut app, fab, dir) = application("vignettes", un_son_par_plan());
    let recues = |fab: &FabriqueSimulee| fab.lecteur.vignettes.lock().unwrap().iter().map(|v| v.as_ref().map(|b| b.starts_with(&[0x89, b'P', b'N', b'G']))).collect::<Vec<_>>();
    // Un lecteur local, vignettes demandées : chaque plan arrive avec la sienne.
    app.settings.generation_son.lecture = Mode::Local;
    app.settings.generation_son.vignettes = true;
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(recues(&fab), vec![Some(true); 3]);
    // Sans l'envoi des vignettes : les textes seuls.
    fab.lecteur.vignettes.lock().unwrap().clear();
    app.settings.generation_son.vignettes = false;
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(recues(&fab), vec![None; 3]);
    // Le lecteur par règles n'a pas besoin d'image.
    fab.lecteur.vignettes.lock().unwrap().clear();
    app.settings.generation_son.lecture = Mode::Regles;
    app.settings.generation_son.vignettes = true;
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(recues(&fab), vec![None; 3]);
    std::fs::remove_dir_all(&dir).ok();
}

// ====================================================================== proposer, générer, accepter

#[test]
fn propose_then_generate_then_accept_places_the_sound_at_the_right_frame_on_the_proposed_track() {
    let (mut app, fab, dir) = application("flux", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let liste = ids(&app);
    assert_eq!(liste.len(), 3);
    // Générer le son du deuxième plan : un fichier dans le cache, la proposition est prête.
    let id = liste[1];
    app.generer_indications(vec![id]);
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::EnGeneration);
    assert!(app.sug.generation_en_cours());
    pomper(&mut app);
    let i = app.doc.sons_proposes.get(id).unwrap().clone();
    assert_eq!(i.status, Statut::Prete);
    let fichier = i.file.clone().expect("fichier");
    assert!(fichier.starts_with(dir.join("cache")) && fichier.is_file(), "{fichier:?}");
    assert_eq!(fab.son.appels.lock().unwrap().len(), 1);
    let demande = fab.son.appels.lock().unwrap()[0].clone();
    assert_eq!((demande.genre, demande.prompt.as_str()), (Genre::Bruitage, "a sound"));
    let voulue = (12.0 / app.doc.fps as f32).max(0.5);
    assert!((demande.secondes - voulue).abs() < 1e-4, "la durée voulue est celle de l'indication : {}", demande.secondes);
    assert!(app.doc.audio_tracks.is_empty(), "rien n'est posé avant l'acceptation");
    // Accepter.
    app.accepter_indication(id);
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks.len(), 1);
    let piste = &app.doc.audio_tracks[0];
    assert_eq!(piste.name, PISTE);
    assert_eq!(piste.clips.len(), 1);
    let clip = piste.clips[0].clone();
    assert_eq!(clip.start, app.doc.panel_start(1) + 5, "l'image du plan 2 où le son est voulu");
    let son = &app.doc.sounds[&clip.sound];
    assert_eq!(son.name, "Son du Plan 2.wav");
    assert_eq!(clip.length, son.frames(app.doc.fps));
    assert_eq!(**son.bytes, std::fs::read(&fichier).unwrap(), "le fichier est copié tel quel dans le projet");
    let i = app.doc.sons_proposes.get(id).unwrap();
    assert_eq!((i.status, i.clip), (Statut::Acceptee, Some(clip.id)));
    assert_eq!(app.sel_clip, Some(clip.id));
    assert!(app.status.contains("Son accepté") && app.status.contains(PISTE), "{}", app.status);
    assert_eq!(app.history.names().0.last().copied(), Some("Accepter un son proposé"));
    // Annuler retire le son et la proposition redevient prête ; rétablir la remet acceptée.
    app.undo();
    pomper(&mut app);
    assert!(app.doc.audio_tracks.iter().all(|t| t.clips.is_empty()));
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Prete);
    app.redo();
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks[0].clips.len(), 1);
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Acceptee);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn several_accepted_sounds_share_the_track_until_they_overlap() {
    let (mut app, _f, dir) = application("pistes", un_son_par_plan());
    let liste = proposes_et_pretes(&mut app);
    assert!(liste.iter().all(|&i| app.doc.sons_proposes.get(i).unwrap().status == Statut::Prete));
    for &i in &liste {
        app.accepter_indication(i);
    }
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks.len(), 1, "trois plans, trois moments différents : une seule piste");
    let debuts: Vec<usize> = app.doc.audio_tracks[0].clips.iter().map(|c| c.start).collect();
    assert_eq!(debuts, (0..3).map(|k| app.doc.panel_start(k) + 5).collect::<Vec<_>>());
    // Un quatrième son voulu au même moment que le premier : une deuxième piste.
    let plan0 = app.doc.panels[0].id;
    let double = app.doc.sons_proposes.ajouter(plan0, IndicationBrute { start: 6, frames: 12, kind: Some(Genre::Bruitage), description: "Pile dessus".into(), prompt: "x".into(), ..Default::default() });
    app.generer_indications(vec![double]);
    pomper(&mut app);
    app.accepter_indication(double);
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks.len(), 2);
    assert_eq!(app.doc.audio_tracks[1].name, "Sons proposés 2");
    // Les pistes de l'utilisateur ne sont pas prises : une piste « Dialogues » verrouillée reste vide.
    app.doc.add_audio_track();
    app.doc.audio_tracks[2].name = "Dialogues".into();
    app.doc.audio_tracks[2].locked = true;
    let plan2 = app.doc.panels[2].id;
    let encore = app.doc.sons_proposes.ajouter(plan2, IndicationBrute { start: 20, frames: 4, kind: Some(Genre::Bruitage), description: "Encore".into(), prompt: "y".into(), ..Default::default() });
    app.generer_indications(vec![encore]);
    pomper(&mut app);
    app.accepter_indication(encore);
    pomper(&mut app);
    assert!(app.doc.audio_tracks[2].clips.is_empty());
    assert_eq!(app.doc.audio_tracks[0].clips.len(), 4, "le dernier tient sur la première piste, libre à cet endroit");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_voice_is_generated_by_the_voice_provider_with_the_default_voice() {
    let (mut app, fab, dir) = application("voix", LecteurSimule::new(|_| vec![IndicationBrute { start: 0, frames: 30, kind: Some(Genre::Voix), description: "MARIE : « Viens. »".into(), text: "Viens.".into(), character: "MARIE".into(), ..Default::default() }]));
    app.settings.generation_son.voix_locale = "ff_siwis".into();
    app.lire_storyboard(Portee::Plans(vec![app.doc.panels[0].id]));
    pomper(&mut app);
    let id = ids(&app)[0];
    app.generer_indications(vec![id]);
    pomper(&mut app);
    let d = fab.voix.appels.lock().unwrap()[0].clone();
    assert_eq!((d.texte.as_str(), d.personnage.as_str(), d.voix.as_str(), d.langue.as_str()), ("Viens.", "MARIE", "ff_siwis", "fr"));
    assert!(fab.son.appels.lock().unwrap().is_empty(), "une voix ne passe pas par le générateur de sons");
    // Une voix choisie sur l'indication l'emporte sur celle des préférences.
    app.doc.sons_proposes.get_mut(id).unwrap().voice = "am_adam".into();
    app.generer_indications(vec![id]);
    pomper(&mut app);
    assert_eq!(fab.voix.appels.lock().unwrap()[1].voix, "am_adam");
    app.accepter_indication(id);
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks[0].clips[0].start, 0);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_second_request_waits_for_the_running_generation_and_then_goes() {
    let (mut app, fab, dir) = application("file", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let liste = ids(&app);
    fab.son.bloque.store(true, std::sync::atomic::Ordering::Relaxed);
    app.generer_indications(vec![liste[0]]);
    let ctx = egui::Context::default();
    let t = Instant::now();
    while fab.son.appels.lock().unwrap().is_empty() {
        app.poll_suggestions(&ctx);
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    app.generer_indications(vec![liste[1]]);
    app.poll_suggestions(&ctx);
    assert_eq!(app.sug.taches.len(), 1, "une seule génération à la fois (un seul serveur, une seule carte)");
    assert_eq!(app.doc.sons_proposes.get(liste[1]).unwrap().status, Statut::EnGeneration, "en attente");
    fab.son.bloque.store(false, std::sync::atomic::Ordering::Relaxed);
    pomper(&mut app);
    assert!(liste[..2].iter().all(|&i| app.doc.sons_proposes.get(i).unwrap().status == Statut::Prete));
    assert_eq!(fab.son.appels.lock().unwrap().len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn accepting_something_that_is_not_ready_or_not_there_says_so_and_changes_nothing() {
    let (mut app, _f, dir) = application("refus", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let id = ids(&app)[0];
    app.accepter_indication(id);
    assert!(app.status.contains("pas prêt"), "{}", app.status);
    assert!(app.sug.acceptations_vides());
    app.generer_indications(vec![id]);
    pomper(&mut app);
    // Le fichier a été effacé entre-temps.
    let f = app.doc.sons_proposes.get(id).unwrap().file.clone().unwrap();
    std::fs::remove_file(&f).unwrap();
    app.accepter_indication(id);
    pomper(&mut app);
    assert!(app.status.contains("Impossible d'accepter"), "{}", app.status);
    assert!(app.doc.audio_tracks.is_empty());
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Prete);
    // Le plan a disparu.
    app.generer_indications(vec![id]);
    pomper(&mut app);
    app.doc.sons_proposes.get_mut(id).unwrap().plan = 987_654;
    app.accepter_indication(id);
    assert!(app.status.contains("n'existe plus"), "{}", app.status);
    assert!(app.doc.audio_tracks.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn reject_reopen_delete_and_edit_a_proposal() {
    let (mut app, _f, dir) = application("gestion", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let liste = ids(&app);
    let (a, b, c) = (liste[0], liste[1], liste[2]);
    app.rejeter_indication(a);
    assert_eq!(app.doc.sons_proposes.get(a).unwrap().status, Statut::Rejetee);
    // Une relecture ne ramène pas ce que le réalisateur a rejeté.
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(ids(&app).len(), 3);
    assert_eq!(app.doc.sons_proposes.get(a).unwrap().status, Statut::Rejetee);
    app.rouvrir_indication(a);
    assert_eq!(app.doc.sons_proposes.get(a).unwrap().status, Statut::Proposee);
    // Modifier : la description, le prompt, le moment ; la relecture n'y touche plus.
    app.appliquer_edition(Edition { id: b, kind: Genre::Ambiance, start: 7, frames: 40, description: "  Vent fort ".into(), prompt: "strong wind".into(), text: String::new(), character: String::new(), voice: String::new() });
    let i = app.doc.sons_proposes.get(b).unwrap();
    assert_eq!((i.kind, i.start, i.frames, i.description.as_str(), i.prompt.as_str(), i.edited), (Genre::Ambiance, 7, 40, "Vent fort", "strong wind", true));
    let plan_b = i.plan;
    app.doc.sons_proposes.fusionner(plan_b, vec![]);
    assert!(app.doc.sons_proposes.get(b).is_some(), "une proposition modifiée n'est pas retirée par une relecture");
    // Supprimer les rejetées, puis une proposition.
    app.rejeter_indication(c);
    app.supprimer_rejetees();
    assert!(app.doc.sons_proposes.get(c).is_none() && app.status.contains("1 proposition rejetée supprimée"), "{}", app.status);
    app.supprimer_indication(a);
    assert_eq!(ids(&app), vec![b]);
    // On ne rejette pas ce qui est accepté, et on ne supprime pas ce qui se génère.
    app.doc.sons_proposes.get_mut(b).unwrap().status = Statut::EnGeneration;
    app.supprimer_indication(b);
    app.rejeter_indication(b);
    assert_eq!(app.doc.sons_proposes.get(b).unwrap().status, Statut::EnGeneration);
    std::fs::remove_dir_all(&dir).ok();
}

// ==================================================================================== annuler

#[test]
fn cancelling_a_generation_restores_the_proposals_and_writes_nothing() {
    let (mut app, fab, dir) = application("annuler-generation", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let liste = ids(&app);
    fab.son.bloque.store(true, std::sync::atomic::Ordering::Relaxed);
    app.generer_indications(liste.clone());
    let ctx = egui::Context::default();
    let t = Instant::now();
    while fab.son.appels.lock().unwrap().is_empty() {
        app.poll_suggestions(&ctx);
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(liste.iter().all(|&i| app.doc.sons_proposes.get(i).unwrap().status == Statut::EnGeneration));
    app.annuler_suggestions();
    assert!(app.status.contains("Annulation"));
    pomper(&mut app);
    assert!(liste.iter().all(|&i| app.doc.sons_proposes.get(i).unwrap().status == Statut::Proposee), "{:?}", liste.iter().map(|&i| app.doc.sons_proposes.get(i).unwrap().status).collect::<Vec<_>>());
    assert!(app.doc.sons_proposes.indications.iter().all(|i| i.file.is_none()));
    assert!(!dir.join("cache").exists() || std::fs::read_dir(dir.join("cache")).unwrap().next().is_none());
    assert!(app.status.contains("annulé"), "{}", app.status);
    assert!(!app.sug.occupe());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_a_reading_stops_it_and_keeps_what_was_read() {
    let mut lent = un_son_par_plan();
    lent.delai = Duration::from_millis(80);
    let (mut app, fab, dir) = application("annuler-lecture", lent);
    for _ in 0..30 {
        let n = app.doc.panels.len() - 1;
        app.doc.add_scene_after(n);
    }
    app.lire_storyboard(Portee::Film);
    let ctx = egui::Context::default();
    let t = Instant::now();
    while fab.lecteur.appels.lock().unwrap().len() < 2 {
        app.poll_suggestions(&ctx);
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    app.annuler_suggestions();
    pomper(&mut app);
    let lus = ids(&app).len();
    assert!(lus >= 1 && lus < 33, "{lus}");
    assert!(fab.lecteur.appels.lock().unwrap().len() < 33);
    assert!(app.status.contains("annulé"), "{}", app.status);
    // Annuler avant la fin de la préparation des textes : rien ne part.
    let avant = fab.lecteur.appels.lock().unwrap().len();
    app.settings.generation_son.lecture = Mode::Local;
    app.lire_storyboard(Portee::Film);
    app.annuler_suggestions();
    pomper(&mut app);
    assert_eq!(fab.lecteur.appels.lock().unwrap().len(), avant);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn opening_another_project_stops_what_was_running_for_the_old_one() {
    let (mut app, fab, dir) = application("autre-projet", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    fab.son.bloque.store(true, std::sync::atomic::Ordering::Relaxed);
    app.generer_indications(ids(&app));
    app.poll_suggestions(&egui::Context::default());
    assert!(app.sug.occupe());
    app.sug.choisie = Some(3);
    app.sug.confirmation = Some(Action::Lire(Portee::Film));
    app.load_doc(Document::new(64, 64, 24));
    assert!(!app.sug.occupe() && app.sug.taches.is_empty() && app.sug.choisie.is_none() && app.sug.confirmation.is_none());
    assert!(app.doc.sons_proposes.is_empty());
    fab.son.bloque.store(false, std::sync::atomic::Ordering::Relaxed);
    pomper(&mut app);
    assert!(app.doc.sons_proposes.is_empty(), "rien n'est rendu au nouveau projet");
    std::fs::remove_dir_all(&dir).ok();
}

// ==================================================================== la confirmation en ligne

#[test]
fn an_online_reader_asks_for_a_confirmation_once_per_project_and_sends_nothing_before() {
    let mut lecteur = un_son_par_plan();
    lecteur.en_ligne = true;
    let (mut app, fab, dir) = application("confirmation", lecteur);
    app.settings.generation_son.lecture = Mode::EnLigne;
    assert!(!app.doc.sons_proposes.envoi_en_ligne_confirme);
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert_eq!(app.sug.confirmation, Some(Action::Lire(Portee::Film)), "la fenêtre attend");
    assert!(!app.sug.lecture_en_cours());
    assert!(fab.lecteur.appels.lock().unwrap().is_empty(), "rien n'est parti");
    assert!(ids(&app).is_empty());
    assert!(app.pane_shown(Pane::SuggestionsSonores), "le panneau s'ouvre sur la demande");
    // Refuser : rien ne part.
    app.refuser_envoi_en_ligne();
    pomper(&mut app);
    assert!(app.sug.confirmation.is_none() && fab.lecteur.appels.lock().unwrap().is_empty());
    assert!(!app.doc.sons_proposes.envoi_en_ligne_confirme);
    assert_eq!(app.status, "Rien n'a été envoyé.");
    // Confirmer : la lecture reprend là où elle attendait, et le choix est retenu pour le projet.
    app.lire_storyboard(Portee::Film);
    app.confirmer_envoi_en_ligne();
    pomper(&mut app);
    assert!(app.doc.sons_proposes.envoi_en_ligne_confirme);
    assert_eq!(fab.lecteur.appels.lock().unwrap().len(), 3);
    assert_eq!(ids(&app).len(), 3);
    // Pas une deuxième fois.
    app.lire_storyboard(Portee::Film);
    assert!(app.sug.confirmation.is_none());
    pomper(&mut app);
    assert_eq!(fab.lecteur.appels.lock().unwrap().len(), 6);
    // La confirmation est dans le fichier du projet, pas dans les réglages de l'application.
    let texte = String::from_utf8(crate::indications::to_bytes(&app.doc).unwrap().unwrap()).unwrap();
    assert!(texte.contains("\"envoi_en_ligne_confirme\": true"), "{texte}");
    // Un autre projet ne l'a pas.
    app.load_doc(Document::new(64, 64, 24));
    assert!(!app.doc.sons_proposes.envoi_en_ligne_confirme);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn local_and_rule_providers_never_ask() {
    let (mut app, fab, dir) = application("sans-confirmation", un_son_par_plan());
    for mode in [Mode::Regles, Mode::Local] {
        app.settings.generation_son.lecture = mode;
        app.lire_storyboard(Portee::Film);
        assert!(app.sug.confirmation.is_none(), "{mode:?}");
        pomper(&mut app);
    }
    assert_eq!(fab.lecteur.appels.lock().unwrap().len(), 6);
    let id = ids(&app)[0];
    app.generer_indications(vec![id]);
    assert!(app.sug.confirmation.is_none());
    pomper(&mut app);
    assert!(!app.doc.sons_proposes.envoi_en_ligne_confirme);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_online_sound_or_voice_provider_asks_too_but_only_for_what_it_would_send() {
    let (mut app, fab, dir) = application("confirmation-sons", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let plan0 = app.doc.panels[0].id;
    let voix = app.doc.sons_proposes.ajouter(plan0, IndicationBrute { start: 0, frames: 24, kind: Some(Genre::Voix), description: "Voix".into(), text: "Bonjour.".into(), ..Default::default() });
    let bruit = ids(&app).into_iter().find(|&i| app.doc.sons_proposes.get(i).unwrap().kind == Genre::Bruitage).unwrap();
    // Les sons en ligne, les voix en local : un bruitage demande, une voix non.
    app.settings.generation_son.son = Mode::EnLigne;
    app.generer_indications(vec![voix]);
    assert!(app.sug.confirmation.is_none(), "la voix est locale");
    pomper(&mut app);
    assert_eq!(fab.voix.appels.lock().unwrap().len(), 1);
    app.generer_indications(vec![bruit]);
    assert_eq!(app.sug.confirmation, Some(Action::Generer(vec![bruit])));
    assert_eq!(app.doc.sons_proposes.get(bruit).unwrap().status, Statut::Proposee, "la proposition n'a pas bougé");
    assert!(fab.son.appels.lock().unwrap().is_empty());
    app.confirmer_envoi_en_ligne();
    pomper(&mut app);
    assert_eq!(app.doc.sons_proposes.get(bruit).unwrap().status, Statut::Prete);
    // Les voix en ligne : même demande, pour les voix cette fois (projet neuf, donc pas encore confirmé).
    app.doc.sons_proposes.envoi_en_ligne_confirme = false;
    app.settings.generation_son.voix = Mode::EnLigne;
    app.generer_indications(vec![voix]);
    assert_eq!(app.sug.confirmation, Some(Action::Generer(vec![voix])));
    app.refuser_envoi_en_ligne();
    assert_eq!(fab.voix.appels.lock().unwrap().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_panel_says_what_goes_online_whatever_the_confirmation() {
    let (mut app, _f, dir) = application("avertissement", un_son_par_plan());
    app.settings.generation_son.lecture = Mode::EnLigne;
    app.settings.generation_son.vignettes = true;
    app.doc.sons_proposes.envoi_en_ligne_confirme = true;
    let ctx = egui::Context::default();
    let sortie = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.suggestions_panel(ui));
    });
    let textes: Vec<String> = sortie.shapes.iter().filter_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
        _ => None,
    }).collect();
    let tout = textes.join("\n");
    assert!(tout.contains("vignettes des plans et leurs textes seront envoyés à Claude"), "{tout}");
    assert!(tout.contains("Envoi en ligne confirmé pour ce projet"), "{tout}");
    std::fs::remove_dir_all(&dir).ok();
}

// =========================================================================== clés et connexion

#[test]
fn a_key_goes_to_the_vault_and_leaves_no_trace_anywhere_else() {
    let (mut app, _f, dir) = application("cles", un_son_par_plan());
    const SECRET: &str = "sk-ant-SECRET-0123456789";
    app.sug.prefs.cle_pour = Some(Service::Claude);
    app.sug.prefs.saisie = format!("  {SECRET}  ");
    assert!(app.valider_cle(Service::Claude));
    assert_eq!(app.sug.coffre.lire(Service::Claude).unwrap().unwrap().voir(), SECRET);
    assert!(app.sug.prefs.saisie.is_empty() && app.sug.prefs.cle_pour.is_none(), "ce qui était tapé est effacé");
    assert!(!app.status.contains("SECRET"), "{}", app.status);
    // Ni les réglages, ni le projet, ni aucun fichier de l'essai ne contiennent la clé.
    app.settings.generation_son.lecture = Mode::EnLigne;
    app.settings.save().unwrap();
    app.doc.sons_proposes.envoi_en_ligne_confirme = true;
    let manifeste = dir.join("Projet").join("Film.pelure");
    crate::project::save(&app.doc, &manifeste, &mut Default::default()).unwrap();
    fn parcourir(d: &std::path::Path, trouves: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                parcourir(&p, trouves);
            } else if std::fs::read(&p).is_ok_and(|b| b.windows(6).any(|w| w == b"SECRET")) {
                trouves.push(p);
            }
        }
    }
    let mut trouves = Vec::new();
    parcourir(&dir, &mut trouves);
    assert!(trouves.is_empty(), "la clé est écrite dans {trouves:?}");
    let reglages = std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap();
    assert!(reglages.contains("[generation_son]") && reglages.contains("lecture = \"en_ligne\""));
    // Une saisie vide n'est pas une clé ; effacer la retire.
    app.sug.prefs.saisie = "   ".into();
    assert!(!app.valider_cle(Service::ElevenLabs));
    assert_eq!(app.sug.prefs.message_cle, "La clé est vide.");
    assert!(!app.sug.coffre.presente(Service::ElevenLabs));
    assert!(app.effacer_cle(Service::Claude));
    assert!(!app.sug.coffre.presente(Service::Claude));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_vault_that_refuses_is_reported_without_closing_the_window() {
    struct Ferme;
    impl Coffre for Ferme {
        fn lire(&self, _: Service) -> Result<Option<Cle>, String> {
            Err("verrouillé".into())
        }
        fn ecrire(&self, _: Service, _: &Cle) -> Result<(), String> {
            Err("le Gestionnaire d'identifiants refuse".into())
        }
        fn effacer(&self, _: Service) -> Result<(), String> {
            Err("verrouillé".into())
        }
    }
    let (mut app, _f, dir) = application("coffre-ferme", un_son_par_plan());
    app.sug.coffre = Arc::new(Ferme);
    app.sug.prefs.cle_pour = Some(Service::Claude);
    app.sug.prefs.saisie = "abc".into();
    assert!(!app.valider_cle(Service::Claude));
    assert_eq!(app.sug.prefs.message_cle, "le Gestionnaire d'identifiants refuse");
    assert_eq!(app.sug.prefs.cle_pour, Some(Service::Claude), "la fenêtre reste ouverte");
    assert!(!app.effacer_cle(Service::Claude));
    std::fs::remove_dir_all(&dir).ok();
}

fn attendre_essai(app: &mut App) {
    let ctx = egui::Context::default();
    let t = Instant::now();
    while app.sug.prefs.essai_en_cours() {
        app.suivre_essai(&ctx);
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn the_test_buttons_report_not_installed_and_a_connection_to_plug_in_without_blocking() {
    let (mut app, _f, dir) = application("essais", un_son_par_plan());
    // Le serveur local : rien n'écoute sur ce port.
    let libre = crate::serveurlocal_tests::libre();
    app.settings.generation_son.port_local = libre;
    app.lancer_essai(None);
    assert_eq!(app.sug.prefs.resultats[&None], "Essai en cours…", "l'essai ne bloque pas : il rend la main aussitôt");
    attendre_essai(&mut app);
    let texte = &app.sug.prefs.resultats[&None];
    assert!(texte.starts_with("Le serveur local ne tourne pas") && texte.contains(&libre.to_string()), "{texte}");
    // Le service en ligne : sans clé, puis avec une clé mais sans transport.
    app.lancer_essai(Some(Role::Lecture));
    attendre_essai(&mut app);
    assert!(app.sug.prefs.resultats[&Some(Role::Lecture)].contains("aucune clé"), "{:?}", app.sug.prefs.resultats);
    app.sug.coffre.ecrire(Service::Claude, &Cle::new("sk-essai")).unwrap();
    app.lancer_essai(Some(Role::Lecture));
    attendre_essai(&mut app);
    let texte = &app.sug.prefs.resultats[&Some(Role::Lecture)];
    assert!(texte.starts_with("Clé enregistrée pour Claude, mais") && texte.contains("TLS"), "{texte}");
    assert!(!texte.contains("sk-essai"));
    std::fs::remove_dir_all(&dir).ok();
}

// ======================================================================= le panneau, l'icône, les commandes

#[test]
fn the_pane_is_closed_by_default_opens_on_the_right_and_has_its_icon_and_commands() {
    use crate::dock::{DockLayout, Group};
    use crate::workspace::Layout;
    assert!(Pane::SuggestionsSonores.on_demand());
    assert_eq!(Pane::SuggestionsSonores.group(), Group::Right);
    assert_eq!(serde_json::to_string(&Pane::SuggestionsSonores).unwrap(), "\"suggestions_sonores\"");
    assert_eq!(Pane::ALL.len(), 16);
    assert!(Pane::ALL.contains(&Pane::SuggestionsSonores));
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert!(!l.contains(Pane::SuggestionsSonores), "fermé par défaut");
    l.open(Pane::SuggestionsSonores);
    assert!(l.contains(Pane::SuggestionsSonores));
    assert!(format!("{:?}", l.principal).contains("SuggestionsSonores"), "{:?}", l.principal);
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.suggestions_sonores"), Some(Pane::SuggestionsSonores));
    for id in ["vue.suggestions_sonores", "suggestion.lire"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    assert!(!Pane::SuggestionsSonores.icon().is_empty() && Pane::SuggestionsSonores.name() == "Suggestions sonores");
}

#[test]
fn the_read_command_opens_the_pane_and_reads_the_chosen_scope() {
    let (mut app, _f, dir) = application("commande", un_son_par_plan());
    assert!(!app.pane_shown(Pane::SuggestionsSonores));
    app.run_action("suggestion.lire");
    assert!(app.pane_shown(Pane::SuggestionsSonores));
    pomper(&mut app);
    assert_eq!(ids(&app).len(), 3);
    // Le clic sur l'icône de la barre ouvre et ferme le panneau.
    app.run_action("vue.suggestions_sonores");
    assert!(!app.pane_shown(Pane::SuggestionsSonores));
    app.run_action("vue.suggestions_sonores");
    assert!(app.pane_shown(Pane::SuggestionsSonores));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn proposals_are_listed_in_the_order_of_the_film() {
    let (mut app, _f, dir) = application("ordre", un_son_par_plan());
    let (p0, p1, p2) = (app.doc.panels[0].id, app.doc.panels[1].id, app.doc.panels[2].id);
    let nouveau = |app: &mut App, plan: u64, start: usize, nom: &str| app.doc.sons_proposes.ajouter(plan, IndicationBrute { start, frames: 5, kind: Some(Genre::Bruitage), description: nom.into(), ..Default::default() });
    let c = nouveau(&mut app, p2, 0, "c");
    let b2 = nouveau(&mut app, p1, 9, "b2");
    let a = nouveau(&mut app, p0, 3, "a");
    let b1 = nouveau(&mut app, p1, 2, "b1");
    nouveau(&mut app, 987_654, 0, "orpheline");
    assert_eq!(ids(&app), vec![a, b1, b2, c]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_panel_and_its_windows_draw_every_state_without_failing() {
    let (mut app, _f, dir) = application("dessin", un_son_par_plan());
    let plan = app.doc.panels[0].id;
    let statuts = [Statut::Proposee, Statut::EnGeneration, Statut::Prete, Statut::Acceptee, Statut::Rejetee];
    for (k, s) in statuts.into_iter().enumerate() {
        let genre = Genre::ALL[k % 4];
        let id = app.doc.sons_proposes.ajouter(plan, IndicationBrute { start: k, frames: 10, kind: Some(genre), description: format!("Proposition {k} avec une description assez longue pour être coupée dans la liste du panneau"), text: if genre.est_voix() { "Une réplique.".into() } else { String::new() }, ..Default::default() });
        let i = app.doc.sons_proposes.get_mut(id).unwrap();
        i.status = s;
        if matches!(s, Statut::Prete | Statut::Acceptee) {
            i.file = Some(dir.join("x.wav"));
        }
        if k == 2 {
            app.sug.choisie = Some(id);
        }
    }
    app.sug.confirmation = Some(Action::Lire(Portee::Film));
    app.settings.generation_son.lecture = Mode::EnLigne;
    app.sug.edition = Some(Edition { id: ids(&app)[0], kind: Genre::Voix, start: 0, frames: 5, description: "d".into(), prompt: String::new(), text: "t".into(), character: "c".into(), voice: "v".into() });
    app.sug.prefs.cle_pour = Some(Service::ElevenLabs);
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                app.suggestions_panel(ui);
                app.generation_prefs_corps(ui);
            });
            app.suggestions_windows(ctx);
        });
    }
    // Les trois rôles en ligne dessinent aussi leurs clés et leurs essais.
    for role in Role::ALL {
        app.settings.generation_son.definir(role, Mode::EnLigne);
    }
    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.generation_prefs_corps(ui));
    });
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_provider_error_is_told_in_the_status_bar_and_the_cue_keeps_its_state() {
    let (mut app, fab, dir) = application("erreur", un_son_par_plan());
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    let id = ids(&app)[0];
    *fab.son.fatale.lock().unwrap() = Some(ErreurGen::NonInstalle("aucun serveur local n'écoute sur 127.0.0.1:8765".into()));
    app.generer_indications(vec![id]);
    pomper(&mut app);
    assert!(app.status.contains("non installé") && app.status.contains("127.0.0.1:8765"), "{}", app.status);
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Proposee);
    // Une lecture dont le serveur n'est pas là : même message, rien n'est ajouté.
    *fab.lecteur.echec.lock().unwrap() = Some(ErreurGen::NonInstalle("aucun serveur local".into()));
    app.lire_storyboard(Portee::Film);
    pomper(&mut app);
    assert!(app.status.contains("non installé"), "{}", app.status);
    assert_eq!(ids(&app).len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

// ================================================================ de bout en bout avec le VRAI serveur
//
// Ces deux essais lancent le vrai serveur Python (sidecar/serveur.py) avec le venv de SHOT-modeles : ils
// sont donc ignorés par `cargo test` et se lancent à la main :
//   cargo test --release --offline --target-dir target\msvc --lib sidecar_reel -- --ignored --nocapture --test-threads=1

/// Comme `pomper`, avec le temps d'un vrai modèle.
fn pomper_longtemps(app: &mut App, secondes: u64) {
    let ctx = egui::Context::default();
    let t = Instant::now();
    loop {
        app.poll_suggestions(&ctx);
        if !app.sug.occupe() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(secondes), "les suggestions ne finissent pas : {}", app.status);
        std::thread::sleep(Duration::from_millis(5));
    }
    app.poll_suggestions(&ctx);
}

fn application_avec_le_vrai_serveur(nom: &str) -> (App, PathBuf) {
    let (mut app, _f, dir) = application(nom, un_son_par_plan());
    app.sug.serveur = crate::serveurlocal::GestionnaireServeur::reel();
    app.sug.fabrique = Arc::new(FabriqueReelle { transport: Arc::new(TransportABrancher), coffre: Arc::new(CoffreMemoire::default()), serveur: app.sug.serveur.clone() });
    // Un port à soi : le serveur de l'utilisateur, s'il tourne, n'est pas dérangé.
    app.settings.generation_son.port_local = crate::serveurlocal_tests::libre();
    app.settings.generation_son.demarrage_auto = true;
    (app, dir)
}

#[test]
#[ignore = "lance le vrai serveur Python et Kokoro (SHOT-modeles requis)"]
fn sidecar_reel_a_voice_cue_is_generated_by_kokoro_listened_to_and_accepted() {
    let (mut app, dir) = application_avec_le_vrai_serveur("sidecar-reel-voix");
    app.doc.panels[0].dialogue = "MARIE : Viens ici, il pleut sur la ville.".into();
    app.lire_storyboard(Portee::Plans(vec![app.doc.panels[0].id]));
    pomper(&mut app);
    let voix: Vec<u64> = ids(&app).into_iter().filter(|&i| app.doc.sons_proposes.get(i).unwrap().kind.est_voix()).collect();
    assert_eq!(voix.len(), 1, "{:#?}", app.doc.sons_proposes.indications);
    let id = voix[0];
    let t = Instant::now();
    app.generer_indications(vec![id]);
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::EnGeneration);
    pomper_longtemps(&mut app, 240);
    println!("voix : premier appel (le serveur démarre, Kokoro se charge) en {:.1} s", t.elapsed().as_secs_f32());
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Prete, "{}", app.status);
    let fichier = app.doc.sons_proposes.get(id).unwrap().file.clone().unwrap();
    let octets = std::fs::read(&fichier).unwrap();
    let pcm = crate::audio::decode(&octets, "wav").expect("un WAV lisible");
    println!("voix : {} octets, {:?}", octets.len(), fichier.file_name().unwrap());
    assert!(pcm.frames() > 1000);
    assert!(app.sug.serveur.en_marche(), "SHOT a lancé le serveur");
    // Une deuxième réplique : le serveur est chaud.
    let deux = app.doc.sons_proposes.ajouter(app.doc.panels[1].id, IndicationBrute { start: 0, frames: 48, kind: Some(Genre::Voix), description: "x".into(), text: "Il pleut toujours.".into(), character: "PAUL".into(), ..Default::default() });
    let t = Instant::now();
    app.generer_indications(vec![deux]);
    pomper_longtemps(&mut app, 60);
    println!("voix : deuxième appel en {:.2} s", t.elapsed().as_secs_f32());
    assert_eq!(app.doc.sons_proposes.get(deux).unwrap().status, Statut::Prete, "{}", app.status);
    // Accepter : le son se pose sur la piste « Sons proposés ».
    app.accepter_indication(id);
    pomper(&mut app);
    assert_eq!(app.doc.audio_tracks[0].name, PISTE);
    assert_eq!(app.doc.sons_proposes.get(id).unwrap().status, Statut::Acceptee);
    // Les bruitages : Stable Audio n'est pas téléchargé, le message le dit.
    let bruit = app.doc.sons_proposes.ajouter(app.doc.panels[2].id, IndicationBrute { start: 0, frames: 24, kind: Some(Genre::Bruitage), description: "Porte".into(), prompt: "door slam".into(), ..Default::default() });
    app.generer_indications(vec![bruit]);
    pomper_longtemps(&mut app, 60);
    println!("bruitage : {}", app.status);
    assert_eq!(app.doc.sons_proposes.get(bruit).unwrap().status, Statut::Proposee);
    assert!(app.status.contains("Stable Audio"), "{}", app.status);
    // SHOT se ferme : le serveur s'arrête.
    app.fermeture();
    assert!(!app.sug.serveur.en_marche());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[ignore = "lance le vrai serveur Python et Qwen2.5-VL (SHOT-modeles requis, ~5 Go de mémoire graphique)"]
fn sidecar_reel_the_storyboard_is_read_by_qwen_with_its_thumbnail() {
    let (mut app, dir) = application_avec_le_vrai_serveur("sidecar-reel-qwen");
    app.settings.generation_son.lecture = Mode::Local;
    app.doc.panels[0].action = "Une jeune femme referme la porte d'entrée en claquant. Il pleut dehors.".into();
    app.doc.panels[0].dialogue = "MARIE : Viens ici.".into();
    app.doc.panels[0].notes = "SFX : pas dans l'escalier".into();
    let t = Instant::now();
    app.lire_storyboard(Portee::Plans(vec![app.doc.panels[0].id]));
    pomper_longtemps(&mut app, 300);
    println!("lecture Qwen (démarrage du serveur + chargement du modèle + un plan) : {:.1} s", t.elapsed().as_secs_f32());
    println!("{}", app.status);
    for i in &app.doc.sons_proposes.indications {
        println!("  {:?} image {} : {} | {} | {}", i.kind, i.start, i.description, i.prompt, i.text);
    }
    assert!(app.doc.sons_proposes.indications.iter().any(|i| i.kind.est_voix() && i.text.contains("Viens ici")), "la voix vient du dialogue");
    assert!(app.doc.sons_proposes.indications.iter().any(|i| !i.kind.est_voix()), "Qwen propose au moins un son");
    app.fermeture();
    std::fs::remove_dir_all(&dir).ok();
}