//! Le menu clic droit des calques (§82) : court, en groupes, le même partout, aucune commande perdue.

use eframe::egui::{self, pos2, vec2, Event, Modifiers, PointerButton, RawInput, Rect, Shape};

use crate::app::App;
use crate::document::{Document, Layer, LayerKind};
use crate::menucalque::{Origine, PREMIER_NIVEAU};

// ------------------------------------------------------------------------------------ l'outillage

fn app_layers(name: &str, kinds: &[LayerKind]) -> App {
    let mut doc = Document::with_frame(64, 48, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].set_length(6);
    for (i, kind) in kinds.iter().enumerate() {
        let cel = doc.new_cel(*kind);
        let mut l = Layer::new(format!("C{}", i + 1), *kind);
        l.frames = vec![Some(cel); 6];
        doc.panels[0].layers.push(l);
    }
    let dir = std::env::temp_dir().join(format!("shot-menucalque-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = egui::Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app.doc = doc;
    app.cur_panel = 0;
    app.cur_layer = 0;
    app.cur_frame = 0;
    app
}

struct Ecran {
    ctx: egui::Context,
    t: f64,
}

type Contenu<'a> = &'a mut dyn FnMut(&mut App, &mut egui::Ui);

impl Ecran {
    fn new() -> Self {
        Self { ctx: egui::Context::default(), t: 0.0 }
    }

    fn image(&mut self, app: &mut App, events: Vec<Event>, contenu: Contenu) -> Vec<(String, Rect)> {
        self.t += 1.0 / 60.0;
        let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 1100.0))), time: Some(self.t), events, ..RawInput::default() };
        let out = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| contenu(app, ui));
        });
        let mut textes = Vec::new();
        for s in &out.shapes {
            collecter(&s.shape, &mut textes);
        }
        textes
    }

    fn chauffer(&mut self, app: &mut App, n: usize, contenu: Contenu) -> Vec<(String, Rect)> {
        let mut textes = Vec::new();
        for _ in 0..n {
            textes = self.image(app, Vec::new(), contenu);
        }
        textes
    }

    fn cliquer(&mut self, app: &mut App, p: egui::Pos2, contenu: Contenu) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }], contenu);
        self.image(app, vec![Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }], contenu);
        self.chauffer(app, 3, contenu)
    }

    /// Le pointeur se pose en `p` assez longtemps pour que la bulle et les sous-menus s'ouvrent.
    fn survoler(&mut self, app: &mut App, p: egui::Pos2, contenu: Contenu, images: usize) -> Vec<(String, Rect)> {
        self.image(app, vec![Event::PointerMoved(p - vec2(0.0, 1.0))], contenu);
        self.chauffer(app, 4, contenu);
        self.image(app, vec![Event::PointerMoved(p)], contenu);
        self.chauffer(app, images, contenu)
    }
}

fn collecter(shape: &Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rect.translate(t.pos.to_vec2()))),
        Shape::Vec(v) => v.iter().for_each(|s| collecter(s, out)),
        _ => {}
    }
}

fn trouver<'a>(textes: &'a [(String, Rect)], morceau: &str) -> Option<&'a (String, Rect)> {
    textes.iter().find(|(t, _)| t.contains(morceau))
}

fn contient(textes: &[(String, Rect)], morceau: &str) -> bool {
    trouver(textes, morceau).is_some()
}

/// Le menu d'un calque posé à plat dans la zone (comme le font les essais d'`aplat_tests`) : ses entrées du premier niveau.
fn menu(app: &mut App, ecran: &mut Ecran, origine: Origine, li: usize) -> Vec<(String, Rect)> {
    let mut contenu = |a: &mut App, ui: &mut egui::Ui| a.layer_menu(ui, li, origine);
    ecran.chauffer(app, 5, &mut contenu)
}

/// Les libellés du premier niveau, de haut en bas (un raccourci ou une flèche sont sur la ligne de leur nom : on garde les
/// textes qui commencent une ligne).
fn premier_niveau(textes: &[(String, Rect)]) -> Vec<String> {
    let mut v: Vec<&(String, Rect)> = textes.iter().filter(|(t, _)| t.chars().any(|c| c.is_alphabetic()) && !t.starts_with("Ctrl") && !t.starts_with("Alt") && !t.starts_with("Maj")).collect();
    v.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    let mut out: Vec<String> = Vec::new();
    let mut dernier = f32::NEG_INFINITY;
    for (t, r) in v {
        if r.center().y - dernier > 6.0 {
            out.push(t.clone());
            dernier = r.center().y;
        }
    }
    out
}

// -------------------------------------------------------------------------------------------- la structure

#[test]
fn the_layer_menu_has_a_dozen_entries_at_most_whatever_the_layer_and_the_place() {
    for kinds in [&[LayerKind::Vector][..], &[LayerKind::Bitmap, LayerKind::Vector, LayerKind::Bitmap]] {
        for origine in [Origine::Panneau, Origine::Timeline, Origine::Montage, Origine::Barre] {
            let mut app = app_layers("douzaine", kinds);
            let mut ecran = Ecran::new();
            let lignes = premier_niveau(&menu(&mut app, &mut ecran, origine, 0));
            assert!(lignes.len() <= PREMIER_NIVEAU, "{origine:?} : {} entrées : {lignes:?}", lignes.len());
            assert!(lignes.len() >= 9, "{origine:?} : {lignes:?}");
        }
    }
}

#[test]
fn the_groups_come_in_this_order_with_separators() {
    let mut app = app_layers("ordre", &[LayerKind::Bitmap, LayerKind::Vector]);
    let mut ecran = Ecran::new();
    let textes = menu(&mut app, &mut ecran, Origine::Panneau, 1);
    // (« Réutiliser dans… », la planche-mère, partie 126, est dans le groupe des gestes, après Dupliquer.)
    let ordre = ["Nouveau calque", "Dupliquer", "Réutiliser dans", "Renommer", "Supprimer", "Fusionner vers le bas", "Fusionner les", "Organiser", "Apparence", "Effets", "Ajouter un aplat", "Plus"];
    let mut dernier = f32::NEG_INFINITY;
    let mut ys = Vec::new();
    for mot in ordre {
        let (_, r) = trouver(&textes, mot).unwrap_or_else(|| panic!("« {mot} » manque : {:?}", premier_niveau(&textes)));
        assert!(r.top() > dernier, "« {mot} » est dans l'ordre : {:?}", premier_niveau(&textes));
        dernier = r.top();
        ys.push(r.center().y);
    }
    // Les séparateurs : un espace plus grand entre les groupes (gestes | fusion | organisation | reste).
    let ecart = |a: usize, b: usize| ys[b] - ys[a];
    let pas = ecart(0, 1);
    assert!(ecart(4, 5) > pas + 2.0 && ecart(6, 7) > pas + 2.0 && ecart(10, 11) > pas + 2.0, "des séparateurs entre les groupes : {ys:?}");
    assert!((ecart(0, 1) - ecart(1, 2)).abs() < 3.0 && (ecart(2, 3) - pas).abs() < 3.0, "pas de séparateur dans le groupe des gestes : {ys:?}");
}

#[test]
fn the_menu_is_the_same_wherever_a_layer_menu_opens() {
    let mut vues: Vec<(Origine, Vec<String>)> = Vec::new();
    for origine in [Origine::Panneau, Origine::Timeline, Origine::Montage, Origine::Barre] {
        let mut app = app_layers("partout", &[LayerKind::Bitmap, LayerKind::Vector]);
        let mut ecran = Ecran::new();
        vues.push((origine, premier_niveau(&menu(&mut app, &mut ecran, origine, 0))));
    }
    for (o, v) in &vues[1..] {
        assert_eq!(v, &vues[0].1, "{o:?} comme {:?}", vues[0].0);
    }
    // Et les menus publics qui y mènent (le « … », le clic droit, le ⋮, le menu Calques) sont ces mêmes menus.
    let mut app = app_layers("partout2", &[LayerKind::Bitmap, LayerKind::Vector]);
    let mut ecran = Ecran::new();
    let mut points: Vec<(&str, Vec<String>)> = Vec::new();
    {
        let mut f: Contenu = &mut |a: &mut App, ui: &mut egui::Ui| a.layer_ops_menu(ui);
        points.push(("menu Calques", premier_niveau(&ecran.chauffer(&mut app, 5, &mut f))));
    }
    {
        let mut f: Contenu = &mut |a: &mut App, ui: &mut egui::Ui| a.layer_more_menu(ui);
        points.push(("« … »", premier_niveau(&ecran.chauffer(&mut app, 5, &mut f))));
    }
    {
        let mut f: Contenu = &mut |a: &mut App, ui: &mut egui::Ui| a.layer_full_menu(ui, 0, false);
        points.push(("clic droit du panneau", premier_niveau(&ecran.chauffer(&mut app, 5, &mut f))));
    }
    {
        let mut f: Contenu = &mut |a: &mut App, ui: &mut egui::Ui| a.layer_head_menu(ui, 0);
        points.push(("clic droit de la timeline", premier_niveau(&ecran.chauffer(&mut app, 5, &mut f))));
    }
    {
        let mut f: Contenu = &mut |a: &mut App, ui: &mut egui::Ui| a.montage_layer_menu(ui, 0);
        points.push(("clic droit du montage", premier_niveau(&ecran.chauffer(&mut app, 5, &mut f))));
    }
    for (nom, v) in points {
        assert_eq!(v, vues[0].1, "{nom}");
    }
}

#[test]
fn a_folder_has_no_effects_or_aplat_entry_and_still_fits() {
    let mut app = app_layers("dossier", &[LayerKind::Bitmap]);
    app.layers_mut().push(Layer::new_folder("Rangement"));
    let mut ecran = Ecran::new();
    let textes = menu(&mut app, &mut ecran, Origine::Panneau, 1);
    let lignes = premier_niveau(&textes);
    assert!(!contient(&textes, "Effets") && !contient(&textes, "aplat"), "{lignes:?}");
    assert!(lignes.len() <= PREMIER_NIVEAU, "{lignes:?}");
}

// ----------------------------------------------------------------------------------- aucune commande perdue

/// Les commandes de l'ancien menu d'un calque (menu Calques, « … », clic droit du panneau, de la timeline, du montage).
const ANCIENNES: &[&str] = &[
    "calque.dupliquer", "calque.supprimer", "calque.couper", "calque.copier", "calque.coller", "calque.coller_remplacer", "calque.miroir_h", "calque.miroir_v",
    "calque.fusionner_dessous", "calque.fusionner_choisis", "calque.fusionner_visibles", "calque.fusionner_tout", "pinceau.fond", "fond.calques_choisis",
    "fond.tout_le_plan", "fond.retirer_choisis", "fond.couleur_partout", "calque.propager", "calque.colorier_zones", "calque.monter", "calque.descendre",
    "calque.dessus", "calque.dessous", "calque.premier", "calque.dernier", "groupes.editer", "groupes.focus", "calque.choisir_groupe", "calque.choisir_autres_groupes",
    "dossier.nouveau", "dossier.ranger", "dossier.deposer", "dossier.sortir", "dossier.joindre", "dossier.supprimer_seul", "calque.choisir_tout",
    "calque.choisir_visibles", "calque.choisir_aucun", "calque.tous_visibles", "calque.tous_caches", "calque.courant_seul", "calque.pochoirs_eteindre",
    "calque.pochoirs_inverser", "calque.supprimer_vides", "calque.supprimer_caches", "calque.verrouiller", "calque.effets",
];

#[test]
fn every_command_of_the_old_menus_is_still_reachable() {
    // Chaque commande est nommée dans le texte d'un des menus (ce nouveau menu, ou les sous-menus qu'il ouvre), et reste dans la
    // palette (Ctrl+K) : la liste des commandes l'a.
    let menus = [
        include_str!("menucalque.rs"),
        include_str!("groupsui.rs"),
        include_str!("fondui.rs"),
        include_str!("calquemenu.rs"),
        include_str!("folders.rs"),
        include_str!("fxui.rs"),
        include_str!("calquecouleurui.rs"),
        include_str!("propagerui.rs"),
        include_str!("coloriageui.rs"),
        include_str!("montagehead.rs"),
        include_str!("layersui.rs"),
        include_str!("layerheads.rs"),
    ]
    .join("\n");
    for id in ANCIENNES {
        assert!(menus.contains(&format!("\"{id}\"")), "{id} n'est plus dans aucun menu");
        assert!(crate::keymap::ACTIONS.iter().any(|a| a.id == *id), "{id} n'est plus une commande de la palette");
    }
    // Les commandes de chaque ligne d'ancien menu sans identifiant (réglages) sont toujours là : leur libellé est écrit.
    let src = include_str!("menucalque.rs");
    for libelle in [
        "Afficher le calque", "Opacité", "Mode de fusion", "Couleur d'étiquette", "Pelure d'oignon du calque", "Préserver la transparence", "Pochoir négatif",
        "Verrouiller la position", "Animation (avant, après, automatismes)", "Calque vidéo", "Calque de couleur", "Réduire la ligne", "Agrandir la ligne",
    ] {
        assert!(src.contains(libelle), "« {libelle} » manque");
    }
    // Les menus de réglages que l'ancien montage avait en sous-menu.
    assert!(src.contains("depth_item") && src.contains("group_menu") && src.contains("behaviour_items") && src.contains("pistes_menu"));
}

#[test]
fn the_submenus_hold_what_was_taken_off_the_first_level() {
    let mut app = app_layers("sous-menus", &[LayerKind::Bitmap, LayerKind::Vector]);
    // (Un écran neuf par sous-menu : le survol d'un second sous-menu n'ouvre rien tant que le premier est ouvert.)
    for (sous, attendus) in [
        ("Organiser", &["Monter le calque", "Descendre le calque", "Aller au calque", "Dossiers", "Groupes"][..]),
        ("Apparence", &["Afficher le calque", "Verrouiller le calque", "Opacité", "Mode de fusion", "Couleur d'étiquette", "Pelure d'oignon"]),
        ("Effets", &["Effets du calque", "Tremblement du trait"]),
        ("Plus", &["Copier, coller", "Retourner le calque", "Réglages avancés", "Calque de couleur", "Choisir", "Afficher", "Fusionner plus de calques", "Pochoirs", "Nettoyer", "Aplat sur plusieurs calques"]),
    ] {
        let mut ecran = Ecran::new();
        let mut contenu = |a: &mut App, ui: &mut egui::Ui| a.layer_menu(ui, 1, Origine::Panneau);
        let textes = ecran.chauffer(&mut app, 5, &mut contenu);
        let (_, r) = trouver(&textes, sous).unwrap_or_else(|| panic!("{sous}")).clone();
        // (Posé à plat dans la zone, le sous-menu s'ouvre au clic ; dans un clic droit, au survol.)
        let ouvert = ecran.cliquer(&mut app, r.center(), &mut contenu);
        for a in attendus {
            if sous == "Effets" {
                // (Le sous-menu des effets s'ouvre avec ses cases ; l'interrupteur n'y est que si le calque a des effets.)
                continue;
            }
            assert!(contient(&ouvert, a), "« {a} » dans « {sous} » : {:?}", ouvert.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>());
        }
    }
}

#[test]
fn the_merge_group_is_visible_and_works_from_the_menu() {
    let mut app = app_layers("fusion", &[LayerKind::Bitmap, LayerKind::Vector]);
    app.cur_layer = 1;
    let mut ecran = Ecran::new();
    let mut contenu = |a: &mut App, ui: &mut egui::Ui| a.layer_menu(ui, 1, Origine::Panneau);
    let textes = ecran.chauffer(&mut app, 5, &mut contenu);
    // Le raccourci est écrit sur la ligne (Ctrl+E) : il est libre dans le fichier de raccourcis.
    let (_, bas) = trouver(&textes, "Fusionner vers le bas").unwrap().clone();
    assert!(textes.iter().any(|(t, r)| t.contains("Ctrl+E") && (r.center().y - bas.center().y).abs() < 4.0), "{:?}", premier_niveau(&textes));
    ecran.cliquer(&mut app, bas.center(), &mut contenu);
    assert_eq!(app.layers().len(), 1, "{}", app.status);
    assert!(app.status.contains("fusionnés") && app.status.contains("bitmap"), "{}", app.status);
}

#[test]
fn a_refused_merge_is_greyed_with_its_reason_in_the_tooltip() {
    let mut app = app_layers("grise", &[LayerKind::Bitmap, LayerKind::Bitmap]);
    app.layers_mut()[0].visible = false;
    app.cur_layer = 1;
    let mut ecran = Ecran::new();
    let mut contenu = |a: &mut App, ui: &mut egui::Ui| a.layer_menu(ui, 1, Origine::Panneau);
    let textes = ecran.chauffer(&mut app, 5, &mut contenu);
    let (_, bas) = trouver(&textes, "Fusionner vers le bas").unwrap().clone();
    // Un clic ne fait rien (l'entrée est grisée)…
    ecran.cliquer(&mut app, bas.center(), &mut contenu);
    assert_eq!(app.layers().len(), 2);
    // … et la bulle dit pourquoi.
    let vus = ecran.survoler(&mut app, bas.center(), &mut contenu, 80);
    assert!(contient(&vus, "est caché"), "{:?}", vus.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>());
    // Les calques choisis : « Choisis plusieurs calques » tant qu'un seul l'est.
    let (_, choisis) = trouver(&textes, "Fusionner les calques choisis").unwrap().clone();
    let vus = ecran.survoler(&mut app, choisis.center(), &mut contenu, 80);
    assert!(contient(&vus, "Ctrl + clic"), "{:?}", vus.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>());
}

#[test]
fn the_shortcut_of_the_merge_is_free_in_the_keymap_file() {
    // Ctrl+E : « Fusionner vers le bas » (déjà posé dans keymap.toml), et aucune autre commande de calque ne le porte.
    let app = app_layers("touche", &[LayerKind::Bitmap]);
    assert_eq!(app.keymap.label("calque.fusionner_dessous"), "Ctrl+E");
    for a in crate::keymap::ACTIONS.iter().filter(|a| a.id != "calque.fusionner_dessous") {
        assert_ne!(app.keymap.label(a.id), "Ctrl+E", "{} porte aussi Ctrl+E", a.id);
    }
}
