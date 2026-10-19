//! Tests de la bibliothèque de bruitages : parcours des dossiers (sur un fil à part), recherche, durées
//! lues à la demande, écoute (sans aucune sortie audio : un faux la remplace), pose dans le montage
//! (copie dans le projet, annulation), réglages.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::audio::{Pcm, RATE};
use crate::bruitages::{self, duration_text, matches, probe_seconds, scan_dir, Bibliotheque, BruitagesReglages, Ecoute, Entry, Sonde, Sortie};
use crate::dock::Pane;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-test-bruitages-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Un fichier WAV 16 bits mono de `n` échantillons à 48 kHz.
fn wav(n: usize, v: i16) -> Vec<u8> {
    let data = (n * 2) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for _ in 0..n {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// Un dossier de sons :
/// ```text
/// racine/Claquement.wav   Bam.wav   notes.txt   .cache/Cache.wav
/// racine/Portes/Grincement.wav   Portes/Claque lourde.WAV   Portes/Vieux/Cri.ogg (faux)
/// ```
fn library(name: &str) -> PathBuf {
    let root = temp(name);
    std::fs::create_dir_all(root.join("Portes").join("Vieux")).unwrap();
    std::fs::create_dir_all(root.join(".cache")).unwrap();
    for (rel, n) in [("Claquement.wav", 24_000), ("Bam.wav", 4_800), ("Portes/Grincement.wav", 96_000), ("Portes/Claque lourde.WAV", 12_000), (".cache/Cache.wav", 100)] {
        std::fs::write(root.join(rel), wav(n, 3000)).unwrap();
    }
    std::fs::write(root.join("notes.txt"), "pas un son").unwrap();
    std::fs::write(root.join("Portes").join("Vieux").join("Cri.ogg"), b"pas vraiment de l'ogg").unwrap();
    root
}

fn collect(dir: &Path, limit: usize) -> (Vec<Entry>, bool) {
    let mut all = Vec::new();
    let cut = scan_dir(dir, 0, &AtomicBool::new(false), limit, |b| all.extend(b));
    (all, cut)
}

/// Attend la fin du parcours, comme l'interface qui reprend la liste image après image.
fn finish(lib: &mut Bibliotheque) {
    let t = Instant::now();
    while lib.scanning() {
        lib.poll();
        assert!(t.elapsed() < Duration::from_secs(20), "parcours trop long");
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ------------------------------------------------------------------------------- parcours

#[test]
fn the_scan_lists_sounds_in_order_with_their_sub_folder() {
    let root = library("ordre");
    let (found, cut) = collect(&root, 1000);
    assert!(!cut);
    let names: Vec<String> = found.iter().map(|e| format!("{}|{}", e.rel, e.name)).collect();
    // Les fichiers d'un dossier d'abord (par nom, sans tenir compte de la casse), puis ses sous-dossiers ;
    // ni le texte, ni le dossier caché ; le « .ogg » est listé : on ne le décode pas avant qu'on le demande.
    assert_eq!(names, vec!["|Bam", "|Claquement", "Portes|Claque lourde", "Portes|Grincement", "Portes/Vieux|Cri"]);
    assert!(found.iter().all(|e| e.path.is_file() && e.bytes > 0));
    assert!(bruitages::is_sound(Path::new("a/b.MP3")) && bruitages::is_sound(Path::new("x.flac")) && !bruitages::is_sound(Path::new("x.txt")) && !bruitages::is_sound(Path::new("x")));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_scan_stops_at_the_limit_and_when_cancelled() {
    let root = library("limite");
    let (found, cut) = collect(&root, 3);
    assert_eq!(found.len(), 3);
    assert!(cut, "la liste est coupée");
    // Annulé d'avance : rien n'est lu.
    let mut all = Vec::new();
    scan_dir(&root, 0, &AtomicBool::new(true), 1000, |b| all.extend(b));
    assert!(all.is_empty());
    // Un dossier qui n'existe pas : rien, sans plantage.
    let (none, cut) = collect(&root.join("absent"), 10);
    assert!(none.is_empty() && !cut);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn scanning_runs_in_the_background_and_fills_the_list_afterwards() {
    let root = temp("fil");
    for i in 0..400 {
        std::fs::write(root.join(format!("son{i:04}.wav")), b"x").unwrap();
    }
    let other = library("fil2");
    let mut lib = Bibliotheque::default();
    lib.scan(&[root.clone(), other.clone()]);
    // L'appel est rendu tout de suite ; la liste est vide tant que l'interface n'a rien repris.
    assert!(lib.scanning() && lib.entries.is_empty() && lib.started);
    finish(&mut lib);
    assert_eq!(lib.entries.len(), 405);
    // Chaque son sait de quel dossier de la bibliothèque il vient.
    assert_eq!(lib.entries.iter().filter(|e| e.folder == 0).count(), 400);
    assert_eq!(lib.entries.iter().filter(|e| e.folder == 1).count(), 5);
    assert!(!lib.truncated);
    // Relancé : la liste repart de zéro.
    lib.scan(&[other.clone()]);
    finish(&mut lib);
    assert_eq!(lib.entries.len(), 5);
    std::fs::remove_dir_all(&root).ok();
    std::fs::remove_dir_all(&other).ok();
}

// ------------------------------------------------------------------------------- recherche

#[test]
fn the_search_needs_every_word_in_the_name_or_the_sub_folder() {
    let root = library("recherche");
    let mut lib = Bibliotheque::default();
    lib.scan(&[root.clone()]);
    finish(&mut lib);
    let shown = |lib: &mut Bibliotheque| -> Vec<String> {
        let rows = lib.visible().to_vec();
        rows.iter().map(|&i| lib.entries[i].name.clone()).collect()
    };
    assert_eq!(shown(&mut lib).len(), 5, "sans recherche : tout");
    lib.search = "CLAQU".into();
    assert_eq!(shown(&mut lib), vec!["Claquement", "Claque lourde"], "sans tenir compte de la casse");
    lib.search = "claque lourde".into();
    assert_eq!(shown(&mut lib), vec!["Claque lourde"], "tous les mots");
    lib.search = "portes cri".into();
    assert_eq!(shown(&mut lib), vec!["Cri"], "le nom du sous-dossier compte");
    lib.search = "zzz".into();
    assert!(shown(&mut lib).is_empty());
    lib.search = "  ".into();
    assert_eq!(shown(&mut lib).len(), 5, "des espaces seuls : tout");
    assert!(matches(&lib.entries[0], "bam") && !matches(&lib.entries[0], "bam portes"));
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------------- durées

#[test]
fn durations_come_from_the_header_and_are_asked_only_for_the_rows_shown() {
    let root = library("duree");
    // L'en-tête du WAV suffit : 24 000 échantillons à 48 kHz = 0,5 s.
    assert_eq!(probe_seconds(&root.join("Claquement.wav")), Some(0.5));
    assert_eq!(probe_seconds(&root.join("Portes").join("Grincement.wav")), Some(2.0));
    assert_eq!(probe_seconds(&root.join("notes.txt")), None, "pas un son");
    assert_eq!(probe_seconds(&root.join("absent.wav")), None);

    // Seules les lignes qu'on voit sont demandées (la sonde compte ses lectures).
    static PROBED: AtomicUsize = AtomicUsize::new(0);
    fn counting(_: &Path) -> Option<f64> {
        PROBED.fetch_add(1, Ordering::SeqCst);
        Some(1.25)
    }
    let mut lib = Bibliotheque::default();
    lib.sonde = Sonde::with(counting);
    lib.scan(&[root.clone()]);
    finish(&mut lib);
    lib.want_durations(0..2);
    lib.want_durations(1..3);
    let t = Instant::now();
    while lib.sonde.seconds.len() < 3 {
        lib.poll();
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(PROBED.load(Ordering::SeqCst), 3, "trois lignes vues, trois lectures, aucune à deux fois");
    assert_eq!(lib.sonde.get(&lib.entries[0].path), Some(Some(1.25)));
    assert_eq!(lib.sonde.get(&lib.entries[4].path), None, "les lignes hors de la vue n'ont pas été lues");
    // Redemander une durée déjà lue ne relit rien.
    lib.want_durations(0..3);
    std::thread::sleep(Duration::from_millis(30));
    lib.poll();
    assert_eq!(PROBED.load(Ordering::SeqCst), 3);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn the_duration_text_is_short_and_french() {
    assert_eq!(duration_text(None), "", "pas encore lue");
    assert_eq!(duration_text(Some(None)), "—", "illisible");
    assert_eq!(duration_text(Some(Some(0.42))), "0,42 s");
    assert_eq!(duration_text(Some(Some(12.34))), "12,3 s");
    assert_eq!(duration_text(Some(Some(65.0))), "1:05");
}

// ----------------------------------------------------------------------------------- écoute

/// Une sortie qui ne joue rien : elle note ce qu'on lui demande.
#[derive(Default)]
struct Faux {
    played: Vec<usize>,
    stopped: usize,
    sounding: bool,
}

impl Sortie for Faux {
    fn jouer(&mut self, pcm: Arc<Pcm>) {
        self.played.push(pcm.frames());
        self.sounding = true;
    }
    fn arreter(&mut self) {
        self.stopped += 1;
        self.sounding = false;
    }
    fn sonne(&self) -> bool {
        self.sounding
    }
}

fn decode_by_size(p: &Path) -> Result<Arc<Pcm>, String> {
    if p.extension().is_some_and(|e| e == "bad") {
        return Err("fichier illisible".into());
    }
    let n = std::fs::metadata(p).map_or(0, |m| m.len() as usize);
    Ok(Arc::new(Pcm::from_interleaved(&vec![0.1; n], 1, RATE)))
}

fn wait_for(ecoute: &mut Ecoute, sortie: &mut Faux, until: impl Fn(&Ecoute) -> bool) -> Option<String> {
    let t = Instant::now();
    let mut last = None;
    while !until(ecoute) {
        if let Some(e) = ecoute.suivre(sortie) {
            last = Some(e);
        }
        assert!(t.elapsed() < Duration::from_secs(10), "l'écoute n'avance pas");
        std::thread::sleep(Duration::from_millis(2));
    }
    last
}

#[test]
fn listening_decodes_apart_then_plays_and_a_second_click_stops() {
    let root = temp("ecoute");
    let (a, b) = (root.join("a.wav"), root.join("b.wav"));
    std::fs::write(&a, vec![0u8; 1000]).unwrap();
    std::fs::write(&b, vec![0u8; 2000]).unwrap();
    let mut e = Ecoute::avec(decode_by_size);
    let mut out = Faux::default();
    assert!(e.courant().is_none() && !e.joue());
    // Le clic ne joue rien lui-même : le son se décode à part, puis part à la sortie.
    e.basculer(&a, &mut out);
    assert!(e.charge() && e.courant() == Some(a.as_path()) && out.played.is_empty());
    wait_for(&mut e, &mut out, |e| e.joue());
    assert_eq!(out.played, vec![1000]);
    // Un autre son : le premier est arrêté, le second part.
    e.basculer(&b, &mut out);
    assert_eq!(out.stopped, 1);
    wait_for(&mut e, &mut out, |e| e.joue());
    assert_eq!(out.played, vec![1000, 2000]);
    // Le même son une seconde fois : l'écoute s'arrête.
    e.basculer(&b, &mut out);
    assert_eq!(out.stopped, 2);
    assert!(e.courant().is_none());
    // La fin du son : l'écoute revient au repos toute seule.
    e.basculer(&a, &mut out);
    wait_for(&mut e, &mut out, |e| e.joue());
    out.sounding = false;
    assert!(e.suivre(&mut out).is_none());
    assert!(e.courant().is_none() && !e.joue());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_sound_that_cannot_be_decoded_says_so_and_plays_nothing() {
    let root = temp("ecoute-erreur");
    let bad = root.join("casse.bad");
    std::fs::write(&bad, b"x").unwrap();
    let mut e = Ecoute::avec(decode_by_size);
    let mut out = Faux::default();
    e.basculer(&bad, &mut out);
    let message = wait_for(&mut e, &mut out, |e| e.courant().is_none());
    assert!(message.is_some_and(|m| m.contains("casse.bad") && m.contains("illisible")));
    assert!(out.played.is_empty() && out.stopped == 0);
    // Avec le vrai décodeur, un faux fichier son donne une erreur claire (jamais de plantage).
    let fake = root.join("faux.ogg");
    std::fs::write(&fake, b"pas de l'ogg").unwrap();
    let mut real = Ecoute::default();
    real.basculer(&fake, &mut out);
    let message = wait_for(&mut real, &mut out, |e| e.courant().is_none());
    assert!(message.is_some());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn what_the_preview_plays_is_the_whole_sound_once_at_full_volume() {
    let pcm = Arc::new(Pcm::from_interleaved(&vec![0.4; 960], 1, RATE));
    let (clips, len) = bruitages::extrait_ecoute(pcm);
    assert_eq!((clips.len(), len), (1, 960));
    assert_eq!((clips[0].start, clips[0].offset, clips[0].len, clips[0].gain), (0, 0, 960, 1.0));
    assert!(clips[0].env.is_none());
}

// ---------------------------------------------------------------------------------- réglages

#[test]
fn the_folders_are_an_application_setting_with_a_serde_default() {
    let mut s = crate::settings::Settings::default();
    assert!(s.bruitages.folders.is_empty());
    assert!(s.bruitages.add(PathBuf::from("C:/Sons/Portes")));
    assert!(s.bruitages.add(PathBuf::from("C:/Sons/Voitures")));
    let text = toml::to_string(&s).unwrap();
    assert!(text.contains("[bruitages]") && text.contains("dossiers"), "{text}");
    let back: crate::settings::Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.bruitages, s.bruitages);
    // Des réglages d'avant la bibliothèque : pas de dossier, rien à signaler.
    let old: crate::settings::Settings = toml::from_str("copie_secours_minutes = 2").unwrap();
    assert_eq!(old.bruitages, BruitagesReglages::default());
    // Sans dossier, la liste n'est pas écrite.
    let table: toml::Table = toml::from_str(&toml::to_string(&crate::settings::Settings::default()).unwrap()).unwrap();
    assert!(table.get("bruitages").and_then(|t| t.as_table()).is_none_or(|t| !t.contains_key("dossiers")));
}

#[test]
fn a_folder_is_added_once_and_a_parent_folder_replaces_its_children() {
    let mut r = BruitagesReglages::default();
    assert!(r.add(PathBuf::from("C:/Sons/Portes")));
    assert!(!r.add(PathBuf::from("C:/Sons/Portes")), "déjà là");
    assert!(!r.add(PathBuf::from("c:/sons/portes")), "sans tenir compte de la casse (Windows)");
    assert!(!r.add(PathBuf::from("C:/Sons/Portes/Vieux")), "déjà dans un dossier de la bibliothèque");
    assert!(r.add(PathBuf::from("C:/Sons")), "le dossier parent");
    assert_eq!(r.folders, vec![PathBuf::from("C:/Sons")], "il remplace ce qu'il contient");
    r.remove(7);
    assert_eq!(r.folders.len(), 1);
    r.remove(0);
    assert!(r.folders.is_empty());
}

// ------------------------------------------------------------------------ pose dans le montage

fn app_with_sound_file(name: &str) -> (App, PathBuf) {
    let dir = temp(name);
    crate::donnees::bac_a_sable(&dir);
    let file = dir.join("Porte qui claque.wav");
    std::fs::write(&file, wav(24_000, 8000)).unwrap();
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (app, file)
}

fn settle(app: &mut App) {
    let ctx = egui::Context::default();
    let t = Instant::now();
    while !app.brt_pending_is_empty() {
        app.poll_bruitages(&ctx);
        assert!(t.elapsed() < Duration::from_secs(20), "pose trop longue");
        std::thread::sleep(Duration::from_millis(2));
    }
    app.poll_bruitages(&ctx);
}

#[test]
fn an_inserted_sound_is_decoded_apart_copied_into_the_project_and_undoable() {
    let (mut app, file) = app_with_sound_file("poser");
    app.cur_panel = 0;
    app.cur_frame = 3;
    app.insert_bruitage(file.clone(), None, None);
    // Le son ne se pose pas dans l'appel : il se charge à part.
    assert!(app.doc.audio_tracks.is_empty());
    assert!(app.status.contains("Chargement"));
    settle(&mut app);
    // À la tête de lecture (image 3), 0,5 s à 24 i/s ou la cadence du projet.
    let track = &app.doc.audio_tracks[0];
    assert_eq!(track.clips.len(), 1);
    let clip = &track.clips[0];
    assert_eq!(clip.start, app.doc.panel_start(0) + 3);
    assert_eq!(clip.length, app.doc.sounds[&clip.sound].frames(app.doc.fps));
    // Le fichier est copié tel quel dans le projet, comme un son importé.
    let sound = &app.doc.sounds[&clip.sound];
    assert_eq!(sound.name, "Porte qui claque.wav");
    assert_eq!(**sound.bytes, std::fs::read(&file).unwrap());
    assert_eq!(app.sel_clip, Some(clip.id));
    // Une étape d'annulation nommée.
    assert_eq!(app.history.names().0.last().copied(), Some("Insérer un bruitage"));
    app.undo();
    assert!(app.doc.audio_tracks.iter().all(|t| t.clips.is_empty()));
    app.redo();
    assert_eq!(app.doc.audio_tracks[0].clips.len(), 1);
    // Enregistré avec le projet : le son est dans son dossier.
    let dir = temp("poser-projet");
    let manifest = dir.join("P.pelure");
    crate::project::save(&app.doc, &manifest, &mut Default::default()).unwrap();
    let copied = dir.join("sons").join(format!("{}.wav", app.doc.audio_tracks[0].clips[0].sound));
    assert_eq!(std::fs::read(copied).unwrap(), std::fs::read(&file).unwrap());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_sound_dropped_on_a_track_goes_on_that_track_at_that_frame() {
    let (mut app, file) = app_with_sound_file("deposer");
    app.doc.add_audio_track();
    app.doc.add_audio_track();
    // Lâché sur la deuxième piste (deux pistes de 42 points sous le haut des pistes), à l'image 7.
    let (left, ppf, lanes_top) = (6.0, 10.0, 200.0);
    let (track, frame) = crate::bruitagesui::drop_target(egui::pos2(left + 7.4 * ppf, lanes_top + 50.0), left, ppf, lanes_top, 2);
    assert_eq!((track, frame), (Some(1), 7));
    // Au-dessus des pistes ou plus bas que la dernière : la première piste libre sera prise.
    assert_eq!(crate::bruitagesui::drop_target(egui::pos2(left, lanes_top - 30.0), left, ppf, lanes_top, 2), (None, 0));
    assert_eq!(crate::bruitagesui::drop_target(egui::pos2(left + 3.0 * ppf, lanes_top + 500.0), left, ppf, lanes_top, 2), (None, 3));
    app.insert_bruitage(file, track, Some(frame));
    settle(&mut app);
    assert!(app.doc.audio_tracks[0].clips.is_empty());
    assert_eq!(app.doc.audio_tracks[1].clips[0].start, 7);
}

#[test]
fn a_sound_goes_on_the_track_of_the_chosen_clip_and_a_locked_track_is_left_alone() {
    let (mut app, file) = app_with_sound_file("piste");
    app.doc.add_audio_track();
    app.doc.add_audio_track();
    app.insert_bruitage(file.clone(), Some(1), Some(0));
    settle(&mut app);
    let chosen = app.doc.audio_tracks[1].clips[0].id;
    assert_eq!(app.sel_clip, Some(chosen));
    // Le suivant, sans piste demandée, va sur la piste de l'extrait choisi (libre plus loin).
    app.insert_bruitage(file.clone(), None, Some(100));
    settle(&mut app);
    assert_eq!(app.doc.audio_tracks[1].clips.len(), 2);
    // Une piste verrouillée n'est pas visée : la première piste libre.
    app.doc.audio_tracks[1].locked = true;
    app.insert_bruitage(file, None, Some(200));
    settle(&mut app);
    assert_eq!(app.doc.audio_tracks[0].clips.len(), 1);
}

#[test]
fn an_unknown_format_is_refused_at_once_and_a_broken_file_is_reported() {
    let (mut app, file) = app_with_sound_file("refus");
    let txt = file.with_file_name("notes.txt");
    std::fs::write(&txt, "x").unwrap();
    app.insert_bruitage(txt, None, None);
    assert!(app.status.contains("format de son non géré"), "{}", app.status);
    assert!(app.brt_pending_is_empty());
    let broken = file.with_file_name("casse.wav");
    std::fs::write(&broken, b"RIFF pas un vrai fichier").unwrap();
    app.insert_bruitage(broken, None, None);
    settle(&mut app);
    assert!(app.status.contains("Impossible d'insérer"), "{}", app.status);
    assert!(app.doc.audio_tracks.is_empty());
}

// --------------------------------------------------------------------------------- le panneau

#[test]
fn the_pane_is_closed_by_default_opens_on_the_right_and_has_its_icon_and_command() {
    use crate::dock::{DockLayout, Group};
    use crate::workspace::Layout;
    assert!(Pane::Bruitages.on_demand());
    assert_eq!(Pane::Bruitages.group(), Group::Right);
    assert_eq!(serde_json::to_string(&Pane::Bruitages).unwrap(), "\"bruitages\"");
    assert!(Pane::ALL.contains(&Pane::Bruitages));
    let mut l = DockLayout::from_legacy(&Layout::default());
    assert!(!l.contains(Pane::Bruitages), "fermé par défaut");
    l.open(Pane::Bruitages);
    assert!(l.contains(Pane::Bruitages));
    assert!(format!("{:?}", l.principal).contains("Bruitages"), "{:?}", l.principal);
    // L'icône de la barre des panneaux et son action.
    assert_eq!(crate::barrepanneaux::pane_de_action("vue.bruitages"), Some(Pane::Bruitages));
    for id in ["vue.bruitages", "bruitage.inserer", "bruitage.ecouter"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    assert!(!Pane::Bruitages.icon().is_empty());
}

#[test]
fn the_commands_need_a_chosen_sound() {
    let (mut app, file) = app_with_sound_file("commandes");
    app.run_action("bruitage.inserer");
    assert!(app.status.contains("Choisis d'abord"), "{}", app.status);
    app.brt.lib.selected = Some(file);
    app.cur_frame = 2;
    app.run_action("bruitage.inserer");
    settle(&mut app);
    assert_eq!(app.doc.audio_tracks[0].clips[0].start, 2);
}
