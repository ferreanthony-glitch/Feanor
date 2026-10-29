//! Mon panneau (essai du menu Labo, 2026-10-28) : ajouter et retirer un bouton, réordonner, séparateurs, réglages du poste (écrits,
//! relus, anciens fichiers), outil perso (capturer puis remettre l'outil, la brosse et la couleur), commande inconnue ignorée, grille.

use eframe::egui::{self, pos2, vec2, Pos2, RawInput, Rect};

use crate::app::{App, Tool};
use crate::brush::BrushSettings;
use crate::brushtex::Target;
use crate::dock::Pane;
use crate::monpanneau::{case_la_plus_proche, commande_connue, disposer, Bouton, OutilPerso, Reglage};
use crate::settings::Settings;

fn headless(name: &str) -> (egui::Context, App) {
    let dir = std::env::temp_dir().join(format!("shot-monpanneau-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    // (Un projet est ouvert : sans projet, les commandes d'un projet ne partent pas, `sansprojet.rs`.)
    app.welcome = false;
    (ctx, app)
}

fn commande(action: &str) -> Bouton {
    Bouton::Commande { action: action.to_string() }
}

fn actions(r: &Reglage) -> Vec<String> {
    r.boutons
        .iter()
        .map(|b| match b {
            Bouton::Commande { action } => action.clone(),
            Bouton::Brosse { nom, .. } => format!("brosse:{nom}"),
            Bouton::Outil(o) => format!("outil:{}", o.nom),
            Bouton::Separateur => "|".to_string(),
            Bouton::Inconnu => "?".to_string(),
        })
        .collect()
}

fn perso(nom: &str, outil: &str) -> OutilPerso {
    OutilPerso { nom: nom.to_string(), outil: outil.to_string(), pinceau: None, couleur: [1, 2, 3] }
}

// ---------------------------------------------------------------------------------------------------- la liste

#[test]
fn a_button_is_added_then_removed() {
    let mut r = Reglage::default();
    assert!(r.est_vide());
    assert!(r.ajouter(commande("outil.pinceau")));
    assert!(r.ajouter(commande("edition.annuler")));
    assert_eq!(actions(&r), ["outil.pinceau", "edition.annuler"]);
    assert!(!r.est_vide());
    // La même commande ne se pose pas deux fois ; la même brosse non plus.
    assert!(!r.ajouter(commande("outil.pinceau")));
    assert!(r.ajouter(Bouton::Brosse { nom: "Craie".into(), source: "Mes brosses".into(), gomme: false }));
    assert!(!r.ajouter(Bouton::Brosse { nom: "Craie".into(), source: "Mes brosses".into(), gomme: false }));
    // Une gomme de même nom est une autre brosse.
    assert!(r.ajouter(Bouton::Brosse { nom: "Craie".into(), source: "Mes brosses".into(), gomme: true }));
    // Deux outils perso de même nom, pourquoi pas : chacun a ses réglages.
    assert!(r.ajouter(Bouton::Outil(perso("A", "outil.pinceau"))));
    assert!(r.ajouter(Bouton::Outil(perso("A", "outil.pinceau"))));
    assert_eq!(r.boutons.len(), 6);
    // Retirer.
    assert_eq!(r.retirer(1), Some(commande("edition.annuler")));
    assert!(!r.contient_commande("edition.annuler"));
    assert!(r.contient_commande("outil.pinceau"));
    assert_eq!(r.retirer(99), None, "un rang hors liste ne fait rien");
    assert_eq!(r.boutons.len(), 5);
    // Retiré, on peut le remettre.
    assert!(r.ajouter(commande("edition.annuler")));
}

#[test]
fn a_button_moves_to_the_place_it_is_dropped_on() {
    let mut r = Reglage::default();
    for a in ["a", "b", "c", "d"] {
        r.ajouter(commande(a));
    }
    assert!(r.deplacer(0, 2));
    assert_eq!(actions(&r), ["b", "c", "a", "d"]);
    assert!(r.deplacer(3, 0));
    assert_eq!(actions(&r), ["d", "b", "c", "a"]);
    assert!(r.deplacer(1, 2));
    assert_eq!(actions(&r), ["d", "c", "b", "a"]);
    // Même place, ou hors liste : rien ne bouge.
    assert!(!r.deplacer(2, 2));
    assert!(!r.deplacer(9, 0));
    assert!(!r.deplacer(0, 9));
    assert_eq!(actions(&r), ["d", "c", "b", "a"]);
}

#[test]
fn a_separator_goes_between_two_buttons() {
    let mut r = Reglage::default();
    for a in ["a", "b", "c"] {
        r.ajouter(commande(a));
    }
    assert_eq!(r.separateur_apres(0), Some(1));
    assert_eq!(actions(&r), ["a", "|", "b", "c"]);
    // Pas contre un autre séparateur, ni tout au bout, ni sur un séparateur.
    assert_eq!(r.separateur_apres(0), None, "il y en a déjà un juste après");
    assert_eq!(r.separateur_apres(1), None, "on ne met pas un séparateur après un séparateur");
    assert_eq!(r.separateur_apres(3), None, "tout au bout, il ne sépare rien");
    assert_eq!(r.separateur_apres(9), None);
    assert_eq!(r.separateur_apres(2), Some(3));
    assert_eq!(actions(&r), ["a", "|", "b", "|", "c"]);
    // On le retire comme un bouton, et il se déplace.
    assert!(r.deplacer(1, 3));
    assert_eq!(actions(&r), ["a", "b", "|", "|", "c"]);
    assert_eq!(r.retirer(2), Some(Bouton::Separateur));
    assert_eq!(actions(&r), ["a", "b", "|", "c"]);
    // Plusieurs séparateurs n'empêchent pas d'ajouter la même commande : ce ne sont pas des commandes.
    assert!(r.ajouter(Bouton::Separateur));
}

#[test]
fn a_personal_tool_is_renamed_and_named_freely() {
    let mut r = Reglage::default();
    assert_eq!(r.nom_libre(Tool::Pencil), "Crayon perso");
    r.ajouter(Bouton::Outil(perso("Crayon perso", "outil.crayon")));
    assert_eq!(r.nom_libre(Tool::Pencil), "Crayon perso 2");
    assert_eq!(r.nom_libre(Tool::Brush), "Pinceau perso");
    assert!(r.renommer(0, "  Mon crayon bleu "));
    assert_eq!(actions(&r), ["outil:Mon crayon bleu"]);
    assert!(!r.renommer(0, "Mon crayon bleu"), "même nom : rien");
    assert!(!r.renommer(0, "   "), "un nom vide ne change rien");
    assert_eq!(actions(&r), ["outil:Mon crayon bleu"]);
    // Une commande ne se renomme pas : son nom est celui du registre.
    r.ajouter(commande("outil.pinceau"));
    assert!(!r.renommer(1, "Autre"));
    assert!(!r.renommer(9, "Autre"));
}

// ------------------------------------------------------------------------------------------------ les réglages

fn brosse_perso() -> BrushSettings {
    BrushSettings { size: 33.5, opacity: 0.4, ..BrushSettings::graphite() }
}

#[test]
fn the_panel_is_kept_in_the_settings_and_read_back() {
    let mut s = Settings::default();
    s.mon_panneau.noms = true;
    s.mon_panneau.ajouter(commande("outil.pinceau"));
    s.mon_panneau.ajouter(Bouton::Brosse { nom: "Ma craie".into(), source: "Mes brosses".into(), gomme: false });
    s.mon_panneau.ajouter(Bouton::Separateur);
    s.mon_panneau.ajouter(Bouton::Outil(OutilPerso { nom: "Crayon bleu".into(), outil: "outil.crayon".into(), pinceau: Some(brosse_perso()), couleur: [10, 20, 200] }));
    s.mon_panneau.ajouter(Bouton::Outil(OutilPerso { nom: "Seau rouge".into(), outil: "outil.remplissage".into(), pinceau: None, couleur: [220, 0, 0] }));
    let texte = toml::to_string(&s).unwrap();
    assert!(texte.contains("mon_panneau"), "{texte}");
    let lu: Settings = toml::from_str(&texte).unwrap();
    assert_eq!(lu.mon_panneau, s.mon_panneau);
    // Rien d'autre n'a bougé dans le fichier.
    assert_eq!(toml::to_string(&lu).unwrap(), texte);
}

#[test]
fn an_empty_panel_writes_nothing_and_old_settings_have_an_empty_panel() {
    let texte = toml::to_string(&Settings::default()).unwrap();
    assert!(!texte.contains("mon_panneau"), "pas de section pour un panneau vide");
    let ancien: Settings = toml::from_str("copie_secours_minutes = 1").unwrap();
    assert!(ancien.mon_panneau.est_vide());
    assert!(ancien.mon_panneau.boutons.is_empty() && !ancien.mon_panneau.noms);
    // « Afficher les noms » seul est un réglage à garder.
    let noms: Settings = toml::from_str("[mon_panneau]\nnoms = true").unwrap();
    assert!(noms.mon_panneau.noms && noms.mon_panneau.boutons.is_empty());
    assert!(toml::to_string(&noms).unwrap().contains("noms = true"));
}

#[test]
fn the_settings_file_of_the_station_keeps_the_panel_between_sessions() {
    let (_ctx, mut app) = headless("fichier");
    app.settings.mon_panneau.noms = true;
    app.mon_panneau_ajouter_commande("outil.gomme");
    app.mon_panneau_ajouter_commande("edition.annuler");
    app.settings.mon_panneau.ajouter(Bouton::Outil(OutilPerso { nom: "À moi".into(), outil: "outil.pinceau".into(), pinceau: Some(brosse_perso()), couleur: [9, 8, 7] }));
    app.settings.save().unwrap();
    // Le lancement suivant relit le fichier.
    let lu = Settings::load();
    assert_eq!(lu.mon_panneau, app.settings.mon_panneau);
    assert_eq!(actions(&lu.mon_panneau), ["outil.gomme", "edition.annuler", "outil:À moi"]);
    // Et le projet n'a pas bougé : Mon panneau n'est dans aucun format de projet (rien de lui dans le document).
}

#[test]
fn an_unknown_command_in_the_settings_is_ignored_without_breaking_anything() {
    // Les réglages d'une autre version : une commande qui n'existe plus, un genre inconnu, un outil disparu, une table abîmée
    // (id qui n'est pas un texte), des séparateurs de trop.
    let texte = r#"
copie_secours_minutes = 7

[mon_panneau]
noms = true

[[mon_panneau.boutons]]
genre = "commande"
id = "outil.pinceau"

[[mon_panneau.boutons]]
genre = "commande"
id = "ancienne.commande_disparue"

[[mon_panneau.boutons]]
genre = "machin_du_futur"
id = "x"

[[mon_panneau.boutons]]
genre = "outil"
nom = "Orphelin"
outil = "outil.disparu"

[[mon_panneau.boutons]]
genre = "commande"
id = 12

[[mon_panneau.boutons]]
genre = "separateur"

[[mon_panneau.boutons]]
genre = "separateur"

[[mon_panneau.boutons]]
genre = "commande"
id = "edition.annuler"
"#;
    let s: Settings = toml::from_str(texte).expect("une commande inconnue ne fait pas échouer la lecture des réglages");
    assert_eq!(s.autosave_minutes, 7, "les autres réglages sont lus");
    assert!(s.mon_panneau.noms);
    let net = s.mon_panneau.nettoye(commande_connue);
    assert_eq!(actions(&net), ["outil.pinceau", "|", "edition.annuler"]);
    assert!(!commande_connue("ancienne.commande_disparue") && commande_connue("outil.pinceau"));
    // Le même chemin que le lancement : réglages écrits avec une commande inconnue, relus par `Settings::load`.
    let (_ctx, mut app) = headless("inconnue");
    app.settings.mon_panneau.ajouter(commande("outil.pinceau"));
    app.settings.mon_panneau.ajouter(commande("ancienne.commande_disparue"));
    app.settings.mon_panneau.ajouter(commande("edition.annuler"));
    app.settings.save().unwrap();
    assert_eq!(actions(&Settings::load().mon_panneau), ["outil.pinceau", "edition.annuler"]);
    // Une commande pas encore faite n'est pas proposée non plus.
    let pas_faite = crate::keymap::ACTIONS.iter().find(|a| !a.implemented);
    if let Some(a) = pas_faite {
        assert!(!commande_connue(a.id), "{}", a.id);
    }
}

#[test]
fn an_unknown_command_cannot_be_added_to_the_panel() {
    let (_ctx, mut app) = headless("refus");
    app.mon_panneau_ajouter_commande("ancienne.commande_disparue");
    assert!(app.settings.mon_panneau.boutons.is_empty());
    assert!(app.status.contains("ne peut pas"), "{}", app.status);
    app.mon_panneau_ajouter_commande("outil.pinceau");
    app.mon_panneau_ajouter_commande("outil.pinceau");
    assert_eq!(app.settings.mon_panneau.boutons.len(), 1);
    assert!(app.status.contains("déjà"), "{}", app.status);
}

// ---------------------------------------------------------------------------------------------- l'outil perso

#[test]
fn a_personal_tool_captures_then_restores_the_tool_the_brush_and_the_color() {
    let (_ctx, mut app) = headless("outil");
    app.set_tool(Tool::Pencil);
    app.pencil = brosse_perso();
    app.color = [10, 20, 30];
    let o = app.capturer_outil_perso("Mon crayon".into());
    assert_eq!((o.outil.as_str(), o.couleur), ("outil.crayon", [10, 20, 30]));
    assert_eq!(o.pinceau, Some(brosse_perso()));
    assert_eq!(o.outil(), Some(Tool::Pencil));
    // On change tout : l'outil, la brosse du crayon, la couleur.
    let pinceau_avant = app.brush;
    app.set_tool(Tool::Eraser);
    app.pencil = BrushSettings { size: 90.0, ..BrushSettings::graphite() };
    app.color = [200, 0, 0];
    assert!(app.appliquer_outil_perso(&o));
    assert_eq!(app.tool, Tool::Pencil);
    assert_eq!(app.pencil, brosse_perso());
    assert_eq!(app.color, [10, 20, 30]);
    assert_eq!(app.brush, pinceau_avant, "la brosse du pinceau n'a pas bougé : l'outil perso est un crayon");
    // La même chose par le clic sur le bouton.
    app.set_tool(Tool::Fill);
    app.color = [1, 1, 1];
    app.pencil.size = 2.0;
    app.settings.mon_panneau.ajouter(Bouton::Outil(o.clone()));
    app.mon_panneau_clic(0);
    assert_eq!((app.tool, app.pencil, app.color), (Tool::Pencil, brosse_perso(), [10, 20, 30]));
}

#[test]
fn a_personal_tool_of_the_brush_or_the_eraser_keeps_its_own_brush() {
    let (_ctx, mut app) = headless("brosses");
    // Le pinceau.
    app.set_tool(Tool::Brush);
    app.brush = BrushSettings { size: 61.0, ..BrushSettings::brush() };
    app.color = [5, 6, 7];
    let pinceau = app.capturer_outil_perso("P".into());
    // La gomme.
    app.set_tool(Tool::Eraser);
    app.eraser = BrushSettings { size: 17.0, ..BrushSettings::eraser() };
    let gomme = app.capturer_outil_perso("G".into());
    assert_eq!(gomme.pinceau.map(|p| p.size), Some(17.0));
    // Un outil sans brosse (Remplir) ne garde que l'outil et la couleur.
    app.set_tool(Tool::Fill);
    let seau = app.capturer_outil_perso("S".into());
    assert_eq!(seau.pinceau, None);
    // On remet chacun.
    app.brush.size = 1.0;
    app.eraser.size = 1.0;
    app.color = [0, 0, 0];
    app.appliquer_outil_perso(&gomme);
    assert_eq!((app.tool, app.eraser.size, app.brush.size), (Tool::Eraser, 17.0, 1.0));
    app.appliquer_outil_perso(&pinceau);
    assert_eq!((app.tool, app.brush.size, app.eraser.size, app.color), (Tool::Brush, 61.0, 17.0, [5, 6, 7]));
    app.brush.size = 2.0;
    app.appliquer_outil_perso(&seau);
    assert_eq!((app.tool, app.brush.size, app.color), (Tool::Fill, 2.0, [5, 6, 7]));
}

#[test]
fn a_personal_tool_whose_tool_is_gone_does_nothing_and_says_so() {
    let (_ctx, mut app) = headless("disparu");
    app.color = [4, 4, 4];
    let tool = app.tool;
    assert!(!app.appliquer_outil_perso(&perso("Fantôme", "outil.disparu")));
    assert_eq!((app.tool, app.color), (tool, [4, 4, 4]));
    assert!(app.status.contains("n'existe plus"), "{}", app.status);
}

#[test]
fn saving_the_current_tool_makes_a_named_button() {
    let (_ctx, mut app) = headless("enregistrer");
    app.set_tool(Tool::Pencil);
    app.color = [30, 60, 90];
    app.mon_panneau_enregistrer_outil();
    app.mon_panneau_enregistrer_outil();
    assert_eq!(actions(&app.settings.mon_panneau), ["outil:Crayon perso", "outil:Crayon perso 2"]);
    // Le nom se saisit tout de suite, sur le dernier bouton.
    assert_eq!(app.mon_panneau.renommer, Some((1, "Crayon perso 2".to_string())));
    // Le bouton montre l'icône de l'outil et la pastille de sa couleur.
    let vue_pastille = match &app.settings.mon_panneau.boutons[0] {
        Bouton::Outil(o) => (o.outil.clone(), o.couleur, o.pinceau.is_some()),
        autre => panic!("{autre:?}"),
    };
    assert_eq!(vue_pastille, ("outil.crayon".to_string(), [30, 60, 90], true));
    // Et il est écrit pour la prochaine fois.
    assert_eq!(actions(&Settings::load().mon_panneau), ["outil:Crayon perso", "outil:Crayon perso 2"]);
}

#[test]
fn a_brush_button_takes_the_brush_of_my_brushes() {
    let (_ctx, mut app) = headless("brosse");
    let idx = app.brushes.save_mine("Ma brosse d'essai", Target::Brush, brosse_perso());
    app.mon_panneau_ajouter_brosse(idx);
    app.mon_panneau_ajouter_brosse(idx);
    assert_eq!(app.settings.mon_panneau.boutons.len(), 1, "une seule fois");
    assert!(matches!(&app.settings.mon_panneau.boutons[0], Bouton::Brosse { nom, source, gomme: false } if nom == "Ma brosse d'essai" && source == "Mes brosses"));
    app.set_tool(Tool::Pencil);
    app.brush = BrushSettings::brush();
    app.mon_panneau_clic(0);
    assert_eq!(app.brush, brosse_perso());
    assert_eq!(app.tool, Tool::Brush, "le pinceau est pris avec elle");
    // Une brosse qui n'est plus dans la bibliothèque : le clic le dit, rien ne change.
    app.settings.mon_panneau.ajouter(Bouton::Brosse { nom: "Disparue".into(), source: "Mes brosses".into(), gomme: false });
    app.set_tool(Tool::Fill);
    app.mon_panneau_clic(1);
    assert_eq!(app.tool, Tool::Fill);
    assert!(app.status.contains("Disparue") || app.status.contains("chargent"), "{}", app.status);
}

#[test]
fn a_command_button_runs_the_command_of_the_registry() {
    let (_ctx, mut app) = headless("commande");
    app.set_tool(Tool::Brush);
    app.mon_panneau_ajouter_commande("outil.gomme");
    app.mon_panneau_clic(0);
    assert_eq!(app.tool, Tool::Eraser, "comme la touche ou la palette");
    // Un bouton hors liste ou un séparateur : rien.
    app.settings.mon_panneau.ajouter(Bouton::Separateur);
    app.mon_panneau_clic(1);
    app.mon_panneau_clic(42);
    assert_eq!(app.tool, Tool::Eraser);
}

// ------------------------------------------------------------------------------ le clic droit et le panneau ouvert

#[test]
fn the_right_click_entry_exists_only_while_the_panel_is_open_and_adds_the_command() {
    let (ctx, mut app) = headless("ouvert");
    assert!(!crate::monpanneauui::ouvert(&ctx));
    app.mon_panneau_suivre(&ctx);
    assert!(!crate::monpanneauui::ouvert(&ctx), "fermé : rien ne change dans l'interface");
    // Une demande d'ajout alors que le panneau est fermé n'ajoute rien.
    crate::monpanneauui::demander_ajout(&ctx, "outil.pinceau");
    app.mon_panneau_suivre(&ctx);
    assert!(app.settings.mon_panneau.boutons.is_empty());
    // Ouvert : le drapeau est levé, la demande ajoute.
    app.show_pane(Pane::MonPanneau);
    assert!(app.pane_shown(Pane::MonPanneau));
    app.mon_panneau_suivre(&ctx);
    assert!(crate::monpanneauui::ouvert(&ctx));
    crate::monpanneauui::demander_ajout(&ctx, "outil.pinceau");
    app.mon_panneau_suivre(&ctx);
    assert_eq!(actions(&app.settings.mon_panneau), ["outil.pinceau"]);
    // Fermé de nouveau : le drapeau retombe.
    app.show_pane(Pane::MonPanneau);
    app.toggle_pane(Pane::MonPanneau);
    app.mon_panneau_suivre(&ctx);
    assert!(!crate::monpanneauui::ouvert(&ctx));
}

#[test]
fn the_icon_of_a_command_is_its_usual_one() {
    let (ctx, _app) = headless("icones");
    use crate::monpanneauui::{apprendre, infos};
    // Un outil : son icône, son nom, sa description.
    let pinceau = infos(&ctx, "outil.pinceau");
    assert_eq!((pinceau.icone.as_str(), pinceau.nom.as_str()), (Tool::Brush.icon(), "Pinceau"));
    assert_eq!(pinceau.description, Tool::Brush.description());
    // Une commande des barres : celle de la barre.
    let annuler = infos(&ctx, "edition.annuler");
    assert_eq!((annuler.icone.as_str(), annuler.nom.as_str()), (egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE, "Annuler"));
    // Un panneau : l'icône de la barre des panneaux.
    assert_eq!(infos(&ctx, "vue.calques").icone, Pane::Layers.icon());
    assert_eq!(infos(&ctx, crate::monpanneau::ACTION).icone, Pane::MonPanneau.icon());
    // Une commande sans icône au registre : une icône par défaut, sous son nom du registre ; jamais d'identifiant brut.
    let autre = infos(&ctx, "vue.cadres_securite");
    assert_eq!(autre.nom, crate::keymap::action_info("vue.cadres_securite").unwrap().label);
    // Ce que SHOT dessine pour elle pendant que le panneau est ouvert devient son icône. Fermé : rien n'est retenu.
    apprendre(&ctx, "vue.cadres_securite", "X", "Zones", "d");
    assert_eq!(infos(&ctx, "vue.cadres_securite").icone, autre.icone);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("shot_mon_panneau_ouvert"), true));
    apprendre(&ctx, "vue.cadres_securite", "X", "Zones", "d");
    assert_eq!(infos(&ctx, "vue.cadres_securite"), crate::monpanneauui::Infos { icone: "X".into(), nom: "Zones".into(), description: "d".into() });
}

#[test]
fn the_panel_draws_every_kind_of_button_with_and_without_names() {
    let (ctx, mut app) = headless("dessin");
    let idx = app.brushes.save_mine("Ma brosse d'essai", Target::Brush, brosse_perso());
    app.mon_panneau_ajouter_commande("outil.gomme");
    app.mon_panneau_ajouter_commande("fichier.enregistrer");
    app.mon_panneau_ajouter_commande("vue.cadres_securite");
    app.settings.mon_panneau.ajouter(Bouton::Separateur);
    app.mon_panneau_ajouter_brosse(idx);
    app.settings.mon_panneau.ajouter(Bouton::Brosse { nom: "Disparue".into(), source: "Mes brosses".into(), gomme: true });
    app.mon_panneau_enregistrer_outil();
    app.settings.mon_panneau.ajouter(Bouton::Outil(perso("Orphelin", "outil.disparu")));
    app.show_pane(Pane::MonPanneau);
    for noms in [false, true] {
        for largeur in [160.0, 420.0] {
            app.settings.mon_panneau.noms = noms;
            let entree = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(largeur, 500.0))), ..Default::default() };
            let _ = ctx.run(entree.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mon_panneau(ui));
            });
            // Avec la recherche du « + » ouverte, et un nom en cours de saisie.
            app.mon_panneau.recherche = Some("pinceau".into());
            let _ = ctx.run(entree.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mon_panneau(ui));
            });
            app.mon_panneau.recherche = Some("ma brosse".into());
            let _ = ctx.run(entree, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.mon_panneau(ui));
            });
            app.mon_panneau.recherche = None;
        }
    }
    // Un panneau vide se dessine aussi (la consigne).
    app.settings.mon_panneau = Reglage::default();
    let _ = ctx.run(RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(300.0, 400.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.mon_panneau(ui));
    });
}

// ----------------------------------------------------------------------------------------------------- la grille

#[test]
fn the_grid_wraps_and_a_separator_never_starts_a_row() {
    let r = |x: f32, y: f32, w: f32| Rect::from_min_size(pos2(x, y), vec2(w, 34.0));
    // Cinq cases de 34 sur 110 points : trois par ligne.
    let (cases, h) = disposer(&[34.0; 5], &[false; 5], 34.0, 3.0, 110.0);
    assert_eq!(cases[0], r(0.0, 0.0, 34.0));
    assert_eq!(cases[2], r(74.0, 0.0, 34.0));
    assert_eq!(cases[3], r(0.0, 37.0, 34.0));
    assert_eq!(h, 71.0);
    // Une seule ligne.
    let (_, h) = disposer(&[34.0; 3], &[false; 3], 34.0, 3.0, 400.0);
    assert_eq!(h, 34.0);
    // Rien : pas de hauteur.
    assert_eq!(disposer(&[], &[], 34.0, 3.0, 100.0).1, 0.0);
    // Un séparateur qui tient reste sur la ligne ; celui qui tomberait en fin de ligne disparaît et la case suivante passe à la ligne.
    let (cases, _) = disposer(&[34.0, 10.0, 34.0], &[false, true, false], 34.0, 3.0, 100.0);
    assert_eq!(cases[1].width(), 10.0);
    assert_eq!(cases[2].min.x, 47.0);
    let (cases, h) = disposer(&[34.0, 34.0, 10.0, 34.0], &[false, false, true, false], 34.0, 3.0, 71.0);
    assert_eq!(cases[2].width(), 0.0, "sans place, le séparateur ne se voit pas");
    assert_eq!(cases[3], r(0.0, 37.0, 34.0), "et la case suivante commence la ligne");
    assert_eq!(h, 71.0);
    // Un séparateur en tête de grille n'a pas de largeur.
    let (cases, _) = disposer(&[10.0, 34.0], &[true, false], 34.0, 3.0, 100.0);
    assert_eq!(cases[0].width(), 0.0);
    assert_eq!(cases[1].min.x, 0.0);
}

#[test]
fn the_drop_target_is_the_cell_under_the_pointer_or_the_nearest_one() {
    let (cases, _) = disposer(&[34.0, 10.0, 34.0, 34.0], &[false, true, false, false], 34.0, 3.0, 400.0);
    // Sur une case.
    assert_eq!(case_la_plus_proche(&cases, pos2(10.0, 10.0)), Some(0));
    assert_eq!(case_la_plus_proche(&cases, pos2(cases[2].center().x, 5.0)), Some(2));
    // Dans l'écart, hors de la grille : la plus proche.
    assert_eq!(case_la_plus_proche(&cases, pos2(500.0, 500.0)), Some(3));
    assert_eq!(case_la_plus_proche(&cases, pos2(-50.0, -50.0)), Some(0));
    // Une case sans largeur (séparateur caché) n'est jamais la cible.
    let (cases, _) = disposer(&[10.0, 34.0], &[true, false], 34.0, 3.0, 100.0);
    assert_eq!(case_la_plus_proche(&cases, pos2(0.0, 5.0)), Some(1));
    assert_eq!(case_la_plus_proche(&[], pos2(0.0, 0.0)), None);
}
