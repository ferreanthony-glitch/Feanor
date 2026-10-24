//! Barres de commandes : ce qui est d'office dans chaque barre, ce qui passe sous « Plus… »,
//! les épingles retenues dans les réglages, et le défilement qui n'existe plus.

use std::collections::BTreeSet;

use eframe::egui::{self, pos2, vec2, RawInput, Rect};

use crate::app::App;
use crate::barres::{dans_la_barre, dans_plus, Barre, Cmd, Genre, ReglageBarre, ReglageBarres};
use crate::settings::Settings;

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-barres-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let app = App::sans_fenetre(&ctx, None);
    (ctx, app)
}

/// Les commandes qu'on voit d'office dans une barre (sans les affichages fixes).
fn d_office(barre: Barre) -> Vec<&'static str> {
    dans_la_barre(barre, &ReglageBarre::default()).into_iter().filter(|c| c.genre != Genre::Fixe).map(|c| c.id).collect()
}

#[test]
fn the_bars_are_slim_by_default() {
    // Barre de commandes : 19 icônes avant, 9 maintenant.
    assert_eq!(
        d_office(Barre::Commandes),
        vec![
            "fichier.nouveau",
            "fichier.ouvrir",
            "fichier.enregistrer",
            "fichier.exporter_film",
            "edition.annuler",
            "edition.retablir",
            "edition.couper",
            "edition.copier",
            "edition.coller",
        ]
    );
    // Les plans et le transport ne sont plus dans cette barre, même sous « Plus… » : ils sont dans celles du montage
    // et de la timeline (une commande, un endroit principal ; `docs/DOUBLONS.md`).
    assert_eq!(Barre::Commandes.commandes().len(), 9);
    // Barre du montage : 21 icônes avant, huit maintenant (les vues dessin et vignettes sont parties ; « Plus… » en plus).
    let montage = d_office(Barre::Montage);
    assert!((8..=12).contains(&montage.len()), "montage : {montage:?}");
    // Barre de la timeline : les dessins qu'on retire ou vide, la cadence, la marque, le commentaire,
    // les flips et le son sont sous « Plus… » ; la pelure d'oignon aussi (elle se règle sous le dessin), et
    // le panneau Effets n'y est plus (sa place est la barre des panneaux).
    let timeline = d_office(Barre::Timeline);
    for sous_plus in ["cadence", "dessin.dupliquer", "dessin.supprimer", "dessin.vider", "marque.basculer", "commentaire.ajouter", "flip.panneau", "vue.pelure", "pelure.panneau", "pelure_avant", "pelure_apres", "mode.basculer"] {
        assert!(!timeline.contains(&sous_plus), "{sous_plus} est sous « Plus… »");
    }
    assert!(Barre::Timeline.cherche("fx.panneau").is_none(), "Effets : la barre des panneaux, pas la timeline");
    for reste in ["lecture.basculer", "dessin.nouveau", "camera.cle_ici", "camera.cle_supprimer"] {
        assert!(timeline.contains(&reste), "{reste} reste dans la barre");
    }
}

#[test]
fn what_the_montage_bar_keeps_and_moves() {
    let montage = d_office(Barre::Montage);
    for reste in ["montage.debut", "lecture.basculer", "lecture.boucle", "panneau.nouveau", "sequence.nouvelle", "plans", "son.importer", "son"] {
        assert!(montage.contains(&reste), "{reste} reste dans la barre du montage");
    }
    // Les vues dessin et vignettes ont quitté la barre (leur action reste sur Maj+Tab).
    for parti in ["vue.dessin", "vue.vignettes"] {
        assert!(Barre::Montage.cherche(parti).is_none(), "{parti} n'est plus dans la barre du montage");
    }
    // Les commandes que INTERFACE.md range sous « Plus… », et la transition (arrivée depuis) ; « Animer ce plan »
    // aussi : l'onglet Animation, en haut à droite, le fait.
    for sous_plus in ["plan.avancer", "plan.reculer", "storyboard.verrouiller", "montage.aimanter", "fichier.importer_video", "commentaire.ajouter", "transition.ajouter", "mode.basculer", "son.lecture"] {
        assert!(!montage.contains(&sous_plus), "{sous_plus} est sous « Plus… »");
    }
}

#[test]
fn no_command_is_twice_in_a_bar_and_every_one_is_either_in_the_bar_or_under_plus() {
    for barre in Barre::TOUTES {
        let mut ids = BTreeSet::new();
        for c in barre.commandes() {
            assert!(ids.insert(c.id), "{barre:?} : {} deux fois", c.id);
        }
        // Une action n'a qu'une icône par barre (sauf les deux vues, qui sont un même choix).
        let mut actions = BTreeSet::new();
        for c in barre.commandes().iter().filter(|c| !c.action.is_empty() && c.action != "vue.planches") {
            assert!(actions.insert(c.action), "{barre:?} : l'action {} a deux icônes", c.action);
        }
        // Toute combinaison d'épingles garde chaque commande à un seul endroit.
        let mut r = ReglageBarre::default();
        for (i, c) in barre.commandes().iter().enumerate() {
            if i % 2 == 0 {
                r.epingler(c, !r.dans_la_barre(c));
            }
        }
        for reglage in [ReglageBarre::default(), r] {
            let barre_ids: BTreeSet<&str> = dans_la_barre(barre, &reglage).into_iter().map(|c| c.id).collect();
            let plus_ids: BTreeSet<&str> = dans_plus(barre, &reglage).into_iter().map(|c| c.id).collect();
            assert!(barre_ids.is_disjoint(&plus_ids), "{barre:?} : une commande des deux côtés");
            let tout: BTreeSet<&str> = barre.commandes().iter().map(|c| c.id).collect();
            assert_eq!(barre_ids.union(&plus_ids).copied().collect::<BTreeSet<_>>(), tout, "{barre:?} : une commande perdue");
        }
    }
}

#[test]
fn every_action_moved_off_a_bar_is_still_reachable() {
    let map = crate::keymap::Keymap::from_texts(crate::keymap::DEFAULT_KEYMAP, None);
    for barre in Barre::TOUTES {
        for c in barre.commandes() {
            match c.genre {
                Genre::Bouton | Genre::Etat => {
                    // Une action de la palette (Ctrl+K) et un bloc dans keymap.toml : raccourci, bulle,
                    // clic droit « Assigner un raccourci ».
                    let info = crate::keymap::action_info(c.action).unwrap_or_else(|| panic!("{barre:?} : {} n'est pas dans keymap::ACTIONS", c.action));
                    assert!(info.implemented, "{} doit être implémentée", c.action);
                    assert!(map.warnings.iter().all(|w| !w.contains(c.action)), "{}", c.action);
                    assert!(!c.icone.is_empty() && !c.nom.is_empty() && !c.description.is_empty(), "{} : icône, nom et bulle", c.id);
                }
                Genre::Menu | Genre::Reglage => assert!(!c.icone.is_empty() && !c.nom.is_empty() && !c.description.is_empty(), "{}", c.id),
                Genre::Fixe => assert!(c.d_office, "un affichage fixe est toujours dans la barre"),
            }
        }
        // Sous « Plus… » : tout ce qui n'est pas dans la barre (le menu les liste, avec leur raccourci).
        let plus: Vec<&Cmd> = dans_plus(barre, &ReglageBarre::default());
        let hors_barre = barre.commandes().iter().filter(|c| !c.d_office && c.genre != Genre::Fixe).count();
        assert_eq!(plus.len(), hors_barre);
    }
}

#[test]
fn pinning_moves_a_command_into_the_bar_and_back_without_leaving_a_trace() {
    let marque = Barre::Timeline.cherche("marque.basculer").unwrap();
    let boucle = Barre::Timeline.cherche("lecture.boucle").unwrap();
    let mut r = ReglageBarre::default();
    assert!(!r.dans_la_barre(marque) && r.dans_la_barre(boucle));
    r.epingler(marque, true);
    r.epingler(marque, true);
    assert!(r.dans_la_barre(marque));
    assert_eq!(r.epinglees, vec!["marque.basculer".to_string()], "une seule fois");
    assert!(dans_la_barre(Barre::Timeline, &r).iter().any(|c| c.id == "marque.basculer"));
    assert!(!dans_plus(Barre::Timeline, &r).iter().any(|c| c.id == "marque.basculer"));
    // Une commande d'origine qu'on retire passe sous « Plus… ».
    r.epingler(boucle, false);
    assert!(!r.dans_la_barre(boucle));
    assert!(dans_plus(Barre::Timeline, &r).iter().any(|c| c.id == "lecture.boucle"));
    // Revenir à l'état d'origine efface le choix.
    r.epingler(marque, false);
    r.epingler(boucle, true);
    assert!(r.est_vide(), "{r:?}");
    // L'ordre de la barre ne change pas : une commande épinglée reprend sa place d'origine.
    let mut r = ReglageBarre::default();
    r.epingler(Barre::Timeline.cherche("cadence").unwrap(), true);
    let ids: Vec<&str> = dans_la_barre(Barre::Timeline, &r).iter().map(|c| c.id).collect();
    let apres = ids.iter().position(|i| *i == "cadence").unwrap();
    assert_eq!(ids[apres - 1], "lecture.boucle");
    // Les affichages fixes ne s'épinglent pas.
    let mut r = ReglageBarre::default();
    r.epingler(Barre::Timeline.cherche("plan").unwrap(), false);
    assert!(r.est_vide());
}

#[test]
fn pins_are_kept_per_bar_in_the_settings() {
    let mut s = Settings::default();
    assert!(s.bars.est_vide());
    // Les barres d'origine n'écrivent rien dans le fichier (ni table, ni clé).
    let text = toml::to_string(&s).unwrap();
    assert!(!text.contains("barres"), "{text}");
    s.bars.timeline.epingler(Barre::Timeline.cherche("marque.basculer").unwrap(), true);
    s.bars.montage.epingler(Barre::Montage.cherche("plan.avancer").unwrap(), true);
    s.bars.commandes.epingler(Barre::Commandes.cherche("fichier.nouveau").unwrap(), false);
    let text = toml::to_string(&s).unwrap();
    let back: Settings = toml::from_str(&text).unwrap();
    assert_eq!(back.bars, s.bars);
    assert_eq!(back.bars.timeline.epinglees, vec!["marque.basculer".to_string()]);
    assert_eq!(back.bars.montage.epinglees, vec!["plan.avancer".to_string()]);
    assert_eq!(back.bars.commandes.retirees, vec!["fichier.nouveau".to_string()]);
    // Des réglages d'avant les barres : les barres d'origine.
    let old: Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(old.bars.est_vide());
}

#[test]
fn a_damaged_pin_list_is_cleaned_when_read() {
    let sale = ReglageBarres {
        commandes: ReglageBarre {
            // « fichier.nouveau » est d'origine dans la barre : il ne peut pas être « épinglé » ; un
            // inconnu et un doublon.
            // (« panneau.nouveau » : une commande que cette barre avait avant la passe de simplification, 2026-10-21 ;
            // un réglage d'avant la perd sans bruit.)
            epinglees: vec!["fichier.nouveau".into(), "panneau.nouveau".into(), "panneau.nouveau".into(), "nimporte.quoi".into()],
            retirees: vec!["fichier.ouvrir".into(), "panneau.dupliquer".into(), "fichier.ouvrir".into()],
        },
        timeline: ReglageBarre { epinglees: vec!["plan".into(), "pelure.panneau".into(), "pelure.panneau".into()], retirees: vec!["fx.panneau".into()] },
        ..Default::default()
    };
    let propre = sale.nettoyees();
    assert!(propre.commandes.epinglees.is_empty(), "{:?}", propre.commandes.epinglees);
    assert_eq!(propre.commandes.retirees, vec!["fichier.ouvrir".to_string()]);
    assert_eq!(propre.timeline.epinglees, vec!["pelure.panneau".to_string()], "un affichage fixe n'est pas une épingle, un doublon une seule fois");
    assert!(propre.timeline.retirees.is_empty(), "une commande qui n'existe plus est oubliée");
}

#[test]
fn pinning_in_the_app_is_remembered_in_the_settings_file() {
    let (_ctx, mut app) = headless("epingle");
    let marque = Barre::Timeline.cherche("marque.basculer").unwrap();
    app.epingler_commande(Barre::Timeline, marque, true);
    assert!(app.settings.bars.de(Barre::Timeline).dans_la_barre(marque));
    let relu = Settings::load();
    assert!(relu.bars.de(Barre::Timeline).dans_la_barre(marque), "le choix est écrit");
    assert!(!relu.bars.de(Barre::Montage).dans_la_barre(Barre::Montage.cherche("plan.avancer").unwrap()), "les autres barres n'ont pas bougé");
    // Une autre séance retrouve la barre.
    let ctx = egui::Context::default();
    let app2 = App::sans_fenetre(&ctx, None);
    assert!(app2.settings.bars.de(Barre::Timeline).dans_la_barre(marque));
}

/// Une barre dessinée dans une zone de `largeur` points : la largeur qu'elle prend.
fn largeur_prise(app: &mut App, barre: Barre, largeur: f32) -> f32 {
    let ctx = egui::Context::default();
    let mut prise = 0.0;
    // (Deux images : la première pose les polices et les tailles.)
    for t in 0..2 {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(largeur, 120.0))), time: Some(t as f64 * 0.1), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = ui.horizontal(|ui| app.barre_ui(ui, barre));
                prise = r.response.rect.width();
            });
        });
    }
    prise
}

#[test]
fn a_bar_never_scrolls_what_does_not_fit_goes_under_plus() {
    let (_ctx, mut app) = headless("etroit");
    for barre in Barre::TOUTES {
        let large = largeur_prise(&mut app, barre, 3000.0);
        assert!(large > 100.0, "{barre:?} : la barre est dessinée ({large})");
        // Dans 520 points (les affichages fixes de la timeline, le titre du plan et l'image courante,
        // en prennent déjà 200), la barre tient : elle ne défile pas, et montre moins de commandes.
        let etroit = largeur_prise(&mut app, barre, 520.0);
        assert!(etroit <= 520.0, "{barre:?} : {etroit} points dans 520");
        assert!(etroit <= large, "{barre:?} : une barre étroite ne montre pas plus de commandes ({etroit} contre {large})");
        if barre == Barre::Timeline {
            assert!(etroit < large, "la timeline, plus étroite, passe des commandes sous « Plus… »");
        }
    }
    // Toutes les commandes épinglées d'une barre trop étroite restent atteignables : elles sont
    // dans « Plus… » (la liste, plus bas, est celle du menu).
    let r = ReglageBarre::default();
    assert!(dans_la_barre(Barre::Timeline, &r).len() + dans_plus(Barre::Timeline, &r).len() >= Barre::Timeline.commandes().len());
}

#[test]
fn the_bar_commands_do_what_their_old_buttons_did() {
    let (_ctx, mut app) = headless("actions");
    // La boucle : l'état de la barre est celui de l'application, et la commande le bascule.
    let boucle = Barre::Timeline.cherche("lecture.boucle").unwrap();
    assert_eq!(app.vue_de(Barre::Timeline, boucle).etat, Some(app.looping));
    let avant = app.looping;
    app.executer_commande(Barre::Timeline, boucle);
    assert_eq!(app.looping, !avant);
    // Le son pendant la lecture : l'icône suit l'état.
    let son = Barre::Montage.cherche("son.lecture").unwrap();
    let on = app.sound_on;
    assert_eq!(app.vue_de(Barre::Montage, son).etat, Some(on));
    app.executer_commande(Barre::Montage, son);
    assert_eq!(app.sound_on, !on);
    // Les deux vues (dessin, vignettes) sont un choix, par leur action (Maj+Tab) : elles ne sont plus dans la barre.
    let avant = app.board_view;
    app.run_action("vue.planches");
    assert!(app.board_view != avant, "Maj+Tab change de vue");
    // « Coller les plans » est grisé tant que rien n'est copié.
    let coller = Barre::Montage.cherche("edition.coller").unwrap();
    assert!(!app.vue_de(Barre::Montage, coller).actif || app.panel_clipboard.is_some());
    // Un plan de plus par la barre du montage, sous « Plus… » ou non.
    let n = app.doc.panels.len();
    app.executer_commande(Barre::Montage, Barre::Montage.cherche("panneau.nouveau").unwrap());
    assert_eq!(app.doc.panels.len(), n + 1);
    // Le bouton « Animer ce plan » entre en animation, et la même commande en revient.
    let animer = Barre::Timeline.cherche("mode.basculer").unwrap();
    app.executer_commande(Barre::Timeline, animer);
    assert_eq!(app.mode, crate::app::Mode::Animation);
    assert_eq!(app.vue_de(Barre::Timeline, animer).nom, "Retour au storyboard");
    app.executer_commande(Barre::Timeline, animer);
    assert_eq!(app.mode, crate::app::Mode::Storyboard);
}

#[test]
fn the_selected_icons_use_the_ink_made_for_the_accent() {
    // Une icône allumée est dessinée avec `theme::sur_accent` (`bulles::symbole_etat`), pas avec
    // la couleur de survol d'egui ; éteinte, elle garde l'encre du thème.
    let on = crate::bulles::symbole_etat("x", true);
    let off = crate::bulles::symbole_etat("x", false);
    let couleur = |t: &egui::RichText| format!("{t:?}");
    assert_ne!(couleur(&on), couleur(&off));
}
