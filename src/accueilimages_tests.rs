//! Les images d'accueil (`accueilimages.rs`) : le tirage (dossier vide, image valide, illisible, jamais deux fois la même),
//! l'absence de décodage à chaque image, l'import (copie, suffixe « _2 »), le retrait, le réglage du dossier et les Préférences.
//! Les essais n'écrivent jamais dans le vrai dossier d'Anthony : un dossier temporaire par essai.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::accueilimages::{self as ai, Tirage};
use crate::document::Document;
use crate::perf_tests::{banc_taille, Banc};
use crate::prefsonglets::{chercher, Groupe, Onglet};

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-accueilimg-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn png(dir: &Path, nom: &str, l: u32, h: u32, rvb: [u8; 3]) -> PathBuf {
    let p = dir.join(nom);
    image::RgbaImage::from_pixel(l, h, image::Rgba([rvb[0], rvb[1], rvb[2], 255])).save(&p).unwrap();
    p
}

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(160, 90, 24), false, 1600, 900);
    b.app.welcome = true;
    b.chauffer(4);
    b
}

/// Fait tourner des images jusqu'à ce que `fini` soit vrai (les fils de travail répondent ensuite).
fn attendre(b: &mut Banc, fini: impl Fn(&Banc) -> bool) {
    let debut = Instant::now();
    while !fini(b) && debut.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(5));
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert!(fini(b), "le fil de travail n'a pas répondu à temps");
}

fn tirage_fini(b: &Banc) -> bool {
    !matches!(b.app.confort.accueil.tirage, Tirage::Attente(_) | Tirage::AFaire)
}

/// Pointe l'accueil sur ce dossier et attend le tirage.
fn tirer_dans(b: &mut Banc, d: &Path) {
    b.app.definir_dossier_accueil(Some(d.to_path_buf()));
    b.chauffer(2);
    attendre(b, tirage_fini);
}

// ------------------------------------------------------------------------------------ le dossier et les fichiers

#[test]
fn the_default_folder_is_documents_shot_identite_accueil_and_never_the_real_one_in_tests() {
    // (En essai : le bac à sable du fil, jamais le vrai dossier d'Anthony.)
    assert!(ai::dossier_par_defaut().ends_with("Accueil"));
    assert!(!ai::dossier_par_defaut().starts_with(r"C:\Users\ferre\Documents"), "{:?}", ai::dossier_par_defaut());
    // Hors essai, c'est Documents\SHOT\Identité\Accueil, calculé comme le dossier des modèles.
    let attendu = crate::nouveau::default_folder().join("SHOT").join("Identité").join("Accueil");
    assert!(attendu.ends_with(Path::new("SHOT").join("Identité").join("Accueil")));
}

#[test]
fn the_list_keeps_only_png_and_jpeg_files_of_the_folder_in_name_order() {
    let d = dossier("liste");
    png(&d, "b.png", 8, 4, [1, 2, 3]);
    png(&d, "A.PNG", 8, 4, [1, 2, 3]);
    image::RgbImage::from_pixel(8, 4, image::Rgb([9, 9, 9])).save(d.join("c.jpg")).unwrap();
    image::RgbImage::from_pixel(8, 4, image::Rgb([9, 9, 9])).save(d.join("d.JPEG")).unwrap();
    std::fs::write(d.join("notes.txt"), "pas une image").unwrap();
    std::fs::create_dir_all(d.join("sous.png")).unwrap();
    png(&d.join("sous.png"), "cachee.png", 4, 4, [0, 0, 0]);
    let noms: Vec<String> = ai::lister(&d).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    assert_eq!(noms, ["A.PNG", "b.png", "c.jpg", "d.JPEG"]);
    assert!(ai::lister(&d.join("absent")).is_empty(), "un dossier absent : rien, sans erreur");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_draw_never_repeats_the_last_image_when_there_is_another() {
    let images: Vec<PathBuf> = ["a.png", "b.png", "c.png"].iter().map(PathBuf::from).collect();
    for h in 0..50u64 {
        assert_ne!(ai::tirer(&images, Some(Path::new("b.png")), h).unwrap(), Path::new("b.png"));
    }
    let seule = [PathBuf::from("a.png")];
    assert_eq!(ai::tirer(&seule, Some(Path::new("a.png")), 7).unwrap(), Path::new("a.png"), "une seule image : la même");
    assert!(ai::tirer(&[], None, 3).is_none());
    // Tous les choix sortent, selon le nombre tiré.
    let vus: std::collections::BTreeSet<_> = (0..10u64).filter_map(|h| ai::tirer(&images, None, h)).collect();
    assert_eq!(vus.len(), 3);
}

// ------------------------------------------------------------------------------------ le tirage de l'accueil

#[test]
fn an_empty_or_missing_folder_shows_the_logo_without_any_error() {
    let mut b = banc();
    let d = dossier("vide");
    tirer_dans(&mut b, &d);
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::Logo), "dossier vide : le logo");
    assert!(!b.app.status.contains("rreur"), "aucune erreur montrée : {:?}", b.app.status);
    tirer_dans(&mut b, &d.join("n-existe-pas"));
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::Logo), "dossier absent : le logo");
    assert!(b.app.welcome_shown());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_valid_image_in_the_folder_replaces_the_logo_and_is_kept_as_a_texture() {
    let mut b = banc();
    let d = dossier("valide");
    png(&d, "plan.png", 320, 180, [200, 30, 30]);
    tirer_dans(&mut b, &d);
    match &b.app.confort.accueil.tirage {
        Tirage::Image(_, taille) => assert_eq!(*taille, [320, 180], "l'image, à sa taille (elle est plus petite que le maximum)"),
        _ => panic!("une image valide : elle remplace le logo"),
    }
    assert_eq!(b.app.confort.accueil.derniere.as_deref(), Some(d.join("plan.png").as_path()));
    // Une grande image est réduite hors du fil de l'interface, à 900 pixels de large au plus.
    png(&d, "grande.png", 3200, 1800, [10, 10, 200]);
    std::fs::remove_file(d.join("plan.png")).unwrap();
    b.app.confort.accueil.oublier_le_tirage();
    b.chauffer(2);
    attendre(&mut b, tirage_fini);
    match &b.app.confort.accueil.tirage {
        Tirage::Image(_, [l, h]) => assert!(*l <= 900 && *h <= 900 && (*l as f32 / *h as f32 - 16.0 / 9.0).abs() < 0.02, "{l}x{h}"),
        _ => panic!("la grande image doit aussi s'afficher"),
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn an_unreadable_image_falls_back_to_the_logo_and_a_good_one_next_to_it_is_found() {
    let mut b = banc();
    let d = dossier("illisible");
    std::fs::write(d.join("cassee.png"), b"ceci n'est pas un png").unwrap();
    tirer_dans(&mut b, &d);
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::Logo), "image illisible : le logo");
    // Avec une bonne image à côté, le tirage tombe dessus, quelle que soit celle qu'il prend d'abord.
    png(&d, "bonne.png", 64, 36, [30, 200, 30]);
    for _ in 0..4 {
        b.app.confort.accueil.oublier_le_tirage();
        b.app.confort.accueil.derniere = None;
        b.chauffer(2);
        attendre(&mut b, tirage_fini);
        assert!(matches!(b.app.confort.accueil.tirage, Tirage::Image(..)), "la bonne image est trouvée");
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn two_images_take_turns_from_one_appearance_of_the_welcome_to_the_next() {
    let mut b = banc();
    let d = dossier("tour");
    png(&d, "un.png", 64, 36, [255, 0, 0]);
    png(&d, "deux.png", 64, 36, [0, 0, 255]);
    tirer_dans(&mut b, &d);
    let mut montrees = vec![b.app.confort.accueil.derniere.clone().unwrap()];
    for _ in 0..4 {
        // Un projet s'ouvre (l'accueil s'en va), puis se ferme (l'accueil revient) : un nouveau tirage.
        b.app.welcome = false;
        b.chauffer(2);
        assert!(matches!(b.app.confort.accueil.tirage, Tirage::AFaire), "l'accueil parti : le tirage est oublié");
        b.app.welcome = true;
        b.chauffer(2);
        attendre(&mut b, tirage_fini);
        montrees.push(b.app.confort.accueil.derniere.clone().unwrap());
    }
    assert!(montrees.windows(2).all(|w| w[0] != w[1]), "jamais deux fois la même de suite : {montrees:?}");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_image_is_drawn_once_per_appearance_not_at_every_frame() {
    let mut b = banc();
    let d = dossier("unefois");
    png(&d, "plan.png", 160, 90, [120, 120, 40]);
    let avant = b.app.confort.accueil.tirages;
    tirer_dans(&mut b, &d);
    let apres_tirage = b.app.confort.accueil.tirages;
    assert_eq!(apres_tirage, avant + 1, "un seul tirage pour ce dossier");
    for _ in 0..60 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert_eq!(b.app.confort.accueil.tirages, apres_tirage, "soixante images de plus : aucun nouveau tirage, aucun nouveau décodage");
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::Image(..)));
    // La fenêtre « Nouveau projet » passe par-dessus puis est annulée : l'image reste, rien n'est redécodé.
    b.app.run_action("fichier.nouveau");
    b.chauffer(4);
    assert!(b.app.new_dialog.is_some());
    b.app.new_dialog = None;
    b.app.welcome = true;
    b.chauffer(4);
    assert!(b.app.welcome_shown());
    assert_eq!(b.app.confort.accueil.tirages, apres_tirage);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_ui_thread_is_not_blocked_by_the_decoding() {
    let mut b = banc();
    let d = dossier("attente");
    png(&d, "plan.png", 160, 90, [1, 1, 1]);
    b.app.definir_dossier_accueil(Some(d.clone()));
    // La première image après le changement lance le fil et rend la main : le tirage est « en attente » ou déjà fini.
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!matches!(b.app.confort.accueil.tirage, Tirage::AFaire));
    attendre(&mut b, tirage_fini);
    std::fs::remove_dir_all(&d).ok();
}

// ------------------------------------------------------------------------------------ l'import et le retrait

#[test]
fn importing_copies_into_the_folder_creating_it_and_a_taken_name_gets_a_suffix() {
    let source = dossier("source");
    let cible = dossier("cible").join("pas-encore");
    let a = png(&source, "plan.png", 32, 18, [5, 5, 5]);
    let a_octets = std::fs::read(&a).unwrap();
    let bilan = ai::importer(&cible, &[a.clone()]);
    assert_eq!(bilan.copies, [cible.join("plan.png")], "le dossier est créé, l'image y est copiée");
    assert!(bilan.erreurs.is_empty());
    assert!(a.is_file(), "la source ne bouge pas");
    // Même nom : « _2 », puis « _3 ».
    let bilan = ai::importer(&cible, &[a.clone()]);
    assert_eq!(bilan.copies, [cible.join("plan_2.png")]);
    let bilan = ai::importer(&cible, &[a.clone(), a.clone()]);
    assert_eq!(bilan.copies, [cible.join("plan_3.png"), cible.join("plan_4.png")]);
    assert_eq!(std::fs::read(cible.join("plan_2.png")).unwrap(), a_octets, "la copie est exacte");
    assert_eq!(ai::lister(&cible).len(), 4);
    // Une copie ne s'écrase jamais.
    std::fs::write(cible.join("plan_5.png"), b"a moi").unwrap();
    let bilan = ai::importer(&cible, &[a.clone(), a.clone()]);
    assert_eq!(bilan.copies, [cible.join("plan_6.png"), cible.join("plan_7.png")]);
    assert_eq!(std::fs::read(cible.join("plan_5.png")).unwrap(), b"a moi");
    // Un fichier qui n'est pas une image, ou déjà dans le dossier, est laissé de côté.
    let texte = source.join("notes.txt");
    std::fs::write(&texte, "x").unwrap();
    let bilan = ai::importer(&cible, &[texte, cible.join("plan.png")]);
    assert!(bilan.copies.is_empty());
    assert_eq!(bilan.ignores, 2);
    assert!(!cible.join("notes.txt").exists());
    std::fs::remove_dir_all(&source).ok();
    std::fs::remove_dir_all(cible.parent().unwrap()).ok();
}

#[test]
fn removing_deletes_only_an_image_of_the_folder_and_nothing_else() {
    let d = dossier("retrait");
    let ailleurs = dossier("ailleurs");
    let a = png(&d, "a.png", 8, 8, [1, 1, 1]);
    let texte = d.join("notes.txt");
    std::fs::write(&texte, "garde-moi").unwrap();
    let hors = png(&ailleurs, "hors.png", 8, 8, [1, 1, 1]);
    std::fs::create_dir_all(d.join("sous")).unwrap();
    let dans_sous = png(&d.join("sous"), "s.png", 8, 8, [1, 1, 1]);
    assert!(ai::retirer(&d, &texte).is_err() && texte.is_file(), "pas une image : intouchable");
    assert!(ai::retirer(&d, &hors).is_err() && hors.is_file(), "une image d'un autre dossier : intouchable");
    assert!(ai::retirer(&d, &dans_sous).is_err() && dans_sous.is_file(), "une image d'un sous-dossier : intouchable");
    assert!(ai::retirer(&d, &d.join("sous")).is_err() && d.join("sous").is_dir(), "un dossier : intouchable");
    assert!(ai::retirer(&d, &d.join("absente.png")).is_err());
    assert!(ai::retirer(&d, &a).is_ok() && !a.exists());
    assert!(texte.is_file());
    std::fs::remove_dir_all(&d).ok();
    std::fs::remove_dir_all(&ailleurs).ok();
}

#[test]
fn importing_and_removing_from_the_app_update_the_list_the_message_and_the_next_draw() {
    let mut b = banc();
    let source = dossier("app-source");
    let d = dossier("app-dossier");
    b.app.definir_dossier_accueil(Some(d.clone()));
    let p = png(&source, "premiere.png", 64, 36, [9, 99, 9]);
    b.app.importer_images_accueil(std::slice::from_ref(&p));
    assert_eq!(ai::lister(&d).len(), 1);
    assert!(b.app.confort.accueil.message.as_deref().unwrap().contains("1 image copiée"), "{:?}", b.app.confort.accueil.message);
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::AFaire), "l'accueil tirera parmi les nouvelles images");
    b.chauffer(2);
    attendre(&mut b, tirage_fini);
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::Image(..)));
    b.app.importer_images_accueil(std::slice::from_ref(&p));
    assert!(d.join("premiere_2.png").is_file());
    // Retirer : la question d'abord (l'état), puis le fichier part.
    b.app.confort.accueil.retrait = Some(d.join("premiere.png"));
    b.app.retirer_image_accueil(&d.join("premiere.png"));
    assert!(!d.join("premiere.png").exists() && d.join("premiere_2.png").is_file());
    assert!(b.app.confort.accueil.retrait.is_none());
    assert!(b.app.confort.accueil.message.as_deref().unwrap().contains("retirée"));
    // Retirer autre chose que ces images est refusé.
    b.app.retirer_image_accueil(&p);
    assert!(p.is_file(), "la source de l'import n'est jamais touchée");
    std::fs::remove_dir_all(&source).ok();
    std::fs::remove_dir_all(&d).ok();
}

// ------------------------------------------------------------------------------------ le réglage du dossier

#[test]
fn the_folder_setting_is_kept_for_this_computer_and_resets_the_draw() {
    let mut b = banc();
    let d = dossier("reglage");
    assert_eq!(b.app.dossier_accueil(), ai::dossier_par_defaut(), "d'origine : le dossier d'Identité\\Accueil");
    assert!(b.app.settings.dossier_accueil.is_empty());
    tirer_dans(&mut b, &d);
    assert_eq!(b.app.dossier_accueil(), d);
    assert!(tirage_fini(&b));
    // Enregistré dans les réglages de ce poste : relu par le lancement suivant.
    assert_eq!(crate::settings::Settings::load().dossier_accueil, d.to_string_lossy());
    // Le changer redemande un tirage ; « Dossier d'origine » rend le dossier de départ et vide le réglage.
    b.app.definir_dossier_accueil(None);
    assert!(matches!(b.app.confort.accueil.tirage, Tirage::AFaire));
    assert_eq!(b.app.dossier_accueil(), ai::dossier_par_defaut());
    assert!(crate::settings::Settings::load().dossier_accueil.is_empty());
    // Un réglage absent des anciens fichiers : le dossier d'origine.
    let ancien: crate::settings::Settings = toml::from_str("").unwrap();
    assert!(ancien.dossier_accueil.is_empty());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_preferences_find_the_welcome_images_group_in_the_appearance_tab() {
    for requete in ["images d'accueil", "importer des images", "accueil", "logo"] {
        let r = chercher(requete);
        assert!(r.iter().any(|t| t.groupe == Groupe::ImagesAccueil && t.onglet == Onglet::Apparence), "« {requete} » : {:?}", r.iter().map(|t| t.groupe).collect::<Vec<_>>());
    }
    assert_eq!(Groupe::ImagesAccueil.titre(), "Images d'accueil");
}

// ------------------------------------------------------------------------------------ les Préférences

fn textes_vus(b: &Banc) -> Vec<String> {
    fn rec(s: &eframe::egui::epaint::Shape, out: &mut Vec<String>) {
        match s {
            eframe::egui::epaint::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            eframe::egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| rec(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &b.formes {
        rec(&s.shape, &mut out);
    }
    out
}

/// Tout ce que la fenêtre Préférences montre en défilant de haut en bas (le contenu hors de la vue n'est pas peint).
fn parcourir(b: &mut Banc) -> Vec<String> {
    use eframe::egui::{pos2, vec2, Event, Modifiers, MouseWheelUnit};
    let mut vus = Vec::new();
    b.chauffer(3);
    for _ in 0..14 {
        vus.extend(textes_vus(b));
        let fen = b.ctx.memory(|m| m.area_rect(eframe::egui::Id::new("Préférences"))).expect("la fenêtre Préférences est ouverte");
        let p = pos2(fen.max.x - 70.0, fen.center().y);
        b.image(vec![Event::PointerMoved(p), Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, -260.0), modifiers: Modifiers::NONE }], 1.0 / 60.0);
        b.chauffer(3);
    }
    vus
}

#[test]
fn the_preferences_list_the_images_with_thumbnails_decoded_once_and_a_remove_button_each() {
    let mut b = banc();
    b.garder_formes = true;
    let d = dossier("prefs");
    png(&d, "un.png", 160, 90, [200, 0, 0]);
    png(&d, "deux.png", 90, 160, [0, 200, 0]);
    std::fs::write(d.join("cassee.png"), b"pas un png").unwrap();
    b.app.definir_dossier_accueil(Some(d.clone()));
    b.app.settings.prefs_onglet = Onglet::Apparence;
    b.app.run_action("fichier.preferences");
    b.chauffer(6);
    attendre(&mut b, |b| b.app.confort.accueil.miniatures_decodees == 3);
    b.chauffer(3);
    let m = &b.app.confort.accueil.miniatures;
    assert_eq!(m.len(), 3);
    assert!(m[&d.join("un.png")].is_some() && m[&d.join("deux.png")].is_some(), "deux miniatures");
    assert!(m[&d.join("cassee.png")].is_none(), "l'illisible n'a pas de miniature, et ne casse rien");
    let t = parcourir(&mut b);
    assert!(t.iter().any(|x| x.contains("Sans image, l'accueil montre le logo.")), "la phrase du logo : {t:?}");
    assert!(t.iter().any(|x| x == "Importer des images…") && t.iter().any(|x| x == "Changer…"));
    assert!(t.iter().any(|x| x == "Retirer"), "un « Retirer » par image : {t:?}");
    assert!(t.iter().any(|x| x == "un.png") && t.iter().any(|x| x == "deux.png"));
    // Soixante images de plus : rien n'est redécodé.
    for _ in 0..60 {
        b.image(Vec::new(), 1.0 / 60.0);
    }
    assert_eq!(b.app.confort.accueil.miniatures_decodees, 3, "les miniatures sont décodées une fois et gardées");
    // Une image ajoutée depuis l'Explorateur apparaît (la liste est relue de temps en temps) et seule elle est décodée.
    png(&d, "trois.png", 64, 36, [0, 0, 200]);
    b.app.confort.accueil.oublier_la_liste();
    attendre(&mut b, |b| b.app.confort.accueil.miniatures_decodees == 4);
    assert_eq!(b.app.confort.accueil.miniatures.len(), 4);
    // Une image retirée du dossier perd sa miniature.
    std::fs::remove_file(d.join("deux.png")).unwrap();
    b.app.confort.accueil.oublier_la_liste();
    b.chauffer(3);
    assert!(!b.app.confort.accueil.miniatures.contains_key(&d.join("deux.png")));
    assert_eq!(b.app.confort.accueil.miniatures_decodees, 4);
    b.app.prefs_window = false;
    std::fs::remove_dir_all(&d).ok();
}
