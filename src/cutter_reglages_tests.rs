//! Partie 169 : les réglages du Cutter dans l'application, au vrai geste sur le banc (`perf_tests::Banc`) : la portée « tous les
//! calques visibles » coupe contre un autre calque sans le modifier ; plusieurs calques choisis, une seule étape d'annulation ; le
//! panneau (ses dessins, le bloc des dépassements en mode Dépassements seulement) écrit les réglages du poste.

use eframe::egui::{pos2, Pos2};

use crate::app::{Mode, Tool};
use crate::bulles::vus;
use crate::cutter::{Mode as ModeCutter, Portee};
use crate::document::{Document, Layer, LayerKind};
use crate::multicalques_tests::{banc, cel_de, glisser, ligne};
use crate::perf_tests::{banc_taille, Banc};
use crate::vector::{Element, VPoint};

fn trait_(a: (f32, f32), b: (f32, f32)) -> Element {
    let n = 20;
    Element::Stroke {
        points: (0..=n).map(|k| {
            let t = k as f32 / n as f32;
            VPoint { x: a.0 + (b.0 - a.0) * t, y: a.1 + (b.1 - a.1) * t, r: 2.0 }
        }).collect(),
        color: [20, 20, 20],
        opacity: 1.0,
        hardness: 1.0,
        brush: None,
        trace: Default::default(),
    }
}

/// Deux calques vectoriels : une horizontale sur le calque courant (0), une verticale en x 200 sur l'autre (1), non choisi.
fn deux_calques() -> Banc {
    let mut doc = Document::new(400, 300, 24);
    doc.panels[0].layers.clear();
    doc.panels[0].length = 4;
    for (i, els) in [vec![trait_((110.0, 150.0), (290.0, 150.0))], vec![trait_((200.0, 100.0), (200.0, 200.0))]].into_iter().enumerate() {
        let id = doc.new_cel(LayerKind::Vector);
        let c = doc.cels.get_mut(&id).unwrap();
        c.vector.as_mut().unwrap().elements = els;
        c.rerender();
        let mut l = Layer::new(&format!("Calque {i}"), LayerKind::Vector);
        l.set(0, Some(id));
        doc.panels[0].layers.push(l);
    }
    let mut b = banc_taille(doc, false, 1280, 720);
    b.app.set_mode(Mode::Animation);
    (b.app.cur_panel, b.app.cur_layer, b.app.cur_frame) = (0, 0, 0);
    b.app.set_tool(Tool::Cutter);
    b.chauffer(8);
    assert_eq!(b.app.chosen_layers().len(), 1);
    b
}

fn bouts(b: &Banc, li: usize) -> Vec<(Pos2, Pos2)> {
    let d = b.app.doc.cels[&cel_de(b, li, 0)].vector.clone().unwrap();
    d.elements
        .iter()
        .filter_map(|e| match e {
            Element::Stroke { points, .. } => Some((points[0].pos().round(), points[points.len() - 1].pos().round())),
            _ => None,
        })
        .collect()
}

#[test]
fn all_visible_layers_cut_without_changing_the_other_layer() {
    // Le calque actif seulement : l'horizontale ne croise rien de son calque et part en entier.
    let mut b = deux_calques();
    let autre = bouts(&b, 1);
    glisser(&mut b, &ligne(pos2(250.0, 135.0), pos2(250.0, 165.0), 6));
    assert!(bouts(&b, 0).is_empty(), "{:?}", bouts(&b, 0));
    // Tous les calques visibles : coupée contre la verticale de l'autre calque, qui ne change pas.
    let mut b = deux_calques();
    b.app.settings.cutter.portee = Portee::Visibles;
    glisser(&mut b, &ligne(pos2(250.0, 135.0), pos2(250.0, 165.0), 6));
    assert_eq!(bouts(&b, 0), vec![(pos2(110.0, 150.0), pos2(200.0, 150.0))]);
    assert_eq!(bouts(&b, 1), autre, "l'autre calque ne bouge pas");
    // Caché, l'autre calque ne coupe plus.
    let mut b = deux_calques();
    b.app.settings.cutter.portee = Portee::Visibles;
    b.app.doc.panels[0].layers[1].visible = false;
    glisser(&mut b, &ligne(pos2(250.0, 135.0), pos2(250.0, 165.0), 6));
    assert!(bouts(&b, 0).is_empty());
}

#[test]
fn several_chosen_layers_are_cut_in_one_undo_step() {
    let mut b = banc(&[LayerKind::Vector, LayerKind::Vector, LayerKind::Vector], Tool::Cutter);
    let avant: Vec<_> = (0..3).map(|li| bouts(&b, li)).collect();
    let etapes = b.app.history.names().0.len();
    // Un geste vertical en travers des trois traits (y 60, 105, 150) : chacun ne croise rien et part en entier.
    glisser(&mut b, &ligne(pos2(200.0, 40.0), pos2(200.0, 170.0), 10));
    for li in 0..3 {
        assert!(bouts(&b, li).is_empty(), "calque {li} : {:?}", bouts(&b, li));
    }
    assert_eq!(b.app.history.names().0.len(), etapes + 1, "une seule étape");
    assert!(b.app.status.contains("3 calques") && b.app.status.contains("3 bouts"), "{}", b.app.status);
    b.app.undo();
    for li in 0..3 {
        assert_eq!(bouts(&b, li), avant[li], "calque {li} rendu par une seule annulation");
    }
}

#[test]
fn the_panel_shows_drawings_and_the_overshoot_block_only_in_that_mode() {
    let mut b = banc(&[LayerKind::Vector], Tool::Cutter);
    vus::vider();
    b.chauffer(2);
    for nom in ["Couper", "Nettoyer les dépassements", "Glisser en travers", "Lasso", "Écart toléré", "Bouts effilés", "Calque actif", "Tous les calques visibles"] {
        assert!(!vus::ou(nom).is_empty(), "« {nom} » dans le panneau : {:?}", vus::noms());
    }
    assert!(vus::ou("Dépassements enlevés jusqu'à").is_empty(), "le bloc des dépassements n'est pas en mode Couper");
    // Un clic sur la tuile « Nettoyer les dépassements » : le mode change, le bloc paraît.
    let r = vus::ou("Nettoyer les dépassements")[0];
    crate::pistes_tests::clic(&mut b, r.center(), eframe::egui::Modifiers::NONE);
    assert_eq!(b.app.settings.cutter.mode, ModeCutter::Depassements);
    vus::vider();
    b.chauffer(2);
    assert!(!vus::ou("Dépassements enlevés jusqu'à").is_empty(), "{:?}", vus::noms());
}
