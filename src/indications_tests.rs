//! Tests des sons proposés : le modèle d'une proposition, son fichier `sons_proposes.json` (aller-retour,
//! protection d'un fichier illisible ou plus récent), la fusion d'une relecture, l'état « accepté »
//! qui suit le son posé, la piste « Sons proposés ».

use std::path::PathBuf;

use crate::document::Document;
use crate::indications::{self, debut_montage, normaliser, parse, piste_pour, to_bytes, Genre, IndicationBrute, SonsProposes, Statut, FILE, PISTE};
use crate::project;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-sons-proposes-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Trois plans de 10 images à 24 i/s.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for p in doc.panels.iter_mut() {
        p.set_length(10);
    }
    doc
}

fn brute(kind: Genre, description: &str) -> IndicationBrute {
    IndicationBrute { start: 2, frames: 24, kind: Some(kind), description: description.into(), prompt: format!("prompt de {description}"), ..Default::default() }
}

fn voix(texte: &str) -> IndicationBrute {
    IndicationBrute { start: 0, frames: 30, kind: Some(Genre::Voix), description: format!("Voix : {texte}"), text: texte.into(), character: "MARIE".into(), ..Default::default() }
}

// ------------------------------------------------------------------------------------ texte

#[test]
fn normalising_removes_accents_case_and_punctuation() {
    assert_eq!(normaliser("  Porte QUI claque !! "), "porte qui claque");
    assert_eq!(normaliser("L'été, à Noël — cœur d'Œdipe"), "l ete a noel coeur d oedipe");
    assert_eq!(normaliser(""), "");
    assert_eq!(normaliser("…?"), "");
}

// ------------------------------------------------------------------------------- le fichier

#[test]
fn the_sidecar_round_trips_in_film_order_and_keeps_the_online_confirmation() {
    let mut doc = film();
    let (p0, p1, p2) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    // Ajoutées dans le désordre : le fichier les range dans l'ordre du film.
    let c = doc.sons_proposes.ajouter(p2, brute(Genre::Ambiance, "Pluie"));
    let a = doc.sons_proposes.ajouter(p0, brute(Genre::Bruitage, "Porte qui claque"));
    let b = doc.sons_proposes.ajouter(p1, voix("Viens ici."));
    {
        let i = doc.sons_proposes.get_mut(b).unwrap();
        i.voice = "ff_siwis".into();
        i.status = Statut::Prete;
        i.file = Some(PathBuf::from("C:/cache/son.wav"));
        i.edited = true;
    }
    doc.sons_proposes.get_mut(a).unwrap().status = Statut::Rejetee;
    doc.sons_proposes.envoi_en_ligne_confirme = true;
    let bytes = to_bytes(&doc).unwrap().expect("il y a des propositions");
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.find("Porte qui claque").unwrap() < text.find("Viens ici").unwrap() && text.find("Viens ici").unwrap() < text.find("Pluie").unwrap(), "{text}");
    // Pas de champ vide dans le fichier.
    assert!(!text.contains("\"personnage\": \"\""), "{text}");
    let back = parse(&text).unwrap();
    assert!(back.envoi_en_ligne_confirme);
    assert_eq!(back.indications.len(), 3);
    for id in [a, b, c] {
        assert_eq!(back.get(id), doc.sons_proposes.get(id), "la proposition {id} est identique");
    }
    assert_eq!(back.get(b).unwrap().file.as_deref(), Some(std::path::Path::new("C:/cache/son.wav")));
}

#[test]
fn nothing_is_written_without_cues_and_without_confirmation() {
    let mut doc = film();
    assert_eq!(to_bytes(&doc).unwrap(), None, "pas de fichier pour un projet sans proposition");
    doc.sons_proposes.envoi_en_ligne_confirme = true;
    let bytes = to_bytes(&doc).unwrap().expect("la confirmation se garde");
    let back = parse(&String::from_utf8(bytes).unwrap()).unwrap();
    assert!(back.envoi_en_ligne_confirme && back.is_empty());
}

#[test]
fn a_cue_of_a_deleted_plan_is_not_written() {
    let mut doc = film();
    let keep = doc.panels[0].id;
    doc.sons_proposes.ajouter(keep, brute(Genre::Bruitage, "Gardée"));
    doc.sons_proposes.ajouter(987_654, brute(Genre::Bruitage, "Orpheline"));
    let text = String::from_utf8(to_bytes(&doc).unwrap().unwrap()).unwrap();
    assert!(text.contains("Gardée") && !text.contains("Orpheline"), "{text}");
}

#[test]
fn a_damaged_or_newer_file_is_refused_with_a_reason() {
    let ok = |extra: &str| format!("{{\"format\": \"shot-sons-proposes\", \"version\": 1{extra}}}");
    assert!(parse(&ok("")).is_ok());
    assert!(parse("pas du json").unwrap_err().contains("illisible"));
    assert!(parse("{\"format\": \"autre\", \"version\": 1}").unwrap_err().contains("pas un fichier de sons proposés"));
    let neuf = "{\"format\": \"shot-sons-proposes\", \"version\": 2, \"champ_futur\": true}";
    assert!(parse(neuf).unwrap_err().contains("plus récente"));
    // Un fichier tronqué.
    let mut doc = film();
    let p = doc.panels[0].id;
    doc.sons_proposes.ajouter(p, brute(Genre::Bruitage, "Coupée"));
    let bytes = to_bytes(&doc).unwrap().unwrap();
    assert!(parse(&String::from_utf8(bytes[..bytes.len() / 2].to_vec()).unwrap()).is_err());
    // Un octet d'ordre (BOM) en tête se lit.
    assert!(parse(&format!("\u{feff}{}", ok(""))).is_ok());
}

#[test]
fn reading_bounds_the_values_and_settles_stale_states() {
    let json = r#"{
        "format": "shot-sons-proposes", "version": 1,
        "indications": [
            {"id": 7, "plan": 1, "image": 0, "images": 0, "genre": "bruitage", "description": "a", "statut": "en_generation"},
            {"id": 7, "plan": 1, "image": 99999999999, "images": 99999999, "genre": "voix", "texte": "b", "statut": "en_generation", "fichier": "C:/cache/b.wav"},
            {"id": 0, "plan": 1, "image": 1, "images": 5, "genre": "ambiance", "statut": "prete"}
        ]
    }"#;
    let s = parse(json).unwrap();
    assert_eq!(s.indications.len(), 3);
    let ids: std::collections::HashSet<u64> = s.indications.iter().map(|i| i.id).collect();
    assert_eq!(ids.len(), 3, "les numéros doublons ou nuls en reçoivent de neufs : {ids:?}");
    assert!(!ids.contains(&0));
    assert_eq!(s.indications[0].frames, 1, "une durée nulle devient 1 image");
    assert_eq!(s.indications[0].status, Statut::Proposee, "une génération restée en cours sans fichier : à refaire");
    assert_eq!(s.indications[1].frames, indications::IMAGES_MAX);
    assert_eq!(s.indications[1].status, Statut::Prete, "avec son fichier, elle est prête");
    assert!(s.indications[1].start <= 10_000_000);
    assert_eq!(s.indications[2].status, Statut::Proposee, "« prête » sans fichier n'a rien à écouter");
    // Les textes trop longs sont coupés.
    let long = "x".repeat(50_000);
    let s = parse(&format!("{{\"format\": \"shot-sons-proposes\", \"version\": 1, \"indications\": [{{\"id\": 1, \"plan\": 1, \"image\": 0, \"images\": 5, \"genre\": \"voix\", \"texte\": \"{long}\"}}]}}")).unwrap();
    assert_eq!(s.indications[0].text.chars().count(), 4_000);
}

#[test]
fn a_ready_sound_whose_file_is_gone_is_proposed_again() {
    let dir = temp("fichiers");
    let la = dir.join("la.wav");
    std::fs::write(&la, b"x").unwrap();
    let mut s = SonsProposes::default();
    let a = s.ajouter(1, brute(Genre::Bruitage, "Présent"));
    let b = s.ajouter(1, brute(Genre::Bruitage, "Absent"));
    for (id, f) in [(a, la.clone()), (b, dir.join("absent.wav"))] {
        let i = s.get_mut(id).unwrap();
        i.status = Statut::Prete;
        i.file = Some(f);
    }
    indications::verifier_fichiers(&mut s);
    assert_eq!((s.get(a).unwrap().status, s.get(a).unwrap().file.is_some()), (Statut::Prete, true));
    assert_eq!((s.get(b).unwrap().status, s.get(b).unwrap().file.is_some()), (Statut::Proposee, false));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ fusion

#[test]
fn a_new_reading_adds_new_cues_keeps_known_ones_and_drops_untouched_stale_ones() {
    let mut s = SonsProposes::default();
    let f = s.fusionner(5, vec![brute(Genre::Bruitage, "Porte qui claque"), brute(Genre::Ambiance, "Pluie"), voix("Viens ici.")]);
    assert_eq!((f.ajoutees, f.retirees, f.connues), (3, 0, 0));
    let porte = s.indications[0].id;
    let pluie = s.indications[1].id;
    let replique = s.indications[2].id;
    // Le réalisateur rejette la porte, génère la pluie, et modifie la réplique.
    s.get_mut(porte).unwrap().status = Statut::Rejetee;
    {
        let i = s.get_mut(pluie).unwrap();
        i.status = Statut::Prete;
        i.file = Some(PathBuf::from("C:/cache/pluie.wav"));
    }
    {
        let i = s.get_mut(replique).unwrap();
        i.text = "Viens vite.".into();
        i.edited = true;
    }
    // Le texte du plan change : la porte est toujours là (plus la pluie), un vent arrive.
    let f = s.fusionner(5, vec![brute(Genre::Bruitage, "porte QUI claque"), brute(Genre::Ambiance, "Vent")]);
    assert_eq!(f.connues, 1, "la porte rejetée est reconnue (casse ignorée) et ne revient pas");
    assert_eq!(f.ajoutees, 1, "le vent est neuf");
    assert_eq!(f.retirees, 0, "la pluie est prête, la réplique a été modifiée : on n'y touche pas");
    assert_eq!(s.indications.len(), 4);
    assert_eq!(s.get(porte).unwrap().status, Statut::Rejetee);
    // Une proposition intacte que la lecture ne refait plus est retirée.
    let f = s.fusionner(5, vec![brute(Genre::Bruitage, "Porte qui claque")]);
    assert_eq!(f.retirees, 1, "le vent, jamais touché, n'est plus demandé");
    assert!(s.indications.iter().all(|i| i.kind != Genre::Ambiance || i.id == pluie));
    // Les autres plans ne sont pas touchés.
    s.fusionner(6, vec![brute(Genre::Bruitage, "Klaxon")]);
    s.fusionner(5, vec![]);
    assert_eq!(s.du_plan(6).count(), 1);
    // Une voix se reconnaît à sa réplique, pas à sa description.
    let mut t = SonsProposes::default();
    t.fusionner(1, vec![voix("Salut.")]);
    let mut autre = voix("Salut.");
    autre.description = "Autre libellé".into();
    assert_eq!(t.fusionner(1, vec![autre]).connues, 1);
}

// --------------------------------------------------------------------------- état et pistes

#[test]
fn the_real_status_follows_the_sound_placed_on_the_timeline() {
    let mut doc = film();
    let plan = doc.panels[0].id;
    let id = doc.sons_proposes.ajouter(plan, brute(Genre::Bruitage, "Coup"));
    let sound = doc.add_sound("coup.wav".into(), "wav".into(), std::sync::Arc::new(vec![]), std::sync::Arc::new(crate::audio::Pcm::from_interleaved(&[0.0; 4800], 1, crate::audio::RATE)));
    let clip = doc.place_sound(sound, 2, None).unwrap();
    {
        let i = doc.sons_proposes.get_mut(id).unwrap();
        i.file = Some(PathBuf::from("C:/cache/coup.wav"));
        i.clip = Some(clip);
        i.status = Statut::Acceptee;
    }
    let reel = |doc: &Document| doc.sons_proposes.get(id).unwrap().statut_reel(doc);
    assert_eq!(reel(&doc), Statut::Acceptee);
    // L'extrait disparaît (annulation) : la proposition est de nouveau prête, son fichier est là.
    doc.remove_clip(clip);
    assert_eq!(reel(&doc), Statut::Prete);
    // Sans fichier non plus : à générer.
    doc.sons_proposes.get_mut(id).unwrap().file = None;
    assert_eq!(reel(&doc), Statut::Proposee);
    // Le retour de l'extrait (rétablir) la fait de nouveau acceptée.
    doc.sons_proposes.get_mut(id).unwrap().status = Statut::Prete;
    doc.sons_proposes.get_mut(id).unwrap().file = Some(PathBuf::from("C:/cache/coup.wav"));
    doc.audio_tracks.clear();
    doc.add_audio_track();
    doc.audio_tracks[0].clips.push(crate::document::SoundClip::new(clip, sound, 2, 0, 5));
    assert_eq!(reel(&doc), Statut::Acceptee);
}

#[test]
fn the_start_of_a_cue_in_the_film_is_its_plan_start_plus_its_frame_clamped_to_the_plan() {
    let mut doc = film();
    let plan1 = doc.panels[1].id;
    let id = doc.sons_proposes.ajouter(plan1, IndicationBrute { start: 4, frames: 5, kind: Some(Genre::Bruitage), ..Default::default() });
    let i = doc.sons_proposes.get(id).unwrap().clone();
    assert_eq!(debut_montage(&doc, &i), Some(doc.panel_start(1) + 4));
    let mut loin = i.clone();
    loin.start = 500;
    assert_eq!(debut_montage(&doc, &loin), Some(doc.panel_start(1) + doc.panels[1].frame_count() - 1), "sur la dernière image du plan");
    let mut orphelin = i;
    orphelin.plan = 123_456;
    assert_eq!(debut_montage(&doc, &orphelin), None);
}

#[test]
fn the_accept_track_is_found_or_created_and_never_a_locked_or_busy_one() {
    let mut doc = film();
    doc.add_audio_track();
    doc.audio_tracks[0].name = "Dialogues".into();
    // Aucune piste « Sons proposés » : une neuve, la dernière, qui porte ce nom.
    let t = piste_pour(&mut doc, 0, 10);
    assert_eq!((t, doc.audio_tracks.len(), doc.audio_tracks[t].name.as_str()), (1, 2, PISTE), "la piste des dialogues n'est pas prise");
    // La même piste sert tant que la place est libre.
    assert_eq!(piste_pour(&mut doc, 20, 30), 1);
    assert_eq!(doc.audio_tracks.len(), 2);
    // Occupée à cet endroit : une deuxième piste, numérotée.
    let sound = doc.add_sound("x.wav".into(), "wav".into(), std::sync::Arc::new(vec![]), std::sync::Arc::new(crate::audio::Pcm::from_interleaved(&[0.0; 480], 1, crate::audio::RATE)));
    doc.add_clip(1, sound, 5, 0, 10).unwrap();
    let t = piste_pour(&mut doc, 8, 12);
    assert_eq!((t, doc.audio_tracks[t].name.as_str()), (2, "Sons proposés 2"));
    // Une piste verrouillée n'est jamais visée.
    doc.audio_tracks[1].locked = true;
    doc.audio_tracks[2].locked = true;
    let t = piste_pour(&mut doc, 40, 50);
    assert_eq!((t, doc.audio_tracks[t].name.as_str()), (3, "Sons proposés 3"));
    // Les pistes de l'utilisateur sont intactes.
    assert_eq!(doc.audio_tracks[0].name, "Dialogues");
}

// ------------------------------------------------------------------------- sur le disque

fn enregistrer(doc: &Document, dir: &std::path::Path) -> PathBuf {
    let manifeste = dir.join("Film.pelure");
    project::save(doc, &manifeste, &mut Default::default()).expect("enregistrement");
    manifeste
}

#[test]
fn the_project_saves_and_reopens_its_proposed_sounds_and_removes_the_file_when_none_is_left() {
    let dir = temp("projet");
    let mut doc = film();
    let p = doc.panels[1].id;
    let a = doc.sons_proposes.ajouter(p, brute(Genre::Ambiance, "Pluie"));
    let b = doc.sons_proposes.ajouter(p, voix("Salut."));
    doc.sons_proposes.get_mut(b).unwrap().status = Statut::Rejetee;
    doc.sons_proposes.envoi_en_ligne_confirme = true;
    let manifeste = enregistrer(&doc, &dir);
    assert!(dir.join(FILE).is_file());
    let l = project::load(&manifeste).unwrap();
    assert!(l.warnings.iter().all(|w| !w.contains(FILE)), "{:?}", l.warnings);
    assert_eq!(l.doc.sons_proposes.indications.len(), 2);
    assert_eq!(l.doc.sons_proposes.get(a), doc.sons_proposes.get(a));
    assert_eq!(l.doc.sons_proposes.get(b).unwrap().status, Statut::Rejetee);
    assert!(l.doc.sons_proposes.envoi_en_ligne_confirme);
    // Sans proposition ni confirmation, le fichier part.
    let mut vide = l.doc;
    vide.sons_proposes = SonsProposes::default();
    enregistrer(&vide, &dir);
    assert!(!dir.join(FILE).exists());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_newer_or_damaged_sidecar_is_reported_and_never_deleted_by_a_save() {
    let dir = temp("protection");
    let mut doc = film();
    let manifeste = enregistrer(&doc, &dir);
    // Écrit par une version plus récente.
    let neuf = "{\"format\": \"shot-sons-proposes\", \"version\": 2, \"champ_futur\": \"à garder\", \"indications\": []}";
    std::fs::write(dir.join(FILE), neuf).unwrap();
    let mut l = project::load(&manifeste).unwrap();
    assert!(l.warnings.iter().any(|w| w.contains(FILE) && w.contains("plus récente")), "{:?}", l.warnings);
    assert!(l.doc.sons_proposes.is_empty());
    // Enregistrer sans proposition : le fichier d'une version plus récente reste, intact.
    project::save(&l.doc, &manifeste, &mut l.written).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join(FILE)).unwrap(), neuf);
    // Avec une proposition neuve : le fichier neuf est écrit, l'ancien est gardé à côté.
    let p = l.doc.panels[0].id;
    l.doc.sons_proposes.ajouter(p, brute(Genre::Bruitage, "Neuve"));
    project::save(&l.doc, &manifeste, &mut l.written).unwrap();
    let garde = dir.join(format!("{FILE}.illisible"));
    assert_eq!(std::fs::read_to_string(&garde).unwrap(), neuf, "l'ancien fichier est mis de côté");
    let relu = project::load(&manifeste).unwrap();
    assert!(relu.warnings.iter().all(|w| !w.contains(FILE)), "{:?}", relu.warnings);
    assert_eq!(relu.doc.sons_proposes.indications.len(), 1);
    // Un fichier abîmé : même traitement.
    let bon = std::fs::read(dir.join(FILE)).unwrap();
    std::fs::write(dir.join(FILE), &bon[..bon.len() / 2]).unwrap();
    let mut l = project::load(&manifeste).unwrap();
    assert!(l.warnings.iter().any(|w| w.contains(FILE)), "{:?}", l.warnings);
    l.doc.sons_proposes = SonsProposes::default();
    project::save(&l.doc, &manifeste, &mut l.written).unwrap();
    assert_eq!(std::fs::read(dir.join(FILE)).unwrap(), bon[..bon.len() / 2], "un fichier abîmé n'est jamais effacé");
    // Plus de .tmp.
    assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
    doc.sons_proposes = SonsProposes::default();
    std::fs::remove_dir_all(&dir).ok();
}
