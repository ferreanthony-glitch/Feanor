//! Travailler à plusieurs : ce que ça coûte (`docs/PERFORMANCE.md`, partie « Coopération »). Les bornes sont larges ; les chiffres,
//! eux, s'impriment avec `cargo test --release cooperation_mesures -- --nocapture --include-ignored`.

use std::time::Instant;

use eframe::egui::Color32;

use crate::cooperation::Horloge;
use crate::cooperation_tests::{confirmer, film, partager, Poste};
use crate::depot::{Curseur, Depot, DossierPartage};
use crate::document::{Cel, LayerKind};
use crate::fiabilite_fixture as fx;
use crate::project::{self, partage_io};
use crate::verrous::Cible;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1e3
}

/// Un film de `n` plans dont le premier est chargé : 8 calques de 12 dessins.
fn gros_film(n: usize) -> crate::document::Document {
    let mut doc = film(n);
    let mut layers = Vec::new();
    for l in 0..8 {
        let mut layer = crate::document::Layer::new(&format!("Calque {l}"), LayerKind::Bitmap);
        for k in 0..12 {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            cel.raster.tile_mut(0)[(l * 12 + k) % 4] = Color32::from_rgba_premultiplied(30 + l as u8, 40 + k as u8, 50, 255);
            layer.frames.push(Some(doc.add_cel(cel)));
        }
        layers.push(layer);
    }
    doc.panels[0].layers = layers;
    doc.panels[0].length = 12;
    doc
}

#[test]
#[ignore = "mesure : cargo test --release cooperation_mesures -- --nocapture --include-ignored"]
fn cooperation_mesures_a_big_shared_film() {
    let racine = fx::dossier("coop-mesures");
    let t = Instant::now();
    let manifeste = partager(&gros_film(600), &racine);
    println!("partager 600 plans (1 gros) : {:.0} ms", ms(t));
    let dossier = DossierPartage::nouveau(&manifeste);
    let mut c = Curseur::default();
    let t = Instant::now();
    dossier.surveiller(&mut c).expect("balayage");
    println!("balayage, premier : {:.2} ms", ms(t));
    let t = Instant::now();
    for _ in 0..20 {
        dossier.surveiller(&mut c).expect("balayage");
    }
    let balayage = ms(t) / 20.0;
    println!("balayage, rien de changé (600 plans) : {balayage:.2} ms");
    let t = Instant::now();
    let loaded = project::load(&manifeste).expect("ouverture");
    println!("ouverture exacte du film partagé : {:.0} ms", ms(t));
    let h = Horloge::reglable(1_760_000_000_000);
    let mut a = Poste::ouvrir(&manifeste, "Anthony", &h);
    let mut b = Poste::ouvrir(&manifeste, "Lea", &h);
    drop(loaded);
    // Un tour du moteur sans rien de neuf.
    a.tour();
    let t = Instant::now();
    for _ in 0..20 {
        a.s.pomper();
    }
    let tour = ms(t) / 20.0;
    println!("tour du moteur, rien de neuf : {tour:.2} ms (sur le fil de fond)");
    // L'interface au repos.
    let t = Instant::now();
    for _ in 0..2000 {
        a.s.sonder(&mut a.doc);
    }
    let sonder = ms(t) / 2000.0 * 1e3;
    println!("session.sonder, au repos : {sonder:.2} µs par image d'écran");
    // Anthony enregistre le gros plan ; Léa le reçoit : lecture sur le fil, pose sur l'interface.
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    a.tour();
    a.dessiner(0, 99);
    let t = Instant::now();
    a.enregistrer();
    println!("enregistrer un plan de 96 dessins (cliché + écriture, sur un fil en vrai) : {:.1} ms", ms(t));
    b.s.pomper();
    let id = b.plan_id(0);
    let octets = std::fs::read(fx::racine(&manifeste).join("plans").join(format!("{id}.json"))).expect("plan");
    let tete = project::Tete::de(&b.doc);
    let t = Instant::now();
    let pc = partage_io::charger_plan(fx::racine(&manifeste), &tete, id, &octets, true).expect("plan lu");
    let lecture = ms(t);
    println!("lire et décoder le plan de 96 dessins (fil de fond) : {lecture:.1} ms");
    let t = Instant::now();
    b.doc.installer_plan(pc).ok().expect("posé");
    let pose = ms(t);
    println!("poser ce plan dans le document (interface) : {:.3} ms", pose);
    // Le montage : relire la structure de 600 plans (fil) et la poser (interface).
    let octets = std::fs::read(&manifeste).expect("structure");
    let t = Instant::now();
    let lue = partage_io::charger_structure(fx::racine(&manifeste), &octets, &Default::default()).expect("structure");
    println!("relire la structure de 600 plans (fil de fond) : {:.1} ms", ms(t));
    let t = Instant::now();
    let mut attente = std::collections::HashMap::new();
    b.doc.adopter_structure(lue.doc, &mut attente);
    let adoption = ms(t);
    println!("poser la structure (interface) : {adoption:.3} ms");
    // L'enregistrement côté interface avec 50 plans tenus sur 600.
    let ids: Vec<Cible> = (0..50).map(|i| Cible::Plan(b.plan_id(i + 1))).collect();
    b.s.prendre(&ids, false);
    confirmer(&h);
    b.tour();
    let t = Instant::now();
    let ctx = b.s.contexte_ecriture(&b.doc, false);
    let mut ecrits = std::collections::HashMap::new();
    let job = partage_io::preparer(&b.doc, &b.manifeste, &mut ecrits, ctx).expect("préparation");
    let cliche = ms(t);
    println!("cliché d'un enregistrement (50 plans tenus sur 600, interface) : {cliche:.2} ms");
    let t = Instant::now();
    job.run_with(None, 4).expect("écriture");
    println!("… écriture de ces 50 plans (fil de fond) : {:.0} ms", ms(t));
    assert!(balayage < 250.0, "balayage {balayage} ms");
    assert!(sonder < 200.0, "sonder {sonder} µs");
    assert!(pose < 20.0 && adoption < 100.0, "pose {pose} ms, structure {adoption} ms");
    assert!(tour < 400.0, "tour {tour} ms");
}
