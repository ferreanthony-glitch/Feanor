//! Essais de la partie 193 : **les bords doux** entre le décor 3D et les cartes (`profondeur.rs`, `docs/INTERFACE.md` §193). Le
//! bord d'un objet du décor qui passe devant une carte, la ligne où une carte inclinée entre dans un objet, et le bord du décor
//! contre le ciel ou le vide sont lissés : au bord, la carte se voit en partie (une rampe d'un ou deux pixels, de la carte au
//! décor) ; l'intérieur est tout caché ou tout visible, exactement comme avant. Par le processeur et par la carte graphique, qui
//! s'accordent ; dans la vue Espace 3D aussi. « Toujours devant le décor 3D » et un calque accroché à la caméra ne sont jamais
//! cachés, même au bord ; sans décor, rien ne change.
//!
//! Et allégée (partie 193, « Allège ») : la profondeur à quatre échantillons n'est dessinée que si une carte du plan la partage et
//! que le décor tient dans le budget avec elle ; sinon, la profondeur en une valeur d'avant, au coût d'avant. Passer de l'une à
//! l'autre ne redessine pas la couleur du décor. L'étalonnage donne le budget d'avant, et celui des bords doux à part.

use eframe::egui::Color32;
use eframe::egui_wgpu::RenderState;

use crate::document::{Document, LayerTransform};
use crate::profondeur::{self as pr, ProfDecor};
use crate::profondeur_tests::{calque_bleu, composee, objet_boite, poser_decor, projet, voile_seul, BLEU};
use crate::render::FondPart;

const W: usize = 320;
const H: usize = 180;

/// Un mur de 3 m de large, 3,5 m de haut et 40 cm d'épaisseur, tourné de 30° autour de la verticale, son milieu à 1 m derrière la
/// feuille : vu de face, son bord du haut est penché (il descend vers le côté le plus loin), ses côtés sont droits.
fn boite_tournee(id: u64) -> crate::decor3d::Objet3d {
    let mut o = objet_boite(id, [-1.5, 0.0, -0.2], [1.5, 3.5, 0.2], [0.0, -1.5, -1.0]);
    o.place.rotation_y = 30.0;
    o
}

/// L'image de la feuille composée par le processeur (le chemin des exports et des vignettes), le décor rendu sur ce fil, par la
/// carte graphique `rs` si elle est prêtée (l'export d'une application qui en a une), sinon par le processeur.
fn feuille_cpu(doc: &Document, rs: Option<&RenderState>) -> Vec<Color32> {
    crate::decor3d::essai_poser(None, rs);
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    if let Some(FondPart::Decor(key)) = &plan.items[0].fond {
        let _ = crate::decor3d::montre(doc, key);
    }
    let img = crate::render::composite_full(doc, &plan);
    crate::decor3d::essai_poser(None, None);
    img
}

/// `f` sur le même projet sans ses calques (le décor seul) : ce qui se voit là où la carte est toute cachée.
fn sans_cartes<R>(doc: &mut Document, f: impl FnOnce(&Document) -> R) -> R {
    let calques = std::mem::take(&mut doc.panels[0].layers);
    let r = f(doc);
    doc.panels[0].layers = calques;
    r
}

/// La part de la carte bleue (opaque) qui se voit au pixel composé `m`, `d` étant le même pixel sans la carte : `m = v · bleu +
/// (1 − v) · d` (valeurs prémultipliées). Rien si la carte et le décor y sont indiscernables.
fn part(m: Color32, d: Color32) -> Option<f32> {
    let f = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32];
    let (m, d, k) = (f(m), f(d), f(BLEU));
    let kd: Vec<f32> = (0..4).map(|i| k[i] - d[i]).collect();
    let n: f32 = kd.iter().map(|v| v * v).sum();
    (n > 400.0).then(|| (0..4).map(|i| (m[i] - d[i]) * kd[i]).sum::<f32>() / n)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sorte {
    /// La carte, toute visible.
    Carte,
    /// Le décor seul : la carte toute cachée (ou absente).
    Decor,
    /// Entre les deux : le bord.
    Bord,
}

fn sorte(m: Color32, d: Color32) -> Sorte {
    if m == d {
        Sorte::Decor
    } else if m == BLEU {
        Sorte::Carte
    } else {
        Sorte::Bord
    }
}

/// Le long d'une suite de pixels (composé, décor seul) qui va de la carte visible au décor qui la cache : les parts de la carte au
/// bord, entre le dernier pixel de carte et le premier de décor. Rien si la suite ne passe pas de l'une à l'autre.
fn rampe(suite: &[(Color32, Color32)]) -> Option<Vec<f32>> {
    let premiere_carte = suite.iter().position(|&(m, d)| sorte(m, d) == Sorte::Carte)?;
    let c = premiere_carte + suite[premiere_carte..].iter().position(|&(m, d)| sorte(m, d) == Sorte::Decor)?;
    let derniere_carte = (premiere_carte..c).rev().find(|&i| sorte(suite[i].0, suite[i].1) == Sorte::Carte)?;
    Some(suite[derniere_carte + 1..c].iter().map(|&(m, d)| part(m, d).unwrap_or(f32::NAN)).collect())
}

/// Vérifie une rampe : au plus deux pixels, des parts strictement entre 0 et 1 qui décroissent de la carte vers le décor.
fn verifier_rampe(r: &[f32], ou: &str) {
    assert!(r.len() <= 2, "{ou} : un bord de plus de deux pixels : {r:?}");
    for (i, v) in r.iter().enumerate() {
        assert!(*v > 0.03 && *v < 0.97, "{ou} : une part de bord hors de ]0, 1[ : {r:?}");
        if i > 0 {
            assert!(*v <= r[i - 1] + 0.03, "{ou} : la rampe ne décroît pas : {r:?}");
        }
    }
}

/// Chaque pixel est la carte, le décor, ou un bord qui touche (à deux pixels près) la carte et le décor là où la carte est cachée :
/// l'intérieur est exactement celui d'avant (tout caché : le décor ; tout visible : la carte). Le nombre de pixels de bord.
fn bords_seulement_au_bord(img: &[Color32], decor: &[Color32], zone: [usize; 4], ou: &str) -> usize {
    let [x0, x1, y0, y1] = zone;
    let mut bords = 0;
    for y in 0..H {
        for x in 0..W {
            let (m, d) = (img[y * W + x], decor[y * W + x]);
            let s = sorte(m, d);
            let dans = x >= x0 && x < x1 && y >= y0 && y < y1;
            if !dans {
                assert_eq!(m, d, "{ou} : hors de la carte ({x}, {y}), le décor seul");
                continue;
            }
            if s != Sorte::Bord {
                continue;
            }
            bords += 1;
            let autour = |k: Sorte| {
                (y.saturating_sub(2)..(y + 3).min(H)).any(|yy| (x.saturating_sub(2)..(x + 3).min(W)).any(|xx| sorte(img[yy * W + xx], decor[yy * W + xx]) == k))
            };
            assert!(autour(Sorte::Carte) && autour(Sorte::Decor), "{ou} : un pixel de bord ({x}, {y}) loin du bord : {m:?} sur {d:?}");
        }
    }
    bords
}

/// La carte de 200 × 140 pixels, à 3 m derrière la feuille (derrière le mur), qui le couvre tout.
const GRANDE: [usize; 4] = [60, 260, 20, 160];

/// Les colonnes qui passent du bleu de la carte (au-dessus de la boîte) au décor (la boîte devant elle) : leurs rampes.
fn rampes_du_haut(img: &[Color32], decor: &[Color32]) -> Vec<Vec<f32>> {
    (GRANDE[0]..GRANDE[1]).filter_map(|x| rampe(&(GRANDE[2]..GRANDE[3]).map(|y| (img[y * W + x], decor[y * W + x])).collect::<Vec<_>>())).collect()
}

fn le_bord_d_un_objet_devant_une_carte_est_doux(img: &[Color32], decor: &[Color32], ou: &str) {
    let bords = bords_seulement_au_bord(img, decor, GRANDE, ou);
    let rampes = rampes_du_haut(img, decor);
    assert!(rampes.len() > 30, "{ou} : la boîte coupe la carte sur {} colonnes", rampes.len());
    for r in &rampes {
        verifier_rampe(r, ou);
    }
    // Le bord penché du haut : au moins une colonne sur deux a un pixel de bord (une arête au pixel n'en aurait aucun ; avec deux
    // rangées d'échantillons par pixel, celles du processeur, une arête qui passe entre elles n'en a pas non plus).
    let douces = rampes.iter().filter(|r| !r.is_empty()).count();
    assert!(douces * 2 >= rampes.len(), "{ou} : {douces} colonnes douces sur {}", rampes.len());
    eprintln!("{ou} : {douces} colonnes douces sur {}, {bords} pixels de bord", rampes.len());
    // Les côtés droits de la boîte, aussi, là où ils tombent entre deux échantillons.
    assert!(bords > douces, "{ou} : {bords} pixels de bord, {douces} en haut");
    // Et l'intérieur reste tout caché ou tout visible.
    let caches = (0..W * H).filter(|&i| img[i] == decor[i] && decor[i].a() == 255 && {
        let (x, y) = (i % W, i / W);
        x >= GRANDE[0] && x < GRANDE[1] && y >= GRANDE[2] && y < GRANDE[3]
    }).count();
    assert!(caches > 2000, "{ou} : {caches} pixels tout cachés");
}

// ---------------------------------------------------------------------------------------------- le calcul

#[test]
fn la_profondeur_garde_quatre_echantillons_et_compte_ceux_qui_cachent() {
    // Les flottants de 16 bits : aller et retour à 0,05 % près, 0 reste 0.
    for v in [0.0f32, 1e-4, 0.0123, 0.1, 0.25, 0.3333, 1.0, 7.5, 123.4] {
        let r = pr::de_16_bits(pr::vers_16_bits(v));
        assert!((r - v).abs() <= v * 5e-4 + 1e-7, "{v} → {r}");
    }
    assert_eq!(pr::vers_16_bits(0.0), 0);
    // Une carte à plat à 4 m (1/4) ; le décor à 2 m (1/2) sur une partie des échantillons, rien ailleurs.
    let plan = [0.0, 0.0, 0.25];
    let m = pr::MOTIF_CARTE;
    assert_eq!(pr::part_cachee(&plan, [0.5; 4], &m, 0.0, 0.0), 1.0, "tout devant : tout caché");
    assert_eq!(pr::part_cachee(&plan, [0.5, 0.0, 0.5, 0.0], &m, 0.0, 0.0), 0.5, "la moitié du pixel");
    assert_eq!(pr::part_cachee(&plan, [0.5, 0.0, 0.0, 0.0], &m, 0.0, 0.0), 0.25);
    assert_eq!(pr::part_cachee(&plan, [0.1; 4], &m, 0.0, 0.0), 0.0, "le décor derrière : rien");
    // Une carte inclinée : sa distance est prise à la place de chaque échantillon. Ici elle passe derrière le décor (1/2) entre
    // x = 0,3 et x = 0,7 du pixel : les deux échantillons de gauche la voient devant, les deux de droite derrière.
    let incl = [-1.0, 0.0, 1.0];
    assert_eq!(pr::part_cachee(&incl, [0.5; 4], &m, 0.0, 0.0), 0.5);
    // Un pixel sans décor se voit tout entier, exactement ; l'opacité ne change pas (au bit près).
    let p = ProfDecor::new(2, 1, vec![[0; 4], [pr::vers_16_bits(0.5), 0, 0, 0]], m);
    let masque = pr::Masque { prof: &p, w: 2, h: 1, plan };
    assert_eq!(masque.visible(0, 0), 1.0);
    assert_eq!(masque.visible(1, 0), 0.75);
    assert_eq!(masque.visible(-1, 0), 1.0, "hors de la feuille");
    for op in 0..=255u8 {
        assert_eq!(pr::opacite_visible(op, 1.0), op);
        assert_eq!(pr::opacite_visible(op, 0.0), 0);
    }
    // La profondeur en une valeur (celle des ombres des cartes) : le plus proche, s'il couvre au moins la moitié du pixel.
    assert_eq!(pr::un_de([0.5, 0.25, 0.0, 0.0]), 0.5);
    assert_eq!(pr::un_de([0.5, 0.0, 0.0, 0.0]), 0.0);
}

// ---------------------------------------------------------------------------------------------- par le processeur

#[test]
fn par_le_processeur_le_bord_d_un_objet_devant_une_carte_est_doux() {
    for (id, ciel) in [(19201u64, false), (19202, true)] {
        let mut doc = projet();
        poser_decor(&mut doc, vec![boite_tournee(id)]);
        if ciel {
            let seq = doc.sequence_of(0);
            doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.ciel = true;
        }
        calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
        let img = feuille_cpu(&doc, None);
        let decor = sans_cartes(&mut doc, |d| feuille_cpu(d, None));
        assert_eq!(decor.iter().all(|c| c.a() == 255), ciel, "le ciel couvre toute la feuille");
        le_bord_d_un_objet_devant_une_carte_est_doux(&img, &decor, if ciel { "processeur, avec le ciel" } else { "processeur, sans ciel" });
    }
}

/// La carte inclinée qui traverse la boîte (tournée de 60° autour de la verticale, et penchée de 25° : la ligne où elle entre dans
/// la face avant de la boîte est penchée sur l'image).
fn carte_qui_traverse(id: u64) -> Document {
    let mut doc = projet();
    poser_decor(&mut doc, vec![objet_boite(id, [-0.5, 0.0, -0.2], [0.5, 2.0, 0.2], [0.0, -1.0, -1.0])]);
    let li = calque_bleu(&mut doc, "Perso", [100, 220, 50, 130], -1.0);
    doc.panels[0].layers[li].transform = LayerTransform { tilt_y: 60.0, tilt_x: 25.0, ..Default::default() };
    doc
}

/// Les lignes de la feuille qui passent de la carte (devant la boîte, à gauche) au décor (la boîte devant elle, à droite), dans la
/// boîte : leurs rampes.
fn rampes_de_l_entree(img: &[Color32], decor: &[Color32]) -> Vec<Vec<f32>> {
    // (Dans la boîte : là où le décor seul est opaque, loin de ses côtés.)
    (40..140)
        .filter_map(|y| {
            let xs: Vec<usize> = (0..W).filter(|&x| decor[y * W + x].a() == 255).collect();
            let (a, b) = (*xs.first()? + 4, *xs.last()? - 4);
            (a < b).then(|| rampe(&(a..b).map(|x| (img[y * W + x], decor[y * W + x])).collect::<Vec<_>>())).flatten()
        })
        .collect()
}

fn l_entree_d_une_carte_inclinee_dans_la_boite_est_douce(img: &[Color32], decor: &[Color32], ou: &str) {
    let rampes = rampes_de_l_entree(img, decor);
    assert!(rampes.len() >= 15, "{ou} : la carte entre dans la boîte sur {} lignes", rampes.len());
    for r in &rampes {
        verifier_rampe(r, ou);
    }
    let douces = rampes.iter().filter(|r| !r.is_empty()).count();
    assert!(douces * 3 >= rampes.len(), "{ou} : {douces} lignes douces sur {}", rampes.len());
    eprintln!("{ou} : {douces} lignes douces sur {}", rampes.len());
}

#[test]
fn par_le_processeur_une_carte_inclinee_entre_dans_la_boite_par_une_ligne_douce() {
    let mut doc = carte_qui_traverse(19203);
    let img = feuille_cpu(&doc, None);
    let decor = sans_cartes(&mut doc, |d| feuille_cpu(d, None));
    l_entree_d_une_carte_inclinee_dans_la_boite_est_douce(&img, &decor, "processeur");
}

#[test]
fn toujours_devant_le_decor_et_accroche_a_la_camera_ne_sont_jamais_caches_meme_au_bord() {
    for (k, sorte) in ["devant", "accroché"].into_iter().enumerate() {
        let mut doc = projet();
        poser_decor(&mut doc, vec![boite_tournee(19204 + k as u64)]);
        let li = calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
        match sorte {
            "devant" => doc.panels[0].layers[li].devant_decor = true,
            _ => doc.panels[0].layers[li].accroche = true,
        }
        let verifier = |img: &[Color32], ou: &str| {
            for y in GRANDE[2]..GRANDE[3] {
                for x in GRANDE[0]..GRANDE[1] {
                    assert_eq!(img[y * W + x], BLEU, "{sorte}, {ou} : ({x}, {y})");
                }
            }
        };
        verifier(&feuille_cpu(&doc, None), "processeur");
        if let Some(rs) = crate::gpu_tests::headless() {
            crate::decor3d::essai_bords(Some(true));
            verifier(&composee(&rs, &doc), "carte graphique");
        }
    }
}

#[test]
fn sans_decor_les_images_sont_celles_d_avant_au_bit_pres() {
    // Sans décor : aucun plan, la composition n'appelle jamais le masque ; avec des plans forcés (rien dessous), la même image.
    let mut doc = projet();
    calque_bleu(&mut doc, "A", GRANDE, -2.0);
    let b = calque_bleu(&mut doc, "B", [100, 150, 40, 120], -0.5);
    doc.panels[0].layers[b].transform = LayerTransform { tilt_y: 35.0, tilt_x: -10.0, ..Default::default() };
    doc.panels[0].layers[b].opacity = 0.6;
    let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
    assert!(plan.items.iter().all(|i| i.profondeur.is_none()));
    let mut faux = plan.clone();
    for i in &mut faux.items {
        i.profondeur = Some([0.0, 0.0, 10.0]);
    }
    let a = crate::render::composite_full(&doc, &plan);
    assert!(a == crate::render::composite_full(&doc, &faux));
    // Le pixel du monde (vignettes, pipette) aussi.
    for (x, y) in [(110i64, 60i64), (160, 90), (61, 21), (300, 170)] {
        assert_eq!(crate::render::composite_pixel_i(&doc, &plan, x, y), a[y as usize * W + x as usize]);
    }
}

// ---------------------------------------------------------------------------------------------- par la carte graphique

#[test]
fn par_la_carte_graphique_les_bords_sont_doux_et_l_export_s_accorde() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    crate::decor3d::essai_bords(Some(true));
    // Le bord de la boîte devant la carte.
    let mut doc = projet();
    poser_decor(&mut doc, vec![boite_tournee(19210)]);
    calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
    let img = composee(&rs, &doc);
    let decor = sans_cartes(&mut doc, |d| composee(&rs, d));
    le_bord_d_un_objet_devant_une_carte_est_doux(&img, &decor, "carte graphique");
    // L'export par le processeur, le décor rendu par la carte (l'export d'une application qui en a une) : le même à quelques
    // niveaux près, bords compris.
    let cpu = feuille_cpu(&doc, Some(&rs));
    let ecart = |a: &[Color32], b: &[Color32]| a.iter().zip(b).map(|(p, q)| (0..4).map(|k| (p[k] as i32 - q[k] as i32).abs()).max().unwrap()).max().unwrap();
    let e = ecart(&img, &cpu);
    assert!(e <= 6, "la carte graphique et l'export par le processeur : {e} niveaux d'écart");
    eprintln!("carte graphique et export par le processeur : {e} niveaux d'écart au plus");
    let bords_gpu: Vec<usize> = (0..W * H).filter(|&i| sorte(img[i], decor[i]) == Sorte::Bord).collect();
    let decor_cpu = sans_cartes(&mut doc, |d| feuille_cpu(d, Some(&rs)));
    assert!(bords_gpu.iter().filter(|&&i| sorte(cpu[i], decor_cpu[i]) == Sorte::Bord).count() * 10 >= bords_gpu.len() * 9, "les mêmes pixels de bord");
    // La carte inclinée qui entre dans la boîte.
    let mut doc = carte_qui_traverse(19211);
    let img = composee(&rs, &doc);
    let decor = sans_cartes(&mut doc, |d| composee(&rs, d));
    l_entree_d_une_carte_inclinee_dans_la_boite_est_douce(&img, &decor, "carte graphique");
    let cpu = feuille_cpu(&doc, Some(&rs));
    let e = ecart(&img, &cpu);
    assert!(e <= 6, "carte inclinée : la carte graphique et l'export par le processeur, {e} niveaux d'écart");
}

#[test]
fn par_la_carte_graphique_en_mode_de_fusion_le_bord_est_doux_aussi() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    crate::decor3d::essai_bords(Some(true));
    let mut doc = projet();
    poser_decor(&mut doc, vec![boite_tournee(19212)]);
    let li = calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
    doc.panels[0].layers[li].blend = crate::blend::BlendMode::Darken;
    let img = composee(&rs, &doc);
    let decor = sans_cartes(&mut doc, |d| composee(&rs, d));
    // (En mode « Assombrir », la carte se mêle au fond : on compte les pixels entre le décor seul et la carte toute visible.)
    doc.panels[0].layers[li].devant_decor = true;
    let tout = composee(&rs, &doc);
    let mut bords = 0;
    for i in 0..W * H {
        if img[i] != decor[i] && img[i] != tout[i] {
            bords += 1;
        }
    }
    assert!(bords > 30, "{bords} pixels de bord en mode de fusion");
    // L'intérieur : tout caché, le décor seul, exactement.
    assert!((0..W * H).filter(|&i| img[i] == decor[i] && tout[i] != decor[i]).count() > 2000);
}

// ---------------------------------------------------------------------------------------------- la vue Espace 3D

#[test]
fn dans_la_vue_espace_3d_les_bords_du_voile_sont_doux() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    // Le bord de la boîte devant une carte, derrière elle : le voile de la carte y remet la boîte en partie.
    let mut doc = projet();
    poser_decor(&mut doc, vec![boite_tournee(19220)]);
    let derriere = crate::espace3d::Xf { pos: [0.0, 0.0, -3.0], rot: [0.0; 3], size: [8.0, 6.0, 1.0] };
    let img = voile_seul(&rs, &doc, &derriere);
    let (pleins, partiels) = (img.iter().filter(|c| c.a() == 255).count(), img.iter().filter(|c| c.a() > 0 && c.a() < 255).count());
    assert!(pleins > 2000 && partiels > 40, "{pleins} pixels où la boîte recouvre la carte, {partiels} au bord");
    // La carte inclinée qui entre dans la boîte : la ligne d'entrée est douce (la distance de la carte prise à la place de chaque
    // échantillon), loin des côtés de la boîte.
    let mut doc = projet();
    poser_decor(&mut doc, vec![objet_boite(19221, [-0.5, 0.0, -0.2], [0.5, 2.0, 0.2], [0.0, -1.0, -1.0])]);
    let boite = voile_seul(&rs, &doc, &derriere);
    let incl = crate::espace3d::Xf { pos: [0.0, 0.0, -1.0], rot: [25.0, 60.0, 0.0], size: [2.0, 1.0, 1.0] };
    let img = voile_seul(&rs, &doc, &incl);
    let mut entree = 0;
    for y in 3..H - 3 {
        for x in 3..W - 3 {
            let dans_la_boite = (y - 3..y + 4).all(|yy| (x - 3..x + 4).all(|xx| boite[yy * W + xx].a() == 255));
            let a = img[y * W + x].a();
            if dans_la_boite && a > 0 && a < 255 {
                entree += 1;
            }
        }
    }
    assert!(entree >= 10, "{entree} pixels doux sur la ligne d'entrée de la carte dans la boîte");
    eprintln!("vue Espace 3D : {partiels} pixels doux au bord du mur, {entree} sur la ligne d'entrée");
}


// ---------------------------------------------------------------------------------------------- allégée

/// Les passes de l'image du plan faites par la carte de `rs` depuis le début : (couleur, profondeur à quatre échantillons).
fn passes(rs: &RenderState) -> (u64, u64) {
    rs.renderer.read().callback_resources.get::<crate::decor3dgpu::Moteur>().map_or((0, 0), |m| (m.passes_couleur, m.passes_quatre))
}

fn image(gpu: &mut crate::gpu::GpuCanvas, rs: &RenderState, doc: &Document) -> Vec<Color32> {
    let plan = crate::render::panel_plan_at(doc, 0, 0, None);
    crate::gpu_tests::render_with(gpu, rs, doc, &plan, true)
}

#[test]
fn sans_carte_qui_partage_la_profondeur_la_passe_a_quatre_echantillons_ne_tourne_pas() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    crate::decor3d::essai_bords(Some(true));
    for sorte in ["sans carte", "devant", "accroché"] {
        let mut doc = projet();
        poser_decor(&mut doc, vec![boite_tournee(19230)]);
        if sorte != "sans carte" {
            let li = calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
            match sorte {
                "devant" => doc.panels[0].layers[li].devant_decor = true,
                _ => doc.panels[0].layers[li].accroche = true,
            }
        }
        let plan = crate::render::panel_plan_at(&doc, 0, 0, None);
        assert!(plan.items.iter().all(|i| i.profondeur.is_none()), "{sorte} : aucune carte ne partage la profondeur");
        let avant = passes(&rs);
        let mut gpu = crate::gpu::GpuCanvas::new(&rs);
        let img = image(&mut gpu, &rs, &doc);
        let apres = passes(&rs);
        assert_eq!(apres.1, avant.1, "{sorte} : pas de profondeur à quatre échantillons");
        assert_eq!(apres.0, avant.0 + 1, "{sorte} : la passe de couleur, une fois");
        // La même image que sans les bords doux (le chemin d'avant la partie 193, forcé).
        crate::decor3d::essai_bords(Some(false));
        let mut gpu2 = crate::gpu::GpuCanvas::new(&rs);
        assert!(img == image(&mut gpu2, &rs, &doc), "{sorte} : l'image d'avant");
        crate::decor3d::essai_bords(Some(true));
    }
    crate::decor3d::essai_bords(None);
}

#[test]
fn toujours_devant_sur_la_seule_carte_change_de_chemin_sans_redessiner_le_decor() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    crate::decor3d::essai_bords(Some(true));
    let mut doc = projet();
    poser_decor(&mut doc, vec![boite_tournee(19231)]);
    let li = calque_bleu(&mut doc, "Perso", GRANDE, -3.0);
    doc.panels[0].layers[li].devant_decor = true;
    let decor = sans_cartes(&mut doc, |d| composee(&rs, d));
    let mut gpu = crate::gpu::GpuCanvas::new(&rs);
    // 1. « Toujours devant » : la profondeur d'avant, la carte toute visible.
    let p0 = passes(&rs);
    let devant = image(&mut gpu, &rs, &doc);
    let p1 = passes(&rs);
    assert_eq!((p1.0 - p0.0, p1.1 - p0.1), (1, 0), "la couleur, sans les quatre échantillons");
    assert!((GRANDE[2]..GRANDE[3]).all(|y| (GRANDE[0]..GRANDE[1]).all(|x| devant[y * W + x] == BLEU)));
    // 2. Décochée : les quatre échantillons seuls, la couleur du décor n'est pas refaite ; les bords doux.
    doc.panels[0].layers[li].devant_decor = false;
    let doux = image(&mut gpu, &rs, &doc);
    let p2 = passes(&rs);
    assert_eq!((p2.0 - p1.0, p2.1 - p1.1), (0, 1), "les quatre échantillons seuls");
    le_bord_d_un_objet_devant_une_carte_est_doux(&doux, &decor, "après « Toujours devant » décoché");
    let mut neuve = crate::gpu::GpuCanvas::new(&rs);
    assert!(doux == image(&mut neuve, &rs, &doc), "la même image que d'une composition neuve");
    let p3 = passes(&rs);
    // 3. Recochée : rien à redessiner, la carte toute visible ; 4. décochée encore : les quatre échantillons déjà faits servent.
    doc.panels[0].layers[li].devant_decor = true;
    assert!(devant == image(&mut gpu, &rs, &doc));
    doc.panels[0].layers[li].devant_decor = false;
    assert!(doux == image(&mut gpu, &rs, &doc));
    assert_eq!(passes(&rs), p3, "ni couleur ni profondeur redessinées");
    // 5. Le décor trop lourd pour les bords doux (le budget le dit) : la profondeur d'avant, des bords nets, au pixel.
    crate::decor3d::essai_bords(Some(false));
    let mut lourd = crate::gpu::GpuCanvas::new(&rs);
    let net = image(&mut lourd, &rs, &doc);
    assert_eq!(passes(&rs).1, p3.1, "pas de quatre échantillons");
    let bords = (0..W * H).filter(|&i| net[i] != decor[i] && net[i] != BLEU).count();
    assert_eq!(bords, 0, "des bords nets : chaque pixel est la carte ou le décor");
    crate::decor3d::essai_bords(None);
}

#[test]
fn les_ombres_des_cartes_marchent_avec_et_sans_les_bords_doux() {
    let Some(rs) = crate::gpu_tests::headless() else { return };
    let mut doc = projet();
    poser_decor(&mut doc, vec![boite_tournee(19232)]);
    let seq = doc.sequence_of(0);
    doc.decors3d.par_sequence.get_mut(&seq).unwrap().aspect.ombres = true;
    calque_bleu(&mut doc, "Perso", [200, 240, 60, 140], -0.5);
    crate::decor3d::essai_poser(Some(crate::decor3dbudget::Qualite::Hautes), None);
    let images: Vec<Vec<Color32>> = [false, true]
        .into_iter()
        .map(|b| {
            crate::decor3d::essai_bords(Some(b));
            composee(&rs, &doc)
        })
        .collect();
    crate::decor3d::essai_bords(None);
    crate::decor3d::essai_poser(None, None);
    let differents = (0..W * H).filter(|&i| (0..4).any(|k| (images[0][i][k] as i32 - images[1][i][k] as i32).abs() > 3)).count();
    assert!(differents * 100 < W * H, "les ombres sont les mêmes, sauf au bord du décor : {differents} pixels");
}

#[test]
fn l_etalonnage_donne_le_budget_d_avant_et_celui_des_bords_doux_a_part() {
    use crate::decor3dbudget::{budget_depuis, BudgetMachine, Chronos, Cout, Qualite};
    let c = |pm: f32| Cout { fixe: 0.2, par_million: pm };
    let sans = Chronos { gpu: Some([c(1.0), c(1.6), c(2.4)]), prof4: None, cpu: Cout { fixe: 5.0, par_million: 900.0 }, vram: Some(8 << 30), ram: Some(32 << 30) };
    let avec = Chronos { prof4: Some(Cout { fixe: 0.05, par_million: 0.5 }), ..sans.clone() };
    let (a, b) = (budget_depuis(&sans, "A".into(), 1), budget_depuis(&avec, "A".into(), 1));
    // Le budget d'un décor est celui d'avant la partie 193 : la passe sans ombres dans le budget d'une passe.
    let ms = BudgetMachine::ms_par_passe();
    assert_eq!(b.triangles, c(1.0).triangles_sous(ms).min(crate::decor3d::PLAFONDS.triangles as u64));
    assert_eq!(a.triangles, b.triangles, "le surcoût des bords doux ne touche pas au budget");
    assert_eq!(b.qualite_pour(500_000), a.qualite_pour(500_000), "ni à la qualité des ombres");
    // Les bords doux : la passe plus leur surcoût.
    assert_eq!(b.triangles_bords, Cout { fixe: 0.25, par_million: 1.5 }.triangles_sous(ms).min(crate::decor3d::PLAFONDS.triangles as u64));
    assert!(b.triangles_bords < b.triangles);
    assert!(b.bords_doux_pour(b.triangles_bords, Qualite::Sans) && !b.bords_doux_pour(b.triangles, Qualite::Sans));
    assert!(!b.bords_doux_pour(b.triangles_bords, Qualite::Hautes) || c(2.4).ms(b.triangles_bords) + 0.05 + 0.5 * b.triangles_bords as f32 / 1e6 <= ms);
    assert!(b.texte().contains("bords doux"), "{}", b.texte());
    // Sans le surcoût mesuré (un budget gardé d'avant la partie 193), pas de bords doux, et le budget se remesure.
    assert!(!a.bords_doux_pour(1000, Qualite::Sans));
    assert!(!crate::decor3dbudget::vaut_pour(Some(&a), "A") && crate::decor3dbudget::vaut_pour(Some(&b), "A"));
    let cpu = budget_depuis(&Chronos { gpu: None, ..avec }, "C".into(), 1);
    assert!(crate::decor3dbudget::vaut_pour(Some(&cpu), "C") && !cpu.bords_doux_pour(1000, Qualite::Sans));
}
