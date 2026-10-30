//! Essais de l'import d'un scénario : lecture (Fountain, texte brut, Final Draft, Word), plans et
//! séquences pour chaque découpage, durées, personnages, annulation en une étape, gros scénario,
//! fichiers abîmés.

use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::App;
use crate::document::Document;
use crate::scenario::{self, characters, heading_of, parse_fountain, parse_text, Format, Kind, Script, TextOptions};
use crate::scenarioplans::{self, build, Dest, Options, Rhythm, Split};

fn quiet(_: f32) {}

fn fountain(s: &str) -> Script {
    parse_fountain(s, &quiet)
}

fn text(s: &str) -> Script {
    parse_text(s, TextOptions::default(), &quiet)
}

/// Une ligne par élément : `H` intitulé, `A` action (lignes séparées par `/`), `D` réplique (`^` :
/// en même temps que la précédente), `T` transition, `S` section, `=` synopsis, `N` note ; les
/// notes de l'élément à la fin, entre crochets.
fn sum(script: &Script) -> Vec<String> {
    script
        .elements
        .iter()
        .map(|e| {
            let mut s = match &e.kind {
                Kind::Heading(h) => format!("H {h}"),
                Kind::Action(t) => format!("A {}", t.replace('\n', "/")),
                Kind::Dialogue(d) => format!("D {}{}", d.line(), if d.dual { " ^" } else { "" }),
                Kind::Transition(t) => format!("T {t}"),
                Kind::Section(t) => format!("S {t}"),
                Kind::Synopsis(t) => format!("= {t}"),
                Kind::Note(t) => format!("N {t}"),
            };
            if !e.notes.is_empty() {
                s.push_str(&format!(" [{}]", e.notes.join("|")));
            }
            s
        })
        .collect()
}

const COURT: &str = "Title: Le Réveil\nAuthor: Anthony F.\n\nINT. CUISINE - NUIT\n\nMarie cherche sa clé.\n\nMARIE\nOù est-elle ?\n\nPAUL\nCherche encore.\n\nMARIE\nMerci.\n\nElle sort.\n\nCUT TO:\n\nEXT. RUE - JOUR\n\nIl pleut.\n";

// ------------------------------------------------------------------------------------ Fountain

#[test]
fn a_fountain_script_gives_headings_action_dialogue_and_transitions() {
    let s = fountain(
        "Title: Le Réveil\nAuthor: Anthony F.\n\nINT. CUISINE - NUIT\n\nMarie cherche sa clé. Elle *jure* entre ses dents.\n\nMARIE (V.O.)\n(bas)\nOù est-elle passée ?\nJe l'avais posée là.\n\nPAUL (CONT'D)\nCherche encore.\n\nCUT TO:\n\nEXT. RUE - JOUR\n\nIl pleut.\n",
    );
    assert_eq!(s.format, Format::Fountain);
    assert_eq!(s.title, "Le Réveil");
    assert_eq!(s.author, "Anthony F.");
    assert_eq!(
        sum(&s),
        vec![
            "H INT. CUISINE – NUIT",
            "A Marie cherche sa clé. Elle jure entre ses dents.",
            "D MARIE (V.O.) (bas) : Où est-elle passée ? Je l'avais posée là.",
            "D PAUL : Cherche encore.",
            "T CUT TO:",
            "H EXT. RUE – JOUR",
            "A Il pleut.",
        ]
    );
}

#[test]
fn heading_variants_are_recognised_and_ordinary_lines_are_not() {
    assert_eq!(heading_of("INT. CUISINE - NUIT").as_deref(), Some("INT. CUISINE – NUIT"));
    assert_eq!(heading_of("EXT./INT. VOITURE - JOUR").as_deref(), Some("EXT./INT. VOITURE – JOUR"));
    assert_eq!(heading_of("int. cuisine - nuit").as_deref(), Some("int. cuisine – nuit"));
    assert_eq!(heading_of("I/E CAMION - SOIR").as_deref(), Some("I/E CAMION – SOIR"));
    assert_eq!(heading_of("INT. CUISINE - NUIT #12#").as_deref(), Some("INT. CUISINE – NUIT"));
    assert_eq!(heading_of("12. INT. CUISINE - NUIT").as_deref(), Some("INT. CUISINE – NUIT"));
    assert_eq!(heading_of("12A  EXT. RUE - JOUR").as_deref(), Some("EXT. RUE – JOUR"));
    assert_eq!(heading_of("INT. CUISINE - NUIT    14").as_deref(), Some("INT. CUISINE – NUIT"));
    assert_eq!(heading_of(".SOUS-SOL").as_deref(), Some("SOUS-SOL"));
    assert_eq!(heading_of("INTÉRIEUR CUISINE - NUIT").as_deref(), Some("INTÉRIEUR CUISINE – NUIT"));
    // Du français courant n'est pas un intitulé.
    assert_eq!(heading_of("Est-ce que tu viens ?"), None);
    assert_eq!(heading_of("Est présent, il sourit."), None);
    assert_eq!(heading_of("Interrogé par la police, Paul se tait."), None);
    assert_eq!(heading_of("Extérieur de la ville."), None);
    assert_eq!(heading_of(""), None);
}

#[test]
fn dual_dialogue_is_two_lines_marked_as_simultaneous() {
    let s = fountain("INT. RING - NUIT\n\nBRICK\nScrew retirement.\n\nSTEEL ^\nScrew retirement too.\n");
    assert_eq!(sum(&s), vec!["H INT. RING – NUIT", "D BRICK : Screw retirement.", "D STEEL : Screw retirement too. ^"]);
}

#[test]
fn notes_boneyard_and_emphasis_are_handled() {
    let s = fountain(
        "INT. SALON - JOUR\n\nPaul entre. [[Vérifier la porte]] Il pose son sac.\n\n[[Note seule sur la scène]]\n\nMARIE\nSalut ! [[à jouer sec]]\n\nAvant /* caché\nsur deux lignes */ après.\n\n/* tout un bloc\n\nsur deux paragraphes */\n\nUn *mot* en **gras** et _souligné_, 3 * 4, snake_case.\n",
    );
    assert_eq!(
        sum(&s),
        vec![
            "H INT. SALON – JOUR",
            "A Paul entre. Il pose son sac. [Vérifier la porte]",
            "N Note seule sur la scène",
            "D MARIE : Salut ! [à jouer sec]",
            "A Avant après.",
            "A Un mot en gras et souligné, 3 * 4, snake_case.",
        ]
    );
    // Un cimetière jamais refermé emporte la fin du texte, avec un mot pour le dire.
    let open = fountain("INT. SALON - JOUR\n\nTexte.\n\n/* jamais refermé\nTexte perdu.\n");
    assert_eq!(sum(&open), vec!["H INT. SALON – JOUR", "A Texte."]);
    assert!(open.warnings.iter().any(|w| w.contains("n'est pas refermé")));
    // Des crochets qui ne forment pas une note restent du texte.
    let loose = fountain("INT. SALON - JOUR\n\nUn [[ouvert sans fin.\n\nSuite.\n");
    assert_eq!(sum(&loose), vec!["H INT. SALON – JOUR", "A Un [[ouvert sans fin.", "A Suite."]);
}

#[test]
fn forced_elements_override_the_guesses() {
    let s = fountain(".SOUS-SOL\n\n!MARIE regarde la porte.\n\n@Mc Donald\nBonjour.\n\n> FONDU AU NOIR.\n\n>FIN<\n\n~Au clair de la lune\n\nINT. CUISINE - NUIT\n\nLA PORTE CLAQUE.\n");
    assert_eq!(
        sum(&s),
        vec![
            "H SOUS-SOL",
            "A MARIE regarde la porte.",
            "D Mc Donald : Bonjour.",
            "T FONDU AU NOIR.",
            "A FIN",
            "A ♪ Au clair de la lune",
            "H INT. CUISINE – NUIT",
            "A LA PORTE CLAQUE.",
        ]
    );
}

#[test]
fn sections_synopses_page_breaks_and_french_transitions() {
    let s = fountain("# Acte I\n\n## Séquence 1\n\n= Marie cherche sa clé\n\nINT. CUISINE - NUIT\n\n===\n\nElle sort.\n\nFONDU AU NOIR.\n\nSMASH CUT TO:\n\nCOUPÉ À :\n");
    assert_eq!(
        sum(&s),
        vec!["S Acte I", "S Séquence 1", "= Marie cherche sa clé", "H INT. CUISINE – NUIT", "A Elle sort.", "T FONDU AU NOIR.", "T SMASH CUT TO:", "T COUPÉ À :"]
    );
}

#[test]
fn a_shouting_line_with_punctuation_is_not_a_character_but_a_name_with_lines_is() {
    let s = fountain("INT. HALL - JOUR\n\nSTOP !\nIl court.\n\nDR. MARTIN\nBonjour.\n\nAGENT 47\n(à l'oreillette)\nRepli.\n");
    assert_eq!(sum(&s), vec!["H INT. HALL – JOUR", "A STOP !/Il court.", "D DR. MARTIN : Bonjour.", "D AGENT 47 (à l'oreillette) : Repli."]);
}

#[test]
fn windows_line_ends_a_byte_order_mark_and_a_missing_final_newline_are_fine() {
    let s = fountain("\u{feff}INT. CUISINE - NUIT\r\n\r\nMarie entre.\r\n\r\nMARIE\r\nSalut.");
    assert_eq!(sum(&s), vec!["H INT. CUISINE – NUIT", "A Marie entre.", "D MARIE : Salut."]);
}

#[test]
fn a_heading_directly_followed_by_action_or_dialogue_is_still_a_heading() {
    let s = fountain("INT. CUISINE - NUIT\nMarie entre.\n\nEXT. RUE - JOUR\nMARIE\nSalut.\n");
    assert_eq!(sum(&s), vec!["H INT. CUISINE – NUIT", "A Marie entre.", "H EXT. RUE – JOUR", "D MARIE : Salut."]);
}

// ------------------------------------------------------------------------------------ texte brut

#[test]
fn plain_text_uses_screenplay_heuristics() {
    let s = text(
        "INT. CUISINE - NUIT\n\nMarie cherche sa clé.\nElle fouille le tiroir.\n\nMARIE\nOù est-elle ?\n\nPAUL : Cherche encore.\nSFX : porte qui grince\n\n12.\n\n(SUITE)\n\nCUT TO:\n\nEXT. RUE - JOUR\n\nIl pleut.\n",
    );
    assert_eq!(s.format, Format::Texte);
    assert_eq!(
        sum(&s),
        vec![
            "H INT. CUISINE – NUIT",
            "A Marie cherche sa clé./Elle fouille le tiroir.",
            "D MARIE : Où est-elle ?",
            "D PAUL : Cherche encore.",
            "A SFX : porte qui grince",
            "T CUT TO:",
            "H EXT. RUE – JOUR",
            "A Il pleut.",
        ]
    );
}

#[test]
fn indented_text_copied_from_a_pdf_keeps_its_speakers_and_drops_the_page_furniture() {
    let s = text(
        "          INT. CUISINE - NUIT                     3\n\nMarie entre.\n\n                    MARIE\n          (bas)\n     Où est-elle ?\n     Je ne la trouve pas.\n                    PAUL\n     Cherche encore.\n\n                                   4.\n          (CONTINUED)\n\nMarie se rassoit.\n",
    );
    assert_eq!(
        sum(&s),
        vec![
            "H INT. CUISINE – NUIT",
            "A Marie entre.",
            "D MARIE (bas) : Où est-elle ? Je ne la trouve pas.",
            "D PAUL : Cherche encore.",
            "A Marie se rassoit.",
        ]
    );
}

#[test]
fn without_any_heading_the_text_still_reads_and_says_so() {
    let bytes = "Marie entre.\n\nMARIE\nSalut.\n".as_bytes();
    let s = scenario::parse_bytes(bytes, "txt", &quiet).unwrap();
    assert_eq!(sum(&s), vec!["A Marie entre.", "D MARIE : Salut."]);
    assert!(s.warnings.iter().any(|w| w.contains("Aucun intitulé")));
}

#[test]
fn old_windows_files_are_decoded() {
    // « Où » et « é » en Windows-1252.
    let bytes = b"INT. CAF\xc9 - NUIT\n\nO\xf9 est-il ?\n";
    let s = scenario::parse_bytes(bytes, "txt", &quiet).unwrap();
    assert_eq!(sum(&s), vec!["H INT. CAFÉ – NUIT", "A Où est-il ?"]);
    // UTF-16 avec marque.
    let mut utf16 = vec![0xFF, 0xFE];
    for u in "INT. CAFÉ - NUIT\n\nSalut.\n".encode_utf16() {
        utf16.extend(u.to_le_bytes());
    }
    let s = scenario::parse_bytes(&utf16, "fountain", &quiet).unwrap();
    assert_eq!(sum(&s), vec!["H INT. CAFÉ – NUIT", "A Salut."]);
}

// ------------------------------------------------------------------------------------ Final Draft, Word

const FDX: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no" ?>
<FinalDraft DocumentType="Script" Template="No" Version="2">
  <Content>
    <Paragraph Type="Scene Heading"><SceneProperties Length="1/8" Page="1" Title=""/><Text>INT. CUISINE - NUIT</Text></Paragraph>
    <Paragraph Type="Action"><Text>Marie </Text><Text Style="Bold">cherche</Text><Text> sa clé &amp; son sac.</Text></Paragraph>
    <Paragraph Type="Character"><Text>MARIE (V.O.)</Text></Paragraph>
    <Paragraph Type="Parenthetical"><Text>(bas)</Text></Paragraph>
    <Paragraph Type="Dialogue"><Text>Où est-elle ?</Text></Paragraph>
    <Paragraph Type="Dialogue"><Text>Je l'avais posée.</Text></Paragraph>
    <DualDialogue>
      <Paragraph Type="Character"><Text>PAUL</Text></Paragraph>
      <Paragraph Type="Dialogue"><Text>Ici.</Text></Paragraph>
      <Paragraph Type="Character"><Text>LUC</Text></Paragraph>
      <Paragraph Type="Dialogue"><Text>Là.</Text></Paragraph>
    </DualDialogue>
    <Paragraph Type="Transition"><Text>CUT TO:</Text></Paragraph>
    <Paragraph Type="Action"><Text>Fin.</Text><ScriptNote ID="1"><Paragraph><Text>à revoir</Text></Paragraph></ScriptNote></Paragraph>
    <Paragraph Type="Action"><Text></Text></Paragraph>
  </Content>
  <TitlePage><Content>
    <Paragraph Type="Action" Alignment="Center"><Text>Le Réveil</Text></Paragraph>
    <Paragraph Type="Action" Alignment="Center"><Text>Written by</Text></Paragraph>
    <Paragraph Type="Action" Alignment="Center"><Text>Anthony F.</Text></Paragraph>
  </Content></TitlePage>
</FinalDraft>"#;

#[test]
fn a_final_draft_script_is_read_by_paragraph_type() {
    let s = scenario::parse_bytes(FDX.as_bytes(), "fdx", &quiet).unwrap();
    assert_eq!(s.format, Format::Fdx);
    assert_eq!(s.title, "Le Réveil");
    assert_eq!(s.author, "Anthony F.");
    assert_eq!(
        sum(&s),
        vec![
            "H INT. CUISINE – NUIT",
            "A Marie cherche sa clé & son sac.",
            "D MARIE (V.O.) (bas) : Où est-elle ? Je l'avais posée.",
            "D PAUL : Ici.",
            "D LUC : Là. ^",
            "T CUT TO:",
            "A Fin. [à revoir]",
        ]
    );
}

#[test]
fn a_final_draft_file_is_recognised_without_its_extension() {
    let s = scenario::parse_bytes(FDX.as_bytes(), "txt", &quiet).unwrap();
    assert_eq!(s.format, Format::Fdx);
}

/// Un zip d'un seul fichier, rangé tel quel (méthode 0).
fn zip_store(name: &str, data: &[u8]) -> Vec<u8> {
    let mut z = Vec::new();
    let (size, nlen) = (data.len() as u32, name.len() as u16);
    z.extend(0x0403_4b50u32.to_le_bytes());
    z.extend([20, 0, 0, 0, 0, 0]);
    z.extend([0u8; 8]);
    z.extend(size.to_le_bytes());
    z.extend(size.to_le_bytes());
    z.extend(nlen.to_le_bytes());
    z.extend([0, 0]);
    z.extend(name.bytes());
    z.extend(data);
    let cd = z.len() as u32;
    z.extend(0x0201_4b50u32.to_le_bytes());
    z.extend([20, 0, 20, 0, 0, 0, 0, 0]);
    z.extend([0u8; 8]);
    z.extend(size.to_le_bytes());
    z.extend(size.to_le_bytes());
    z.extend(nlen.to_le_bytes());
    z.extend([0u8; 12]);
    z.extend(0u32.to_le_bytes());
    z.extend(name.bytes());
    let cd_size = z.len() as u32 - cd;
    z.extend(0x0605_4b50u32.to_le_bytes());
    z.extend([0, 0, 0, 0]);
    z.extend(1u16.to_le_bytes());
    z.extend(1u16.to_le_bytes());
    z.extend(cd_size.to_le_bytes());
    z.extend(cd.to_le_bytes());
    z.extend([0, 0]);
    z
}

fn docx(paragraphs: &[(&str, &str)]) -> Vec<u8> {
    let mut xml = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#);
    for (style, text) in paragraphs {
        xml.push_str("<w:p>");
        if !style.is_empty() {
            xml.push_str(&format!(r#"<w:pPr><w:pStyle w:val="{style}"/></w:pPr>"#));
        }
        // Le texte est coupé en deux morceaux, l'espace de fin gardé (xml:space="preserve").
        let mid = (0..=text.len() / 2).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
        let (a, b) = text.split_at(mid);
        xml.push_str(&format!(r#"<w:r><w:t xml:space="preserve">{a}</w:t></w:r><w:r><w:t>{b}</w:t></w:r></w:p>"#));
    }
    xml.push_str("</w:body></w:document>");
    zip_store("word/document.xml", xml.as_bytes())
}

#[test]
fn a_word_document_with_screenplay_styles_is_read_by_style() {
    let bytes = docx(&[("SceneHeading", "INT. CUISINE - NUIT"), ("Action", "Marie entre."), ("Character", "MARIE"), ("Parenthetical", "(bas)"), ("Dialogue", "Salut.")]);
    let s = scenario::parse_bytes(&bytes, "docx", &quiet).unwrap();
    assert_eq!(s.format, Format::Docx);
    assert_eq!(sum(&s), vec!["H INT. CUISINE – NUIT", "A Marie entre.", "D MARIE (bas) : Salut."]);
}

#[test]
fn a_plain_word_document_goes_through_the_text_heuristics_one_paragraph_per_line() {
    let bytes = docx(&[("", "INT. CUISINE - NUIT"), ("", "Marie entre."), ("", "Elle s'assoit."), ("", "MARIE"), ("", "Salut.")]);
    let s = scenario::parse_bytes(&bytes, "docx", &quiet).unwrap();
    assert_eq!(s.format, Format::Docx);
    assert_eq!(sum(&s), vec!["H INT. CUISINE – NUIT", "A Marie entre.", "A Elle s'assoit.", "D MARIE : Salut."]);
}

// ------------------------------------------------------------------------------------ erreurs

#[test]
fn damaged_input_gives_a_french_error_never_a_panic() {
    let err = |bytes: &[u8], ext: &str| scenario::parse_bytes(bytes, ext, &quiet).unwrap_err();
    assert!(err(b"", "txt").contains("vide"));
    let pdf = err(b"%PDF-1.4\n1 0 obj", "pdf");
    assert!(pdf.contains("PDF") && pdf.contains(".fountain") && pdf.contains(".txt"), "{pdf}");
    assert!(err(b"%PDF-1.7 camouflage", "txt").contains("PDF"));
    assert!(err(&[0, 1, 2, 0, 0, 3, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0], "txt").contains("ne ressemble pas à du texte"));
    assert!(err(b"   \n\n \t\n", "fountain").contains("Aucun scénario reconnu"));
    assert!(err(b"# Seulement une section\n\n= et un synopsis\n", "fountain").contains("Aucun scénario reconnu"));
    assert!(err(b"<FinalDraft><Content><Paragraph Type=\"Action\"><Text>Salut", "fdx").contains("illisible"));
    assert!(err(b"<?xml version=\"1.0\"?><html><body/></html>", "fdx").contains("n'est pas un scénario Final Draft"));
    assert!(err(b"<FinalDraft></FinalDraft>", "fdx").contains("aucun paragraphe"));
    assert!(err(b"PK\x03\x04 pas un vrai zip", "docx").contains("Word"));
    let no_text = zip_store("word/styles.xml", b"<w:styles/>");
    assert!(err(&no_text, "docx").contains("word/document.xml"));
    let broken = zip_store("word/document.xml", b"<w:document><w:body><w:p><w:t>Salut</w:p></w:body></w:document>");
    assert!(err(&broken, "docx").contains("Word illisible"));
    assert!(err(b"x", "doc").contains(".docx"));
    assert!(err(b"{\\rtf1 abc}", "rtf").contains(".rtf"));
}

#[test]
fn a_missing_file_and_a_folder_are_reported() {
    let missing = std::env::temp_dir().join(format!("shot-scenario-absent-{}.fountain", std::process::id()));
    let e = scenario::read_file(&missing, &quiet).unwrap_err();
    assert!(e.starts_with("Impossible de lire"), "{e}");
    let e = scenario::read_file(&std::env::temp_dir(), &quiet).unwrap_err();
    assert!(e.contains("n'est pas un fichier"), "{e}");
}

#[test]
fn odd_inputs_never_panic_the_readers() {
    let pieces = [
        "[[", "]]", "/*", "*/", "\u{e000}12\u{e001}", "\u{e000}", "#", "##", "=", "===", ">", "<", "> x <", "@", "!", "~", "^", "(", ")", "()", "(V.O.", ".", "..", "INT.", "EXT. X #", "12.", "12A",
        "\n", "\n\n", "  ", "\t", "MARIE", "MARIE ^", "MARIE (V.O.) (", "É", "ß", "日本語", ":", " : ", "SFX :", "CUT TO:", "Title:", "Title: x\n  y", "\r", "%", "–", "- ", "\u{feff}",
    ];
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..400 {
        let n = 1 + (next() % 40) as usize;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(pieces[(next() % pieces.len() as u64) as usize]);
        }
        for script in [fountain(&s), text(&s), parse_text(&s, TextOptions { line_is_paragraph: true }, &quiet)] {
            for split in Split::ALL {
                let outline = build(&script, &Options { split, ..Options::default() });
                let mut doc = Document::with_frame(320, 180, 24);
                scenarioplans::append_to_project(&mut doc, &outline, Dest::End, 0);
                assert!(doc.is_plans() || outline.plan_count() == 0, "{s:?}");
            }
        }
        // Les octets bruts, par le chemin des fichiers.
        let _ = scenario::parse_bytes(s.as_bytes(), "fountain", &quiet);
        let _ = scenario::parse_bytes(s.as_bytes(), "fdx", &quiet);
        let _ = scenario::parse_bytes(s.as_bytes(), "docx", &quiet);
    }
}

// ------------------------------------------------------------------------------------ plans

#[test]
fn the_three_splits_give_the_documented_plans() {
    let script = fountain(COURT);
    let titles = |o: &scenarioplans::Outline| o.sequences.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
    let counts = |o: &scenarioplans::Outline| o.sequences.iter().map(|s| s.plans.len()).collect::<Vec<_>>();

    // Un plan par scène : les blocs sont ses images de storyboard.
    let by_scene = build(&script, &Options { split: Split::Scene, ..Options::default() });
    assert_eq!(titles(&by_scene), vec!["INT. CUISINE – NUIT", "EXT. RUE – JOUR"]);
    assert_eq!(counts(&by_scene), vec![1, 1]);
    assert_eq!(by_scene.sequences[0].plans[0].beats.len(), 5);

    // Un plan par paragraphe d'action et par échange.
    let by_paragraph = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    assert_eq!(counts(&by_paragraph), vec![3, 1]);
    let exchange = &by_paragraph.sequences[0].plans[1].beats[0];
    assert_eq!(exchange.dialogue, "MARIE : Où est-elle ?\nPAUL : Cherche encore.\nMARIE : Merci.");
    assert!(exchange.action.is_empty());

    // Un plan par réplique.
    let by_line = build(&script, &Options { split: Split::Line, ..Options::default() });
    assert_eq!(counts(&by_line), vec![5, 1]);
    assert_eq!(by_line.sequences[0].plans[2].beats[0].dialogue, "PAUL : Cherche encore.");
    assert_eq!(by_line.plan_count(), 6);
}

#[test]
fn texts_go_to_the_right_fields_and_transitions_and_scene_become_notes() {
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    let first = &o.sequences[0].plans[0].beats[0];
    assert_eq!(first.action, "Marie cherche sa clé.");
    assert_eq!(first.notes, vec!["Décor : INT. CUISINE – NUIT".to_string()]);
    let last = &o.sequences[0].plans[2].beats[0];
    assert_eq!(last.action, "Elle sort.");
    assert_eq!(last.notes, vec!["Transition : CUT TO:".to_string()]);
    // Sans l'option, pas de décor dans les notes.
    let plain = build(&script, &Options { split: Split::Paragraph, scene_in_notes: false, ..Options::default() });
    assert!(plain.sequences[0].plans[0].beats[0].notes.is_empty());
    // Le décor du deuxième plan d'une séquence n'est pas répété.
    assert!(o.sequences[0].plans[1].beats[0].notes.is_empty());
}

#[test]
fn sections_synopses_and_stray_notes_find_a_plan() {
    let script = fountain("# Acte I\n\n[[avant tout]]\n\nINT. CUISINE - NUIT\n\n= Marie cherche sa clé\n\nMarie entre.\n\nEXT. RUE - JOUR\n\nIl pleut.\n\n# Acte II\n");
    let o = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    // Rien avant le premier intitulé ne fait une séquence : section et note vont à sa scène.
    assert_eq!(o.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["INT. CUISINE – NUIT", "EXT. RUE – JOUR"]);
    let notes_of = |s: usize, p: usize| o.sequences[s].plans[p].beats[0].notes.clone();
    let expected: Vec<String> = ["Décor : INT. CUISINE – NUIT", "Section : Acte I", "Note : avant tout", "Synopsis : Marie cherche sa clé"].map(String::from).to_vec();
    assert_eq!(notes_of(0, 0), expected);
    // Une section sans scène derrière elle devient une note du dernier plan.
    assert_eq!(notes_of(1, 0), vec!["Décor : EXT. RUE – JOUR".to_string(), "Section : Acte II".to_string()]);

    // De l'action avant le premier intitulé fait la séquence « Ouverture ».
    let opening = fountain("Un long plan sur la mer.\n\nINT. CUISINE - NUIT\n\nMarie entre.\n");
    let o = build(&opening, &Options::default());
    assert_eq!(o.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Ouverture", "INT. CUISINE – NUIT"]);
    assert!(o.sequences[0].plans[0].beats[0].notes.is_empty(), "pas de décor à noter pour Ouverture");
}

#[test]
fn the_sample_script_of_the_docs_imports_into_the_expected_structure() {
    let script = scenario::parse_bytes(include_bytes!("../docs/exemples/Le-Reveil.fountain"), "fountain", &quiet).unwrap();
    assert_eq!(script.title, "Le Réveil");
    assert_eq!(script.author, "Anthony F.");
    assert!(script.warnings.is_empty(), "{:?}", script.warnings);
    let o = build(&script, &Options::default());
    assert_eq!(o.sequences.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["INT. CUISINE – NUIT", "EXT. RUE – AUBE", "INT. CUISINE – JOUR"]);
    let names: Vec<&str> = o.characters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["MARIE", "PAUL"]);
    let mut doc = blank_project();
    scenarioplans::fill_new_project(&mut doc, &o, &script.title, &script.author).unwrap();
    assert!(doc.is_plans());
    // Le son : une ligne « SFX : … » reste dans l'action, les répliques sont « NOM : … ».
    assert!(doc.panels[0].action.contains("SFX : tiroir qui grince"));
    assert!(doc.panels.iter().any(|p| p.dialogue.contains("PAUL (derrière la porte) : Ouvre.")));
    assert!(doc.panels.iter().any(|p| p.dialogue == "MARIE : Tu as la clé ?\nPAUL : Tu as la clé ?"), "répliques simultanées : deux lignes dans le même plan");
    assert!(doc.panels.iter().any(|p| p.notes.contains("Transition : CUT TO:")));
    assert!(doc.panels.iter().any(|p| p.notes.contains("Transition : FONDU AU NOIR.")));
    assert!(doc.panels.iter().any(|p| p.notes.contains("Note : Plan fixe, rester long sur le rideau")));
    assert!(doc.panels[0].notes.contains("Section : Acte I") && doc.panels[0].notes.contains("Synopsis : Marie cherche sa clé"));
}

#[test]
fn a_scene_without_content_still_makes_one_plan_and_an_empty_script_makes_none() {
    let script = fountain("INT. CUISINE - NUIT\n\nEXT. RUE - JOUR\n\nIl pleut.\n");
    for split in Split::ALL {
        let o = build(&script, &Options { split, ..Options::default() });
        assert_eq!(o.sequences.iter().map(|s| s.plans.len()).collect::<Vec<_>>(), vec![1, 1], "{split:?}");
    }
    let nothing = fountain("# Titre seul\n");
    assert_eq!(build(&nothing, &Options::default()).plan_count(), 0);
}

#[test]
fn the_duration_heuristic_is_documented_and_adjustable() {
    let r = Rhythm::default();
    assert_eq!((r.words_per_second, r.seconds_per_action_line, r.min_seconds), (2.5, 1.0, 1.0));
    // Répliques : 2,5 mots par seconde.
    assert_eq!(r.speech_seconds(10), 4.0);
    // Action : une seconde par ligne de 60 signes.
    assert_eq!(r.action_seconds("Une ligne."), 1.0);
    assert_eq!(r.action_seconds(&"a".repeat(60)), 1.0);
    assert_eq!(r.action_seconds(&"a".repeat(61)), 2.0);
    assert_eq!(r.action_seconds("Une.\nDeux.\nTrois."), 3.0);
    assert_eq!(r.action_seconds(""), 0.0);
    // Au moins une seconde, au plus trente.
    assert_eq!(r.clamp(0.2), 1.0);
    assert_eq!(r.clamp(100.0), 30.0);
    assert_eq!(r.clamp(7.5), 7.5);
    // Une case vidée ou absurde ne casse rien.
    let odd = Rhythm { words_per_second: 0.0, seconds_per_action_line: f32::NAN, min_seconds: -4.0, max_seconds: f32::INFINITY }.sane();
    assert!(odd.words_per_second >= 0.2 && odd.seconds_per_action_line.is_finite() && odd.min_seconds >= 0.1 && odd.max_seconds >= odd.min_seconds);

    // Un plan d'échange : 6 mots, 2,4 s ; un plan d'action d'une ligne : 1 s.
    let o = build(&fountain(COURT), &Options { split: Split::Paragraph, ..Options::default() });
    let secs = |p: usize| o.sequences[0].plans[p].seconds();
    assert_eq!(secs(0), 1.0);
    assert!((secs(1) - 2.4).abs() < 1e-4, "{}", secs(1));
    // Un autre rythme change les durées.
    let slow = build(&fountain(COURT), &Options { split: Split::Paragraph, rhythm: Rhythm { words_per_second: 1.0, ..Rhythm::default() }, ..Options::default() });
    assert!((slow.sequences[0].plans[1].seconds() - 6.0).abs() < 1e-4);
}

// ------------------------------------------------------------------------------------ dans le projet

fn blank_project() -> Document {
    Document::with_frame(640, 360, 24).created_now()
}

#[test]
fn a_new_project_receives_sequences_plans_texts_and_durations() {
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    let mut doc = blank_project();
    let applied = scenarioplans::fill_new_project(&mut doc, &o, &script.title, &script.author).unwrap();
    assert_eq!((applied.first, applied.plans, applied.sequences), (0, 4, 2));
    assert!(doc.is_plans(), "chaque plan est une scène à un seul panneau");
    assert_eq!(doc.properties.title, "Le Réveil");
    assert_eq!(doc.properties.author, "Anthony F.");
    assert_eq!(doc.sequence_order().len(), 2);
    assert_eq!(doc.sequence_label(doc.sequence_order()[0]), "INT. CUISINE – NUIT");
    assert_eq!(doc.sequence_label(doc.sequence_order()[1]), "EXT. RUE – JOUR");
    let p = &doc.panels;
    assert_eq!(p[0].action, "Marie cherche sa clé.");
    assert_eq!(p[0].notes, "Décor : INT. CUISINE – NUIT");
    assert_eq!(p[1].dialogue, "MARIE : Où est-elle ?\nPAUL : Cherche encore.\nMARIE : Merci.");
    assert_eq!(p[2].notes, "Transition : CUT TO:");
    assert_eq!(p[3].action, "Il pleut.");
    // Durées en images à 24 i/s : 1 s, 2,4 s (57,6 images, arrondies), 1 s, 1 s.
    assert_eq!(p.iter().map(|x| x.length).collect::<Vec<_>>(), vec![24, 58, 24, 24]);
    assert!(p.iter().all(|x| x.marks.is_empty()));
    // Les personnages sont dans les notes du projet.
    assert_eq!(doc.properties.notes, "Personnages du scénario :\n- MARIE (2 répliques)\n- PAUL (1 réplique)");
}

#[test]
fn one_plan_per_scene_makes_storyboard_images_with_their_own_texts() {
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Scene, ..Options::default() });
    let mut doc = blank_project();
    scenarioplans::fill_new_project(&mut doc, &o, "", "").unwrap();
    assert!(doc.is_plans());
    assert_eq!(doc.panels.len(), 2);
    let p = &doc.panels[0];
    assert_eq!(p.action, "Marie cherche sa clé.");
    assert_eq!(p.marks.len(), 4);
    assert_eq!(p.marks.iter().map(|m| m.frame).collect::<Vec<_>>(), vec![24, 24 + 29, 24 + 29 + 24, 24 + 29 + 24 + 24]);
    assert_eq!(p.marks[0].dialogue, "MARIE : Où est-elle ?");
    assert_eq!(p.marks[2].dialogue, "MARIE : Merci.");
    assert_eq!(p.marks[3].action, "Elle sort.");
    assert_eq!(p.marks[3].notes, "Transition : CUT TO:");
    assert!(p.marks.windows(2).all(|w| w[0].frame < w[1].frame && w[0].id != w[1].id));
    assert!(p.marks.iter().all(|m| m.frame > 0 && m.frame < p.length));
    assert_eq!(p.length, 24 + 29 + 24 + 24 + 24, "somme des images de ses blocs");
    // Le titre du projet, faute de titre dans le scénario, reste celui du projet neuf.
    assert_eq!(doc.properties.title, "");
}

#[test]
fn one_plan_per_line_gives_a_plan_per_speaker_turn() {
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Line, ..Options::default() });
    let mut doc = blank_project();
    scenarioplans::fill_new_project(&mut doc, &o, "", "").unwrap();
    assert_eq!(doc.panels.len(), 6);
    assert_eq!(doc.panels.iter().map(|p| p.dialogue.as_str()).collect::<Vec<_>>(), vec!["", "MARIE : Où est-elle ?", "PAUL : Cherche encore.", "MARIE : Merci.", "", ""]);
    assert!(doc.is_plans());
}

#[test]
fn appending_replaces_a_blank_project_and_otherwise_goes_after_the_sequence_of_the_current_plan() {
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    // Projet vierge : son plan vide est remplacé.
    let mut doc = blank_project();
    assert!(scenarioplans::is_blank(&doc));
    scenarioplans::append_to_project(&mut doc, &o, Dest::End, 0).unwrap();
    assert_eq!(doc.panels.len(), 4);
    assert!(!scenarioplans::is_blank(&doc));
    assert!(doc.is_plans());

    // Projet déjà rempli : à la fin, ou après la séquence du plan courant (une séquence ne se coupe pas).
    let names = |d: &Document| d.sequence_order().iter().map(|s| d.sequence_label(*s)).collect::<Vec<_>>();
    let other = fountain("INT. SALON - JOUR\n\nTexte.\n");
    let small = build(&other, &Options::default());
    let mut at_end = doc_clone(&doc);
    let a = scenarioplans::append_to_project(&mut at_end, &small, Dest::End, 0).unwrap();
    assert_eq!((a.first, at_end.panels.len()), (4, 5));
    assert_eq!(names(&at_end), vec!["INT. CUISINE – NUIT", "EXT. RUE – JOUR", "INT. SALON – JOUR"]);

    let mut after = doc_clone(&doc);
    // Le plan 1 est au milieu de la première séquence (3 plans) : l'import va après ses trois plans.
    let a = scenarioplans::append_to_project(&mut after, &small, Dest::AfterCurrent, 1).unwrap();
    assert_eq!(a.first, 3);
    assert_eq!(names(&after), vec!["INT. CUISINE – NUIT", "INT. SALON – JOUR", "EXT. RUE – JOUR"]);
    assert_eq!(after.panels[3].action, "Texte.");
    assert!(after.is_plans());
    let ids: std::collections::HashSet<u64> = after.panels.iter().map(|p| p.id).collect();
    assert_eq!(ids.len(), after.panels.len());

    // Rien à importer : le projet ne bouge pas.
    let none = build(&fountain("# Section seule\n"), &Options::default());
    assert!(scenarioplans::append_to_project(&mut after, &none, Dest::End, 0).is_none());
    assert_eq!(after.panels.len(), 5);
}

fn doc_clone(d: &Document) -> Document {
    // (Un Document ne se clone pas : on le refait de la même façon.)
    let script = fountain(COURT);
    let o = build(&script, &Options { split: Split::Paragraph, ..Options::default() });
    let mut doc = blank_project();
    assert_eq!(d.panels.len(), 4);
    scenarioplans::fill_new_project(&mut doc, &o, &script.title, &script.author);
    doc
}

#[test]
fn characters_are_counted_and_noted_in_the_project_notes() {
    let script = fountain("INT. A - JOUR\n\nMARIE\nUn deux trois.\n\nPAUL (V.O.)\nQuatre.\n\nMARIE (CONT'D)\nCinq six.\n\n@Mc Donald\nSept.\n");
    let found = characters(&script.elements);
    assert_eq!(found.iter().map(|c| (c.name.as_str(), c.lines, c.words)).collect::<Vec<_>>(), vec![("MARIE", 2, 5), ("PAUL", 1, 1), ("Mc Donald", 1, 1)]);

    // Un autre import s'ajoute à la liste, sans toucher au reste des notes.
    let mut doc = blank_project();
    doc.properties.notes = "À faire : musique.".into();
    scenarioplans::note_characters(&mut doc, &found);
    assert_eq!(doc.properties.notes, "À faire : musique.\n\nPersonnages du scénario :\n- MARIE (2 répliques)\n- Mc Donald (1 réplique)\n- PAUL (1 réplique)");
    scenarioplans::note_characters(&mut doc, &found[..1]);
    assert_eq!(scenarioplans::read_characters(&doc.properties.notes), vec![("MARIE".to_string(), 4), ("Mc Donald".to_string(), 1), ("PAUL".to_string(), 1)]);
    assert!(doc.properties.notes.starts_with("À faire : musique."));
    // Le bloc est remplacé sur place : les notes écrites après lui restent.
    let text = "Avant\n\nPersonnages du scénario :\n- A (1 réplique)\n\nAprès";
    assert_eq!(scenarioplans::write_characters(text, &[("B".into(), 3)]), "Avant\n\nPersonnages du scénario :\n- B (3 répliques)\n\nAprès");
}

// ------------------------------------------------------------------------------------ gros scénario

fn long_script(scenes: usize) -> String {
    let mut s = String::from("Title: Épopée\nAuthor: Test\n\n");
    for i in 0..scenes {
        s.push_str(&format!("INT. LIEU {i} - {}\n\n", if i % 2 == 0 { "JOUR" } else { "NUIT" }));
        for k in 0..3 {
            s.push_str(&format!("Paul traverse la pièce {k} en regardant par la fenêtre.\nIl s'arrête, hésite, puis repart vers la porte.\n\n"));
        }
        for k in 0..6 {
            let who = ["MARIE", "PAUL", "LUC"][k % 3];
            s.push_str(&format!("{who}\n(bas)\nJe ne sais pas si on doit continuer comme ça, la nuit tombe déjà.\nOn en parle demain ?\n\n"));
        }
        s.push_str("CUT TO:\n\n");
    }
    s
}

#[test]
fn a_120_page_script_reads_and_builds_in_under_a_second() {
    // Environ 43 lignes par scène : 160 scènes font plus de 6 600 lignes, les 55 lignes d'une
    // page de scénario sur 120 pages.
    let source = long_script(160);
    let lines = source.lines().count();
    assert!(lines > 6600, "{lines} lignes");
    let t = Instant::now();
    let script = fountain(&source);
    assert_eq!(script.headings(), 160);
    for split in Split::ALL {
        let o = build(&script, &Options { split, ..Options::default() });
        assert!(o.plan_count() >= 160);
        let mut doc = blank_project();
        scenarioplans::fill_new_project(&mut doc, &o, &script.title, &script.author).unwrap();
        assert!(doc.is_plans());
    }
    let elapsed = t.elapsed();
    assert!(elapsed < Duration::from_secs(1), "lecture + 3 découpages + 3 projets : {elapsed:?}");

    // En texte brut aussi.
    let t = Instant::now();
    let plain = text(&source);
    assert_eq!(plain.headings(), 160);
    assert_eq!(characters(&plain.elements).len(), 3);
    assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
}

// ------------------------------------------------------------------------------------ dans l'application

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-scenario-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    (ctx, app)
}

/// Fait tourner la fenêtre jusqu'à ce que la lecture soit finie.
fn wait_for_reading(ctx: &egui::Context, app: &mut App) {
    let t = Instant::now();
    loop {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
        let w = app.scenario_window.as_ref().expect("fenêtre ouverte");
        if w.ready() || w.failure().is_some() {
            return;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "lecture trop longue");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn step_names(app: &App) -> Vec<String> {
    app.history.names().0.into_iter().map(str::to_string).collect()
}

#[test]
fn the_import_is_one_undo_step_named_after_the_command() {
    let (_ctx, mut app) = headless("undo");
    let script = fountain(COURT);
    // Un projet qui a déjà du contenu.
    app.doc.panels[0].action = "Plan existant.".into();
    let before = app.doc.panels.len();
    // L'import garde le scénario : une étape, aucun plan.
    let steps_start = step_names(&app).len();
    assert!(app.scenario_keep(&script, "essai"));
    assert_eq!(app.doc.panels.len(), before, "l'import ne crée aucun plan");
    assert!(app.doc.scenario.is_some());
    assert_eq!(step_names(&app).len(), steps_start + 1);
    assert_eq!(step_names(&app).last().map(String::as_str), Some("Importer un scénario"));
    let steps_before = step_names(&app).len();
    for split in Split::ALL {
        let o = build(&script, &Options { split, ..Options::default() });
        assert!(app.scenario_into_project(&o, Dest::End, "essai", &script));
    }
    let after = app.doc.panels.len();
    assert_eq!(after, before + 2 + 4 + 6, "trois créations : 2 + 4 + 6 plans");
    assert_eq!(step_names(&app).len(), steps_before + 3, "une étape par création");
    assert_eq!(step_names(&app).last().map(String::as_str), Some("Créer les plans du scénario"));
    assert_eq!(app.doc.scenario.as_ref().unwrap().liens.len(), 6, "les liens sont ceux de la dernière création");

    // Annuler défait le dernier import en entier, rétablir le refait.
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), before + 2 + 4);
    app.run_action("edition.annuler");
    app.run_action("edition.annuler");
    assert_eq!(app.doc.panels.len(), before);
    assert_eq!(app.doc.panels[0].action, "Plan existant.");
    assert_eq!(app.doc.properties.notes, "", "la liste des personnages est annulée avec le reste");
    assert!(app.doc.scenario.as_ref().unwrap().liens.is_empty(), "le scénario gardé reste, sans lien");
    app.run_action("edition.annuler");
    assert!(app.doc.scenario.is_none(), "annuler l'import retire aussi le scénario");
    app.run_action("edition.retablir");
    assert!(app.doc.scenario.is_some());
    app.run_action("edition.retablir");
    assert_eq!(app.doc.panels.len(), before + 2);
    assert!(app.doc.properties.notes.contains("MARIE"));
}

#[test]
fn the_window_reads_on_another_thread_then_imports_into_the_open_project_or_a_new_one() {
    let (ctx, mut app) = headless("fenetre");
    let dir = std::env::temp_dir().join(format!("shot-scenario-fichiers-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("Le Réveil.fountain");
    std::fs::write(&file, COURT).unwrap();

    // Lecture, puis dessin de la fenêtre sans plantage.
    app.read_scenario(file.clone());
    wait_for_reading(&ctx, &mut app);
    assert!(app.scenario_window.as_ref().unwrap().ready());
    for _ in 0..3 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
    }

    // Dans un nouveau projet : le projet ouvert est remplacé, au titre du scénario ; le scénario est gardé, aucun plan créé.
    assert_eq!(app.scenario_window.as_ref().unwrap().mode(), crate::scenarioui::Mode::Garder);
    app.scenario_new_project();
    assert!(app.scenario_window.is_none());
    assert_eq!(app.doc.properties.title, "Le Réveil");
    assert!(app.doc.is_plans());
    assert_eq!(app.doc.panels.len(), 1, "un plan vierge, comme tout projet neuf");
    assert!(app.doc.scenario.as_ref().is_some_and(|g| g.nom == "Le Réveil.fountain" && g.liens.is_empty()));
    assert!(app.modified && app.path.is_none());
    assert!(app.status.contains("nouveau projet"), "{}", app.status);

    // « Créer les plans depuis le scénario… » : la fenêtre part du scénario gardé, sans fichier.
    app.run_action("scenario.creer_plans");
    let w = app.scenario_window.as_ref().expect("fenêtre de découpage");
    assert_eq!(w.mode(), crate::scenarioui::Mode::Plans);
    assert!(w.ready());
    for _ in 0..3 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
    }
    app.scenario_window = None;

    // Un fichier introuvable : la fenêtre montre l'erreur.
    app.read_scenario(dir.join("absent.fountain"));
    wait_for_reading(&ctx, &mut app);
    let w = app.scenario_window.as_ref().unwrap();
    assert!(w.failure().unwrap().starts_with("Impossible de lire"));
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.scenario_window_ui(ctx));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_command_exists_in_the_palette_and_the_file_menu_action_list() {
    let info = crate::keymap::action_info("fichier.importer_scenario").expect("action déclarée");
    assert!(info.label.starts_with("Importer un scénario"));
    assert!(info.implemented);
    assert_eq!(scenarioplans::Split::ALL.len(), 3);
    assert_eq!(crate::scenarioui::duration_text(0.0), "0 s");
    assert_eq!(crate::scenarioui::duration_text(125.0), "2 min 05 s");
    assert_eq!(crate::scenarioui::duration_text(3725.0), "1 h 02 min");
}
