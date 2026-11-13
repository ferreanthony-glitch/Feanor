//! Essais des dessins posés à la main dans une trajectoire en clés d'images (`docs/INTERFACE.md`, partie 152). Anthony,
//! 2026-11-13 : « Une clé de dessin posée à la main entre deux marques est écrasée au recalcul suivant du chemin : une boîte
//! de dialogue demande si on écrase le dessin ou non. » ; puis : « Je préfère qu'on évite au maximum la boîte de dialogue.
//! Tout ce qui peut s'en passer s'en passera. »
//!
//! Ce cas s'en passe : la trajectoire ne remplace jamais une clé de dessin qu'elle n'a pas posée. La clé reste à son image,
//! avec son dessin, quoi qu'on fasse du chemin ; un jalon qui tomberait sur elle est posé sur l'image libre d'à côté ; la
//! barre d'état le dit une fois ; aucune fenêtre ne s'ouvre. Sur la feuille et dans l'espace (deux cartes choisies). De vrais
//! gestes de souris, la vraie commande « Nouvelle clé », de vrais traits de pinceau.

use eframe::egui::{pos2, Modifiers};

use crate::app::Tool;
use crate::document::CelId;
use crate::espace3d;
use crate::montagekeys::KeyRef;
use crate::perf_tests::{banc_taille, Banc};
use crate::project;
use crate::trajectoire::Cles;
use crate::trajectoire3d::Sujet;
use crate::trajectoire3d_images_tests as espace;
use crate::trajectoire3d_tests::{a_l_ecran, appuyer, geste, lacher, tracer_au_sol, trajectoire};
use crate::trajectoireui_tests as feuille;

// ------------------------------------------------------------------------------------------------ outils d'essai

/// Les dessins exposés par le calque `li` du plan courant, image par image.
fn exposes(b: &Banc, li: usize) -> Vec<Option<CelId>> {
    (0..b.app.frame_count()).map(|f| b.app.layers()[li].cel_at(f)).collect()
}

/// Les expositions du dessin `cel` sur le calque `li` : pour chacune, l'image où elle commence et celle où elle finit (exclue).
fn expositions(b: &Banc, li: usize, cel: CelId) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for (f, c) in exposes(b, li).iter().enumerate() {
        if *c == Some(cel) {
            match out.last_mut() {
                Some(d) if d.1 == f => d.1 = f + 1,
                _ => out.push((f, f + 1)),
            }
        }
    }
    out
}

/// Les images des jalons de la trajectoire du calque `li`.
fn jalons(b: &Banc, li: usize) -> Vec<usize> {
    b.app.layers()[li].trajectoire.as_ref().map(|t| t.images_des_jalons(b.app.frame_count())).unwrap_or_default()
}

/// Les dessins des jalons de la trajectoire du calque `li`.
fn des_jalons(b: &Banc, li: usize) -> Vec<CelId> {
    b.app.layers()[li].trajectoire.as_ref().map(|t| t.dessins.iter().map(|d| d.dessin).collect()).unwrap_or_default()
}

/// La dernière étape d'annulation.
fn etape(b: &Banc) -> String {
    b.app.history.names().0.last().map(|s| s.to_string()).unwrap_or_default()
}

/// **Une clé de dessin posée à la main** à l'image `image` du calque `li` : la tête de lecture y va, et la commande
/// « Nouvelle clé à l'image courante, copie du dessin » (F5) la pose. Rend son dessin, un dessin de plus.
fn nouvelle_cle(b: &mut Banc, li: usize, image: usize) -> CelId {
    let avant = b.app.layers()[li].cel_at(image);
    (b.app.cur_layer, b.app.cur_frame) = (li, image);
    b.chauffer(1);
    b.app.run_action("dessin.nouvelle_cle");
    b.chauffer(1);
    let cle = b.app.layers()[li].cel_at(image).expect("une clé de dessin à cette image");
    assert!(Some(cle) != avant, "la commande a posé une clé à l'image {} : {}", image + 1, b.app.status);
    assert_eq!(expositions(b, li, cle).first().map(|e| e.0), Some(image), "elle commence à cette image");
    cle
}

/// La clé posée à la main est toujours là : le dessin `cel` a une seule exposition sur le calque `li`, qui commence à son
/// image `image` ; et aucune fenêtre n'est ouverte.
#[track_caller]
fn toujours_a_son_image(b: &Banc, li: usize, cel: CelId, image: usize, quand: &str) {
    let e = expositions(b, li, cel);
    assert!(e.len() == 1 && e[0].0 == image, "{quand} : la clé posée à la main est à l'image {}, une seule fois : {e:?} ({})", image + 1, b.app.status);
    assert!(!b.app.modal_open(), "{quand} : aucune fenêtre ne s'est ouverte");
}

/// La phrase de la barre d'état pour un seul calque.
fn gardes_dit(n: usize, images: &str) -> String {
    let s = if n > 1 { "s" } else { "" };
    format!("Trajectoire : {n} dessin{s} posé{s} à la main gardé{s} entre les marques ({images}).")
}

/// « Temps égaux entre les marques », ou la sorte des clés : ce que font les boutons des réglages de la feuille.
fn regler(b: &mut Banc, nom: &str, f: impl Fn(&mut crate::trajectoire::Trajectoire)) {
    let snap = b.app.snapshot();
    f(b.app.layers_mut()[0].trajectoire.as_mut().expect("un chemin"));
    b.app.trajectoire_recalculer(0);
    b.app.commit_named(snap, nom);
    b.chauffer(2);
}

/// Supprime la clé de dessin de l'image `image` du calque `li` (la commande de la timeline) ; `vide` : ses images restent
/// vides, sinon la clé d'avant les reprend.
fn supprimer_la_cle(b: &mut Banc, li: usize, image: usize, vide: bool) {
    let (plan, calque) = (b.app.doc.panels[0].id, b.app.layers()[li].id);
    b.app.key_sel = vec![KeyRef { panel: plan, layer: calque, start: image }];
    b.app.delete_keys(vide);
    b.chauffer(1);
}

// ------------------------------------------------------------------------------------------------ sur la feuille

/// Le banc de la feuille (`trajectoireui_tests::banc`) : un calque vectoriel, son dessin du départ, un chemin en clés d'images
/// et une marque au milieu : jalons aux images 0, 12 et 23. Rend le banc et le dessin du départ.
fn banc_feuille() -> (Banc, CelId) {
    let mut b = feuille::banc();
    b.app.traj.cles_propose = Cles::Images;
    let source = feuille::dessiner_le_depart(&mut b);
    feuille::tracer_le_chemin(&mut b);
    feuille::marquer_le_milieu(&mut b);
    b.chauffer(2);
    assert_eq!(jalons(&b, 0), [0, 12, 23]);
    assert!(!b.app.status.contains("posé à la main"), "un chemin sur ses seuls dessins n'a rien à dire : {}", b.app.status);
    (b, source)
}

/// Une clé posée à la main à l'image `image`, **et un trait de pinceau dessus** (un vrai trait de souris sur la feuille, à la
/// hauteur `y`) : rend son dessin.
fn cle_dessinee_sur_la_feuille(b: &mut Banc, image: usize, y: f32) -> CelId {
    let cle = nouvelle_cle(b, 0, image);
    let traits = feuille::traits(b, cle);
    b.app.set_tool(Tool::Brush);
    feuille::glisser(b, &[(300.0, y), (320.0, y + 10.0), (340.0, y)]);
    b.app.set_tool(Tool::Trajectoire);
    assert_eq!(feuille::traits(b, cle), traits + 1, "le trait de pinceau est sur la clé posée à la main ({})", b.app.status);
    cle
}

/// Glisse la marque du chemin de la feuille qui est à l'abscisse `de`, jusqu'à l'abscisse `a` (le chemin est droit, à la
/// hauteur `y`).
fn glisser_la_marque(b: &mut Banc, de: f32, a: f32, y: f32) {
    feuille::glisser(b, &[(de, y), ((de + a) / 2.0, y + 1.0), (a, y)]);
    b.chauffer(2);
    assert_eq!(etape(b), "Glisser une marque", "{}", b.app.status);
}

/// Recale sur la timeline la clé de dessin d'un jalon, de l'image `de` à l'image `a` (le montage des clés, clé seule).
fn recaler(b: &mut Banc, li: usize, de: usize, a: usize) {
    let (plan, calque) = (b.app.doc.panels[0].id, b.app.layers()[li].id);
    b.app.key_sel = vec![KeyRef { panel: plan, layer: calque, start: de }];
    assert!(b.app.begin_keys_drag(de), "{}", b.app.status);
    b.app.drag_keys_mode(a, crate::handles::KeyMove::Single);
    b.app.end_keys_drag();
    b.chauffer(2);
}

#[test]
fn on_the_sheet_a_drawing_key_placed_by_hand_between_two_marks_stays_at_its_image_whatever_is_done_to_the_path() {
    let (mut b, source) = banc_feuille();
    // Une clé de dessin posée à la main à l'image 11 (entre le départ et la marque), et un trait de pinceau dessus.
    let x = cle_dessinee_sur_la_feuille(&mut b, 10, 60.0);
    assert_eq!(expositions(&b, 0, x), [(10, 12)], "elle tient jusqu'à la marque");
    let tel_quel = |b: &Banc, quand: &str| {
        toujours_a_son_image(b, 0, x, 10, quand);
        assert_eq!(feuille::traits(b, x), 2, "{quand} : le même dessin, avec ce qu'on y a dessiné");
    };
    assert!(!b.app.status.contains("posé à la main"), "poser une clé et dessiner : aucun calcul, rien à dire");

    // La marque glisse le long du chemin (de 50 à 70 %) : la clé n'est pas remplacée, et la barre d'état le dit, une fois.
    glisser_la_marque(&mut b, 200.0, 260.0, 150.0);
    tel_quel(&b, "une marque glissée");
    let d = des_jalons(&b, 0);
    assert_eq!((expositions(&b, 0, source), expositions(&b, 0, x), expositions(&b, 0, d[1]), expositions(&b, 0, d[2])), (vec![(0, 10)], vec![(10, 12)], vec![(12, 23)], vec![(23, 24)]), "le dessin du départ est exposé jusqu'à elle");
    assert!(b.app.status.ends_with(&gardes_dit(1, "image 11")), "{}", b.app.status);
    b.app.status.clear();
    glisser_la_marque(&mut b, 260.0, 230.0, 150.0);
    tel_quel(&b, "la marque glissée de nouveau");
    assert!(!b.app.status.contains("posé à la main"), "les mêmes clés ne se redisent pas : {}", b.app.status);

    // Une marque posée après elle (à 80 % du chemin), puis une avant (à 20 %).
    feuille::glisser(&mut b, &[(290.0, 135.0), (290.0, 165.0)]);
    assert_eq!((etape(&b), jalons(&b, 0)), ("Marque de trajectoire".to_string(), vec![0, 8, 15, 23]));
    tel_quel(&b, "une marque posée après elle");
    assert_eq!(expositions(&b, 0, x), [(10, 15)], "elle tient jusqu'au jalon suivant");
    feuille::glisser(&mut b, &[(110.0, 135.0), (110.0, 165.0)]);
    assert_eq!(jalons(&b, 0), [0, 6, 12, 17, 23], "{}", b.app.status);
    tel_quel(&b, "une marque posée avant elle");
    assert_eq!(expositions(&b, 0, x), [(10, 12)], "une marque écourte son exposition, jamais sa clé");
    // Cette marque arrachée (tirée loin du chemin) : sa copie s'en va avec elle, la clé posée à la main reste.
    let copie = des_jalons(&b, 0)[1];
    feuille::glisser(&mut b, &[(110.0, 150.0), (110.0, 105.0), (110.0, 60.0)]);
    assert_eq!((etape(&b), jalons(&b, 0)), ("Retirer une marque".to_string(), vec![0, 8, 15, 23]), "{}", b.app.status);
    tel_quel(&b, "une marque retirée");
    assert!(expositions(&b, 0, copie).is_empty(), "le dessin de la marque retirée, lui, n'est pas pris pour un dessin posé à la main");
    assert_eq!(expositions(&b, 0, x), [(10, 15)]);

    // Le jalon suivant recalé sur la timeline (sa clé de dessin, de l'image 16 à l'image 14), puis « Temps égaux ».
    recaler(&mut b, 0, 15, 13);
    assert_eq!(jalons(&b, 0), [0, 8, 13, 23], "{}", b.app.status);
    tel_quel(&b, "un jalon recalé sur la timeline");
    assert_eq!(expositions(&b, 0, x), [(10, 13)]);
    regler(&mut b, "Temps égaux entre les marques", |t| t.temps_egaux());
    assert_eq!(jalons(&b, 0), [0, 8, 15, 23]);
    tel_quel(&b, "« Temps égaux »");
    assert_eq!(expositions(&b, 0, x), [(10, 15)]);

    // Un second trait long, plus bas : le chemin est retracé, les dessins des jalons le suivent, pas le dessin posé à la main.
    feuille::glisser(&mut b, &[(50.0, 200.0), (110.0, 200.0), (170.0, 200.0), (230.0, 200.0), (290.0, 200.0), (350.0, 200.0)]);
    assert_eq!(etape(&b), "Retracer la trajectoire", "{}", b.app.status);
    tel_quel(&b, "le chemin retracé");
    assert_eq!(expositions(&b, 0, x), [(10, 15)]);

    // Une annulation, puis rétablir : la clé est là dans les deux états.
    let retrace = (exposes(&b, 0), b.app.layers()[0].trajectoire.clone());
    b.app.undo();
    tel_quel(&b, "annuler");
    b.app.redo();
    b.chauffer(2);
    assert_eq!((exposes(&b, 0), b.app.layers()[0].trajectoire.clone()), retrace, "rétablir rend tout");

    // Clés d'interpolation, puis de nouveau clés d'images : les dessins des jalons s'en vont et reviennent, elle reste.
    regler(&mut b, "Sorte des clés de la trajectoire", |t| t.cles = Cles::Interpolation);
    assert!(des_jalons(&b, 0).is_empty());
    tel_quel(&b, "en clés d'interpolation");
    regler(&mut b, "Sorte des clés de la trajectoire", |t| t.cles = Cles::Images);
    assert_eq!((des_jalons(&b, 0).len(), jalons(&b, 0)), (4, vec![0, 8, 15, 23]), "{}", b.app.status);
    tel_quel(&b, "de nouveau en clés d'images");
    assert_eq!(expositions(&b, 0, x), [(10, 15)]);
    assert!(!des_jalons(&b, 0).contains(&x), "ce n'est toujours pas un dessin de jalon");

    // Enregistré, relu : tout revient tel quel (rien de plus n'est écrit pour cela), et un calcul sur le projet relu la laisse.
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire-main-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    let manifest = dir.join("M.shot");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let texte = std::fs::read_to_string(&manifest).expect("le manifeste");
    assert!(!texte.contains("\"retires\""), "rien de neuf dans le fichier : la note des marques retirées n'y est jamais écrite");
    let relu = project::load(&manifest).expect("relu").doc;
    assert_eq!(relu.panels[0].layers[0].trajectoire, b.app.layers()[0].trajectoire);
    assert_eq!(relu.panels[0].layers[0].frames, b.app.layers()[0].frames);
    std::fs::remove_dir_all(&dir).ok();
    let attendu = exposes(&b, 0);
    let mut r = banc_taille(relu, false, 1280, 720);
    (r.app.cur_panel, r.app.cur_layer, r.app.cur_frame) = (0, 0, 0);
    r.app.trajectoire_recalculer_dans(0, 0);
    r.chauffer(3);
    assert_eq!(exposes(&r, 0), attendu, "relu, recalculé : la même timeline");
    assert!(r.app.status.ends_with(&gardes_dit(1, "image 11")), "{}", r.app.status);

    // On supprime soi-même sa clé (la commande de la timeline) : la clé d'avant reprend ses images, tout de suite, et le
    // calcul suivant fait comme si elle n'avait jamais été là.
    let d = des_jalons(&b, 0);
    supprimer_la_cle(&mut b, 0, 10, false);
    assert!(expositions(&b, 0, x).is_empty());
    assert_eq!(expositions(&b, 0, d[1]), [(8, 15)], "le dessin du jalon d'avant tient de nouveau jusqu'au suivant : {}", b.app.status);
    b.app.status.clear();
    b.app.trajectoire_recalculer_dans(0, 0);
    b.chauffer(2);
    assert_eq!((des_jalons(&b, 0), expositions(&b, 0, d[1])), (d.clone(), vec![(8, 15)]));
    assert!(!b.app.status.contains("posé à la main"), "{}", b.app.status);
    // Une annulation rend la clé ; supprimée en laissant ses images vides, c'est le calcul suivant qui les comble.
    b.app.undo();
    tel_quel(&b, "la suppression annulée");
    supprimer_la_cle(&mut b, 0, 10, true);
    assert_eq!((10..15).map(|f| b.app.layers()[0].cel_at(f)).collect::<Vec<_>>(), vec![None; 5], "{}", b.app.status);
    b.app.trajectoire_recalculer_dans(0, 0);
    assert_eq!(expositions(&b, 0, d[1]), [(8, 15)], "le dessin du jalon comble le vide");
}

/// Un jalon qui tomberait exactement sur l'image d'une clé posée à la main est posé sur l'image libre d'à côté, celle d'après
/// de préférence ; la barre d'état le dit ; aucun dessin n'est perdu.
#[test]
fn a_milestone_that_would_land_on_the_image_of_a_hand_placed_key_is_put_on_the_next_free_image_and_the_status_says_so() {
    let (mut b, source) = banc_feuille();
    let x = nouvelle_cle(&mut b, 0, 8);
    assert_eq!(expositions(&b, 0, x), [(8, 12)]);
    let tombait = "Trajectoire : une marque tombait sur un dessin posé à la main, à l'image 9 : elle est posée à l'image 10.";
    // Une seconde marque (à 80 %) : avec deux marques en temps égaux, la première tomberait à l'image 9, sur la clé.
    feuille::glisser(&mut b, &[(290.0, 135.0), (290.0, 165.0)]);
    b.chauffer(2);
    assert_eq!(etape(&b), "Marque de trajectoire", "{}", b.app.status);
    assert_eq!(jalons(&b, 0), [0, 9, 15, 23], "la marque est à l'image d'après : {}", b.app.status);
    toujours_a_son_image(&b, 0, x, 8, "une marque posée");
    let d = des_jalons(&b, 0);
    assert_eq!((expositions(&b, 0, source), expositions(&b, 0, x), expositions(&b, 0, d[1]), expositions(&b, 0, d[2])), (vec![(0, 8)], vec![(8, 9)], vec![(9, 15)], vec![(15, 23)]), "aucun dessin n'est perdu");
    assert!(b.app.status.starts_with("Marque posée"), "{}", b.app.status);
    assert!(b.app.status.contains(tombait), "{}", b.app.status);
    assert!(b.app.status.ends_with(&gardes_dit(1, "image 9")), "{}", b.app.status);
    // « Temps égaux » la ramènerait sur la clé : elle est posée à côté de nouveau, et c'est dit de nouveau.
    b.app.status.clear();
    regler(&mut b, "Temps égaux entre les marques", |t| t.temps_egaux());
    assert_eq!(jalons(&b, 0), [0, 9, 15, 23]);
    toujours_a_son_image(&b, 0, x, 8, "« Temps égaux »");
    assert_eq!(b.app.status, tombait);
    // « Finir à la tête de lecture », la tête de lecture sur la clé : l'arrivée est posée à l'image d'après.
    b.app.cur_frame = 8;
    b.app.status.clear();
    b.app.trajectoire_fin_ici();
    b.chauffer(2);
    toujours_a_son_image(&b, 0, x, 8, "« Finir à la tête de lecture »");
    assert_eq!(jalons(&b, 0).last(), Some(&9), "l'arrivée est à l'image d'après : {:?} ({})", jalons(&b, 0), b.app.status);
    assert!(b.app.status.contains("Trajectoire : l'arrivée tombait sur un dessin posé à la main, à l'image 9 : elle est posée à l'image 10."), "{}", b.app.status);
    // Trois annulations : la marque, ses temps et la fin reviennent, la clé n'a jamais bougé.
    b.app.undo();
    b.app.undo();
    b.app.undo();
    assert_eq!(jalons(&b, 0), [0, 12, 23]);
    assert_eq!(expositions(&b, 0, x), [(8, 12)]);
}

/// La règle est **dans la pose des dessins**, pas dans les gestes : un calcul demandé directement
/// (`trajectoire_recalculer_dans`, sans aucun geste) laisse la clé posée à la main ; refait autant de fois qu'on veut, il ne
/// change rien et ne redit rien ; une clé de plus se dit, une de moins non.
#[test]
fn a_recompute_called_directly_keeps_a_hand_placed_key_and_many_recomputes_say_it_once() {
    let (mut b, _) = banc_feuille();
    let x = nouvelle_cle(&mut b, 0, 10);
    let avant = (exposes(&b, 0), b.app.layers()[0].trajectoire.clone(), b.app.history.undo_len());
    b.app.status.clear();
    b.app.trajectoire_recalculer_dans(0, 0);
    assert_eq!(exposes(&b, 0), avant.0, "la clé posée à la main n'est pas remplacée");
    b.chauffer(2);
    assert_eq!(b.app.status, gardes_dit(1, "image 11"));
    // Beaucoup de calculs à la suite, une image d'écran entre chacun : rien ne change, rien n'est redit.
    b.app.status.clear();
    for image in 0..12 {
        b.app.trajectoire_recalculer_dans(0, 0);
        b.app.trajectoire_recalculer(0);
        b.chauffer(1);
        assert_eq!((exposes(&b, 0), b.app.layers()[0].trajectoire.clone()), (avant.0.clone(), avant.1.clone()), "calcul {image}");
        assert_eq!(b.app.status, "", "calcul {image} : dit une fois, pas à chaque calcul");
        assert!(!b.app.modal_open());
    }
    assert_eq!(b.app.history.undo_len(), avant.2);
    // Une seconde clé posée à la main (image 17) : le calcul suivant les dit toutes les deux.
    let y = nouvelle_cle(&mut b, 0, 16);
    b.app.status.clear();
    b.app.trajectoire_recalculer_dans(0, 0);
    b.chauffer(2);
    assert_eq!(b.app.status, gardes_dit(2, "images 11 et 17"));
    assert_eq!((expositions(&b, 0, x), expositions(&b, 0, y)), (vec![(10, 12)], vec![(16, 23)]));
    // Celle-ci supprimée : il en reste une, déjà dite.
    supprimer_la_cle(&mut b, 0, 16, false);
    b.app.status.clear();
    b.app.trajectoire_recalculer_dans(0, 0);
    b.chauffer(2);
    assert_eq!(b.app.status, "");
    assert_eq!(expositions(&b, 0, x), [(10, 12)]);
}

/// Un calque dupliqué : ses dessins sont des copies, et sa trajectoire notait encore ceux de l'original. Au premier calcul,
/// chaque jalon prend pour sien le dessin qui est à son image : rien ne bouge, et les deux calques n'ont aucun dessin en commun.
#[test]
fn a_duplicated_layer_keeps_its_own_drawings_at_its_milestones_at_its_first_recompute() {
    let (mut b, _) = banc_feuille();
    let original = (exposes(&b, 0), des_jalons(&b, 0));
    b.app.duplicate_layer();
    let li = b.app.cur_layer;
    assert_eq!(li, 1, "{}", b.app.status);
    let copie = exposes(&b, li);
    assert!(copie.iter().flatten().all(|c| !original.1.contains(c)), "les dessins de la copie sont des copies");
    b.app.status.clear();
    b.app.trajectoire_recalculer(li);
    b.chauffer(2);
    assert_eq!((exposes(&b, li), jalons(&b, li)), (copie.clone(), vec![0, 12, 23]), "rien n'a bougé dans la copie : {}", b.app.status);
    assert_eq!(des_jalons(&b, li), vec![copie[0].unwrap(), copie[12].unwrap(), copie[23].unwrap()], "ses jalons ont ses propres dessins");
    assert_eq!(b.app.layers()[li].trajectoire.as_ref().and_then(|t| t.source), copie[0]);
    assert_eq!((exposes(&b, 0), des_jalons(&b, 0)), original, "l'original n'a pas changé");
    assert_eq!(b.app.status, "", "ce ne sont pas des dessins posés à la main");
}

// ------------------------------------------------------------------------------------------------ dans l'espace

/// Le nombre de pixels encrés du dessin `cel` (sur la feuille).
fn encre(b: &Banc, cel: CelId) -> usize {
    let raster = &b.app.doc.cels[&cel].raster;
    let (w, h) = (b.app.doc.width as i64, b.app.doc.height as i64);
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| raster.get_i(x, y).a() > 0).count()
}

/// Un vrai trait de pinceau dans la vue Espace 3D, sur la carte du calque courant, à l'image courante ; puis l'outil
/// Trajectoire et la vue d'avant reviennent.
fn dessiner_sur_la_carte(b: &mut Banc) {
    let vue_d_avant = b.app.espace3d.cam;
    b.app.run_action("espace3d.cadrer");
    b.app.run_action("outil.pinceau");
    b.chauffer(3);
    assert!(b.app.e3_dessine(), "le pinceau a le pointeur dans la vue");
    let vue = b.app.e3_carte_vue().expect("on dessine sur une carte");
    let (a, c) = (vue.vers_ecran(pos2(300.0, 250.0)).expect("le départ du trait se voit"), vue.vers_ecran(pos2(380.0, 270.0)).expect("son arrivée aussi"));
    let zone = b.app.canvas_rect.shrink(4.0);
    assert!(zone.contains(a) && zone.contains(c), "le trait tient dans la vue : {a:?} → {c:?} dans {zone:?}");
    geste(b, &[a, c], Modifiers::NONE);
    b.app.espace3d.cam = vue_d_avant;
    b.app.run_action("outil.trajectoire");
    b.chauffer(3);
    assert!(b.app.e3_trace());
}

/// Glisse la marque du chemin d'essai dans la vue, jusqu'à la fraction `vers` du chemin : un vrai geste de souris.
fn glisser_la_marque_dans_la_vue(b: &mut Banc, vers: f32) {
    let m = trajectoire(b, 0).marques[0];
    let (prise, la) = (a_l_ecran(b, espace::sur_le_chemin(b, m)), a_l_ecran(b, espace::sur_le_chemin(b, vers)));
    appuyer(b, &[prise, la], Modifiers::NONE);
    lacher(b, la, Modifiers::NONE);
    assert_eq!(etape(b), "Glisser une marque", "{}", b.app.status);
}

/// Les images des clés de position du calque `li` (toutes tenues, en clés d'images).
fn cles_tenues(b: &Banc, li: usize) -> Vec<usize> {
    assert!(b.app.layers()[li].pos_keys.iter().all(|k| k.tenue));
    b.app.layers()[li].pos_keys.iter().map(|k| k.frame).collect()
}

#[test]
fn in_space_hand_placed_keys_of_two_chosen_cards_stay_at_their_images_and_a_retimed_milestone_goes_beside_with_its_position_key() {
    let (mut b, source0) = espace::banc_images(2);
    let ids: Vec<u64> = b.app.layers().iter().map(|l| l.id).collect();
    let les_deux = vec![espace3d::card_selection(ids[0]), espace3d::card_selection(ids[1])];
    let cartes = [Sujet::Carte(ids[0]), Sujet::Carte(ids[1])];
    let source1 = b.app.layers()[1].cel_at(0).expect("un dessin au départ de la seconde carte");
    // Les deux cartes suivent le même chemin, en clés d'images : jalons aux images 0, 12 et 23.
    b.app.espace3d.sel = les_deux.clone();
    tracer_au_sol(&mut b);
    espace::marquer(&mut b, 0.15);
    for li in 0..2 {
        assert_eq!((jalons(&b, li), des_jalons(&b, li).len(), cles_tenues(&b, li)), (vec![0, 12, 23], 3, vec![0, 12, 23]), "{}", b.app.status);
    }
    // Des clés posées à la main : deux sur la carte 1 (images 6 et 17), une sur la carte 0 (image 11), sur laquelle on dessine
    // dans la vue, au pinceau.
    let y1 = nouvelle_cle(&mut b, 1, 5);
    let y2 = nouvelle_cle(&mut b, 1, 16);
    let x = nouvelle_cle(&mut b, 0, 10);
    let avant = encre(&b, x);
    dessiner_sur_la_carte(&mut b);
    let encre_de_x = encre(&b, x);
    assert!(encre_de_x > avant, "le trait de pinceau est sur la clé posée à la main ({})", b.app.status);
    assert_eq!(encre(&b, source0), avant, "et pas sur le dessin du départ");
    let tels_quels = |b: &Banc, quand: &str| {
        toujours_a_son_image(b, 0, x, 10, quand);
        toujours_a_son_image(b, 1, y1, 5, quand);
        toujours_a_son_image(b, 1, y2, 16, quand);
        assert_eq!(encre(b, x), encre_de_x, "{quand} : le même dessin, avec ce qu'on y a dessiné");
    };

    // Un calcul pour les deux cartes : la marque glisse dans la vue. Les trois clés restent, la barre d'état le dit une fois.
    b.app.espace3d.sel = les_deux.clone();
    b.chauffer(2);
    glisser_la_marque_dans_la_vue(&mut b, 0.6);
    tels_quels(&b, "une marque glissée dans la vue");
    let (d0, d1) = (des_jalons(&b, 0), des_jalons(&b, 1));
    assert_eq!((d0[0], d1[0]), (source0, source1));
    assert_eq!((expositions(&b, 0, source0), expositions(&b, 0, x), expositions(&b, 0, d0[1]), expositions(&b, 0, d0[2])), (vec![(0, 10)], vec![(10, 12)], vec![(12, 23)], vec![(23, 24)]));
    assert_eq!((expositions(&b, 1, source1), expositions(&b, 1, y1), expositions(&b, 1, d1[1]), expositions(&b, 1, y2)), (vec![(0, 5)], vec![(5, 12)], vec![(12, 16)], vec![(16, 23)]));
    assert!(b.app.status.ends_with("Trajectoire : 3 dessins posés à la main gardés entre les marques (« Carte 0 » : image 11 ; « Carte 1 » : images 6 et 17)."), "{}", b.app.status);
    // Les clés de position tenues ne changent pas : une par jalon ; le dessin posé à la main se voit là où la carte est tenue.
    assert_eq!((cles_tenues(&b, 0), cles_tenues(&b, 1)), (vec![0, 12, 23], vec![0, 12, 23]));
    let poses = (exposes(&b, 0), exposes(&b, 1));
    b.app.status.clear();
    glisser_la_marque_dans_la_vue(&mut b, 0.4);
    tels_quels(&b, "la marque glissée de nouveau");
    assert_eq!((exposes(&b, 0), exposes(&b, 1)), poses);
    assert!(!b.app.status.contains("posé à la main"), "les mêmes clés ne se redisent pas : {}", b.app.status);

    // Le jalon de la marque recalé sur la timeline par sa clé de position, un calcul à chaque image du geste. Sur l'image de la
    // clé posée à la main de la carte 0 (l'image 11) : il est posé à l'image d'après, sa clé de position avec lui, et c'est dit.
    // (Le geste est fait d'une traite, sans image d'écran entre ses pas : sans bouton tenu, la timeline le clorait.)
    let etapes = b.app.history.undo_len();
    assert!(b.app.begin_pos_drag(0, 12, 12), "{}", b.app.status);
    b.app.drag_pos_keys(10);
    toujours_a_son_image(&b, 0, x, 10, "le jalon tiré sur son image");
    assert_eq!((jalons(&b, 0), cles_tenues(&b, 0)), (vec![0, 11, 23], vec![0, 11, 23]), "{}", b.app.status);
    assert_eq!((expositions(&b, 0, x), expositions(&b, 0, d0[1])), (vec![(10, 11)], vec![(11, 23)]));
    b.app.end_pos_drag();
    b.chauffer(2);
    assert_eq!((etape(&b), b.app.history.undo_len()), ("Recaler la trajectoire".to_string(), etapes + 1));
    assert_eq!((jalons(&b, 0), cles_tenues(&b, 0), expositions(&b, 0, x)), (vec![0, 11, 23], vec![0, 11, 23], vec![(10, 11)]));
    assert!(b.app.status.ends_with("Trajectoire : une marque tombait sur un dessin posé à la main, à l'image 11 : elle est posée à l'image 12."), "{}", b.app.status);
    b.app.undo();
    b.chauffer(2);
    assert_eq!((exposes(&b, 0), exposes(&b, 1), jalons(&b, 0), cles_tenues(&b, 0)), (poses.0.clone(), poses.1.clone(), vec![0, 12, 23], vec![0, 12, 23]), "une annulation rend tout");
    // Plus loin, plus près, avant elle, puis de retour : son exposition suit le jalon suivant, sa clé ne bouge jamais.
    assert!(b.app.begin_pos_drag(0, 12, 12), "{}", b.app.status);
    for (vers, attendu) in [(14, (10, 14)), (17, (10, 17)), (7, (10, 23)), (12, (10, 12))] {
        b.app.drag_pos_keys(vers);
        assert_eq!((jalons(&b, 0)[1], cles_tenues(&b, 0)[1]), (vers, vers));
        assert_eq!(expositions(&b, 0, x), [attendu], "le jalon à l'image {}", vers + 1);
    }
    b.app.cancel_pos_drag();
    b.chauffer(2);
    assert_eq!((exposes(&b, 0), exposes(&b, 1)), poses, "le geste abandonné : tout est comme avant");

    // La marque arrachée dans la vue (une étape pour les deux cartes), annulée, rétablie.
    b.app.espace3d.sel = les_deux.clone();
    b.chauffer(2);
    let etapes = b.app.history.undo_len();
    let prise = a_l_ecran(&b, espace::sur_le_chemin(&b, trajectoire(&b, 0).marques[0]));
    geste(&mut b, &[prise, prise + eframe::egui::vec2(0.0, -90.0)], Modifiers::NONE);
    assert_eq!((etape(&b), b.app.history.undo_len(), jalons(&b, 0), jalons(&b, 1)), ("Retirer une marque".to_string(), etapes + 1, vec![0, 23], vec![0, 23]), "{}", b.app.status);
    tels_quels(&b, "la marque retirée");
    assert_eq!((expositions(&b, 0, x), expositions(&b, 1, y1), expositions(&b, 1, y2)), (vec![(10, 23)], vec![(5, 16)], vec![(16, 23)]));
    assert!(expositions(&b, 0, d0[1]).is_empty() && expositions(&b, 1, d1[1]).is_empty(), "les copies de la marque retirée s'en vont avec elle");
    let sans = (exposes(&b, 0), exposes(&b, 1));
    b.app.undo();
    b.chauffer(2);
    assert_eq!((exposes(&b, 0), exposes(&b, 1)), poses, "une annulation rend la marque et ses copies");
    tels_quels(&b, "annuler");
    b.app.redo();
    b.chauffer(2);
    assert_eq!((exposes(&b, 0), exposes(&b, 1)), sans);
    b.app.undo();

    // Clés d'interpolation, puis de nouveau clés d'images, pour les deux cartes : elles restent.
    b.app.e3_traj_sorte(&cartes, Cles::Interpolation);
    assert!(des_jalons(&b, 0).is_empty() && des_jalons(&b, 1).is_empty());
    tels_quels(&b, "en clés d'interpolation");
    b.app.e3_traj_sorte(&cartes, Cles::Images);
    b.chauffer(2);
    assert_eq!((des_jalons(&b, 0).len(), des_jalons(&b, 1).len(), cles_tenues(&b, 0)), (3, 3, vec![0, 12, 23]), "{}", b.app.status);
    tels_quels(&b, "de nouveau en clés d'images");
    assert_eq!((expositions(&b, 0, x), expositions(&b, 1, y1), expositions(&b, 1, y2)), (vec![(10, 12)], vec![(5, 12)], vec![(16, 23)]));

    // Un calcul demandé directement, sans geste : de même.
    let poses = (exposes(&b, 0), exposes(&b, 1));
    b.app.trajectoire_recalculer_dans(0, 0);
    b.app.trajectoire_recalculer_dans(0, 1);
    assert_eq!((exposes(&b, 0), exposes(&b, 1)), poses);

    // Enregistré, relu : les deux cartes reviennent telles quelles.
    let dir = std::env::temp_dir().join(format!("feanor-trajectoire3d-main-{}-{:?}", std::process::id(), std::thread::current().id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("dossier d'essai");
    let manifest = dir.join("M.shot");
    project::save(&b.app.doc, &manifest, &mut Default::default()).expect("enregistré");
    let relu = project::load(&manifest).expect("relu").doc;
    for li in 0..2 {
        assert_eq!(relu.panels[0].layers[li].trajectoire, b.app.layers()[li].trajectoire, "la carte {li}");
        assert_eq!(relu.panels[0].layers[li].frames, b.app.layers()[li].frames);
    }
    std::fs::remove_dir_all(&dir).ok();

    // On supprime soi-même la clé de la carte 0 : le dessin du départ tient de nouveau jusqu'à la marque.
    supprimer_la_cle(&mut b, 0, 10, false);
    b.app.trajectoire_recalculer_dans(0, 0);
    assert_eq!(expositions(&b, 0, source0), [(0, 12)]);
}
