//! La timeline plus claire (partie 164) : des lignes toutes pareilles, seulement l'exception à droite du nom, la couleur du
//! rangement sur toute la ligne, et la barre du calque courant au-dessus des lignes.

use eframe::egui::Color32;

use crate::blend::BlendMode;
use crate::pistes_tests::{app_with, doc_with};

#[test]
fn a_normal_layer_shows_nothing_and_each_exception_says_what_it_is() {
    let mut app = app_with("exceptions", doc_with(&[&["A", "B"]]));
    assert!(app.exceptions_du_calque(0).is_empty(), "un calque normal : rien à droite du nom");
    // (L'œil et le cadenas ont leur propre case : un calque caché ou verrouillé n'ajoute rien ici.)
    app.layers_mut()[0].visible = false;
    app.layers_mut()[0].locked = true;
    assert!(app.exceptions_du_calque(0).is_empty());
    {
        let l = &mut app.layers_mut()[0];
        l.opacity = 0.6;
        l.blend = BlendMode::Multiply;
        l.stencil = true;
        l.keep_alpha = true;
        l.position_locked = true;
    }
    let textes: Vec<String> = app.exceptions_du_calque(0).into_iter().map(|(_, t)| t).collect();
    assert_eq!(textes[0], "60 %");
    assert_eq!(textes[1], BlendMode::Multiply.label());
    assert!(textes.contains(&"Pochoir".to_string()) && textes.contains(&"Position verrouillée".to_string()), "{textes:?}");
    assert!(app.exceptions_du_calque(1).is_empty(), "l'autre calque reste normal");
}

#[test]
fn a_row_takes_the_colour_of_its_folder_or_group_and_a_folder_takes_it_stronger() {
    use crate::layerheads::fond_teinte;
    let base = Color32::from_rgb(40, 40, 40);
    let rouge = Color32::from_rgb(240, 40, 40);
    assert_eq!(fond_teinte(base, None, false), base, "sans rangement : le fond d'origine");
    let calque = fond_teinte(base, Some(rouge), false);
    let dossier = fond_teinte(base, Some(rouge), true);
    assert!(calque.r() > base.r() && calque.r() < rouge.r(), "une teinte légère : {calque:?}");
    assert!(dossier.r() > calque.r(), "le dossier, plus fort : il sert de titre");
    assert_eq!((calque.g(), calque.b()), (40, 40));
}

/// À regarder (hors de la suite) : la timeline de l'animation sur un plan de 48 images et 15 calques rangés dans cinq dossiers, dont
/// trois fermés ; le calque courant est « Chat trait ». Une capture dans `FEANOR_CAPTURE_DIR`. Sans carte graphique : rien.
#[test]
#[ignore = "écrit une capture : FEANOR_CAPTURE_DIR"]
fn timeline_claire_capture() {
    use crate::document::{Document, Layer, LayerKind};
    use crate::workspace::Espace;
    let dossier = std::env::var_os("FEANOR_CAPTURE_DIR").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dossier).unwrap();
    const FR: usize = 48;
    // Du haut vers le bas : (nom, dossier, sorte de clés) ; « » : un dossier.
    let pile: &[(&str, &str, u8)] = &[
        ("Lina", "", 0), ("Lina trait", "Lina", 1), ("Lina yeux", "Lina", 1), ("Lina couleur", "Lina", 1), ("Lina ombre", "Lina", 1),
        ("Chat", "", 0), ("Chat trait", "Chat", 1), ("Chat couleur", "Chat", 1),
        ("Effets", "", 0), ("Poussière", "Effets", 2), ("Pluie", "Effets", 2), ("Lueur", "Effets", 2),
        ("Décor", "", 0), ("Ciel", "Décor", 3), ("Maison", "Décor", 3), ("Premier plan", "Décor", 3),
        ("Notes", "", 0), ("Notes réal", "Notes", 2), ("Timing", "Notes", 2),
        ("Fond", "", 3),
    ];
    let mut doc = Document::new(1920, 1080, 24);
    doc.frame = [1920, 1080];
    let mut layers: Vec<Layer> = Vec::new();
    let mut ids = std::collections::HashMap::new();
    for &(nom, parent, sorte) in pile {
        let mut l = if sorte == 0 && nom != "Fond" { Layer::new_folder(nom) } else { Layer::new(nom, LayerKind::Bitmap) };
        if sorte == 0 {
            ids.insert(nom, l.id);
        }
        l.parent = ids.get(parent).copied();
        l.frames = vec![None; FR];
        l.collapsed = matches!(nom, "Effets" | "Décor" | "Notes");
        layers.push(l);
    }
    layers.reverse();
    doc.panels[0].layers = layers;
    doc.panels[0].set_length(FR);
    let mut b = crate::perf_tests::banc(doc, true);
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(6);
    b.app.tool = crate::app::Tool::Brush;
    b.app.brush.size = 24.0;
    let n = b.app.layers().len();
    for li in 0..n {
        let (nom, sorte) = { let l = &b.app.layers()[li]; (l.name.clone(), pile.iter().find(|p| p.0 == l.name).map_or(0, |p| p.2)) };
        if b.app.layers()[li].folder {
            continue;
        }
        (b.app.cur_layer, b.app.cur_frame) = (li, 0);
        b.app.color = [(li * 40 % 255) as u8, 90, 160];
        let p0 = b.app.to_layer(eframe::egui::pos2(300.0 + li as f32 * 60.0, 300.0));
        b.app.begin_paint(p0, 1.0, false);
        for k in 1..12 {
            let p = b.app.to_layer(eframe::egui::pos2(300.0 + li as f32 * 60.0 + k as f32 * 20.0, 300.0 + k as f32 * 15.0));
            b.app.stroke_to(p, 1.0);
        }
        b.app.end_stroke();
        let Some(a) = b.app.layers()[li].frames.iter().copied().flatten().next() else { panic!("pas de dessin sur {nom}") };
        let autres: Vec<_> = (0..3).map(|_| { let c = b.app.doc.cels[&a].clone(); b.app.doc.add_cel(c) }).collect();
        let cles = [a, autres[0], autres[1], autres[2]];
        let mut s: u64 = (li as u64 + 3) * 7919;
        let mut r = || { s = (s * 9301 + 49297) % 233280; s as f32 / 233280.0 };
        let mut frames = vec![None; FR];
        let mut f = 0usize;
        let mut k = 0usize;
        match sorte {
            3 => frames = vec![Some(a); FR],
            2 => {
                f = (r() * 8.0) as usize;
                while f < FR {
                    let d = 2 + (r() * 3.0) as usize;
                    for g in f..(f + d).min(FR) { frames[g] = Some(cles[k % 4]); }
                    k += 1;
                    f += d + 3 + (r() * 9.0) as usize;
                }
            }
            _ => {
                while f < FR {
                    let d = 2 + (r() * 3.0) as usize;
                    for g in f..(f + d).min(FR) { frames[g] = Some(cles[k % 4]); }
                    k += 1;
                    f += d;
                }
            }
        }
        b.app.layers_mut()[li].frames = frames;
    }
    // Ce qui sort de l'ordinaire : Lina ombre à 60 % en produit, Pluie cachée, le dossier Décor verrouillé.
    let rang = |b: &crate::perf_tests::Banc, nom: &str| b.app.layers().iter().position(|l| l.name == nom).unwrap();
    let ombre = rang(&b, "Lina ombre");
    b.app.layers_mut()[ombre].opacity = 0.6;
    b.app.layers_mut()[ombre].blend = crate::blend::BlendMode::Multiply;
    let pluie = rang(&b, "Pluie");
    b.app.layers_mut()[pluie].visible = false;
    let decor = rang(&b, "Décor");
    b.app.layers_mut()[decor].locked = true;
    b.app.renderer.invalidate_all();
    b.app.set_espace(Espace::Animation);
    b.app.cur_layer = rang(&b, "Chat trait");
    b.app.cur_frame = 20;
    b.chauffer(10);
    let Some(rgba) = b.capturer() else { return println!("TIMELINE pas de carte graphique") };
    image::save_buffer(dossier.join("timeline-claire.png"), &rgba, 1920, 1080, image::ColorType::Rgba8).expect("png");
    println!("TIMELINE timeline-claire.png");
}
