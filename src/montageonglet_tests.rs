//! Le troisième onglet « Montage » (en haut à droite, à côté de Storyboard et d'Animation) : les trois
//! onglets passent d'un espace de travail à l'autre sans rien changer au projet, la disposition de
//! l'espace Montage (le film en haut, toutes les pistes en bas), sa barre, Tab, Alt+M, la disposition
//! retenue, et le repos de l'application.

use eframe::egui::{self, epaint::Shape, Event, Key, Modifiers, PointerButton, Pos2};

use crate::app::{App, Mode};
use crate::barres::{dans_la_barre, dans_plus, Barre, Genre, ReglageBarre};
use crate::document::Document;
use crate::dock::Pane;
use crate::perf_tests::{banc_taille, Banc};
use crate::settings::Settings;
use crate::workspace::{self, Espace, Layout};

fn film() -> Document {
    let mut doc = Document::with_frame(64, 48, 24);
    for _ in 0..3 {
        let last = doc.panels.len() - 1;
        doc.add_scene_after(last);
    }
    doc
}

fn banc() -> Banc {
    let mut b = banc_taille(film(), false, 1600, 900);
    b.garder_textes = true;
    b.garder_formes = true;
    b.chauffer(6);
    b
}

fn has(app: &App, pane: Pane) -> bool {
    app.dock_layout().contains(pane)
}

/// Le centre d'un texte peint par la dernière image (un onglet du menu, par exemple).
fn centre_du_texte(b: &Banc, texte: &str) -> Pos2 {
    for clipped in &b.formes {
        if let Shape::Text(t) = &clipped.shape {
            if t.galley.job.text == texte {
                return t.pos + t.galley.size() / 2.0;
            }
        }
    }
    panic!("le texte « {texte} » n'est pas à l'écran : {:?}", b.textes);
}

fn clic(b: &mut Banc, p: Pos2) {
    b.image(vec![Event::PointerMoved(p)], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.image(vec![Event::PointerGone], 1.0 / 60.0);
    b.chauffer(2);
}

fn touche(b: &mut Banc, key: Key, modifiers: Modifiers) {
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }], 1.0 / 60.0);
    b.image(vec![Event::Key { key, physical_key: Some(key), pressed: false, repeat: false, modifiers }], 1.0 / 60.0);
    b.chauffer(2);
}

// ------------------------------------------------------------------------------ les trois onglets

#[test]
fn the_three_tabs_are_drawn_in_the_work_order_and_switch_spaces_by_clicks() {
    let mut b = banc();
    // Dans l'ordre du travail, de gauche à droite : Storyboard, Animation, Montage.
    let (s, a, m) = (centre_du_texte(&b, " Storyboard "), centre_du_texte(&b, " Animation "), centre_du_texte(&b, " Montage "));
    assert!(s.x < a.x && a.x < m.x, "Storyboard, Animation, Montage : {s:?} {a:?} {m:?}");
    assert!(s.y < 40.0 && a.y < 40.0 && m.y < 40.0, "dans la barre de menus, en haut");
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard));
    clic(&mut b, m);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Montage, Mode::Storyboard), "Montage : l'espace Montage, côté storyboard");
    let (s, a, m) = (centre_du_texte(&b, " Storyboard "), centre_du_texte(&b, " Animation "), centre_du_texte(&b, " Montage "));
    clic(&mut b, a);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation));
    let (s2, m2) = (centre_du_texte(&b, " Storyboard "), centre_du_texte(&b, " Montage "));
    assert_eq!((s.x, m.x), (s2.x, m2.x), "les onglets ne bougent pas d'un espace à l'autre");
    clic(&mut b, m2);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Montage, Mode::Storyboard), "d'Animation à Montage");
    clic(&mut b, s2);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard));
}

#[test]
fn switching_tabs_keeps_the_playhead_the_selection_and_the_project() {
    let mut b = banc();
    b.app.select_panel(2);
    b.app.cur_frame = 0;
    b.app.board_sel = vec![b.app.doc.panels[1].id];
    let (panel, frame, global, layer) = (b.app.cur_panel, b.app.cur_frame, b.app.global_frame(), b.app.cur_layer);
    let (ids, total, selection): (Vec<u64>, usize, Vec<u64>) =
        (b.app.doc.panels.iter().map(|p| p.id).collect(), b.app.doc.total_frames(), b.app.board_selection());
    let undo = b.app.history.names().0.len();
    for espace in [Espace::Montage, Espace::Animation, Espace::Montage, Espace::Storyboard, Espace::Animation, Espace::Storyboard, Espace::Montage] {
        b.app.set_espace(espace);
        b.chauffer(2);
        assert_eq!((b.app.cur_panel, b.app.cur_layer), (panel, layer), "{espace:?} : le plan courant");
        assert_eq!(b.app.global_frame(), global, "{espace:?} : la tête de lecture");
        assert_eq!(b.app.cur_frame, frame);
        assert_eq!(b.app.board_selection(), selection, "{espace:?} : les plans choisis");
        assert_eq!(b.app.doc.panels.iter().map(|p| p.id).collect::<Vec<_>>(), ids);
        assert_eq!(b.app.doc.total_frames(), total);
        assert_eq!(b.app.history.names().0.len(), undo, "aucune étape d'annulation : rien n'a changé dans le projet");
        assert!(!b.app.playing);
    }
}

#[test]
fn only_the_selected_tab_is_lit() {
    // Le texte allumé prend l'encre faite pour l'accent (`bulles::encre_etat`), les autres gardent la leur.
    let mut b = banc();
    let couleur = |b: &Banc, texte: &str| -> egui::Color32 {
        for clipped in &b.formes {
            if let Shape::Text(t) = &clipped.shape {
                if t.galley.job.text == texte {
                    return t.galley.job.sections[0].format.color;
                }
            }
        }
        panic!("{texte}");
    };
    let sur_accent = crate::theme::sur_accent();
    for (espace, allume) in [(Espace::Storyboard, " Storyboard "), (Espace::Animation, " Animation "), (Espace::Montage, " Montage ")] {
        b.app.set_espace(espace);
        b.chauffer(3);
        for texte in [" Storyboard ", " Animation ", " Montage "] {
            assert_eq!(couleur(&b, texte) == sur_accent, texte == allume, "{} allumé, {texte}", espace.nom());
        }
    }
}

// ------------------------------------------------------------------------------ Tab et Alt+M

#[test]
fn alt_m_goes_to_the_montage_and_no_other_shortcut_uses_it() {
    let b = banc();
    let alt = Modifiers { alt: true, ..Modifiers::NONE };
    for mode in [Mode::Storyboard, Mode::Animation] {
        assert_eq!(b.app.keymap.find(Key::M, Some(Key::M), alt, mode), Some("espace.montage"));
    }
    assert!(b.app.keymap.warnings.iter().all(|w| !w.contains("Alt+M") && !w.contains("espace.montage")), "{:?}", b.app.keymap.warnings);
    assert_eq!(b.app.keymap.label("espace.montage"), "Alt+M");
    // « M » seul reste le miroir.
    assert_eq!(b.app.keymap.find(Key::M, Some(Key::M), Modifiers::NONE, Mode::Storyboard), Some("vue.miroir"));
}

#[test]
fn the_keys_reach_the_montage_and_tab_leaves_it_for_the_storyboard() {
    let mut b = banc();
    // (Comme la vraie fenêtre : Tab est retiré à egui avant qu'il ne passe d'un champ à l'autre.)
    b.crochet = true;
    let alt = Modifiers { alt: true, ..Modifiers::NONE };
    touche(&mut b, Key::M, alt);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Montage, Mode::Storyboard), "Alt+M");
    // Tab, depuis le Montage : le storyboard.
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard), "Tab revient au storyboard");
    // Tab depuis le storyboard : l'animation ; Tab encore : le storyboard.
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation));
    touche(&mut b, Key::Tab, Modifiers::NONE);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Storyboard, Mode::Storyboard));
    // Alt+M depuis l'animation.
    touche(&mut b, Key::Tab, Modifiers::NONE);
    touche(&mut b, Key::M, alt);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Montage, Mode::Storyboard));
}

#[test]
fn animating_a_plan_from_the_montage_and_coming_back_returns_to_the_montage() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    let c = Barre::MontageEdition.cherche("mode.basculer").unwrap();
    assert_eq!(b.app.vue_de(Barre::MontageEdition, c).nom, "Animer ce plan");
    b.app.executer_commande(Barre::MontageEdition, c);
    assert_eq!((b.app.espace, b.app.mode), (Espace::Animation, Mode::Animation));
    // Tab revient du côté storyboard où l'on était : le Montage.
    b.app.run_action("mode.basculer");
    assert_eq!((b.app.espace, b.app.mode), (Espace::Montage, Mode::Storyboard));
}

// ------------------------------------------------------------------------------ la disposition

#[test]
fn the_montage_layout_is_made_for_editing() {
    let layout = Espace::Montage.defaut();
    let dock = layout.dock_layout();
    // Le lecteur et la bande, la fiche du plan, et les panneaux du travail de montage.
    for p in [Pane::Canvas, Pane::Montage, Pane::Sheet, Pane::Bruitages, Pane::SuggestionsSonores, Pane::Comments, Pane::History] {
        assert!(dock.contains(p), "{p:?} est dans l'espace Montage");
    }
    // Pas de panneaux de dessin, ni de feuille d'exposition, ni de références (à la demande).
    for p in [Pane::Tools, Pane::Color, Pane::Brushes, Pane::ToolOptions, Pane::Guides, Pane::Layers, Pane::Effects, Pane::XSheet, Pane::References] {
        assert!(!dock.contains(p), "{p:?} n'est pas dans l'espace Montage");
    }
    assert!(!layout.tool_bar, "pas de barre d'outils verticale");
    assert!(layout.command_bar);
    // Le lecteur en haut, la bande en bas, sur toute la largeur, plus haute que celle du storyboard et de l'animation.
    match &dock.principal {
        crate::dock::DockNode::Column { part, haut, bas } => {
            assert_eq!(bas.as_ref(), &crate::dock::DockNode::Tabs { panneaux: vec![Pane::Montage], actif: 0 });
            assert!(matches!(haut.as_ref(), crate::dock::DockNode::Row { gauche, .. } if matches!(gauche.as_ref(), crate::dock::DockNode::Tabs { panneaux, .. } if panneaux == &vec![Pane::Canvas])));
            let bande = (1.0 - part) * 820.0;
            assert!(bande > 400.0, "la bande fait {bande} points sur 820");
            assert!(*part > 0.3, "et le lecteur garde de la place : {part}");
        }
        autre => panic!("le lecteur au-dessus de la bande : {autre:?}"),
    }
    assert!(layout.montage_height > Espace::Animation.defaut().montage_height);
    assert!(layout.montage_height > Espace::Storyboard.defaut().montage_height);
    // Les deux autres espaces n'ont pas bougé.
    assert_eq!(Espace::Storyboard.defaut(), Layout::default());
    assert!(!Espace::Storyboard.defaut().dock_layout().contains(Pane::Comments));
    assert!(!Espace::Animation.defaut().dock_layout().contains(Pane::Bruitages));
}

#[test]
fn entering_the_montage_shows_its_panes_and_the_camera_and_leaving_gives_the_stage_back() {
    let mut b = banc();
    assert!(!b.app.camera_view);
    assert!(has(&b.app, Pane::Layers) && has(&b.app, Pane::Color) && b.app.layout.tool_bar);
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    for p in [Pane::Canvas, Pane::Montage, Pane::Sheet, Pane::Bruitages, Pane::Comments, Pane::History] {
        assert!(has(&b.app, p), "{p:?}");
    }
    assert!(!has(&b.app, Pane::Layers) && !has(&b.app, Pane::Color) && !has(&b.app, Pane::Brushes) && !has(&b.app, Pane::ToolOptions));
    assert!(!b.app.layout.tool_bar, "la barre d'outils verticale est repliée");
    assert!(b.app.camera_view, "le lecteur montre le film à travers la caméra");
    assert_eq!(b.app.pane_title(Pane::Canvas), "Lecteur");
    assert_eq!(b.app.pane_title(Pane::Montage), "Montage");
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(2);
    assert!(!b.app.camera_view, "la vue du plateau revient, puisque c'est l'entrée dans le Montage qui l'avait changée");
    assert!(b.app.layout.tool_bar && has(&b.app, Pane::Layers));
    assert_eq!(b.app.pane_title(Pane::Canvas), "Dessin");
    // Une vue caméra choisie avant d'entrer dans le Montage reste telle quelle en en sortant.
    b.app.set_camera_view(true);
    b.app.set_espace(Espace::Montage);
    b.app.set_espace(Espace::Storyboard);
    assert!(b.app.camera_view);
}

#[test]
fn every_panel_of_the_panel_bar_opens_in_the_montage_space() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    for e in crate::barrepanneaux::GROUPES.iter().flat_map(|g| g.iter()).filter(|e| e.visible(false)) {
        b.app.show_pane(e.pane);
        assert!(has(&b.app, e.pane), "{:?}", e.pane);
    }
}

#[test]
fn the_montage_layout_is_remembered_restored_and_reset() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    // L'utilisateur ferme l'historique et le met à sa main.
    assert!(has(&b.app, Pane::History));
    b.app.toggle_pane(Pane::History);
    assert!(!has(&b.app, Pane::History));
    b.app.set_espace(Espace::Storyboard);
    assert!(!has(&b.app, Pane::Comments), "le storyboard n'a pas les commentaires");
    b.app.set_espace(Espace::Animation);
    b.app.set_espace(Espace::Montage);
    assert!(!has(&b.app, Pane::History), "l'espace Montage retrouve ce qu'on y avait fait");
    assert!(has(&b.app, Pane::Comments));
    // Écrit dans les réglages, pour une autre séance.
    b.app.set_espace(Espace::Storyboard);
    b.app.remember_layout(&b.ctx);
    let file = Settings::load();
    let retenue = file.layout_of(Espace::Montage).expect("la disposition du Montage est retenue");
    assert!(!retenue.dock_layout().contains(Pane::History) && retenue.dock_layout().contains(Pane::Comments));
    assert!(!retenue.tool_bar);
    // « Rétablir la disposition d'origine » : l'historique revient, seul l'espace en cours est touché.
    b.app.set_espace(Espace::Montage);
    b.app.run_action("espace.retablir");
    assert!(has(&b.app, Pane::History) && has(&b.app, Pane::Bruitages));
    assert_eq!(b.app.layout.montage_height, Espace::Montage.defaut().montage_height);
    assert_eq!(b.app.espace, Espace::Montage);
    // Enregistrer la disposition comme espace de travail nommé garde celle du Montage.
    let mut b2 = banc();
    b2.app.set_espace(Espace::Montage);
    let courante = b2.app.current_layout();
    workspace::save_workspace(&mut b2.app.settings.workspaces, "Mon montage", courante.clone());
    b2.app.set_espace(Espace::Animation);
    let w = b2.app.settings.workspaces.iter().find(|w| w.name == "Mon montage").unwrap().layout.clone();
    b2.app.apply_layout(w);
    assert!(has(&b2.app, Pane::Comments) && !has(&b2.app, Pane::Layers));
}

#[test]
fn a_session_closed_in_the_montage_starts_in_the_montage() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    b.app.toggle_pane(Pane::History);
    b.app.remember_layout(&b.ctx);
    let ctx = egui::Context::default();
    let app2 = App::sans_fenetre(&ctx, None);
    assert_eq!((app2.espace, app2.mode), (Espace::Montage, Mode::Storyboard));
    assert!(has(&app2, Pane::Comments) && !has(&app2, Pane::History));
    assert!(app2.camera_view, "le lecteur est en vue caméra dès l'ouverture");
}

#[test]
fn the_old_montage_layout_gives_way_to_the_new_one_but_a_changed_one_stays() {
    // Retenue sans que l'utilisateur y ait touché : remplacée à la lecture.
    let mut b = banc();
    let mut s = Settings::load();
    s.montage_layout = Some(workspace_d_avant());
    s.save().unwrap();
    assert!(Settings::load().montage_layout.is_none(), "l'ancienne disposition d'origine cède la place");
    // Une disposition faite à la main reste.
    let mut changee = workspace_d_avant();
    let mut dock = changee.dock_layout();
    dock.close(Pane::Bruitages);
    changee.panneaux = Some(dock);
    s.montage_layout = Some(changee.clone());
    s.save().unwrap();
    assert_eq!(Settings::load().montage_layout.map(|l| l.dock_layout().contains(Pane::Bruitages)), Some(false));
    // La nouvelle disposition d'origine, elle, reste telle quelle.
    s.montage_layout = Some(Espace::Montage.defaut());
    s.save().unwrap();
    assert!(Settings::load().montage_layout.is_some());
    b.app.set_espace(Espace::Montage);
}

/// L'ancienne disposition d'origine de l'espace Montage (la bande en grand, la fiche, les sons).
fn workspace_d_avant() -> Layout {
    let base = Layout { tools: false, sheet: true, montage_height: 420.0, ..Layout::default() };
    let mut dock = crate::dock::DockLayout::from_legacy(&Layout { sheet: false, ..base.clone() });
    dock.open(Pane::Sheet);
    dock.open(Pane::Bruitages);
    dock.open(Pane::SuggestionsSonores);
    Layout { panneaux: Some(dock), ..base }
}

// ------------------------------------------------------------------------------ la barre

fn d_office(barre: Barre) -> Vec<&'static str> {
    dans_la_barre(barre, &ReglageBarre::default()).into_iter().filter(|c| c.genre != Genre::Fixe).map(|c| c.id).collect()
}

#[test]
fn the_montage_bar_shows_the_editing_commands_and_keeps_the_others_under_plus() {
    let montage = d_office(Barre::MontageEdition);
    for visible in [
        "montage.debut",
        "lecture.basculer",
        "lecture.boucle",
        "son.lecture",
        "panneau.nouveau",
        "panneau.couper",
        "panneau.fusionner_suivant",
        "montage.debut_a_tete",
        "montage.fin_a_tete",
        "montage.duree",
        "plan.avancer",
        "plan.reculer",
        "montage.aimanter",
        "plans",
        "transition.ajouter",
        "son.importer",
        "son.nouvelle_piste",
        "son.couper",
        "son",
        "vue.bruitages",
        "piste.replier_tout",
        "piste.deplier_tout",
        "commentaire.ajouter",
        "fichier.exporter_relecture",
        "fichier.importer_notes",
        "fichier.exporter_montage",
        "fichier.exporter_fiche",
        "mode.basculer",
    ] {
        assert!(montage.contains(&visible), "{visible} est dans la barre de l'espace Montage");
    }
    // Ces commandes sont sous « Plus… » dans la barre du montage du storyboard : ici elles se voient.
    let storyboard = d_office(Barre::Montage);
    for vue_ici in ["plan.avancer", "plan.reculer", "montage.aimanter", "transition.ajouter", "commentaire.ajouter", "fichier.exporter_montage"] {
        assert!(!storyboard.contains(&vue_ici) && montage.contains(&vue_ici), "{vue_ici}");
    }
    // Aucune commande de la barre du montage n'est perdue : celles qui ne sont pas d'office sont sous « Plus… ».
    let plus: Vec<&str> = dans_plus(Barre::MontageEdition, &ReglageBarre::default()).into_iter().map(|c| c.id).collect();
    for c in Barre::Montage.commandes() {
        assert!(Barre::MontageEdition.cherche(c.id).is_some(), "{} manque dans la barre de l'espace Montage", c.id);
    }
    for sous_plus in ["storyboard.verrouiller", "numerotation.verrouiller", "envoi.marquer", "version.nouvelle", "comparer.ouvrir", "fichier.importer_video", "vue.dessin", "vue.vignettes"] {
        assert!(plus.contains(&sous_plus), "{sous_plus} est sous « Plus… »");
    }
    // Le Montage a sa barre ; les autres espaces gardent la leur.
    assert_eq!(Barre::du_montage_dans(Espace::Montage), Barre::MontageEdition);
    assert_eq!(Barre::du_montage_dans(Espace::Storyboard), Barre::Montage);
    assert_eq!(Barre::du_montage_dans(Espace::Animation), Barre::Montage);
    assert!(Barre::MontageEdition.est_du_montage() && Barre::Montage.est_du_montage() && !Barre::Timeline.est_du_montage());
}

#[test]
fn the_editing_commands_do_their_work() {
    let mut b = banc();
    b.app.set_espace(Espace::Montage);
    // « Couper le plan » : un plan de plus, à la tête de lecture.
    b.app.doc.panels[0].length = 10;
    b.app.select_panel(0);
    b.app.cur_frame = 4;
    let n = b.app.doc.panels.len();
    b.app.executer_commande(Barre::MontageEdition, Barre::MontageEdition.cherche("panneau.couper").unwrap());
    assert_eq!(b.app.doc.panels.len(), n + 1, "{}", b.app.status);
    // « Bruitages » : un interrupteur qui suit le panneau.
    let c = Barre::MontageEdition.cherche("vue.bruitages").unwrap();
    assert_eq!(b.app.vue_de(Barre::MontageEdition, c).etat, Some(true), "le panneau Bruitages est ouvert dans cet espace");
    b.app.executer_commande(Barre::MontageEdition, c);
    assert_eq!(b.app.vue_de(Barre::MontageEdition, c).etat, Some(false));
    // « Nouvelle piste son ».
    let pistes = b.app.doc.audio_tracks.len();
    b.app.executer_commande(Barre::MontageEdition, Barre::MontageEdition.cherche("son.nouvelle_piste").unwrap());
    assert_eq!(b.app.doc.audio_tracks.len(), pistes + 1);
    // Lire : le film entier, comme dans la barre du storyboard.
    let lire = Barre::MontageEdition.cherche("lecture.basculer").unwrap();
    b.app.executer_commande(Barre::MontageEdition, lire);
    assert!(b.app.playing && b.app.play_montage);
    b.app.executer_commande(Barre::MontageEdition, lire);
    assert!(!b.app.playing);
}

#[test]
fn pins_of_the_montage_bar_are_its_own_and_survive_a_round_trip() {
    let mut s = Settings::default();
    s.bars.montage_edition.epingler(Barre::MontageEdition.cherche("storyboard.verrouiller").unwrap(), true);
    s.bars.montage_edition.epingler(Barre::MontageEdition.cherche("montage.duree").unwrap(), false);
    let back: Settings = toml::from_str(&toml::to_string(&s).unwrap()).unwrap();
    assert_eq!(back.bars.montage_edition.epinglees, vec!["storyboard.verrouiller".to_string()]);
    assert_eq!(back.bars.montage_edition.retirees, vec!["montage.duree".to_string()]);
    assert!(back.bars.montage.est_vide() && back.bars.timeline.est_vide(), "les autres barres n'ont pas bougé");
    // Les barres d'origine n'écrivent rien.
    assert!(!toml::to_string(&Settings::default()).unwrap().contains("montage_edition"));
    // Un identifiant de la barre du storyboard qui n'est pas dans celle-ci est nettoyé.
    let sale = crate::barres::ReglageBarres {
        montage_edition: ReglageBarre { epinglees: vec!["nimporte.quoi".into()], retirees: vec!["montage.duree".into(), "montage.duree".into()] },
        ..Default::default()
    };
    let propre = sale.nettoyees();
    assert!(propre.montage_edition.epinglees.is_empty() && propre.montage_edition.retirees == vec!["montage.duree".to_string()]);
}

// ------------------------------------------------------------------------------ vitesse et repos

#[test]
fn switching_between_the_three_tabs_is_instant() {
    let mut b = banc();
    let t = std::time::Instant::now();
    for _ in 0..20 {
        for e in Espace::TOUS {
            b.app.set_espace(e);
        }
    }
    // 60 changements d'espace : aucun rejeu, aucun calcul d'image.
    assert!(t.elapsed().as_millis() < 2000, "{:?}", t.elapsed());
    assert!(b.app.fit_set.is_empty());
}

#[test]
fn the_montage_space_is_idle_at_rest_and_drawing_it_stays_cheap() {
    let mut b = banc();
    // Le même repos que celui du storyboard : le premier saut de temps peut réveiller une fois (un délai qui
    // s'écoule), pas plus.
    let au_repos = |b: &mut Banc| -> Vec<std::time::Duration> {
        b.chauffer(8);
        b.image(Vec::new(), 0.5);
        (0..6).map(|_| b.image(Vec::new(), 0.5).reveil).collect()
    };
    let storyboard = au_repos(&mut b);
    assert!(storyboard.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "le storyboard au repos : {storyboard:?}");
    b.app.set_espace(Espace::Montage);
    let repos = au_repos(&mut b);
    assert!(repos.iter().all(|r| *r >= std::time::Duration::from_secs(1)), "l'espace Montage ne fait pas repeindre au repos : {repos:?}");
    // Le texte de la barre d'état dit où l'on est.
    b.chauffer(2);
    assert!(b.textes.iter().any(|t| t.starts_with("Montage · ")), "{:?}", b.textes);
    // Une image d'écran de l'espace Montage reste une affaire de quelques millisecondes (sans carte graphique).
    let ms: Vec<f64> = (0..5).map(|_| b.image(Vec::new(), 1.0 / 60.0).construction).collect();
    let mediane = {
        let mut m = ms.clone();
        m.sort_by(|a, b| a.partial_cmp(b).unwrap());
        m[2]
    };
    assert!(mediane < 200.0, "une image du Montage : {ms:?} ms");
}
