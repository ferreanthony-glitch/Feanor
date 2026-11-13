//! Premier contact (partie 89) : la visite « Premier jour » à la place de la pluie de bulles, le Labo caché d'origine, le
//! vocabulaire (Fiche du plan, Fond de la feuille, Vectoriel), les anciens noms toujours trouvables par Ctrl+K.

use eframe::egui::{epaint::Shape, Event, Modifiers, PointerButton, Pos2};

use crate::conseils::Conseil;
use crate::document::{Document, LayerKind};
use crate::palette::{palette_commands, rank};
use crate::perf_tests::{banc_taille, Banc};
use crate::premierjour::{Etat, Reglage, ETAPES, PROPOSITIONS_MAX};
use crate::settings::Settings;
use crate::workspace::Espace;

fn banc() -> Banc {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1600, 900);
    b.garder_formes = true;
    // Comme l'application lancée par l'utilisateur, sur une machine neuve.
    b.app.confort.conseils_actifs = true;
    b.app.confort.conseils_max_seance = 1;
    b.app.confort.visite_offre = true;
    b.app.settings.premier_jour = Reglage::default();
    b.chauffer(6);
    b
}

fn textes(shape: &Shape, out: &mut Vec<(String, Pos2)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.pos + t.galley.size() / 2.0)),
        Shape::Vec(v) => v.iter().for_each(|s| textes(s, out)),
        _ => {}
    }
}

fn tous_les_textes(b: &Banc) -> Vec<(String, Pos2)> {
    let mut out = Vec::new();
    for s in &b.formes {
        textes(&s.shape, &mut out);
    }
    out
}

fn est_la(b: &Banc, texte: &str) -> bool {
    tous_les_textes(b).iter().any(|(t, _)| t.contains(texte))
}

fn place(b: &Banc, texte: &str) -> Pos2 {
    tous_les_textes(b).into_iter().find(|(t, _)| t.contains(texte)).unwrap_or_else(|| panic!("« {texte} » n'est pas à l'écran")).1
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.chauffer(2);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(3);
}

fn bulles(b: &Banc) -> usize {
    tous_les_textes(b).iter().filter(|(t, _)| t == "Compris").count()
}

fn disque() -> String {
    std::fs::read_to_string(crate::donnees::roaming().unwrap().join("reglages.toml")).unwrap_or_default()
}

// ------------------------------------------------------------------------------------------------------ les règles

#[test]
fn the_offer_is_for_a_new_machine_and_later_makes_at_most_two_proposals() {
    let mut r = Reglage::default();
    assert!(r.est_vide());
    assert!(r.a_proposer(true) && !r.a_proposer(false), "machine neuve seulement");
    r.etat = Etat::PlusTard;
    r.propositions = 1;
    assert!(r.a_proposer(false), "« Plus tard » revient, même si la machine n'est plus neuve");
    r.propositions = PROPOSITIONS_MAX;
    assert!(!r.a_proposer(true), "mais pas plus de {PROPOSITIONS_MAX} propositions en tout");
    for etat in [Etat::NePlusProposer, Etat::EnCours, Etat::Fini] {
        r.etat = etat;
        r.propositions = 0;
        assert!(!r.a_proposer(true), "{etat:?}");
    }
}

#[test]
fn the_choice_is_kept_in_the_settings_file_and_nothing_is_written_by_default() {
    assert!(!toml::to_string(&Settings::default()).unwrap().contains("premier_jour"));
    assert!(!toml::to_string(&Settings::default()).unwrap().contains("fonctions_experimentales"));
    let mut s = Settings::default();
    s.premier_jour = Reglage { etat: Etat::EnCours, etape: 2, propositions: 1 };
    s.experimental = true;
    let ecrit = toml::to_string(&s).unwrap();
    assert!(ecrit.contains("[premier_jour]") && ecrit.contains("en_cours") && ecrit.contains("fonctions_experimentales = true"), "{ecrit}");
    let relu: Settings = toml::from_str(&ecrit).unwrap();
    assert_eq!(relu.premier_jour, s.premier_jour);
    assert!(relu.experimental);
    // Des réglages d'avant : rien de retenu, fonctions expérimentales cachées.
    let ancien: Settings = toml::from_str("").unwrap();
    assert!(ancien.premier_jour.est_vide() && !ancien.experimental);
}

#[test]
fn there_are_five_steps_in_the_order_of_the_work_and_every_button_is_a_command() {
    assert_eq!(ETAPES.len(), 5);
    let titres: Vec<&str> = ETAPES.iter().map(|e| e.titre).collect();
    assert_eq!(titres, ["Créer un projet", "Dessiner des vignettes", "Mettre une durée", "Écouter l'animatique", "Exporter"]);
    for e in &ETAPES {
        assert!(crate::keymap::action_info(e.action).is_some_and(|a| a.implemented), "{} n'est pas une commande", e.action);
        assert!(e.texte.ends_with('.'));
        for interdit in ["Storyboard Pro", "TVPaint", "Photoshop", "Procreate", "EbSynth"] {
            assert!(!e.texte.contains(interdit) && !e.titre.contains(interdit));
        }
    }
}

// ------------------------------------------------------------------------------------------------------ l'écran

#[test]
fn at_the_first_launch_there_is_one_proposal_and_no_pile_of_bubbles() {
    let mut b = banc();
    // Un seul message, discret, avec ses trois réponses ; ni « Régler SHOT… ? », ni conseil.
    assert!(est_la(&b, "Première fois sur FEANOR ?"));
    let n = tous_les_textes(&b).iter().filter(|(t, _)| t.contains("Première fois sur FEANOR ?")).count();
    assert_eq!(n, 1, "une seule proposition");
    for r in ["Visite rapide (2 min)", "Plus tard", "Ne plus proposer"] {
        assert!(est_la(&b, r), "{r}");
    }
    assert!(!est_la(&b, "Régler FEANOR pour cette machine ?"));
    assert_eq!(bulles(&b), 0);
    // On change d'endroit : les conseils attendent que la proposition ait sa réponse.
    for espace in [Espace::Montage, Espace::Vignettes, Espace::Animation] {
        b.app.set_espace(espace);
        b.chauffer(4);
        assert_eq!(bulles(&b), 0, "{espace:?} : aucun conseil par-dessus la proposition");
        assert_eq!(b.app.confort.conseil, None);
    }
    assert!(b.app.settings.conseils.est_vide(), "aucun conseil n'est noté vu");
    // Le diagnostic des performances ne se propose plus au lancement.
    assert!(!b.app.diag.offre);
}

#[test]
fn ne_plus_proposer_is_remembered_and_the_tips_then_come_one_per_session() {
    let mut b = banc();
    let p = place(&b, "Ne plus proposer");
    clic(&mut b, p);
    assert_eq!(b.app.settings.premier_jour.etat, Etat::NePlusProposer);
    assert!(!b.app.confort.visite_offre);
    assert!(!est_la(&b, "Première fois sur FEANOR ?"));
    assert!(disque().contains("ne_plus_proposer"), "{}", disque());
    // Sur une machine neuve, une séance suivante ne repropose rien.
    assert!(!b.app.settings.premier_jour.a_proposer(true));
    // Les conseils reprennent la main, au compte-gouttes : un seul par séance.
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Montage));
    assert_eq!(bulles(&b), 1);
    let compris = place(&b, "Compris");
    clic(&mut b, compris);
    b.app.set_espace(Espace::Vignettes);
    b.chauffer(5);
    assert_eq!(b.app.confort.conseil, None, "un seul conseil dans la séance");
    assert!(!b.app.settings.conseils.a_vu(Conseil::Vignettes));
    // « Revoir les conseils » (Aide) les rend, et le conseil de l'endroit revient tout de suite.
    b.app.run_action("aide.conseils");
    b.chauffer(4);
    assert_eq!(b.app.confort.conseil, Some(Conseil::Vignettes));
}

#[test]
fn plus_tard_counts_and_the_tour_can_always_be_started_from_aide() {
    let mut b = banc();
    let p = place(&b, "Plus tard");
    clic(&mut b, p);
    assert_eq!((b.app.settings.premier_jour.etat, b.app.settings.premier_jour.propositions), (Etat::PlusTard, 1));
    assert!(b.app.settings.premier_jour.a_proposer(false), "elle revient au prochain lancement");
    assert!(!est_la(&b, "Première fois sur FEANOR ?"));
    // Aide > Visite rapide : le parcours part, la carte de l'étape se montre (un projet est là : l'étape 1 est faite).
    assert!(crate::keymap::action_info("aide.visite").is_some_and(|a| a.implemented));
    b.app.run_action("aide.visite");
    b.chauffer(4);
    assert_eq!(b.app.settings.premier_jour.etat, Etat::EnCours);
    assert_eq!(b.app.settings.premier_jour.etape, 1);
    assert!(est_la(&b, "Visite rapide · étape 2 sur 5 : Dessiner des vignettes"));
    assert!(est_la(&b, "Ouvrir les Vignettes"));
    // Le bouton de l'étape ouvre l'endroit.
    let ouvrir = place(&b, "Ouvrir les Vignettes");
    clic(&mut b, ouvrir);
    assert_eq!(b.app.espace, Espace::Vignettes);
    // Suivant, suivant, suivant : les étapes défilent, la dernière propose le diagnostic.
    for attendu in ["étape 3 sur 5 : Mettre une durée", "étape 4 sur 5 : Écouter l'animatique", "étape 5 sur 5 : Exporter"] {
        let suivant = place(&b, "Suivant");
        clic(&mut b, suivant);
        assert!(est_la(&b, attendu), "{attendu}");
    }
    assert!(est_la(&b, "Régler FEANOR pour cette machine…"));
    assert!(disque().contains("en_cours"));
    let fin = place(&b, "Terminer");
    clic(&mut b, fin);
    assert_eq!(b.app.settings.premier_jour.etat, Etat::Fini);
    assert!(!est_la(&b, "Visite rapide ·"));
    // Elle se relance quand on veut.
    b.app.run_action("aide.visite");
    assert_eq!(b.app.settings.premier_jour.etat, Etat::EnCours);
}

#[test]
fn the_card_never_sits_above_a_window_or_a_dialog() {
    let mut b = banc();
    b.app.run_action("aide.visite");
    b.chauffer(4);
    assert!(est_la(&b, "Visite rapide · étape"));
    b.app.pending = Some(crate::app::Pending::Close);
    b.chauffer(3);
    assert!(!est_la(&b, "Visite rapide · étape"), "une boîte de dialogue tient la main");
    b.app.pending = None;
    b.chauffer(3);
    assert!(est_la(&b, "Visite rapide · étape"), "elle revient après");
    // Une fenêtre (Préférences, palette de commandes…) : même chose, et les conseils attendent aussi.
    b.app.prefs_window = true;
    b.chauffer(4);
    assert!(!est_la(&b, "Visite rapide · étape"), "Préférences ouvertes : la carte s'efface");
    assert!(b.app.fenetre_ouverte());
    b.app.prefs_window = false;
    b.chauffer(4);
    assert!(est_la(&b, "Visite rapide · étape"));
    // « Quitter la visite » : plus rien d'office.
    let q = place(&b, "Quitter la visite");
    clic(&mut b, q);
    assert_eq!(b.app.settings.premier_jour.etat, Etat::Fini);
}

#[test]
fn nothing_is_proposed_in_the_tests_or_when_the_app_is_not_the_users() {
    let mut b = banc_taille(Document::with_frame(64, 48, 24), false, 1200, 800);
    assert!(!b.app.confort.visite_offre && !b.app.confort.conseils_actifs);
    b.chauffer(6);
    assert!(!est_la(&b, "Première fois sur FEANOR ?"));
    assert!(b.app.settings.premier_jour.est_vide());
}

// ------------------------------------------------------------------------------------------------------ le Labo

#[test]
fn the_labo_menu_is_hidden_until_the_experimental_box_is_ticked() {
    let mut b = banc();
    b.app.confort.visite_offre = false;
    b.chauffer(3);
    assert!(!b.app.settings.experimental);
    assert!(!tous_les_textes(&b).iter().any(|(t, _)| t == "Labo"), "plus de menu Labo d'origine");
    assert!(tous_les_textes(&b).iter().any(|(t, _)| t == "Aide"));
    b.app.settings.experimental = true;
    b.chauffer(3);
    assert!(tous_les_textes(&b).iter().any(|(t, _)| t == "Labo"), "avec la case, le menu Labo revient");
}

#[test]
fn the_experimental_panels_are_not_offered_by_default_but_every_command_stays_in_ctrl_k() {
    let commandes = palette_commands();
    for id in ["vue.suggestions_sonores", "suggestion.lire", "vue.mon_panneau", "pinceau.enregistrer_traits", "calque.propager", "aide.visite", "aide.diagnostic"] {
        assert!(commandes.iter().any(|(i, _)| *i == id), "{id} n'est plus dans la palette");
    }
    // Chaque commande se retrouve en tapant son nom entier.
    for (id, nom) in &commandes {
        let trouves = rank(&commandes, nom);
        assert!(trouves.iter().any(|&i| commandes[i].0 == *id), "« {nom} » ne trouve pas {id}");
    }
    // Les suggestions sonores ne sont plus dans la disposition d'origine du montage, ni dans Affichage > Panneaux (sans la case).
    assert!(!Espace::Montage.defaut().dock_layout().contains(crate::dock::Pane::SuggestionsSonores));
}

#[test]
fn the_old_names_are_synonyms_in_ctrl_k() {
    let commandes = palette_commands();
    let trouve = |q: &str, id: &str| {
        let r = rank(&commandes, q);
        assert!(r.iter().any(|&i| commandes[i].0 == id), "« {q} » ne trouve pas {id}");
    };
    trouve("Propriétés du plan", "panneau.proprietes");
    trouve("proprietes plan", "panneau.proprietes");
    trouve("Fiche du plan", "panneau.proprietes");
    trouve("Diagnostic des performances", "aide.diagnostic");
    trouve("régler SHOT pour cette machine", "aide.diagnostic");
    trouve("Arrière-plan aucun", "fond.aucun");
    trouve("arrière-plan damier", "fond.damier");
    trouve("Onglet Arrière-plan", "onglet.arriere_plan");
    trouve("Fond de la feuille damier", "fond.damier");
    trouve("Fond de la feuille", "onglet.arriere_plan");
    trouve("Labo", "vue.mon_panneau");
    trouve("Labo", "vue.suggestions_sonores");
    trouve("Labo", "pinceau.enregistrer_traits");
    trouve("Mon panneau (essai)", "vue.mon_panneau");
    trouve("Propager la peinture", "calque.propager");
    trouve("visite guidée", "aide.visite");
    // Chaque ancien nom pointe vers une commande qui existe.
    for (id, _) in crate::palette::ANCIENS_NOMS {
        assert!(crate::keymap::action_info(id).is_some_and(|a| a.implemented), "{id}");
    }
}

// ------------------------------------------------------------------------------------------------------ le vocabulaire

#[test]
fn the_plan_sheet_has_one_name_and_the_sheet_background_another_than_the_colour_pair() {
    for a in crate::keymap::ACTIONS {
        assert!(!a.label.contains("Propriétés du plan"), "{} : « Fiche du plan » partout", a.id);
        // « Arrière-plan » ne désigne plus que le second carré du couple de couleurs, jamais la feuille.
        if a.label.contains("Arrière-plan") {
            panic!("{} : « Papier » pour la feuille, « couleur d'arrière-plan » pour le couple de couleurs : {}", a.id, a.label);
        }
    }
    assert_eq!(crate::keymap::action_info("panneau.proprietes").unwrap().label.split(' ').take(3).collect::<Vec<_>>(), ["Fiche", "du", "plan"]);
    assert!(crate::keymap::action_info("fond.damier").unwrap().label.starts_with("Papier"));
    assert!(crate::keymap::action_info("pastille.arriere_plan").unwrap().label.contains("arrière-plan"), "le sens « couleur » reste clair");
    // L'onglet de la timeline.
    assert!(crate::keymap::action_info("onglet.arriere_plan").unwrap().label.contains("Papier"));
}

#[test]
fn new_vector_layers_are_called_vectoriel_and_existing_layers_keep_their_names() {
    let mut b = banc();
    let avant: Vec<String> = b.app.layers().iter().map(|l| l.name.clone()).collect();
    b.app.add_layer(LayerKind::Vector);
    let nom = b.app.layer().name.clone();
    assert!(nom.starts_with("Vectoriel "), "{nom}");
    b.app.add_layer(LayerKind::Bitmap);
    assert!(b.app.layer().name.starts_with("Calque "));
    // Les calques qui étaient là gardent leur nom (un projet ancien n'est pas renommé).
    for n in &avant {
        assert!(b.app.layers().iter().any(|l| &l.name == n), "« {n} » a disparu");
    }
    b.app.doc.panels[0].layers[0].name = "Vecto 3".into();
    b.app.add_layer(LayerKind::Bitmap);
    assert!(b.app.layers().iter().any(|l| l.name == "Vecto 3"), "un ancien nom n'est jamais réécrit");
}

#[test]
fn the_board_panel_no_longer_contradicts_itself() {
    let mut b = banc();
    b.app.confort.visite_offre = false;
    b.app.set_espace(Espace::Vignettes);
    b.app.show_pane(crate::dock::Pane::Vignettes);
    b.chauffer(8);
    assert!(est_la(&b, "Calques d'annotation"));
    assert!(est_la(&b, "Dessin de la case"));
    // (« Aucune annotation pour l'instant : le premier trait d'annotation crée son calque » est dans le registre des explications,
    // avec l'animation « Annotations », partie 153.)
    assert!(!est_la(&b, "Aucune annotation pour l'instant"));
    assert!(crate::aideanim::anim("vignettes.annotations").is_some_and(|a| a.texte.contains("Aucune annotation pour l'instant")));
    assert!(!est_la(&b, "Aucun calque d'annotation"));
}

#[test]
fn the_diagnostic_window_speaks_to_the_user_with_tu_like_the_rest_of_the_app() {
    for fichier in [include_str!("diagnosticui.rs"), include_str!("diagnostic.rs")] {
        for (n, ligne) in fichier.lines().enumerate() {
            // Les chaînes de l'écran seulement (les commentaires du code peuvent parler d'« un utilisateur »).
            if ligne.trim_start().starts_with("//") {
                continue;
            }
            for vous in ["votre ", "Votre ", "vos ", "décochez", "Cochez", "Cliquez"] {
                assert!(!ligne.contains(vous), "ligne {} : « {vous} » : le tutoiement domine", n + 1);
            }
        }
    }
}
