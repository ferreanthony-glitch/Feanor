//! Glisser des calques dans un dossier (Anthony, 2026-11-11 : « glisser des calques dans un dossier ne fonctionne pas », partie 139).
//!
//! Avant : un glissé ne déplaçait que le calque pris, même parmi plusieurs calques choisis ; le lâcher sur la ligne d'un dossier ne
//! le rangeait pas dedans (le seul chemin était le trait juste sous le dossier) ; le calque posé juste sous un dossier vide ne
//! pouvait pas y entrer du tout (« lâché juste au-dessus de lui-même : il ne bouge pas ») ; rien n'entrait dans un dossier fermé.
//!
//! Maintenant : lâchés sur la ligne d'un dossier, les calques y entrent, ouvert ou fermé, vide ou non ; les calques choisis partent
//! ensemble, dans leur ordre ; entre deux lignes, rien ne change. Les premiers essais sont purs (`folders::poser`,
//! `layerheads::cible_du_glisse`) ; les derniers font le geste dans le panneau Calques et dans la colonne de la bande du montage.

use eframe::egui::{pos2, Event};

use crate::calques_timeline_tests::{banc_de_retakes, colonne, nom_au_panneau, rang, trouver};
use crate::document::{Layer, LayerKind};
use crate::folders::{poser, Pose};
use crate::layerheads::{cible_du_glisse, Cible, Row};
use crate::pistes::Pile;
use crate::pistes_tests::{etapes, glisser};
use crate::workspace::Espace;

// ------------------------------------------------------------------------------------ l'outillage

/// La pile de sa capture, du bas vers le haut. Du haut vers le bas : « Plein » (dossier fermé) [A, B], « Vide » (dossier ouvert,
/// sans calque), V1, V2, V3, Réf.
fn pile() -> Vec<Layer> {
    let mut plein = Layer::new_folder("Plein");
    plein.collapsed = true;
    let vide = Layer::new_folder("Vide");
    let dans = |nom: &str, dossier: &Layer| {
        let mut l = Layer::new(nom, LayerKind::Vector);
        l.parent = Some(dossier.id);
        l
    };
    let du_haut = vec![
        plein.clone(),
        dans("A", &plein),
        dans("B", &plein),
        vide,
        Layer::new("V1", LayerKind::Vector),
        Layer::new("V2", LayerKind::Vector),
        Layer::new("V3", LayerKind::Vector),
        Layer::new("Réf", LayerKind::Bitmap),
    ];
    du_haut.into_iter().rev().collect()
}

fn id(l: &[Layer], nom: &str) -> u64 {
    l.iter().find(|x| x.name == nom).unwrap_or_else(|| panic!("pas de calque « {nom} »")).id
}

/// La pile du haut vers le bas : le nom de chaque calque, précédé de celui de son dossier (« Vide/V1 »).
fn du_haut(l: &[Layer]) -> Vec<String> {
    l.iter()
        .rev()
        .map(|x| match x.parent.and_then(|p| l.iter().find(|f| f.id == p)) {
            Some(f) => format!("{}/{}", f.name, x.name),
            None => x.name.clone(),
        })
        .collect()
}

/// Les lignes d'une liste de calques de quarante points, du haut vers le bas, les calques d'un dossier fermé n'en ayant pas.
fn lignes(l: &[Layer], h: f32) -> Vec<Row> {
    let fermes: Vec<u64> = l.iter().filter(|x| x.folder && x.collapsed).map(|x| x.id).collect();
    let mut top = 0.0;
    (0..l.len())
        .rev()
        .filter(|&li| l[li].parent.is_none_or(|p| !fermes.contains(&p)))
        .map(|li| {
            let row = Row { li, top, h };
            top += h;
            row
        })
        .collect()
}

// ------------------------------------------------------------------------------------ la pile

#[test]
fn layers_dropped_on_a_folder_go_in_together_in_their_order() {
    let l = pile();
    let (plein, vide) = (id(&l, "Plein"), id(&l, "Vide"));
    // Les trois calques choisis lâchés sur le dossier vide : dedans, dans leur ordre.
    let trois = [id(&l, "V1"), id(&l, "V2"), id(&l, "V3")];
    let pose = poser(&l, &trois, Pose::Dans(vide)).expect("les trois calques entrent");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/A", "Plein/B", "Vide", "Vide/V1", "Vide/V2", "Vide/V3", "Réf"]);
    // Le calque posé juste sous le dossier vide y entre aussi (avant : « lâché juste au-dessus de lui-même, il ne bouge pas »).
    let pose = poser(&l, &[id(&l, "V1")], Pose::Dans(vide)).expect("le calque du dessous entre");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/A", "Plein/B", "Vide", "Vide/V1", "V2", "V3", "Réf"]);
    // Dans un dossier fermé : en haut de ses calques ; il reste fermé.
    let pose = poser(&l, &[id(&l, "V2")], Pose::Dans(plein)).expect("on entre dans un dossier fermé");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/V2", "Plein/A", "Plein/B", "Vide", "V1", "V3", "Réf"]);
    assert!(pose.iter().find(|x| x.id == plein).is_some_and(|f| f.collapsed), "le dossier reste fermé");
    // Déjà en haut de ce dossier : rien ne change, pas d'étape pour rien.
    assert_eq!(poser(&pose, &[id(&l, "V2")], Pose::Dans(plein)).map(|p| du_haut(&p)), None);
    // Des calques venus de deux endroits (un du dossier fermé, un hors dossier) entrent ensemble, le plus haut reste le plus haut.
    let pose = poser(&l, &[id(&l, "V3"), id(&l, "A")], Pose::Dans(vide)).expect("deux calques de deux endroits");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/B", "Vide", "Vide/A", "Vide/V3", "V1", "V2", "Réf"]);
    // Un dossier n'entre jamais dans un dossier (un seul niveau).
    assert_eq!(poser(&l, &[vide], Pose::Dans(plein)).map(|p| du_haut(&p)), None);
    assert_eq!(poser(&l, &[id(&l, "V1"), vide], Pose::Dans(plein)).map(|p| du_haut(&p)), None);
}

#[test]
fn between_two_lines_the_old_rules_hold_for_one_layer_or_several() {
    let l = pile();
    // Entre deux calques d'un dossier : dedans, à cette place.
    let pose = poser(&l, &[id(&l, "V3")], Pose::AuDessusDe(id(&l, "B"))).expect("entre A et B");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/A", "Plein/V3", "Plein/B", "Vide", "V1", "V2", "Réf"]);
    // Au-dessus d'un dossier : dehors, tout en haut ; et un calque rangé en sort.
    let pose = poser(&l, &[id(&l, "A")], Pose::AuDessusDe(id(&l, "Plein"))).expect("A sort de son dossier");
    assert_eq!(du_haut(&pose), ["A", "Plein", "Plein/B", "Vide", "V1", "V2", "V3", "Réf"]);
    // Sous la dernière ligne : tout en bas. Deux calques choisis partent ensemble.
    let pose = poser(&l, &[id(&l, "V1"), id(&l, "V2")], Pose::Sous(id(&l, "Réf"))).expect("tout en bas");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/A", "Plein/B", "Vide", "V3", "Réf", "V1", "V2"]);
    // Le trait juste au-dessus d'un des calques glissés, quand ils sont déjà d'un seul tenant : rien ne bouge.
    assert_eq!(poser(&l, &[id(&l, "V2"), id(&l, "V3")], Pose::AuDessusDe(id(&l, "V3"))).map(|p| du_haut(&p)), None);
    assert_eq!(poser(&l, &[id(&l, "V2"), id(&l, "V3")], Pose::AuDessusDe(id(&l, "Réf"))).map(|p| du_haut(&p)), None);
    // Un dossier glissé emmène ses calques ; posé parmi les siens, il ne bouge pas.
    let pose = poser(&l, &[id(&l, "Plein")], Pose::Sous(id(&l, "Réf"))).expect("le dossier descend avec ses calques");
    assert_eq!(du_haut(&pose), ["Vide", "V1", "V2", "V3", "Réf", "Plein", "Plein/A", "Plein/B"]);
    assert_eq!(poser(&l, &[id(&l, "Plein")], Pose::AuDessusDe(id(&l, "B"))).map(|p| du_haut(&p)), None);
    // Le trait juste sous un dossier ouvert : dedans (l'ancien chemin, qui reste).
    let pose = poser(&l, &[id(&l, "V2")], Pose::AuDessusDe(id(&l, "V1"))).expect("juste sous le dossier vide");
    assert_eq!(du_haut(&pose), ["Plein", "Plein/A", "Plein/B", "Vide", "Vide/V2", "V1", "V3", "Réf"]);
}

// ------------------------------------------------------------------------------------ le pointeur

#[test]
fn the_pointer_reads_into_the_folder_on_its_line_and_between_lines_elsewhere() {
    let l = pile();
    let rows = lignes(&l, 40.0);
    assert_eq!(rows.iter().map(|r| l[r.li].name.as_str()).collect::<Vec<_>>(), ["Plein", "Vide", "V1", "V2", "V3", "Réf"]);
    let rang = |nom: &str| l.iter().position(|x| x.name == nom).unwrap();
    let v1 = [rang("V1")];
    // Sur la ligne du dossier vide (40 à 80) : dedans.
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 60.0, true), Some(Cible::Dossier(Pose::Dans(id(&l, "Vide")), 40.0, 40.0)));
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 79.0, true), Some(Cible::Dossier(Pose::Dans(id(&l, "Vide")), 40.0, 40.0)), "jusqu'au bas de sa ligne");
    // Son bord du haut (six points) reste « au-dessus de lui ».
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 43.0, true), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "Vide")), 40.0)));
    // Sur la ligne d'un dossier fermé : dedans aussi.
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 20.0, true), Some(Cible::Dossier(Pose::Dans(id(&l, "Plein")), 0.0, 40.0)));
    // Sur sa propre ligne : rien (un clic qui a tremblé ne range ni ne sort rien).
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 100.0, true), None);
    assert_eq!(cible_du_glisse(&rows, &l, &[rang("V1"), rang("V2")], 150.0, true), None, "ni sur celle d'un autre calque glissé");
    // Entre deux lignes, comme avant : au-dessus du calque de la ligne d'en dessous ; sous la dernière ligne.
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 170.0, true), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "V3")), 160.0)));
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 190.0, true), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "Réf")), 200.0)));
    assert_eq!(cible_du_glisse(&rows, &l, &v1, 300.0, true), Some(Cible::Trait(Pose::Sous(id(&l, "Réf")), 240.0)));
    // Un dossier qu'on glisse n'entre pas dans un dossier : sur la ligne d'un autre dossier, il se pose au-dessus ou au-dessous.
    let vide = [rang("Vide")];
    assert_eq!(cible_du_glisse(&rows, &l, &vide, 10.0, false), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "Plein")), 0.0)));
    assert_eq!(cible_du_glisse(&rows, &l, &vide, 30.0, false), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "Vide")), 40.0)));
    // La ligne fine d'un dossier dans la bande du montage (13 points) : son bord du haut n'en est que le quart.
    let fines = lignes(&l, 13.0);
    assert_eq!(cible_du_glisse(&fines, &l, &v1, 2.0, true), Some(Cible::Trait(Pose::AuDessusDe(id(&l, "Plein")), 0.0)));
    assert_eq!(cible_du_glisse(&fines, &l, &v1, 5.0, true), Some(Cible::Dossier(Pose::Dans(id(&l, "Plein")), 0.0, 13.0)));
    // Une liste sans ligne : rien.
    assert_eq!(cible_du_glisse(&[], &l, &v1, 5.0, true), None);
}

// ------------------------------------------------------------------------------------ à l'écran

fn parent(app: &crate::app::App, nom: &str) -> Option<String> {
    let l = app.layers();
    l[rang(app, nom)].parent.and_then(|p| l.iter().find(|f| f.id == p)).map(|f| f.name.clone())
}

#[test]
fn on_screen_chosen_layers_dragged_onto_a_folder_of_the_layers_panel_go_in() {
    // « Dossier 2 » fermé. Le panneau Calques, de haut en bas : Dossier 2, V13, Dossier 1 [Dessin, V5, V4], Réf.
    let mut b = banc_de_retakes(true, "V13");
    b.app.set_espace(Espace::Storyboard);
    b.app.show_pane(crate::dock::Pane::Layers);
    b.chauffer(8);
    // « V13 » (courant) et « Réf » choisis.
    let reference = rang(&b.app, "Réf");
    b.app.click_layer_head(reference, true, false);
    b.chauffer(3);
    assert_eq!(b.app.chosen_layers().len(), 2);
    let n0 = etapes(&b);
    // (Le pointeur arrive d'ailleurs : son premier geste se perd dans le banc d'essai.)
    b.image(vec![Event::PointerMoved(pos2(900.0, 300.0))], 1.0 / 60.0);
    b.chauffer(2);
    // On prend « V13 » par son nom et on le lâche au milieu du rang de « Dossier 1 ».
    let (de, sur) = (nom_au_panneau(&b, "V13"), nom_au_panneau(&b, "Dossier 1"));
    glisser(&mut b, de.center(), pos2(sur.center().x, sur.center().y + 10.0));
    assert_eq!((parent(&b.app, "V13"), parent(&b.app, "Réf")), (Some("Dossier 1".to_string()), Some("Dossier 1".to_string())), "{}", b.app.status);
    assert_eq!(etapes(&b), n0 + 1, "une seule étape d'annulation");
    assert!(b.app.status.contains("2 calques rangés dans « Dossier 1 »"), "{}", b.app.status);
    // En haut du dossier, dans leur ordre ; le calque courant et le choix n'ont pas changé.
    let fi = rang(&b.app, "Dossier 1");
    let enfants: Vec<String> = crate::folders::children_of(b.app.layers(), fi).iter().rev().map(|&i| b.app.layers()[i].name.clone()).collect();
    assert_eq!(enfants, ["V13", "Réf", "Dessin", "V5", "V4"]);
    assert_eq!(b.app.layers()[b.app.cur_layer].name, "V13");
    assert_eq!(b.app.chosen_layers().len(), 2, "les deux calques restent choisis");
    // Ctrl+Z rend tout.
    b.app.undo();
    b.chauffer(4);
    assert_eq!((parent(&b.app, "V13"), parent(&b.app, "Réf")), (None, None));
    // Un calque seul, lâché sur un dossier fermé : dedans, et le dossier reste fermé.
    b.image(Vec::new(), 0.5);
    let reference = rang(&b.app, "Réf");
    b.app.click_layer_head(reference, false, false);
    b.chauffer(4);
    let (de, sur) = (nom_au_panneau(&b, "Réf"), nom_au_panneau(&b, "Dossier 2"));
    glisser(&mut b, de.center(), pos2(sur.center().x, sur.center().y + 10.0));
    assert_eq!(parent(&b.app, "Réf"), Some("Dossier 2".to_string()), "{}", b.app.status);
    assert!(b.app.layers()[rang(&b.app, "Dossier 2")].collapsed, "le dossier reste fermé");
    assert!(b.app.status.contains("1 calque rangé dans « Dossier 2 »"), "{}", b.app.status);
    assert_eq!(parent(&b.app, "V13"), None, "« V13 » n'était plus choisi : il n'a pas suivi");
}

#[test]
fn on_screen_a_layer_dragged_onto_the_thin_line_of_a_folder_in_the_strip_goes_in() {
    // La colonne de la bande du montage, toute la pile : les calques et les lignes fines des dossiers.
    let mut b = banc_de_retakes(true, "V13");
    b.app.settings.montage_pile = Pile::Toute;
    b.chauffer(6);
    let n0 = etapes(&b);
    b.image(vec![Event::PointerMoved(pos2(900.0, 300.0))], 1.0 / 60.0);
    b.chauffer(2);
    let t = colonne(&b);
    let (de, sur) = (trouver(&t, "V13")[0], trouver(&t, "Dossier 1")[0]);
    glisser(&mut b, de.center(), sur.center());
    assert_eq!(parent(&b.app, "V13"), Some("Dossier 1".to_string()), "{}", b.app.status);
    assert_eq!(etapes(&b), n0 + 1);
    assert!(b.app.status.contains("1 calque rangé dans « Dossier 1 »"), "{}", b.app.status);
    // Et il en ressort, lâché au-dessus de la ligne de « Dossier 2 » (tout en haut de la colonne).
    b.image(Vec::new(), 0.5);
    let t = colonne(&b);
    let (de, sur) = (trouver(&t, "V13")[0], trouver(&t, "Dossier 2")[0]);
    glisser(&mut b, de.center(), pos2(sur.center().x, sur.top() - 4.0));
    assert_eq!(parent(&b.app, "V13"), None, "{}", b.app.status);
    assert!(b.app.status.contains("sorti de « Dossier 1 »"), "{}", b.app.status);
}
