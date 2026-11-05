//! L'aplat « fonctionne parfaitement sans créer de problèmes » (le réalisateur, V1, 2026-11-05, branche `aplat-fiable`) : les
//! combinaisons qui cassaient, ou qu'aucun essai ne couvrait (partie 103 de docs/INTERFACE.md).

use eframe::egui::{pos2, Color32, Pos2};

use crate::app::{App, Tool};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::fond::{self, FondReglages};
use crate::perf_tests::{banc, se_reposer, Banc};
use crate::personnagesui::Choix;
use crate::render;

const BRUN: [u8; 3] = [140, 90, 70];
const TRAIT: [u8; 3] = [30, 20, 20];
const BLANC: [u8; 3] = [255, 255, 255];

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// Un calque vectoriel « Dessin » à l'aplat blanc, et une pastille « Lou » (trait sombre, aplat brun).
fn banc_dessin() -> Banc {
    crate::fxcache::monde::ecrire();
    let mut doc = Document::new(400, 300, 24);
    let mut l = Layer::new("Dessin", LayerKind::Vector);
    l.fond = Some(FondReglages { couleur: BLANC, debord: 3, trous: 6, bord: true, intelligent: false });
    doc.panels[0].layers[0] = l;
    let mut b = banc(doc, false);
    b.app.tool = Tool::Brush;
    let lou = b.app.perso_ajouter().expect("personnage");
    b.app.perso_ecrire(Choix::Personnage(lou), "Lou".into(), TRAIT, Some(BRUN));
    b.app.perso_choisir(Choix::Personnage(lou));
    b
}

fn polyligne(app: &mut App, points: &[(f32, f32)]) {
    let mut pts: Vec<Pos2> = Vec::new();
    for w in points.windows(2) {
        let (a, z) = (pos2(w[0].0, w[0].1), pos2(w[1].0, w[1].1));
        let n = (a.distance(z) / 4.0).ceil().max(1.0) as usize;
        for k in 0..n {
            pts.push(a + (z - a) * (k as f32 / n as f32));
        }
    }
    pts.push(pos2(points[points.len() - 1].0, points[points.len() - 1].1));
    app.begin_paint(pts[0], 1.0, false);
    for p in &pts[1..] {
        app.stroke_to(*p, 1.0);
    }
    app.end_stroke();
}

/// Un anneau carré fermé d'un seul trait (sans attendre le fil de fond).
fn anneau(app: &mut App, x0: f32, y0: f32, x1: f32, y1: f32) {
    polyligne(app, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0 - 8.0)]);
}

fn attendre(b: &mut Banc) {
    b.app.perso_attendre();
    se_reposer(b, 60);
    b.app.perso_attendre();
}

fn etapes(app: &App) -> usize {
    app.history.names().0.len()
}

fn le_dessin(app: &App) -> CelId {
    app.doc.panels[0].layers[0].frames.iter().flatten().next().copied().expect("le dessin")
}

/// Combien de pixels de peinture d'aplat (strate) le dessin porte.
fn peinture(app: &App) -> usize {
    let cel = le_dessin(app);
    let Some(s) = app.doc.cels[&cel].fond.as_deref() else { return 0 };
    let r = &s.peinture.raster;
    (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).filter(|&(x, y)| r.get(x, y).a() > 0).count()
}

/// (pixels d'aplat plein hors du trait, ceux qui n'ont pas la couleur) vus comme à l'écran (`composite_pixel`).
fn aplat_hors_couleur(app: &App, attendue: [u8; 3]) -> (usize, usize) {
    let cel = le_dessin(app);
    let c = &app.doc.cels[&cel];
    let r = app.layers()[0].fond.expect("aplat");
    let couverture = fond::silhouette(&c.raster, &r);
    let plan = render::panel_plan_at(&app.doc, 0, 0, None);
    let (w, h) = (c.raster.width, c.raster.height);
    let (mut dedans, mut mauvais) = (0, 0);
    for y in 0..h {
        for x in 0..w {
            if couverture[y * w + x] == 255 && c.raster.get(x, y).a() == 0 {
                dedans += 1;
                if render::composite_pixel(&app.doc, &plan, x, y) != rgb(attendue) {
                    mauvais += 1;
                }
            }
        }
    }
    (dedans, mauvais)
}

// ------------------------------------------------------------------------ 1. le fond qui arrive tard

/// Un anneau de Lou en bas à droite, posé et attendu : le dessin existe déjà quand les traits suivants arrivent.
fn avec_un_premier_anneau() -> Banc {
    let mut b = banc_dessin();
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    assert!(peinture(&b.app) > 300, "le premier anneau a son aplat");
    b
}

#[test]
fn a_late_character_fill_joins_the_step_of_its_stroke_not_the_screen_alone() {
    let mut b = avec_un_premier_anneau();
    let base = peinture(&b.app);
    let avant = etapes(&b.app);
    // Un trait, puis tout de suite un autre geste ailleurs : le fond du premier arrive après le second.
    anneau(&mut b.app, 40.0, 40.0, 140.0, 140.0);
    anneau(&mut b.app, 30.0, 190.0, 110.0, 270.0);
    attendre(&mut b);
    assert_eq!(etapes(&b.app), avant + 2, "deux traits : deux étapes, le fond n'en ajoute pas");
    let complet = peinture(&b.app);
    assert!(complet > base + 600, "les trois anneaux ont leur aplat ({base} puis {complet})");
    // Annuler le dernier, puis l'autre : le dessin retrouve la peinture d'avant, sans résidu du fond arrivé tard.
    b.app.undo();
    b.app.undo();
    assert_eq!(peinture(&b.app), base, "tout annuler remet l'aplat comme avant les deux traits (aucun résidu)");
    // Tout rétablir rend tout.
    b.app.redo();
    b.app.redo();
    assert_eq!(peinture(&b.app), complet, "tout rétablir rend l'aplat des deux traits");
    let (dedans, mauvais) = aplat_hors_couleur(&b.app, BRUN);
    assert!(dedans > 1000 && mauvais == 0, "après annuler puis rétablir : aplat brun partout ({dedans}, {mauvais} autres)");
}

#[test]
fn a_character_fill_that_arrives_after_its_stroke_was_undone_comes_back_with_a_redo() {
    let mut b = avec_un_premier_anneau();
    let base = peinture(&b.app);
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    // Défait avant que le fond n'arrive.
    b.app.undo();
    attendre(&mut b);
    assert_eq!(peinture(&b.app), base, "le trait est défait : son fond n'a rien posé à l'écran");
    b.app.redo();
    let (dedans, mauvais) = aplat_hors_couleur(&b.app, BRUN);
    assert!(dedans > 1000 && mauvais == 0, "rétablir rend le trait avec son aplat brun ({dedans} pixels, {mauvais} autres)");
    assert!(peinture(&b.app) > base + 300);
    // Et un seul Ctrl+Z défait le tout.
    b.app.undo();
    assert_eq!(peinture(&b.app), base);
}

#[test]
fn a_new_stroke_after_an_undone_one_does_not_inherit_its_late_fill() {
    // Le premier anneau est sans pastille : le dessin existe, sans peinture d'aplat.
    let mut b = banc_dessin();
    let lou = b.app.doc.personnages[0].id;
    b.app.perso_lacher();
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    assert_eq!(peinture(&b.app), 0);
    b.app.perso_choisir(Choix::Personnage(lou));
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    anneau(&mut b.app, 30.0, 190.0, 110.0, 270.0);
    attendre(&mut b);
    b.app.undo();
    b.app.undo();
    // Sans pastille, au même endroit : l'aplat est celui du calque, pas un reste du brun du trait défait.
    b.app.perso_lacher();
    b.app.layers_mut()[0].fond.as_mut().expect("aplat").couleur = BLANC;
    assert_eq!(peinture(&b.app), 0, "tout annulé : aucune peinture d'aplat");
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    let cel = le_dessin(&b.app);
    let c = &b.app.doc.cels[&cel];
    let r = b.app.layers()[0].fond.expect("aplat");
    let couverture = fond::silhouette(&c.raster, &r);
    let plan = render::panel_plan_at(&b.app.doc, 0, 0, None);
    let (mut dedans, mut bruns) = (0, 0);
    for y in 0..170 {
        for x in 0..170 {
            if couverture[y * 400 + x] == 255 && c.raster.get(x, y).a() == 0 {
                dedans += 1;
                if render::composite_pixel(&b.app.doc, &plan, x, y) == rgb(BRUN) {
                    bruns += 1;
                }
            }
        }
    }
    assert!(dedans > 500, "il y a de l'aplat ({dedans})");
    assert_eq!(bruns, 0, "{bruns} pixels sur {dedans} gardent le brun du trait défait");
}
// ------------------------------------------------------------------------ 3. variantes de séquence, enregistrer, rouvrir

/// L'image d'un plan telle qu'on la voit (aplat, trait, calques) : un pixel sur trois, ce qui suffit à voir une différence d'aplat.
fn image(doc: &Document, plan: usize, frame: usize) -> Vec<Color32> {
    let p = render::panel_plan_at(doc, plan, frame, None);
    let mut out = Vec::new();
    for y in (0..doc.height).step_by(3) {
        for x in (0..doc.width).step_by(3) {
            out.push(render::composite_pixel(doc, &p, x, y));
        }
    }
    out
}

/// Combien de pixels diffèrent nettement (plus de 24 niveaux) entre deux images.
fn ecarts(a: &[Color32], b: &[Color32]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(p, q)| p.r().abs_diff(q.r()) > 24 || p.g().abs_diff(q.g()) > 24 || p.b().abs_diff(q.b()) > 24 || p.a().abs_diff(q.a()) > 24).count()
}

fn aplat_visible(doc: &Document, plan: usize, frame: usize, attendue: [u8; 3]) -> usize {
    image(doc, plan, frame).iter().filter(|c| **c == rgb(attendue)).count()
}

fn aller_retour(doc: &Document, nom: &str) -> (crate::project::Loaded, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("shot-aplat-fiable-{nom}-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("T.pelure");
    crate::project::save(doc, &manifest, &mut Default::default()).unwrap();
    (crate::project::load(&manifest).unwrap(), dir)
}

#[test]
fn an_aplat_survives_a_sequence_version_switch_and_a_save_then_reopen() {
    use crate::variantes::Nouvelle;
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    let seq = b.app.doc.sequence_of(0);
    let version_b_seule = image(&b.app.doc, 0, 0);
    assert!(aplat_visible(&b.app.doc, 0, 0, BRUN) > 300, "l'aplat du premier anneau est brun");
    // « Nouvelle version de séquence » par copie : le film reste ; la copie (A) est rangée.
    assert!(b.app.structure_op(|d, i| d.nouvelle_variante(seq, Nouvelle::Copier, 5).map(|_| i)));
    b.chauffer(3);
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &version_b_seule), 0, "la nouvelle version montre le même film");
    // Dans la version B, un deuxième anneau.
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    let b_deux = image(&b.app.doc, 0, 0);
    assert!(ecarts(&b_deux, &version_b_seule) > 100, "le deuxième anneau est là");
    // On revient à la version A : un seul anneau, son aplat intact.
    assert!(b.app.structure_op(|d, i| d.changer_de_variante(seq, 1).then_some(i)));
    b.chauffer(3);
    let a_vue = image(&b.app.doc, 0, 0);
    assert_eq!(ecarts(&a_vue, &version_b_seule), 0, "la version A est restée comme avant la copie (un anneau, aplat brun)");
    // Enregistrer, rouvrir : la version A est celle du film, la B rangée ; les deux gardent leur aplat.
    let (relu, dir) = aller_retour(&b.app.doc, "variantes");
    assert!(relu.warnings.is_empty(), "{:?}", relu.warnings);
    assert_eq!(ecarts(&image(&relu.doc, 0, 0), &version_b_seule), 0, "la version A rouverte montre son aplat");
    let mut doc = relu.doc;
    let seq2 = doc.sequence_of(0);
    assert!(doc.changer_de_variante(seq2, 2), "la version B se rouvre");
    assert_eq!(ecarts(&image(&doc, 0, 0), &b_deux), 0, "la version B rouverte montre ses deux aplats");
    std::fs::remove_dir_all(dir).ok();
}

// ------------------------------------------------------------------------ 2. « La clé » : la couleur suit

#[test]
fn a_stroke_without_pastille_follows_the_key_colour_of_its_drawing() {
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    // Le dessin est tout peint de brun (comme après « La clé »), alors que la couleur du calque est blanche.
    b.app.layers_mut()[0].fond.as_mut().expect("aplat").couleur = BLANC;
    b.app.perso_lacher();
    let avant = etapes(&b.app);
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    let (dedans, mauvais) = aplat_hors_couleur(&b.app, BRUN);
    assert!(dedans > 1000, "les deux anneaux ont de l'aplat ({dedans})");
    assert_eq!(mauvais, 0, "le second anneau a pris la couleur de la clé : {mauvais} pixels sur {dedans} n'ont pas le brun");
    assert_eq!(etapes(&b.app), avant + 1, "toujours une étape par trait");
    b.app.undo();
    assert!(aplat_hors_couleur(&b.app, BRUN).0 > 300 && aplat_hors_couleur(&b.app, BRUN).1 == 0, "annuler le second anneau garde le premier brun");
    b.app.redo();
    assert_eq!(aplat_hors_couleur(&b.app, BRUN).1, 0, "rétablir rend le second anneau brun");
}

#[test]
fn a_hand_painted_patch_is_not_a_key_colour() {
    let mut b = banc_dessin();
    b.app.perso_lacher();
    // Un anneau sans pastille : aplat blanc du calque. Un petit carré peint à la main dans la strate n'est pas une teinte.
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    let cel = le_dessin(&b.app);
    {
        let c = b.app.doc.cels.get_mut(&cel).expect("dessin");
        let (w, h) = (c.raster.width, c.raster.height);
        let s = c.fond.get_or_insert_with(|| Box::new(crate::document::Strate::new(w, h)));
        for y in 80..100 {
            for x in 80..100 {
                let id = s.peinture.raster.tile_index_of(x, y);
                s.peinture.raster.tile_mut(id)[(y % 64) * 64 + x % 64] = rgb([200, 30, 30]);
            }
        }
        s.peinture.version += 1;
        c.version += 1;
    }
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    let (dedans, _) = aplat_hors_couleur(&b.app, BLANC);
    assert!(dedans > 1000);
    let plan = render::panel_plan_at(&b.app.doc, 0, 0, None);
    assert_eq!(render::composite_pixel(&b.app.doc, &plan, 300, 200), rgb(BLANC), "le second anneau garde l'aplat du calque");
}

#[test]
fn a_pasted_key_keeps_the_colour_of_its_colour_layer() {
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    b.app.aplat_cible = None;
    b.app.cur_layer = 0;
    b.app.new_colour_layer();
    b.app.sync_couleurs();
    b.chauffer(3);
    let colour = b.app.cur_layer;
    let trait_li = (0..b.app.layers().len()).find(|&i| b.app.layers()[i].couleur.is_none()).expect("le calque de trait");
    let ccel = b.app.layers()[colour].frames[0].expect("la couleur suit le trait");
    {
        let c = b.app.doc.cels.get_mut(&ccel).expect("dessin de couleur");
        let id = c.raster.tile_index_of(100, 100);
        c.raster.tile_mut(id)[(100 % 64) * 64 + 100 % 64] = rgb([200, 30, 30]);
        c.version += 1;
    }
    b.app.cur_layer = trait_li;
    b.app.cur_frame = 0;
    b.app.copy_keys();
    b.app.cur_frame = 12;
    b.app.paste_keys_at_head();
    b.chauffer(3);
    let trait_colle = b.app.layers()[trait_li].frames[12].expect("la clé collée");
    assert_ne!(Some(trait_colle), b.app.layers()[trait_li].frames[0], "un dessin neuf ({})", b.app.status);
    let dessin = b.app.layers()[colour].couleur.as_ref().and_then(|c| c.dessins.get(&trait_colle)).copied().expect("la clé collée a son dessin de couleur");
    assert_eq!(b.app.doc.cels[&dessin].raster.get(100, 100), rgb([200, 30, 30]), "et il garde la couleur de la clé copiée");
    assert_ne!(Some(dessin), b.app.layers()[colour].frames[0], "un dessin de couleur à lui");
    // Annuler le collage retire tout, rétablir le rend.
    b.app.undo();
    assert_eq!(b.app.layers()[trait_li].frames[12], b.app.layers()[trait_li].frames[0]);
    b.app.redo();
    let colle = b.app.layers()[trait_li].frames[12].expect("rétabli");
    let d2 = b.app.layers()[colour].couleur.as_ref().and_then(|c| c.dessins.get(&colle)).copied().expect("couleur rétablie");
    assert_eq!(b.app.doc.cels[&d2].raster.get(100, 100), rgb([200, 30, 30]));
}
// ------------------------------------------------------------------------ 4. d'autres combinaisons

/// Le film de `banc_dessin` avec, sous le calque « Dessin » (à aplat), un calque bitmap « Bas » dont un trait croise l'anneau.
fn banc_deux_calques() -> Banc {
    let mut b = banc_dessin();
    // Le calque du bas : un trait de pinceau classique, sans pastille.
    b.app.doc.panels[0].layers.insert(0, Layer::new("Bas", LayerKind::Bitmap));
    b.app.cur_layer = 0;
    b.app.perso_lacher();
    polyligne(&mut b.app, &[(20.0, 100.0), (380.0, 100.0)]);
    b.chauffer(3);
    // Puis l'anneau de Lou, sur le calque du haut.
    b.app.cur_layer = 1;
    b.app.perso_choisir(Choix::Personnage(b.app.doc.personnages[0].id));
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    assert_eq!(b.app.doc.panels[0].layers.len(), 2);
    b
}

#[test]
fn merging_the_aplat_layer_down_keeps_the_picture_and_undo_redo_keep_it_too() {
    let mut b = banc_deux_calques();
    let avant = image(&b.app.doc, 0, 0);
    assert!(aplat_visible(&b.app.doc, 0, 0, BRUN) > 300);
    b.app.cur_layer = 1;
    b.app.merge_layer_down();
    se_reposer(&mut b, 120);
    b.app.fusion_attendre();
    se_reposer(&mut b, 60);
    assert_eq!(b.app.doc.panels[0].layers.len(), 1, "un seul calque");
    let fusionne = image(&b.app.doc, 0, 0);
    assert!(ecarts(&avant, &fusionne) < 40, "la fusion garde l'image ({} pixels changés)", ecarts(&avant, &fusionne));
    b.app.undo();
    assert_eq!(b.app.doc.panels[0].layers.len(), 2);
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &avant), 0, "annuler rend les deux calques et l'aplat");
    b.app.redo();
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &fusionne), 0, "rétablir rend la fusion");
}

#[test]
fn splitting_a_plan_then_joining_it_again_keeps_the_aplat_and_a_stroke_in_one_half_stays_there() {
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    let entier = image(&b.app.doc, 0, 0);
    assert!(b.app.structure_op_dessins(|d, i| d.split_panel(i, 12)));
    b.chauffer(3);
    assert_eq!(b.app.doc.panels.len(), 2);
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &entier), 0, "la première moitié garde l'aplat");
    assert_eq!(ecarts(&image(&b.app.doc, 1, 0), &entier), 0, "la deuxième moitié aussi");
    // Un anneau de plus dans la première moitié : la deuxième ne le montre pas.
    b.app.cur_panel = 0;
    b.app.cur_layer = 0;
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    assert_eq!(ecarts(&image(&b.app.doc, 1, 0), &entier), 0, "la deuxième moitié n'a pas reçu le trait de la première");
    assert!(ecarts(&image(&b.app.doc, 0, 0), &entier) > 100, "la première l'a");
    // Réunir, annuler, rétablir : le même dessin.
    b.app.cur_panel = 0;
    assert!(b.app.structure_op_dessins(|d, i| d.merge_with_next(i)));
    b.chauffer(3);
    let reuni = image(&b.app.doc, 0, 0);
    b.app.undo();
    assert_eq!(b.app.doc.panels.len(), 2);
    b.app.redo();
    assert_eq!(b.app.doc.panels.len(), 1);
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &reuni), 0, "annuler puis rétablir « réunir » rend le même dessin");
}

#[test]
fn a_pasted_aplat_layer_keeps_its_own_aplat_when_the_original_goes_away() {
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    attendre(&mut b);
    let avant = image(&b.app.doc, 0, 0);
    b.app.cur_layer = 0;
    b.app.copy_layer();
    b.app.paste_layer();
    assert_eq!(b.app.doc.panels[0].layers.len(), 2, "le calque est collé");
    // On retire l'original : la copie montre le même aplat (sa strate est la sienne).
    b.app.doc.panels[0].layers.remove(0);
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &avant), 0, "la copie a son aplat à elle");
    // Un trait sur la copie ne change plus rien chez personne d'autre : annuler, rétablir.
    b.app.cur_layer = 0;
    anneau(&mut b.app, 220.0, 150.0, 340.0, 260.0);
    attendre(&mut b);
    let deux = image(&b.app.doc, 0, 0);
    b.app.undo();
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &avant), 0, "annuler le trait de la copie rend l'état d'avant");
    b.app.redo();
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &deux), 0);
}

#[test]
fn undo_and_redo_in_every_order_never_leave_a_residue_of_aplat() {
    let mut b = avec_un_premier_anneau();
    let vide = image(&b.app.doc, 0, 0);
    let base = peinture(&b.app);
    for k in 0..3 {
        let dx = 20.0 + 70.0 * k as f32;
        anneau(&mut b.app, dx, 30.0, dx + 55.0, 120.0);
        if k != 1 {
            // (Un des traits n'attend pas : son fond arrive après le suivant.)
            attendre(&mut b);
        }
    }
    attendre(&mut b);
    let complet = image(&b.app.doc, 0, 0);
    let p_complet = peinture(&b.app);
    assert!(p_complet > base + 600, "les trois traits ont leur aplat");
    for _ in 0..3 {
        b.app.undo();
    }
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &vide), 0, "tout annulé : comme au départ");
    assert_eq!(peinture(&b.app), base, "aucun reste d'aplat");
    for _ in 0..3 {
        b.app.redo();
    }
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &complet), 0, "tout rétabli : comme à la fin");
    assert_eq!(peinture(&b.app), p_complet);
    // Dans le désordre.
    for geste in [true, true, false, true, false, false, true, false] {
        if geste {
            b.app.undo();
        } else {
            b.app.redo();
        }
    }
    assert_eq!(ecarts(&image(&b.app.doc, 0, 0), &complet), 0, "annuler et rétablir dans le désordre rend le même dessin");
    assert_eq!(peinture(&b.app), p_complet);
}
// ------------------------------------------------------------------------ 5. une commande juste après le trait

/// La peinture d'aplat (pixels) que portent les dessins du plan `plan` (la couleur du calque, elle, ne dit rien : c'est la peinture qui
/// porte la couleur d'un trait de personnage).
fn peinture_du_plan(doc: &Document, plan: usize) -> usize {
    let mut vus: Vec<CelId> = Vec::new();
    let mut n = 0;
    for l in &doc.panels[plan].layers {
        for c in l.frames.iter().flatten() {
            if vus.contains(c) {
                continue;
            }
            vus.push(*c);
            if let Some(s) = doc.cels.get(c).and_then(|c| c.fond.as_deref()) {
                let r = &s.peinture.raster;
                n += (0..r.height).flat_map(|y| (0..r.width).map(move |x| (x, y))).filter(|&(x, y)| r.get(x, y).a() > 0).count();
            }
        }
    }
    n
}

#[test]
fn a_command_right_after_a_stroke_takes_the_aplat_of_that_stroke_along() {
    use crate::variantes::Nouvelle;
    // Une version de séquence rangée juste après le trait (le fond n'est pas encore arrivé) : la version rangée l'a aussi.
    let mut b = avec_un_premier_anneau();
    let seq = b.app.doc.sequence_of(0);
    let base = peinture_du_plan(&b.app.doc, 0);
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    assert!(b.app.structure_op(|d, i| d.nouvelle_variante(seq, Nouvelle::Copier, 5).map(|_| i)));
    attendre(&mut b);
    let complet = peinture_du_plan(&b.app.doc, 0);
    assert!(complet > base + 300, "le film a la peinture du dernier trait ({base} puis {complet})");
    assert!(b.app.structure_op(|d, i| d.changer_de_variante(seq, 1).then_some(i)));
    b.chauffer(3);
    assert_eq!(peinture_du_plan(&b.app.doc, 0), complet, "la version rangée a aussi la peinture du dernier trait");

    // Couper le plan juste après le trait : les deux moitiés ont la peinture.
    let mut b = avec_un_premier_anneau();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    assert!(b.app.structure_op_dessins(|d, i| d.split_panel(i, 12)));
    attendre(&mut b);
    let (premiere, seconde) = (peinture_du_plan(&b.app.doc, 0), peinture_du_plan(&b.app.doc, 1));
    assert!(premiere > 1000, "la première moitié a la peinture ({premiere})");
    assert_eq!(seconde, premiere, "la deuxième moitié a la peinture du dernier trait aussi");
}

#[test]
fn saving_right_after_a_stroke_writes_the_aplat_of_that_stroke() {
    let mut b = avec_un_premier_anneau();
    let base = peinture_du_plan(&b.app.doc, 0);
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    let dir = std::env::temp_dir().join(format!("shot-aplat-fiable-enreg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert!(b.app.save_to_blocking(dir.join("essai.shot")), "{}", b.app.status);
    assert!(!b.app.modified, "rien n'arrive après l'enregistrement");
    let complet = peinture_du_plan(&b.app.doc, 0);
    assert!(complet > base + 300, "le trait a sa peinture ({base} puis {complet})");
    let relu = crate::project::load(&dir.join("essai.shot")).expect("rouvrir");
    assert_eq!(peinture_du_plan(&relu.doc, 0), complet, "le fichier a la peinture des deux anneaux");
    let _ = std::fs::remove_dir_all(&dir);
}
#[test]
fn a_late_fill_on_a_drawing_born_with_its_stroke_follows_the_merged_step() {
    // Le trait fait naître le dessin (case vide) : l'historique regroupe ses étapes en une ; le fond qui arrive après un Ctrl+Z
    // retrouve quand même cette étape (aucun reste dans le dessin défait), et revient avec le rétablissement.
    let mut b = banc_dessin();
    anneau(&mut b.app, 40.0, 40.0, 160.0, 160.0);
    b.app.undo();
    attendre(&mut b);
    assert!(b.app.doc.used_cels().iter().all(|c| b.app.doc.cels.get(c).is_none_or(|c| c.fond.as_deref().is_none_or(|s| s.peinture.raster.is_blank()))), "aucune peinture d'aplat ne reste dans le projet défait");
    b.app.redo();
    let (dedans, mauvais) = aplat_hors_couleur(&b.app, BRUN);
    assert!(dedans > 500 && mauvais == 0, "rétablir rend le trait avec son aplat ({dedans} pixels, {mauvais} autres)");
    b.app.undo();
    assert!(b.app.doc.used_cels().iter().all(|c| b.app.doc.cels.get(c).is_none_or(|c| c.fond.as_deref().is_none_or(|s| s.peinture.raster.is_blank()))));
}
