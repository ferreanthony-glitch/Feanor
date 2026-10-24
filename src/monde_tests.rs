//! Le hors-feuille de l'écran (`monde.rs`, audit du 2026-10-23, point 7) : il ne se refait que quand ce que le plan pose
//! hors de la feuille change, sur un autre fil, et son résultat est celui d'un hors-feuille refait de zéro.

use std::time::{Duration, Instant};

use eframe::egui::{self, Color32};

use crate::blend::BlendMode;
use crate::document::{Cel, CelId, Document, LayerKind};
use crate::monde::Monde;
use crate::raster::{outer_id, Tile, TILE, TILE_PIXELS};
use crate::render::{Area, Plan, PlanItem, Pre};
use crate::vector::Affine;

/// Une tuile pleine de cette couleur, avec un motif (pour que les décalages se voient).
fn tuile(c: [u8; 4], motif: usize) -> Tile {
    let mut px = [Color32::TRANSPARENT; TILE_PIXELS];
    for (i, p) in px.iter_mut().enumerate() {
        if (i / 7 + i % 5 + motif) % 3 != 0 {
            *p = Color32::from_rgba_premultiplied(c[0], c[1], c[2], c[3]);
        }
    }
    Tile::new(px)
}

/// Un dessin de la feuille `doc` : ces tuiles de la fenêtre et ces tuiles hors d'elle (coordonnées de tuile).
fn dessin(doc: &mut Document, fenetre: &[usize], dehors: &[(i32, i32)], c: [u8; 4]) -> CelId {
    let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
    for (k, &i) in fenetre.iter().enumerate() {
        cel.raster.replace_slot(i, Some(tuile(c, k)));
    }
    for (k, &(tx, ty)) in dehors.iter().enumerate() {
        cel.raster.replace_slot(outer_id(tx, ty), Some(tuile(c, k + 1)));
    }
    doc.add_cel(cel)
}

fn item(cel: CelId) -> PlanItem {
    PlanItem { cel, opacity: 255, tint: None, transform: None, blend: BlendMode::Normal, fx: Vec::new(), ctg: None, fond: None, pre: None }
}

/// Une feuille 400 × 300 (ses tuiles de bord dépassent : 400 = 6 × 64 + 16) : deux calques qui dépassent (deux dessins
/// chacun), deux qui restent dans la feuille.
struct Scene {
    doc: Document,
    dehors: [[CelId; 2]; 2],
    dedans: [[CelId; 2]; 2],
}

fn scene() -> Scene {
    let mut doc = Document::new(400, 300, 24);
    // (Fenêtre de 7 × 5 tuiles : la colonne 6 et la rangée 4 débordent de la feuille.)
    let a = [dessin(&mut doc, &[0, 6, 13], &[(-2, 1), (-1, 1), (-1, 2)], [200, 30, 30, 255]), dessin(&mut doc, &[7, 20], &[(-3, 0), (-1, -1)], [180, 60, 20, 230])];
    let b = [dessin(&mut doc, &[34, 27], &[(8, 4), (9, 5), (7, -2)], [20, 40, 200, 200]), dessin(&mut doc, &[33], &[(8, 6), (10, 1)], [30, 90, 210, 255])];
    // (Les calques qui restent dans la feuille sont loin des bords : même déplacés de quelques pixels, ils n'en sortent pas.)
    let c = [dessin(&mut doc, &[8, 9, 15], &[], [20, 160, 40, 255]), dessin(&mut doc, &[16, 18], &[], [60, 140, 40, 180])];
    let d = [dessin(&mut doc, &[10, 17], &[], [120, 20, 140, 255]), dessin(&mut doc, &[11], &[], [150, 40, 160, 255])];
    Scene { doc, dehors: [a, b], dedans: [c, d] }
}

/// Un hasard reproductible.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Les réglages d'un calque du plan, tirés au hasard ou laissés.
#[derive(Clone)]
struct Calque {
    dessin: usize,
    visible: bool,
    opacity: u8,
    tint: Option<[u8; 3]>,
    blend: BlendMode,
    deplace: Option<(f32, f32, f32)>,
}

impl Calque {
    fn neuf() -> Calque {
        Calque { dessin: 0, visible: true, opacity: 255, tint: None, blend: BlendMode::Normal, deplace: None }
    }

    fn tirer(&mut self, r: &mut Lcg) {
        match r.below(6) {
            0 => self.dessin = 1 - self.dessin,
            1 => self.visible = !self.visible || r.below(3) > 0,
            2 => self.opacity = [255, 200, 120, 40][r.below(4) as usize],
            3 => self.tint = [None, Some([255, 0, 0]), Some([0, 120, 255])][r.below(3) as usize],
            4 => self.blend = [BlendMode::Normal, BlendMode::Darken, BlendMode::Dissolve][r.below(3) as usize],
            _ => self.deplace = [None, Some((30.0, -20.0, 1.0)), Some((-70.0, 15.0, 1.0)), Some((10.0, 5.0, 1.3))][r.below(4) as usize],
        }
    }

    fn element(&self, cels: &[CelId; 2]) -> Option<PlanItem> {
        if !self.visible {
            return None;
        }
        let pre = self.deplace.map(|(dx, dy, k)| Pre { aff: Affine { a: k, b: 0.0, c: 0.0, d: k, tx: dx, ty: dy }, dims: None, warp: None, card: false });
        Some(PlanItem { opacity: self.opacity, tint: self.tint, blend: self.blend, pre, ..item(cels[self.dessin]) })
    }
}

fn plan_de(s: &Scene, calques: &[Calque; 4]) -> Plan {
    let cels = [&s.dehors[0], &s.dedans[0], &s.dehors[1], &s.dedans[1]];
    Plan { paper: Some([255, 255, 255]), items: calques.iter().zip(cels).filter_map(|(c, cels)| c.element(cels)).collect() }
}

/// Fait avancer `monde` jusqu'à ce qu'il n'ait plus rien à composer (en laissant au fil du hors-feuille le temps d'arriver).
fn settle(monde: &mut Monde, ctx: &egui::Context, doc: &Document, plan: &Plan, view: Area) {
    monde.update(ctx, doc, plan, view, false, &[]);
    let t = Instant::now();
    while monde.pending() > 0 && t.elapsed() < Duration::from_secs(20) {
        if monde.busy() {
            std::thread::sleep(Duration::from_millis(1));
        }
        monde.update(ctx, doc, plan, view, false, &[]);
    }
    assert_eq!(monde.pending(), 0, "le hors-feuille finit");
}

/// Les deux hors-feuilles montrent les mêmes pixels partout où ils se recouvrent.
fn memes_pixels(a: &Monde, b: &Monde) -> Result<usize, String> {
    let (Some((za, ka)), Some((zb, kb))) = (a.coverage(), b.coverage()) else {
        return if a.coverage().is_none() && b.coverage().is_none() { Ok(0) } else { Err("l'un montre un hors-feuille, l'autre pas".into()) };
    };
    assert_eq!((ka, kb), (1, 1));
    let mut vus = 0;
    for y in za.y0.max(zb.y0)..za.y1.min(zb.y1) {
        for x in za.x0.max(zb.x0)..za.x1.min(zb.x1) {
            let (p, q) = (a.pixel(x, y), b.pixel(x, y));
            if p != q {
                return Err(format!("({x},{y}) : {p:?} au lieu de {q:?}"));
            }
            vus += usize::from(p.is_some_and(|c| c.a() > 0));
        }
    }
    Ok(vus)
}

#[test]
fn the_world_after_random_parameter_changes_is_the_world_rebuilt_from_scratch() {
    let s = scene();
    let ctx = egui::Context::default();
    let view = Area { x0: -400, y0: -300, x1: 800, y1: 600 };
    let mut calques = [Calque::neuf(), Calque::neuf(), Calque::neuf(), Calque::neuf()];
    let mut monde = Monde::new();
    settle(&mut monde, &ctx, &s.doc, &plan_de(&s, &calques), view);
    let mut r = Lcg(7);
    let mut verifies = 0;
    for step in 0..120 {
        let n = 1 + r.below(2);
        for _ in 0..n {
            let k = r.below(4) as usize;
            calques[k].tirer(&mut r);
        }
        let plan = plan_de(&s, &calques);
        // Une image d'écran, parfois plusieurs avant que le hors-feuille arrive (un réglage tiré en continu).
        monde.update(&ctx, &s.doc, &plan, view, false, &[]);
        if step % 3 != 2 {
            continue;
        }
        settle(&mut monde, &ctx, &s.doc, &plan, view);
        let mut neuf = Monde::new();
        settle(&mut neuf, &ctx, &s.doc, &plan, view);
        match memes_pixels(&monde, &neuf) {
            Ok(_) => verifies += 1,
            Err(e) => panic!("pas à pas {step} : {e}"),
        }
    }
    assert!(verifies >= 40);
    assert!(monde.rebuilds() > 10, "des réglages ont bien refait le hors-feuille ({})", monde.rebuilds());
}

#[test]
fn nothing_is_rebuilt_when_only_layers_inside_the_sheet_change() {
    let s = scene();
    let ctx = egui::Context::default();
    let view = Area { x0: -400, y0: -300, x1: 800, y1: 600 };
    let mut calques = [Calque::neuf(), Calque::neuf(), Calque::neuf(), Calque::neuf()];
    let mut monde = Monde::new();
    settle(&mut monde, &ctx, &s.doc, &plan_de(&s, &calques), view);
    let (rebuilds, composed) = (monde.rebuilds(), monde.composed());
    // Les calques qui restent dans la feuille : autre dessin (la lecture), opacité, teinte (pelure), mode, petit
    // déplacement, caché.
    let mut r = Lcg(3);
    for _ in 0..200 {
        let k = [1, 3][r.below(2) as usize];
        calques[k].tirer(&mut r);
        if calques[k].deplace.is_some_and(|(dx, dy, z)| dx.abs() > 20.0 || dy.abs() > 20.0 || z != 1.0) {
            calques[k].deplace = Some((4.0, -3.0, 1.0));
        }
        let plan = plan_de(&s, &calques);
        assert!(monde.update(&ctx, &s.doc, &plan, view, false, &[]).is_some());
        assert!(!monde.busy());
    }
    assert_eq!((monde.rebuilds(), monde.composed()), (rebuilds, composed), "rien ne s'est refait");
    // L'opacité d'un calque qui dépasse : le hors-feuille se refait, sur un autre fil, l'image d'avant restant à l'écran.
    // Un pixel rouge du calque qui dépasse (tuile (-2, 1)).
    let (px, py) = (-128..-64).flat_map(|x| (64..128).map(move |y| (x, y))).find(|&(x, y)| monde.pixel(x, y).is_some_and(|c| c.g() < 100)).expect("du rouge hors de la feuille");
    let rouge = monde.pixel(px, py).expect("dans la zone");
    calques[0].opacity = 60;
    let plan = plan_de(&s, &calques);
    monde.update(&ctx, &s.doc, &plan, view, false, &[]);
    assert_eq!(monde.rebuilds(), rebuilds + 1);
    if monde.busy() {
        assert_eq!(monde.pixel(px, py), Some(rouge), "en attendant, l'image d'avant");
    }
    settle(&mut monde, &ctx, &s.doc, &plan, view);
    // (Sur le papier blanc : le rouge plus transparent tire vers le blanc.)
    assert!(monde.pixel(px, py).is_some_and(|c| c.g() > rouge.g() + 50), "puis la nouvelle : {:?} après {rouge:?}", monde.pixel(px, py));
}

#[test]
fn a_layer_moved_past_the_sheet_edge_counts_and_one_well_inside_does_not() {
    let s = scene();
    let dans = |dx: f32, k: f32| {
        let mut c = Calque::neuf();
        c.deplace = Some((dx, 0.0, k));
        crate::monde::contributes(&s.doc, &c.element(&s.dedans[0]).expect("élément"))
    };
    assert!(!crate::monde::contributes(&s.doc, &item(s.dedans[0][0])));
    assert!(crate::monde::contributes(&s.doc, &item(s.dehors[0][0])));
    assert!(!dans(5.0, 1.0));
    assert!(dans(-600.0, 1.0), "poussé hors de la feuille");
    assert!(dans(0.0, 3.0), "agrandi jusqu'à dépasser");
    // Un dessin vide ne montre rien nulle part.
    let mut doc = Document::new(400, 300, 24);
    let vide = doc.add_cel(Cel::new(400, 300, LayerKind::Bitmap));
    assert!(!crate::monde::contributes(&doc, &item(vide)));
    // Les tuiles de bord dépassent de la feuille (400 n'est pas un multiple de 64).
    let mut doc = Document::new(400, 300, 24);
    let bord = dessin(&mut doc, &[6], &[], [1, 2, 3, 255]);
    assert!(crate::monde::contributes(&doc, &item(bord)));
    let _ = TILE;
}

// ---------------------------------------------------------------------------------------------- mesures

/// Une scène 1920 × 1080 : un décor de 5 760 × 1 080 qui dépasse des deux côtés (un travelling), un personnage animé dans
/// la feuille (un dessin par image), un premier plan qui dépasse un peu ; un second plan, autre décor.
struct Grande {
    doc: Document,
    decor: [CelId; 2],
    perso: Vec<CelId>,
    avant: CelId,
}

fn grande() -> Grande {
    let mut doc = Document::new(1920, 1080, 24);
    let across = 30;
    let fenetre: Vec<usize> = (0..across * 17).collect();
    let dehors: Vec<(i32, i32)> = (-30..60).filter(|tx| !(0..30).contains(tx)).flat_map(|tx| (0..17).map(move |ty| (tx, ty))).collect();
    let decor = [dessin(&mut doc, &fenetre, &dehors, [90, 120, 160, 255]), dessin(&mut doc, &fenetre, &dehors, [160, 120, 90, 255])];
    let perso = (0..12).map(|k| dessin(&mut doc, &[(8 + k % 3) * across + 10, 9 * across + 11, 10 * across + 12], &[], [20, 20, 20, 255])).collect();
    let avant = dessin(&mut doc, &[16 * across, 16 * across + 1], &[(-1, 16), (-2, 16), (-1, 17)], [40, 70, 30, 255]);
    Grande { doc, decor, perso, avant }
}

/// Le temps d'interface de `update` à chaque image d'écran (ms) : moyenne et pire, sur `n` images, et les hors-feuilles
/// refaits. `plan(f)` : le plan de l'image `f`.
fn mesurer<M>(n: usize, monde: &mut M, mut update: impl FnMut(&mut M, &Plan), plan: impl Fn(usize) -> Plan) -> (f64, f64) {
    let mut t = Vec::with_capacity(n);
    for f in 0..n {
        let p = plan(f);
        let d = Instant::now();
        update(monde, &p);
        t.push(d.elapsed().as_secs_f64() * 1e3);
        // (Une image d'écran toutes les 8 ms, comme SHOT sans entrée.)
        std::thread::sleep(Duration::from_millis(8));
    }
    (t.iter().sum::<f64>() / n as f64, t.iter().cloned().fold(0.0, f64::max))
}

/// Mesure (`--ignored --nocapture`) : avant (`monde_avant`, le code d'eb82649) et après, dans le même processus, sur la
/// même scène et la même vue (le décor dépasse), quatre gestes : la lecture (le personnage change de dessin à chaque
/// image), l'opacité du personnage tirée, l'opacité du décor tirée, passer d'un plan à l'autre à chaque image.
#[test]
#[ignore]
fn perf_monde() {
    let g = grande();
    let ctx = egui::Context::default();
    let view = Area { x0: -1920, y0: -200, x1: 3840, y1: 1300 };
    let base = |decor: usize, perso: usize, op_perso: u8, op_decor: u8| Plan {
        paper: Some([255, 255, 255]),
        items: vec![PlanItem { opacity: op_decor, ..item(g.decor[decor]) }, PlanItem { opacity: op_perso, ..item(g.perso[perso % g.perso.len()]) }, item(g.avant)],
    };
    let gestes: [(&str, Box<dyn Fn(usize) -> Plan>); 4] = [
        ("lecture (le personnage change de dessin)", Box::new(|f| base(0, f, 255, 255))),
        ("opacité du personnage tirée", Box::new(|f| base(0, 0, (255 - (f * 3) % 200) as u8, 255))),
        ("opacité du décor tirée", Box::new(|f| base(0, 0, 255, (255 - (f * 3) % 200) as u8))),
        ("changer de plan à chaque image", Box::new(|f| base(f % 2, f, 255, 255))),
    ];
    for serie in 0..2 {
        for (nom, plan) in &gestes {
            let mut avant = crate::monde_avant::Monde::new();
            let v = view;
            let c = ctx.clone();
            let d = &g.doc;
            // (La texture remplie d'abord : la mesure est celle du geste.)
            for _ in 0..400 {
                avant.update(&c, d, &plan(0), v, false, &[]);
                if avant.pending() == 0 {
                    break;
                }
            }
            let (am, ap) = mesurer(90, &mut avant, |m, p| {
                m.update(&c, d, p, v, false, &[]);
            }, plan);
            let mut apres = Monde::new();
            settle(&mut apres, &ctx, d, &plan(0), v);
            let r0 = apres.rebuilds();
            let (bm, bp) = mesurer(90, &mut apres, |m, p| {
                m.update(&c, d, p, v, false, &[]);
            }, plan);
            println!(
                "MONDE série {serie}, {nom} : avant {am:.2} ms en moyenne (pire {ap:.1}) ; après {bm:.2} ms (pire {bp:.1}), {} hors-feuilles refaits sur l'autre fil en 90 images",
                apres.rebuilds() - r0
            );
        }
    }
}
