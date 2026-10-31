//! Passe « sans marques commerciales » et « Papier » (2026-10-30, §90 de docs/INTERFACE.md) : aucun nom de logiciel du
//! commerce dans ce que l'utilisateur lit (commandes de Ctrl+K, bulles, messages, filtres des fenêtres de fichiers,
//! « Origine » des raccourcis). Les noms de FORMATS de fichiers (.abr, .kpp, .fdx, .docx, FCPXML, Kitsu…) restent.

/// Les noms à ne plus montrer. Comparaison sensible à la casse, mot entier pour les noms courts.
const MARQUES: &[&str] = &[
    "TVPaint", "Photoshop", "Procreate", "Krita", "MyPaint", "Final Draft", "Final Cut", "LibreOffice", "Premiere", "Avid", "DaVinci",
    "After Effects", "Miro", "PureRef", "EbSynth", "Toon Boom", "Storyboard Pro", "Dropbox", "Excel", "Word", "Google", "Illustrator",
    "Clip Studio", "Figma", "Celtx", "GIMP",
];

/// Des chaînes qui contiennent un nom de marque pour une raison technique, jamais montrées : variable d'environnement,
/// dossier d'installation, signature de format.
const TECHNIQUES: &[&str] = &["PELURE_TVPAINT", "TVPaint Developpement", "Krita (x64)", "GIMP Palette", "{feff}GIMP"];

fn contient_mot(texte: &str, mot: &str) -> bool {
    let mut debut = 0;
    while let Some(p) = texte[debut..].find(mot) {
        let a = debut + p;
        let b = a + mot.len();
        let avant = texte[..a].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let apres = texte[b..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if avant && apres {
            return true;
        }
        debut = b;
    }
    false
}

fn marque_dans(texte: &str) -> Option<&'static str> {
    let mut propre = texte.to_string();
    for t in TECHNIQUES {
        propre = propre.replace(t, "");
    }
    MARQUES.iter().copied().find(|m| contient_mot(&propre, m))
}

/// Les littéraux « ... » d'une source Rust, hors commentaires de ligne (`//`, `///`, `//!`), sur une ou plusieurs lignes.
fn litteraux(source: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut ligne = 1;
    let mut courant: Option<(usize, String)> = None;
    for l in source.lines() {
        if courant.is_none() && l.trim_start().starts_with("//") {
            ligne += 1;
            continue;
        }
        let mut chars = l.chars().peekable();
        while let Some(c) = chars.next() {
            match (&mut courant, c) {
                (None, '"') => courant = Some((ligne, String::new())),
                (None, '\'') => {
                    // un caractère entre apostrophes ('"' par exemple) n'ouvre pas de chaîne ; une apostrophe de lifetime non plus
                    let mut copie = chars.clone();
                    let (x, y) = (copie.next(), copie.next());
                    if x == Some('"') && y == Some('\'') {
                        chars.next();
                        chars.next();
                    }
                }
                (None, '/') if chars.peek() == Some(&'/') => break,
                (Some(_), '\\') => {
                    chars.next();
                }
                (Some(_), '"') => out.push(courant.take().unwrap()),
                (Some((_, s)), c) => s.push(c),
                _ => {}
            }
        }
        if let Some((_, s)) = &mut courant {
            s.push(' ');
        }
        ligne += 1;
    }
    out
}

#[test]
fn the_string_extractor_skips_comments_and_reads_multiline_strings() {
    let src = "// « Krita » en commentaire\nlet a = \"un \\\" texte\"; // Photoshop\nlet b = \"deux\n lignes Excel\";\nlet c = '\"'; let d = \"ok\";";
    let l: Vec<String> = litteraux(src).into_iter().map(|(_, s)| s).collect();
    assert_eq!(l.len(), 3, "{l:?}");
    assert!(l[1].contains("Excel") && l[2] == "ok");
    assert!(marque_dans("Brosses (.abr)").is_none());
    assert_eq!(marque_dans("brosse Photoshop"), Some("Photoshop"));
    assert!(marque_dans("un mot : wordpress").is_none(), "mot entier seulement");
    assert!(marque_dans("GIMP Palette").is_none(), "signature de format");
}

#[test]
fn no_trademark_in_the_command_names_of_ctrl_k() {
    for (id, nom) in crate::palette::palette_commands() {
        assert!(marque_dans(nom).is_none(), "{id} : « {nom} » montre une marque ({:?})", marque_dans(nom));
    }
    // L'étiquette complète de toutes les actions, doubles comprises.
    for a in crate::keymap::ACTIONS {
        assert!(marque_dans(a.label).is_none(), "{} : « {} » montre une marque", a.id, a.label);
    }
}

#[test]
fn no_trademark_in_the_origin_shown_with_the_shortcuts() {
    // « Origine : … » est montré en bulle dans la fenêtre des raccourcis : le champ `source` du fichier.
    for (n, l) in crate::keymap::DEFAULT_KEYMAP.lines().enumerate() {
        let l = l.trim_start();
        if let Some(rest) = l.strip_prefix("source") {
            assert!(marque_dans(rest).is_none(), "keymap.toml ligne {} : {l}", n + 1);
        }
    }
}

#[test]
fn no_trademark_in_the_texts_of_the_ui_and_import_export_sources() {
    let fichiers: &[(&str, &str)] = &[
        ("abr.rs", include_str!("abr.rs")),
        ("barres.rs", include_str!("barres.rs")),
        ("brushui.rs", include_str!("brushui.rs")),
        ("cooperationui.rs", include_str!("cooperationui.rs")),
        ("dynui.rs", include_str!("dynui.rs")),
        ("editorial.rs", include_str!("editorial.rs")),
        ("editorialui.rs", include_str!("editorialui.rs")),
        ("exportui.rs", include_str!("exportui.rs")),
        ("fichejob.rs", include_str!("fichejob.rs")),
        ("ficheui.rs", include_str!("ficheui.rs")),
        ("keymap.rs", include_str!("keymap.rs")),
        ("kritabrush.rs", include_str!("kritabrush.rs")),
        ("nuancier.rs", include_str!("nuancier.rs")),
        ("nuancierui.rs", include_str!("nuancierui.rs")),
        ("palette.rs", include_str!("palette.rs")),
        ("panels.rs", include_str!("panels.rs")),
        ("pinceauxui.rs", include_str!("pinceauxui.rs")),
        ("prefsui.rs", include_str!("prefsui.rs")),
        ("procreate.rs", include_str!("procreate.rs")),
        ("propagerui.rs", include_str!("propagerui.rs")),
        ("scenario.rs", include_str!("scenario.rs")),
        ("scenariofdx.rs", include_str!("scenariofdx.rs")),
        ("scenariopanneau.rs", include_str!("scenariopanneau.rs")),
        ("tvpbrush.rs", include_str!("tvpbrush.rs")),
        ("bottomtabs.rs", include_str!("bottomtabs.rs")),
        ("nouveauui.rs", include_str!("nouveauui.rs")),
        ("listeplansui.rs", include_str!("listeplansui.rs")),
        ("paquetui.rs", include_str!("paquetui.rs")),
        ("nommageui.rs", include_str!("nommageui.rs")),
        ("modeleepisodeui.rs", include_str!("modeleepisodeui.rs")),
        ("fildecisionui.rs", include_str!("fildecisionui.rs")),
        ("premierjour.rs", include_str!("premierjour.rs")),
    ];
    let mut fautes = Vec::new();
    for (nom, source) in fichiers {
        for (ligne, texte) in litteraux(source) {
            // `palette.rs` garde les anciens noms (synonymes de recherche, jamais affichés) ; `export.rs` cherche ffmpeg dans
            // le dossier de Krita sans le dire.
            if *nom == "palette.rs" || texte.contains("word/document.xml") {
                continue;
            }
            if let Some(m) = marque_dans(&texte) {
                fautes.push(format!("{nom}:{ligne} ({m}) : {}", texte.chars().take(90).collect::<String>()));
            }
        }
    }
    assert!(fautes.is_empty(), "des marques dans les textes :\n{}", fautes.join("\n"));
}

#[test]
fn the_old_trademark_command_words_still_find_their_commands() {
    let commandes = crate::palette::palette_commands();
    let trouve = |q: &str, id: &str| {
        let r = crate::palette::rank(&commandes, q);
        assert!(r.iter().any(|&i| commandes[i].0 == id), "« {q} » ne trouve pas {id}");
    };
    trouve("EbSynth", "calque.propager");
}

// ------------------------------------------------------------------------------------------------------ Papier

#[test]
fn the_sheet_background_is_called_papier_everywhere_and_the_old_names_stay_synonyms() {
    for id in ["onglet.arriere_plan", "fond.aucun", "fond.couleur", "fond.damier"] {
        let a = crate::keymap::action_info(id).unwrap();
        assert!(a.label.contains("Papier"), "{id} : {}", a.label);
        assert!(!a.label.contains("Fond de la feuille") && !a.label.contains("Arrière-plan"), "{id} : {}", a.label);
    }
    // « Couleur d'arrière-plan » (le couple de couleurs) ne change pas.
    assert!(crate::keymap::action_info("pastille.arriere_plan").unwrap().label.to_lowercase().contains("arrière-plan"));
    let commandes = crate::palette::palette_commands();
    for (q, id) in [
        ("Fond de la feuille", "onglet.arriere_plan"),
        ("Fond de la feuille damier", "fond.damier"),
        ("Fond de la feuille couleur", "fond.couleur"),
        ("Arrière-plan aucun", "fond.aucun"),
        ("papier damier", "fond.damier"),
    ] {
        let r = crate::palette::rank(&commandes, q);
        assert!(r.iter().any(|&i| commandes[i].0 == id), "« {q} » ne trouve pas {id}");
    }
    // Les textes de la timeline et de Nouveau projet.
    for (nom, source) in [("bottomtabs.rs", include_str!("bottomtabs.rs")), ("nouveauui.rs", include_str!("nouveauui.rs"))] {
        for (ligne, texte) in litteraux(source) {
            assert!(!texte.contains("ond de la feuille"), "{nom}:{ligne} : {texte}");
        }
    }
}
