//! Tests de la relecture par le réalisateur : la page HTML (échappement, hors ligne, tous les plans), le fichier de
//! notes qui revient (format, limites, dessins), le placement des notes sur les plans (identifiant, numéro,
//! sans plan), l'importation (idempotente, annulable), le PDF avec commentaires et cadres de notes, la tâche
//! d'export et les fenêtres.

use std::time::Duration;

use eframe::egui;

use crate::app::App;
use crate::commentaires::{self, Annotation, Comment};
use crate::document::{Document, ImageMark};
use crate::relecture::{self, Meta, Notes, Options, Via};
use crate::relecturejob::RelectureJob;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("shot-relecture-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Trois plans de 10, 20 et 30 images (24 i/s), le deuxième avec une marque d'image à l'image 8.
fn film() -> Document {
    let mut doc = Document::new(32, 32, 24);
    doc.add_scene_after(0);
    doc.add_scene_after(1);
    for (p, n) in doc.panels.iter_mut().zip([10, 20, 30]) {
        p.set_length(n);
    }
    doc.panels[0].action = "Il entre.".into();
    doc.panels[1].dialogue = "— Bonjour !".into();
    let mark = doc.alloc();
    doc.panels[1].marks.push(ImageMark { id: mark, frame: 8, color: 0, action: "Elle sourit.".into(), dialogue: String::new(), notes: "Gros plan".into() });
    doc
}

fn meta() -> Meta {
    Meta { title: "Mon film".into(), project: "Mon film".into(), paquet: "abc123".into(), envoi: Some("v2 au réal".into()), now: 1_760_000_000, nonce: "nonce0123456789".into(), file_stem: "notes-Mon_film".into(), ..Default::default() }
}

/// Un faux JPEG distinct par rang (la page ne le décode pas).
fn fake_images(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| vec![0xFF, 0xD8, i as u8, 7, 8, 9, 0xFF, 0xD9]).collect()
}

fn page_of(doc: &Document, opts: &Options) -> String {
    let panels: Vec<usize> = (0..doc.panels.len()).collect();
    let (pkg, frames) = relecture::build(doc, &panels, opts, meta());
    relecture::page(&pkg, &fake_images(frames.len()))
}

/// Un PNG `w` × `h` : transparent partout sauf un trait rouge.
fn png_with_stroke(w: usize, h: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; w * h * 4];
    for x in 0..w {
        let y = h / 2;
        rgba[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[214, 38, 28, 255]);
    }
    crate::io::encode_png(&rgba, w, h).unwrap()
}

fn data_url(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", crate::base64::encode(png))
}

/// Un fichier de notes, en JSON, avec `notes` (déjà en JSON).
fn notes_json(notes: &str) -> String {
    format!(r#"{{"format":"shot-relecture","version":1,"paquet":"abc123","projet":"Mon film","relecteur":"Claire","cree":1760000500,"general":"Très bien.","notes":[{notes}]}}"#)
}

fn note_json(id: &str, plan: u64, image: u64, numero: &str, texte: &str) -> String {
    format!(r#"{{"id":"{id}","plan":"{plan}","image":"{image}","numero":"{numero}","titre":"Plan {numero}","texte":"{texte}"}}"#)
}

// ------------------------------------------------------------------------------------ la page

#[test]
fn text_of_the_project_is_escaped_for_html_and_attributes() {
    let s = esc_all("<script>alert(1)</script> & \"guillemets\" 'apostrophes' `accent` \u{7}contrôle\r\nligne");
    assert!(!s.contains('<') && !s.contains('>') && !s.contains('"') && !s.contains('\'') && !s.contains('`'), "{s}");
    assert!(s.contains("&lt;script&gt;") && s.contains("&amp;") && s.contains("&quot;") && s.contains("&#39;"), "{s}");
    assert!(!s.contains('\u{7}') && !s.contains('\r'), "les caractères de contrôle partent");
    assert!(s.contains('\n'), "les retours à la ligne restent (la page les montre)");
}

fn esc_all(s: &str) -> String {
    relecture::esc(s)
}

#[test]
fn the_page_has_every_plan_with_its_number_version_duration_and_texts() {
    let mut doc = film();
    // Le plan 2 a deux versions : « V2 » seulement pour lui.
    doc.nouvelle_version(1, 5).expect("version");
    let page = page_of(&doc, &Options::default());
    for (i, p) in doc.panels.iter().enumerate() {
        assert!(page.contains(&format!("id=\"p{}\"", p.id)), "plan {i} absent");
        assert!(page.contains(&format!("href=\"#p{}\"", p.id)), "lien du sommaire du plan {i}");
        assert!(page.contains(&format!("data-plan=\"{}\"", p.id)));
    }
    assert!(page.contains("Plan 1"), "{}", &page[..200.min(page.len())]);
    assert_eq!(page.matches("<span class=\"version\">V2</span>").count(), 1, "une seule étiquette de version : celle du plan à deux versions");
    assert_eq!(page.matches("<span class=\"version\">").count(), 1);
    assert!(page.contains("Il entre.") && page.contains("— Bonjour !"));
    // Le plan 2 a une marque d'image : deux images de storyboard, la seconde avec ses propres textes.
    assert!(page.contains("Image 1 sur 2") && page.contains("Image 2 sur 2"));
    assert!(page.contains("Elle sourit.") && page.contains("Gros plan"));
    // Une carte par image de storyboard : 4 (1 + 2 + 1).
    assert_eq!(page.matches("<section class=\"carte\"").count(), 4);
    assert!(page.contains("Séquence 1"));
    assert!(page.contains("id=\"relecteur\"") && page.contains("id=\"enregistrer\"") && page.contains("id=\"general\""));
}

#[test]
fn the_page_works_offline_and_forbids_anything_else() {
    let page = page_of(&film(), &Options { playback: true, ..Options::default() });
    assert!(!page.contains("http://") && !page.contains("https://") && !page.contains("//cdn"), "aucune adresse du dehors");
    assert!(!page.contains("xmlns"), "pas d'espace de noms SVG qui ressemble à une adresse");
    for forbidden in ["<link", "<iframe", "<object", "<embed", "<form", "<base", "src=\"//", "@import", "url(", "fetch(", "XMLHttpRequest", "sendBeacon", "WebSocket", "eval(", "innerHTML", "outerHTML", "document.write", "importScripts", "new Function"] {
        assert!(!page.contains(forbidden), "{forbidden} ne doit pas être dans la page");
    }
    assert!(page.contains("Content-Security-Policy") && page.contains("default-src 'none'") && page.contains("script-src 'nonce-nonce0123456789'"));
    assert_eq!(page.matches("<script").count(), 1, "un seul script, celui de SHOT");
    assert!(page.contains("<script nonce=\"nonce0123456789\">"));
    // Chaque image est dans la page.
    assert!(page.matches("src=\"data:image/jpeg;base64,").count() >= 4);
    for img in page.split("src=\"").skip(1) {
        assert!(img.starts_with("data:image/jpeg;base64,"), "toute source d'image est embarquée : {}", &img[..30.min(img.len())]);
    }
}

#[test]
fn hostile_titles_and_texts_cannot_break_out_of_the_page() {
    let mut doc = film();
    let evil = "</script><script>alert(1)</script><img src=x onerror=alert(2)>\" onmouseover=\"alert(3)";
    doc.panels[0].action = evil.into();
    doc.panels[0].dialogue = evil.into();
    doc.panels[0].notes = evil.into();
    doc.properties.title = evil.into();
    doc.panels[1].marks[0].action = evil.into();
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 0, evil, 1_700_000_000);
    doc.comment_mut(id).unwrap().text = evil.into();
    let panels = [0, 1, 2];
    let mut m = meta();
    m.title = evil.into();
    m.project = evil.into();
    m.envoi = Some(evil.into());
    m.file_stem = evil.into();
    let (pkg, frames) = relecture::build(&doc, &panels, &Options::default(), m);
    let page = relecture::page(&pkg, &fake_images(frames.len()));
    assert_eq!(page.matches("<script").count(), 1, "aucun script de plus");
    assert_eq!(page.matches("</script>").count(), 1);
    assert!(!page.contains("<img src=x"), "aucune balise venue du projet");
    assert!(!page.contains("\" onmouseover=\""), "aucun attribut venu du projet : les guillemets sont échappés");
    assert!(page.contains("&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(page.contains("&quot; onmouseover=&quot;alert(3)"));
}

#[test]
fn the_script_is_the_same_for_every_project_and_calls_nothing_outside() {
    let a = page_of(&film(), &Options::default());
    let mut other = film();
    other.panels[0].action = "Tout autre texte".into();
    let b = page_of(&other, &Options::default());
    let script = |p: &str| p.split("<script nonce=").nth(1).and_then(|s| s.split("</script>").next()).unwrap_or("").to_string();
    assert_eq!(script(&a), script(&b), "le script ne contient rien du projet");
    let js = script(&a);
    for forbidden in ["fetch", "XMLHttpRequest", "sendBeacon", "WebSocket", "eval(", "innerHTML", "document.write", "importScripts", "http", "location", "cookie", "postMessage", "Worker(", "import("] {
        assert!(!js.contains(forbidden), "{forbidden} dans le script");
    }
    // Il fait ce qu'il doit : enregistrer un fichier de notes au bon format.
    for needed in ["shot-relecture", "Blob", "localStorage", "pointerdown", "toDataURL", "download"] {
        assert!(js.contains(needed), "{needed} manque au script");
    }
}

#[test]
fn existing_comments_are_context_in_the_page_only_when_asked() {
    let mut doc = film();
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 2, "Anthony", 1_760_000_000);
    doc.comment_mut(id).unwrap().text = "Couper deux images".into();
    let resolved = doc.add_comment(a, 3, "Anthony", 1_760_000_000);
    doc.comment_mut(resolved).unwrap().text = "Déjà réglé".into();
    doc.comment_mut(resolved).unwrap().done = true;
    let with = page_of(&doc, &Options { comments: true, ..Options::default() });
    assert!(with.contains("Couper deux images") && with.contains("Anthony") && with.contains("Déjà noté dans SHOT"));
    assert!(!with.contains("Déjà réglé"), "un commentaire résolu n'est pas du contexte");
    let without = page_of(&doc, &Options { comments: false, ..Options::default() });
    assert!(!without.contains("Couper deux images") && !without.contains("Déjà noté dans SHOT"));
}

#[test]
fn the_playback_samples_the_plan_and_merges_identical_images() {
    assert_eq!(relecture::sample_positions(5, 16), vec![0, 1, 2, 3, 4]);
    assert_eq!(relecture::sample_positions(100, 4), vec![0, 25, 50, 75]);
    assert_eq!(relecture::sample_positions(0, 4), vec![0]);
    let doc = film();
    let panels = [0, 1, 2];
    let (off, frames_off) = relecture::build(&doc, &panels, &Options::default(), meta());
    assert!(off.plans.iter().all(|p| p.play.is_empty()));
    let (on, frames_on) = relecture::build(&doc, &panels, &Options { playback: true, ..Options::default() }, meta());
    assert_eq!(on.stills, frames_off.len(), "les images de storyboard d'abord");
    assert!(frames_on.len() > frames_off.len());
    for p in &on.plans {
        let held: u32 = p.play.iter().map(|&(_, h)| h).sum();
        assert_eq!(held as usize, p.frames, "{} : la lecture couvre tout le plan", p.title);
        assert!(p.play.iter().all(|&(i, _)| i >= on.stills), "les images de lecture sont après les images de storyboard");
    }
    // Des images identiques à la suite ne font qu'une image tenue plus longtemps.
    let imgs = vec![vec![1u8], vec![1u8], vec![2u8], vec![1u8]];
    let merged = relecture::merge_identical(&[(0, 2), (1, 3), (2, 1), (3, 4)], &imgs);
    assert_eq!(merged, vec![(0, 5), (2, 1), (3, 4)]);
    // Dans la page : un lecteur par plan, avec les durées.
    let page = relecture::page(&on, &fake_images(frames_on.len()));
    assert_eq!(page.matches("class=\"lecteur\"").count(), 3);
    assert!(page.contains("data-h=\"") && page.contains("data-fps=\"24\""));
}

#[test]
fn a_sequence_of_plans_is_exported_in_the_film_order_whatever_the_request() {
    let doc = film();
    let (pkg, frames) = relecture::build(&doc, &[2, 0], &Options::default(), meta());
    assert_eq!(pkg.plans.iter().map(|p| p.id).collect::<Vec<_>>(), vec![doc.panels[2].id, doc.panels[0].id], "l'ordre demandé est gardé");
    assert_eq!(frames.len(), 2);
    // Un rang qui n'existe pas est laissé de côté.
    let (pkg, _) = relecture::build(&doc, &[0, 99], &Options::default(), meta());
    assert_eq!(pkg.plans.len(), 1);
}

// ------------------------------------------------------------------------------------ le fichier de notes

#[test]
fn a_notes_file_is_read_with_its_reviewer_date_notes_and_general_note() {
    let doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    let png = data_url(&png_with_stroke(96, 54));
    let text = notes_json(&format!(
        r#"{},{{"id":"abc123-{b}-{b}","plan":{b},"image":{b},"numero":"2","titre":"Plan 2","texte":"Plus large","dessin":"{png}","cree":1760000400}}"#,
        note_json(&format!("abc123-{a}-{a}"), a, a, "1", "Trop court")
    ));
    let notes = relecture::parse_notes(text.as_bytes()).expect("lu");
    assert_eq!((notes.reviewer.as_str(), notes.project.as_str(), notes.created), ("Claire", "Mon film", 1_760_000_500));
    assert_eq!(notes.notes.len(), 3, "deux notes et la note générale");
    assert!(notes.notes[0].general && notes.notes[0].text == "Très bien.");
    assert_eq!((notes.notes[1].plan, notes.notes[1].image, notes.notes[1].text.as_str()), (Some(a), Some(a), "Trop court"));
    assert!(notes.notes[2].drawing.is_some() && notes.notes[2].created == 1_760_000_400);
    assert_eq!((notes.ignored, notes.drawings_refused), (0, 0));
}

#[test]
fn a_file_that_is_not_a_notes_file_says_so() {
    for bad in ["", "pas du json", "[]", "{}", r#"{"format":"autre","version":1}"#, r#"{"format":"shot-commentaires","version":1,"notes":[]}"#] {
        let e = relecture::parse_notes(bad.as_bytes()).unwrap_err();
        assert!(e.contains("pas un fichier de notes de relecture"), "{bad:?} : {e}");
    }
    assert!(relecture::parse_notes(&[0xFF, 0xFE, 0x00]).is_err(), "pas de l'UTF-8");
    let e = relecture::parse_notes(br#"{"format":"shot-relecture","version":99}"#).unwrap_err();
    assert!(e.contains("plus récente"), "{e}");
}

#[test]
fn a_notes_file_is_untrusted_texts_ids_and_counts_are_bounded() {
    // Textes : nettoyés et coupés ; identifiants : lettres, chiffres, - _ . seulement.
    let long = "é".repeat(commentaires::TEXT_MAX + 500);
    let evil_name = "Claire\u{202E}\u{0007}\r\nDupont";
    let text = format!(
        r#"{{"format":"shot-relecture","version":1,"relecteur":{},"notes":[
            {{"id":"../../etc/passwd","plan":"1","image":"1","texte":"a"}},
            {{"id":"ok-1","plan":"1","image":"1","texte":{}}},
            {{"id":"ok-1","plan":"1","image":"1","texte":"doublon"}},
            {{"id":"C:\\Windows\\x","plan":"2","image":"2","texte":"b"}},
            {{"id":12,"plan":2,"image":2,"texte":"c"}},
            {{"plan":"3","image":"3","texte":"d"}},
            {{"texte":"sans plan ni identifiant"}},
            {{"id":"vide","plan":"4","texte":"   "}}
        ]}}"#,
        serde_json::to_string(evil_name).unwrap(),
        serde_json::to_string(&long).unwrap()
    );
    let notes = relecture::parse_notes(text.as_bytes()).unwrap();
    assert!(!notes.reviewer.contains('\u{202E}') && !notes.reviewer.contains('\u{7}') && !notes.reviewer.contains('\r'), "{:?}", notes.reviewer);
    let ids: Vec<&str> = notes.notes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.iter().all(|id| id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))), "{ids:?}");
    assert!(!ids.iter().any(|id| id.contains("..") && id.contains('/')), "{ids:?}");
    assert!(ids.contains(&"ok-1") && ids.contains(&"12"), "{ids:?}");
    assert_eq!(ids.iter().filter(|id| **id == "ok-1").count(), 1, "un identifiant en double est ignoré");
    let big = notes.notes.iter().find(|n| n.id == "ok-1").unwrap();
    assert_eq!(big.text.chars().count(), commentaires::TEXT_MAX, "texte coupé");
    // Compté : le doublon, la note sans rien, la note vide.
    assert!(notes.ignored >= 3, "{}", notes.ignored);
    // Sans identifiant lisible mais avec un plan : la clé est refaite de ce qu'on connaît.
    assert!(notes.notes.iter().any(|n| n.text == "d" && n.id == "sans-paquet-3-3"), "{ids:?}");
}

#[test]
fn at_most_a_few_thousand_notes_are_read() {
    let one = |i: usize| format!(r#"{{"id":"n{i}","plan":"1","image":"1","texte":"t{i}"}}"#);
    let all: Vec<String> = (0..relecture::NOTES_MAX + 25).map(one).collect();
    let notes = relecture::parse_notes(notes_json(&all.join(",")).as_bytes()).unwrap();
    assert_eq!(notes.notes.len(), relecture::NOTES_MAX + 1, "les notes lues et la note générale");
    assert_eq!(notes.ignored, 25);
}

#[test]
fn a_too_big_file_is_refused_before_it_is_read() {
    let dir = temp("gros");
    let path = dir.join("gros.json");
    let f = std::fs::File::create(&path).unwrap();
    f.set_len(relecture::NOTES_MAX_BYTES + 10).unwrap();
    drop(f);
    let e = relecture::read_notes_file(&path).unwrap_err();
    assert!(e.contains("trop gros"), "{e}");
    assert!(relecture::read_notes_file(&dir.join("absent.json")).is_err());
    let ok = dir.join("ok.json");
    std::fs::write(&ok, notes_json("")).unwrap();
    assert!(relecture::read_notes_file(&ok).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ les dessins

#[test]
fn a_drawing_is_decoded_under_limits_and_rewritten_clean() {
    let png = png_with_stroke(96, 54);
    let a = relecture::decode_overlay(&data_url(&png)).unwrap().expect("un dessin");
    assert!(a.0.starts_with(&[0x89, b'P', b'N', b'G']));
    // Trop large : réduit à 1280.
    let wide = relecture::decode_overlay(&data_url(&png_with_stroke(2000, 40))).unwrap().unwrap();
    let r = image::ImageReader::with_format(std::io::Cursor::new(&wide.0[..]), image::ImageFormat::Png).into_dimensions().unwrap();
    assert_eq!(r.0, 1280);
    // Entièrement transparent : rien à montrer.
    let blank = crate::io::encode_png(&vec![0u8; 20 * 20 * 4], 20, 20).unwrap();
    assert_eq!(relecture::decode_overlay(&data_url(&blank)).unwrap(), None);
}

#[test]
fn a_bad_drawing_is_refused_without_stopping_the_import() {
    for (what, url) in [
        ("pas un PNG déclaré", "data:image/jpeg;base64,/9j/4AAQ".to_string()),
        ("une adresse", "https://exemple.fr/x.png".to_string()),
        ("une adresse locale", "file:///C:/secret.png".to_string()),
        ("base64 cassé", "data:image/png;base64,@@@@".to_string()),
        ("pas une signature PNG", format!("data:image/png;base64,{}", crate::base64::encode(b"GIF89a......"))),
        ("PNG coupé", data_url(&png_with_stroke(96, 54)[..40])),
        ("trop large", data_url(&png_with_stroke(4200, 8))),
        ("trop de pixels", data_url(&crate::io::encode_png(&vec![0u8; 3000 * 3000 * 4], 3000, 3000).unwrap())),
        ("trop lourd", format!("data:image/png;base64,{}", "A".repeat(commentaires::ANNOTATION_B64_MAX + 4))),
    ] {
        assert!(relecture::decode_overlay(&url).is_err(), "{what}");
    }
    // Dans un fichier : la note est gardée, son dessin refusé et compté.
    let note = format!(r#"{{"id":"n1","plan":"1","image":"1","texte":"Le texte reste","dessin":"https://exemple.fr/x.png"}}"#);
    let notes = relecture::parse_notes(notes_json(&note).as_bytes()).unwrap();
    let n = notes.notes.iter().find(|n| n.id == "n1").unwrap();
    assert!(n.drawing.is_none() && n.text == "Le texte reste");
    assert_eq!(notes.drawings_refused, 1);
    // Un dessin seul, sans texte, est une note.
    let note = format!(r#"{{"id":"n2","plan":"1","image":"1","texte":"","dessin":"{}"}}"#, data_url(&png_with_stroke(96, 54)));
    let notes = relecture::parse_notes(notes_json(&note).as_bytes()).unwrap();
    assert!(notes.notes.iter().any(|n| n.id == "n2" && n.drawing.is_some()));
}

// ------------------------------------------------------------------------------------ le placement

fn notes_for(doc: &Document, entries: &[(u64, u64, &str, &str)]) -> Notes {
    let _ = doc;
    let list: Vec<String> = entries.iter().enumerate().map(|(k, (plan, image, numero, text))| note_json(&format!("abc123-{k}"), *plan, *image, numero, text)).collect();
    relecture::parse_notes(notes_json(&list.join(",")).as_bytes()).unwrap()
}

#[test]
fn notes_find_their_plan_by_identifier_even_if_the_numbers_changed() {
    let mut doc = film();
    let (a, b, c) = (doc.panels[0].id, doc.panels[1].id, doc.panels[2].id);
    // La note dit « plan 7 » mais l'identifiant est celui du deuxième plan : l'identifiant gagne.
    let notes = notes_for(&doc, &[(a, a, "1", "un"), (b, b, "7", "deux"), (c, c, "3", "trois")]);
    let places = relecture::place(&doc, &notes);
    let find = |text: &str| places.iter().find(|p| notes.notes[p.note].text == text).unwrap();
    assert_eq!(find("deux").plan, Some((1, Via::Id)));
    assert_eq!(find("un").plan, Some((0, Via::Id)));
    assert_eq!(find("trois").plan, Some((2, Via::Id)));
    // Le film a changé depuis : un plan déplacé garde son identifiant.
    let moved = doc.move_plans(&[c], Some(a)).is_some();
    assert!(moved);
    let places = relecture::place(&doc, &notes);
    assert_eq!(places.iter().find(|p| notes.notes[p.note].text == "trois").unwrap().plan, Some((0, Via::Id)));
    assert_eq!(places.iter().find(|p| notes.notes[p.note].text == "un").unwrap().plan, Some((1, Via::Id)));
}

#[test]
fn notes_fall_back_to_the_plan_number_when_the_identifier_is_gone() {
    let doc = film();
    // Identifiants inconnus (autre copie du projet), numéros 2 et 3 connus, 9 inconnu.
    let notes = notes_for(&doc, &[(9001, 9001, "2", "par numéro"), (9002, 9002, "3", "aussi"), (9003, 9003, "9", "perdue"), (9004, 9004, "", "sans rien")]);
    let places = relecture::place(&doc, &notes);
    let find = |text: &str| places.iter().find(|p| notes.notes[p.note].text == text).unwrap();
    assert_eq!(find("par numéro").plan, Some((1, Via::Number)));
    assert_eq!(find("aussi").plan, Some((2, Via::Number)));
    assert_eq!(find("perdue").plan, None);
    assert_eq!(find("sans rien").plan, None);
    let sum = relecture::preview(&doc, &notes, &places, None);
    assert_eq!((sum.imported, sum.plans, sum.by_number), (3, 2, 2), "deux notes et la note générale, sur deux plans");
    assert_eq!(sum.unmatched.len(), 2, "les deux notes sans plan sont comptées");
}

#[test]
fn the_image_of_a_note_is_found_by_its_storyboard_mark() {
    let doc = film();
    let b = doc.panels[1].id;
    let mark = doc.panels[1].marks[0].id;
    let notes = notes_for(&doc, &[(b, mark, "2", "sur la marque"), (b, b, "2", "sur le plan"), (b, 424_242, "2", "marque disparue")]);
    let places = relecture::place(&doc, &notes);
    let find = |text: &str| places.iter().find(|p| notes.notes[p.note].text == text).unwrap();
    assert_eq!((find("sur la marque").frame, find("sur la marque").image_lost), (8, false));
    assert_eq!((find("sur le plan").frame, find("sur le plan").image_lost), (0, false));
    assert_eq!((find("marque disparue").frame, find("marque disparue").image_lost), (0, true), "au début du plan, signalé");
}

#[test]
fn importing_creates_comments_with_author_date_text_color_and_drawing() {
    let mut doc = film();
    let b = doc.panels[1].id;
    let png = data_url(&png_with_stroke(96, 54));
    let text = notes_json(&format!(r#"{{"id":"n1","plan":"{b}","image":"{b}","numero":"2","texte":"Plus large ?","dessin":"{png}","cree":1760000400}}"#));
    let notes = relecture::parse_notes(text.as_bytes()).unwrap();
    let places = relecture::place(&doc, &notes);
    let sum = relecture::apply(&mut doc, &notes, &places, None, 1_760_001_000);
    assert_eq!(sum.message(), "2 notes importées sur 1 plan.", "la note et la note générale, posée sur le même plan");
    let c = doc.comments.iter().find(|c| c.note.as_deref() == Some("n1")).expect("commentaire de la note");
    assert_eq!((c.panel, c.frame, c.author.as_str(), c.created, c.color, c.done), (b, 0, "Claire", 1_760_000_400, relecture::REVIEW_COLOR, false));
    assert_eq!(c.text, "Plus large ?");
    assert!(c.annotation.is_some());
    // La note générale est sur le premier plan qui reçoit une note.
    let g = doc.comments.iter().find(|c| c.note.as_deref() == Some("abc123-general")).expect("note générale");
    assert_eq!(g.panel, b);
    assert!(g.text.starts_with("Note générale : Très bien."));
    // Ids de commentaires distincts, sans toucher à ceux d'avant.
    let ids: std::collections::HashSet<u64> = doc.comments.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), doc.comments.len());
}

#[test]
fn importing_the_same_file_twice_does_not_duplicate_the_notes() {
    let mut doc = film();
    let (a, b) = (doc.panels[0].id, doc.panels[1].id);
    let notes = notes_for(&doc, &[(a, a, "1", "un"), (b, b, "2", "deux")]);
    let places = relecture::place(&doc, &notes);
    let first = relecture::apply(&mut doc, &notes, &places, None, 1_760_001_000);
    assert_eq!(first.imported, 3);
    let n = doc.comments.len();
    let again = relecture::place(&doc, &notes);
    assert!(again.iter().all(|p| p.already));
    let second = relecture::apply(&mut doc, &notes, &again, None, 1_760_002_000);
    assert_eq!((second.imported, second.already), (0, 3));
    assert_eq!(doc.comments.len(), n, "aucun commentaire de plus");
    assert!(second.message().contains("3 notes déjà importées"), "{}", second.message());
    // Un fichier qui s'enrichit (même paquet, une note de plus) : seule la nouvelle est posée.
    let more = notes_for(&doc, &[(a, a, "1", "un"), (b, b, "2", "deux"), (a, a, "1", "trois")]);
    let mut more = more;
    more.notes[3].id = "abc123-nouvelle".into();
    let places = relecture::place(&doc, &more);
    let third = relecture::apply(&mut doc, &more, &places, None, 1_760_003_000);
    assert_eq!((third.imported, third.already), (1, 3 - 0));
    assert_eq!(doc.comments.len(), n + 1);
}

#[test]
fn the_summary_says_notes_plans_and_unmatched_notes_in_plain_french() {
    let s = relecture::Summary { imported: 12, plans: 9, unmatched: vec![3], ..Default::default() };
    assert_eq!(s.message(), "12 notes importées sur 9 plans, 1 note sans plan (plan supprimé).");
    let s = relecture::Summary { imported: 1, plans: 1, ..Default::default() };
    assert_eq!(s.message(), "1 note importée sur 1 plan.");
    let s = relecture::Summary { imported: 2, plans: 2, already: 1, ignored: 2, drawings_refused: 1, unmatched: vec![0, 1], ..Default::default() };
    assert_eq!(s.message(), "2 notes importées sur 2 plans, 1 note déjà importée, 2 notes sans plan (plan supprimé), 2 notes ignorées (illisibles), 1 dessin refusé.");
}

#[test]
fn notes_without_a_plan_can_be_put_on_a_chosen_plan_with_their_origin() {
    let mut doc = film();
    let notes = notes_for(&doc, &[(9001, 9001, "14", "perdue")]);
    let places = relecture::place(&doc, &notes);
    let none = relecture::preview(&doc, &notes, &places, None);
    assert_eq!(none.unmatched.len(), 1);
    let sum = relecture::apply(&mut doc, &notes, &places, Some(2), 1_760_001_000);
    assert!(sum.unmatched.is_empty() && sum.imported == 2, "{sum:?}");
    let c = doc.comments.iter().find(|c| c.note.as_deref() == Some("abc123-0")).unwrap();
    assert_eq!(c.panel, doc.panels[2].id);
    assert!(c.text.starts_with("[Note pour Plan 14, introuvable dans ce projet]") && c.text.ends_with("perdue"), "{}", c.text);
}

#[test]
fn nothing_touches_the_disk_while_importing_notes() {
    // Aucun nom de fichier du fichier de notes n'est utilisé : un identifiant en chemin est refusé ou
    // gardé comme simple clé, et l'import ne crée rien.
    let dir = temp("rien-sur-disque");
    let before: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    let mut doc = film();
    let a = doc.panels[0].id;
    let note = format!(r#"{{"id":"..\\..\\evil.png","plan":"{a}","image":"{a}","texte":"x","dessin":"data:image/png;base64,{}"}}"#, crate::base64::encode(b"\x89PNG\r\n\x1a\nxx"));
    let notes = relecture::parse_notes(notes_json(&note).as_bytes()).unwrap();
    let places = relecture::place(&doc, &notes);
    relecture::apply(&mut doc, &notes, &places, None, 1);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), before.len());
    assert!(doc.comments.iter().all(|c| c.note.as_deref().is_none_or(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))));
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ le projet

#[test]
fn the_note_key_and_the_drawing_survive_saving_and_reopening_the_project() {
    let mut doc = film();
    let b = doc.panels[1].id;
    let png = data_url(&png_with_stroke(96, 54));
    let note = format!(r#"{{"id":"n1","plan":"{b}","image":"{b}","numero":"2","texte":"Plus large","dessin":"{png}"}}"#);
    let notes = relecture::parse_notes(notes_json(&note).as_bytes()).unwrap();
    let places = relecture::place(&doc, &notes);
    relecture::apply(&mut doc, &notes, &places, None, 1_760_001_000);
    let dir = temp("projet");
    let manifest = dir.join("F.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(dir.join(commentaires::FILE)).unwrap();
    assert!(text.contains("\"dessin\"") && text.contains("\"note\": \"n1\""), "{}", &text[..text.len().min(300)]);
    let loaded = crate::project::load(&manifest).unwrap();
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let c = loaded.doc.comments.iter().find(|c| c.note.as_deref() == Some("n1")).expect("relu");
    assert!(c.annotation.is_some() && c.text == "Plus large");
    let b64 = crate::base64::encode(&c.annotation.as_ref().unwrap().0);
    // Importer de nouveau dans le projet rouvert : toujours pas de doublon (la clé a voyagé avec le projet).
    let mut reopened = loaded.doc;
    let again = relecture::place(&reopened, &notes);
    assert!(again.iter().filter(|p| notes.notes[p.note].id == "n1").all(|p| p.already));
    let n = reopened.comments.len();
    relecture::apply(&mut reopened, &notes, &again, None, 5);
    assert_eq!(reopened.comments.len(), n);
    // Même chose avec un dessin abîmé dans commentaires.json : le commentaire est lu, sans le dessin.
    let broken = text.replace(&b64[..40], "AAAA");
    std::fs::write(dir.join(commentaires::FILE), broken).unwrap();
    let loaded = crate::project::load(&manifest).unwrap();
    let c = loaded.doc.comments.iter().find(|c| c.note.as_deref() == Some("n1")).expect("commentaire gardé");
    assert!(c.annotation.is_none() && c.text == "Plus large");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn old_comment_files_without_the_new_fields_still_load_and_write_the_same() {
    let text = r#"{"format":"shot-commentaires","version":1,"commentaires":[{"id":1,"plan":5,"image":2,"texte":"vieux","auteur":"A","couleur":[1,2,3],"cree":9}]}"#;
    let list = commentaires::parse(text).unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].note.is_none() && list[0].annotation.is_none());
    let json = serde_json::to_string(&list[0]).unwrap();
    assert!(!json.contains("dessin") && !json.contains("\"note\""), "rien d'écrit en plus pour un commentaire ordinaire : {json}");
    let c: Comment = serde_json::from_str(&json).unwrap();
    assert_eq!(c, list[0]);
    assert_eq!(Annotation::from_png(b"pas un png".to_vec()), None);
}

#[test]
fn importing_is_one_named_undo_step() {
    let dir = temp("annulation");
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    let (a, b) = (app.doc.panels[0].id, app.doc.panels[1].id);
    let notes = notes_for(&app.doc, &[(a, a, "1", "un"), (b, b, "2", "deux")]);
    let before = app.doc.comments.len();
    let steps = app.history.names().0.len();
    let message = app.relecture_apply_notes(&notes, false);
    assert_eq!(message, "3 notes importées sur 2 plans.");
    assert_eq!(app.doc.comments.len(), before + 3);
    let (undo, _) = app.history.names();
    assert_eq!(undo.len(), steps + 1, "une seule étape pour toutes les notes");
    assert_eq!(*undo.last().unwrap(), "Importer les notes de relecture");
    assert!(app.modified);
    app.undo();
    assert_eq!(app.doc.comments.len(), before, "Ctrl+Z retire tout d'un coup");
    app.redo();
    assert_eq!(app.doc.comments.len(), before + 3);
    // Une seconde importation du même fichier ne fait rien, pas même une étape d'annulation.
    let steps = app.history.names().0.len();
    assert!(app.relecture_apply_notes(&notes, false).starts_with("Rien à importer"));
    assert_eq!(app.history.names().0.len(), steps);
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ le PDF

#[test]
fn the_pdf_can_carry_the_comments_and_blank_note_areas() {
    let mut doc = crate::export_tests::doc_with_drawing(4, 6);
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 2, "Anthony", 1);
    doc.comment_mut(id).unwrap().text = "Couper deux images".into();
    let resolved = doc.add_comment(a, 2, "Anthony", 1);
    doc.comment_mut(resolved).unwrap().text = "Déjà réglé".into();
    doc.comment_mut(resolved).unwrap().done = true;
    let panels: Vec<usize> = (0..4).collect();
    let (_, plain) = crate::export::storyboard_of(&doc, &panels);
    assert!(plain.iter().all(|b| b.comments.is_empty()), "sans l'option : aucun commentaire");
    let (shots, with) = crate::export::storyboard_of_with(&doc, &panels, true);
    assert_eq!(with[0].comments, vec![("Anthony".to_string(), "Couper deux images".to_string())], "le commentaire résolu n'y est pas");
    assert!(with[1..].iter().all(|b| b.comments.is_empty()));
    // Les lignes de la case : « Commentaire (Anthony) : … ».
    let lines = crate::pdf::board_lines(&with[0], 250.0, 8);
    assert!(lines.iter().any(|l| l.label.as_deref() == Some("Commentaire (Anthony) : ")), "{lines:?}");
    // Le PDF : avec cadres de notes, trois images par page.
    let img = {
        let rgb = image::RgbImage::from_pixel(16, 9, image::Rgb([200, 200, 200]));
        let mut data = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut data).encode(rgb.as_raw(), 16, 9, image::ExtendedColorType::Rgb8).unwrap();
        crate::pdf::BoardImage { width: 16, height: 9, jpeg: data }
    };
    let images: Vec<_> = (0..shots.len()).map(|_| crate::pdf::BoardImage { width: img.width, height: img.height, jpeg: img.jpeg.clone() }).collect();
    let normal = crate::pdf::storyboard_with("Essai", &with, &images, 16.0 / 9.0, false);
    let print = crate::pdf::storyboard_with("Essai", &with, &images, 16.0 / 9.0, true);
    let count = |b: &[u8]| String::from_utf8_lossy(b).matches("/Type /Page /Parent").count();
    assert_eq!(count(&normal), 1, "quatre images tiennent sur une page de six");
    assert_eq!(count(&print), 2, "trois par page avec les cadres de notes");
    let text = String::from_utf8_lossy(&print);
    assert!(text.contains("(Notes) Tj") && text.contains("Commentaire"), "le cadre « Notes » et le commentaire sont dans le PDF");
    assert!(!String::from_utf8_lossy(&normal).contains("(Notes) Tj"), "pas de cadre sans l'option");
    assert_eq!(crate::pdf::storyboard("Essai", &with, &images, 16.0 / 9.0), normal, "l'ancienne fonction n'a pas changé");
}

// ------------------------------------------------------------------------------------ la tâche

fn run(job: &mut RelectureJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.running() {
        job.step(doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120), "l'export ne finit pas");
    }
}

#[test]
fn the_export_writes_one_offline_page_and_an_optional_printable_pdf() {
    let doc = crate::export_tests::doc_with_drawing(5, 6);
    let dir = temp("export");
    let html = dir.join("Essai_relecture.html");
    let panels: Vec<usize> = (0..5).collect();
    let opts = Options { playback: true, comments: true, pdf: true };
    let mut job = RelectureJob::start(&doc, &panels, &opts, meta(), &html).expect("démarré");
    assert!(job.rendering());
    run(&mut job, &doc);
    let report = job.report().cloned().unwrap().expect("réussi");
    assert_eq!((report.plans, report.stills), (5, 5));
    let page = std::fs::read_to_string(&html).unwrap();
    for p in &doc.panels {
        assert!(page.contains(&format!("id=\"p{}\"", p.id)));
    }
    assert!(!page.contains("http://") && !page.contains("https://"));
    assert_eq!(page.matches("<section class=\"carte\"").count(), 5);
    // (Ces plans sont immobiles : leurs images de lecture sont identiques, fondues en une seule, il n'y a rien à lire.)
    assert!(!page.contains("class=\"lecteur\""), "un plan immobile n'a pas de lecteur");
    assert!(report.bytes as usize == page.len());
    let pdf = std::fs::read(dir.join("Essai_relecture.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4") && String::from_utf8_lossy(&pdf).matches("/Type /Page /Parent").count() == 2, "5 images, 3 par page");
    let left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(!left.iter().any(|n| n.ends_with(".part")), "aucun fichier à moitié écrit : {left:?}");
    // Les images de la page sont de vrais JPEG.
    let first = page.split("src=\"data:image/jpeg;base64,").nth(1).unwrap().split('"').next().unwrap();
    let bytes = crate::base64::decode(first).unwrap();
    assert!(image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}

/// Écrit une page d'essai à regarder dans un navigateur : `SHOT_RELECTURE_ECHANTILLON=<fichier.html>` puis
/// `cargo test --release relecture_tests::write_a_sample_page -- --ignored`. (Pas un essai : un outil.)
#[test]
#[ignore]
fn write_a_sample_page() {
    let Some(out) = std::env::var_os("SHOT_RELECTURE_ECHANTILLON") else { return };
    let mut doc = crate::export_tests::doc_with_drawing(4, 8);
    // Le premier plan bouge : un second dessin, un coup sur deux.
    let id2 = doc.new_cel(crate::document::LayerKind::Bitmap);
    {
        let r = &mut doc.cels.get_mut(&id2).unwrap().raster;
        for y in 0..120 {
            for x in 0..150 {
                let idx = r.tile_index_of(x, y);
                r.tile_mut(idx)[(y % 64) * 64 + x % 64] = eframe::egui::Color32::from_rgba_premultiplied(220, 30, 30, 255);
            }
        }
    }
    for f in (0..8).step_by(2) {
        doc.panels[0].layers[0].frames[f] = Some(id2);
    }
    doc.panels[0].action = "Il entre dans la salle, la porte claque.".into();
    doc.panels[1].dialogue = "— Tu viens ? <b>gras</b> & \"guillemets\"".into();
    let a = doc.panels[0].id;
    let id = doc.add_comment(a, 2, "Anthony", 1_760_000_000);
    doc.comment_mut(id).unwrap().text = "Couper deux images ici.".into();
    let panels: Vec<usize> = (0..4).collect();
    let path = std::path::PathBuf::from(out);
    let mut job = RelectureJob::start(&doc, &panels, &Options { playback: true, comments: true, pdf: true }, meta(), &path).unwrap();
    run(&mut job, &doc);
    job.report().cloned().unwrap().expect("réussi");
}

#[test]
fn stopping_the_export_leaves_no_file_and_keeps_an_older_page() {
    let doc = crate::export_tests::doc_with_drawing(6, 4);
    let dir = temp("arret");
    let html = dir.join("page.html");
    std::fs::write(&html, "ancienne page").unwrap();
    let panels: Vec<usize> = (0..6).collect();
    let opts = Options { pdf: true, ..Options::default() };
    let mut job = RelectureJob::start(&doc, &panels, &opts, meta(), &html).unwrap();
    job.step(&doc, Duration::from_millis(5));
    job.cancel();
    assert!(!job.running());
    assert_eq!(job.report().cloned().unwrap().err().as_deref(), Some("Export annulé."));
    assert_eq!(std::fs::read_to_string(&html).unwrap(), "ancienne page", "la page d'avant n'est pas effacée");
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["page.html".to_string()], "{names:?}");
    // Fermer SHOT pendant l'export revient au même.
    let mut job = RelectureJob::start(&doc, &panels, &opts, meta(), &html).unwrap();
    job.step(&doc, Duration::from_millis(5));
    drop(job);
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(names, vec!["page.html".to_string()], "{names:?}");
    assert!(RelectureJob::start(&doc, &[], &opts, meta(), &html).is_err(), "rien à exporter");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_export_does_not_block_the_interface_thread_for_long() {
    // Chaque pas rend la main : l'interface ne calcule que quelques images à la fois.
    let doc = crate::export_tests::doc_with_drawing(8, 6);
    let dir = temp("pas");
    let panels: Vec<usize> = (0..8).collect();
    let mut job = RelectureJob::start(&doc, &panels, &Options { playback: true, ..Options::default() }, meta(), &dir.join("p.html")).unwrap();
    let mut worst = Duration::ZERO;
    let t = std::time::Instant::now();
    while job.running() {
        let s = std::time::Instant::now();
        job.step(&doc, Duration::from_millis(20));
        worst = worst.max(s.elapsed());
        std::thread::sleep(Duration::from_millis(1));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    // (Un pas peut finir une image commencée : on exige seulement qu'il rende la main bien avant la fin de l'export.)
    assert!(worst < Duration::from_secs(5), "{worst:?}");
    std::fs::remove_dir_all(&dir).ok();
}

// ------------------------------------------------------------------------------------ les fenêtres

fn frame(ctx: &egui::Context, app: &mut App) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0))), time: Some(0.0), ..Default::default() };
    let _ = ctx.run(input, |ctx| app.relecture_windows(ctx));
}

#[test]
fn the_actions_exist_and_open_their_windows() {
    for id in ["fichier.exporter_relecture", "fichier.importer_notes"] {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
    let dir = temp("fenetres");
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    app.run_action("fichier.exporter_relecture");
    assert!(app.relecture.export.is_some());
    for _ in 0..3 {
        frame(&ctx, &mut app);
    }
    assert!(app.relecture.export.is_some(), "la fenêtre reste ouverte");
    assert!(!app.relecture.rendering() && !app.modal_open(), "pas de verrou tant que rien ne s'exporte");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_pdf_export_window_offers_the_comments_option() {
    assert!(!crate::export::ExportSettings::default().comments, "comme avant tant qu'on ne coche pas");
    let dir = temp("fenetre-pdf");
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    let a = app.doc.panels[0].id;
    app.doc.add_comment(a, 1, "A", 1);
    app.run_action("fichier.exporter_pdf");
    assert!(app.export_dialog.as_ref().is_some_and(|s| s.format == crate::export::Format::Pdf));
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0))), time: Some(0.0), ..Default::default() };
    fn collect(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for _ in 0..3 {
        let output = ctx.run(input.clone(), |ctx| app.export_windows(ctx));
        output.shapes.iter().for_each(|s| collect(&s.shape, &mut found));
    }
    let texts = found.join("|");
    assert!(texts.contains("Avec les commentaires"), "{texts}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_dropped_json_opens_the_import_window_and_reads_on_another_thread() {
    let dir = temp("depot");
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    let (a, b) = (app.doc.panels[0].id, app.doc.panels[1].id);
    let file = dir.join("notes-Mon_film-2026-10-20.json");
    std::fs::write(&file, notes_json(&format!("{},{}", note_json("abc123-a", a, a, "1", "un"), note_json("abc123-b", b, b, "2", "deux")))).unwrap();
    assert!(!app.import_dropped_notes(&dir.join("image.png")), "seul un .json est pris");
    assert!(app.import_dropped_notes(&file));
    assert!(app.relecture.import.is_some());
    // La lecture vient sur un autre fil : on laisse tourner l'interface jusqu'à l'avoir.
    let t = std::time::Instant::now();
    loop {
        frame(&ctx, &mut app);
        if matches!(app.relecture.import.as_ref().map(|w| w.is_ready()), Some(true)) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "le fichier n'est pas lu");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Pas un fichier de notes : un message, pas de plantage.
    let bad = dir.join("autre.json");
    std::fs::write(&bad, "{\"nom\": 1}").unwrap();
    assert!(app.import_dropped_notes(&bad));
    let t = std::time::Instant::now();
    loop {
        frame(&ctx, &mut app);
        if app.relecture.import.as_ref().is_some_and(|w| w.error().is_some()) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.relecture.import.as_ref().unwrap().error().unwrap().contains("pas un fichier de notes"));
    assert!(app.doc.comments.is_empty(), "lire ne change rien au projet");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_drawing_shows_for_the_chosen_comment_and_can_be_hidden() {
    let dir = temp("calque");
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = film();
    let b = app.doc.panels[1].id;
    let mark = app.doc.panels[1].marks[0].id;
    let png = data_url(&png_with_stroke(96, 54));
    let note = format!(r#"{{"id":"n1","plan":"{b}","image":"{mark}","texte":"Plus large","dessin":"{png}"}}"#);
    let notes = relecture::parse_notes(notes_json(&note).as_bytes()).unwrap();
    app.relecture_apply_notes(&notes, false);
    let id = app.doc.comments.iter().find(|c| c.note.as_deref() == Some("n1")).unwrap().id;
    app.select_comment(id, true);
    assert_eq!((app.cur_panel, app.cur_frame), (1, 8), "le commentaire est sur l'image de la marque");
    let painter = ctx.layer_painter(egui::LayerId::background());
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))), time: Some(0.0), ..Default::default() };
    app.canvas_rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
    let t = std::time::Instant::now();
    loop {
        let _ = ctx.run(input.clone(), |ctx| app.paint_review_drawing(ctx, &painter));
        if app.relecture.has_drawing_texture() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "le dessin n'est pas décodé");
        std::thread::sleep(Duration::from_millis(5));
    }
    // Éteint : la texture est lâchée. Une autre image du plan : le dessin n'est pas montré.
    app.relecture.montrer_dessin = false;
    let _ = ctx.run(input.clone(), |ctx| app.paint_review_drawing(ctx, &painter));
    assert!(!app.relecture.has_drawing_texture());
    app.relecture.montrer_dessin = true;
    app.cur_frame = 2;
    let _ = ctx.run(input, |ctx| app.paint_review_drawing(ctx, &painter));
    assert!(!app.relecture.has_drawing_texture(), "le dessin vise l'image de la marque, pas le début du plan");
    std::fs::remove_dir_all(&dir).ok();
}

/// Un fichier de notes écrit par la vraie page, dans un vrai navigateur (dessins au stylet en trois couleurs, gomme, annulation) :
/// SHOT le lit tel quel.
#[test]
fn a_notes_file_written_by_a_real_browser_is_read() {
    let text = include_str!("../assets/relecture/essai-navigateur.json");
    let notes = relecture::parse_notes(text.as_bytes()).expect("lu");
    assert_eq!((notes.reviewer.as_str(), notes.project.as_str()), ("Claire Test", "Mon film"));
    assert_eq!((notes.ignored, notes.drawings_refused), (0, 0));
    assert_eq!(notes.notes.len(), 3, "deux images annotées et la note générale");
    let with_drawing: Vec<_> = notes.notes.iter().filter(|n| n.drawing.is_some()).collect();
    assert_eq!(with_drawing.len(), 2);
    let two = notes.notes.iter().find(|n| n.text.starts_with("Raccord")).expect("note multiligne");
    assert!(two.text.contains('\n'), "les retours à la ligne passent");
    let d = two.drawing.as_ref().unwrap();
    let (w, h) = image::ImageReader::with_format(std::io::Cursor::new(&d.0[..]), image::ImageFormat::Png).into_dimensions().unwrap();
    assert_eq!((w, h), (960, 576), "le dessin couvre le cadre de la page d'essai (150 × 90) : 960 de large, même proportion");
    // Le texte du navigateur garde ses signes tels quels (rien n'y est du HTML).
    assert!(notes.notes.iter().any(|n| n.text.contains("<b>svp</b>") && n.text.contains('"')));
}

#[test]
fn the_page_script_and_style_are_static_and_have_no_external_reference() {
    let css = include_str!("../assets/relecture/page.css");
    for forbidden in ["url(", "@import", "http", "expression("] {
        assert!(!css.contains(forbidden), "{forbidden} dans le style");
    }
    let js = include_str!("../assets/relecture/page.js");
    assert_eq!(js.matches('{').count(), js.matches('}').count(), "accolades équilibrées");
    assert_eq!(js.matches('(').count(), js.matches(')').count(), "parenthèses équilibrées");
    assert!(!js.contains("</script"), "le script ne ferme pas sa propre balise");
}
