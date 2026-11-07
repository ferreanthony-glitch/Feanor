//! L'export de la planche, sans carte graphique : ses tuiles se calculent exactes (`plancheui::rendre_les_tuiles`), ce qui peut durer, et Annuler ou fermer FEANOR n'attend pas ce calcul
//! (`export::Arret`). Arrêté, l'export ne laisse pas de `.part` et ne remplace pas le fichier qui existait.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::Color32;

use crate::document::{Document, LayerKind};
use crate::export::{self, ExportJob, Progress};
use crate::fx::{Effect, EffectKind};
use crate::fxtrait_tests::smooth_stroke;
use crate::perf_tests::{attente, avec_notes_et_cameras, banc, Banc};
use crate::planche::{self, Options, Sortie};
use crate::raster::{Raster, TILE};
use crate::traits_continus_tests::des_plans;
use crate::vector::{Element, Trace, VectorDrawing};

const ROUGE: Color32 = Color32::from_rgb(200, 30, 40);

/// Un trait peint à plat, une bande de `y0` à `y0 + 12`, sur toute la largeur de la feuille.
fn bande(r: &mut Raster, largeur: usize, y0: usize) {
    let t = TILE as i64;
    for y in y0 as i64..y0 as i64 + 12 {
        for x in 0..largeur as i64 {
            let id = r.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
            r.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = ROUGE;
        }
    }
}

/// `n` plans bitmap à mouvement de caméra (une tuile chacun), `calques` calques par plan, et un flou gaussien de `flou` pixels sur chacun (0 : aucun). `rayures` : des bandes sur toute la
/// hauteur de la feuille (de quoi faire travailler le flou) ; sinon une seule, au milieu du cadre.
fn film_a_tuiles(n: usize, calques: usize, flou: f32, rayures: bool) -> Document {
    let mut doc = des_plans(n, true);
    let (w, h) = (doc.width, doc.height);
    for p in 0..n {
        doc.set_duration(p, 24);
        let modele = doc.panels[p].layers[0].clone();
        doc.panels[p].layers.clear();
        for c in 0..calques {
            let id = doc.new_cel(LayerKind::Bitmap);
            let cel = doc.cels.get_mut(&id).unwrap();
            if rayures {
                for y in (20 + 9 * c..h - 12).step_by(48) {
                    bande(&mut cel.raster, w, y);
                }
            } else {
                bande(&mut cel.raster, w, h / 2 + 30 * c);
            }
            cel.version += 1;
            let mut l = modele.clone();
            l.id = crate::document::fresh_layer_id();
            l.frames = vec![Some(id); 24];
            if flou > 0.0 {
                let mut e = Effect::new(EffectKind::GaussianBlur);
                e.values = vec![flou];
                l.fx.push(e);
            }
            doc.panels[p].layers.push(l);
        }
    }
    avec_notes_et_cameras(&mut doc, true);
    doc
}

fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("shot-planche-arret-{nom}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dossier");
    d
}

/// La tâche d'export de tous les plans, poussée jusqu'à la fin des images : le fil qui finit est dans les tuiles.
fn jusqu_aux_tuiles(b: &Banc, opt: &Options, sortie: &Path) -> ExportJob {
    let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
    let mut job = b.app.planche_job(&plans, opt, sortie).expect("tâche");
    for _ in attente() {
        job.step(&b.app.doc, Duration::from_millis(20));
        if matches!(job.progress(), Progress::Finishing) {
            return job;
        }
        assert!(job.outcome.is_none(), "l'export s'est arrêté avant les tuiles : {:?}", job.outcome);
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("les images de l'export ne finissent pas");
}

/// Annule pendant les tuiles (au moins `apres` faites) ; rend le temps d'annulation, et attend que le fil qui finit soit parti.
fn annuler_dans_les_tuiles(mut job: ExportJob, apres: usize, fermer: bool) -> (Duration, usize, usize, Duration) {
    let arret = job.arret_pour_essai().expect("un export arrêtable");
    for _ in attente() {
        if arret.tuiles_faites() >= apres {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let au_moment = arret.tuiles_faites();
    let t = Instant::now();
    if fermer {
        drop(job);
    } else {
        job.cancel();
        assert!(matches!(&job.outcome, Some(Err(_))), "{:?}", job.outcome);
    }
    let reaction = t.elapsed();
    for _ in attente() {
        if arret.fil_fini() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(arret.fil_fini(), "le fil qui finit s'en va");
    (reaction, au_moment, arret.tuiles_faites(), t.elapsed())
}

fn sans_part(sortie: &Path) {
    assert!(!export::part_of(sortie).exists(), "pas de fichier incomplet");
}

/// Annuler pendant les tuiles exactes rend la main tout de suite ; le fil s'arrête à la tuile suivante, le fichier d'avant reste intact, pas de `.part`.
#[test]
fn cancelling_during_the_exact_tiles_answers_at_once_and_keeps_the_old_file() {
    let b = banc(film_a_tuiles(8, 3, 40.0, true), false);
    let opt = Options::default();
    let dir = dossier("annuler");
    for sortie in [Sortie::Png, Sortie::PdfUnePage] {
        let chemin = dir.join(format!("planche-{sortie:?}.bin"));
        let avant = b"le fichier d'avant".to_vec();
        std::fs::write(&chemin, &avant).unwrap();
        let job = jusqu_aux_tuiles(&b, &Options { sortie, ..opt.clone() }, &chemin);
        let (reaction, au_moment, a_la_fin, fin_du_fil) = annuler_dans_les_tuiles(job, 1, false);
        println!("ANNULER {sortie:?} : la main en {reaction:?}, {au_moment} tuiles à ce moment, {a_la_fin} à la fin du fil, parti après {fin_du_fil:?}");
        assert!(reaction < Duration::from_millis(1500), "annuler rend la main vite : {reaction:?}");
        assert!(a_la_fin <= au_moment + 1, "le fil s'arrête à la tuile suivante : {au_moment} puis {a_la_fin}");
        assert_eq!(std::fs::read(&chemin).unwrap(), avant, "le fichier d'avant n'est pas remplacé");
        sans_part(&chemin);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// La fermeture de FEANOR (la tâche lâchée) pendant les tuiles ne gèle pas non plus, même quand la tuile en cours, dix calques flous, dure plus que l'attente : son fil, seul, n'écrit plus rien.
#[test]
fn closing_during_the_exact_tiles_does_not_freeze() {
    let b = banc(film_a_tuiles(2, 10, 40.0, true), false);
    let dir = dossier("fermer");
    let chemin = dir.join("planche.png");
    let job = jusqu_aux_tuiles(&b, &Options::default(), &chemin);
    let (reaction, _, _, fin_du_fil) = annuler_dans_les_tuiles(job, 1, true);
    println!("FERMER : la main en {reaction:?}, le fil parti après {fin_du_fil:?}");
    assert!(reaction < Duration::from_millis(1500), "fermer ne gèle pas : {reaction:?}");
    assert!(!chemin.exists(), "pas de fichier après l'arrêt");
    sans_part(&chemin);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Annuler à n'importe quel moment de la fin (tuiles, composition, encodage, écriture) : le fichier d'avant, ou le fichier complet s'il a fini juste avant. Jamais rien d'autre, jamais de `.part`.
#[test]
fn cancelling_at_any_moment_of_the_end_leaves_the_old_file_or_the_complete_one() {
    let b = banc(film_a_tuiles(2, 1, 0.0, false), false);
    let dir = dossier("instants");
    let avant = b"le fichier d'avant".to_vec();
    let mut vus = (0, 0);
    for k in 0..24u64 {
        let chemin = dir.join(format!("planche-{k}.png"));
        std::fs::write(&chemin, &avant).unwrap();
        let mut job = jusqu_aux_tuiles(&b, &Options::default(), &chemin);
        let arret = job.arret_pour_essai().unwrap();
        std::thread::sleep(Duration::from_millis(k * 12));
        job.cancel();
        for _ in attente() {
            if arret.fil_fini() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        drop(job);
        sans_part(&chemin);
        let la = std::fs::read(&chemin).unwrap();
        if la == avant {
            vus.0 += 1;
        } else {
            vus.1 += 1;
            image::load_from_memory(&la).expect("un fichier complet");
        }
    }
    println!("INSTANTS : le fichier d'avant {} fois, le fichier complet {} fois", vus.0, vus.1);
    assert!(vus.0 > 0, "des arrêts sont tombés avant la fin");
    let _ = std::fs::remove_dir_all(&dir);
}

/// L'écriture arrêtée, avant ou après le début : rien sur la cible, pas de `.part`. Sans arrêt : le fichier, entier, plus grand qu'un morceau.
#[test]
fn the_stoppable_write_leaves_nothing_once_stopped() {
    let dir = dossier("ecriture");
    let chemin = dir.join("sortie.bin");
    let gros = vec![7u8; 9 << 20];
    export::write_atomic_sauf_arret(&chemin, &gros, &export::Arret::jamais()).expect("écrit");
    assert_eq!(std::fs::read(&chemin).unwrap(), gros);
    sans_part(&chemin);
    let mut job = ExportJob::collect_arretable("essai".into(), "Image", &chemin, Vec::new(), |_, _| Ok(()), |_: Vec<()>, _: &export::Arret| Ok(()));
    let arret = job.arret_pour_essai().unwrap();
    job.cancel();
    assert!(export::write_atomic_sauf_arret(&chemin, b"autre chose", &arret).is_err());
    assert_eq!(std::fs::read(&chemin).unwrap(), gros, "la cible est intacte");
    sans_part(&chemin);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Sans carte graphique : la tuile d'un plan flou montre le flou, quoi que la réserve commune tienne. (Le défaut de `docs/FIABILITE.md`, section 20 : le mode doux rendait la tuile du flou
/// identique à celle du trait net.)
#[test]
fn the_exact_tile_shows_the_blur_without_a_graphics_card() {
    let tuile = |b: &Banc, plan: usize| {
        let opt = b.app.planche_options(&Options::default());
        let plans: Vec<usize> = (0..b.app.doc.panels.len()).collect();
        let p = planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
        let (e, ..) = planche::dimensions(p.cadre, opt.resolution);
        let k = p.cartes.iter().position(|c| c.mouvement.is_some() && c.plan_reel == plan).expect("la case à cadres");
        b.app.planche_tuiles_pour_essai(&p, e, &opt).swap_remove(k).expect("sa tuile")
    };
    let encre = |t: &planche::TuileImage| t.rvba.chunks_exact(4).filter(|c| c[3] != 0 && c[0] as i32 - c[1] as i32 > 2).count();
    let net = banc(film_a_tuiles(2, 1, 0.0, false), false);
    let mut flou = banc(film_a_tuiles(2, 1, 40.0, false), false);
    assert!(net.app.gpu.is_none() && flou.app.gpu.is_none(), "sans carte graphique");
    crate::fx::forget_all();
    let (a, b) = (tuile(&net, 1), tuile(&flou, 1));
    println!("TUILE sans carte : encre nette {}, floue {}", encre(&a), encre(&b));
    assert!(encre(&a) > 100, "le trait est là");
    assert!(encre(&b) * 2 > encre(&a) * 3, "la tuile montre le flou : net {}, flou {}", encre(&a), encre(&b));
    for _ in 0..4 {
        flou.image(Vec::new(), 1.0 / 60.0);
    }
    assert!(tuile(&flou, 1).rvba == b.rvba, "le même fichier, quoi que la réserve commune tienne");
}

/// Le temps de réaction d'Annuler sur un gros cas (60 plans, 3 calques flous), à la tuile 5. `cargo test --release mesure_annuler_les_tuiles -- --ignored --nocapture`.
#[test]
#[ignore]
fn mesure_annuler_les_tuiles() {
    let b = banc(film_a_tuiles(60, 3, 40.0, true), false);
    let dir = dossier("mesure");
    let chemin = dir.join("planche.png");
    let t = Instant::now();
    let job = jusqu_aux_tuiles(&b, &Options::default(), &chemin);
    println!("MESURE images de l'export : {:?}", t.elapsed());
    let (reaction, au_moment, a_la_fin, fin_du_fil) = annuler_dans_les_tuiles(job, 5, false);
    println!("MESURE annuler : la main en {reaction:?} ; {au_moment} tuiles à ce moment, {a_la_fin} à la fin du fil ; le fil parti après {fin_du_fil:?}");
    assert!(!export::part_of(&chemin).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Un dessin : `traits` traits de `points` points, avec appuis et instants comme un vrai trait de stylet.
fn dessin(traits: usize, points: usize, graine: usize) -> VectorDrawing {
    let elements = (0..traits)
        .map(|k| {
            let y = 20.0 + ((graine * 13 + k * 17) % 1000) as f32;
            let mut e = smooth_stroke(&[(20.0, y, 2.5), (20.0 + 5.0 * points as f32, y + 20.0, 3.5)]);
            if let Element::Stroke { points, trace, .. } = &mut e {
                *trace = Trace { pressure: vec![0.5; points.len()], time: (0..points.len()).map(|i| i as f64 / 120.0).collect(), ..Trace::default() };
            }
            e
        })
        .collect();
    VectorDrawing { elements }
}

/// Ce que coûte, au clic et sur le fil de l'interface, le cliché des tuiles d'un film de 300 plans vectoriels, sans et avec effet du trait. `cargo test --release mesure_le_cliche_des_tuiles -- --ignored --nocapture`.
#[test]
#[ignore]
fn mesure_le_cliche_des_tuiles() {
    for (calques, traits, points) in [(2, 30, 150), (3, 60, 200)] {
        let mut doc = des_plans(300, false);
        for p in 0..300 {
            doc.set_duration(p, 24);
            let modele = doc.panels[p].layers[0].clone();
            doc.panels[p].layers.clear();
            for c in 0..calques {
                let id = doc.new_cel(LayerKind::Vector);
                let cel = doc.cels.get_mut(&id).unwrap();
                cel.vector = Some(dessin(traits, points, p * 5 + c));
                cel.rerender();
                let mut l = modele.clone();
                l.id = crate::document::fresh_layer_id();
                l.kind = LayerKind::Vector;
                l.frames = vec![Some(id); 24];
                doc.panels[p].layers.push(l);
            }
        }
        avec_notes_et_cameras(&mut doc, true);
        let mut b = banc(doc, false);
        let opt = b.app.planche_options(&Options::default());
        let plans: Vec<usize> = (0..300).collect();
        let p = planche::construire(&b.app.doc, &plans, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
        let (s, ..) = planche::dimensions(p.cadre, opt.resolution);
        for effet in [false, true] {
            for pl in &mut b.app.doc.panels {
                for l in &mut pl.layers {
                    l.fx = if effet { vec![Effect::new(EffectKind::Sketch)] } else { Vec::new() };
                }
            }
            let (_, m0) = crate::lourd_tests::memoire();
            let t = Instant::now();
            let travaux = b.app.planche_tuiles_a_rendre(&p, s, &opt);
            let duree = t.elapsed();
            let (_, m1) = crate::lourd_tests::memoire();
            println!("CLICHE {calques} calques, {traits} traits de {points} points, 300 plans, effet du trait {effet} : {duree:?} au clic, {:.0} Mo de plus, {} tuiles", m1 - m0, travaux.iter().flatten().count());
            let t = Instant::now();
            drop(travaux);
            println!("CLICHE lâché en {:?}", t.elapsed());
        }
    }
}
