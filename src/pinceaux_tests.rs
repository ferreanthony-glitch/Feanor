//! Tests de la liste des pinceaux (ancien panneau Pinceaux, 2026-10-18) : le rangement des réglages en dossiers, la recherche,
//! l'état retenu d'une séance à l'autre, la migration des réglages de l'application, les
//! catégories qu'on allume et qu'on éteint.

use crate::brush::{BrushSettings, PencilGrain, Source};
use crate::brushtex::{BrushLibrary, Preset, Target, MINE};
use crate::pinceaux::*;
use crate::settings::Settings;

fn preset(name: &str, target: Target, source: &str) -> Preset {
    Preset { name: name.into(), target, settings: BrushSettings::brush(), source: source.into() }
}

fn noms(lib: &BrushLibrary, d: &Dossier) -> Vec<String> {
    d.rangs.iter().map(|&i| lib.presets[i].name.clone()).collect()
}

/// Les réglages fournis tombent dans le dossier de leur famille, d'après leur nom.
#[test]
fn builtin_brushes_go_into_their_family_folder() {
    let lib = BrushLibrary::builtin();
    let dossiers = ranger(&lib.presets, Target::Brush, "");
    let get = |nom: &str| dossiers.iter().find(|d| d.nom == nom).map(|d| noms(&lib, d)).unwrap_or_default();
    assert_eq!(get(CRAYONS), ["Crayon graphite", "Crayon de couleur", "Crayon net"]);
    assert_eq!(get(ENCRES), ["Encre", "Feutre"]);
    assert_eq!(get(PEINTURE), ["Pinceau rond", "Gouache"]);
    assert_eq!(get(CRAIE), ["Fusain", "Craie"]);
    // Pas de gomme parmi les pinceaux, et « Mes brosses » se montre vide.
    assert!(dossiers.iter().all(|d| d.nom != GOMMES));
    assert_eq!(get(MINE), Vec::<String>::new());
    assert!(dossiers.iter().any(|d| d.nom == MINE));
    // Les gommes ont leur dossier.
    let gommes = ranger(&lib.presets, Target::Eraser, "");
    assert_eq!(gommes.iter().map(|d| d.nom.as_str()).collect::<Vec<_>>(), [MINE, GOMMES]);
    assert_eq!(noms(&lib, &gommes[1]), ["Gomme", "Gomme à grain"]);
}

/// « Mes brosses » est le premier dossier de la liste, avant celles de SHOT et les dossiers importés (Anthony,
/// 2026-10-24), qu'il soit vide ou non, pour les pinceaux comme pour les gommes ; une recherche ne le montre que
/// s'il répond.
#[test]
fn my_brushes_come_first() {
    let mut lib = BrushLibrary::builtin();
    lib.presets.push(preset("Importé", Target::Brush, "pack.abr"));
    let premier = |lib: &BrushLibrary, cible, q: &str| ranger(&lib.presets, cible, q).first().map(|d| d.nom.clone());
    assert_eq!(premier(&lib, Target::Brush, ""), Some(MINE.to_string()), "vide, il est quand même en tête");
    assert_eq!(premier(&lib, Target::Eraser, ""), Some(MINE.to_string()));
    lib.save_mine("Ma craie", Target::Brush, BrushSettings::chalk());
    let d = ranger(&lib.presets, Target::Brush, "");
    assert_eq!(d[0].nom, MINE);
    assert_eq!(noms(&lib, &d[0]), ["Ma craie"]);
    assert_eq!(d.iter().map(|d| d.nom.as_str()).collect::<Vec<_>>(), [MINE, CRAYONS, ENCRES, PEINTURE, CRAIE, "pack.abr"]);
    // Il s'ouvre d'office (on y retrouve ce qu'on a gardé), et une recherche qui ne l'atteint pas le cache.
    assert!(Panneau::default().ouvert(MINE));
    assert_eq!(premier(&lib, Target::Brush, "fusain"), Some(CRAIE.to_string()));
    assert_eq!(premier(&lib, Target::Brush, "ma craie"), Some(MINE.to_string()));
}

/// Chaque réglage est dans un seul dossier : la somme des dossiers est la liste de l'outil.
#[test]
fn every_preset_is_in_exactly_one_folder() {
    let mut lib = BrushLibrary::builtin();
    lib.presets.push(preset("Importé 1", Target::Brush, "raccourcis outils.tvpx"));
    lib.presets.push(preset("Importé 2", Target::Brush, "raccourcis outils.tvpx"));
    lib.presets.push(preset("Autre", Target::Brush, "pack.abr"));
    lib.save_mine("Ma brosse", Target::Brush, BrushSettings::pencil());
    let dossiers = ranger(&lib.presets, Target::Brush, "");
    let mut tous: Vec<usize> = dossiers.iter().flat_map(|d| d.rangs.iter().copied()).collect();
    tous.sort_unstable();
    let voulus: Vec<usize> = lib.presets.iter().enumerate().filter(|(_, p)| p.target == Target::Brush).map(|(i, _)| i).collect();
    assert_eq!(tous, voulus);
    // Mes brosses d'abord, puis celles de SHOT, puis un dossier par fichier, dans l'ordre où ils arrivent.
    let ordre: Vec<&str> = dossiers.iter().map(|d| d.nom.as_str()).collect();
    assert_eq!(ordre, [MINE, CRAYONS, ENCRES, PEINTURE, CRAIE, "raccourcis outils.tvpx", "pack.abr"]);
    let importe = dossiers.iter().find(|d| d.nom == "raccourcis outils.tvpx").unwrap();
    assert!(importe.importe);
    assert_eq!(importe.rangs.len(), 2);
    assert!(!dossiers.iter().find(|d| d.nom == MINE).unwrap().importe);
}

/// La recherche ignore majuscules et accents, ne garde que les dossiers qui répondent, et ne
/// montre plus « Mes brosses » vide.
#[test]
fn search_by_name_ignores_case_and_accents() {
    let mut lib = BrushLibrary::builtin();
    lib.presets.push(preset("Éponge sèche", Target::Brush, "pack.abr"));
    let d = ranger(&lib.presets, Target::Brush, "  EPONGE ");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].nom, "pack.abr");
    assert_eq!(noms(&lib, &d[0]), ["Éponge sèche"]);
    // « cray » : les trois crayons, seuls.
    let d = ranger(&lib.presets, Target::Brush, "cray");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].nom, CRAYONS);
    assert_eq!(d[0].rangs.len(), 3);
    assert!(ranger(&lib.presets, Target::Brush, "zzz").is_empty());
    // Sans recherche, tout revient.
    assert!(ranger(&lib.presets, Target::Brush, "").len() >= 5);
}

#[test]
fn mine_and_imported_can_be_deleted_but_not_pelure() {
    assert!(!supprimable(&preset("Craie", Target::Brush, PELURE)));
    assert!(supprimable(&preset("Ma craie", Target::Brush, MINE)));
    assert!(supprimable(&preset("Craie", Target::Brush, "pack.abr")));
    let lib = BrushLibrary::builtin();
    let courant = lib.presets[2].settings;
    assert_eq!(choisi(&lib.presets, Target::Brush, &courant), Some(2));
    let autre = BrushSettings { size: 123.0, ..courant };
    assert_eq!(choisi(&lib.presets, Target::Brush, &autre), None);
    // Une gomme n'est pas choisie pour un pinceau.
    assert_eq!(choisi(&lib.presets, Target::Brush, &lib.presets[9].settings), None);
}

#[test]
fn folder_state_defaults_and_remembers_only_changes() {
    let mut p = Panneau::default();
    assert!(p.liste);
    assert_eq!(p.onglet, Onglet::Pinceaux);
    assert!(p.ouvert(CRAYONS) && p.ouvert(MINE));
    assert!(!p.ouvert("raccourcis outils.tvpx"), "un dossier importé démarre replié");
    p.regler("raccourcis outils.tvpx", true);
    p.regler(CRAYONS, false);
    assert!(p.ouvert("raccourcis outils.tvpx") && !p.ouvert(CRAYONS));
    assert_eq!(p.dossiers.len(), 2);
    // Revenir à l'état par défaut n'en garde rien.
    p.regler(CRAYONS, true);
    assert_eq!(p.dossiers.len(), 1);
}

/// L'état s'écrit dans les réglages de l'application et se relit ; des réglages d'avant le
/// panneau (sans la clé) ou une clé qu'on ne connaît plus s'ouvrent avec les valeurs par défaut.
#[test]
fn panel_state_round_trips_and_old_settings_migrate() {
    let mut s = Settings::default();
    s.brushes_panel.liste = false;
    s.brushes_panel.liste_ouverte = true;
    s.brushes_panel.onglet = Onglet::Reglages;
    s.brushes_panel.categorie = Categorie::Grain;
    s.brushes_panel.regler("raccourcis outils.tvpx", true);
    s.brushes_panel.regler(PEINTURE, false);
    let text = toml::to_string(&s).expect("écriture");
    let back: Settings = toml::from_str(&text).expect("relecture");
    assert_eq!(back.brushes_panel, s.brushes_panel);

    // Un fichier d'avant le panneau : tout par défaut, le reste intact.
    let old: Settings = toml::from_str("copie_secours_minutes = 7\n").expect("ancien fichier");
    assert_eq!(old.autosave_minutes, 7);
    assert_eq!(old.brushes_panel, Panneau::default());
    assert!(!old.brushes_panel.liste_ouverte, "la liste des pinceaux est repliée d'office");
    // Un fichier partiel, ou avec des valeurs d'une version à venir, ne perd pas les réglages.
    let part: Settings = toml::from_str("copie_secours_minutes = 3\n[panneau_pinceaux]\nliste = false\n").expect("partiel");
    assert!(!part.brushes_panel.liste);
    assert_eq!(part.brushes_panel.onglet, Onglet::Pinceaux);
    assert!(part.brushes_panel.dossiers.is_empty());
}

#[test]
fn category_switches_exist_only_where_they_make_sense() {
    let avec: Vec<Categorie> = Categorie::ALL.into_iter().filter(|c| c.a_une_case()).collect();
    assert_eq!(avec, [Categorie::Dispersion, Categorie::Grain, Categorie::Couleur, Categorie::Lissage]);
    // Sans case, toujours « allumée ».
    let s = BrushSettings::brush();
    for c in [Categorie::Pointe, Categorie::Taille, Categorie::Toucher] {
        assert!(c.allumee(&s));
    }
}

/// Éteindre une catégorie annule son effet ; la rallumer avec ce qu'elle était rend les mêmes
/// valeurs, et sans mémoire elle prend une valeur de départ visible.
#[test]
fn turning_a_category_off_and_on_keeps_its_values() {
    // Dispersion.
    let base = BrushSettings { scatter: 1.5, scatter_along: 0.5, size_jitter: 0.3, angle_jitter: 40.0, count: 4, ..BrushSettings::brush() };
    assert!(Categorie::Dispersion.allumee(&base));
    let mut s = base;
    Categorie::Dispersion.eteindre(&mut s);
    assert!(!Categorie::Dispersion.allumee(&s));
    assert_eq!((s.scatter, s.count), (0.0, 1));
    Categorie::Dispersion.allumer(&mut s, Some(&base));
    assert_eq!(s, base);
    let mut vide = BrushSettings::brush();
    Categorie::Dispersion.allumer(&mut vide, None);
    assert!(Categorie::Dispersion.allumee(&vide));

    // Grain : papier, force, grain du crayon.
    let base = BrushSettings::graphite();
    assert!(Categorie::Grain.allumee(&base), "le graphite a un papier et du grain");
    let mut s = base;
    Categorie::Grain.eteindre(&mut s);
    assert!(!Categorie::Grain.allumee(&s));
    assert_eq!(s.pencil_grain, PencilGrain::None);
    Categorie::Grain.allumer(&mut s, Some(&base));
    assert_eq!((s.paper, s.grain, s.pencil_grain, s.paper_scale), (base.paper, base.grain, base.pencil_grain, base.paper_scale));
    let mut vide = BrushSettings::brush();
    Categorie::Grain.allumer(&mut vide, None);
    assert!(Categorie::Grain.allumee(&vide) && vide.paper.is_some() && vide.grain > 0.0);

    // Couleur et mélange : mélange, charge, couleur A→B.
    let mut base = BrushSettings { mix: 0.6, charge: 0.5, ..BrushSettings::brush() };
    base.dynamics.color.source = Source::Pressure;
    assert!(Categorie::Couleur.allumee(&base));
    let mut s = base;
    Categorie::Couleur.eteindre(&mut s);
    assert!(!Categorie::Couleur.allumee(&s));
    assert_eq!((s.mix, s.charge, s.dynamics.color.source), (0.0, 1.0, Source::None));
    Categorie::Couleur.allumer(&mut s, Some(&base));
    assert_eq!(s, base);

    // Lissage : selon le type, le réglage qui compte.
    let base = BrushSettings { smoothing: 0.7, ..BrushSettings::brush() };
    let mut s = base;
    Categorie::Lissage.eteindre(&mut s);
    assert!(!Categorie::Lissage.allumee(&s));
    Categorie::Lissage.allumer(&mut s, Some(&base));
    assert_eq!(s.smoothing, 0.7);
    // Une mémoire où la catégorie était déjà éteinte ne ramène pas du vide : valeur de départ.
    let mut s = BrushSettings { smoothing: 0.0, smooth_px: 0.0, smooth_points: 1, ..BrushSettings::brush() };
    let avant = s;
    Categorie::Lissage.allumer(&mut s, Some(&avant));
    assert!(Categorie::Lissage.allumee(&s));
}

/// Éteindre une catégorie ne touche pas à ce qui est à une autre.
#[test]
fn a_category_only_changes_its_own_settings() {
    let base = BrushSettings::chalk();
    for c in [Categorie::Dispersion, Categorie::Grain, Categorie::Couleur, Categorie::Lissage] {
        let mut s = base;
        c.eteindre(&mut s);
        assert_eq!((s.size, s.opacity, s.hardness, s.tip, s.roundness, s.spacing), (base.size, base.opacity, base.hardness, base.tip, base.roundness, base.spacing), "{c:?}");
    }
}

#[test]
fn plie_removes_case_and_accents() {
    assert_eq!(plie("Éponge ÀÇÊ Gomme à grain"), "eponge ace gomme a grain");
}
