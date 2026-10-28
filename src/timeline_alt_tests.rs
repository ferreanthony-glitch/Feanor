//! Essais d'**Alt + glisser** les images choisies de la timeline (Anthony, 2026-10-28) : elles sont dupliquées à l'endroit lâché (Ctrl fait de
//! même, Maj insère, sans touche déplace) ; les copies ont leurs propres dessins ; une seule étape d'annulation ; Échap abandonne.
//! (Les petits outils de ce fichier servent aussi aux essais du collage, `timeline_cles_tests.rs`.)

use std::collections::BTreeSet;

use eframe::egui::{Color32, Modifiers};

use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::fiabilite_fixture::{instantane, meme_projet};
use crate::images::DropMode;
use crate::perf_tests::{banc_taille, Banc};
use crate::workspace::Espace;

/// Deux plans de 10 images ; un calque « Dessin » à cinq dessins d'une image (images 0 à 4), puis rien. Rend les dessins de chaque plan.
pub(crate) fn film() -> (Document, Vec<Vec<CelId>>) {
    let mut doc = Document::with_frame(32, 24, 24);
    doc.add_scene_after(0);
    let mut cels = Vec::new();
    for k in 0..2 {
        doc.panels[k].layers.clear();
        doc.panels[k].set_length(10);
        let mut ids = Vec::new();
        let mut l = Layer::new("Dessin", LayerKind::Bitmap);
        for _ in 0..5 {
            let mut c = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            // (Un pixel qui dit de quel dessin il s'agit.)
            c.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(10 + (ids.len() + 20 * k) as u8, 10, 10, 255);
            let id = doc.add_cel(c);
            ids.push(id);
            l.frames.push(Some(id));
        }
        doc.panels[k].layers.push(l);
        cels.push(ids);
    }
    (doc, cels)
}

/// Le film dans l'espace Animation (la timeline est à l'écran), au plan 0.
pub(crate) fn banc(doc: Document) -> Banc {
    let mut b = banc_taille(doc, false, 1000, 700);
    b.app.set_espace(Espace::Animation);
    b.app.set_panel(0);
    (b.app.cur_layer, b.app.cur_frame) = (0, 0);
    b.chauffer(4);
    b
}

pub(crate) fn photo(b: &Banc) -> String {
    instantane(&b.app.doc, &BTreeSet::new())
}

/// Les images du calque 0 du plan `plan`.
pub(crate) fn images(b: &Banc, plan: usize) -> Vec<Option<CelId>> {
    b.app.doc.panels[plan].layers[0].frames.clone()
}

/// Le rouge du premier pixel d'un dessin (de quoi le reconnaître).
pub(crate) fn rouge(b: &Banc, id: CelId) -> u8 {
    b.app.doc.cels[&id].raster.get(0, 0).r()
}

/// Lâche (au prochain affichage de la timeline) des images prises à la colonne `de` jusqu'à la colonne `a`, avec les touches `m` tenues.
fn lacher(b: &mut Banc, de: usize, a: usize, m: Modifiers) {
    b.app.images_focus = true;
    b.app.image_drag = Some((de, a));
    b.touches = m;
    b.chauffer(2);
    b.touches = Modifiers::NONE;
    b.chauffer(1);
}

const ALT: Modifiers = Modifiers { alt: true, ..Modifiers::NONE };
const CTRL: Modifiers = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
const MAJ: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };

#[test]
fn the_keys_held_at_the_drop_choose_the_gesture() {
    assert_eq!(DropMode::de_touches(&Modifiers::NONE), DropMode::Move);
    assert_eq!(DropMode::de_touches(&ALT), DropMode::Copy, "Alt copie");
    assert_eq!(DropMode::de_touches(&CTRL), DropMode::Copy, "Ctrl copie toujours");
    assert_eq!(DropMode::de_touches(&MAJ), DropMode::Insert, "Maj insère toujours");
    assert_eq!(DropMode::de_touches(&Modifiers { alt: true, shift: true, ..Modifiers::NONE }), DropMode::Copy);
}

#[test]
fn alt_drag_duplicates_the_chosen_images_with_their_own_drawings() {
    let (doc, cels) = film();
    let mut b = banc(doc);
    b.app.frame_sel = Some((1, 3));
    let avant = photo(&b);
    let n = b.app.history.undo_len();
    // Prises à l'image 1, lâchées à l'image 6 : cinq images plus loin.
    lacher(&mut b, 1, 6, ALT);
    let f = images(&b, 0);
    assert_eq!(f.len(), 8, "{f:?} : {}", b.app.status);
    // Les originaux sont toujours là…
    assert_eq!((f[1], f[2]), (Some(cels[0][1]), Some(cels[0][2])), "Alt copie : rien ne quitte sa place");
    // …et les copies, à l'endroit lâché, sont de nouveaux dessins qui ont les mêmes pixels.
    let (c6, c7) = (f[6].expect("copie"), f[7].expect("copie"));
    assert!(!cels[0].contains(&c6) && !cels[0].contains(&c7) && c6 != c7, "des dessins à elles");
    assert_eq!((rouge(&b, c6), rouge(&b, c7)), (rouge(&b, cels[0][1]), rouge(&b, cels[0][2])));
    // Une seule étape d'annulation.
    assert_eq!(b.app.history.undo_len(), n + 1);
    b.app.run_action("edition.annuler");
    b.chauffer(1);
    meme_projet(&avant, &photo(&b)).expect("une annulation rend le plan");
}

#[test]
fn ctrl_still_copies_shift_still_inserts_and_no_key_moves() {
    // Ctrl : la même copie.
    let (doc, cels) = film();
    let mut b = banc(doc);
    b.app.frame_sel = Some((1, 3));
    lacher(&mut b, 1, 6, CTRL);
    let f = images(&b, 0);
    assert_eq!((f[1], f[2]), (Some(cels[0][1]), Some(cels[0][2])));
    assert!(f[6].is_some() && !cels[0].contains(&f[6].unwrap()));
    // Sans touche : déplacées (la place qu'elles quittent est vide).
    let (doc, cels) = film();
    let mut b = banc(doc);
    b.app.frame_sel = Some((1, 3));
    lacher(&mut b, 1, 6, Modifiers::NONE);
    let f = images(&b, 0);
    assert_eq!((f[1], f[2]), (None, None));
    assert_eq!((f[6], f[7]), (Some(cels[0][1]), Some(cels[0][2])), "les mêmes dessins, déplacés");
    // Maj : insérées (la suite recule pour elles).
    let (doc, cels) = film();
    let mut b = banc(doc);
    b.app.frame_sel = Some((1, 3));
    lacher(&mut b, 1, 2, MAJ);
    let f = images(&b, 0);
    assert_eq!(&f[..5], &[Some(cels[0][0]), Some(cels[0][3]), Some(cels[0][1]), Some(cels[0][2]), Some(cels[0][4])], "{f:?}");
}

#[test]
fn escape_abandons_an_alt_drag() {
    let (doc, _) = film();
    let mut b = banc(doc);
    b.app.frame_sel = Some((1, 3));
    b.app.images_focus = true;
    let avant = photo(&b);
    let n = b.app.history.undo_len();
    b.app.image_drag = Some((1, 6));
    b.touches = ALT;
    // Échap (ou Ctrl+Z) pendant le geste : tout revient comme avant, rien n'est enregistré.
    b.app.cancel_drags();
    b.chauffer(2);
    b.touches = Modifiers::NONE;
    assert!(b.app.image_drag.is_none());
    assert_eq!(b.app.history.undo_len(), n);
    meme_projet(&avant, &photo(&b)).expect("rien n'a changé");
}
