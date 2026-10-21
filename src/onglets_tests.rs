//! Onglets de projets (`onglets.rs`, `ongletsui.rs`) : plusieurs projets ouverts dans la même fenêtre.
//!
//! Ce que ces essais gardent : chaque onglet a son annulation, sa tête de lecture, sa sélection, sa vue ;
//! changer d'onglet ne relit rien sur le disque ; l'enregistrement va au bon projet, même quand on change d'onglet
//! pendant qu'il s'écrit ; fermer un onglet modifié demande ; quitter avec plusieurs onglets modifiés demande une
//! seule fois ; aucun cache (vignettes, effets, textures) ne montre les pixels d'un autre projet ; les
//! presse-papiers voyagent d'un onglet à l'autre ; la session revient au lancement ; chaque projet a sa copie de
//! secours ; trois onglets au repos ne demandent aucune repeinture.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, Color32, Event, Key, Modifiers, PointerButton, Rect};

use crate::app::{Mode, Pending};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::dossier;
use crate::onglets::CHAMPS_DE_PROJET;
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::project;

// ---------------------------------------------------------------------------------------------- outils

/// Un projet de `n` images : le premier pixel de chaque dessin a ce rouge (de quoi reconnaître le projet).
fn projet(rouge: u8, n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for k in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        cel.raster.tile_mut(k)[0] = Color32::from_rgba_premultiplied(rouge, 10, 10, 255);
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

/// Un projet dont chaque dessin est entièrement de cette couleur (les vignettes ont alors une couleur).
fn projet_plein(couleur: Color32, n: usize) -> Document {
    let mut doc = Document::with_frame(320, 180, 24);
    let mut layer = Layer::new("Calque", LayerKind::Bitmap);
    for _ in 0..n {
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
        for k in 0..cel.raster.tiles.len() {
            cel.raster.tile_mut(k).fill(couleur);
        }
        layer.frames.push(Some(doc.add_cel(cel)));
    }
    doc.panels[0].layers = vec![layer];
    doc.panels[0].length = n;
    doc
}

/// Ouvre `doc` dans un onglet neuf (comme le fait un projet qu'on crée ou qu'on ouvre).
fn onglet_avec(b: &mut Banc, doc: Document) {
    assert!(b.app.nouvel_onglet(), "{}", b.app.status);
    assert!(b.app.charger_projet(doc));
    b.app.welcome = false;
    b.chauffer(2);
}

/// Le dessin de la première image du plan courant.
fn premier_dessin(b: &Banc) -> CelId {
    b.app.doc.panels[0].layers[0].frames[0].expect("dessin")
}

/// Le rouge du premier pixel du premier dessin du projet affiché.
fn rouge(b: &Banc) -> u8 {
    b.app.doc.cels[&premier_dessin(b)].raster.get(0, 0).r()
}

/// Le dessin de la première image, modifié comme un trait : pixel changé, version et projet modifié.
fn modifier(b: &mut Banc, r: u8) {
    let id = premier_dessin(b);
    let cel = b.app.doc.cels.get_mut(&id).expect("dessin");
    cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(r, 10, 10, 255);
    cel.version += 1;
    b.app.modified = true;
}

/// Le rouge du premier pixel du premier dessin tel que le fichier le garde.
fn rouge_sur_disque(manifeste: &Path) -> u8 {
    let loaded = project::load(manifeste).expect("projet lisible");
    let id = loaded.doc.panels[0].layers[0].frames[0].expect("dessin");
    loaded.doc.cels[&id].raster.get(0, 0).r()
}

/// Range le projet affiché dans un dossier d'essai (`Nom/Nom.pelure`) : il est enregistré, avec son chemin.
fn enregistrer_a(b: &mut Banc, dir: &Path, nom: &str) -> PathBuf {
    let manifeste = dir.join(nom).join(format!("{nom}.pelure"));
    b.app.path = Some(manifeste.clone());
    b.app.modified = true;
    assert!(b.app.save());
    pomper(b);
    assert!(!b.app.modified, "enregistré");
    manifeste
}

/// Des images jusqu'à la fin de toutes les écritures, dans tous les onglets.
fn pomper(b: &mut Banc) {
    for _ in 0..800 {
        if !b.app.infos_onglets().iter().any(|i| i.ecriture) {
            return;
        }
        b.image(Vec::new(), 0.016);
        std::thread::sleep(Duration::from_millis(4));
    }
    panic!("les enregistrements ne se terminent pas");
}

/// Des images jusqu'à ce que `fini` soit vrai.
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

/// Ouvre le projet `manifeste` comme Ouvrir le fait (sur un autre fil), et attend qu'il soit là.
fn ouvrir(b: &mut Banc, manifeste: &Path) {
    b.app.open_path(manifeste.to_path_buf());
    assert!(jusqu_a(b, Duration::from_secs(60), |b| b.app.open_job.is_none()), "l'ouverture finit : {}", b.app.status);
    b.chauffer(2);
}

/// Le nom des onglets dans l'ordre de la bande.
fn noms(b: &Banc) -> Vec<String> {
    b.app.infos_onglets().into_iter().map(|i| i.nom).collect()
}

fn ctrl() -> Modifiers {
    Modifiers::COMMAND
}

/// Une touche pressée pendant une image.
fn touche(b: &mut Banc, key: Key, modifiers: Modifiers) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }], 1.0 / 60.0);
}

/// Moyenne (r, g, b) d'une vignette.
fn teinte(image: &egui::ColorImage) -> [f32; 3] {
    let n = image.pixels.len().max(1) as f32;
    let mut s = [0.0f32; 3];
    for p in &image.pixels {
        s[0] += p.r() as f32;
        s[1] += p.g() as f32;
        s[2] += p.b() as f32;
    }
    [s[0] / n, s[1] / n, s[2] / n]
}

/// Une feuille d'une autre taille que celle de `projet` (320 × 180 de cadre, feuille deux fois plus grande).
fn projet_grand() -> Document {
    let mut grand = Document::with_frame(640, 360, 24);
    grand.panels[0].layers = vec![Layer::new("Calque", LayerKind::Bitmap)];
    grand
}

// ---------------------------------------------------------------------------------------------- garde

/// Les champs d'`App` qui ne sont PAS propres à un projet : le reste de l'état de l'application. Un champ neuf
/// d'`App` doit être rangé dans l'un des deux jeux : `etat_projet!` (`onglets.rs`) s'il ne vaut que pour un projet, ici
/// sinon. Sans cet essai, un champ de projet oublié fuirait d'un onglet à l'autre sans que rien ne le dise.
const PARTAGES: &[&str] = &[
    "ctx", "board_view", "tool", "brush", "pencil", "eraser", "brushes", "brushes_job", "reopen_probe", "fill", "shape", "vector_erase_whole", "paint_behind", "fill_shapes", "toucher",
    "pinceaux", "color", "hsv", "couleurs", "canvas_rect", "ppp", "onion", "onion_window", "onion_shift", "onion_preset_name", "xsheet", "roles_view", "role_menu", "xsheet_role_menu",
    "flips", "flips_window", "fx_auto_key", "fx_keys_shown", "fx_key_clipboard", "fx_clip_recent", "fx_stack_clipboard", "fx_pane_focus", "fx_pile_recent", "layers_pane_focus","propagation", "coloriage", "sb_window",
    "scenario_window", "sb_export", "light_table", "light_table_strength", "groups_window", "groups_used_only", "image_clipboard", "key_clipboard", "tool_spring", "fake_hold_times",
    "time_display", "empty_x", "timeline_thumbs", "mark_color", "mark_repeat", "mark_next_layer", "mark_used_only", "bottom_tab", "checker", "notes", "properties_window", "settings_tab",
    "brush_previews", "preset_edit", "brush_import_notes", "record_strokes", "camera_mask", "top_view", "play_meter", "frame_start", "export_dialog", "export_last", "export_job", "export_projet", "ffmpeg",
    "settings", "layout", "dock", "detached", "detach_request", "exposition_masquee", "espace", "vue_camera_du_montage", "settings_a_ecrire", "workspace_name", "orphans", "default_exposure", "looping", "audio",
    "audio_tried", "audio_clock", "sound_on", "sound_scrub", "brt", "sug", "activity", "perf_applied", "diag", "keep_proportions", "field_edit_alive", "deform_mode", "select_rect",
    "pixel_clipboard", "layer_clipboard", "multi_clip", "panel_clipboard", "cam_key_clipboard", "cam_clip_recent", "cam_fx", "cadrage", "cadrage_cache", "text_font", "text_size",
    "text_fonts", "text_font_filter", "color_select", "tool_recent", "tool_flyout", "paste_event", "messages", "last_status", "toasts", "messages_window", "palette", "export_hub",
    "prefs_window", "editorial", "fiche", "about_window", "theme_applied", "theme_unsaved","pane_heights", "fit_set", "fit_busy", "recent_to_open", "recent_presence",
    "maximized_once", "clipboard", "montage_head_w", "snap_on", "length_field_active", "pen_pressure", "pen_touch", "input_time", "last_frame_time", "picking", "status", "new_dialog",
    "show_help", "allow_close", "export_transparent", "follow_frame", "following", "hover_canvas", "pending_keys", "assign", "keys_filter", "keymap", "title", "display_info", "gpu",
    "quitter_prevenu", "collage", "onglets", "actif", "prochain_onglet", "secours_actif", "session_a_ecrire", "session_prete", "session_file", "session_affichee", "groupe",
];

/// Les noms des champs de `pub struct App`, lus dans `app.rs`.
fn champs_de_app() -> Vec<String> {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("app.rs")).expect("app.rs");
    let debut = source.find("pub struct App {").expect("la structure App");
    let corps = &source[debut..];
    let fin = corps.find("\n}\n").expect("fin de la structure");
    corps[..fin]
        .lines()
        .filter_map(|l| {
            let l = l.strip_prefix("    ")?;
            if l.starts_with(' ') || l.starts_with("//") {
                return None;
            }
            let l = l.strip_prefix("pub(crate) ").or_else(|| l.strip_prefix("pub ")).unwrap_or(l);
            let (nom, _) = l.split_once(": ")?;
            nom.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_').then(|| nom.to_string())
        })
        .collect()
}

#[test]
fn every_field_of_the_app_is_either_a_project_field_or_shared() {
    let champs = champs_de_app();
    assert!(champs.len() > 300, "la lecture de app.rs ne voit plus les champs ({})", champs.len());
    let mut oublies = Vec::new();
    for c in &champs {
        let projet = CHAMPS_DE_PROJET.contains(&c.as_str());
        let partage = PARTAGES.contains(&c.as_str());
        if projet && partage {
            panic!("le champ « {c} » est à la fois de projet (etat_projet!) et partagé (PARTAGES)");
        }
        if !projet && !partage {
            oublies.push(c.clone());
        }
    }
    assert!(
        oublies.is_empty(),
        "ces champs d'App ne sont classés nulle part : s'ils ne valent que pour un projet (document, sélection, geste, cache, tâche de fond), ajoute-les à etat_projet! \
         dans onglets.rs ; sinon à PARTAGES dans onglets_tests.rs : {oublies:?}"
    );
    for p in PARTAGES.iter().chain(CHAMPS_DE_PROJET.iter()) {
        assert!(champs.iter().any(|c| c == p), "« {p} » n'est plus un champ d'App : à retirer de la liste");
    }
    let mut vus = std::collections::BTreeSet::new();
    for c in CHAMPS_DE_PROJET {
        assert!(vus.insert(*c), "« {c} » est listé deux fois dans etat_projet!");
    }
}

// ---------------------------------------------------------------------------------------------- les onglets

#[test]
fn a_new_tab_is_the_welcome_screen_and_the_first_tab_is_kept_aside() {
    let mut b = banc(projet(200, 2), false);
    assert_eq!(b.app.onglets.len(), 1);
    assert!(b.app.nouvel_onglet());
    assert_eq!((b.app.onglets.len(), b.app.actif), (2, 1));
    assert!(b.app.welcome_shown(), "un onglet neuf montre l'accueil");
    assert_eq!((b.app.doc.frame, b.app.doc.fps), ([1920, 1080], 24));
    assert_eq!(noms(&b), ["Sans titre", "Nouvel onglet"]);
    // Le premier projet est rangé, intact.
    let range = b.app.onglets[0].etat.as_ref().expect("rangé");
    assert_eq!(range.doc.frame, [320, 180]);
    assert_eq!(range.doc.panels[0].layers[0].frames.len(), 2);
    // Un onglet vierge n'en ouvre pas un second.
    assert!(b.app.nouvel_onglet());
    assert_eq!(b.app.onglets.len(), 2);
}

#[test]
fn projects_not_yet_saved_get_numbered_names() {
    let mut b = banc(projet(1, 1), false);
    onglet_avec(&mut b, projet(2, 1));
    onglet_avec(&mut b, projet(3, 1));
    assert!(b.app.nouvel_onglet());
    assert_eq!(noms(&b), ["Sans titre", "Sans titre 2", "Sans titre 3", "Nouvel onglet"]);
}

#[test]
fn undo_is_isolated_per_tab() {
    let mut b = banc(projet(200, 2), false);
    let n0 = b.app.layers().len();
    b.app.add_layer(LayerKind::Bitmap);
    assert_eq!(b.app.layers().len(), n0 + 1);
    onglet_avec(&mut b, projet(50, 3));
    let n1 = b.app.layers().len();
    b.app.add_layer(LayerKind::Bitmap);
    b.app.add_layer(LayerKind::Bitmap);
    assert_eq!(b.app.layers().len(), n1 + 2);
    // L'annulation du deuxième onglet ne touche pas au premier.
    b.app.undo();
    b.app.undo();
    assert_eq!(b.app.layers().len(), n1);
    b.app.undo();
    assert!(b.app.status.contains("Rien à annuler"), "{}", b.app.status);
    assert_eq!(b.app.layers().len(), n1, "rien à annuler dans cet onglet : le premier n'y est pour rien");
    // Le premier garde son étape.
    assert!(b.app.activer_onglet(0));
    assert_eq!(b.app.layers().len(), n0 + 1);
    b.app.undo();
    assert_eq!(b.app.layers().len(), n0);
    b.app.redo();
    assert_eq!(b.app.layers().len(), n0 + 1);
    // Et le deuxième garde ses deux rétablir.
    assert!(b.app.activer_onglet(1));
    b.app.redo();
    b.app.redo();
    assert_eq!(b.app.layers().len(), n1 + 2);
}

#[test]
fn switching_keeps_the_playhead_the_selection_and_the_view() {
    let mut b = banc(projet(200, 6), false);
    b.app.cur_frame = 3;
    b.app.view.zoom = 3.25;
    b.app.layer_sel = vec![4242];
    b.app.frame_sel = Some((1, 4));
    b.app.timeline_cell = 33.0;
    b.app.montage_zoom = 7.0;
    b.app.set_mode(Mode::Animation);
    b.app.fit_pending = false;
    b.app.playing = true;
    onglet_avec(&mut b, projet(50, 3));
    // Le premier onglet ne joue plus et le nouveau n'a rien des réglages du premier.
    assert!(!b.app.playing);
    assert_eq!(b.app.mode, Mode::Storyboard);
    assert_eq!(b.app.espace.mode(), Mode::Storyboard, "l'espace de travail suit le mode de l'onglet");
    assert_eq!(b.app.cur_frame, 0);
    assert!(b.app.layer_sel.is_empty() && b.app.frame_sel.is_none());
    assert_ne!(b.app.view.zoom, 3.25);
    b.app.cur_frame = 1;
    b.app.layer_sel = vec![7];
    assert!(b.app.activer_onglet(0));
    assert_eq!((b.app.cur_frame, b.app.view.zoom, b.app.layer_sel.clone(), b.app.frame_sel), (3, 3.25, vec![4242], Some((1, 4))));
    assert_eq!((b.app.timeline_cell, b.app.montage_zoom, b.app.mode), (33.0, 7.0, Mode::Animation));
    assert_eq!(b.app.espace.mode(), Mode::Animation, "le projet revient dans son espace de travail");
    assert!(!b.app.playing);
    assert!(b.app.activer_onglet(1));
    assert_eq!((b.app.cur_frame, b.app.layer_sel.clone()), (1, vec![7]));
}

#[test]
fn switching_does_not_read_the_disk_again() {
    let dir = dossier("onglets-disque");
    let mut b = banc(projet(200, 3), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    onglet_avec(&mut b, projet(90, 3));
    let bb = enregistrer_a(&mut b, &dir, "B");
    // Plus rien sur le disque : changer d'onglet ne le relit pas.
    std::fs::remove_dir_all(a.parent().expect("dossier")).expect("effacé");
    std::fs::remove_dir_all(bb.parent().expect("dossier")).expect("effacé");
    for _ in 0..3 {
        assert!(b.app.activer_onglet(0));
        assert_eq!(rouge(&b), 200);
        assert!(b.app.activer_onglet(1));
        assert_eq!(rouge(&b), 90);
    }
    assert!(b.app.open_job.is_none(), "aucune lecture n'a été lancée");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_same_project_is_never_opened_twice() {
    let dir = dossier("onglets-doublon");
    let mut b = banc(projet(200, 3), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    onglet_avec(&mut b, projet(90, 3));
    assert_eq!((b.app.onglets.len(), b.app.actif), (2, 1));
    // Ouvrir le projet du premier onglet : son onglet passe à l'écran, rien de plus n'est lu.
    b.app.open_path(a.clone());
    assert!(b.app.open_job.is_none());
    assert_eq!((b.app.onglets.len(), b.app.actif), (2, 0));
    assert!(b.app.status.contains("déjà ouvert"), "{}", b.app.status);
    // Même chemin écrit autrement (la casse), comme Windows le voit.
    assert!(b.app.activer_onglet(1));
    b.app.open_path(PathBuf::from(a.to_string_lossy().to_uppercase()));
    assert_eq!((b.app.onglets.len(), b.app.actif), (2, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn opening_a_project_takes_a_new_tab_unless_the_current_one_is_the_welcome_screen() {
    let dir = dossier("onglets-ouvrir");
    // Un projet sur le disque.
    let mut a = banc(projet(77, 2), false);
    let chemin = enregistrer_a(&mut a, &dir, "Source");
    // Un SHOT neuf (l'accueil) : le projet prend la place de l'accueil.
    let mut b = banc(projet(1, 1), false);
    b.app.welcome = true;
    b.app.doc = Document::with_frame(1920, 1080, 24);
    ouvrir(&mut b, &chemin);
    assert_eq!(b.app.onglets.len(), 1, "l'accueil est remplacé, pas gardé à côté");
    assert_eq!(b.app.path.as_deref(), Some(chemin.as_path()));
    // Un autre projet, ensuite : un onglet de plus, et le premier n'est pas touché.
    let autre = dossier("onglets-ouvrir-b");
    let mut c = banc(projet(33, 2), false);
    let chemin_b = enregistrer_a(&mut c, &autre, "Autre");
    ouvrir(&mut b, &chemin_b);
    assert_eq!(b.app.onglets.len(), 2);
    assert_eq!(rouge(&b), 33);
    assert_eq!(b.app.onglets[0].etat.as_ref().expect("rangé").path.as_deref(), Some(chemin.as_path()));
    assert!(!b.app.modified, "rien à enregistrer après une ouverture");
    // Le réglage éteint : le projet remplace celui qui est affiché (après la question habituelle).
    b.app.settings.open_in_new_tab = false;
    b.app.modified = true;
    b.app.request(Pending::Open);
    assert!(b.app.pending.is_some(), "projet modifié : la boîte habituelle demande");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&autre);
}

#[test]
fn saving_goes_to_the_right_project() {
    let dir = dossier("onglets-enregistrer");
    let mut b = banc(projet(200, 3), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    onglet_avec(&mut b, projet(90, 3));
    let bb = enregistrer_a(&mut b, &dir, "B");
    modifier(&mut b, 50);
    assert!(b.app.activer_onglet(0));
    modifier(&mut b, 120);
    assert!(b.app.infos_onglets().iter().all(|i| i.modifie), "les deux sont modifiés");
    // Enregistrer dans le deuxième onglet n'écrit que lui.
    assert!(b.app.activer_onglet(1));
    assert!(b.app.save());
    pomper(&mut b);
    assert_eq!((rouge_sur_disque(&a), rouge_sur_disque(&bb)), (200, 50));
    let infos = b.app.infos_onglets();
    assert!(infos[0].modifie && !infos[1].modifie, "seul le projet enregistré est propre");
    assert!(b.app.activer_onglet(0));
    assert!(b.app.save());
    pomper(&mut b);
    assert_eq!((rouge_sur_disque(&a), rouge_sur_disque(&bb)), (120, 50));
    assert!(b.app.infos_onglets().iter().all(|i| !i.modifie));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_save_started_in_a_tab_is_adopted_by_that_tab_after_switching() {
    let dir = dossier("onglets-fond");
    let mut b = banc(projet(200, 4), false);
    onglet_avec(&mut b, projet(90, 3));
    modifier(&mut b, 91);
    assert!(b.app.activer_onglet(0));
    // Le premier projet n'a pas encore de fichier : l'écriture le crée.
    let manifeste = dir.join("Film").join("Film.pelure");
    b.app.path = Some(manifeste.clone());
    modifier(&mut b, 201);
    assert!(b.app.save());
    assert!(b.app.save_en_cours());
    // On change d'onglet pendant l'écriture : elle continue, et son résultat revient à son onglet.
    assert!(b.app.activer_onglet(1));
    assert!(b.app.onglets[0].etat.as_ref().expect("rangé").saving.is_some() || !b.app.onglets[0].etat.as_ref().expect("rangé").modified);
    assert!(b.app.modified, "le deuxième onglet reste modifié : l'écriture n'est pas la sienne");
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets[0].etat.as_ref().is_some_and(|e| e.saving.is_none())), "l'écriture finit");
    assert_eq!(rouge_sur_disque(&manifeste), 201);
    let infos = b.app.infos_onglets();
    assert!(!infos[0].modifie && !infos[0].ecriture, "le premier onglet est propre : {:?}", infos[0]);
    assert!(infos[1].modifie && infos[1].actif, "le deuxième n'a pas bougé : {:?}", infos[1]);
    assert_eq!(b.app.onglets[0].etat.as_ref().expect("rangé").path.as_deref(), Some(manifeste.as_path()));
    assert!(b.app.path.is_none(), "le chemin du premier projet n'est pas allé au deuxième");
    assert!(b.app.status.contains("Film") && b.app.status.contains("Enregistré"), "le message dit de quel onglet il parle : {}", b.app.status);
    assert!(b.app.settings.recent_projects.iter().any(|p| p == &manifeste), "le projet enregistré est un projet récent");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn epochs_are_unique_so_a_job_only_answers_its_own_tab() {
    let mut b = banc(projet(1, 1), false);
    onglet_avec(&mut b, projet(2, 1));
    onglet_avec(&mut b, projet(3, 1));
    let mut epoques: Vec<u64> = (0..3).map(|i| if i == b.app.actif { b.app.doc_epoch } else { b.app.onglets[i].etat.as_ref().expect("rangé").doc_epoch }).collect();
    epoques.sort_unstable();
    epoques.dedup();
    assert_eq!(epoques.len(), 3, "trois onglets, trois époques");
    // Remplacer le projet d'un onglet lui donne une époque que personne n'a eue.
    let avant = b.app.doc_epoch;
    b.app.load_doc(projet(9, 1));
    assert!(b.app.doc_epoch != avant && !epoques.contains(&b.app.doc_epoch));
}

// ---------------------------------------------------------------------------------------------- fermer

#[test]
fn closing_a_modified_inactive_tab_brings_it_up_and_asks() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    assert_eq!(b.app.onglets.len(), 3);
    // Le premier onglet est modifié, le troisième est affiché.
    assert!(b.app.activer_onglet(0));
    modifier(&mut b, 201);
    assert!(b.app.activer_onglet(2));
    b.app.fermer_onglet(0);
    assert_eq!(b.app.actif, 0, "l'onglet à fermer passe à l'écran pour que la question se voie");
    assert!(matches!(b.app.pending, Some(Pending::Close)), "la boîte « Modifications non enregistrées »");
    assert_eq!(b.app.onglets.len(), 3);
    // « Ne pas enregistrer » : l'onglet se ferme, le voisin de droite s'affiche.
    let action = b.app.pending.take().expect("question");
    b.app.perform(action);
    assert_eq!(b.app.onglets.len(), 2);
    assert_eq!(b.app.actif, 0);
    assert_eq!(rouge(&b), 90, "le voisin de droite devient l'onglet affiché");
}

#[test]
fn closing_a_clean_inactive_tab_does_not_ask_and_does_not_move() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    assert!(b.app.activer_onglet(1));
    b.app.fermer_onglet(0);
    assert_eq!(b.app.onglets.len(), 2);
    assert_eq!(b.app.actif, 0, "l'onglet affiché est le même, son rang a baissé");
    assert_eq!(rouge(&b), 90);
    assert!(b.app.pending.is_none());
    b.app.fermer_onglet(1);
    assert_eq!((b.app.onglets.len(), b.app.actif), (1, 0));
    assert_eq!(rouge(&b), 90);
}

#[test]
fn closing_the_last_tab_leaves_an_empty_project_and_the_welcome_screen() {
    let mut b = banc(projet(200, 2), false);
    b.app.fermer_onglet(0);
    assert_eq!(b.app.onglets.len(), 1);
    assert!(b.app.welcome_shown());
    assert!(b.app.onglet_vierge(), "l'accueil est de nouveau un onglet vierge : un projet qu'on ouvre y prend la place");
    assert_eq!(b.app.doc.frame, [320, 180], "un projet vide du même format");
    assert!(b.app.path.is_none() && !b.app.modified);
}

#[test]
fn closing_the_last_of_several_tabs_shows_the_left_neighbour() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    b.app.fermer_onglet(2);
    assert_eq!((b.app.onglets.len(), b.app.actif), (2, 1));
    assert_eq!(rouge(&b), 90);
}

#[test]
fn saving_then_closing_writes_the_project_and_closes_its_tab() {
    let dir = dossier("onglets-enregistrer-fermer");
    let mut b = banc(projet(200, 2), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    modifier(&mut b, 66);
    onglet_avec(&mut b, projet(90, 2));
    assert!(b.app.activer_onglet(0));
    b.app.fermer_onglet(0);
    assert!(b.app.pending.is_some());
    let action = b.app.pending.take().expect("question");
    // « Enregistrer » : l'écriture, puis la fermeture.
    assert!(b.app.save_then(Some(action)));
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets.len() == 1), "l'onglet se ferme à la fin de l'écriture");
    assert_eq!(rouge_sur_disque(&a), 66);
    assert_eq!(rouge(&b), 90);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_close_waiting_for_a_save_follows_it_even_when_another_tab_is_shown_meanwhile() {
    let dir = dossier("onglets-fermer-apres");
    let mut b = banc(projet(200, 6), false);
    let a = dir.join("A").join("A.pelure");
    b.app.path = Some(a.clone());
    modifier(&mut b, 150);
    onglet_avec(&mut b, projet(90, 2));
    assert!(b.app.activer_onglet(0));
    // « Enregistrer » dans la boîte de fermeture, puis on regarde l'autre onglet avant la fin de l'écriture.
    assert!(b.app.save_then(Some(Pending::Close)));
    assert!(b.app.activer_onglet(1));
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets.len() == 1), "l'onglet se ferme à la fin de son écriture : {}", b.app.status);
    assert_eq!(rouge_sur_disque(&a), 150);
    assert_eq!(rouge(&b), 90, "l'autre projet est resté affiché");
    // Fermer l'onglet affiché pendant qu'il s'écrit (rien de modifié depuis le cliché) : la fermeture suit l'écriture.
    let mut c = banc(projet(120, 5), false);
    c.app.path = Some(dir.join("C").join("C.pelure"));
    modifier(&mut c, 121);
    onglet_avec(&mut c, projet(1, 1));
    assert!(c.app.activer_onglet(0));
    assert!(c.app.save());
    c.app.fermer_onglet(0);
    assert!(c.app.after_save.is_some(), "la fermeture attend l'écriture");
    assert!(jusqu_a(&mut c, Duration::from_secs(60), |c| c.app.onglets.len() == 1));
    assert_eq!(rouge_sur_disque(&dir.join("C").join("C.pelure")), 121);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stroke_in_progress_ends_in_its_own_tab_when_switching() {
    let mut b = banc(projet(200, 2), false);
    b.app.begin_paint(pos2(40.0, 40.0), 1.0, false);
    b.app.stroke_to(pos2(90.0, 70.0), 1.0);
    assert!(b.app.stroke.is_some() || b.app.vstroke.is_some(), "un trait est en cours");
    onglet_avec(&mut b, projet(90, 2));
    assert!(b.app.stroke.is_none() && b.app.vstroke.is_none(), "le nouvel onglet n'a pas de trait en cours");
    assert!(b.app.history.is_empty() && !b.app.modified);
    assert!(b.app.activer_onglet(0));
    assert!(b.app.stroke.is_none() && b.app.vstroke.is_none(), "le trait s'est terminé dans son onglet");
    assert!(b.app.history.serial() > 0 && b.app.modified, "une étape d'annulation, dans le premier onglet");
}

#[test]
fn a_project_file_dropped_on_the_window_opens_in_a_tab() {
    let dir = dossier("onglets-depot");
    let mut a = banc(projet(77, 2), false);
    let chemin = enregistrer_a(&mut a, &dir, "Depose");
    let mut b = banc(projet(1, 1), false);
    assert!(!b.app.import_dropped_project(&dir.join("notes.txt")));
    assert!(b.app.import_dropped_project(&chemin));
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.open_job.is_none()));
    b.chauffer(2);
    assert_eq!(b.app.onglets.len(), 2);
    assert_eq!(rouge(&b), 77);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_others_can_save_them_all_first() {
    let dir = dossier("onglets-autres-enregistrer");
    let mut b = banc(projet(200, 2), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    modifier(&mut b, 201);
    onglet_avec(&mut b, projet(90, 2));
    let c = enregistrer_a(&mut b, &dir, "C");
    modifier(&mut b, 91);
    onglet_avec(&mut b, projet(50, 2));
    let garde = b.app.onglets[b.app.actif].id;
    b.app.fermer_les_autres(garde);
    assert_eq!(b.app.groupe.as_ref().map(|g| g.onglets.len()), Some(2));
    b.app.groupe_choisir(0);
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets.len() == 1), "les autres se ferment une fois écrits : {}", b.app.status);
    assert_eq!((rouge_sur_disque(&a), rouge_sur_disque(&c)), (201, 91));
    assert_eq!(rouge(&b), 50, "l'onglet gardé est resté affiché");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_others_asks_once_for_the_modified_ones() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    let garde = b.app.onglets[b.app.actif].id;
    assert!(b.app.activer_onglet(0));
    modifier(&mut b, 201);
    assert!(b.app.activer_onglet(2));
    b.app.fermer_les_autres(garde);
    let g = b.app.groupe.as_ref().expect("une seule boîte");
    assert_eq!(g.onglets.len(), 1, "le seul projet modifié est listé");
    assert_eq!(b.app.onglets.len(), 3, "rien n'est fermé avant la réponse");
    b.app.groupe_choisir(2);
    assert!(b.app.groupe.is_none() && b.app.onglets.len() == 3, "Annuler ne ferme rien");
    b.app.fermer_les_autres(garde);
    b.app.groupe_choisir(1);
    assert_eq!(b.app.onglets.len(), 1);
    assert_eq!(rouge(&b), 50);
    // Aucun onglet modifié : tout se ferme sans question.
    onglet_avec(&mut b, projet(10, 1));
    onglet_avec(&mut b, projet(11, 1));
    let garde = b.app.onglets[b.app.actif].id;
    b.app.fermer_les_autres(garde);
    assert!(b.app.groupe.is_none());
    assert_eq!(b.app.onglets.len(), 1);
}

#[test]
fn quitting_with_several_modified_tabs_asks_once_and_lists_them() {
    let dir = dossier("onglets-quitter");
    let mut b = banc(projet(200, 2), false);
    let a = enregistrer_a(&mut b, &dir, "A");
    onglet_avec(&mut b, projet(90, 2));
    let bb = enregistrer_a(&mut b, &dir, "B");
    onglet_avec(&mut b, projet(50, 2));
    let c = enregistrer_a(&mut b, &dir, "C");
    // Rien de modifié : on quitte tout de suite.
    b.app.request(Pending::Quit);
    assert!(b.app.allow_close && b.app.groupe.is_none(), "rien à perdre : pas de question");
    b.app.allow_close = false;
    // Deux onglets modifiés, dont un caché : une seule boîte.
    modifier(&mut b, 51);
    assert!(b.app.activer_onglet(0));
    modifier(&mut b, 201);
    b.app.request(Pending::Quit);
    assert!(!b.app.allow_close && b.app.pending.is_none());
    let g = b.app.groupe.as_ref().expect("la boîte des projets non enregistrés");
    let mut listes: Vec<&str> = g.onglets.iter().map(|(_, n)| n.as_str()).collect();
    listes.sort_unstable();
    assert_eq!(listes, ["A", "C"]);
    assert!(b.app.modal_open(), "la boîte tient la main");
    // Annuler : rien ne bouge.
    b.app.groupe_choisir(2);
    assert!(b.app.groupe.is_none() && !b.app.allow_close);
    // Enregistrer tout : les deux projets sont écrits, puis SHOT quitte.
    b.app.request(Pending::Quit);
    b.app.groupe_choisir(0);
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.allow_close), "quitte une fois tout écrit : {}", b.app.status);
    assert_eq!((rouge_sur_disque(&a), rouge_sur_disque(&bb), rouge_sur_disque(&c)), (201, 90, 51));
    // Ne pas enregistrer : on quitte tel quel.
    b.app.allow_close = false;
    modifier(&mut b, 5);
    // Un seul onglet modifié : la question est celle de toujours, pour le projet affiché.
    b.app.request(Pending::Quit);
    assert!(b.app.groupe.is_none() && matches!(b.app.pending, Some(Pending::Quit)), "un seul projet à perdre : la boîte habituelle");
    b.app.pending = None;
    let autre = (b.app.actif + 1) % b.app.onglets.len();
    assert!(b.app.activer_onglet(autre));
    modifier(&mut b, 6);
    b.app.request(Pending::Quit);
    assert!(b.app.groupe.is_some());
    b.app.groupe_choisir(1);
    assert!(b.app.allow_close);
    assert_eq!(rouge_sur_disque(&a), 201, "le fichier n'a pas été touché");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_waits_for_a_save_in_a_hidden_tab() {
    let dir = dossier("onglets-quitter-ecriture");
    let mut b = banc(projet(200, 6), false);
    let a_path = dir.join("A").join("A.pelure");
    b.app.path = Some(a_path.clone());
    modifier(&mut b, 140);
    assert!(b.app.save());
    onglet_avec(&mut b, projet(90, 2));
    // L'écriture du premier onglet dure peut-être encore : quitter l'attend.
    b.app.request(Pending::Quit);
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.allow_close));
    b.app.fermeture();
    assert_eq!(rouge_sur_disque(&a_path), 140);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------- caches

/// Les fils de vignettes n'ont plus rien à faire pour l'onglet caché `i` (au plus dix secondes d'attente : un calcul
/// déjà commencé finit de lui-même quand il voit qu'on l'a abandonné).
fn calme_de_l_onglet(b: &Banc, i: usize) -> bool {
    let t = Instant::now();
    loop {
        let e = b.app.onglets[i].etat.as_ref().expect("rangé");
        if e.thumbs.outstanding() == 0 && !e.layer_thumbs.busy() {
            return true;
        }
        if t.elapsed() > Duration::from_secs(10) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Des images jusqu'à ce que les vignettes de l'onglet affiché soient faites ; leur teinte moyenne.
fn teinte_du_montage(b: &mut Banc) -> [f32; 3] {
    b.attendre_vignettes();
    b.chauffer(3);
    let id = b.app.doc.panels[0].id;
    for largeur in (64..=1024).step_by(64) {
        if let Some(image) = b.app.thumbs.image_of(id, largeur) {
            return teinte(image);
        }
    }
    panic!("aucune vignette du premier plan n'a été faite");
}

#[test]
fn thumbnails_never_show_another_projects_pixels() {
    crate::fxcache::monde::ecrire();
    let rouge_plein = Color32::from_rgb(220, 20, 20);
    let bleu_plein = Color32::from_rgb(20, 20, 220);
    // Deux projets qui ont les mêmes numéros de plan et de dessin, et des couleurs opposées.
    let mut b = banc(projet_plein(rouge_plein, 2), false);
    let a_rouge = teinte_du_montage(&mut b);
    assert!(a_rouge[0] > 150.0 && a_rouge[2] < 100.0, "le premier projet est rouge : {a_rouge:?}");
    let identiques = b.app.doc.panels[0].id;
    onglet_avec(&mut b, projet_plein(bleu_plein, 2));
    assert_eq!(b.app.doc.panels[0].id, identiques, "mêmes numéros de plan : les vignettes se rangent sous le même numéro");
    let b_bleu = teinte_du_montage(&mut b);
    assert!(b_bleu[2] > 150.0 && b_bleu[0] < 100.0, "le deuxième est bleu, pas rouge : {b_bleu:?}");
    // Retour au premier : ses vignettes sont les siennes, déjà faites.
    assert!(b.app.activer_onglet(0));
    let encore = teinte_du_montage(&mut b);
    assert!(encore[0] > 150.0 && encore[2] < 100.0, "le premier est resté rouge : {encore:?}");
    // Et le deuxième, rangé, a gardé les siennes.
    let range = b.app.onglets[1].etat.as_ref().expect("rangé");
    let id = range.doc.panels[0].id;
    let image = (64..=1024).step_by(64).find_map(|w| range.thumbs.image_of(id, w)).expect("vignette rangée");
    assert!(teinte(image)[2] > 150.0);
}

#[test]
fn thumbnails_asked_before_a_switch_never_answer_the_new_tab() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet_plein(Color32::from_rgb(220, 20, 20), 2), false);
    // Des demandes de vignettes partent (la bande vient d'être dessinée)…
    b.chauffer(2);
    // …et on change d'onglet sans les attendre : l'onglet qui part n'occupe plus les fils.
    assert!(b.app.nouvel_onglet());
    assert!(b.app.charger_projet(projet_plein(Color32::from_rgb(20, 20, 220), 2)));
    b.app.welcome = false;
    // (Un calcul déjà parti s'arrête de lui-même, dès qu'il voit son abandon : on lui laisse le temps de le voir.)
    assert!(calme_de_l_onglet(&b, 0), "plus de demande en file ni en calcul pour l'onglet caché");
    let bleu = teinte_du_montage(&mut b);
    assert!(bleu[2] > 150.0 && bleu[0] < 100.0, "{bleu:?}");
}

#[test]
fn derived_effect_drawings_never_cross_from_one_project_to_the_other() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet_plein(Color32::from_rgb(220, 20, 20), 1), false);
    let flou = crate::fx::EffectKind::GaussianBlur;
    b.app.toggle_fx(0, flou);
    let plan = b.app.build_plan(0, false, true);
    let item = plan.items.iter().find(|i| !i.fx.is_empty()).expect("un calque avec effet").clone();
    let id = crate::fx::shown_id(&item);
    let version = b.app.doc.cels[&item.cel].version;
    assert!(crate::fx::shown(&b.app.doc, &item).is_some(), "calculé");
    let a = crate::fxcache::get(id, version).expect("gardé dans le cache");
    // Un autre projet : mêmes numéros, mêmes effets, autre couleur.
    onglet_avec(&mut b, projet_plein(Color32::from_rgb(20, 20, 220), 1));
    assert!(crate::fxcache::get(id, version).is_none(), "le cache du premier projet n'est plus là pour le second");
    b.app.toggle_fx(0, flou);
    let plan = b.app.build_plan(0, false, true);
    let item_b = plan.items.iter().find(|i| !i.fx.is_empty()).expect("effet").clone();
    assert_eq!(crate::fx::shown_id(&item_b), id, "même numéro dérivé : c'est bien le piège");
    assert!(crate::fx::shown(&b.app.doc, &item_b).is_some());
    let bleu = crate::fxcache::get(id, b.app.doc.cels[&item_b.cel].version).expect("calculé pour le second");
    assert!(!Arc::ptr_eq(&a, &bleu));
    let centre = |c: &Cel| c.raster.get(100, 100);
    assert!(centre(&a).r() > 150 && centre(&bleu).b() > 150 && centre(&bleu).r() < 80, "chaque cache a ses couleurs");
    // Retour : le premier retrouve exactement ce qu'il avait calculé, sans recalcul.
    assert!(b.app.activer_onglet(0));
    let retrouve = crate::fxcache::get(id, version).expect("rendu à son onglet");
    assert!(Arc::ptr_eq(&a, &retrouve), "le même dessin dérivé, pas une copie recalculée");
}

#[test]
fn gpu_textures_follow_their_tab_and_are_not_sent_again() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet_plein(Color32::from_rgb(220, 20, 20), 2), true);
    if b.app.gpu.is_none() {
        println!("onglets : pas de carte graphique ici, essai des textures passé");
        return;
    }
    b.chauffer(4);
    let avant = b.app.gpu.as_ref().expect("gpu").stats();
    assert!(avant.textures > 0, "le premier projet a ses textures : {avant:?}");
    onglet_avec(&mut b, projet_plein(Color32::from_rgb(20, 20, 220), 2));
    b.chauffer(4);
    let milieu = b.app.gpu.as_ref().expect("gpu").stats();
    assert!(milieu.textures > 0 && milieu.bytes > 0, "le second a les siennes");
    let lu = b.app.gpu.as_ref().expect("gpu").read_frame();
    let centre = lu[lu.len() / 2];
    assert!(centre.b() > centre.r(), "l'image composée est celle du second projet : {centre:?}");
    assert!(b.app.activer_onglet(0));
    b.chauffer(4);
    let apres = b.app.gpu.as_ref().expect("gpu").stats();
    assert_eq!(apres.textures, avant.textures, "les textures du premier sont revenues");
    assert_eq!(apres.copies, milieu.copies, "aucune tuile n'a été renvoyée à la carte en revenant");
    let lu = b.app.gpu.as_ref().expect("gpu").read_frame();
    let centre = lu[lu.len() / 2];
    assert!(centre.r() > centre.b(), "l'image composée est de nouveau celle du premier projet : {centre:?}");
}

#[test]
fn the_caches_of_hidden_tabs_stay_within_the_effect_cache_limit() {
    crate::fxcache::monde::ecrire();
    // Rend la limite quoi qu'il arrive (elle est commune à tous les essais).
    struct Rend(usize);
    impl Drop for Rend {
        fn drop(&mut self) {
            crate::fxcache::set_limit(self.0);
        }
    }
    let _rend = Rend(crate::fxcache::limite_octets());
    let flou = crate::fx::EffectKind::GaussianBlur;
    let mut b = banc(projet_plein(Color32::from_rgb(220, 20, 20), 1), false);
    let calcule = |b: &mut Banc| {
        b.app.toggle_fx(0, flou);
        let plan = b.app.build_plan(0, false, true);
        let item = plan.items.iter().find(|i| !i.fx.is_empty()).expect("effet").clone();
        assert!(crate::fx::shown(&b.app.doc, &item).is_some());
    };
    calcule(&mut b);
    for _ in 0..3 {
        onglet_avec(&mut b, projet_plein(Color32::from_rgb(20, 20, 220), 1));
        calcule(&mut b);
    }
    // Quatre onglets, un dessin dérivé d'environ un mégaoctet chacun, gardés tant que la réserve le permet.
    let garde = |b: &Banc| b.app.onglets.iter().filter_map(|t| t.etat.as_ref()).filter_map(|e| e.reserve_fx.as_ref()).map(|r| r.bytes()).sum::<usize>();
    assert!(garde(&b) > 0, "la réserve est rangée avec les onglets cachés");
    crate::fxcache::set_limit(1 << 20);
    // Un changement d'onglet fait le ménage : les plus anciennement vus partent d'abord.
    assert!(b.app.activer_onglet(0));
    assert!(garde(&b) <= crate::fxcache::limite_octets(), "{} octets gardés pour une limite de {}", garde(&b), crate::fxcache::limite_octets());
    // Ce que l'onglet affiché a gardé n'est pas touché.
    assert!(b.app.doc.cels.len() == 1);
}

#[test]
fn hidden_tabs_run_nothing_and_hold_no_pending_work() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet_plein(Color32::from_rgb(220, 20, 20), 3), false);
    b.app.toggle_play_scope(true);
    assert!(b.app.playing);
    b.image(Vec::new(), 1.0 / 24.0);
    onglet_avec(&mut b, projet(1, 1));
    let range = b.app.onglets[0].etat.as_ref().expect("rangé");
    assert!(!range.playing, "la lecture s'arrête avec l'onglet");
    assert!(!range.rejeu.has_pending());
    assert!(calme_de_l_onglet(&b, 0), "plus de vignette en file ni en calcul pour l'onglet caché");
    let range = b.app.onglets[0].etat.as_ref().expect("rangé");
    // Des images pendant que l'autre onglet est affiché : l'onglet caché ne change pas d'un bit.
    let version_avant = range.doc.cels.values().map(|c| c.version).sum::<u64>();
    let serie_avant = range.history.serial();
    for _ in 0..30 {
        b.image(Vec::new(), 0.1);
    }
    let range = b.app.onglets[0].etat.as_ref().expect("rangé");
    assert_eq!((range.doc.cels.values().map(|c| c.version).sum::<u64>(), range.history.serial()), (version_avant, serie_avant));
    assert!(!range.playing && range.thumbs.outstanding() == 0);
}

// ---------------------------------------------------------------------------------------------- presse-papiers

/// Un projet de deux plans de 24 images.
fn projet_deux_plans(rouge: u8) -> Document {
    let mut doc = projet(rouge, 4);
    let i = doc.add_scene_after(0);
    doc.panels[i].layers = vec![Layer::new("Autre", LayerKind::Bitmap)];
    for p in &mut doc.panels {
        p.set_length(24);
    }
    doc
}

#[test]
fn copied_plans_paste_into_another_tab_as_independent_copies() {
    let mut b = banc(projet_deux_plans(200), false);
    let plans_a = b.app.doc.panels.len();
    b.app.board_sel = vec![b.app.doc.panels[0].id];
    b.app.panel_focus = true;
    b.app.copy_panels();
    assert!(b.app.panel_clipboard.is_some());
    onglet_avec(&mut b, projet_deux_plans(90));
    let plans_b = b.app.doc.panels.len();
    b.app.panel_focus = true;
    b.app.paste_panels();
    assert_eq!(b.app.doc.panels.len(), plans_b + 1, "{}", b.app.status);
    let rouges = |b: &Banc| -> Vec<CelId> {
        b.app.doc.panels.iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.frames.iter().flatten()).copied().filter(|id| b.app.doc.cels[id].raster.get(0, 0).r() == 200).collect()
    };
    let copies = rouges(&b);
    assert!(!copies.is_empty(), "les dessins du plan copié sont dans le projet qui reçoit");
    // Modifier la copie ne touche pas à l'original.
    b.app.doc.cels.get_mut(&copies[0]).expect("dessin").raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(1, 2, 3, 255);
    assert!(b.app.activer_onglet(0));
    assert_eq!(b.app.doc.panels.len(), plans_a);
    assert_eq!(rouge(&b), 200, "le premier projet est intact");
    // Et le presse-papiers sert encore (coller deux fois).
    assert!(b.app.panel_clipboard.is_some());
}

#[test]
fn plans_from_a_sheet_of_another_size_are_adapted_and_a_different_rate_is_flagged() {
    let mut b = banc(projet_deux_plans(200), false);
    b.app.board_sel = vec![b.app.doc.panels[0].id];
    b.app.copy_panels();
    // Une feuille d'une autre taille : adaptée (ex-refus, `collage.rs`).
    onglet_avec(&mut b, projet_grand());
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.collage.tache.is_none()), "{}", b.app.status);
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    assert!(b.app.status.contains("320×180") && b.app.status.contains("×2"), "le message dit d'où elle vient et l'échelle : {}", b.app.status);
    assert!(b.app.panel_clipboard.is_some(), "la copie reste, pour un projet de la bonne taille");
    // La même taille, une autre cadence : collé, et prévenu (sans la question, qu'on a coupée).
    let mut autre_cadence = projet_deux_plans(50);
    autre_cadence.fps = 25;
    onglet_avec(&mut b, autre_cadence);
    b.app.settings.collage.demander = false;
    let n = b.app.doc.panels.len();
    b.app.paste_panels();
    assert_eq!(b.app.doc.panels.len(), n + 1);
    assert!(b.app.status.contains("24 i/s") && b.app.status.contains("25 i/s"), "{}", b.app.status);
}

#[test]
fn a_copied_layer_pastes_into_another_tab_with_its_own_drawings() {
    let mut b = banc(projet(200, 3), false);
    b.app.copy_layer();
    onglet_avec(&mut b, projet(90, 3));
    let (calques, dessins) = (b.app.layers().len(), b.app.doc.cels.len());
    b.app.paste_layer();
    assert_eq!(b.app.layers().len(), calques + 1, "{}", b.app.status);
    assert!(b.app.doc.cels.len() > dessins, "les dessins du calque sont copiés dans le projet");
    let colle = &b.app.layers()[b.app.cur_layer];
    let id = colle.frames[0].expect("image");
    assert_eq!(b.app.doc.cels[&id].raster.get(0, 0).r(), 200, "le dessin collé est celui du premier projet");
    assert!(b.app.modified && b.app.history.serial() > 0, "une étape d'annulation");
    // Une feuille d'une autre taille : adapté (ex-refus, `collage.rs`).
    onglet_avec(&mut b, projet_grand());
    let calques = b.app.layers().len();
    b.app.paste_layer();
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.collage.tache.is_none()), "{}", b.app.status);
    assert_eq!(b.app.layers().len(), calques + 1, "{}", b.app.status);
    assert!(b.app.status.contains("320×180") && b.app.status.contains("×2"), "{}", b.app.status);
}

#[test]
fn copied_images_paste_into_another_tab() {
    let mut b = banc(projet(200, 4), false);
    b.app.frame_sel = Some((1, 3));
    b.app.images_focus = true;
    b.app.copy_images();
    onglet_avec(&mut b, projet(90, 4));
    b.app.images_focus = true;
    let avant = b.app.doc.cels.len();
    b.app.cur_frame = 0;
    b.app.paste_images(false);
    assert_eq!(b.app.doc.cels.len(), avant + 2, "{}", b.app.status);
    let layer = b.app.layer();
    let colles: Vec<u8> = layer.frames.iter().take(2).map(|f| b.app.doc.cels[&f.expect("image")].raster.get(0, 0).r()).collect();
    assert_eq!(colles.len(), 2);
    // Autre taille de feuille : adaptées (ex-refus, `collage.rs`).
    onglet_avec(&mut b, projet_grand());
    b.app.paste_images(false);
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.collage.tache.is_none()), "{}", b.app.status);
    assert!(b.app.status.contains("320×180") && b.app.status.contains("×2"), "{}", b.app.status);
    let colle = b.app.layer().frames[0].expect("image collée");
    assert_eq!(b.app.doc.cels[&colle].raster.width, 1280, "à la taille de la feuille qui reçoit");
}

#[test]
fn camera_keys_and_effects_travel_between_tabs() {
    use crate::camera::{Camera, CameraKey};
    let mut b = banc(projet_deux_plans(200), false);
    let cam = |x: f32| Camera { x, y: 50.0, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
    b.app.doc.panels[0].camera = vec![CameraKey::new(2, cam(10.0)), CameraKey::new(6, cam(30.0))];
    let id = b.app.doc.panels[0].id;
    b.app.cam_key_sel = vec![crate::camkeys::CamKeyRef { panel: id, frame: 2 }, crate::camkeys::CamKeyRef { panel: id, frame: 6 }];
    b.app.copy_cam_keys();
    assert!(b.app.cam_key_clipboard.is_some());
    // Un effet sur un calque.
    b.app.toggle_fx(0, crate::fx::EffectKind::GaussianBlur);
    b.app.fx_copy_all(0);
    assert!(b.app.fx_stack_clipboard.is_some());
    onglet_avec(&mut b, projet_deux_plans(90));
    assert!(b.app.cam_key_sel.is_empty(), "le choix de clés est celui de l'onglet");
    b.app.cur_frame = 3;
    b.app.paste_cam_keys_at_head();
    let frames: Vec<usize> = b.app.doc.panels[0].camera.iter().map(|k| k.frame).collect();
    // (Le plan a déjà sa clé de départ à l'image 0 ; les deux clés copiées arrivent sur la tête de lecture avec leur écart.)
    assert_eq!(frames, vec![0, 3, 7], "les clés copiées dans l'autre onglet, avec leur écart : {}", b.app.status);
    assert_eq!(b.app.doc.panels[0].camera[1].cam.x, 10.0);
    assert_eq!(b.app.doc.panels[0].camera[2].cam.x, 30.0);
    // Les effets : sur le calque courant du nouveau projet.
    let li = b.app.cur_layer;
    assert!(b.app.layers()[li].fx.is_empty());
    b.app.fx_paste(vec![li], false);
    assert_eq!(b.app.layers()[li].fx.len(), 1, "{}", b.app.status);
    assert_eq!(b.app.layers()[li].fx[0].kind, crate::fx::EffectKind::GaussianBlur);
}

#[test]
fn montage_keys_travel_between_tabs_and_adapt_to_another_sheet_size() {
    let mut b = banc(projet(200, 4), false);
    b.app.cur_frame = 0;
    b.app.copy_keys();
    assert!(b.app.key_clipboard.is_some(), "{}", b.app.status);
    onglet_avec(&mut b, projet(90, 4));
    b.app.cur_frame = 2;
    b.app.paste_keys_at_head();
    let id = b.app.layer().frames[2].expect("clé collée");
    assert_eq!(b.app.doc.cels[&id].raster.get(0, 0).r(), 200, "le dessin de la clé copiée dans l'autre onglet : {}", b.app.status);
    // Une feuille d'une autre taille : adaptées (ex-refus, `collage.rs`).
    onglet_avec(&mut b, projet_grand());
    b.app.paste_keys_at_head();
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.collage.tache.is_none()), "{}", b.app.status);
    assert!(b.app.status.contains("320×180") && b.app.status.contains("×2"), "{}", b.app.status);
}

#[test]
fn vector_elements_copied_in_one_tab_paste_into_another() {
    use crate::vector::{Element, VPoint};
    use crate::vtools::VSelection;
    let vecto = |rouge: u8| {
        let mut doc = Document::with_frame(320, 180, 24);
        let mut layer = Layer::new("Trait", LayerKind::Vector);
        let mut cel = Cel::new(doc.width, doc.height, LayerKind::Vector);
        cel.vector.as_mut().expect("dessin vectoriel").elements.push(Element::Stroke {
            points: vec![VPoint { x: 20.0, y: 40.0, r: 4.0 }, VPoint { x: 180.0, y: 40.0, r: 4.0 }],
            color: [rouge, 0, 0],
            opacity: 1.0,
            hardness: 0.9,
            brush: None,
            trace: Default::default(),
        });
        layer.frames.push(Some(doc.add_cel(cel)));
        doc.panels[0].layers = vec![layer];
        doc.panels[0].length = 1;
        doc
    };
    let mut b = banc(vecto(201), false);
    let cel = premier_dessin(&b);
    b.app.vsel = Some(VSelection { cel, items: vec![0] });
    assert!(b.app.copy_selection());
    onglet_avec(&mut b, vecto(5));
    b.app.paste();
    let id = premier_dessin(&b);
    let elements = &b.app.doc.cels[&id].vector.as_ref().expect("vectoriel").elements;
    assert_eq!(elements.len(), 2, "l'élément copié dans l'autre onglet est collé : {}", b.app.status);
    assert!(matches!(&elements[1], Element::Stroke { color, .. } if color[0] == 201));
}

#[test]
fn pixels_copied_in_one_tab_paste_into_another() {
    use crate::pixsel::Floating;
    let mut b = banc(projet(200, 2), false);
    b.app.pixel_clipboard = Some(Floating { x0: 4, y0: 4, w: 2, h: 2, pixels: vec![Color32::from_rgba_premultiplied(250, 0, 0, 255); 4] });
    onglet_avec(&mut b, projet(90, 2));
    b.app.bitmap_paste();
    let id = premier_dessin(&b);
    assert_eq!(b.app.doc.cels[&id].raster.get(4, 4).r(), 250, "la zone copiée dans l'autre onglet est posée : {}", b.app.status);
    assert!(b.app.bsel.is_some(), "elle reste soulevée, prête à être déplacée");
}

// ---------------------------------------------------------------------------------------------- clavier

#[test]
fn ctrl_tab_cycles_the_tabs_and_tab_alone_still_switches_the_mode() {
    let mut b = banc(projet(200, 2), false);
    // (Comme la vraie fenêtre : Tab est retiré à egui avant qu'il ne passe d'un champ à l'autre.)
    b.crochet = true;
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    assert_eq!(b.app.actif, 2);
    // Les raccourcis annoncés sont les bons, dans les deux modes.
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(b.app.keymap.find(Key::Tab, Some(Key::Tab), ctrl(), mode), Some("onglet.suivant"));
        assert_eq!(b.app.keymap.find(Key::Tab, Some(Key::Tab), Modifiers { shift: true, ..ctrl() }, mode), Some("onglet.precedent"));
        assert_eq!(b.app.keymap.find(Key::T, Some(Key::T), ctrl(), mode), Some("onglet.nouveau"));
        assert_eq!(b.app.keymap.find(Key::W, Some(Key::W), ctrl(), mode), Some("fichier.fermer"));
    }
    assert_eq!(b.app.keymap.find(Key::Tab, Some(Key::Tab), Modifiers::NONE, Mode::Storyboard), Some("mode.basculer"));
    // Au clavier : suivant (en boucle), précédent.
    touche(&mut b, Key::Tab, ctrl());
    assert_eq!(b.app.actif, 0, "après le dernier, le premier");
    touche(&mut b, Key::Tab, Modifiers { shift: true, ..ctrl() });
    assert_eq!(b.app.actif, 2, "avant le premier, le dernier");
    // Tab seul : toujours le mode (storyboard / animation), l'onglet ne bouge pas.
    let mode = b.app.mode;
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_ne!(b.app.mode, mode);
    assert_eq!(b.app.actif, 2);
    // Ctrl+T : un onglet vierge ; Ctrl+W : le ferme.
    touche(&mut b, Key::T, ctrl());
    assert_eq!((b.app.onglets.len(), b.app.actif), (4, 3));
    assert!(b.app.onglet_vierge());
    touche(&mut b, Key::W, ctrl());
    assert_eq!(b.app.onglets.len(), 3);
}

// ---------------------------------------------------------------------------------------------- la bande

fn places(b: &Banc) -> Vec<(u64, Rect, Rect)> {
    b.ctx.data(|d| d.get_temp::<Vec<(u64, Rect, Rect)>>(egui::Id::new("onglets_places"))).unwrap_or_default()
}

fn clic(b: &mut Banc, p: egui::Pos2, bouton: PointerButton) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: bouton, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: bouton, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
}

#[test]
fn the_strip_shows_each_tab_and_answers_clicks() {
    let dir = dossier("onglets-bande");
    let mut b = banc(projet(200, 2), false);
    enregistrer_a(&mut b, &dir, "Premier");
    onglet_avec(&mut b, projet(90, 2));
    enregistrer_a(&mut b, &dir, "Deuxieme");
    onglet_avec(&mut b, projet(50, 2));
    enregistrer_a(&mut b, &dir, "Troisieme");
    b.garder_textes = true;
    b.chauffer(3);
    for nom in ["Premier", "Deuxieme", "Troisieme"] {
        assert!(b.textes.iter().any(|t| t == nom), "l'onglet « {nom} » est dans la bande : {:?}", b.textes);
    }
    let p = places(&b);
    assert_eq!(p.len(), 3);
    // Clic sur le premier onglet : il passe à l'écran.
    clic(&mut b, p[0].1.center() - egui::vec2(20.0, 0.0), PointerButton::Primary);
    assert_eq!(b.app.actif, 0);
    // Clic du milieu sur le deuxième : il se ferme (propre, sans question).
    let p = places(&b);
    clic(&mut b, p[1].1.center() - egui::vec2(20.0, 0.0), PointerButton::Middle);
    assert_eq!(noms(&b), ["Premier", "Troisieme"]);
    assert!(b.app.pending.is_none());
    // La croix de l'onglet affiché le ferme aussi.
    let p = places(&b);
    clic(&mut b, p[0].2.center(), PointerButton::Primary);
    assert_eq!(noms(&b), ["Troisieme"]);
    // « + » : un onglet vierge. Le bouton est juste après le dernier onglet.
    let p = places(&b);
    let plus = pos2(p[0].1.right() + 22.0, p[0].1.center().y);
    clic(&mut b, plus, PointerButton::Primary);
    assert_eq!(b.app.onglets.len(), 2, "{:?}", noms(&b));
    assert!(b.app.onglet_vierge());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Peint la fenêtre avec trois onglets (un modifié, un survolé) dans un PNG, pour regarder la bande :
/// `SHOT_CAPTURE_DIR=… cargo test --release --offline --target-dir target\msvc capture_the_strip -- --ignored --nocapture`.
#[test]
#[ignore]
fn capture_the_strip() {
    let Ok(sortie) = std::env::var("SHOT_CAPTURE_DIR") else {
        println!("SHOT_CAPTURE_DIR n'est pas défini : rien à peindre");
        return;
    };
    let dir = dossier("onglets-capture");
    let mut b = banc(projet_plein(Color32::from_rgb(220, 90, 40), 3), true);
    enregistrer_a(&mut b, &dir, "La poursuite");
    onglet_avec(&mut b, projet_plein(Color32::from_rgb(40, 120, 220), 3));
    enregistrer_a(&mut b, &dir, "Bus de nuit (v2)");
    modifier(&mut b, 91);
    onglet_avec(&mut b, projet_plein(Color32::from_rgb(60, 180, 90), 3));
    b.app.activer_onglet(1);
    b.chauffer(6);
    let p = places(&b);
    // La souris sur le premier onglet (survol, croix visible).
    b.image(vec![Event::PointerMoved(p[0].1.center() - egui::vec2(30.0, 0.0))], 1.0 / 60.0);
    b.chauffer(3);
    let Some(rgba) = b.capturer() else {
        println!("pas de carte graphique : rien à peindre");
        return;
    };
    let fichier = Path::new(&sortie).join("onglets-bande.png");
    image::save_buffer(&fichier, &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    println!("ONGLETS capture : {}", fichier.display());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_modified_tab_is_marked_and_a_dialog_freezes_the_strip() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    modifier(&mut b, 91);
    let infos = b.app.infos_onglets();
    assert!(!infos[0].a_perdre() && infos[1].a_perdre());
    assert_eq!(b.app.bulle_onglet(&infos[1]), "Pas encore enregistré sur le disque. Modifié depuis le dernier enregistrement.");
    // Une boîte ouverte : la bande ne répond plus (ni clic, ni raccourci).
    b.app.pending = Some(Pending::Close);
    b.chauffer(2);
    let p = places(&b);
    clic(&mut b, p[0].1.center() - egui::vec2(20.0, 0.0), PointerButton::Primary);
    assert_eq!(b.app.actif, 1, "une boîte de dialogue tient la main");
    touche(&mut b, Key::Tab, ctrl());
    assert_eq!(b.app.actif, 1);
}

#[test]
fn tabs_can_be_reordered_and_the_shown_one_follows() {
    let mut b = banc(projet(200, 2), false);
    onglet_avec(&mut b, projet(90, 2));
    onglet_avec(&mut b, projet(50, 2));
    // Trois onglets : 200, 90, 50 ; le troisième est affiché.
    let ids: Vec<u64> = b.app.onglets.iter().map(|t| t.id).collect();
    b.app.deplacer_onglet(2, 0);
    assert_eq!(b.app.onglets.iter().map(|t| t.id).collect::<Vec<_>>(), vec![ids[2], ids[0], ids[1]]);
    assert_eq!(b.app.actif, 0, "l'onglet affiché a suivi son déplacement");
    assert_eq!(rouge(&b), 50);
    b.app.deplacer_onglet(1, 2);
    assert_eq!(b.app.actif, 0);
    assert_eq!(rouge(&b), 50);
    b.app.deplacer_onglet(0, 2);
    assert_eq!(b.app.actif, 2);
    assert_eq!(rouge(&b), 50);
    assert!(b.app.activer_onglet(0));
    assert_eq!(rouge(&b), 90, "chaque onglet garde son projet après les déplacements");
}

// ---------------------------------------------------------------------------------------------- session et secours

#[test]
fn the_session_is_remembered_and_comes_back_in_order() {
    let dir = dossier("onglets-session");
    let mut b = banc(projet(200, 2), false);
    b.app.welcome = true;
    let p1 = enregistrer_a(&mut b, &dir, "Un");
    onglet_avec(&mut b, projet(90, 2));
    let p2 = enregistrer_a(&mut b, &dir, "Deux");
    onglet_avec(&mut b, projet(50, 2));
    let p3 = enregistrer_a(&mut b, &dir, "Trois");
    assert!(b.app.activer_onglet(1));
    b.app.retenir_la_session();
    assert_eq!(b.app.settings.session_tabs, vec![p1.clone(), p2.clone(), p3.clone()]);
    assert_eq!(b.app.settings.session_active, 1);
    // Écrit dans les réglages : le lancement suivant les lit.
    let relus = crate::settings::Settings::load();
    assert_eq!((relus.session_tabs.clone(), relus.session_active), (vec![p1.clone(), p2.clone(), p3.clone()], 1));
    // Un SHOT neuf : tout revient, l'onglet affiché à la fin est le deuxième.
    let mut n = banc(projet(1, 1), false);
    n.app.welcome = true;
    n.app.doc = Document::with_frame(1920, 1080, 24);
    n.app.settings = relus;
    n.app.settings.reopen_last = true;
    n.app.reopen_last_project();
    assert!(jusqu_a(&mut n, Duration::from_secs(120), |n| n.app.session_prete && n.app.open_job.is_none() && n.app.reopen_probe.is_none() && n.app.onglets.len() == 3), "la session revient : {}", n.app.status);
    assert_eq!(noms(&n), ["Un", "Deux", "Trois"]);
    assert_eq!(n.app.actif, 1);
    assert_eq!(n.app.path.as_deref(), Some(p2.as_path()));
    // Un projet disparu est sauté sans bruit.
    std::fs::remove_dir_all(p3.parent().expect("dossier")).expect("effacé");
    let mut m = banc(projet(1, 1), false);
    m.app.welcome = true;
    m.app.doc = Document::with_frame(1920, 1080, 24);
    m.app.settings.session_tabs = vec![p1.clone(), p2.clone(), p3.clone()];
    m.app.settings.session_active = 2;
    m.app.settings.reopen_last = true;
    m.app.reopen_last_project();
    assert!(jusqu_a(&mut m, Duration::from_secs(120), |m| m.app.session_prete && m.app.open_job.is_none() && m.app.reopen_probe.is_none() && m.app.onglets.len() == 2));
    assert_eq!(noms(&m), ["Un", "Deux"]);
    // Le réglage éteint : rien ne revient.
    let mut o = banc(projet(1, 1), false);
    o.app.welcome = true;
    o.app.settings.session_tabs = vec![p1.clone()];
    o.app.settings.reopen_last = false;
    o.app.reopen_last_project();
    assert!(o.app.reopen_probe.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_session_is_not_overwritten_while_it_comes_back() {
    let dir = dossier("onglets-session-attente");
    let mut b = banc(projet(200, 2), false);
    let p1 = enregistrer_a(&mut b, &dir, "Un");
    b.app.retenir_la_session();
    assert_eq!(b.app.settings.session_tabs, vec![p1.clone()]);
    // Pendant la réouverture, `retenir_la_session` ne touche à rien (la liste serait réduite au premier projet).
    b.app.session_prete = false;
    b.app.settings.session_tabs = vec![p1.clone(), dir.join("Autre").join("Autre.pelure")];
    b.app.retenir_la_session();
    assert_eq!(b.app.settings.session_tabs.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_tab_has_its_own_rescue_copy_and_a_crash_offers_every_project() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet(200, 2), false);
    b.app.secours_actif = true;
    b.app.settings.autosave_minutes = 1;
    b.app.demarrer_secours();
    let racine = crate::recovery::root().expect("dossier des copies de secours (bac à sable)");
    assert!(b.app.recovery.is_some());
    modifier(&mut b, 201);
    onglet_avec(&mut b, projet(90, 2));
    assert!(b.app.recovery.is_some(), "le nouvel onglet a la sienne");
    modifier(&mut b, 91);
    // Changer d'onglet ne prend aucun cliché (il reste instantané) : le premier onglet, caché, a sa copie au rythme du
    // délai, comme l'onglet affiché. Ici le délai est nul.
    assert!(b.app.activer_onglet(0));
    assert!(b.app.activer_onglet(1));
    assert!(b.app.onglets[0].etat.as_ref().expect("rangé").recovery.as_ref().is_some_and(|r| !r.busy() && r.info.copied.is_none()), "rien n'a été copié par le changement lui-même");
    assert!(b.app.tick_secours_cache(0, Some(Duration::ZERO)).is_some(), "la copie du premier onglet part");
    // (Les images d'écran suivantes terminent la copie : `tick_onglets`.)
    assert!(
        jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.onglets[0].etat.as_ref().expect("rangé").recovery.as_ref().is_some_and(|r| !r.busy() && r.info.copied.is_some())),
        "la copie du premier onglet (caché) se termine"
    );
    // Le deuxième est affiché : sa copie est celle du délai ; on la demande.
    {
        let titre = b.app.base_name_or("Sans titre");
        let rec = b.app.recovery.as_mut().expect("copie du deuxième");
        let snap = crate::recovery::Snapshot { doc: &b.app.doc, serial: b.app.history.serial(), modified: true, title: titre, project: None, project_written: &b.app.written };
        rec.start_copy(snap);
    }
    assert!(jusqu_a(&mut b, Duration::from_secs(60), |b| b.app.recovery.as_ref().is_some_and(|r| !r.busy() && r.info.copied.is_some())), "la copie du deuxième se termine");
    // Un plantage : les verrous se libèrent sans que les dossiers soient effacés.
    let Banc { app, .. } = b;
    drop(app);
    let orphelins = crate::recovery::orphans(&racine, None);
    assert_eq!(orphelins.len(), 2, "chaque projet a sa copie : {orphelins:?}");
    // Le démarrage suivant les propose tous, chacun s'ouvre dans son onglet.
    let mut n = banc(projet(1, 1), false);
    n.app.welcome = true;
    n.app.doc = Document::with_frame(1920, 1080, 24);
    for o in orphelins {
        n.app.open_orphan(o);
    }
    n.chauffer(2);
    assert_eq!(n.app.onglets.len(), 2, "un onglet par copie");
    let mut rouges: Vec<u8> = Vec::new();
    for i in 0..2 {
        let doc = if i == n.app.actif { &n.app.doc } else { &n.app.onglets[i].etat.as_ref().expect("rangé").doc };
        let id = doc.panels[0].layers[0].frames[0].expect("dessin");
        rouges.push(doc.cels[&id].raster.get(0, 0).r());
    }
    rouges.sort_unstable();
    assert_eq!(rouges, vec![91, 201], "chaque copie a le dernier état de son projet");
    assert!(n.app.infos_onglets().iter().all(|i| i.modifie), "les copies ouvertes sont à enregistrer");
}

#[test]
fn closing_a_tab_removes_its_rescue_copy() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(projet(200, 2), false);
    b.app.secours_actif = true;
    b.app.demarrer_secours();
    onglet_avec(&mut b, projet(90, 2));
    let racine = crate::recovery::root().expect("racine");
    let contenu = || -> Vec<String> { std::fs::read_dir(&racine).map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default() };
    let avant = contenu();
    assert_eq!(avant.len(), 2, "{} : {avant:?}", racine.display());
    b.app.fermer_onglet(1);
    // (Effacée sur un fil : on attend un peu.)
    let t = Instant::now();
    while std::fs::read_dir(&racine).map(|d| d.count()).unwrap_or(0) > 1 && t.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(std::fs::read_dir(&racine).map(|d| d.count()).unwrap_or(0), 1, "le dossier de l'onglet fermé a disparu");
}

// ---------------------------------------------------------------------------------------------- repos et mesures

/// Attend que les onglets cachés n'aient plus de calcul de vignette en route (un calcul abandonné finit de lui-même et
/// réveille l'écran une dernière fois en finissant), puis que l'application se repose. Rend le nombre d'images.
fn au_calme(b: &mut Banc) -> usize {
    for i in 0..b.app.onglets.len() {
        if i != b.app.actif {
            assert!(calme_de_l_onglet(b, i), "l'onglet caché {i} finit ses calculs abandonnés");
        }
    }
    // (Le fil qui finit réveille l'écran juste après avoir quitté la file.)
    std::thread::sleep(Duration::from_millis(150));
    se_reposer(b, 3000)
}

#[test]
fn three_open_tabs_at_rest_ask_for_no_repaint() {
    crate::fxcache::monde::ecrire();
    let mut b = banc(crate::perf_tests::projet_long(12, 3, 24, 4), false);
    onglet_avec(&mut b, crate::perf_tests::projet_long(8, 2, 24, 3));
    onglet_avec(&mut b, crate::perf_tests::projet_long(6, 2, 12, 3));
    assert_eq!(b.app.onglets.len(), 3);
    assert!(au_calme(&mut b) < 3000, "SHOT finit par se reposer avec trois onglets");
    let repos: Vec<Duration> = (0..10).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    let pire = repos.iter().min().copied().unwrap_or(Duration::MAX);
    assert!(pire >= Duration::from_secs(1), "trois onglets au repos : SHOT demande à être repeint dans {pire:?}");
    // Après des allers-retours entre les onglets, le repos revient.
    for k in 0..6 {
        assert!(b.app.activer_onglet(k % 3));
        b.image(Vec::new(), 0.1);
    }
    assert!(au_calme(&mut b) < 3000, "SHOT se repose après des changements d'onglet");
    let repos: Vec<Duration> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(repos.iter().min().copied().unwrap_or(Duration::MAX) >= Duration::from_secs(1));
}

#[test]
fn switching_is_a_swap_not_a_reload() {
    // Un projet lourd : 30 plans de 24 images. Changer d'onglet ne copie aucun dessin.
    crate::fxcache::monde::ecrire();
    let mut b = banc(crate::perf_tests::projet_long(30, 3, 24, 6), false);
    onglet_avec(&mut b, crate::perf_tests::projet_long(30, 3, 24, 6));
    let cels = b.app.doc.cels.len();
    let t = Instant::now();
    let n = 200;
    for k in 0..n {
        assert!(b.app.activer_onglet(k % 2));
    }
    let moyenne = t.elapsed().as_secs_f64() * 1e3 / n as f64;
    println!("ONGLETS changement d'onglet (sans interface) : {moyenne:.3} ms en moyenne, {cels} dessins par projet");
    assert!(moyenne < 25.0, "changer d'onglet doit être instantané : {moyenne:.2} ms");
    assert_eq!(b.app.doc.cels.len(), cels, "rien n'a été perdu ni copié");
}

/// Mesure : temps d'un changement d'onglet (et de l'image d'écran qui suit), mémoire par projet ouvert.
/// `cargo test --release --offline --target-dir target\msvc perf_tabs -- --ignored --nocapture --test-threads=1`
#[test]
#[ignore]
fn perf_tabs() {
    crate::fxcache::monde::ecrire();
    fn memoire_mo() -> f64 {
        let sortie = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).WorkingSet64", std::process::id())])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&sortie.stdout).trim().parse::<f64>().unwrap_or(0.0) / 1048576.0
    }
    let avant = memoire_mo();
    let mut b = banc(crate::perf_tests::projet_long(30, 3, 24, 6), true);
    b.chauffer(10);
    let un_projet = memoire_mo();
    println!("ONGLETS mémoire : démarrage {avant:.0} Mo, un projet lourd ouvert {un_projet:.0} Mo");
    for k in 1..4 {
        onglet_avec(&mut b, crate::perf_tests::projet_long(30, 3, 24, 6));
        b.chauffer(10);
        let maintenant = memoire_mo();
        println!("ONGLETS mémoire : {} projets ouverts {maintenant:.0} Mo (+{:.0} Mo depuis le premier, {:.0} Mo par projet de plus)", k + 1, maintenant - un_projet, (maintenant - un_projet) / k as f64);
    }
    // Au repos avec quatre onglets : le processeur que le processus consomme en trois secondes (les fils de fond,
    // l'interface ne tourne pas pendant ce temps).
    fn processeur_ms() -> f64 {
        let sortie = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-Process -Id {}).TotalProcessorTime.TotalMilliseconds", std::process::id())])
            .output()
            .expect("powershell");
        String::from_utf8_lossy(&sortie.stdout).trim().replace(',', ".").parse::<f64>().unwrap_or(0.0)
    }
    assert!(se_reposer(&mut b, 3000) < 3000);
    let avant_cpu = processeur_ms();
    std::thread::sleep(Duration::from_secs(3));
    let apres_cpu = processeur_ms();
    println!("ONGLETS repos : quatre onglets ouverts, {:.0} ms de processeur en 3 s sans image d'écran", apres_cpu - avant_cpu);
    // Pour comparer : des images d'écran sans changer d'onglet.
    let mut calmes: Vec<f64> = (0..60).map(|_| b.image(Vec::new(), 1.0 / 60.0).total()).collect();
    calmes.sort_by(f64::total_cmp);
    println!("ONGLETS image d'écran sans changement d'onglet : médiane {:.2} ms (p95 {:.2})", calmes[30], calmes[57]);
    // Changer d'onglet : l'action seule, puis l'image d'écran qui montre le nouvel onglet.
    let mut actions = Vec::new();
    let mut images = Vec::new();
    for k in 0..60 {
        let t = Instant::now();
        assert!(b.app.activer_onglet(k % 4));
        actions.push(t.elapsed().as_secs_f64() * 1e3);
        let m = b.image(Vec::new(), 1.0 / 60.0);
        images.push(m.total());
    }
    // Avec la copie de secours active : changer d'onglet ne prend aucun cliché (elle suit le délai, `tick_onglets`).
    b.app.secours_actif = true;
    b.app.settings.autosave_minutes = 1;
    let mut avec_secours = Vec::new();
    for k in 0..24 {
        if k < 4 {
            b.app.demarrer_secours();
            b.app.activer_onglet(k % 4);
            b.app.demarrer_secours();
        }
        b.app.record_step();
        let t = Instant::now();
        assert!(b.app.activer_onglet((b.app.actif + 1) % 4));
        avec_secours.push(t.elapsed().as_secs_f64() * 1e3);
        b.image(Vec::new(), 1.0 / 60.0);
    }
    avec_secours.sort_by(f64::total_cmp);
    println!(
        "ONGLETS changement d'onglet depuis un onglet modifié, copie de secours active (aucun cliché au changement) : médiane {:.3} ms (max {:.3})",
        avec_secours[avec_secours.len() / 2],
        avec_secours[avec_secours.len() - 1]
    );
    b.app.secours_actif = false;
    actions.sort_by(f64::total_cmp);
    images.sort_by(f64::total_cmp);
    let med = |v: &Vec<f64>| v[v.len() / 2];
    println!(
        "ONGLETS changement d'onglet : action médiane {:.3} ms (max {:.3}) ; image d'écran qui suit médiane {:.2} ms (p95 {:.2}, max {:.2}, carte graphique : {})",
        med(&actions),
        actions[actions.len() - 1],
        med(&images),
        images[images.len() * 95 / 100],
        images[images.len() - 1],
        b.app.gpu.is_some()
    );
}
