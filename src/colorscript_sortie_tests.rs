//! Essais de la sortie dédiée du color script (partie 109) : présentations (planche, bande continue, bande de couleurs seule), image d'un plan
//! (première, milieu, moyenne, première clé), sorties (PNG, PNG par séquence, PDF A3 / A4 avec bloc de titre), données CSV et JSON, étendue, noms
//! du modèle de noms, annulation, film de 300 plans, case du paquet de livraison. Les fichiers sont relus pour de bon.

use image::{Rgb, RgbImage};

use super::tests::{dossier, lancer, plans, tout};
use super::*;
use crate::paquet::{PaquetJob, Reglages as PaquetReglages};
use crate::perf_tests::Banc;
use crate::relectureui::Etendue;
use crate::vignettes_tests::{deux_sequences, ouvert};

/// Quatre plans (24, 48, 24 puis 72 images) en deux séquences : un plan rouge, un bleu, un vert, un jaune.
fn film_couleurs() -> Document {
    let mut doc = deux_sequences();
    doc.properties.title = "Le Lac Étoilé".into();
    doc.properties.episode = "2".into();
    for (p, c) in [[255u8, 0, 0], [0, 0, 255], [0, 200, 0], [255, 255, 0]].iter().enumerate() {
        doc.ajouter_couleur_unie(p, *c, None, Some(0)).expect("couleur unie");
    }
    for (p, n) in [24usize, 48, 24, 72].iter().enumerate() {
        doc.panels[p].set_length(*n);
    }
    let (s0, s1) = (doc.sequence_of(0), doc.sequence_of(3));
    assert_ne!(s0, s1);
    for (id, nom) in [(s0, "Séquence « Été », finale"), (s1, "Épilogue")] {
        doc.sequences.iter_mut().find(|s| s.id == id).expect("séquence").name = nom.into();
    }
    doc
}

const ROUGE: [u8; 3] = [255, 0, 0];
const BLEU: [u8; 3] = [0, 0, 255];
const VERT: [u8; 3] = [0, 200, 0];
const JAUNE: [u8; 3] = [255, 255, 0];

fn proche(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|c| (a[c] as i32 - b[c] as i32).abs() <= 3)
}

/// Pour un JPEG : à quelques points près.
fn proche_large(a: [u8; 3], b: [u8; 3]) -> bool {
    (0..3).all(|c| (a[c] as i32 - b[c] as i32).abs() <= 16)
}

fn centre(img: &RgbImage, e: &Entree) -> [u8; 3] {
    img.get_pixel((e.x + e.w as i32 / 2) as u32, (e.y + e.h as i32 / 2) as u32).0
}

fn entrees(spec: &Spec) -> Vec<&Entree> {
    spec.blocs.iter().flat_map(|b| b.entrees.iter()).collect()
}

/// Lance l'export `o` des plans `plans` vers `out` et attend sa fin : le compte rendu doit être une réussite.
fn faire(b: &mut Banc, plans: &[usize], o: &Options, out: &std::path::Path) {
    b.app.export_job = None;
    let job = b.app.colorscript_job(plans, o, out).expect("tâche");
    lancer(b, job);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone()).expect("fini");
    assert!(fin.as_ref().is_ok_and(|m| m.contains("Color script exporté")), "{fin:?}");
}

#[test]
fn the_three_presentations_lay_out_as_asked() {
    let doc = film_couleurs();
    let o = Options { presentation: Presentation::Bande, bande_largeur: 1000, bande_hauteur: 100, espace: 4, ..Options::default() };
    let s = construire(&doc, &tout(&doc), &o);
    let e = entrees(&s);
    assert_eq!(e.len(), 4);
    // Une seule ligne, largeur totale juste (un écart entre les deux séquences), colonnes collées dans une séquence.
    assert!(e.iter().all(|x| x.y == e[0].y && x.h == 100));
    assert_eq!(e.iter().map(|x| x.w).sum::<u32>(), 1000 - 4);
    assert_eq!(s.taille[0], 1000 + 2 * s.marge as u32);
    assert_eq!(e[1].x, e[0].x + e[0].w as i32);
    assert_eq!(e[3].x, e[2].x + e[2].w as i32 + 4, "l'écart entre séquences");
    // La largeur suit la durée : 24, 48, 24, 72 images.
    for (x, d) in e.iter().zip([24.0f32, 48.0, 24.0, 72.0]) {
        assert!((x.w as f32 / 996.0 - d / 168.0).abs() < 0.004, "largeur {} pour {d} images", x.w);
    }
    assert!((e[1].w as f32 / e[0].w as f32 - 2.0).abs() < 0.05 && (e[3].w as f32 / e[0].w as f32 - 3.0).abs() < 0.05);
    // Toutes égales.
    let egales = construire(&doc, &tout(&doc), &Options { proportionnelle: false, ..o.clone() });
    assert!(entrees(&egales).iter().all(|x| x.w == 249), "{:?}", entrees(&egales).iter().map(|x| x.w).collect::<Vec<_>>());
    // La bande de couleurs seule : mêmes colonnes, ni vignette ni bande dessous, des images de travail petites.
    let couleurs = construire(&doc, &tout(&doc), &Options { presentation: Presentation::Couleurs, bande: true, ..o.clone() });
    assert_eq!(entrees(&couleurs).iter().map(|x| (x.x, x.w)).collect::<Vec<_>>(), e.iter().map(|x| (x.x, x.w)).collect::<Vec<_>>());
    assert_eq!(couleurs.bande_h, 0);
    assert!(couleurs.echantillons().iter().all(|x| x.cible[0] == 64));
    // La bande continue a sa bande de couleurs dominantes sous les colonnes si on la demande, le minutage dans une rangée à part.
    let avec = construire(&doc, &tout(&doc), &Options { bande: true, minutage: true, ..o.clone() });
    assert!(avec.bande_h > 0 && avec.taille[1] > s.taille[1]);
    assert_eq!(entrees(&avec)[3].minutage.as_deref(), Some("0:04"), "le plan 4 entre à 96 images : 4 s");
    // La planche d'avant reste la même : des images toutes de la même taille.
    let p = construire(&doc, &tout(&doc), &Options::default());
    assert!(entrees(&p).iter().all(|x| (x.w, x.h) == (p.cellule[0], p.cellule[1])));
}

#[test]
fn the_image_of_a_plan_is_first_middle_average_or_first_key() {
    let mut doc = plans(1);
    doc.panels[0].set_length(24);
    assert_eq!(prise_de(&doc, Prise::Premiere, 0, 0, 24), (0, vec![0]));
    assert_eq!(prise_de(&doc, Prise::Milieu, 0, 0, 24), (11, vec![11]));
    assert_eq!(prise_de(&doc, Prise::Moyenne, 0, 0, 24), (11, vec![0, 5, 11, 17, 23]), "cinq images réparties, la première et la dernière comprises");
    assert_eq!(prise_de(&doc, Prise::Moyenne, 0, 0, 3).1, vec![0, 1, 2], "un plan court : toutes ses images");
    // La première clé : le plan commence vide, son premier dessin arrive à l'image 7 ; un calque masqué ne compte pas.
    let cel = doc.new_cel(crate::document::LayerKind::Bitmap);
    let autre = doc.new_cel(crate::document::LayerKind::Bitmap);
    let mut cache = crate::document::Layer::new("Caché", crate::document::LayerKind::Bitmap);
    cache.frames = vec![Some(autre); 24];
    cache.visible = false;
    doc.panels[0].layers.push(cache);
    let mut l = crate::document::Layer::new("Dessin", crate::document::LayerKind::Bitmap);
    l.frames = (0..24).map(|f| if f >= 7 { Some(cel) } else { None }).collect();
    doc.panels[0].layers.push(l);
    assert_eq!(premiere_cle(&doc, 0, 0, 24), 7);
    assert_eq!(prise_de(&doc, Prise::PremiereCle, 0, 0, 24), (7, vec![7]));
    // Dessin dès la première image : la clé est la première image ; rien de visible : la première image de l'étendue.
    doc.panels[0].layers.last_mut().unwrap().frames = vec![Some(cel); 24];
    assert_eq!(premiere_cle(&doc, 0, 0, 24), 0);
    doc.panels[0].layers.last_mut().unwrap().visible = false;
    assert_eq!(premiere_cle(&doc, 0, 3, 24), 3);
    // Dans la construction : l'entrée porte les images choisies.
    let spec = construire(&doc, &[0], &Options { prise: Prise::Moyenne, ..Options::default() });
    assert_eq!(spec.blocs[0].entrees[0].echantillons, vec![0, 5, 11, 17, 23]);
    assert_eq!(spec.echantillons().len(), 5);
    assert_eq!(spec.images(), vec![(0, 11)]);
}

#[test]
fn a_red_then_blue_film_gives_red_then_blue_in_every_presentation_and_widths_follow_durations() {
    let dir = dossier("couleurs");
    let mut b = ouvert(film_couleurs(), 3);
    let plans = tout(&b.app.doc);
    let attendues = [ROUGE, BLEU, VERT, JAUNE];
    let avant = (b.app.history.serial(), b.app.doc_epoch, b.app.doc.panels.len());
    for (nom, presentation) in [("planche", Presentation::Planche), ("bande", Presentation::Bande), ("couleurs", Presentation::Couleurs)] {
        let out = dir.join(format!("{nom}.png"));
        let o = Options { presentation, bande_largeur: 1000, bande_hauteur: 60, espace: 4, ..Options::default() };
        faire(&mut b, &plans, &o, &out);
        let spec = construire(&b.app.doc, &plans, &o);
        let img = image::open(&out).expect("PNG").to_rgb8();
        assert_eq!(img.dimensions(), (spec.taille[0], spec.taille[1]), "{nom}");
        for (k, e) in entrees(&spec).iter().enumerate() {
            assert!(proche(centre(&img, e), attendues[k]), "{nom} : plan {k} : {:?} au lieu de {:?}", centre(&img, e), attendues[k]);
        }
    }
    // La largeur proportionnelle, relue dans le fichier : le bleu (48 images) fait deux fois le rouge (24).
    let o = Options { presentation: Presentation::Couleurs, bande_largeur: 1000, bande_hauteur: 60, espace: 4, ..Options::default() };
    let img = image::open(dir.join("couleurs.png")).unwrap().to_rgb8();
    let spec = construire(&b.app.doc, &plans, &o);
    let e = entrees(&spec);
    let y = (e[0].y + 30) as u32;
    let course = |img: &RgbImage, couleur: [u8; 3]| (0..img.width()).filter(|&x| proche(img.get_pixel(x, y).0, couleur)).count() as i32;
    assert_eq!((course(&img, ROUGE), course(&img, BLEU), course(&img, VERT), course(&img, JAUNE)), (e[0].w as i32, e[1].w as i32, e[2].w as i32, e[3].w as i32));
    assert!((course(&img, BLEU) as f32 / course(&img, ROUGE) as f32 - 2.0).abs() < 0.05, "le bleu dure deux fois le rouge");
    assert!((course(&img, JAUNE) as f32 / course(&img, ROUGE) as f32 - 3.0).abs() < 0.05);
    // Toutes égales : des largeurs égales.
    let egales = Options { proportionnelle: false, ..o.clone() };
    faire(&mut b, &plans, &egales, &dir.join("egales.png"));
    let img = image::open(dir.join("egales.png")).unwrap().to_rgb8();
    assert_eq!((course(&img, ROUGE), course(&img, BLEU)), (249, 249));
    // Pour un plan uni, la moyenne et la dominante se confondent.
    faire(&mut b, &plans, &Options { couleur: CouleurPlan::Moyenne, ..o.clone() }, &dir.join("moyenne.png"));
    let img = image::open(dir.join("moyenne.png")).unwrap().to_rgb8();
    assert!(proche(centre(&img, e[2]), VERT));
    // Le projet n'a pas bougé.
    assert_eq!((b.app.history.serial(), b.app.doc_epoch, b.app.doc.panels.len()), avant, "l'export ne modifie pas le projet");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_average_over_the_duration_sits_between_the_first_and_the_last_image() {
    // Un plan de 24 images : du rouge, et du bleu par-dessus dont l'opacité monte de 0 à 1.
    let mut doc = Document::with_frame(160, 90, 24);
    doc.panels[0].set_length(24);
    doc.ajouter_couleur_unie(0, ROUGE, None, Some(0)).unwrap();
    doc.ajouter_couleur_unie(0, BLEU, None, None).unwrap();
    let haut = doc.panels[0].layers.len() - 1;
    doc.panels[0].layers[haut].put_opacity_key(0, 0.0);
    doc.panels[0].layers[haut].put_opacity_key(23, 1.0);
    let dir = dossier("moyenne");
    let mut b = ouvert(doc, 3);
    let mut couleur = |prise| {
        let o = Options { presentation: Presentation::Couleurs, couleur: CouleurPlan::Moyenne, prise, ..Options::default() };
        let out = dir.join("m.png");
        faire(&mut b, &[0], &o, &out);
        let spec = construire(&b.app.doc, &[0], &o);
        let img = image::open(&out).unwrap().to_rgb8();
        centre(&img, entrees(&spec)[0])
    };
    let (premiere, milieu, moyenne) = (couleur(Prise::Premiere), couleur(Prise::Milieu), couleur(Prise::Moyenne));
    assert!(proche(premiere, ROUGE), "{premiere:?}");
    assert!(milieu[2] > 80 && milieu[2] < 160, "{milieu:?}");
    assert!(moyenne[0] > 100 && moyenne[0] < 160 && moyenne[2] > 95 && moyenne[2] < 155 && moyenne[1] == 0, "{moyenne:?}");
    assert!((moyenne[0] as i32 - milieu[0] as i32).abs() < 30, "la moyenne est près du milieu (variation linéaire) : {moyenne:?} {milieu:?}");
    // La moyenne de deux images (le calcul lui-même).
    let mut c = Cumul { taille: [2, 1], somme: vec![0; 6], n: 0 };
    c.ajouter(&RgbImage::from_pixel(2, 1, Rgb([255, 0, 0])));
    c.ajouter(&RgbImage::from_pixel(2, 1, Rgb([0, 0, 255])));
    assert_eq!(c.image().get_pixel(0, 0).0, [128, 0, 128]);
    std::fs::remove_dir_all(&dir).ok();
}

/// Les octets de `bytes` à partir de `debut` commencent-ils par `s` ?
fn commence(bytes: &[u8], debut: usize, s: &str) -> bool {
    bytes.get(debut..).is_some_and(|b| b.starts_with(s.as_bytes()))
}

fn trouver(bytes: &[u8], s: &str) -> Option<usize> {
    bytes.windows(s.len()).position(|w| w == s.as_bytes())
}

/// Un PDF qui s'ouvre : la table des références pointe sur de vrais objets. Rend la taille de la page, l'image JPEG décodée et le texte de la page.
fn lire_pdf(bytes: &[u8]) -> ((f32, f32), RgbImage, String) {
    assert!(bytes.starts_with(b"%PDF-1.4") && bytes.ends_with(b"%%EOF\n"));
    let sx = trouver(bytes, "startxref\n").expect("startxref") + "startxref\n".len();
    let fin = bytes[sx..].iter().position(|&c| c == b'\n').unwrap();
    let xref: usize = std::str::from_utf8(&bytes[sx..sx + fin]).unwrap().parse().unwrap();
    assert!(commence(bytes, xref, "xref\n0 "), "startxref pointe sur la table");
    let ligne = bytes[xref + 5..].iter().position(|&c| c == b'\n').unwrap();
    let n: usize = std::str::from_utf8(&bytes[xref + 5 + 2..xref + 5 + ligne]).unwrap().trim().parse().unwrap();
    let debut = xref + 5 + ligne + 1;
    for i in 1..n {
        let entree = std::str::from_utf8(&bytes[debut + i * 20..debut + i * 20 + 20]).unwrap();
        let off: usize = entree[..10].parse().unwrap();
        assert!(commence(bytes, off, &format!("{i} 0 obj")), "objet {i} à l'adresse {off}");
    }
    let m = trouver(bytes, "/MediaBox [0 0 ").expect("page") + "/MediaBox [0 0 ".len();
    let fin = bytes[m..].iter().position(|&c| c == b']').unwrap();
    let dims: Vec<f32> = std::str::from_utf8(&bytes[m..m + fin]).unwrap().trim().split(' ').map(|x| x.parse().unwrap()).collect();
    // L'image : le flux qui suit « /Subtype /Image ».
    let i = trouver(bytes, "/Subtype /Image").expect("image");
    let s0 = i + trouver(&bytes[i..], "stream\n").unwrap() + "stream\n".len();
    let s1 = s0 + trouver(&bytes[s0..], "\nendstream").unwrap();
    let img = image::load_from_memory(&bytes[s0..s1]).expect("JPEG lisible").to_rgb8();
    // Le texte de la page : le flux de contenu.
    let t = trouver(bytes, "BT /F2 18").expect("titre");
    let t1 = t + trouver(&bytes[t..], "\nendstream").unwrap();
    ((dims[0], dims[1]), img, String::from_utf8_lossy(&bytes[t..t1]).to_string())
}

#[test]
fn the_pdf_opens_has_the_right_page_and_a_title_block() {
    let dir = dossier("pdf");
    let mut b = ouvert(film_couleurs(), 3);
    let plans = tout(&b.app.doc);
    for (papier, (w, h)) in [(Papier::A4, (841.89f32, 595.28f32)), (Papier::A3, (1190.55, 841.89))] {
        let out = dir.join(format!("cs_{}.pdf", papier.nom().replace(' ', "_")));
        let o = Options { sortie: Sortie::Pdf, papier, version: "v2 final".into(), presentation: Presentation::Bande, bande_largeur: 1600, bande_hauteur: 160, ..Options::default() };
        faire(&mut b, &plans, &o, &out);
        let bytes = std::fs::read(&out).unwrap();
        let ((pw, ph), img, texte) = lire_pdf(&bytes);
        assert_eq!((pw, ph), (w, h), "page {}", papier.nom());
        assert!(pw > ph, "paysage");
        assert!(trouver(&bytes, "/Count 1").is_some(), "une page");
        // L'image du color script est dedans, dans les bonnes proportions, avec ses couleurs (JPEG : à quelques points près).
        let spec = construire(&b.app.doc, &plans, &o);
        let ratio = |x: (u32, u32)| x.0 as f32 / x.1 as f32;
        assert!((ratio(img.dimensions()) - ratio((spec.taille[0], spec.taille[1]))).abs() < 0.01, "{:?} contre {:?}", img.dimensions(), spec.taille);
        let k = img.width() as f32 / spec.taille[0] as f32;
        let e = entrees(&spec);
        let pris = |e: &Entree| img.get_pixel(((e.x + e.w as i32 / 2) as f32 * k) as u32, ((e.y + e.h as i32 / 2) as f32 * k) as u32).0;
        assert!(proche_large(pris(e[0]), ROUGE) && proche_large(pris(e[1]), BLEU), "{:?} {:?}", pris(e[0]), pris(e[1]));
        // Le bloc de titre : le titre du projet, la version, la date, en texte (WinAnsi).
        let attendu = |s: &str| crate::pdf::pdf_string(&crate::pdf::winansi(s));
        assert!(texte.contains(&attendu("Le Lac Étoilé — épisode 2")), "titre dans {texte}");
        assert!(texte.contains(&attendu("Version : v2 final")));
        assert!(texte.contains(&attendu(&format!("Date : {}", crate::incrustation::date_text(crate::incrustation::now_secs())))));
        assert!(texte.contains("(Color script)"));
        // L'image est dans la page, sous le bloc de titre.
        let i = trouver(&bytes, " cm /Im").expect("pose de l'image");
        let m = String::from_utf8_lossy(&bytes[i.saturating_sub(60)..i]).to_string();
        let n: Vec<f32> = m.split_whitespace().rev().take(6).map(|x| x.parse().unwrap_or(f32::NAN)).collect();
        let (largeur_img, hauteur_img, x_img, y_img) = (n[5], n[2], n[1], n[0]);
        assert!(x_img >= 0.0 && x_img + largeur_img <= pw + 0.1 && y_img >= 0.0 && y_img + hauteur_img <= ph - 28.0 - 46.0 + 0.1, "image hors de la page ou sur le titre : {m}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_csv_and_the_json_are_exact_with_their_accents() {
    let dir = dossier("donnees");
    let mut b = ouvert(film_couleurs(), 3);
    let plans = tout(&b.app.doc);
    let noms = b.app.doc.noms_de_tous_les_plans();
    assert_eq!(noms[0], "SQ01_PL001");
    let o = Options { presentation: Presentation::Couleurs, donnees: Donnees::Csv, bande_largeur: 500, ..Options::default() };
    let out = dir.join("film.png");
    faire(&mut b, &plans, &o, &out);
    let attendu_csv = format!(
        "sequence,plan,case,entree_s,duree_s,moyenne,couleur_1,part_1,couleur_2,part_2,couleur_3,part_3,couleur_4,part_4,couleur_5,part_5\r\n\
         \"Séquence « Été », finale\",{},1,0.000,1.000,#FF0000,#FF0000,1.0000,,,,,,,,\r\n\
         \"Séquence « Été », finale\",{},1,1.000,2.000,#0000FF,#0000FF,1.0000,,,,,,,,\r\n\
         \"Séquence « Été », finale\",{},1,3.000,1.000,#00C800,#00C800,1.0000,,,,,,,,\r\n\
         Épilogue,{},1,4.000,3.000,#FFFF00,#FFFF00,1.0000,,,,,,,,\r\n",
        noms[0], noms[1], noms[2], noms[3]
    );
    let lu = std::fs::read(dir.join("film.csv")).expect("CSV à côté de l'image");
    assert!(!lu.starts_with(&[0xEF, 0xBB, 0xBF]), "UTF-8 sans BOM, comme la liste des plans");
    assert_eq!(String::from_utf8(lu).expect("UTF-8"), attendu_csv);
    // Le JSON : les mêmes valeurs, les nombres sont des nombres.
    faire(&mut b, &plans, &Options { donnees: Donnees::Json, ..o.clone() }, &out);
    let texte = std::fs::read_to_string(dir.join("film.json")).expect("JSON à côté de l'image");
    assert!(texte.contains("« Été »") && texte.contains("Épilogue") && texte.contains("Étoilé"), "les accents passent tels quels, sans \\u");
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    assert_eq!((v["format"].as_str(), v["projet"].as_str(), v["episode"].as_str(), v["cadence"].as_u64()), (Some("feanor-color-script"), Some("Le Lac Étoilé"), Some("2"), Some(24)));
    let images = v["images"].as_array().unwrap();
    assert_eq!(images.len(), 4);
    assert_eq!(images[1]["sequence"], "Séquence « Été », finale");
    assert_eq!(images[1]["plan"], noms[1].as_str());
    assert_eq!(images[1]["entree_s"], 1.0);
    assert_eq!(images[1]["duree_s"], 2.0);
    assert_eq!(images[1]["moyenne"], "#0000FF");
    assert_eq!(images[1]["couleurs"], serde_json::json!([{ "hex": "#0000FF", "part": 1.0 }]));
    assert_eq!(images[3]["sequence"], "Épilogue");
    assert_eq!(images[3]["entree_s"], 4.0);
    assert_eq!(images[3]["duree_s"], 3.0);
    // Des parts qui se partagent : deux couleurs dans une image.
    let doc = film_couleurs();
    let spec = construire(&doc, &[0, 1], &Options { donnees: Donnees::Csv, ..Options::default() });
    let l = lignes(&spec, &[(RgbImage::from_pixel(8, 8, Rgb([10, 20, 30])), vec![(ROUGE, 0.75), (BLEU, 0.25)]), (RgbImage::from_pixel(8, 8, Rgb([0, 0, 0])), Vec::new())]);
    assert_eq!(l.len(), 2);
    let texte = csv(&l);
    assert!(texte.lines().nth(1).unwrap().ends_with(",#FF0000,0.7500,#0000FF,0.2500,,,,,,"), "{texte}");
    assert!(texte.lines().nth(2).unwrap().ends_with(",#000000,,,,,,,,,,"), "{texte}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_scope_the_names_and_one_png_per_sequence() {
    let dir = dossier("noms");
    let mut doc = film_couleurs();
    // Le modèle d'origine : le film porte le nom du projet ; une séquence, « SQ01 ».
    assert_eq!(nom_modele(&doc, 0, true), "SQ01");
    assert_eq!(nom_modele(&doc, 3, true), "SQ02");
    assert_eq!(nom_modele(&doc, 0, false), crate::paquet::base_du_film(&doc));
    // Un modèle de studio : l'épisode et la séquence, sans plan ni version.
    doc.properties.serie = "Brawlidays".into();
    doc.properties.modele_noms = crate::nommage::MODELES_PRETS[1].1.into();
    assert_eq!(nom_modele(&doc, 0, false), "Brawlidays_EP02");
    assert_eq!(nom_modele(&doc, 1, true), "Brawlidays_EP02_SQ001");
    assert_eq!(nom_modele(&doc, 3, true), "Brawlidays_EP02_SQ002");
    let tous = tout(&doc);
    assert_eq!(noms_de_fichiers(&doc, &tous, &Options::default()), ["Brawlidays_EP02_colorscript"]);
    assert_eq!(noms_de_fichiers(&doc, &[0, 1, 2], &Options::default()), ["Brawlidays_EP02_SQ001_colorscript"], "une seule séquence : son nom");
    let par_seq = Options { sortie: Sortie::PngParSequence, ..Options::default() };
    assert_eq!(noms_de_fichiers(&doc, &tous, &par_seq), ["Brawlidays_EP02_SQ001_colorscript", "Brawlidays_EP02_SQ002_colorscript"]);
    // Un modèle sans numéro de séquence : les deux noms seraient égaux, le second prend « _2 ».
    doc.properties.modele_noms = "{projet}_{plan:03}".into();
    assert_eq!(noms_de_fichiers(&doc, &tous, &par_seq), ["Brawlidays_colorscript", "Brawlidays_colorscript_2"]);
    doc.properties.modele_noms = crate::nommage::MODELES_PRETS[1].1.into();
    // Les chemins : une image d'ensemble avec ses données à côté ; un dossier de PNG par séquence.
    let (images, donnees) = chemins(&doc, &tous, &Options { donnees: Donnees::Json, ..Options::default() }, &dir.join("film.png"));
    assert_eq!((images, donnees), (vec![dir.join("film.png")], Some(dir.join("film.json"))));
    let (images, donnees) = chemins(&doc, &tous, &Options { donnees: Donnees::Csv, ..par_seq.clone() }, &dir);
    assert_eq!(images, vec![dir.join("Brawlidays_EP02_SQ001_colorscript.png"), dir.join("Brawlidays_EP02_SQ002_colorscript.png")]);
    assert_eq!(donnees, Some(dir.join("Brawlidays_EP02_colorscript.csv")));
    assert_eq!(chemins(&doc, &tous, &Options::default(), &dir.join("a.png")).1, None);
    // Un PNG par séquence, pour de bon : deux fichiers de la bonne taille, les bonnes couleurs, et les données du film entier.
    let mut b = ouvert(doc, 3);
    let o = Options { donnees: Donnees::Csv, presentation: Presentation::Bande, bande_largeur: 800, bande_hauteur: 50, ..par_seq.clone() };
    faire(&mut b, &tous, &o, &dir);
    let mut fichiers: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    fichiers.sort();
    assert_eq!(fichiers, ["Brawlidays_EP02_SQ001_colorscript.png", "Brawlidays_EP02_SQ002_colorscript.png", "Brawlidays_EP02_colorscript.csv"]);
    let sp = specs(&b.app.doc, &tous, &o);
    assert_eq!(sp.len(), 2);
    let (i0, i1) = (image::open(dir.join(&fichiers[0])).unwrap().to_rgb8(), image::open(dir.join(&fichiers[1])).unwrap().to_rgb8());
    assert_eq!((i0.dimensions(), i1.dimensions()), ((sp[0].taille[0], sp[0].taille[1]), (sp[1].taille[0], sp[1].taille[1])));
    let e0 = entrees(&sp[0]);
    assert_eq!(e0.len(), 3);
    assert!(proche(centre(&i0, e0[0]), ROUGE) && proche(centre(&i0, e0[1]), BLEU) && proche(centre(&i0, e0[2]), VERT));
    assert!(proche(centre(&i1, entrees(&sp[1])[0]), JAUNE), "la seconde séquence : le plan jaune seul");
    assert_eq!(std::fs::read_to_string(dir.join(&fichiers[2])).unwrap().lines().count(), 5, "l'en-tête et les quatre plans du film");
    // L'étendue : la séquence courante, puis les plans choisis (comme les autres exports).
    let seq = b.app.plans_de(Etendue::Sequence, None);
    assert_eq!(seq, vec![0, 1, 2]);
    let os = Options { presentation: Presentation::Couleurs, bande_largeur: 600, ..Options::default() };
    faire(&mut b, &seq, &os, &dir.join("seq.png"));
    assert_eq!(entrees(&construire(&b.app.doc, &seq, &os)).len(), 3);
    let choisis = vec![b.app.doc.panels[1].id, b.app.doc.panels[3].id];
    b.app.board_sel = choisis;
    let plans_choisis = b.app.plans_de(Etendue::Choisis, None);
    assert_eq!(plans_choisis, vec![1, 3]);
    let ch = dir.join("choisis.png");
    faire(&mut b, &plans_choisis, &os, &ch);
    let spec = construire(&b.app.doc, &plans_choisis, &os);
    let img = image::open(&ch).unwrap().to_rgb8();
    let e = entrees(&spec);
    assert_eq!(e.len(), 2);
    assert!(proche(centre(&img, e[0]), BLEU) && proche(centre(&img, e[1]), JAUNE));
    assert_eq!(spec.blocs.len(), 2, "deux séquences, deux blocs");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelling_leaves_no_file_not_even_the_data_or_a_sequence_image() {
    let dir = dossier("annuler");
    let mut b = ouvert(film_couleurs(), 3);
    let plans = tout(&b.app.doc);
    for o in [
        Options { donnees: Donnees::Csv, ..Options::default() },
        Options { sortie: Sortie::PngParSequence, donnees: Donnees::Json, prise: Prise::Moyenne, ..Options::default() },
        Options { sortie: Sortie::Pdf, presentation: Presentation::Couleurs, ..Options::default() },
    ] {
        b.app.export_job = None;
        let out = if o.sortie == Sortie::PngParSequence { dir.clone() } else { dir.join(format!("x.{}", o.sortie.extension())) };
        let job = b.app.colorscript_job(&plans, &o, &out).unwrap();
        b.app.start_job(job);
        if let Some(j) = b.app.export_job.as_mut() {
            j.cancel();
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        b.chauffer(3);
        let restes: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(restes.is_empty(), "rien ne reste après l'annulation : {restes:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_film_of_300_plans_is_prepared_fast_and_never_freezes_the_screen() {
    let doc = plans(300);
    let t = std::time::Instant::now();
    let o = Options { prise: Prise::Moyenne, donnees: Donnees::Csv, minutage: true, numeros: true, presentation: Presentation::Bande, ..Options::default() };
    let spec = construire(&doc, &tout(&doc), &o);
    assert_eq!(spec.nombre(), 300);
    assert_eq!(entrees(&spec).iter().map(|e| e.w).sum::<u32>(), 3000);
    assert!(t.elapsed() < std::time::Duration::from_millis(2000), "la préparation d'un film de 300 plans : {:?}", t.elapsed());
    // Le rendu se fait par la tâche de fond d'export : l'écran n'attend jamais plus de quelques dizaines de millisecondes par image.
    let dir = dossier("trois-cents");
    let mut b = ouvert(doc, 3);
    let plans = tout(&b.app.doc);
    let job = b.app.colorscript_job(&plans, &Options { presentation: Presentation::Couleurs, ..Options::default() }, &dir.join("long.png")).unwrap();
    b.app.start_job(job);
    let (debut, mut pire) = (std::time::Instant::now(), std::time::Duration::ZERO);
    while b.app.export_job.as_ref().is_some_and(|j| j.outcome.is_none()) && debut.elapsed() < std::time::Duration::from_secs(120) {
        let t = std::time::Instant::now();
        b.image(Vec::new(), 1.0 / 60.0);
        pire = pire.max(t.elapsed());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    b.chauffer(2);
    let fin = b.app.export_job.as_ref().and_then(|j| j.outcome.clone());
    assert!(fin.as_ref().is_some_and(|o| o.is_ok()), "{fin:?}");
    assert!(pire < std::time::Duration::from_secs(2), "une image d'écran a duré {pire:?}");
    assert!(dir.join("long.png").is_file());
    std::fs::remove_dir_all(&dir).ok();
}

fn courir(job: &mut PaquetJob, doc: &Document) {
    let t = std::time::Instant::now();
    while job.running() {
        job.step(doc, std::time::Duration::from_millis(8));
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(t.elapsed() < std::time::Duration::from_secs(240), "paquet trop long : {:?}", job.status().0);
    }
}

fn fichiers_de(dir: &std::path::Path) -> Vec<String> {
    fn rec(d: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rec(&p, base, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"));
            }
        }
    }
    let mut v = Vec::new();
    rec(dir, dir, &mut v);
    v.sort();
    v
}

#[test]
fn the_delivery_package_has_a_colour_script_box_off_by_default_and_the_others_do_not_change() {
    let doc = film_couleurs();
    let sans_ffmpeg = PaquetReglages { videos: false, animatique: false, ..PaquetReglages::default() };
    assert!(!PaquetReglages::default().color_script, "décochée d'origine");
    let rien = PaquetReglages { videos: false, vignettes: false, pdf: false, animatique: false, liste_csv: false, liste_json: false, ..Default::default() };
    assert!(rien.vide());
    assert!(!PaquetReglages { color_script: true, ..rien.clone() }.vide(), "le color script suffit à faire un paquet");
    // Sans la case : exactement les fichiers d'avant.
    let dest = dossier("paquet-sans");
    let mut job = PaquetJob::start(&doc, &sans_ffmpeg, 0, None, &dest, Vec::new()).unwrap();
    courir(&mut job, &doc);
    job.outcome.take().unwrap().expect("paquet");
    let sans = fichiers_de(&dest);
    assert!(!sans.is_empty() && sans.iter().all(|f| !f.contains("colorscript")), "{sans:?}");
    // Avec la case : le PNG d'ensemble et le CSV, au bon endroit, dans le manifeste, et rien d'autre de changé.
    let dest2 = dossier("paquet-avec");
    let mut job = PaquetJob::start(&doc, &PaquetReglages { color_script: true, ..sans_ffmpeg.clone() }, 0, None, &dest2, Vec::new()).unwrap();
    courir(&mut job, &doc);
    let rapport = job.outcome.take().unwrap().expect("paquet");
    let avec = fichiers_de(&dest2);
    let base = crate::paquet::base_du_film(&doc);
    let racine = "Le Lac Étoilé/EP02/storyboard";
    let nouveaux: Vec<&String> = avec.iter().filter(|f| !sans.contains(f)).collect();
    assert_eq!(nouveaux, [&format!("{racine}/{base}_colorscript.csv"), &format!("{racine}/{base}_colorscript.png")]);
    assert_eq!(sans.iter().filter(|f| !f.ends_with("manifest.json")).count(), avec.iter().filter(|f| !f.contains("colorscript") && !f.ends_with("manifest.json")).count());
    let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&rapport.manifeste).unwrap()).unwrap();
    let sortes: Vec<&str> = m["fichiers"].as_array().unwrap().iter().filter(|f| f["chemin"].as_str().unwrap().contains("colorscript")).map(|f| f["sorte"].as_str().unwrap()).collect();
    assert_eq!(sortes, ["color_script", "color_script"]);
    for f in m["fichiers"].as_array().unwrap() {
        let octets = std::fs::read(rapport.racine.join(f["chemin"].as_str().unwrap())).unwrap();
        assert_eq!(f["sha256"].as_str().unwrap(), crate::sha256::de_octets(&octets));
    }
    let png = image::open(rapport.racine.join(format!("{base}_colorscript.png"))).expect("PNG du paquet").to_rgb8();
    let spec = construire(&doc, &tout(&doc), &Options { numeros: true, bande: true, donnees: Donnees::Csv, ..Options::default() });
    assert_eq!(png.dimensions(), (spec.taille[0], spec.taille[1]));
    assert!(proche(centre(&png, entrees(&spec)[0]), ROUGE) && proche(centre(&png, entrees(&spec)[1]), BLEU));
    let csv = std::fs::read_to_string(rapport.racine.join(format!("{base}_colorscript.csv"))).unwrap();
    assert_eq!(csv.lines().count(), 5);
    // Arrêter le paquet pendant le color script : rien ne reste.
    let dest3 = dossier("paquet-annule");
    let seul = PaquetReglages { color_script: true, ..rien };
    let mut job = PaquetJob::start(&doc, &seul, 0, None, &dest3, Vec::new()).unwrap();
    job.step(&doc, std::time::Duration::from_millis(8));
    job.cancel();
    assert_eq!(std::fs::read_dir(&dest3).unwrap().count(), 0, "{:?}", std::fs::read_dir(&dest3).unwrap().flatten().map(|e| e.file_name()).collect::<Vec<_>>());
    for d in [dest, dest2, dest3] {
        std::fs::remove_dir_all(&d).ok();
    }
}
