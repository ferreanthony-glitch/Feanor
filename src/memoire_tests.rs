//! Mémoire des dessins (`residence.rs`) : mesures sur un gros projet (`#[ignore]`, lancées à la main) et essais.
//!
//! Mesure : `cargo test --release --offline --target-dir target\msvc --lib memoire_mesurer -- --ignored --nocapture`,
//! avec `LOURD_PROJET` (le manifeste d'un film fait par `lourd_generer`) ; `MEMOIRE_BUDGET_MO` : le budget des dessins
//! (0 : sans limite, comme avant).

use std::time::Instant;

use eframe::egui::{vec2, Event, Modifiers};

use crate::lourd_tests::{appui, jusqu_au_calme, memoire, ouvrir_mesure, rapport, scene};
use crate::perf_tests::{Banc, Mesure};

/// Ce que les dessins tiennent en mémoire : tuiles des dessins bitmap, des images des dessins vectoriels, des strates
/// Fond, des étapes d'annulation, et les caches des traits.
fn poids(b: &Banc) -> String {
    let d = &b.app.doc;
    let (mut bit, mut vec, mut fond) = (0usize, 0usize, 0usize);
    for c in d.cels.values() {
        let n = c.raster.tiles.iter().filter(|t| t.is_some()).count() + c.raster.outer.len();
        if c.vector.is_some() {
            vec += n;
        } else {
            bit += n;
        }
        if let Some(s) = &c.fond {
            fond += s.peinture.raster.tiles.iter().filter(|t| t.is_some()).count() + s.coupe.tiles.iter().filter(|t| t.is_some()).count();
        }
    }
    let mo = |n: usize| n as f64 * 16384.0 / 1e6;
    format!(
        "bitmap {:.0} Mo, images vectorielles {:.0} Mo, fonds {:.0} Mo, annulation {:.0} Mo, caches des traits {:.0} Mo",
        mo(bit),
        mo(vec),
        mo(fond),
        mo(b.app.history.tuiles_gardees()),
        crate::vector::cache_total_bytes() as f64 / 1e6
    )
}

fn mem(titre: &str, b: &Banc) {
    let (ws, pv) = memoire();
    println!("MEM {titre:<40} RSS {ws:>7.0} Mo, privée {pv:>7.0} Mo | {}", poids(b));
}

/// Un trait de pinceau sur le dessin, en `n` images.
fn trait_sur_le_dessin(b: &mut Banc, n: usize, k: usize) -> Vec<Mesure> {
    let r = b.app.canvas_rect;
    let zone = r.shrink2(vec2(r.width() * 0.2, r.height() * 0.3));
    let y = zone.top() + zone.height() * ((k % 7) as f32 / 7.0);
    let (de, a) = (eframe::egui::pos2(zone.left(), y), eframe::egui::pos2(zone.right(), y + 20.0));
    let mut m = Vec::new();
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    m.push(b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0));
    for s in 1..=n {
        m.push(b.image(vec![Event::PointerMoved(de + (a - de) * (s as f32 / n as f32))], 1.0 / 60.0));
    }
    m.push(b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0));
    m
}

/// Mesure A/B (deux passages du même essai, `MEMOIRE_BUDGET_MO` = 0 puis le budget) : mémoire après l'ouverture, après
/// avoir parcouru tous les plans, après un long historique ; temps des images aux changements de plan et à la lecture.
#[test]
#[ignore]
fn memoire_mesurer() {
    use crate::app::Tool;
    use crate::workspace::Espace;
    let budget: Option<u64> = std::env::var("MEMOIRE_BUDGET_MO").ok().and_then(|v| v.parse().ok());
    println!("MEMOIRE budget des dessins : {}", budget.map_or("réglage d'office".into(), |b| if b == 0 { "sans limite".into() } else { format!("{b} Mo") }));
    let t0 = Instant::now();
    let mut b = ouvrir_mesure(Espace::Storyboard);
    println!("MEMOIRE ouvert et au calme en {:.1} s", t0.elapsed().as_secs_f64());
    mem("après l'ouverture (au calme)", &b);
    let n = b.app.doc.panels.len();

    // Parcourir tous les plans, trois images chacun.
    let mut changements = Vec::new();
    let mut suivantes = Vec::new();
    for i in 0..n {
        b.app.select_panel(i);
        changements.push(b.image(Vec::new(), 1.0 / 60.0));
        suivantes.push(b.image(Vec::new(), 1.0 / 60.0));
        suivantes.push(b.image(Vec::new(), 1.0 / 60.0));
    }
    rapport("parcours : image du changement de plan", &changements);
    rapport("parcours : les deux images suivantes", &suivantes);
    let (i, ms, pire) = jusqu_au_calme(&mut b, 300.0);
    println!("PERF après le parcours : {i} images, {ms:.0} ms jusqu'au calme, pire image {pire:.1} ms");
    mem("après avoir parcouru tous les plans", &b);

    // Lecture du montage depuis le début, 15 s.
    b.app.set_espace(Espace::Montage);
    b.chauffer(5);
    b.app.goto_global(0);
    b.app.toggle_play_scope(true);
    let m = scene("lecture du montage (24 images/s, 15 s)", &mut b, 360, 1.0 / 24.0, |_, _| Vec::new());
    println!("     lecture : {} images sur {} dépassent 41,7 ms ; plan {} à la fin", m.iter().filter(|x| x.total() > 41.7).count(), m.len(), b.app.cur_panel);
    b.app.playing = false;
    mem("après la lecture", &b);

    // Un long historique : 150 traits, un plan différent à chaque fois.
    b.app.set_espace(Espace::Storyboard);
    b.chauffer(5);
    b.app.tool = Tool::Brush;
    let mut traits = Vec::new();
    for k in 0..150 {
        b.app.select_panel((k * 7) % n);
        b.chauffer(2);
        let li = b.app.doc.panels[b.app.cur_panel].layers.iter().position(|l| l.kind == crate::document::LayerKind::Bitmap && !l.folder && l.couleur.is_none());
        if let Some(li) = li {
            b.app.cur_layer = li;
        }
        traits.extend(trait_sur_le_dessin(&mut b, 8, k));
    }
    rapport("historique : images des traits", &traits);
    println!("     {} étapes d'annulation", b.app.history.undo_len());
    jusqu_au_calme(&mut b, 120.0);
    mem("après 150 traits", &b);
    let mut annuler = Vec::new();
    for _ in 0..150 {
        let t = Instant::now();
        b.app.undo();
        annuler.push(t.elapsed().as_secs_f64() * 1e3);
        b.image(Vec::new(), 1.0 / 60.0);
    }
    annuler.sort_by(f64::total_cmp);
    println!("PERF annuler 150 fois : moyenne {:.2} ms, pire {:.2} ms", annuler.iter().sum::<f64>() / 150.0, annuler.last().unwrap_or(&0.0));
    jusqu_au_calme(&mut b, 120.0);
    mem("après 150 annulations", &b);
    std::process::exit(0);
}
