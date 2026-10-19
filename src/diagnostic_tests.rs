//! Tests du diagnostic des performances (`diagnostic.rs`, `diagbanc.rs`, `diagnosticui.rs`,
//! `materiel.rs`) : le profil choisi d'après des mesures inventées, chaque réglage branché sur ce
//! qu'il commande, appliquer puis revenir, les anciens fichiers de réglages, la proposition du
//! premier lancement. Le banc réel (15 secondes) s'affiche avec :
//! `cargo test --release --offline --target-dir target\msvc diag_bench -- --ignored --nocapture`.

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use crate::diagbanc::Banc;
use crate::diagnostic::{self as d, EtatDiagnostic, Interrupteur, Mesures, PerfReglages, Profil, TraitsTextures, Vignettes};
use crate::materiel::{Carte, Ecran, GenreCarte, Materiel, Memoire, Processeur};
use crate::perf_tests::banc;
use crate::settings::Settings;

const GO: u64 = 1 << 30;

fn machine(ram_go: u64, fils: usize, genre: GenreCarte, vram_go: Option<u64>) -> Materiel {
    Materiel {
        processeur: Processeur { nom: "Processeur d'essai".into(), logiques: fils, physiques: Some(fils / 2), avx2: true, fma: true, bmi2: true },
        memoire: Some(Memoire { totale: ram_go * GO, disponible: ram_go * GO / 2 }),
        carte: Some(Carte { nom: "Carte d'essai".into(), api: "Dx12".into(), pilote: "1.0".into(), genre, vram: vram_go.map(|g| g * GO) }),
        ecran: Some(Ecran { fenetre: [1920.0, 1080.0], moniteur: Some([2560.0, 1440.0]), ppp: 1.0 }),
        disque: None,
    }
}

fn forte() -> Materiel {
    machine(32, 16, GenreCarte::Dediee, Some(8))
}

/// Les mesures du poste de référence, multipliées par `k` (2 : deux fois plus lent).
fn mesures(k: f64) -> Mesures {
    Mesures {
        composition_ms: Some(d::REF_COMPOSITION_MS * k),
        trait_ms: Some(d::REF_TRAIT_MS * k),
        flou_plein_ms: Some(d::REF_FLOU_MS * k),
        flou_apercu_ms: Some(d::REF_FLOU_MS * k / 3.0),
        lecture_ms: Some(d::REF_LECTURE_MS * k),
        lecture_pire_ms: Some(d::REF_LECTURE_MS * k * 1.5),
        envoi_carte_mo_s: Some(d::REF_ENVOI_MO_S / k),
        duree_s: 15.0,
    }
}

// ---------------------------------------------------------------------- le profil

#[test]
fn a_strong_machine_gets_quality_and_is_told_why() {
    let c = d::choisir_profil(&forte(), &mesures(1.0));
    assert_eq!(c.profil, Profil::Qualite);
    let raisons = c.raisons().join(" ");
    assert!(raisons.contains("marge"), "{raisons}");
    // Les caches grandissent avec la mémoire : 32 Go de mémoire vive et 8 Go de mémoire vidéo.
    let q = d::reglages_du_profil(Profil::Qualite, &forte());
    assert_eq!(q.cache_carte_mo, 2048);
    assert_eq!((q.cache_traits_mo, q.cache_effets_mo), (1024, 1536));
    assert!(!q.effets_reduits && !q.tremblement_coupe && !q.animations_coupees);
    assert_eq!(q.traits_textures, TraitsTextures::Exact);
}

#[test]
fn modest_hardware_gets_balanced_and_weak_hardware_gets_performance() {
    // Mémoire juste (12 Go) : équilibré.
    let c = d::choisir_profil(&machine(12, 12, GenreCarte::Dediee, Some(6)), &mesures(1.0));
    assert_eq!(c.profil, Profil::Equilibre);
    assert!(c.raisons().join(" ").contains("12"), "{:?}", c.raisons());
    // 8 fils : bien. 6 fils : équilibré.
    assert_eq!(d::choisir_profil(&machine(32, 8, GenreCarte::Dediee, Some(8)), &mesures(1.0)).profil, Profil::Qualite);
    assert_eq!(d::choisir_profil(&machine(32, 6, GenreCarte::Dediee, Some(8)), &mesures(1.0)).profil, Profil::Equilibre);
    // Graphisme intégré : équilibré ; mémoire vidéo de 1 Go : performance.
    assert_eq!(d::choisir_profil(&machine(32, 16, GenreCarte::Integree, None), &mesures(1.0)).profil, Profil::Equilibre);
    assert_eq!(d::choisir_profil(&machine(32, 16, GenreCarte::Dediee, Some(1)), &mesures(1.0)).profil, Profil::Performance);
    // Peu de mémoire, peu de fils, rendu logiciel : performance.
    assert_eq!(d::choisir_profil(&machine(6, 16, GenreCarte::Dediee, Some(8)), &mesures(1.0)).profil, Profil::Performance);
    assert_eq!(d::choisir_profil(&machine(32, 2, GenreCarte::Dediee, Some(8)), &mesures(1.0)).profil, Profil::Performance);
    assert_eq!(d::choisir_profil(&machine(32, 16, GenreCarte::Logicielle, None), &mesures(1.0)).profil, Profil::Performance);
    // Pas de carte du tout (OpenGL de secours).
    let mut sans = forte();
    sans.carte = None;
    assert_eq!(d::choisir_profil(&sans, &mesures(1.0)).profil, Profil::Performance);
}

#[test]
fn slow_measurements_lower_the_profile_whatever_the_hardware_says() {
    let m = forte();
    // Un peu plus lent que la référence : rien ne pèse.
    assert_eq!(d::choisir_profil(&m, &mesures(1.8)).profil, Profil::Qualite);
    // Plus de 2,2 fois plus lent : équilibré. Plus de 4,5 fois : performance.
    assert_eq!(d::choisir_profil(&m, &mesures(2.5)).profil, Profil::Equilibre);
    assert_eq!(d::choisir_profil(&m, &mesures(5.0)).profil, Profil::Performance);
    // Une seule mesure lente suffit, et la phrase la cite avec ses chiffres.
    let mut x = mesures(1.0);
    x.trait_ms = Some(d::REF_TRAIT_MS * 6.0);
    let c = d::choisir_profil(&m, &x);
    assert_eq!(c.profil, Profil::Performance);
    let raisons = c.raisons().join(" ");
    assert!(raisons.contains("trait texturé") && raisons.contains("plus lent"), "{raisons}");
    // L'envoi à la carte est un débit : plus il est bas, plus c'est lent.
    let mut x = mesures(1.0);
    x.envoi_carte_mo_s = Some(d::REF_ENVOI_MO_S / 6.0);
    assert_eq!(d::choisir_profil(&m, &x).profil, Profil::Performance);
    // Sans aucune mesure (banc interrompu), le matériel décide seul.
    assert_eq!(d::choisir_profil(&m, &Mesures::default()).profil, Profil::Qualite);
    assert_eq!(d::choisir_profil(&machine(6, 16, GenreCarte::Dediee, Some(8)), &Mesures::default()).profil, Profil::Performance);
}

#[test]
fn a_slow_disk_is_reported_but_changes_no_profile() {
    let mut m = forte();
    m.disque = Some(crate::materiel::Disque { ecriture: 30.0, lecture: 80.0, taille_mo: 64 });
    let c = d::choisir_profil(&m, &mesures(1.0));
    assert_eq!(c.profil, Profil::Qualite);
    assert!(c.constats.iter().any(|k| k.poids == 0 && k.texte.contains("Disque lent")));
}

#[test]
fn each_profile_sets_what_it_promises() {
    let faible = machine(6, 4, GenreCarte::Integree, Some(1));
    let e = d::reglages_du_profil(Profil::Equilibre, &forte());
    assert!(e.effets_reduits);
    assert_eq!(e.traits_textures, TraitsTextures::Repos);
    assert!(!e.tremblement_coupe && !e.animations_coupees);
    assert_eq!(e.pelure_max, 6);
    assert_eq!(e.vignettes, Vignettes::Normales);
    // Performance : tout ce qui est gourmand s'éteint ou se réduit.
    let p = d::reglages_du_profil(Profil::Performance, &faible);
    assert!(p.effets_reduits && p.tremblement_coupe && p.animations_coupees);
    assert_eq!(p.traits_textures, TraitsTextures::Rapide);
    assert_eq!(p.pelure_max, 2);
    assert_eq!(p.vignettes, Vignettes::Minimales);
    assert_eq!((p.cache_traits_mo, p.cache_effets_mo), (256, 256));
    assert!(p.cache_carte_mo <= 512);
    // Sur 4 fils, un reste à l'interface ; sur 16, tous restent permis.
    assert_eq!(p.fils, 3);
    assert_eq!(d::reglages_du_profil(Profil::Performance, &forte()).fils, 0);
    assert_eq!(d::reglages_du_profil(Profil::Performance, &forte()).vignettes, Vignettes::Reduites);
    // Les profils sont bornés comme les réglages à la main.
    for profil in Profil::TOUS {
        for m in [&forte(), &faible] {
            let r = d::reglages_du_profil(profil, m);
            assert_eq!(r, r.sanitized(), "{profil:?}");
        }
    }
    // Qualité sur une machine qui n'a rien à agrandir : les réglages d'avant.
    assert_eq!(d::reglages_du_profil(Profil::Qualite, &faible), PerfReglages::default());
}

// ---------------------------------------------------------------------- les réglages

#[test]
fn each_switch_maps_to_exactly_one_setting_and_changes_what_the_program_reads() {
    let loin = d::reglages_du_profil(Profil::Performance, &machine(6, 4, GenreCarte::Integree, Some(1)));
    let origine = PerfReglages::default();
    assert_eq!(Interrupteur::TOUS.len(), 10);
    for i in Interrupteur::TOUS {
        let mut x = origine;
        i.copier(&mut x, &loin);
        assert!(!i.egal(&x, &origine), "{i:?} : le réglage n'a pas bougé");
        // Aucun autre réglage n'a bougé.
        for j in Interrupteur::TOUS.into_iter().filter(|j| *j != i) {
            assert!(j.egal(&x, &origine), "{i:?} a aussi changé {j:?}");
        }
        // Et le programme lit autre chose.
        assert_ne!(x.cibles(), origine.cibles(), "{i:?} ne change rien de ce que le programme lit");
        assert_ne!(i.lecture(&x), i.lecture(&origine), "{i:?}");
        assert_ne!(i.texte(&x), i.texte(&origine), "{i:?}");
        assert!(!i.nom().is_empty() && !i.aide().is_empty());
    }
    // Les dix réglages sont toute la structure : un seul tour de table les remet tous.
    let mut tout = origine;
    for i in Interrupteur::TOUS {
        i.copier(&mut tout, &loin);
    }
    assert_eq!(tout, loin);
    assert_eq!(d::differences(&origine, &loin).len(), 10);
    assert!(d::differences(&loin, &loin).is_empty());
    // Un seul réglage ne demande pas d'ouvrir un projet de nouveau : les traits texturés.
    assert_eq!(Interrupteur::TOUS.iter().filter(|i| !i.immediat()).collect::<Vec<_>>(), vec![&Interrupteur::TraitsTextures]);
}

#[test]
fn the_settings_become_the_values_the_program_reads() {
    let c = PerfReglages::default().cibles();
    assert_eq!((c.effets_min, c.tremblement_coupe, c.fils, c.vignettes_pct, c.animations_coupees), (0, false, 0, 100, false));
    assert_eq!(c.pelure_max, crate::onion::MAX_NEIGHBOURS);
    assert_eq!((c.cache_carte, c.cache_traits, c.cache_effets), (1 << 30, 512 << 20, 768 << 20));
    let p = PerfReglages {
        effets_reduits: true,
        tremblement_coupe: true,
        pelure_max: 2,
        cache_carte_mo: 512,
        cache_traits_mo: 256,
        cache_effets_mo: 384,
        fils: 3,
        vignettes: Vignettes::Minimales,
        animations_coupees: true,
        traits_textures: TraitsTextures::Rapide,
    }
    .cibles();
    assert_eq!((p.effets_min, p.tremblement_coupe, p.pelure_max, p.fils, p.vignettes_pct, p.animations_coupees), (1, true, 2, 3, 25, true));
    assert_eq!((p.cache_carte, p.cache_traits, p.cache_effets), (512 << 20, 256 << 20, 384 << 20));
    // Hors bornes (fichier édité à la main) : ramené dans les bornes.
    let fou = PerfReglages { pelure_max: 99, cache_carte_mo: 1, cache_traits_mo: 999_999, cache_effets_mo: 0, fils: 9999, ..Default::default() }.sanitized();
    assert_eq!((fou.pelure_max, fou.cache_carte_mo, fou.cache_traits_mo, fou.cache_effets_mo, fou.fils), (12, 128, 16384, 64, 256));
}

/// Chaque réglage commande bien un comportement du programme (les fonctions que le programme
/// appelle, avec la valeur du réglage).
#[test]
fn each_setting_changes_the_behaviour_it_commands() {
    // Effets en aperçu réduit : même zoomé à 100 %, au moins la moitié.
    assert_eq!(crate::fxproxy::level_with_floor(1.0, 0), 0);
    assert_eq!(crate::fxproxy::level_with_floor(1.0, 1), 1);
    assert_eq!(crate::fxproxy::level_with_floor(0.2, 1), 2, "un zoom éloigné réduit davantage");
    // Tremblement : « Animer à l'arrêt » joue ou non.
    let mut valeurs = vec![0.0; crate::tremblement::VARIANT + 1];
    valeurs[crate::tremblement::INTENSITY] = 4.0;
    valeurs[crate::tremblement::REST] = 1.0;
    assert!(crate::tremblement::Settings::of_with(&valeurs, true).rest);
    assert!(!crate::tremblement::Settings::of_with(&valeurs, false).rest);
    // Pelure : au plus N images de chaque côté, sans toucher au réglage de la pelure.
    let mut calque = crate::document::Layer::new("c", crate::document::LayerKind::Bitmap);
    calque.frames = (1..=20).map(Some).collect();
    let pelure = crate::onion::OnionSettings { before: 8, after: 8, ..Default::default() };
    let (av, ap) = crate::onion::neighbours_within(&calque, 10, &pelure, &[], 12);
    assert_eq!((av.len(), ap.len()), (8, 8));
    let (av, ap) = crate::onion::neighbours_within(&calque, 10, &pelure, &[], 2);
    assert_eq!((av.len(), ap.len()), (2, 2));
    assert_eq!(av[0].1, 0, "les plus proches restent");
    assert_eq!(pelure.before, 8);
    // Fils : plafonnés, jamais plus que de cœurs, jamais zéro.
    assert_eq!(crate::parallele::coeurs_avec(16, 0), 16);
    assert_eq!(crate::parallele::coeurs_avec(16, 4), 4);
    assert_eq!(crate::parallele::coeurs_avec(2, 8), 2);
    assert_eq!(crate::parallele::coeurs_avec(0, 0), 1);
    // Vignettes : la définition baisse, la forme reste.
    let doc = crate::document::Document::with_frame(1920, 1080, 24);
    assert_eq!(crate::thumbs::thumb_size_at(&doc, 192, 100), (192, 108));
    assert_eq!(crate::thumbs::thumb_size_at(&doc, 192, 50), (96, 54));
    assert_eq!(crate::thumbs::thumb_size_at(&doc, 192, 25), (48, 27));
    assert_eq!(crate::thumbs::thumb_size_at(&doc, 64, 25).0, 32, "jamais sous 32 points");
    // Animations : le bord glisse, ou saute.
    assert!(crate::ajuste::ease_with(0.0, 100.0, false) < 100.0);
    assert_eq!(crate::ajuste::ease_with(0.0, 100.0, true), 100.0);
    // Traits texturés : le rendu exact vient tout de suite, après le repos, ou jamais.
    use crate::diagnosticui::{rendus_exacts, RendusExacts, REPOS_S};
    use d::TraitsTextures::*;
    assert_eq!(rendus_exacts(Exact, 0.0, true), RendusExacts::Maintenant);
    assert_eq!(rendus_exacts(Repos, 0.5, false), RendusExacts::Attendre(REPOS_S - 0.5));
    assert_eq!(rendus_exacts(Repos, REPOS_S + 1.0, false), RendusExacts::Maintenant);
    assert_eq!(rendus_exacts(Repos, 100.0, true), RendusExacts::Attendre(0.0), "jamais pendant la lecture");
    assert_eq!(rendus_exacts(Rapide, 100.0, false), RendusExacts::Jamais);
}

// ---------------------------------------------------------------------- appliquer, revenir

fn tout_cocher(courant: &PerfReglages, cible: &PerfReglages) -> Vec<Interrupteur> {
    d::differences(courant, cible)
}

#[test]
fn apply_then_revert_restores_exactly_what_was_there() {
    // L'utilisateur avait déjà réglé des choses à la main.
    let origine = PerfReglages { cache_traits_mo: 300, vignettes: Vignettes::Reduites, pelure_max: 4, ..Default::default() };
    let cible = d::reglages_du_profil(Profil::Performance, &machine(6, 4, GenreCarte::Integree, Some(1)));
    let mut etat = EtatDiagnostic::default();
    let mut courant = origine;
    let coches = tout_cocher(&courant, &cible);
    etat.appliquer(&mut courant, &cible, &coches, Profil::Performance, 1_760_000_000);
    assert_eq!(courant, cible);
    assert!(etat.peut_revenir());
    assert_eq!(etat.profil, Some(Profil::Performance));
    assert_eq!(etat.avant, Some(origine));
    assert!(etat.revenir(&mut courant));
    assert_eq!(courant, origine, "les valeurs d'avant, exactement");
    assert!(!etat.peut_revenir());
    assert!(!etat.revenir(&mut courant), "rien à remettre une deuxième fois");
    assert_eq!(courant, origine);
}

#[test]
fn only_the_ticked_switches_are_applied_and_reverted() {
    let origine = PerfReglages::default();
    let cible = d::reglages_du_profil(Profil::Performance, &machine(6, 4, GenreCarte::Integree, Some(1)));
    let mut etat = EtatDiagnostic::default();
    let mut courant = origine;
    let coches = [Interrupteur::TraitsTextures, Interrupteur::Animations];
    etat.appliquer(&mut courant, &cible, &coches, Profil::Performance, 5);
    assert_eq!(courant.traits_textures, TraitsTextures::Rapide);
    assert!(courant.animations_coupees);
    // Les autres n'ont pas bougé.
    for i in Interrupteur::TOUS.into_iter().filter(|i| !coches.contains(i)) {
        assert!(i.egal(&courant, &origine), "{i:?} ne devait pas bouger");
    }
    // Un réglage changé à la main depuis (hors de ceux posés) reste après le retour.
    courant.cache_carte_mo = 700;
    // Un réglage posé, rechangé à la main, revient à sa valeur d'avant.
    courant.animations_coupees = false;
    assert!(etat.revenir(&mut courant));
    assert_eq!(courant, PerfReglages { cache_carte_mo: 700, ..origine });
}

#[test]
fn applying_twice_reverts_to_the_state_before_the_last_application() {
    let m = machine(6, 4, GenreCarte::Integree, Some(1));
    let (eq, perf) = (d::reglages_du_profil(Profil::Equilibre, &m), d::reglages_du_profil(Profil::Performance, &m));
    let mut etat = EtatDiagnostic::default();
    let mut courant = PerfReglages::default();
    let coches = tout_cocher(&courant, &eq);
    etat.appliquer(&mut courant, &eq, &coches, Profil::Equilibre, 1);
    let coches = tout_cocher(&courant, &perf);
    etat.appliquer(&mut courant, &perf, &coches, Profil::Performance, 2);
    assert_eq!(courant, perf);
    etat.revenir(&mut courant);
    assert_eq!(courant, eq);
}

#[test]
fn the_diagnostic_state_survives_a_restart() {
    let mut s = Settings::default();
    let cible = d::reglages_du_profil(Profil::Performance, &machine(6, 4, GenreCarte::Integree, Some(1)));
    let mut courant = s.perf;
    let coches = tout_cocher(&courant, &cible);
    s.diagnostic.appliquer(&mut courant, &cible, &coches, Profil::Performance, 1_760_000_000);
    s.perf = courant;
    let texte = toml::to_string(&s).expect("écriture");
    let mut relu: Settings = toml::from_str(&texte).expect("relecture");
    assert_eq!(relu.perf, s.perf);
    assert_eq!(relu.diagnostic, s.diagnostic);
    // Revenir après le redémarrage remet les valeurs d'avant.
    let mut c = relu.perf;
    assert!(relu.diagnostic.revenir(&mut c));
    assert_eq!(c, PerfReglages::default());
}

// ---------------------------------------------------------------------- les anciens fichiers

#[test]
fn an_old_settings_file_without_the_new_sections_loads_with_the_old_behaviour() {
    let vieux = "copie_secours_minutes = 3\ncurseur_systeme = false\n\n[pelure]\nbefore = 2\n";
    let s: Settings = toml::from_str(vieux).expect("un ancien fichier se lit");
    assert_eq!(s.autosave_minutes, 3);
    assert_eq!(s.perf, PerfReglages::default());
    assert_eq!(s.diagnostic, EtatDiagnostic::default());
    assert!(s.diagnostic.a_proposer(), "aucune trace d'un diagnostic : à proposer");
    // Un fichier vide aussi.
    let vide: Settings = toml::from_str("").expect("fichier vide");
    assert_eq!(vide.perf, PerfReglages::default());
}

#[test]
fn a_partial_performance_section_keeps_the_defaults_for_the_rest() {
    let s: Settings = toml::from_str("[performance]\ncache_carte_mo = 2048\ntraits_textures = \"repos\"\n").expect("section partielle");
    assert_eq!(s.perf.cache_carte_mo, 2048);
    assert_eq!(s.perf.traits_textures, TraitsTextures::Repos);
    assert_eq!(PerfReglages { cache_carte_mo: 2048, traits_textures: TraitsTextures::Repos, ..Default::default() }, s.perf);
    // Une section inconnue ou un réglage d'une version future ne gênent pas.
    let futur: Settings = toml::from_str("[performance]\nreglage_futur = 3\n").expect("réglage inconnu");
    assert_eq!(futur.perf, PerfReglages::default());
    // Un nouveau fichier écrit par SHOT garde toutes les clés.
    let texte = toml::to_string(&Settings::default()).expect("écriture");
    assert!(texte.contains("[performance]") && texte.contains("effets_apercu_reduit") && texte.contains("traits_textures"), "{texte}");
}

// ---------------------------------------------------------------------- le premier lancement

#[test]
fn the_first_launch_flag_offers_the_diagnostic_once() {
    let mut e = EtatDiagnostic::default();
    assert!(e.a_proposer());
    e.propose = true;
    assert!(!e.a_proposer(), "après une réponse (même « plus tard »), plus de proposition");
    let mut f = EtatDiagnostic::default();
    f.note_fait(1_760_000_000);
    assert!(!f.a_proposer(), "un diagnostic fait, plus de proposition");
    // Un profil appliqué sans que la proposition ait été affichée (menu Aide) : pareil.
    let mut g = EtatDiagnostic::default();
    let mut c = PerfReglages::default();
    g.appliquer(&mut c, &PerfReglages::default(), &[], Profil::Qualite, 9);
    assert!(!g.a_proposer());
}

#[test]
fn the_offer_is_shown_once_never_applies_anything_and_never_in_the_tests() {
    // Les essais n'affichent jamais la proposition.
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), false);
    assert!(!b.app.diag.offre, "pas de proposition dans les essais");
    b.garder_textes = true;
    b.chauffer(2);
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.textes.iter().any(|t| t.contains("Plus tard")));
    // Sur une machine neuve : le petit message, avec ses deux boutons.
    b.app.diag.offre = true;
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let joint = b.textes.join("\n");
    assert!(joint.contains("Lancer le diagnostic") && joint.contains("Plus tard"), "{joint}");
    assert!(joint.contains("Rien n'est appliqué sans votre accord"), "{joint}");
    // Le message est fixe : visible, il ne fait pas repeindre l'application en continu.
    assert!(crate::perf_tests::se_reposer(&mut b, 3000) < 3000, "SHOT se repose, message affiché");
    let reveils: Vec<_> = (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect();
    assert!(reveils.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "la proposition fait repeindre : {reveils:?}");
    // « Plus tard » : le message disparaît, la réponse est gardée, rien n'est appliqué.
    let avant = b.app.settings.perf;
    b.app.repondre_offre(false);
    assert!(!b.app.diag.offre && !b.app.diag.ouvert);
    assert!(b.app.settings.diagnostic.propose && !b.app.settings.diagnostic.a_proposer());
    assert_eq!(b.app.settings.perf, avant);
    let relu = Settings::load();
    assert!(!relu.diagnostic.a_proposer(), "la réponse est enregistrée");
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.textes.iter().any(|t| t.contains("Plus tard")));
}

// ---------------------------------------------------------------------- la fenêtre et l'action

#[test]
fn the_action_opens_the_window_with_the_detected_machine() {
    let info = crate::keymap::action_info("aide.diagnostic").expect("action déclarée");
    assert!(info.implemented && info.label.contains("Diagnostic des performances"));
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), false);
    b.garder_textes = true;
    b.image(Vec::new(), 1.0 / 60.0);
    assert!(!b.textes.iter().any(|t| t.contains("Lancer le diagnostic")));
    b.app.run_action("aide.diagnostic");
    assert!(b.app.diag.ouvert);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    let joint = b.textes.join("\n");
    for attendu in ["Diagnostic des performances", "Votre machine", "Processeur", "Mémoire vive", "Lancer le diagnostic", "Écran"] {
        assert!(joint.contains(attendu), "« {attendu} » manque :\n{joint}");
    }
    // Rien n'a été lancé ni changé.
    assert!(b.app.diag.banc.is_none());
    assert_eq!(b.app.settings.perf, PerfReglages::default());
}

#[test]
fn the_preferences_show_every_switch() {
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), false);
    b.garder_textes = true;
    // (Une fenêtre haute : le texte hors de l'écran n'est pas peint.)
    b.taille = eframe::egui::vec2(1920.0, 3200.0);
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    b.image(Vec::new(), 1.0 / 60.0);
    let joint = b.textes.join("\n");
    for attendu in [
        "Performance",
        "Effets en aperçu réduit",
        "Traits vectoriels texturés",
        "Tremblement du trait",
        "Pelure d'oignon",
        "Cache de la carte graphique",
        "Cache des traits vectoriels",
        "Cache des effets",
        "Fils de calcul",
        "Vignettes",
        "Animations",
    ] {
        assert!(joint.contains(attendu), "« {attendu} » manque dans les Préférences :\n{joint}");
    }
}

/// Un réglage modifié dans les réglages arrive dans le programme à l'image suivante, sans
/// redémarrer.
#[test]
fn a_changed_setting_reaches_the_program_at_the_next_frame() {
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), false);
    let defaut = b.ctx.style().animation_time;
    assert!(defaut > 0.0);
    // (Le cache des effets : la valeur est reprise, puis remise ; les autres réglages touchent
    // des modules que d'autres essais lisent en même temps.)
    b.app.settings.perf.cache_effets_mo = 700;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(b.app.perf_applied.cache_effets_mo, 700);
    assert_eq!(crate::fxcache::limit(), 700 << 20);
    b.app.settings.perf.cache_effets_mo = d::CACHE_EFFETS_MO;
    b.image(Vec::new(), 1.0 / 60.0);
    assert_eq!(crate::fxcache::limit(), 768 << 20);
    // Les animations d'egui suivent le réglage (le style de ce contexte, propre à cet essai).
    assert_eq!(b.ctx.style().animation_time, defaut);
}

/// Un projet qui s'ouvre avec des dessins vectoriels texturés en rendu rapide : le rendu exact
/// vient tout de suite (réglage d'avant), après le repos, ou seulement avant un export.
#[test]
fn the_textured_strokes_setting_decides_when_the_exact_render_comes() {
    // (Le projet est posé comme dans les essais de performance, sans `load_doc` : celui-ci vide
    // les dessins dérivés des effets, que d'autres essais utilisent en même temps.)
    let ouvrir = |b: &mut crate::perf_tests::Banc, mode: TraitsTextures| {
        let (doc, id) = crate::affichage_tests::doc_with(1920, 1080, crate::affichage_tests::gros_traits(2, 1920, 1080, 3));
        b.app.settings.perf.traits_textures = mode;
        b.app.doc = doc;
        b.app.rejeu.cancel_all();
        b.app.rejeu_idle.clear();
        b.app.rejeu_apres_ouverture(id);
        id
    };
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), false);
    // Exact : le rejeu est demandé à l'ouverture, comme avant le diagnostic.
    let id = ouvrir(&mut b, TraitsTextures::Exact);
    assert!(b.app.rejeu.is_pending(id) && b.app.rejeu_idle.is_empty());
    // Rapide jusqu'au repos : rien n'est demandé tant que ça bouge, tout l'est après 2 secondes de calme.
    let id = ouvrir(&mut b, TraitsTextures::Repos);
    assert!(!b.app.rejeu.is_pending(id) && b.app.rejeu_idle == vec![id]);
    let bouge = b.souris(100.0, 100.0);
    b.image(bouge, 0.1);
    assert_eq!(b.app.rejeu_idle, vec![id], "la souris bouge : on attend");
    b.image(Vec::new(), 0.5);
    assert!(!b.app.rejeu.is_pending(id));
    b.image(Vec::new(), 3.0);
    assert!(b.app.rejeu_idle.is_empty() && b.app.rejeu.is_pending(id), "au repos, le rendu exact est demandé");
    // Rapide partout : rien ne vient seul, ni après un long repos ; avant un export, tout est exact.
    let id = ouvrir(&mut b, TraitsTextures::Rapide);
    b.image(Vec::new(), 30.0);
    b.image(Vec::new(), 30.0);
    assert_eq!(b.app.rejeu_idle, vec![id]);
    let avant = b.app.doc.cels[&id].version;
    b.app.exactify_idle();
    assert!(b.app.rejeu_idle.is_empty());
    assert!(b.app.doc.cels[&id].version > avant, "le dessin a été rendu exactement");
    assert_eq!(b.app.doc.cels[&id].vector.as_ref().map(|v| v.missing_caches()), Some(0), "tous ses traits ont leurs pixels exacts");
}

// ---------------------------------------------------------------------- le matériel

#[test]
fn the_hardware_is_detected() {
    let m = Materiel::systeme();
    assert!(m.processeur.logiques >= 1);
    #[cfg(target_arch = "x86_64")]
    {
        // SHOT est compilé pour x86-64-v3 : le processeur qui l'exécute l'a.
        assert!(m.processeur.avx2 && m.processeur.fma && m.processeur.bmi2 && m.processeur.x86_64_v3());
    }
    #[cfg(windows)]
    {
        let r = m.memoire.expect("la mémoire vive se lit sous Windows");
        assert!(r.totale >= 1 << 30 && r.disponible > 0 && r.disponible <= r.totale);
        assert!(!m.processeur.nom.is_empty(), "le nom du processeur se lit dans le registre");
        let physiques = m.processeur.physiques.expect("cœurs physiques");
        assert!(physiques >= 1 && physiques <= m.processeur.logiques);
    }
    let lignes = m.lignes();
    assert!(lignes.iter().any(|(n, v)| *n == "Processeur" && !v.is_empty()));
    assert!(lignes.iter().any(|(n, _)| *n == "Mémoire vive"));
}

#[test]
fn the_disk_test_cleans_up_after_itself_and_can_be_cancelled() {
    let dossier = std::env::temp_dir().join(format!("shot-diag-essai-{}", std::process::id()));
    std::fs::create_dir_all(&dossier).unwrap();
    let non = AtomicBool::new(false);
    let disque = crate::materiel::mesurer_disque(&dossier, 4, &non).expect("disque");
    assert!(disque.ecriture > 0.0 && disque.lecture > 0.0 && disque.taille_mo == 4);
    assert_eq!(std::fs::read_dir(&dossier).unwrap().count(), 0, "le fichier d'essai doit disparaître");
    let oui = AtomicBool::new(true);
    assert!(crate::materiel::mesurer_disque(&dossier, 4, &oui).is_none());
    assert_eq!(std::fs::read_dir(&dossier).unwrap().count(), 0, "annulé, il disparaît aussi");
    // Un dossier qui n'existe pas : pas de mesure, pas de panique.
    assert!(crate::materiel::mesurer_disque(&dossier.join("absent"), 4, &non).is_none());
    let _ = std::fs::remove_dir_all(&dossier);
}

#[test]
fn the_bench_can_be_cancelled_at_once_and_never_blocks() {
    let t = Instant::now();
    let banc = Banc::lancer(None, || {});
    banc.annuler();
    let debut = Instant::now();
    while !banc.fini() && debut.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(banc.fini(), "le banc doit s'arrêter peu après l'annulation");
    let r = banc.joindre();
    assert!(r.annule);
    assert!(t.elapsed() < Duration::from_secs(20));
}

// ---------------------------------------------------------------------- le rapport

#[test]
fn the_report_gathers_the_machine_the_measures_and_the_switches() {
    let m = forte();
    let x = mesures(1.0);
    let c = d::choisir_profil(&m, &x);
    let cible = d::reglages_du_profil(Profil::Equilibre, &m);
    let r = d::rapport(&m, &x, &c, &PerfReglages::default(), &cible, 1_760_000_000);
    assert!(r.starts_with("SHOT — Rapport du diagnostic des performances"));
    for attendu in ["MATÉRIEL", "MESURES", "PROFIL PROPOSÉ : Qualité", "RÉGLAGES (actuel -> proposé)", "Carte d'essai", "Lecture d'un plan chargé", "2025-10-09"] {
        assert!(r.contains(attendu), "« {attendu} » manque :\n{r}");
    }
    // Le profil équilibré change des réglages (marqués d'une étoile).
    assert!(r.contains("* Effets en aperçu réduit"));
    assert!(!r.contains("* Animations de l'interface"));
    assert_eq!(d::date_texte(0), "1970-01-01 00:00 (UTC)");
    assert_eq!(d::date_texte(951_782_400 + 3600 * 13 + 60 * 5), "2000-02-29 13:05 (UTC)");
}

// ---------------------------------------------------------------------- le banc réel

/// Images de la fenêtre (proposition, fenêtre, résultats, mesures en cours, Préférences) dans le
/// dossier `SHOT_CAPTURE_DIR`, pour en vérifier la mise en page à l'œil. Sans carte graphique : rien.
#[test]
#[ignore]
fn diag_capture() {
    let dossier = std::env::var_os("SHOT_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let mut b = banc(crate::document::Document::with_frame(1920, 1080, 24), true);
    let sauver = |b: &crate::perf_tests::Banc, nom: &str| {
        let Some(rgba) = b.capturer() else { return println!("DIAG pas de carte graphique : {nom} ignorée") };
        image::save_buffer(dossier.join(nom), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
        println!("DIAG {nom}");
    };
    b.app.diag.offre = true;
    b.chauffer(6);
    sauver(&b, "diag-1-offre.png");
    b.app.diag.offre = false;
    b.app.run_action("aide.diagnostic");
    b.chauffer(6);
    sauver(&b, "diag-2-fenetre.png");
    // Des résultats inventés sur la vraie machine.
    let materiel = b.app.diag.materiel.clone().expect("matériel");
    let x = mesures(2.8);
    let choix = d::choisir_profil(&materiel, &x);
    let profil = choix.profil;
    b.app.diag.fait = Some(crate::diagnosticui::Termine { mesures: x, materiel, choix, date: 1_760_000_000 });
    let courant = b.app.settings.perf;
    b.app.diag.regarder(profil, &courant);
    b.chauffer(6);
    sauver(&b, "diag-3-resultats.png");
    b.app.diag.regarder(Profil::Performance, &courant);
    b.chauffer(6);
    sauver(&b, "diag-4-performance.png");
    // Les mesures en cours.
    b.app.diag.fait = None;
    b.app.diag.banc = Some(Banc::lancer(None, || {}));
    std::thread::sleep(Duration::from_millis(2500));
    b.chauffer(4);
    sauver(&b, "diag-5-mesures.png");
    b.app.diag.banc.as_ref().unwrap().annuler();
    b.app.diag.ouvert = false;
    b.app.run_action("fichier.preferences");
    b.chauffer(8);
    sauver(&b, "diag-6-preferences.png");
}

/// Le banc de 15 secondes, avec la carte graphique de la machine (un périphérique sans fenêtre).
#[test]
#[ignore]
fn diag_bench_reel() {
    let rs = crate::perf_tests::headless();
    let mut mat = Materiel::systeme();
    mat.carte = rs.as_ref().map(|rs| crate::materiel::carte(&rs.adapter.get_info()));
    println!("DIAG matériel : {mat:#?}");
    let t = Instant::now();
    let banc = Banc::lancer(rs.as_ref().map(|rs| (rs.device.clone(), rs.queue.clone())), || {});
    let mut derniere = String::new();
    while !banc.fini() {
        let e = banc.avancement.etape();
        if e != derniere {
            println!("DIAG [{:>5.1} s, {:>3.0} %] {e}", t.elapsed().as_secs_f64(), banc.avancement.fraction() * 100.0);
            derniere = e;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let r = banc.joindre();
    mat.disque = r.disque.clone();
    println!("DIAG durée totale : {:.1} s", t.elapsed().as_secs_f64());
    for l in d::lignes_mesures(&r.mesures) {
        println!("DIAG {l}");
    }
    println!("DIAG disque : {:?}", r.disque);
    let c = d::choisir_profil(&mat, &r.mesures);
    println!("DIAG profil : {:?} — {:?}", c.profil, c.raisons());
}
