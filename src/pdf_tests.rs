//! Tests du PDF de storyboard (phase 3c).

use std::time::Duration;

use crate::export::{self, ExportJob, Range};
use crate::pdf::{self, Board, BoardImage};

#[test]
fn french_text_is_encoded_in_winansi() {
    let bytes = pdf::winansi("Été « ça » coûte 5 € — œuvre… 日");
    assert_eq!(bytes[0], 0xC9, "É");
    assert_eq!(bytes[1], b't');
    assert!(bytes.contains(&0xAB) && bytes.contains(&0xBB), "guillemets");
    assert!(bytes.contains(&0xE7), "ç");
    assert!(bytes.contains(&0xFB), "û");
    assert!(bytes.contains(&0x80) && bytes.contains(&0x97) && bytes.contains(&0x9C) && bytes.contains(&0x85));
    assert_eq!(*bytes.last().unwrap(), b'?', "hors de la table : ?");
}

#[test]
fn widths_and_wrapping() {
    // H 722 + e 556 + l 222 + l 222 + o 556 = 2278 millièmes.
    assert!((pdf::text_width("Hello", false, 10.0) - 22.78).abs() < 1e-3);
    assert!(pdf::text_width("Hello", true, 10.0) > pdf::text_width("Hello", false, 10.0));
    assert_eq!(pdf::text_width("é", false, 10.0), pdf::text_width("e", false, 10.0));
    let text = "Le bus s'arrête devant l'école. Brume épaisse sur la place, on entend des voix au loin.";
    let lines = pdf::wrap(text, false, 8.0, 100.0, 30.0);
    assert!(lines.len() > 2);
    assert!(pdf::text_width(&lines[0], false, 8.0) <= 70.0, "la première ligne laisse la place du libellé");
    for l in &lines[1..] {
        assert!(pdf::text_width(l, false, 8.0) <= 100.0, "{l}");
    }
    assert_eq!(lines.join(" "), text, "aucun mot perdu");
    // Retours à la ligne gardés ; mot plus long qu'une ligne coupé.
    assert_eq!(pdf::wrap("un\ndeux", false, 8.0, 100.0, 0.0), vec!["un", "deux"]);
    let long = "a".repeat(200);
    let parts = pdf::wrap(&long, false, 8.0, 50.0, 0.0);
    assert!(parts.len() > 3 && parts.concat() == long);
}

fn board(seq: &str, action: &str) -> Board {
    Board {
        title: "Scène 1 — panneau 1/1".into(),
        duration: "2s 00".into(),
        sequence: seq.into(),
        action: action.into(),
        dialogue: String::new(),
        notes: String::new(),
    }
}

#[test]
fn long_texts_end_with_an_ellipsis() {
    let mut b = board("1", &"Une phrase assez longue pour remplir plusieurs lignes. ".repeat(10));
    b.dialogue = "— Bonjour !".into();
    let lines = pdf::board_lines(&b, 250.0, 4);
    assert_eq!(lines.len(), 4);
    assert_eq!(lines[0].label.as_deref(), Some("Action : "));
    assert!(lines[1..].iter().all(|l| l.label.is_none()), "le dialogue ne tient plus");
    assert!(lines[3].text.ends_with('…'));
    // Tout tient : pas de « … », chaque champ a son libellé.
    let short = Board { dialogue: "— Bonjour !".into(), notes: "Plan large".into(), ..board("1", "Il entre.") };
    let lines = pdf::board_lines(&short, 250.0, 6);
    let labels: Vec<_> = lines.iter().filter_map(|l| l.label.clone()).collect();
    assert_eq!(labels, vec!["Action : ", "Dialogue : ", "Notes : "]);
    assert!(lines.iter().all(|l| !l.text.ends_with('…')));
}

#[test]
fn six_per_page_and_a_new_page_per_sequence() {
    let boards: Vec<Board> = (0..14).map(|i| board(if i < 8 { "1" } else { "2" }, "")).collect();
    let pages = pdf::paginate(&boards);
    let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
    assert_eq!(sizes, vec![6, 2, 6]);
    assert_eq!(pages[2][0], 8);
}

/// Petit JPEG gris valide.
fn jpeg() -> BoardImage {
    let img = image::RgbImage::from_pixel(16, 9, image::Rgb([200, 200, 200]));
    let mut data = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut data)
        .encode(img.as_raw(), 16, 9, image::ExtendedColorType::Rgb8)
        .unwrap();
    BoardImage { width: 16, height: 9, jpeg: data }
}

#[test]
fn pdf_structure_is_valid() {
    let boards: Vec<Board> = (0..7).map(|i| board("1", &format!("Panneau {i} : (parenthèses) et \\ barre"))).collect();
    let images: Vec<BoardImage> = (0..7).map(|_| jpeg()).collect();
    let bytes = pdf::storyboard("Mon film — essai", &boards, &images, 16.0 / 9.0);
    assert!(bytes.starts_with(b"%PDF-1.4"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    let text = String::from_utf8_lossy(&bytes);
    // Deux pages, sept images.
    assert_eq!(text.matches("/Type /Page /Parent").count(), 2);
    assert_eq!(text.matches("/Subtype /Image").count(), 7);
    // La table des positions pointe bien sur chaque objet (positions en octets du fichier).
    let tail_at = bytes.windows(10).rposition(|w| w == b"startxref\n").unwrap() + 10;
    let tail = String::from_utf8(bytes[tail_at..].to_vec()).unwrap();
    let start: usize = tail.lines().next().unwrap().parse().unwrap();
    assert!(bytes[start..].starts_with(b"xref\n0 "));
    let table = String::from_utf8(bytes[start..].to_vec()).unwrap();
    let count: usize = table[7..].split_whitespace().next().unwrap().parse().unwrap();
    let entries: Vec<usize> = table.lines().skip(3).take(count - 1).map(|l| l[..10].parse().unwrap()).collect();
    assert_eq!(entries.len(), count - 1);
    for (i, &offset) in entries.iter().enumerate() {
        let head = format!("{} 0 obj", i + 1);
        assert!(bytes[offset..].starts_with(head.as_bytes()), "objet {}", i + 1);
    }
    // Parenthèses et barre échappées, accents en octal.
    assert!(text.contains("\\(parenth\\350ses\\) et \\\\ barre"));
}

#[test]
fn pdf_export_job() {
    let doc = crate::export_tests::doc_with_drawing(8, 2);
    let dir = std::env::temp_dir().join(format!("pelure-test-pdf-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("planches.pdf");
    let frames = export::frames_for(&doc, Range::Montage, 0);
    let panels = export::panels_of(&frames);
    assert_eq!(panels, (0..8).collect::<Vec<_>>());
    let (shots, boards) = export::storyboard_of(&doc, &panels);
    assert_eq!(shots.len(), 8, "sans marque d'image : une planche par plan");
    let mut job = ExportJob::pdf(&out, "Essai", shots, boards, doc.frame);
    let t = std::time::Instant::now();
    while job.outcome.is_none() {
        job.step(&doc, Duration::from_millis(20));
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(60));
    }
    assert!(matches!(job.outcome, Some(Ok(_))), "{:?}", job.outcome);
    let bytes = std::fs::read(&out).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(text.matches("/Type /Page /Parent").count(), 2, "8 planches = 2 pages");
    assert_eq!(text.matches("/Filter /DCTDecode").count(), 8);
    // Annuler : pas de fichier.
    let out2 = dir.join("annule.pdf");
    let (shots, boards) = export::storyboard_of(&doc, &panels);
    let mut job = ExportJob::pdf(&out2, "Essai", shots, boards, doc.frame);
    job.step(&doc, Duration::from_millis(5));
    job.cancel();
    assert!(!out2.exists());
    std::fs::remove_dir_all(&dir).ok();
}
