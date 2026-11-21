//! Essais de la colonne en pile (`colonne.rs`, `colonneui.rs`, `docs/INTERFACE.md` partie 194) : les piles d'origine de chaque
//! espace de travail, ranger un panneau (▲, ▼, glisser), le replier, sa hauteur bornée, celui qui grandit, le menu
//! « + Panneau », sortir un onglet de Propriétés et l'y remettre, la pile retenue par espace de travail et relue des réglages,
//! les dispositions de la version 9 converties sans perdre un panneau, la bande du bas sur toute la largeur sous la colonne,
//! et un panneau ouvert par sa commande qui se montre dans la pile. Les essais d'écran passent par la vraie fenêtre sans écran
//! (`Banc`) ; on y trouve chaque bouton par le nom de sa bulle (`bulles::vus`).

use eframe::egui::{pos2, vec2, Event, Modifiers, PointerButton, Pos2, Rect};

use crate::bulles::vus;
use crate::colonne::{self, Colonne, Fermeture, PanneauPile, Sorte, Voulue};
use crate::dock::{self, DetachedPanel, DockLayout, DockNode, FloatingPanel, Pane};
use crate::perf_tests::{banc, Banc};
use crate::pistes_tests::clic;
use crate::workspace::{Espace, Layout};

// ------------------------------------------------------------------------------------------------ outils

/// La pile d'une disposition : la sorte et les panneaux de chaque panneau de la pile, de haut en bas.
fn pile(l: &DockLayout) -> Vec<(Sorte, Vec<Pane>)> {
    l.colonne.panneaux.iter().map(|p| (p.sorte, p.panneaux.clone())).collect()
}

fn simple(p: &[Pane]) -> (Sorte, Vec<Pane>) {
    (Sorte::Simple, p.to_vec())
}

fn props(p: &[Pane]) -> (Sorte, Vec<Pane>) {
    (Sorte::Proprietes, p.to_vec())
}

fn tabs(p: &[Pane]) -> DockNode {
    DockNode::Tabs { panneaux: p.to_vec(), actif: 0 }
}

fn row(part: f32, a: DockNode, b: DockNode) -> DockNode {
    DockNode::Row { part, gauche: Box::new(a), droite: Box::new(b) }
}

fn column(part: f32, a: DockNode, b: DockNode) -> DockNode {
    DockNode::Column { part, haut: Box::new(a), bas: Box::new(b) }
}

/// La fenêtre principale est-elle « le dessin et la colonne côte à côte, la bande dessous, sur toute la largeur » ?
fn bande_sous_tout(n: &DockNode) -> bool {
    match n {
        DockNode::Column { haut, bas, .. } => {
            **bas == tabs(&[Pane::Montage]) && matches!(&**haut, DockNode::Row { gauche, droite, .. } if **gauche == tabs(&[Pane::Canvas]) && **droite == tabs(&[Pane::Colonne]))
        }
        _ => false,
    }
}

fn compte(l: &DockLayout, pane: Pane) -> usize {
    l.panes().iter().filter(|p| **p == pane).count()
}

/// Une pile de trois panneaux simples : la Couleur, les Effets, la Bibliothèque.
fn trois() -> Colonne {
    Colonne::new(vec![PanneauPile::simple(Pane::Color), PanneauPile::simple(Pane::Effects), PanneauPile::simple(Pane::Bibliotheque)])
}

fn ordre(c: &Colonne) -> Vec<Pane> {
    c.panneaux.iter().filter_map(PanneauPile::devant).collect()
}

/// Un banc sur un film de deux plans, dans l'espace `espace`.
fn banc_dans(espace: Espace) -> Banc {
    let mut b = banc(crate::pistes_tests::doc_with(&[&["Décor", "Perso", "Ombre"], &["Seul"]]), false);
    b.app.set_espace(espace);
    // (Partie 197 : ces essais portent sur la pile de la partie 194, la Couleur et Propriétés ; la Scène, en haut de celle du
    // Storyboard depuis, a les siens, `scene_tests.rs`.)
    sans_la_scene(&mut b.app);
    b.chauffer(6);
    b
}

/// La pile d'origine du Storyboard de la partie 194 (la Couleur, Propriétés) : celle d'aujourd'hui sans la Scène (partie 197).
fn pile_194() -> Colonne {
    let mut c = Espace::Storyboard.defaut().dock_layout().colonne;
    c.retirer(Pane::Scene);
    c
}

/// Partie 197 : retire le panneau Scène de la pile (il est en haut de celle d'origine du Storyboard), pour un essai qui porte sur les
/// autres panneaux et leur place.
pub(crate) fn sans_la_scene(app: &mut crate::app::App) {
    app.colonne.retirer(Pane::Scene);
}

fn ou(nom: &str) -> Rect {
    *vus::ou(nom).first().unwrap_or_else(|| panic!("« {nom} » n'est pas à l'écran : {:?}", vus::noms()))
}

/// Les rectangles des éléments dont la bulle s'appelle `nom`, de haut en bas.
fn de_haut_en_bas(nom: &str) -> Vec<Rect> {
    let mut v = vus::ou(nom);
    v.sort_by(|a, c| a.top().total_cmp(&c.top()));
    v
}

/// La place à l'écran du panneau d'egui_dock qui montre `pane`.
fn place(b: &Banc, pane: Pane) -> Rect {
    let state = b.app.dock.as_ref().expect("la disposition à l'écran");
    let (s, n, _) = state.find_tab(&pane).unwrap_or_else(|| panic!("{pane:?} n'est pas dans egui_dock"));
    match &state[s][n] {
        egui_dock::Node::Leaf { rect, .. } => *rect,
        _ => unreachable!(),
    }
}

fn appui(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Appuie en `de`, glisse jusqu'à `a` en quelques images, lâche.
fn glisser(b: &mut Banc, de: Pos2, a: Pos2) {
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    b.image(vec![appui(a, false)], 1.0 / 60.0);
    b.chauffer(2);
}

// ------------------------------------------------------------------------------------------------ 1. les piles d'origine

#[test]
fn each_workspace_has_its_own_default_stack_and_keeps_every_panel_it_had() {
    let sb = Espace::Storyboard.defaut().dock_layout();
    // Storyboard : la Couleur (avec les Brosses dédiées, comme avant) en haut, puis Propriétés et ses onglets. (Partie 197 : la Scène
    // au-dessus de tout.)
    assert_eq!(pile(&sb), vec![simple(&[Pane::Scene]), simple(&[Pane::Color, Pane::Personnages]), props(&[Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario])]);
    assert_eq!(sb.colonne.onglet_devant(), Some(Pane::ToolOptions), "les Réglages de l'outil devant");
    assert_eq!(sb.version, dock::LAYOUT_VERSION);
    assert_eq!(dock::LAYOUT_VERSION, 11);
    // Animation : la même, sans le Scénario (il n'y était pas).
    let anim = Espace::Animation.defaut().dock_layout();
    assert_eq!(pile(&anim), vec![simple(&[Pane::Color, Pane::Personnages]), props(&[Pane::ToolOptions, Pane::Sheet, Pane::Effects])]);
    // Vignettes : la Planche, la Couleur, puis Propriétés (Réglages, Scénario) ; pas de bande.
    let vig = Espace::Vignettes.defaut().dock_layout();
    assert_eq!(pile(&vig), vec![simple(&[Pane::Vignettes]), simple(&[Pane::Color]), props(&[Pane::ToolOptions, Pane::Scenario])]);
    assert!(!vig.contains(Pane::Montage));
    // Montage : la Fiche du plan, sortie des onglets de Propriétés (la fermer l'y remet), au-dessus des panneaux du son.
    let mon = Espace::Montage.defaut().dock_layout();
    assert_eq!(pile(&mon), vec![simple(&[Pane::Sheet]), props(&[Pane::Bruitages, Pane::Comments, Pane::History])]);
    assert!(mon.colonne.panneaux[0].sorti);
    // Chaque panneau une fois ; rien dans la fenêtre principale que le dessin, la colonne et la bande.
    fn collect(n: &DockNode, out: &mut Vec<Pane>) {
        match n {
            DockNode::Tabs { panneaux, .. } => out.extend(panneaux),
            DockNode::Row { gauche: a, droite: b, .. } | DockNode::Column { haut: a, bas: b, .. } => {
                collect(a, out);
                collect(b, out);
            }
        }
    }
    for (e, l) in [(Espace::Storyboard, &sb), (Espace::Animation, &anim), (Espace::Vignettes, &vig), (Espace::Montage, &mon)] {
        for p in Pane::ALL {
            assert!(compte(l, p) <= 1, "{e:?} : {p:?} en double");
        }
        let mut arbre = Vec::new();
        collect(&l.principal, &mut arbre);
        assert!(arbre.iter().all(|p| matches!(p, Pane::Canvas | Pane::Colonne | Pane::Montage)), "{e:?} : {arbre:?}");
        assert!(!l.contains(Pane::Espace3d), "{e:?} : le panneau Espace 3D n'arrive qu'avec la vue");
    }
    // Ce que le Storyboard montrait dans sa colonne avant la partie 194, chaque panneau à sa place d'avant (Réglages, Fiche,
    // Effets et Scénario en onglets ; la Couleur et les Brosses dédiées ensemble).
    // (Partie 197 : la Scène au-dessus.)
    assert_eq!(sb.colonne.panes(), vec![Pane::Scene, Pane::Color, Pane::Personnages, Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario]);
}

// ------------------------------------------------------------------------------------------------ 2. ranger

#[test]
fn up_down_and_dragging_a_panel_move_it_in_the_stack() {
    let mut c = trois();
    c.monter(0);
    assert_eq!(ordre(&c), vec![Pane::Color, Pane::Effects, Pane::Bibliotheque], "le premier ne monte pas plus haut");
    c.descendre(2);
    assert_eq!(ordre(&c), vec![Pane::Color, Pane::Effects, Pane::Bibliotheque], "le dernier ne descend pas plus bas");
    c.descendre(0);
    assert_eq!(ordre(&c), vec![Pane::Effects, Pane::Color, Pane::Bibliotheque]);
    c.monter(2);
    assert_eq!(ordre(&c), vec![Pane::Effects, Pane::Bibliotheque, Pane::Color]);

    // Où tombe un panneau glissé : devant le premier panneau dont le pointeur est au-dessus du milieu, sinon en bas.
    let tranches = [(0.0, 100.0), (100.0, 200.0), (200.0, 300.0)];
    assert_eq!(colonne::index_de_depot(&tranches, 10.0), 0);
    assert_eq!(colonne::index_de_depot(&tranches, 60.0), 1);
    assert_eq!(colonne::index_de_depot(&tranches, 160.0), 2);
    assert_eq!(colonne::index_de_depot(&tranches, 290.0), 3);
    // Glissé du haut et lâché tout en bas ; glissé du bas et lâché en haut ; lâché juste sous lui-même : il ne bouge pas.
    let mut c = trois();
    c.deposer(0, 3);
    assert_eq!(ordre(&c), vec![Pane::Effects, Pane::Bibliotheque, Pane::Color]);
    c.deposer(2, 0);
    assert_eq!(ordre(&c), vec![Pane::Color, Pane::Effects, Pane::Bibliotheque]);
    c.deposer(1, 2);
    assert_eq!(ordre(&c), vec![Pane::Color, Pane::Effects, Pane::Bibliotheque]);
    c.deposer(1, 1);
    assert_eq!(ordre(&c), vec![Pane::Color, Pane::Effects, Pane::Bibliotheque]);
}

#[test]
fn the_buttons_and_the_grip_of_a_panel_header_move_it_on_screen() {
    let mut b = banc_dans(Espace::Storyboard);
    assert_eq!(pile(&b.app.dock_layout()), vec![simple(&[Pane::Color, Pane::Personnages]), props(&[Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario])]);
    // ▼ sur la Couleur (le premier « Descendre le panneau » de la colonne, de haut en bas) : elle passe sous Propriétés.
    clic(&mut b, de_haut_en_bas("Descendre le panneau")[0].center(), Modifiers::NONE);
    assert_eq!(b.app.colonne.panneaux[0].sorte, Sorte::Proprietes, "{:?}", b.app.colonne);
    assert_eq!(b.app.colonne.panneaux[1].panneaux, vec![Pane::Color, Pane::Personnages]);
    // ▲ la remonte.
    clic(&mut b, de_haut_en_bas("Monter le panneau").last().unwrap().center(), Modifiers::NONE);
    assert_eq!(b.app.colonne.panneaux[0].panneaux, vec![Pane::Color, Pane::Personnages]);
    // La poignée de la Couleur, glissée tout en bas de la colonne : elle y tombe.
    let depart = de_haut_en_bas("Déplacer le panneau")[0].center();
    let colonne = place(&b, Pane::Colonne);
    let arrivee = pos2(colonne.center().x, colonne.bottom() - 12.0);
    b.image(vec![Event::PointerMoved(depart)], 1.0 / 60.0);
    b.image(vec![appui(depart, true)], 1.0 / 60.0);
    for k in 1..=8 {
        b.image(vec![Event::PointerMoved(depart + (arrivee - depart) * (k as f32 / 8.0))], 1.0 / 60.0);
    }
    assert_eq!(b.app.colonne_etat.glisse, Some(0), "le panneau glisse");
    b.image(vec![appui(arrivee, false)], 1.0 / 60.0);
    b.chauffer(2);
    assert_eq!(b.app.colonne_etat.glisse, None);
    assert_eq!(b.app.colonne.panneaux[1].panneaux, vec![Pane::Color, Pane::Personnages], "la Couleur est tombée en bas : {:?}", b.app.colonne);
}

// ------------------------------------------------------------------------------------------------ 3. replier

#[test]
fn a_folded_panel_shows_only_its_header_and_opening_it_unfolds_it() {
    let mut c = trois();
    c.replier(1);
    assert!(c.panneaux[1].replie);
    assert!(!c.devant(Pane::Effects), "replié, il ne se voit pas");
    assert!(c.devant(Pane::Color));
    // Le corps d'un panneau replié n'a pas de hauteur : seul son en-tête est là.
    let h = colonne::hauteurs(600.0, &[Some(Voulue::ferme(150.0)), None, Some(Voulue::ferme(200.0))], c.qui_grandit());
    assert_eq!(h[1], 0.0);
    // Le montrer (sa commande : `App::show_pane`, `toggle_pane`) le déplie ; le ranger dans la pile, déjà là, ne change rien.
    c.ouvrir(Pane::Effects);
    assert!(c.panneaux[1].replie);
    c.montrer(Pane::Effects);
    assert!(!c.panneaux[1].replie && c.devant(Pane::Effects));
    c.replier(1);
    c.replier(1);
    assert!(!c.panneaux[1].replie, "deux clics : déplié de nouveau");

    // À l'écran : un clic sur le titre de Propriétés le replie ; le corps (ses onglets d'icônes) disparaît.
    let mut b = banc_dans(Espace::Storyboard);
    assert!(!vus::ou("Fiche du plan").is_empty(), "l'onglet de la fiche est là : {:?}", vus::noms());
    clic(&mut b, ou("Propriétés").center(), Modifiers::NONE);
    assert!(b.app.colonne.panneaux[1].replie, "{:?}", b.app.colonne);
    assert!(vus::ou("Fiche du plan").is_empty(), "replié : plus d'onglets");
    clic(&mut b, ou("Propriétés").center(), Modifiers::NONE);
    assert!(!b.app.colonne.panneaux[1].replie);
}

// ------------------------------------------------------------------------------------------------ 4. la hauteur

#[test]
fn a_dragged_height_stays_within_its_bounds() {
    let mut c = trois();
    c.regler_hauteur(0, 10.0);
    assert_eq!(c.panneaux[0].hauteur, Some(colonne::HAUTEUR_MIN));
    c.regler_hauteur(0, 5000.0);
    assert_eq!(c.panneaux[0].hauteur, Some(colonne::HAUTEUR_MAX));
    c.regler_hauteur(0, 180.0);
    assert_eq!(c.panneaux[0].hauteur, Some(180.0));
    // Une hauteur absurde lue dans les réglages est ramenée dans ses bornes, ou oubliée.
    let mut abime = trois();
    abime.panneaux[0].hauteur = Some(f32::NAN);
    abime.panneaux[1].hauteur = Some(-40.0);
    let propre = abime.nettoyee(&mut Vec::new());
    assert_eq!(propre.panneaux[0].hauteur, None);
    assert_eq!(propre.panneaux[1].hauteur, Some(colonne::HAUTEUR_MIN));
    // « Réajuster les panneaux » oublie les hauteurs tirées : chacun reprend celle de son contenu.
    c.oublier_les_hauteurs();
    assert!(c.panneaux.iter().all(|p| p.hauteur.is_none()));
    // Pas assez de place : (partie 196.5) les panneaux qui disent la hauteur de leur contenu la gardent, celui qui grandit a son
    // minimum, et la colonne défile ; seules les listes à leur hauteur d'origine cèdent, au prorata, jusqu'à leur minimum.
    // (Avant : tous se partageaient la place au prorata, et la Couleur s'écrasait.)
    let h = colonne::hauteurs(400.0, &[Some(Voulue::ferme(300.0)), Some(Voulue::ferme(300.0)), Some(Voulue::ferme(100.0))], Some(2));
    assert_eq!((h[0], h[1], h[2]), (300.0, 300.0, colonne::GRANDIT_MIN), "gardées, la colonne défile : {h:?}");
    let h = colonne::hauteurs(400.0, &[Some(Voulue::souple(230.0)), Some(Voulue::souple(230.0)), Some(Voulue::ferme(100.0))], Some(2));
    assert!((h[0] - h[1]).abs() < 0.01 && h[0] >= colonne::HAUTEUR_MIN, "au prorata, au moins le minimum : {h:?}");
    assert!(h[2] >= colonne::GRANDIT_MIN - 0.01, "{h:?}");
    assert!(h.iter().sum::<f32>() + 3.0 * colonne::ENTETE + 2.0 * colonne::BORD <= 400.0 + 0.5, "{h:?}");

    // À l'écran : le bord du bas de la Couleur, juste au-dessus de l'en-tête de Propriétés, tiré de 60 points vers le bas.
    let mut b = banc_dans(Espace::Storyboard);
    assert_eq!(b.app.colonne.panneaux[0].hauteur, None, "la hauteur de son contenu, d'origine");
    let colonne = place(&b, Pane::Colonne);
    let poignees = de_haut_en_bas("Déplacer le panneau");
    let bord = pos2(colonne.center().x, poignees[1].center().y - colonne::ENTETE * 0.5 - colonne::BORD * 0.5);
    let avant = b.app.colonne_etat.tranches[0];
    glisser(&mut b, bord, bord + vec2(0.0, 60.0));
    let h = b.app.colonne.panneaux[0].hauteur.expect("une hauteur tirée à la main");
    assert!(h > colonne::HAUTEUR_MIN && h <= colonne::HAUTEUR_MAX, "{h}");
    let apres = b.app.colonne_etat.tranches[0];
    assert!((apres.1 - apres.0) - (avant.1 - avant.0) > 40.0, "le panneau a grandi : {avant:?} → {apres:?}");
}

// ------------------------------------------------------------------------------------------------ 5. celui qui grandit

#[test]
fn properties_grows_otherwise_the_last_open_panel() {
    let mut c = pile_194();
    assert_eq!(c.qui_grandit(), Some(1), "Propriétés, ouvert");
    c.replier(1);
    assert_eq!(c.qui_grandit(), Some(0), "Propriétés replié : le dernier ouvert");
    c.replier(0);
    assert_eq!(c.qui_grandit(), None, "tout replié : aucun");
    let mut sans = trois();
    assert_eq!(sans.qui_grandit(), Some(2), "sans Propriétés : le dernier ouvert");
    sans.replier(2);
    assert_eq!(sans.qui_grandit(), Some(1));
    // Celui qui grandit prend tout ce qui reste : la colonne moins les en-têtes, le bord du panneau fixe et sa hauteur.
    let h = colonne::hauteurs(600.0, &[Some(Voulue::ferme(150.0)), Some(Voulue::ferme(50.0))], Some(1));
    assert_eq!(h[0], 150.0);
    assert!((h[1] - (600.0 - 2.0 * colonne::ENTETE - colonne::BORD - 150.0)).abs() < 0.01, "{h:?}");

    // À l'écran : Propriétés descend jusqu'en bas de la colonne ; ses onglets d'icônes sont à la verticale, sur son bord gauche.
    let b = banc_dans(Espace::Storyboard);
    let colonne = place(&b, Pane::Colonne);
    let derniere = *b.app.colonne_etat.tranches.last().unwrap();
    assert!((derniere.1 - colonne.bottom()).abs() < 2.0, "{derniere:?} / {colonne:?}");
    let fiche = ou("Fiche du plan");
    assert!(fiche.left() < colonne.left() + 40.0, "{fiche:?} / {colonne:?}");
    let reglages = ou("Réglages de l'outil");
    assert!(reglages.top() < fiche.top() && (reglages.center().x - fiche.center().x).abs() < 1.0, "à la verticale");
}

// ------------------------------------------------------------------------------------------------ 6. « + Panneau »

#[test]
fn the_add_panel_menu_lists_what_the_workspace_can_show_and_greys_what_is_there() {
    let sb = Espace::Storyboard.defaut().dock_layout();
    let la = |p: Pane| sb.contains(p);
    let liste = colonne::propositions(Espace::Storyboard, false, false, true, &la);
    assert_eq!(liste[0].pane, None, "Propriétés en tête");
    assert!(liste[0].deja_la);
    let de = |p: Pane| liste.iter().find(|x| x.pane == Some(p)).copied();
    assert!(de(Pane::Color).unwrap().deja_la && de(Pane::ToolOptions).unwrap().deja_la, "déjà là");
    assert!(!de(Pane::Bibliotheque).unwrap().deja_la && !de(Pane::History).unwrap().deja_la);
    assert!(de(Pane::XSheet).is_none(), "pas de feuille d'exposition hors de l'animation");
    assert!(de(Pane::SuggestionsSonores).is_none() && de(Pane::MonPanneau).is_none(), "pas d'essais");
    assert!([Pane::Canvas, Pane::Montage, Pane::Colonne, Pane::Tools].iter().all(|p| de(*p).is_none()));
    // Les panneaux qui servent à ce travail d'abord (ceux que proposait la barre des panneaux), dans leur ordre.
    let rangs: Vec<Pane> = liste.iter().filter_map(|x| x.pane).collect();
    let d_office = crate::barrepanneaux::panneaux_d_office(Espace::Storyboard);
    assert_eq!(&rangs[..d_office.len()], d_office);
    // En animation, la feuille d'exposition ; avec la case des essais, les suggestions sonores.
    let anim = colonne::propositions(Espace::Animation, true, true, false, &|_| false);
    assert!(anim.iter().any(|x| x.pane == Some(Pane::XSheet)) && anim.iter().any(|x| x.pane == Some(Pane::SuggestionsSonores)));
    assert!(!anim[0].deja_la, "sans Propriétés : il se propose");

    // Ajouter : en bas de la pile ; Propriétés revient avec ses onglets d'origine, moins ceux qui sont ailleurs.
    let mut c = sb.colonne.clone();
    c.ajouter(Pane::Bibliotheque);
    assert_eq!(c.panneaux.last().unwrap().panneaux, vec![Pane::Bibliotheque]);
    c.ajouter(Pane::Bibliotheque);
    assert_eq!(c.panneaux.len(), 4, "déjà là : rien de plus (la Scène, la Couleur, Propriétés, la Bibliothèque)");
    let mut sans = Colonne::new(vec![PanneauPile::simple(Pane::Sheet)]);
    sans.ajouter_proprietes(&colonne::onglets_d_origine(Espace::Storyboard), &|p| p == Pane::Effects);
    assert_eq!(pile(&DockLayout { colonne: sans, ..sb.clone() }).last().unwrap(), &props(&[Pane::ToolOptions, Pane::Scenario]));

    // À l'écran : « + Panneau » en haut de la colonne ; la Bibliothèque, choisie dans son menu, arrive en bas de la pile.
    let mut b = banc_dans(Espace::Storyboard);
    let colonne = place(&b, Pane::Colonne);
    let bouton = ou("Ajouter un panneau");
    assert!(colonne.contains(bouton.center()) && bouton.top() < colonne.top() + 30.0, "{bouton:?} / {colonne:?}");
    clic(&mut b, bouton.center(), Modifiers::NONE);
    let biblio = *vus::ou("Bibliothèque").last().unwrap_or_else(|| panic!("le menu : {:?}", vus::noms()));
    clic(&mut b, biblio.center(), Modifiers::NONE);
    assert_eq!(b.app.colonne.panneaux.last().unwrap().panneaux, vec![Pane::Bibliotheque], "{:?}", b.app.colonne);
}

// ------------------------------------------------------------------------------------------------ 7. sortir un onglet

#[test]
fn popping_out_the_front_tab_makes_a_panel_above_properties_and_closing_it_brings_the_tab_back() {
    let mut c = pile_194();
    c.montrer(Pane::Sheet);
    assert_eq!(c.sortir_l_onglet(), Some(Pane::Sheet));
    assert_eq!(c.panneaux.len(), 3);
    assert_eq!(c.panneaux[1].panneaux, vec![Pane::Sheet], "juste au-dessus de Propriétés");
    assert!(c.panneaux[1].sorti);
    assert_eq!(c.panneaux[2].panneaux, vec![Pane::ToolOptions, Pane::Effects, Pane::Scenario], "il a quitté les onglets");
    // Le fermer le remet en onglet, à son rang, devant.
    assert_eq!(c.fermer_panneau(1), Fermeture::RevenuEnOnglet);
    assert_eq!(c.panneaux[1].panneaux, vec![Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario]);
    assert_eq!(c.onglet_devant(), Some(Pane::Sheet));
    // Un panneau qui n'est pas sorti des onglets se ferme pour de bon.
    assert_eq!(c.fermer_panneau(0), Fermeture::Ferme);
    assert!(!c.contient(Pane::Color));
    // Tous sortis : Propriétés reste, vide.
    while c.sortir_l_onglet().is_some() {}
    let vide = &c.panneaux[c.proprietes().unwrap()];
    assert!(vide.panneaux.is_empty() && c.panneaux.len() == 5);

    // À l'écran : ↗ sort les Réglages de l'outil en un panneau ; son ✕ les remet en onglet.
    let mut b = banc_dans(Espace::Storyboard);
    clic(&mut b, ou("Sortir l'onglet en panneau").center(), Modifiers::NONE);
    assert_eq!(pile(&b.app.dock_layout()), vec![simple(&[Pane::Color, Pane::Personnages]), simple(&[Pane::ToolOptions]), props(&[Pane::Sheet, Pane::Effects, Pane::Scenario])]);
    clic(&mut b, de_haut_en_bas("Fermer le panneau")[1].center(), Modifiers::NONE);
    assert_eq!(pile(&b.app.dock_layout()), vec![simple(&[Pane::Color, Pane::Personnages]), props(&[Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario])]);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::ToolOptions));
}

// ------------------------------------------------------------------------------------------------ 8. retenue par espace

#[test]
fn each_workspace_keeps_its_stack_and_it_survives_the_settings_file() {
    let mut b = banc_dans(Espace::Storyboard);
    b.app.colonne.descendre(0);
    b.app.colonne.replier(1);
    b.chauffer(2);
    let la_mienne = b.app.dock_layout().colonne;
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(pile(&b.app.dock_layout()), vec![simple(&[Pane::Sheet]), props(&[Pane::Bruitages, Pane::Comments, Pane::History])], "le Montage a la sienne");
    b.app.colonne.montrer(Pane::History);
    b.chauffer(2);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(4);
    assert!(b.app.dock_layout().colonne.proche_de(&la_mienne), "le Storyboard retrouve la sienne : {:?}", b.app.colonne);
    b.app.set_espace(Espace::Montage);
    b.chauffer(4);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::History), "et le Montage la sienne");
    // « Rétablir la disposition » : la pile d'origine de l'espace.
    b.app.run_action("espace.retablir");
    b.chauffer(2);
    assert!(b.app.dock_layout().colonne.proche_de(&Espace::Montage.defaut().dock_layout().colonne));

    // Le fichier des réglages (TOML) la garde : sorte, panneaux, devant, replié, hauteur, sorti.
    let mut l = Espace::Storyboard.defaut().dock_layout();
    l.colonne.sortir_l_onglet();
    l.colonne.regler_hauteur(0, 222.0);
    l.colonne.replier(0);
    let mut s = crate::settings::Settings::default();
    s.set_layout_of(Espace::Storyboard, Layout { panneaux: Some(l.clone()), ..Layout::default() });
    s.set_layout_of(Espace::Montage, Espace::Montage.defaut());
    let texte = toml::to_string(&s).expect("les réglages s'écrivent");
    assert!(texte.contains("proprietes") && texte.contains("colonne"), "{texte}");
    let relu: crate::settings::Settings = toml::from_str(&texte).expect("et se relisent");
    let relu = crate::settings::Settings::apres_lecture(relu);
    assert_eq!(relu.layout_of(Espace::Storyboard).unwrap().dock_layout(), l);
    assert!(relu.layout_of(Espace::Montage).unwrap().dock_layout().close_to(&Espace::Montage.defaut().dock_layout()));
    // Et en JSON.
    let json = serde_json::to_string(&l).unwrap();
    assert_eq!(serde_json::from_str::<DockLayout>(&json).unwrap(), l);
    // Une pile abîmée (deux Propriétés, un panneau en double, un panneau qui ne va pas dans la colonne) se répare.
    let abimee = Colonne::new(vec![
        PanneauPile::proprietes(vec![Pane::ToolOptions]),
        PanneauPile::simple(Pane::Canvas),
        PanneauPile::proprietes(vec![Pane::Sheet, Pane::ToolOptions]),
        PanneauPile::simple(Pane::Color),
    ]);
    let d = DockLayout { colonne: abimee, ..Espace::Storyboard.defaut().dock_layout() }.sanitized();
    assert_eq!(pile(&d), vec![props(&[Pane::ToolOptions, Pane::Sheet]), simple(&[Pane::Color])]);
    assert_eq!(compte(&d, Pane::Canvas), 1);
}

// ------------------------------------------------------------------------------------------------ 9. la version 9

/// La disposition du Storyboard de la version 9 (partie 161), telle que les réglages la gardent : la colonne unique, Réglages,
/// Fiche, Effets, Scénario et Historique en onglets (les Effets devant), la Couleur et les Brosses dédiées dessous ; la bande en
/// bas ; les Références flottantes, la Bibliothèque dans sa fenêtre.
fn storyboard_v9() -> DockLayout {
    let cote = column(0.55, DockNode::Tabs { panneaux: vec![Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario, Pane::History], actif: 2 }, tabs(&[Pane::Color, Pane::Personnages]));
    DockLayout {
        version: 9,
        principal: column(1.0 - 290.0 / 820.0, row(0.8, tabs(&[Pane::Canvas]), cote), tabs(&[Pane::Montage])),
        fenetres: vec![FloatingPanel { panneaux: tabs(&[Pane::References]), x: 30.0, y: 40.0, largeur: 300.0, hauteur: 400.0 }],
        detachees: vec![DetachedPanel { panneau: Pane::Bibliotheque, x: 0.0, y: 0.0, largeur: 0.0, hauteur: 0.0 }],
        colonne: Colonne::default(),
    }
}

#[test]
fn a_version_9_layout_becomes_a_stack_without_losing_a_panel() {
    let old = storyboard_v9();
    let avant = old.panes();
    let new = old.clone().upgraded();
    assert_eq!(new.version, dock::LAYOUT_VERSION);
    for p in &avant {
        assert_eq!(compte(&new, *p), 1, "{p:?}");
    }
    assert_eq!(pile(&new), vec![simple(&[Pane::Color, Pane::Personnages]), props(&[Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario, Pane::History])]);
    assert_eq!(new.colonne.onglet_devant(), Some(Pane::Effects), "l'onglet qui était devant le reste");
    assert!(bande_sous_tout(&new.principal), "{:?}", new.principal);
    assert_eq!(new.fenetres, old.fenetres, "le panneau flottant ne bouge pas");
    assert_eq!(new.detachees, old.detachees, "celui dans sa fenêtre non plus");
    // Lue par les réglages (le fichier TOML d'avant), la même chose.
    let l = Layout { panneaux: Some(old.clone()), ..Layout::default() };
    assert_eq!(l.dock_layout(), new);

    // Les espaces de la version 9 : le Montage (la fiche, et les panneaux du son en onglets), les Vignettes (la Planche, les
    // Réglages, la Couleur et le Scénario en onglets).
    let montage = DockLayout {
        version: 9,
        principal: column(0.46, row(0.8, tabs(&[Pane::Canvas]), column(0.4, tabs(&[Pane::Sheet]), tabs(&[Pane::Bruitages, Pane::Comments, Pane::History]))), tabs(&[Pane::Montage])),
        fenetres: Vec::new(),
        detachees: Vec::new(),
        colonne: Colonne::default(),
    }
    .upgraded();
    assert_eq!(pile(&montage), vec![props(&[Pane::Sheet, Pane::Bruitages, Pane::Comments, Pane::History])]);
    assert!(bande_sous_tout(&montage.principal));
    let vignettes = DockLayout {
        version: 9,
        principal: row(0.8, tabs(&[Pane::Canvas]), column(0.2, tabs(&[Pane::Vignettes]), column(0.5, tabs(&[Pane::ToolOptions]), tabs(&[Pane::Color, Pane::Scenario])))),
        fenetres: Vec::new(),
        detachees: Vec::new(),
        colonne: Colonne::default(),
    }
    .upgraded();
    assert_eq!(pile(&vignettes), vec![simple(&[Pane::Vignettes]), simple(&[Pane::Color]), props(&[Pane::ToolOptions, Pane::Scenario])]);
    assert_eq!(vignettes.principal, row(1.0 - dock::COLONNE / 1454.0, tabs(&[Pane::Canvas]), tabs(&[Pane::Colonne])));

    // Une disposition de la version 9 où la bande était à côté de la colonne (sous le dessin seulement) : elle passe dessous,
    // sur toute la largeur, à la hauteur qu'elle avait.
    let a_cote = DockLayout {
        version: 9,
        principal: row(0.8, column(0.6, tabs(&[Pane::Canvas]), tabs(&[Pane::Montage])), tabs(&[Pane::ToolOptions, Pane::Effects])),
        fenetres: Vec::new(),
        detachees: Vec::new(),
        colonne: Colonne::default(),
    }
    .upgraded();
    assert!(bande_sous_tout(&a_cote.principal), "{:?}", a_cote.principal);
    assert!(matches!(a_cote.principal, DockNode::Column { part, .. } if (part - 0.6).abs() < 1e-4));
    assert_eq!(pile(&a_cote), vec![props(&[Pane::ToolOptions, Pane::Effects])]);

    // Les dispositions plus anciennes passent par la version 9, puis par la pile : aucune ne perd de panneau.
    let v8 = DockLayout { version: 8, ..storyboard_v9() }.upgraded();
    for p in &avant {
        assert_eq!(compte(&v8, *p), 1, "{p:?}");
    }
}

// ------------------------------------------------------------------------------------------------ 10. la bande en bas

#[test]
fn the_bottom_band_spans_the_whole_width_under_the_column_in_every_workspace() {
    // (Sauf dans le Storyboard depuis l'audit du 2026-11-21 : la colonne y prend toute la hauteur, voir l'essai suivant.)
    for e in [Espace::Animation, Espace::Montage] {
        assert!(bande_sous_tout(&e.defaut().dock_layout().principal), "{e:?} : {:?}", e.defaut().dock_layout().principal);
    }
    // Rouvrir la bande : de nouveau dessous, sur toute la largeur.
    let mut l = Espace::Storyboard.defaut().dock_layout();
    l.close(Pane::Montage);
    assert!(!l.contains(Pane::Montage));
    l.open(Pane::Montage);
    assert!(bande_sous_tout(&l.principal), "{:?}", l.principal);
    // Une colonne refaite (tout fermé, puis un panneau rouvert) se pose à droite du dessin, au-dessus de la bande.
    let mut l = Espace::Storyboard.defaut().dock_layout();
    for p in l.colonne.panes() {
        l.close(p);
    }
    assert!(!l.contains(Pane::Colonne), "pile vide : plus de colonne");
    l.open(Pane::Color);
    assert!(bande_sous_tout(&l.principal), "{:?}", l.principal);

    // À l'écran (1 920 × 1 080) : la bande va du bord du dessin au bord droit de la fenêtre, sous la colonne.
    for e in [Espace::Montage] {
        let b = banc_dans(e);
        let (dessin, colonne, bande) = (place(&b, Pane::Canvas), place(&b, Pane::Colonne), place(&b, Pane::Montage));
        assert!(colonne.bottom() <= bande.top() + 1.0, "{e:?} : la colonne s'arrête au-dessus de la bande ({colonne:?}, {bande:?})");
        assert!(bande.left() <= dessin.left() + 1.0 && bande.right() >= colonne.right() - 1.0, "{e:?} : toute la largeur ({dessin:?}, {colonne:?}, {bande:?})");
        assert!(bande.right() >= b.ctx.screen_rect().right() - 1.0, "{e:?} : plus de barre des panneaux au bord droit");
        assert!((colonne.width() - dock::COLONNE).abs() < 6.0, "{e:?} : la colonne garde ses 290 points ({})", colonne.width());
    }
}

/// Audit utilisateur du 2026-11-21 (lot 5) : dans le Storyboard, la colonne prend toute la hauteur de la fenêtre et la bande s'arrête à
/// son bord ; une disposition retenue d'avant (version 10) qui avait la forme d'origine la reçoit, une autre ne bouge pas.
#[test]
fn in_the_storyboard_the_column_takes_the_whole_height_and_the_band_stops_at_its_edge() {
    let haute = |n: &DockNode| match n {
        DockNode::Row { gauche, droite, .. } => {
            **droite == tabs(&[Pane::Colonne]) && matches!(&**gauche, DockNode::Column { haut, bas, .. } if **haut == tabs(&[Pane::Canvas]) && **bas == tabs(&[Pane::Montage]))
        }
        _ => false,
    };
    let d = Espace::Storyboard.defaut().dock_layout();
    assert!(haute(&d.principal), "{:?}", d.principal);
    // La forme d'origine d'avant : convertie, la bande à la même hauteur.
    let mut avant = DockLayout::pour_le_montage(290.0);
    assert!(bande_sous_tout(&avant.principal));
    let DockNode::Column { part, .. } = avant.principal.clone() else { unreachable!() };
    avant.colonne_haute();
    assert!(haute(&avant.principal), "{:?}", avant.principal);
    assert!(matches!(&avant.principal, DockNode::Row { gauche, .. } if matches!(&**gauche, DockNode::Column { part: p, .. } if (*p - part).abs() < 1e-6)));
    // Une disposition arrangée autrement reste comme elle est.
    let mut autre = DockLayout::pour_les_vignettes();
    let garde = autre.principal.clone();
    autre.colonne_haute();
    assert_eq!(autre.principal, garde);
    // Les réglages lus : la disposition 10 du Storyboard passe à la nouvelle forme, celle du Montage garde la sienne.
    let mut vieux = DockLayout::pour_le_montage(290.0);
    vieux.version = 10;
    let mut s = crate::settings::Settings::default();
    s.layout = Some(Layout { panneaux: Some(vieux.clone()), ..Layout::default() });
    s.montage_layout = Some(Layout { panneaux: Some(vieux), ..Layout::default() });
    let s = crate::settings::Settings::apres_lecture(s);
    assert!(haute(&s.layout.as_ref().unwrap().dock_layout().principal));
    assert!(bande_sous_tout(&s.montage_layout.as_ref().unwrap().dock_layout().principal));
    // À l'écran : la colonne descend jusqu'au bas de la bande, la bande s'arrête à son bord gauche.
    let b = banc_dans(Espace::Storyboard);
    let (dessin, colonne, bande) = (place(&b, Pane::Canvas), place(&b, Pane::Colonne), place(&b, Pane::Montage));
    assert!(colonne.bottom() >= bande.bottom() - 1.0, "la colonne va jusqu'en bas ({colonne:?}, {bande:?})");
    assert!(bande.right() <= colonne.left() + 1.0 && bande.left() <= dessin.left() + 1.0, "la bande s'arrête à la colonne ({bande:?}, {colonne:?})");
    assert!((colonne.width() - dock::COLONNE).abs() < 6.0, "la colonne garde ses 290 points ({})", colonne.width());
}

/// Audit utilisateur du 2026-11-21 (lot 5) : le panneau qui grandit et qui dit la hauteur de son contenu la reçoit tant que les listes
/// peuvent la céder ; elles ne passent jamais sous leur plancher.
#[test]
fn the_growing_panel_gets_the_height_of_its_content_while_the_lists_can_give_it() {
    let h = colonne::hauteurs(800.0, &[Some(Voulue::souple(230.0)), Some(Voulue::souple(230.0)), Some(Voulue::ferme(400.0))], Some(2));
    assert!((h[2] - 400.0).abs() < 1.0, "{h:?}");
    assert!(h[0] >= colonne::HAUTEUR_MIN && h[0] < 230.0 && (h[0] - h[1]).abs() < 0.01, "{h:?}");
    // Trop peu de place : les listes à leur plancher, pas en dessous ; celui qui grandit a le reste.
    let h = colonne::hauteurs(500.0, &[Some(Voulue::souple(230.0)), Some(Voulue::souple(230.0)), Some(Voulue::ferme(400.0))], Some(2));
    assert!((h[0] - colonne::HAUTEUR_MIN).abs() < 0.01 && h[2] >= colonne::GRANDIT_MIN, "{h:?}");
    // Assez de place : personne ne cède.
    let h = colonne::hauteurs(1200.0, &[Some(Voulue::souple(230.0)), Some(Voulue::ferme(400.0))], Some(1));
    assert!((h[0] - 230.0).abs() < 0.01 && h[1] >= 400.0, "{h:?}");
}

// ------------------------------------------------------------------------------------------------ 11. ouvrir par sa commande

#[test]
fn opening_a_panel_by_its_action_shows_it_in_the_stack() {
    let mut b = banc_dans(Espace::Storyboard);
    // Fermé : sa commande l'ouvre en onglet de Propriétés, à son rang, devant.
    assert!(!b.app.pane_shown(Pane::History));
    b.app.run_action("vue.historique");
    b.chauffer(2);
    assert!(b.app.colonne.en_onglet_de_proprietes(Pane::History));
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::History));
    assert!(b.app.panneau_devant(Pane::History));
    assert!(!vus::ou("Historique").is_empty(), "son onglet d'icône est là : {:?}", vus::noms());
    // Un autre onglet devant : sa commande le remet devant ; visible, elle le ferme.
    b.app.colonne.montrer(Pane::Sheet);
    b.app.run_action("vue.historique");
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::History));
    b.app.run_action("vue.historique");
    b.chauffer(2);
    assert!(!b.app.pane_shown(Pane::History));
    // Propriétés replié : la commande de la fiche le déplie, la fiche devant.
    let i = b.app.colonne.proprietes().unwrap();
    b.app.colonne.replier(i);
    assert!(!b.app.panneau_devant(Pane::Sheet));
    b.app.run_action("vue.fiche");
    assert!(!b.app.colonne.panneaux[i].replie && b.app.colonne.onglet_devant() == Some(Pane::Sheet));
    // La couleur : son panneau de la pile, déplié ; les Brosses dédiées devant dans le même panneau.
    b.app.colonne.replier(0);
    b.app.run_action("vue.personnages");
    assert!(!b.app.colonne.panneaux[0].replie && b.app.colonne.panneaux[0].devant() == Some(Pane::Personnages));
    // Sans Propriétés, un panneau rouvert arrive en bas de la pile.
    let mut l = b.app.dock_layout();
    l.colonne.fermer_panneau(l.colonne.proprietes().unwrap());
    b.app.set_dock_layout(l);
    b.app.run_action("vue.bibliotheque");
    b.chauffer(2);
    assert_eq!(b.app.colonne.panneaux.last().unwrap().panneaux, vec![Pane::Bibliotheque]);
    // Affichage > Panneaux fait de même (`toggle_pane`).
    b.app.toggle_pane(Pane::Comments);
    assert_eq!(b.app.colonne.panneaux.last().unwrap().panneaux, vec![Pane::Comments]);
}

#[test]
fn the_3d_space_panel_still_comes_to_the_front_of_properties_with_the_view_and_goes_back_when_it_closes() {
    let mut b = banc_dans(Espace::Storyboard);
    assert!(!b.app.pane_shown(Pane::Espace3d));
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.espace3d_vue_ouverte());
    assert!(b.app.colonne.en_onglet_de_proprietes(Pane::Espace3d), "{:?}", b.app.colonne);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::Espace3d));
    let onglets = &b.app.colonne.panneaux[b.app.colonne.proprietes().unwrap()].panneaux;
    assert_eq!(onglets, &vec![Pane::ToolOptions, Pane::Sheet, Pane::Effects, Pane::Scenario, Pane::Espace3d]);
    // La disposition retenue garde Réglages devant (la vue ne l'y a mis que pour un moment).
    assert_eq!(b.app.current_layout().dock_layout().colonne.onglet_devant(), Some(Pane::ToolOptions));
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert_eq!(b.app.colonne.onglet_devant(), Some(Pane::ToolOptions), "Réglages repasse devant");
    assert!(!b.app.pane_shown(Pane::Espace3d), "et l'onglet part avec la vue (partie 159)");
    // Sorti en panneau à lui, il reste dans la pile, la vue fermée.
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    b.app.colonne.mettre_devant(Pane::Espace3d);
    b.app.colonne.sortir_l_onglet();
    b.app.run_action("espace3d.basculer");
    b.chauffer(3);
    assert!(b.app.colonne.panneaux.iter().any(|p| !p.est_proprietes() && p.panneaux == vec![Pane::Espace3d]), "{:?}", b.app.colonne);
}

#[test]
fn a_panel_of_the_stack_can_still_float_or_have_its_own_window() {
    let mut b = banc_dans(Espace::Storyboard);
    // Flotter : il quitte la pile pour un panneau flottant d'egui_dock ; « Dans la colonne » l'y remet.
    b.app.colonne_etat.apres.push(crate::colonneui::Apres::Flotter(vec![Pane::Color, Pane::Personnages], 0, pos2(300.0, 200.0)));
    b.chauffer(2);
    let l = b.app.dock_layout();
    assert!(!l.colonne.contient(Pane::Color));
    assert_eq!(l.fenetres.len(), 1);
    assert_eq!(l.fenetres[0].panneaux, tabs(&[Pane::Color, Pane::Personnages]));
    b.app.colonne_etat.apres.push(crate::colonneui::Apres::DansLaColonne(Pane::Color));
    b.chauffer(2);
    assert!(b.app.colonne.contient(Pane::Color));
    // Lâché hors de la colonne, un panneau glissé par sa poignée flotte.
    let rang = b.app.colonne.rang_de(Pane::Color).unwrap();
    let depart = de_haut_en_bas("Déplacer le panneau")[rang].center();
    let dessin = place(&b, Pane::Canvas);
    glisser(&mut b, depart, dessin.center());
    assert!(!b.app.colonne.contient(Pane::Color), "{:?}", b.app.colonne);
    assert!(b.app.dock_layout().fenetres.iter().any(|w| matches!(&w.panneaux, DockNode::Tabs { panneaux, .. } if panneaux.contains(&Pane::Color))));
    // Dans sa fenêtre à part (clic droit sur l'en-tête), puis de retour dans la pile quand elle se ferme.
    b.app.detach_request = Some(Pane::Effects);
    b.chauffer(2);
    assert!(!b.app.colonne.contient(Pane::Effects) && b.app.detached.iter().any(|d| d.panneau == Pane::Effects));
    let mut l = b.app.dock_layout();
    l.detachees.retain(|d| d.panneau != Pane::Effects);
    l.open(Pane::Effects);
    assert!(l.colonne.en_onglet_de_proprietes(Pane::Effects), "il revient en onglet de Propriétés");
}

/// Pour les essais d'un panneau à grille ou d'une longue liste (Préréglages, Bibliothèque…) : `pane` sort des onglets de
/// Propriétés en un panneau à lui (toute la largeur de la colonne, comme un onglet d'avant la partie 194) et les autres panneaux
/// de la pile se replient, pour qu'il ait toute la hauteur de la colonne.
pub(crate) fn toute_la_place(app: &mut crate::app::App, pane: Pane) {
    if app.colonne.en_onglet_de_proprietes(pane) {
        app.colonne.mettre_devant(pane);
        app.colonne.sortir_l_onglet();
    }
    for (i, p) in app.colonne.panneaux.iter_mut().enumerate() {
        let _ = i;
        p.replie = !p.panneaux.contains(&pane);
    }
}

/// Pour les essais réglés au pixel près sur la place du dessin d'avant la partie 194 (une fenêtre de 1 920 de large, que leurs
/// captures supposent) : la colonne prend aussi les 34 points de l'ancienne barre des panneaux du bord droit, comme un bord
/// tiré à la main (`Settings::panes_fixed`), et le dessin retrouve exactement sa largeur d'avant.
pub(crate) fn dessin_d_avant_la_pile(b: &mut Banc) {
    let ecart = crate::theme::dock(egui_dock::Style::from_egui(b.ctx.style().as_ref())).separator.width;
    b.app.settings.panes_fixed.push((Pane::Canvas, Pane::Colonne));
    let Some(state) = b.app.dock.as_mut() else { return };
    let tree = state.main_surface_mut();
    for i in 0..tree.len() {
        if let egui_dock::Node::Horizontal { rect, fraction, .. } = &mut tree[egui_dock::NodeIndex(i)] {
            if rect.width() > 0.0 {
                let w = dock::COLONNE + 34.0;
                *fraction = ((rect.width() - w - ecart * 0.5) / rect.width()).clamp(0.05, 0.95);
            }
        }
    }
}

/// Le corps (là où se dessine le contenu) du panneau de la pile qui montre `pane`, à la dernière image : sans son en-tête, ni,
/// pour un onglet de Propriétés, la bande des onglets d'icônes.
pub(crate) fn corps_dans_la_pile(b: &Banc, pane: Pane) -> Option<Rect> {
    let i = b.app.colonne.rang_de(pane)?;
    let p = &b.app.colonne.panneaux[i];
    if p.replie {
        return None;
    }
    let zone = b.app.colonne_etat.zone?;
    let (haut, bas) = *b.app.colonne_etat.tranches.get(i)?;
    let bord = if Some(i) == b.app.colonne.qui_grandit() { 0.0 } else { colonne::BORD };
    let gauche = if p.est_proprietes() { zone.left() + crate::colonneui::BANDE_ONGLETS + 1.0 } else { zone.left() };
    Some(Rect::from_min_max(pos2(gauche, haut + colonne::ENTETE), pos2(zone.right(), bas - bord)))
}
