//! Tests des espaces de travail (phase 4b).

use crate::settings::Settings;
use crate::workspace::{self, Layout, Workspace};

#[test]
fn builtin_layouts() {
    let all = workspace::builtin();
    let names: Vec<&str> = all.iter().map(|w| w.1).collect();
    assert_eq!(names, vec!["Standard", "Dessin", "Timeline", "Vue d'ensemble"]);
    assert_eq!(all[0].2, Layout::default());
    let dessin = &all[1].2;
    assert!(dessin.tools && !dessin.sheet, "Dessin : plus de place pour dessiner");
    assert!(all[2].2.montage_height > Layout::default().montage_height, "Timeline : bande plus haute");
    assert!(all[3].2.vignettes && !all[3].2.tools, "Vue d'ensemble : vignettes");
    // Chaque disposition a son action dans le fichier de raccourcis.
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for (action, _, _) in &all {
        assert!(map.warnings.iter().all(|w| !w.contains(action)), "{action}");
    }
}

#[test]
fn saving_a_workspace_replaces_one_with_the_same_name() {
    let mut list: Vec<Workspace> = Vec::new();
    let small = Layout { sheet_width: 200.0, ..Layout::default() };
    workspace::save_workspace(&mut list, " Mon dessin ", small.clone());
    workspace::save_workspace(&mut list, "", Layout::default());
    assert_eq!(list.len(), 1, "nom vide : rien");
    assert_eq!(list[0].name, "Mon dessin");
    let wide = Layout { sheet_width: 400.0, ..Layout::default() };
    workspace::save_workspace(&mut list, "MON DESSIN", wide.clone());
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].layout, wide);
}

#[test]
fn layouts_are_kept_in_the_settings() {
    let mut s = Settings::default();
    s.layout = Some(Layout { strip: false, timeline_height: 333.0, ..Layout::default() });
    workspace::save_workspace(&mut s.workspaces, "Montage", Layout { tools: false, ..Layout::default() });
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.layout, s.layout);
    assert_eq!(back.workspaces, s.workspaces);
    // Disposition incomplète ou absurde dans le fichier : valeurs par défaut, tailles bornées.
    let partial: Layout = toml::from_str("bande = false\nlargeur_fiche = 5.0").unwrap();
    assert!(!partial.strip && partial.tools);
    assert_eq!(partial.sanitized().sheet_width, 160.0);
}
