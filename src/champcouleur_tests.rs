//! La pipette des champs de couleur (`champcouleur.rs`, Anthony, 2026-10-28) : une seule enveloppe pour tous les choix
//! de couleur de SHOT ; le geste (une seule capture d'écran, le pixel visé écrit dans CE champ) ; Échap et le clic droit
//! l'annulent ; le clic de la pipette ne pose aucun trait et n'active rien dessous ; une seule étape d'annulation ;
//! sans capture, la couleur du dessin sous le curseur ; la couleur de dessin et la pipette de l'outil ne bougent pas.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Context, Event, Id, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Sense, Ui, UserData, ViewportCommand, ViewportId};

use crate::app::{App, Tool};
use crate::champcouleur;
use crate::document::{Document, Layer, LayerKind};
use crate::perf_tests::{banc_taille, Banc};

const ECRAN: [f32; 2] = [600.0, 400.0];
const ROUGE: [u8; 3] = [200, 10, 10];
const BLEU: [u8; 3] = [10, 20, 220];
const VERT: [u8; 3] = [10, 220, 30];

// ------------------------------------------------------------------------------------ un petit banc d'egui

/// Une fenêtre d'egui avec un champ de couleur, un « dessin » dessous (qui compte les appuis) et un bouton dessous.
struct Terrain<T> {
    ctx: Context,
    t: f64,
    /// Les demandes de capture d'écran vues dans la sortie d'egui.
    captures: usize,
    /// Les clics que `surveiller` a renvoyés à l'application (pas de capture), et ce qu'elle répond.
    repli_demande: Vec<Pos2>,
    repli_reponse: Option<[u8; 3]>,
    e: T,
    contenu: fn(&mut T, &Context, &mut Ui),
}

impl<T> Terrain<T> {
    fn new(e: T, contenu: fn(&mut T, &Context, &mut Ui)) -> Self {
        Terrain { ctx: Context::default(), t: 0.0, captures: 0, repli_demande: Vec::new(), repli_reponse: None, e, contenu }
    }

    fn image(&mut self, events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let mut entree = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(ECRAN[0], ECRAN[1]))), time: Some(self.t), events, ..Default::default() };
        // Ce que fait `App::raw_input_hook`.
        champcouleur::filtrer_entree(&self.ctx, &mut entree);
        let ctx = self.ctx.clone();
        let (e, contenu, repli_demande, repli_reponse) = (&mut self.e, self.contenu, &mut self.repli_demande, self.repli_reponse);
        let sortie = ctx.run(entree, |ctx| {
            // Ce que fait `App::frame`.
            if let Some(pos) = champcouleur::surveiller(ctx) {
                repli_demande.push(pos);
                champcouleur::fournir_repli(ctx, repli_reponse);
            }
            egui::CentralPanel::default().show(ctx, |ui| contenu(e, ctx, ui));
        });
        self.captures += sortie.viewport_output.get(&ViewportId::ROOT).map_or(0, |v| v.commands.iter().filter(|c| matches!(c, ViewportCommand::Screenshot(_))).count());
    }

    fn chauffer(&mut self, n: usize) {
        for _ in 0..n {
            self.image(Vec::new());
        }
    }

    fn bouton(&mut self, pos: Pos2, bouton: PointerButton, pressed: bool) {
        self.image(vec![Event::PointerButton { pos, button: bouton, pressed, modifiers: Modifiers::NONE }]);
    }

    fn cliquer_avec(&mut self, pos: Pos2, bouton: PointerButton) {
        self.image(vec![Event::PointerMoved(pos)]);
        self.chauffer(2);
        self.bouton(pos, bouton, true);
        self.bouton(pos, bouton, false);
        self.chauffer(2);
    }

    fn cliquer(&mut self, pos: Pos2) {
        self.cliquer_avec(pos, PointerButton::Primary);
    }

    /// Ce que l'écran a montré à la carte graphique, rendu par egui à l'image suivante.
    fn livrer_capture(&mut self, image: ColorImage) {
        self.image(vec![Event::Screenshot { viewport_id: ViewportId::ROOT, user_data: UserData::default(), image: Arc::new(image) }]);
    }

    fn popup_ouvert(&self) -> bool {
        self.ctx.memory(|m| m.any_popup_open())
    }

    fn bouton_pipette(&self) -> Rect {
        self.ctx.data(|d| d.get_temp::<Rect>(Id::new("champcouleur_essai_bouton"))).expect("la fenêtre de choix montre sa pipette")
    }
}

struct Champ {
    couleur: [u8; 3],
    changements: usize,
    case: Rect,
    dessin: Rect,
    appuis_dessin: usize,
    clics_bouton: usize,
}

impl Default for Champ {
    fn default() -> Self {
        Champ { couleur: [0, 0, 0], changements: 0, case: Rect::NOTHING, dessin: Rect::NOTHING, appuis_dessin: 0, clics_bouton: 0 }
    }
}

fn champ_contenu(e: &mut Champ, _ctx: &Context, ui: &mut Ui) {
    let r = champcouleur::srgb(ui, &mut e.couleur);
    e.case = r.rect;
    if r.changed() {
        e.changements += 1;
    }
    // Un grand « dessin », en partie sous la fenêtre de choix, et un bouton dessous.
    let (rect, reponse) = ui.allocate_exact_size(vec2(560.0, 200.0), Sense::click_and_drag());
    e.dessin = rect;
    if reponse.is_pointer_button_down_on() || reponse.clicked() || reponse.drag_started() {
        e.appuis_dessin += 1;
    }
    if ui.button("dessous").clicked() {
        e.clics_bouton += 1;
    }
}

/// Une capture de 1200 × 800 pixels pour l'écran de 600 × 400 points : rouge à gauche de x = 50 points, `droite` ailleurs.
fn capture(droite: [u8; 3]) -> ColorImage {
    let mut image = ColorImage::new([1200, 800], Color32::from_rgb(ROUGE[0], ROUGE[1], ROUGE[2]));
    for y in 0..800 {
        for x in 100..1200 {
            image.pixels[y * 1200 + x] = Color32::from_rgb(droite[0], droite[1], droite[2]);
        }
    }
    image
}

/// Un terrain avec le choix de couleur ouvert et la pipette armée.
fn arme() -> Terrain<Champ> {
    let mut t = Terrain::new(Champ { couleur: [1, 2, 3], ..Default::default() }, champ_contenu);
    t.chauffer(3);
    let case = t.e.case;
    t.cliquer(case.center());
    assert!(t.popup_ouvert(), "un clic sur la case ouvre la fenêtre de choix");
    t.chauffer(2);
    let pipette = t.bouton_pipette();
    t.cliquer(pipette.center());
    assert!(champcouleur::armee(&t.ctx), "un clic sur la pipette arme le geste");
    assert!(t.popup_ouvert(), "la fenêtre de choix reste ouverte");
    t
}

// ------------------------------------------------------------------------------------ le geste

#[test]
fn the_pipette_takes_the_pixel_under_the_click_from_one_capture_and_writes_only_this_field() {
    let mut t = arme();
    // (Quelques images, puis la capture : une seule demande, pas une par image.)
    t.chauffer(12);
    assert_eq!(t.captures, 1, "une seule capture d'écran demandée");
    t.livrer_capture(capture(BLEU));
    t.chauffer(15);
    assert_eq!(t.captures, 1, "pas une capture par image");
    assert!(champcouleur::armee(&t.ctx), "la pipette attend le clic");
    let (avant, appuis, boutons) = (t.e.changements, t.e.appuis_dessin, t.e.clics_bouton);
    // Un clic dans le « dessin », hors de la fenêtre de choix : le pixel bleu de la capture.
    let cible = pos2(t.e.dessin.left() + 450.0, t.e.dessin.top() + 100.0);
    t.cliquer(cible);
    assert_eq!(t.e.couleur, BLEU, "le pixel sous le clic, lu dans la capture");
    assert_eq!(t.e.changements, avant + 1, "le champ change une seule fois");
    assert_eq!(t.e.appuis_dessin, appuis, "le clic de la pipette ne touche pas le dessin dessous");
    assert_eq!(t.e.clics_bouton, boutons);
    assert!(!champcouleur::armee(&t.ctx), "le geste est fini");
    assert_eq!(t.captures, 1);
    assert!(t.popup_ouvert(), "la fenêtre de choix reste ouverte pour la suite");

    // Un second geste, avec un clic sur la fenêtre de choix elle-même : la couleur prise est celle de la capture, et le clic
    // n'a pas remué le carré de teinte.
    let pipette = t.bouton_pipette();
    t.cliquer(pipette.center());
    t.chauffer(12);
    assert_eq!(t.captures, 2, "une capture par geste");
    t.livrer_capture(capture(VERT));
    t.chauffer(3);
    let dans_la_fenetre = pos2(pipette.center().x + 20.0, pipette.center().y + 120.0);
    t.cliquer(dans_la_fenetre);
    assert_eq!(t.e.couleur, VERT, "le clic sur le carré de teinte ne le change pas : la pipette seule décide");
    assert_eq!(t.captures, 2);
}

#[test]
fn escape_cancels_the_gesture_and_leaves_the_field_and_the_window_alone() {
    let mut t = arme();
    t.chauffer(12);
    t.livrer_capture(capture(BLEU));
    t.chauffer(3);
    let avant = (t.e.couleur, t.e.changements);
    t.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    t.chauffer(3);
    assert!(!champcouleur::armee(&t.ctx), "Échap annule");
    assert_eq!((t.e.couleur, t.e.changements), avant, "rien n'est écrit");
    assert!(t.popup_ouvert(), "le premier Échap ne sert qu'à la pipette : la fenêtre reste ouverte");
    // Plus rien n'est retiré de l'entrée : un clic sur le dessin est un vrai clic.
    let cible = pos2(t.e.dessin.left() + 450.0, t.e.dessin.top() + 100.0);
    let appuis = t.e.appuis_dessin;
    t.cliquer(cible);
    assert!(t.e.appuis_dessin > appuis, "après l'annulation, les clics redeviennent normaux");
    assert_eq!(t.e.couleur, avant.0);
}

#[test]
fn a_right_click_cancels_too() {
    let mut t = arme();
    t.chauffer(12);
    t.livrer_capture(capture(BLEU));
    t.chauffer(3);
    let avant = (t.e.couleur, t.e.changements, t.e.appuis_dessin);
    t.cliquer_avec(pos2(t.e.dessin.left() + 450.0, t.e.dessin.top() + 100.0), PointerButton::Secondary);
    assert!(!champcouleur::armee(&t.ctx));
    assert_eq!((t.e.couleur, t.e.changements, t.e.appuis_dessin), avant, "ni couleur ni appui ailleurs");
}

#[test]
fn an_ordinary_click_on_the_drawing_is_seen_without_the_pipette() {
    // (La preuve que le banc voit les appuis : sans pipette, le « dessin » en compte.)
    let mut t = Terrain::new(Champ::default(), champ_contenu);
    t.chauffer(3);
    t.cliquer(pos2(t.e.dessin.left() + 450.0, t.e.dessin.top() + 100.0));
    assert!(t.e.appuis_dessin >= 1);
}

#[test]
fn without_a_capture_the_click_asks_the_application_for_the_colour_under_the_cursor() {
    let mut t = arme();
    t.repli_reponse = Some([9, 8, 7]);
    // Aucune capture n'arrive (affichage de secours) : le clic demande à l'application.
    t.chauffer(12);
    let cible = pos2(t.e.dessin.left() + 450.0, t.e.dessin.top() + 100.0);
    t.cliquer(cible);
    assert_eq!(t.repli_demande.len(), 1, "une seule demande, à la position du clic");
    assert!((t.repli_demande[0] - cible).length() < 1.0);
    assert_eq!(t.e.couleur, [9, 8, 7]);
    assert!(!champcouleur::armee(&t.ctx));
    assert_eq!(t.e.appuis_dessin, 0);

    // L'application ne trouve rien à cet endroit : le geste s'arrête, le champ garde sa couleur.
    let pipette = t.bouton_pipette();
    t.repli_reponse = None;
    t.cliquer(pipette.center());
    t.chauffer(12);
    t.cliquer(cible);
    assert_eq!(t.repli_demande.len(), 2);
    assert_eq!(t.e.couleur, [9, 8, 7]);
    assert!(!champcouleur::armee(&t.ctx));
}

#[test]
fn the_capture_maps_screen_points_to_its_own_pixels() {
    let ecran = Rect::from_min_size(pos2(0.0, 0.0), vec2(2.0, 1.0));
    let mut image = ColorImage::new([4, 2], Color32::BLACK);
    image.pixels[1 * 4 + 3] = Color32::from_rgb(1, 2, 3); // pixel (3, 1) : le coin bas droit
    image.pixels[0] = Color32::from_rgb(9, 9, 9);
    assert_eq!(champcouleur::pixel_sous(&image, pos2(1.9, 0.9), ecran), Some([1, 2, 3]));
    assert_eq!(champcouleur::pixel_sous(&image, pos2(2.0, 1.0), ecran), Some([1, 2, 3]), "le bord tombe sur le dernier pixel");
    assert_eq!(champcouleur::pixel_sous(&image, pos2(0.1, 0.1), ecran), Some([9, 9, 9]));
    assert_eq!(champcouleur::pixel_sous(&image, pos2(-3.0, 0.5), ecran), None, "hors de l'écran : rien");
    assert_eq!(champcouleur::pixel_sous(&image, pos2(f32::NAN, 0.5), ecran), None);
}

// ------------------------------------------------------------------------------------ une seule enveloppe

#[test]
fn every_colour_field_goes_through_the_one_wrapper() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let (mut fautifs, mut appels) = (Vec::new(), 0);
    for entree in std::fs::read_dir(dir).expect("dossier src") {
        let chemin = entree.expect("fichier").path();
        let nom = chemin.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !nom.ends_with(".rs") || nom.ends_with("_tests.rs") {
            continue;
        }
        let texte = std::fs::read_to_string(&chemin).expect("source lisible");
        if nom == "champcouleur.rs" {
            continue;
        }
        appels += texte.matches("champcouleur::srgb").count() + texte.matches("champcouleur::srgba").count();
        for interdit in ["color_edit_button", "color_picker_color32", "color_picker_hsva_2d", "color_edit_button_hsva"] {
            for (n, ligne) in texte.lines().enumerate() {
                // (Un commentaire peut en parler.)
                if ligne.contains(interdit) && !ligne.trim_start().starts_with("//") {
                    fautifs.push(format!("{nom}:{}", n + 1));
                }
            }
        }
    }
    assert!(appels >= 20, "seulement {appels} champs passent par l'enveloppe : la recherche ne les voit plus");
    assert!(fautifs.is_empty(), "des choix de couleur d'egui hors de `champcouleur.rs` (ils n'ont pas de pipette) : {fautifs:?}");
}

// ------------------------------------------------------------------------------------ une seule étape d'annulation

fn app_essai(nom: &str) -> App {
    let dir = std::env::temp_dir().join(format!("shot-champcouleur-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::create_dir_all(&dir);
    crate::donnees::bac_a_sable(&dir);
    let ctx = Context::default();
    let mut app = App::sans_fenetre(&ctx, None);
    app.welcome = false;
    app
}

struct ChampDuProjet {
    app: App,
    case: Rect,
}

fn champ_du_projet(e: &mut ChampDuProjet, _ctx: &Context, ui: &mut Ui) {
    // (Comme « Couleur A » du panneau Plans : un champ qui appartient au projet.)
    let mut a = e.app.doc.paper;
    let reponse = champcouleur::srgb(ui, &mut a);
    e.case = reponse.rect;
    if e.app.edited(&reponse) {
        e.app.doc.paper = a;
    }
}

#[test]
fn a_pick_in_a_project_field_is_one_undo_step() {
    let mut t = Terrain::new(ChampDuProjet { app: app_essai("annulation"), case: Rect::NOTHING }, champ_du_projet);
    t.e.app.doc.paper = [1, 2, 3];
    t.chauffer(3);
    let etapes = |t: &Terrain<ChampDuProjet>| t.e.app.history.names().0.len();
    let avant = etapes(&t);
    let case = t.e.case;
    t.cliquer(case.center());
    t.chauffer(2);
    let pipette = t.bouton_pipette();
    t.cliquer(pipette.center());
    t.chauffer(12);
    t.livrer_capture(capture(BLEU));
    t.chauffer(3);
    t.cliquer(pos2(450.0, 380.0));
    assert_eq!(t.e.app.doc.paper, BLEU, "le champ du projet a la couleur prise");
    assert_eq!(etapes(&t), avant + 1, "une seule étape d'annulation pour toute la prise");
    // Annuler rend la couleur d'avant.
    t.e.app.undo();
    assert_eq!(t.e.app.doc.paper, [1, 2, 3]);
}

// ------------------------------------------------------------------------------------ dans l'application

/// Un plan 400 × 300 : un calque bitmap moitié rouge (à gauche), moitié bleu, à l'écran.
fn banc_dessin() -> Banc {
    use crate::raster::TILE;
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers = vec![Layer::new("Dessin", LayerKind::Bitmap)];
    doc.panels[0].length = 4;
    let id = doc.new_cel(LayerKind::Bitmap);
    for k in 0..4 {
        doc.panels[0].layers[0].set(k, Some(id));
    }
    let r = &mut doc.cels.get_mut(&id).expect("le dessin").raster;
    for y in 0..300 {
        for x in 0..400 {
            let c = if x < 200 { Color32::from_rgb(220, 0, 0) } else { Color32::from_rgb(0, 0, 220) };
            let idx = r.tile_index_of(x, y);
            r.tile_mut(idx)[(y % TILE) * TILE + x % TILE] = c;
        }
    }
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(crate::app::Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.tool = Tool::Brush;
    b.app.color = [0, 255, 0];
    b.app.brush.size = 20.0;
    // Comme la vraie fenêtre : les événements passent par `raw_input_hook`.
    b.crochet = true;
    b.chauffer(10);
    assert!(b.app.canvas_rect.width() > 200.0, "le dessin est à l'écran : {:?}", b.app.canvas_rect);
    b
}

fn sur_le_dessin(b: &Banc, x: f32, y: f32) -> Pos2 {
    b.app.view.to_screen(b.app.canvas_rect, b.app.canvas_size(), pos2(x, y))
}

fn glisser(b: &mut Banc, de: Pos2, a: Pos2) {
    let bouton = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![bouton(de, true)], 1.0 / 60.0);
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(a, false)], 1.0 / 60.0);
    b.chauffer(3);
}

fn pixel_vert_pose(b: &Banc) -> bool {
    match b.app.layers()[0].frames.first().copied().flatten() {
        Some(cel) => (0..300).step_by(3).any(|y| (0..400).step_by(3).any(|x| b.app.doc.cels[&cel].raster.get(x, y) == Color32::from_rgb(0, 255, 0))),
        None => false,
    }
}

#[test]
fn the_pipette_click_on_the_drawing_lays_no_stroke_and_keeps_the_drawing_colour() {
    // Sans pipette : le même geste pose un trait (la preuve que le banc dessine).
    let mut b = banc_dessin();
    let (de, a) = (sur_le_dessin(&b, 60.0, 150.0), sur_le_dessin(&b, 140.0, 150.0));
    let avant = b.app.history.names().0.len();
    glisser(&mut b, de, a);
    assert!(b.app.history.names().0.len() > avant && pixel_vert_pose(&b), "sans pipette, le geste pose un trait");

    // Avec la pipette armée : aucun trait, aucune étape, la couleur de dessin et l'outil restent.
    let mut b = banc_dessin();
    let cible = Id::new("champ d'essai");
    champcouleur::armer(&b.ctx, cible);
    b.chauffer(3);
    let (etapes, outil) = (b.app.history.names().0.len(), b.app.tool);
    // (Un clic à gauche du dessin, dans la partie rouge : la capture manque dans l'essai, c'est le repli qui répond.)
    let (de, a) = (sur_le_dessin(&b, 60.0, 150.0), sur_le_dessin(&b, 140.0, 150.0));
    let bouton = |pos, pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(Vec::new(), 1.0 / 60.0);
    b.image(vec![bouton(de, true)], 1.0 / 60.0);
    // Le champ a reçu le pixel du dessin sous le curseur (le repli de l'application, sans capture), dès l'image de l'appui.
    assert_eq!(champcouleur::prise_pour(&b.ctx, cible), Some([220, 0, 0]));
    for k in 1..=6 {
        b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / 6.0))], 1.0 / 60.0);
    }
    b.image(vec![bouton(a, false)], 1.0 / 60.0);
    b.chauffer(3);
    assert_eq!(b.app.history.names().0.len(), etapes, "aucune étape d'annulation");
    assert!(!pixel_vert_pose(&b), "aucun trait n'est posé");
    assert_eq!(b.app.color, [0, 255, 0], "la couleur de dessin ne change pas");
    assert_eq!(b.app.tool, outil);
    assert!(!champcouleur::armee(&b.ctx), "le clic a terminé le geste");

    // Et le geste fini, l'application dessine de nouveau.
    glisser(&mut b, de, a);
    assert!(pixel_vert_pose(&b), "après la pipette, les traits reviennent");
}

#[test]
fn escape_in_the_application_cancels_the_pipette_before_any_shortcut_sees_it() {
    let mut b = banc_dessin();
    champcouleur::armer(&b.ctx, Id::new("champ d'essai"));
    b.chauffer(3);
    assert!(champcouleur::armee(&b.ctx));
    b.image(vec![Event::Key { key: Key::Escape, physical_key: Some(Key::Escape), pressed: true, repeat: false, modifiers: Modifiers::NONE }], 1.0 / 60.0);
    b.chauffer(2);
    assert!(!champcouleur::armee(&b.ctx));
    assert_eq!(champcouleur::prise_pour(&b.ctx, Id::new("champ d'essai")), None);
    assert_eq!(b.app.color, [0, 255, 0]);
}
