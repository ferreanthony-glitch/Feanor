//! Tests de la fenêtre « À propos » : la version vient de Cargo.toml, les noms d'avant n'y sont plus, la
//! commande est dans la liste des actions (donc dans Ctrl+K), les polices de marque sont créditées, et ffmpeg
//! avec sa licence (LGPL, partie 124).

use crate::aproposui::{ffmpeg_explication, ffmpeg_text, polices_text, version_text, CONFIGURATION_FFMPEG, DOSSIER_LICENCE_FFMPEG, LICENCE_FFMPEG, SOURCES_FFMPEG};

#[test]
fn version_and_fonts_are_written() {
    assert_eq!(version_text(), format!("Version {}", env!("CARGO_PKG_VERSION")));
    assert!(version_text().chars().any(|c| c.is_ascii_digit()));
    assert!(polices_text().contains("Big Shoulders Display") && polices_text().contains("IBM Plex") && polices_text().contains("OFL"));
}

#[test]
fn ffmpeg_is_credited_with_its_licence_its_configuration_and_its_sources() {
    // La mention courte, puis le dépliant : le texte de la LGPL v2.1 tel quel, la configuration exacte (sans GPL ni nonfree), les sources.
    assert!(ffmpeg_text().contains("FFmpeg") && ffmpeg_text().contains("LGPL") && ffmpeg_text().contains("sans composant GPL"));
    assert!(LICENCE_FFMPEG.contains("GNU LESSER GENERAL PUBLIC LICENSE") && LICENCE_FFMPEG.contains("Version 2.1, February 1999"));
    assert!(LICENCE_FFMPEG.len() > 20_000, "le texte complet de la licence");
    assert!(CONFIGURATION_FFMPEG.contains("--toolchain=msvc") && CONFIGURATION_FFMPEG.contains("--disable-everything"));
    assert!(!CONFIGURATION_FFMPEG.contains("--enable-gpl") && !CONFIGURATION_FFMPEG.contains("nonfree") && !CONFIGURATION_FFMPEG.contains("libopenh264"), "{CONFIGURATION_FFMPEG}");
    assert!(ffmpeg_explication().contains("programme à part") && ffmpeg_explication().contains("8.1.3") && ffmpeg_explication().contains(DOSSIER_LICENCE_FFMPEG));
    assert!(SOURCES_FFMPEG.starts_with("https://ffmpeg.org/") && SOURCES_FFMPEG.contains("8.1.3"));
    assert_eq!(DOSSIER_LICENCE_FFMPEG, "ffmpeg-licence", "le dossier de la version portable");
}

#[test]
fn the_command_is_listed_for_the_palette() {
    let a = crate::keymap::action_info("aide.a_propos").expect("action déclarée");
    assert!(a.implemented);
    assert!(a.label.contains("FEANOR"));
}