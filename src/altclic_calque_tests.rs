//! Partie 165.8 (Anthony : « Alt+clic n'est plus la pipette, il permet de sélectionner le calque cliqué ») : dans la vue de dessin,
//! Alt + clic rend actif le calque du plan courant dont l'encre est sous le curseur (le plus haut qu'on voit) ; les calques cachés et
//! ceux à l'opacité 0 sont ignorés, un calque d'un dossier fermé devient actif quand même ; dans le vide, rien ne change et la barre
//! d'état le dit. Le geste n'écrit rien dans le projet (aucune étape d'annulation) et ne touche pas à la couleur. L'outil Pipette reste
//! la pipette.

use eframe::egui::{Color32, Modifiers};

use crate::app::Tool;
use crate::degrade_tests::{banc_degrade, glisser};
use crate::document::{CelId, Document, Layer, LayerKind};
use crate::perf_tests::Banc;
use crate::pistes_tests::etapes;
use crate::raster::TILE;

const ROUGE: Color32 = Color32::from_rgb(220, 20, 20);
const BLEU: Color32 = Color32::from_rgb(20, 40, 220);
const VERT: Color32 = Color32::from_rgb(20, 180, 60);

/// Un carré plein de `c` de (x0, y0) à (x1, y1) exclus, dans le dessin `cel`.
fn carre(doc: &mut Document, cel: CelId, (x0, y0): (usize, usize), (x1, y1): (usize, usize), c: Color32) {
    let d = doc.cels.get_mut(&cel).expect("le dessin");
    for y in y0..y1 {
        for x in x0..x1 {
            let i = d.raster.tile_index_of(x, y);
            d.raster.tile_mut(i)[(y % TILE) * TILE + x % TILE] = c;
        }
    }
    d.version += 1;
}

/// Un plan de 400 × 300 : du bas vers le haut, « Bas » (rouge, de 50 à 150), « Haut » (bleu, de 100 à 200 : il couvre le coin de
/// « Bas »), « Caché » (vert, de 250 à 350, l'œil fermé) et « Transparent » (vert, de 250 à 350 aussi, opacité 0). Le calque courant
/// est « Bas », l'outil le pinceau, la couleur noire.
fn banc_alt() -> Banc {
    let mut b = banc_degrade(LayerKind::Bitmap);
    let doc = &mut b.app.doc;
    let mut calques = Vec::new();
    for (nom, de, a, c) in [("Bas", (50, 50), (150, 150), ROUGE), ("Haut", (100, 100), (200, 200), BLEU), ("Caché", (250, 50), (350, 150), VERT), ("Transparent", (250, 50), (350, 150), VERT)] {
        let cel = doc.new_cel(LayerKind::Bitmap);
        carre(doc, cel, de, a, c);
        let mut l = Layer::new(nom, LayerKind::Bitmap);
        l.frames = vec![Some(cel); 4];
        calques.push(l);
    }
    calques[2].visible = false;
    calques[3].opacity = 0.0;
    doc.panels[0].layers = calques;
    b.app.cur_layer = 0;
    b.app.set_tool(Tool::Brush);
    b.app.color = [0, 0, 0];
    b.chauffer(6);
    b
}

fn nom_courant(b: &Banc) -> String {
    b.app.layers()[b.app.cur_layer].name.clone()
}

/// Un Alt + clic au point (x, y) de la feuille.
fn alt_clic(b: &mut Banc, x: f32, y: f32) {
    glisser(b, &[(x, y)], Modifiers::ALT);
    b.chauffer(2);
}

#[test]
fn alt_click_on_two_overlapping_layers_takes_the_one_on_top() {
    let mut b = banc_alt();
    let (n, couleur, modifie) = (etapes(&b), b.app.color, b.app.modified);
    // Là où « Haut » couvre « Bas » : « Haut », le plus haut qu'on voit.
    alt_clic(&mut b, 120.0, 120.0);
    assert_eq!(nom_courant(&b), "Haut", "{}", b.app.status);
    assert_eq!(b.app.status, "Calque actif : Haut");
    // Là où seul « Bas » a de l'encre : « Bas ».
    alt_clic(&mut b, 60.0, 60.0);
    assert_eq!(nom_courant(&b), "Bas", "{}", b.app.status);
    assert_eq!(b.app.status, "Calque actif : Bas");
    // Ni annulation, ni couleur prise, ni trait : le geste ne change pas le projet.
    assert_eq!(etapes(&b), n, "aucune étape d'annulation");
    assert_eq!(b.app.color, couleur, "la couleur n'a pas changé (ce n'est plus la pipette)");
    assert_eq!(b.app.modified, modifie, "rien d'écrit dans le projet");
}

#[test]
fn hidden_layers_and_layers_at_zero_opacity_are_skipped() {
    let mut b = banc_alt();
    // Sous le curseur, seuls « Caché » (l'œil fermé) et « Transparent » (opacité 0) ont de l'encre : rien n'est choisi.
    alt_clic(&mut b, 300.0, 100.0);
    assert_eq!(nom_courant(&b), "Bas");
    assert_eq!(b.app.status, "Aucun dessin sous le curseur");
    // L'œil rouvert, « Caché » répond (« Transparent », au-dessus, reste ignoré).
    b.app.doc.panels[0].layers[2].visible = true;
    alt_clic(&mut b, 300.0, 100.0);
    assert_eq!(nom_courant(&b), "Caché", "{}", b.app.status);
}

#[test]
fn alt_click_in_the_void_changes_nothing() {
    let mut b = banc_alt();
    b.app.cur_layer = 1;
    b.chauffer(2);
    let (n, couleur) = (etapes(&b), b.app.color);
    alt_clic(&mut b, 30.0, 250.0);
    assert_eq!(nom_courant(&b), "Haut", "le calque actif ne change pas");
    assert_eq!(b.app.status, "Aucun dessin sous le curseur");
    assert_eq!((etapes(&b), b.app.color), (n, couleur));
    assert!(b.app.layers().iter().all(|l| l.frames.iter().all(Option::is_some)), "aucun trait, aucun dessin de plus");
}

#[test]
fn a_layer_in_a_closed_folder_becomes_active_and_the_picker_tool_stays_a_picker() {
    let mut b = banc_alt();
    // « Haut » rangé dans un dossier fermé, posé tout en haut de la pile.
    let mut dossier = Layer::new_folder("Dossier");
    dossier.collapsed = true;
    let id = dossier.id;
    b.app.doc.panels[0].layers[1].parent = Some(id);
    b.app.doc.panels[0].layers.insert(2, dossier);
    b.chauffer(2);
    alt_clic(&mut b, 150.0, 150.0);
    assert_eq!(nom_courant(&b), "Haut", "{}", b.app.status);
    // L'outil Pipette prend toujours la couleur, sans changer de calque.
    b.app.set_tool(Tool::Picker);
    b.chauffer(1);
    glisser(&mut b, &[(60.0, 60.0)], Modifiers::NONE);
    assert_eq!(nom_courant(&b), "Haut");
    let [r, g, bl] = b.app.color;
    assert!(r > 200 && g < 40 && bl < 40, "la pipette a pris le rouge de « Bas » : {:?}", b.app.color);
}
