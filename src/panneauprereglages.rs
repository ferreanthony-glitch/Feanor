//! Le panneau Préréglages (partie 170, Anthony, 2026-11-18 : « des préréglages d'animation de caméra à intégrer directement
//! dans la timeline »).
//!
//! Une vignette par préréglage de caméra (`camfx::Preset`) : une petite scène de storyboard vue par la caméra, qui **joue le
//! mouvement du préréglage** quand le pointeur passe dessus (le vrai mouvement, celui du générateur `camfx::Generator`, avec
//! ses réglages d'origine). On la glisse sur la piste Caméra de la timeline (charge `blocscamera::PresetDrag`), ou on clique
//! dessus pour la poser à la tête de lecture : un bloc de plus, vivant, sans clé (`blocscamera.rs`).
//!
//! Les vignettes ne s'animent qu'au survol : un panneau ouvert ne fait pas redessiner FEANOR sans cesse.
//!
//! Depuis le lot 2 (partie 171), les vignettes sont rangées par famille (`camfx::Famille`), et la petite scène a de la
//! profondeur : un travelling y fait passer le rocher vite et les montagnes à peine. Une cible y vise le visage du
//! personnage (la maison pour le zoom vers un point), comme sur la page de choix.
//!
//! Depuis le lot 3 (partie 172), la petite scène est **filmée par la vraie caméra** de l'effet (`camfx::cuire_vite` sur une
//! caméra de base, puis `camespace::image_de`) : un panoramique, une orbite, une grue ou un survol s'y voient en perspective,
//! chaque trait à sa profondeur. Les trajectoires toutes faites (`trajformes`) ont leur famille, à la fin.
//!
//! Depuis le lot 4 (partie 175), les **transitions** du montage ont leur famille (`transitions::Kind`) : la vignette joue la
//! transition entre la petite scène et la même scène de près, sur le visage ; on la glisse sur une coupe du montage (charge
//! `transitionsui::TransitionDrag`), ou on clique pour la poser sur la coupe la plus proche de la tête de lecture. Les **plans
//! types** (partie 176, `planstypes`) suivent : la vignette montre le plan tel qu'il sera posé (carton, titre, générique qui
//! monte, écran partagé, carte, inserts) ; on la glisse entre deux plans du montage (charge `planstypes::PlanTypeDrag`), ou sur
//! un plan pour le lieu et la date et le générique de début.

use std::sync::OnceLock;

use eframe::egui::{self, pos2, vec2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind};

use crate::app::App;
use crate::blocscamera::PresetDrag;
use crate::camera::{self, Camera};
use crate::camespace;
use crate::camfx::{self, Params, Preset, Vivant};
use crate::theme;
use crate::trajformes::{self, Forme, FormeDrag};
use crate::transitions::{self, Kind, Mix, Side, Zone};
use crate::transitionsui::TransitionDrag;
use crate::planstypes::{PlanType, PlanTypeDrag};

/// Largeur d'une vignette, en points.
const VIGNETTE_W: f32 = 132.0;
/// Le papier et l'encre des vignettes (un dessin de storyboard, quel que soit le thème).
const PAPIER: Color32 = Color32::from_rgb(251, 250, 246);
const ENCRE: Color32 = Color32::from_rgb(30, 30, 34);
/// Pause entre deux passages du mouvement, en secondes.
const PAUSE: f32 = 0.7;

/// Un trait de la petite scène : ses points, en part de la largeur du cadre (x vers la droite, y vers le bas, origine au centre
/// du cadre), fermé ou non, rempli d'une teinte ou non.
struct Trait {
    points: Vec<(f32, f32)>,
    ferme: bool,
    teinte: Option<Color32>,
    /// Rempli jusqu'à cette hauteur, en bandes verticales (une crête de montagnes, qui n'est pas convexe).
    jusqu_a: Option<f32>,
    /// Profondeur : la distance à la caméra, en part de celle du plan de référence, moins 1 (0 : au plan de référence ; 4 : cinq
    /// fois plus loin ; négative : plus près). Un travelling fait grandir chacun à sa vitesse.
    profondeur: f32,
}

/// Les profondeurs des traits de la scène, dans l'ordre de `scene` : les montagnes et l'horizon au loin, la route, la maison,
/// l'arbre, le personnage au plan de référence, le poteau, le rocher devant.
const PROFONDEURS: [f32; 15] = [4.0, 4.0, 0.4, 0.4, 0.8, 0.8, 0.8, 0.6, 0.6, 0.0, 0.0, 0.0, 0.0, 0.3, -0.5];

/// Le visage du personnage et la maison, là où visent les cibles des vignettes (en part de la largeur du cadre).
const VISAGE: [f32; 2] = [0.08, -0.075];
const MAISON: [f32; 2] = [-0.29, -0.01];

/// La petite scène : un sol, des montagnes, une maison, un arbre, un personnage et un rocher devant.
fn scene() -> &'static [Trait] {
    static SCENE: OnceLock<Vec<Trait>> = OnceLock::new();
    SCENE.get_or_init(|| {
        let ligne = |p: &[(f32, f32)]| Trait { points: p.to_vec(), ferme: false, teinte: None, jusqu_a: None, profondeur: 0.0 };
        let forme = |p: Vec<(f32, f32)>, t: Color32| Trait { points: p, ferme: true, teinte: Some(t), jusqu_a: None, profondeur: 0.0 };
        let rond = |cx: f32, cy: f32, rx: f32, ry: f32, n: usize| (0..n).map(|i| i as f32 / n as f32 * std::f32::consts::TAU).map(|a| (cx + a.cos() * rx, cy + a.sin() * ry)).collect::<Vec<_>>();
        let crete: Vec<(f32, f32)> = (0..=40).map(|i| -1.6 + i as f32 * 0.08).map(|x| (x, 0.055 - (0.06 + 0.03 * (x * 7.0).sin() + 0.02 * (x * 13.0).cos()))).collect();
        let mut v = vec![Trait { points: crete, ferme: false, teinte: Some(Color32::from_rgb(230, 227, 220)), jusqu_a: Some(0.06), profondeur: 0.0 }];
        v.push(ligne(&[(-1.6, 0.06), (1.6, 0.06)]));
        v.push(ligne(&[(0.02, 0.28), (-0.12, 0.12), (-0.24, 0.07)]));
        v.push(ligne(&[(0.14, 0.28), (-0.06, 0.12), (-0.2, 0.07)]));
        v.push(forme(vec![(-0.38, -0.03), (-0.2, -0.03), (-0.2, 0.06), (-0.38, 0.06)], Color32::from_rgb(244, 242, 236)));
        v.push(forme(vec![(-0.4, -0.03), (-0.29, -0.11), (-0.18, -0.03)], Color32::from_rgb(216, 212, 204)));
        v.push(ligne(&[(-0.31, 0.06), (-0.31, 0.02), (-0.27, 0.02), (-0.27, 0.06)]));
        v.push(forme(vec![(0.385, 0.06), (0.39, -0.02), (0.41, -0.02), (0.415, 0.06)], Color32::from_rgb(203, 198, 188)));
        v.push(forme(rond(0.4, -0.075, 0.065, 0.055, 18), Color32::from_rgb(224, 220, 211)));
        v.push(ligne(&[(0.07, 0.0), (0.06, 0.07)]));
        v.push(ligne(&[(0.09, 0.0), (0.1, 0.07)]));
        v.push(forme(vec![(0.055, -0.05), (0.105, -0.05), (0.1, 0.005), (0.06, 0.005)], Color32::from_rgb(185, 180, 170)));
        v.push(forme(rond(0.08, -0.075, 0.022, 0.025, 14), Color32::from_rgb(246, 244, 239)));
        v.push(ligne(&[(0.6, 0.06), (0.6, -0.1), (0.63, -0.1)]));
        v.push(forme(rond(-0.33, 0.24, 0.12, 0.08, 18), Color32::from_rgb(171, 166, 157)));
        for (t, z) in v.iter_mut().zip(PROFONDEURS) {
            t.profondeur = z;
        }
        v
    })
}

/// Les réglages que joue la vignette de `preset` : ceux d'origine, la cible sur le visage (ou la maison).
fn reglages_de_vignette(preset: Preset, fps: u32) -> Params {
    let p = Params::new(preset, fps);
    match (preset.poignee(), preset) {
        (_, Preset::ZoomPoint) => Params { vers: MAISON, ..p },
        (crate::camfx::Poignee::Cible, _) => Params { vers: VISAGE, ..p },
        _ => p,
    }
}

/// Le cadre des vignettes : une image de 1600 × 900 pixels, filmée d'une feuille de la même taille (partie 172 : la petite
/// scène est vue par une vraie caméra, celle que l'effet donne, comme dans le plan).
const CADRE: [usize; 2] = [1600, 900];

/// La caméra des vignettes au repos : au centre de la feuille, de face.
fn camera_de_base() -> Camera {
    Camera::centered(CADRE[0], CADRE[1])
}

/// Le point de l'espace (`camespace`, pixels de la feuille) du point (`x`, `y`) d'un trait à la profondeur `z` : posé comme
/// un calque en profondeur, que la caméra de base voit tel qu'il est dessiné.
fn point_de_la_scene(x: f32, y: f32, z: f32) -> camespace::V3 {
    let (w, h) = (CADRE[0] as f32, CADRE[1] as f32);
    let d0 = camera::ref_distance(CADRE);
    [w / 2.0 + x * w * (1.0 + z), -(h / 2.0) - y * w * (1.0 + z), -d0 * z]
}

/// La caméra de la vignette de `preset` à l'image `rel` de son mouvement : celle que montrerait l'effet vivant posé sur un plan
/// à la caméra de base (`camfx::cuire_vite`).
fn camera_de_vignette(preset: Preset, fps: u32, rel: usize) -> Camera {
    let params = reglages_de_vignette(preset, fps);
    let base = camera_de_base();
    let v = Vivant { params, start: 0 };
    let keys = camfx::cuire_vite(&v, fps, CADRE, params.duration + 1, &[], &|_| base).unwrap_or_default();
    camera::camera_at(&keys, rel, base)
}

/// La caméra de la vignette de `forme` à l'avancement `t` (0 à 1) de son chemin : elle part de la caméra de base et garde le
/// visage du personnage à sa place, comme une trajectoire toute faite garde son sujet (`trajformes`).
fn camera_de_forme(forme: Forme, t: f32) -> Camera {
    let base = camera_de_base();
    let d0 = camera::ref_distance(CADRE);
    let (fw, fh) = (CADRE[0] as f32, CADRE[1] as f32);
    let (u, v) = (fw / 2.0 + VISAGE[0] * fw, fh / 2.0 + VISAGE[1] * fw);
    let sujet = camespace::point_vu(&base, CADRE, u, v);
    let oeil = camespace::oeil(&base, d0);
    let (dx, dz) = (sujet[0] - oeil[0], sujet[2] - oeil[2]);
    let e = dx.hypot(dz).max(1.0);
    let devant = [dx / e, 0.0, dz / e];
    let droite = [-devant[2], 0.0, devant[0]];
    let p = forme.point(camera::ease(t, 1.0, 0.5, 0.5));
    let d = [0, 1, 2].map(|i| (p[0] * droite[i] + p[1] * devant[i]) * e + if i == 1 { p[2] * e } else { 0.0 });
    let portee = camespace::deplacer(&base, CADRE, d);
    camespace::viser(&portee, CADRE, sujet, u, v).unwrap_or(portee)
}

/// La petite scène filmée par `cam`, peinte dans `r`.
fn peindre_scene(painter: &egui::Painter, r: Rect, cam: &Camera) {
    peindre_scene_a(painter, r, cam, 1.0, true);
}

/// La petite scène filmée par `cam`, peinte dans `r` à l'opacité `alpha`, avec le bord du cadre ou non (partie 175 : une
/// transition en peint deux, glissées, coupées, l'une sur l'autre).
fn peindre_scene_a(painter: &egui::Painter, r: Rect, cam: &Camera, alpha: f32, bord: bool) {
    let painter = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    let opaque = |c: Color32| if alpha >= 1.0 { c } else { c.gamma_multiply(alpha.max(0.0)) };
    painter.rect_filled(r, 2.0, opaque(PAPIER));
    let w = r.width();
    let (cx, cy) = (CADRE[0] as f32 / 2.0, CADRE[1] as f32 / 2.0);
    let echelle = w / CADRE[0] as f32;
    // (Un point derrière la caméra, ou si près d'elle qu'il partirait à l'infini, n'est pas dessiné.)
    let ecran = |x: f32, y: f32, z: f32| -> Option<Pos2> {
        let [u, v] = camespace::image_de(cam, CADRE, point_de_la_scene(x, y, z))?;
        let q = pos2(r.center().x + (u - cx) * echelle, r.center().y + (v - cy) * echelle);
        (q.x.abs() < 1e5 && q.y.abs() < 1e5).then_some(q)
    };
    let stroke = Stroke::new((w / 110.0).max(1.0), opaque(ENCRE));
    for t in scene() {
        let z = t.profondeur;
        let pts: Vec<Option<Pos2>> = t.points.iter().map(|&(x, y)| ecran(x, y, z)).collect();
        match (t.teinte, t.jusqu_a) {
            (Some(c), Some(bas)) => {
                for w2 in t.points.windows(2) {
                    let (a, b) = (w2[0], w2[1]);
                    if let (Some(p1), Some(p2), Some(p3), Some(p4)) = (ecran(a.0, a.1, z), ecran(b.0, b.1, z), ecran(b.0, bas, z), ecran(a.0, bas, z)) {
                        painter.add(egui::Shape::convex_polygon(vec![p1, p2, p3, p4], opaque(c), Stroke::NONE));
                    }
                }
            }
            (Some(c), None) => {
                if let Some(tous) = pts.iter().copied().collect::<Option<Vec<Pos2>>>() {
                    painter.add(egui::Shape::convex_polygon(tous, opaque(c), Stroke::NONE));
                }
            }
            _ => {}
        }
        if t.ferme && pts.iter().all(|p| p.is_some()) {
            painter.add(egui::Shape::closed_line(pts.iter().flatten().copied().collect(), stroke));
            continue;
        }
        // Le trait, en morceaux là où il passe derrière la caméra.
        let mut morceau: Vec<Pos2> = Vec::new();
        let boucle = if t.ferme { pts.first() } else { None };
        for p in pts.iter().chain(boucle) {
            match p {
                Some(q) => morceau.push(*q),
                None => {
                    if morceau.len() >= 2 {
                        painter.add(egui::Shape::line(std::mem::take(&mut morceau), stroke));
                    }
                    morceau.clear();
                }
            }
        }
        if morceau.len() >= 2 {
            painter.add(egui::Shape::line(morceau, stroke));
        }
    }
    // Le bord du cadre, à la couleur de la caméra.
    if bord {
        painter.rect_stroke(r, 2.0, Stroke::new(2.0_f32, theme::camera()), StrokeKind::Inside);
    }
}

/// Le plan entrant des vignettes de transition (partie 175) : la même scène, de près, sur le visage du personnage.
fn camera_plan_b() -> Camera {
    let base = camera_de_base();
    let w = CADRE[0] as f32;
    Camera { x: base.x + VISAGE[0] * w, y: base.y + VISAGE[1] * w + 0.03 * w, zoom: 2.6, ..base }
}

/// Les images de la vignette d'une transition de sorte `kind` : le plan sortant seul, la transition, le plan entrant seul.
const AVANT: usize = 8;
fn images_de_transition(kind: Kind) -> usize {
    if kind == Kind::Flash {
        4
    } else {
        14
    }
}

/// La vignette d'une transition à l'image `rel` : la petite scène, puis la même de près, mêlées comme le montage les mêle
/// (`transitions::Zone::mix`, les mêmes avancées, glissements, flous et objet que dans le film).
fn peindre_transition(painter: &egui::Painter, r: Rect, kind: Kind, rel: usize) {
    let n = images_de_transition(kind);
    let cut = AVANT + n / 2;
    let zone = Zone {
        id: 0,
        kind,
        cut,
        start: AVANT,
        end: AVANT + n,
        out: Some(Side { panel: 0, start: 0, len: cut }),
        inn: Some(Side { panel: 1, start: cut, len: n + AVANT }),
        align: transitions::Align::Centre,
        couleur: transitions::COULEUR_DEFAUT,
    };
    let mix = zone.mix(rel).unwrap_or(Mix { a: (rel < AVANT).then_some((0, 0)), b: (rel >= AVANT).then_some((1, 0)), m: 1.0, ..Mix::default() });
    let (cam_a, cam_b) = (camera_de_base(), camera_plan_b());
    let w = r.width();
    let coupe = |x0: f32, x1: f32| painter.with_clip_rect(Rect::from_x_y_ranges(x0.max(r.left())..=x1.min(r.right()), r.y_range()).intersect(painter.clip_rect()));
    painter.rect_filled(r, 2.0, Color32::BLACK);
    if mix.glisse {
        let (da, db) = (mix.shift_a * w, mix.shift_b * w);
        let pa = coupe(r.left(), r.right() + da);
        let pb = coupe(r.left() + db, r.right());
        peindre_scene_a(&pa, r.translate(vec2(da, 0.0)), &cam_a, 1.0, false);
        peindre_scene_a(&pb, r.translate(vec2(db, 0.0)), &cam_b, 1.0, false);
        // Le flou du raccord filé : des copies décalées, pâles, de part et d'autre.
        if mix.blur > 0.0 {
            for f in [-1.0, -0.5, 0.5, 1.0] {
                let d = f * mix.blur * w;
                peindre_scene_a(&pa, r.translate(vec2(da + d, 0.0)), &cam_a, 0.3, false);
                peindre_scene_a(&pb, r.translate(vec2(db + d, 0.0)), &cam_b, 0.3, false);
            }
        }
    } else if let Some(c) = mix.objet {
        let x = r.left() + c * w;
        peindre_scene_a(&coupe(r.left(), x), r, &cam_a, 1.0, false);
        peindre_scene_a(&coupe(x, r.right()), r, &cam_b, 1.0, false);
        let o = transitions::OBJET_COULEUR;
        let coins: Vec<Pos2> = transitions::objet_coins(c).iter().map(|p| pos2(r.left() + p[0] * w, r.top() + p[1] * r.height())).collect();
        painter.with_clip_rect(r.intersect(painter.clip_rect())).add(egui::Shape::convex_polygon(coins, Color32::from_rgb(o[0], o[1], o[2]), Stroke::NONE));
    } else if mix.wipe && mix.a.is_some() && mix.b.is_some() {
        let x = r.left() + mix.m.clamp(0.0, 1.0) * w;
        peindre_scene_a(&coupe(r.left(), x), r, &cam_b, 1.0, false);
        peindre_scene_a(&coupe(x, r.right()), r, &cam_a, 1.0, false);
    } else {
        if mix.a.is_some() {
            peindre_scene_a(painter, r, &cam_a, 1.0, false);
        }
        if mix.b.is_some() {
            peindre_scene_a(painter, r, &cam_b, if mix.a.is_some() { mix.m } else { 1.0 }, false);
        }
    }
    if mix.k > 0.0 {
        let c = mix.color;
        painter.with_clip_rect(r.intersect(painter.clip_rect())).rect_filled(r, 2.0, Color32::from_rgb(c[0], c[1], c[2]).gamma_multiply(mix.k.clamp(0.0, 1.0)));
    }
    painter.rect_stroke(r, 2.0, Stroke::new(2.0_f32, theme::camera()), StrokeKind::Inside);
}

/// La vignette du plan type `t` à l'image `rel` (sur `n`) : le plan tel qu'il sera posé, en petit (partie 176).
fn peindre_plan_type(painter: &egui::Painter, r: Rect, t: PlanType, rel: usize, n: usize) {
    let painter = painter.with_clip_rect(r.intersect(painter.clip_rect()));
    let (w, h) = (r.width(), r.height());
    let at = |x: f32, y: f32| pos2(r.left() + x * w, r.top() + y * h);
    let u = rel as f32 / n.max(1) as f32;
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let fondu = |a: f32| -> f32 { (u / a).min((1.0 - u) / a).clamp(0.0, 1.0) };
    let ecrire = |t: &str, taille: f32, c: Color32, p: Pos2, ancre: egui::Align2| {
        painter.text(p, ancre, t, FontId::proportional(taille), c);
    };
    let noir = rgb([8, 8, 10]);
    let blanc = rgb([250, 250, 248]);
    match t {
        PlanType::CouleurUnie => {
            painter.rect_filled(r, 2.0, rgb(crate::couleurunie::DEFAUT));
        }
        PlanType::CartonNoir => {
            painter.rect_filled(r, 2.0, noir);
        }
        PlanType::CartonBlanc => {
            painter.rect_filled(r, 2.0, blanc);
        }
        PlanType::Titre => {
            painter.rect_filled(r, 2.0, noir);
            ecrire("Titre du film", h * 0.16, blanc.gamma_multiply(fondu(0.15)), r.center(), egui::Align2::CENTER_CENTER);
        }
        PlanType::Intertitre => {
            painter.rect_filled(r, 2.0, noir);
            ecrire("Trois jours plus tard…", h * 0.1, blanc.gamma_multiply(fondu(0.1)), r.center(), egui::Align2::CENTER_CENTER);
        }
        PlanType::LieuEtDate => {
            peindre_scene_a(&painter, r, &camera_de_base(), 1.0, false);
            let mot = "Paris, 1962";
            let k = (u * 2.5 * mot.chars().count() as f32).ceil() as usize;
            let visible: String = mot.chars().take(k.max(1)).collect();
            painter.rect_filled(Rect::from_min_max(at(0.0, 0.78), at(0.55, 0.94)), 0.0, Color32::from_black_alpha(90));
            ecrire(&visible, h * 0.1, blanc, at(0.05, 0.9), egui::Align2::LEFT_BOTTOM);
        }
        PlanType::GeneriqueDebut => {
            peindre_scene_a(&painter, r, &camera_de_base(), 1.0, false);
            let noms = [("Un film de", "Prénom Nom"), ("Avec", "Prénom Nom"), ("Musique", "Prénom Nom")];
            let k = ((u * 3.0) as usize).min(2);
            let local = u * 3.0 - k as f32;
            let a = (local / 0.2).min((1.0 - local) / 0.2).clamp(0.0, 1.0);
            painter.rect_filled(Rect::from_min_max(at(0.2, 0.66), at(0.8, 0.92)), 3.0, Color32::from_black_alpha((110.0 * a) as u8));
            ecrire(noms[k].0, h * 0.07, blanc.gamma_multiply(a), at(0.5, 0.72), egui::Align2::CENTER_CENTER);
            ecrire(noms[k].1, h * 0.11, blanc.gamma_multiply(a), at(0.5, 0.84), egui::Align2::CENTER_CENTER);
        }
        PlanType::GeneriqueFin => {
            painter.rect_filled(r, 2.0, noir);
            let lignes = ["Générique", "", "Réalisation", "Prénom Nom", "", "Storyboard", "Prénom Nom", "", "Musique", "Prénom Nom"];
            let pas = h * 0.12;
            let hauteur = lignes.len() as f32 * pas;
            let y0 = r.bottom() - u * (h + hauteur);
            for (k, l) in lignes.iter().enumerate() {
                let gris = k % 3 == 2;
                ecrire(l, h * if k == 0 { 0.11 } else if gris { 0.07 } else { 0.09 }, if gris { rgb([165, 165, 170]) } else { blanc }, pos2(r.center().x, y0 + k as f32 * pas), egui::Align2::CENTER_CENTER);
            }
        }
        PlanType::Logo => {
            painter.rect_filled(r, 2.0, noir);
            let a = fondu(0.2);
            painter.circle_stroke(r.center(), h * 0.22, Stroke::new(h * 0.04, theme::camera().gamma_multiply(a)));
            ecrire("LOGO", h * 0.11, blanc.gamma_multiply(a), r.center(), egui::Align2::CENTER_CENTER);
        }
        PlanType::Partage2 | PlanType::Partage3 | PlanType::Partage4 => {
            painter.rect_filled(r, 2.0, blanc);
            let e = (w * 0.02).max(1.5);
            let colonne = |x: f32| {
                painter.rect_filled(Rect::from_center_size(at(x, 0.5), vec2(e, h)), 0.0, noir);
            };
            match t {
                PlanType::Partage2 => colonne(0.5),
                PlanType::Partage3 => {
                    colonne(1.0 / 3.0);
                    colonne(2.0 / 3.0);
                }
                _ => {
                    colonne(0.5);
                    painter.rect_filled(Rect::from_center_size(r.center(), vec2(w, e)), 0.0, noir);
                }
            }
            let cases: &[(f32, f32)] = match t {
                PlanType::Partage2 => &[(0.25, 0.5), (0.75, 0.5)],
                PlanType::Partage3 => &[(1.0 / 6.0, 0.5), (0.5, 0.5), (5.0 / 6.0, 0.5)],
                _ => &[(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)],
            };
            for (k, (x, y)) in cases.iter().enumerate() {
                ecrire(&format!("{}", k + 1), h * 0.14, rgb([170, 168, 160]), at(*x, *y), egui::Align2::CENTER_CENTER);
            }
        }
        PlanType::Precedemment | PlanType::ASuivre => {
            peindre_scene_a(&painter, r, &if (u * 3.0) as usize % 2 == 0 { camera_de_base() } else { camera_plan_b() }, 1.0, false);
            painter.rect_filled(Rect::from_min_max(at(0.0, 0.06), at(0.55, 0.22)), 0.0, noir);
            ecrire(if t == PlanType::Precedemment { "PRÉCÉDEMMENT…" } else { "À SUIVRE…" }, h * 0.09, blanc, at(0.04, 0.14), egui::Align2::LEFT_CENTER);
        }
        PlanType::Carte => {
            painter.rect_filled(r, 2.0, rgb([232, 220, 192]));
            let brun = rgb([120, 92, 60]);
            let rivage: Vec<Pos2> = (0..=24).map(|k| k as f32 / 24.0).map(|v| at(0.08 + 0.3 * v + 0.04 * (v * 9.0).sin(), 0.05 + 0.9 * v)).collect();
            painter.add(egui::Shape::line(rivage, Stroke::new(1.5_f32, brun)));
            let (a, b, m) = (at(0.3, 0.68), at(0.76, 0.32), at(0.45, 0.25));
            let courbe = |v: f32| {
                let q = 1.0 - v;
                pos2(q * q * a.x + 2.0 * q * v * m.x + v * v * b.x, q * q * a.y + 2.0 * q * v * m.y + v * v * b.y)
            };
            let jusqu_a = (u * 1.5).min(1.0);
            for k in 0..18 {
                let (v0, v1) = (k as f32 / 18.0, ((k as f32 + 0.55) / 18.0).min(jusqu_a));
                if v0 >= jusqu_a {
                    break;
                }
                painter.line_segment([courbe(v0), courbe(v1)], Stroke::new(2.0_f32, rgb([150, 40, 30])));
            }
            painter.circle_filled(a, h * 0.03, rgb([150, 40, 30]));
            painter.circle_filled(b, h * 0.03, rgb([150, 40, 30]));
        }
        PlanType::Telephone => {
            painter.rect_filled(r, 2.0, rgb([196, 204, 214]));
            let corps = Rect::from_center_size(r.center(), vec2(h * 0.46, h * 0.9));
            painter.rect_filled(corps, h * 0.06, rgb([28, 28, 32]));
            let ecran = corps.shrink2(vec2(h * 0.025, h * 0.06));
            painter.rect_filled(ecran, h * 0.02, rgb([246, 246, 248]));
            for k in 0..3 {
                if u * 4.0 < 0.5 + k as f32 {
                    break;
                }
                let moi = k == 1;
                let y = ecran.top() + (0.12 + 0.13 * k as f32) * h;
                let bw = ecran.width() * 0.6;
                let x0 = if moi { ecran.right() - bw - h * 0.02 } else { ecran.left() + h * 0.02 };
                painter.rect_filled(Rect::from_min_size(pos2(x0, y - h * 0.035), vec2(bw, h * 0.07)), h * 0.03, if moi { rgb([52, 120, 246]) } else { rgb([226, 226, 232]) });
            }
        }
        PlanType::Ordinateur => {
            painter.rect_filled(r, 2.0, rgb([58, 62, 70]));
            let moniteur = Rect::from_center_size(at(0.5, 0.45), vec2(w * 0.62, h * 0.66));
            painter.rect_filled(Rect::from_center_size(pos2(moniteur.center().x, moniteur.bottom() + h * 0.08), vec2(w * 0.12, h * 0.14)), 0.0, rgb([40, 40, 44]));
            painter.rect_filled(moniteur, h * 0.02, rgb([24, 24, 28]));
            let ecran = moniteur.shrink(h * 0.025);
            painter.rect_filled(ecran, 0.0, rgb([70, 110, 160]));
            let fenetre = ecran.shrink2(vec2(w * 0.06, h * 0.06));
            painter.rect_filled(fenetre, 0.0, rgb([244, 244, 246]));
            painter.rect_filled(Rect::from_min_max(fenetre.min, pos2(fenetre.right(), fenetre.top() + h * 0.06)), 0.0, rgb([210, 212, 218]));
            for k in 0..4 {
                let y = fenetre.top() + (0.12 + 0.07 * k as f32) * h;
                painter.rect_filled(Rect::from_min_size(pos2(fenetre.left() + w * 0.03, y), vec2(fenetre.width() * if k == 3 { 0.4 } else { 0.8 }, h * 0.02)), 0.0, rgb([190, 192, 198]));
            }
        }
        PlanType::Journal => {
            painter.rect_filled(r, 2.0, rgb([66, 66, 70]));
            let page = Rect::from_center_size(at(0.5, 0.52), vec2(h * 0.62, h * 0.94));
            painter.rect_filled(page, 0.0, rgb([240, 236, 224]));
            let encre = rgb([30, 28, 26]);
            ecrire("LE QUOTIDIEN", h * 0.07, encre, pos2(page.center().x, page.top() + h * 0.08), egui::Align2::CENTER_CENTER);
            painter.rect_filled(Rect::from_min_max(pos2(page.left() + h * 0.04, page.top() + h * 0.14), pos2(page.right() - h * 0.04, page.top() + h * 0.15)), 0.0, encre);
            ecrire("LA UNE", h * 0.08, encre, pos2(page.center().x, page.top() + h * 0.22), egui::Align2::CENTER_CENTER);
            let mut y = page.top() + h * 0.32;
            while y < page.bottom() - h * 0.05 {
                painter.rect_filled(Rect::from_min_max(pos2(page.left() + h * 0.04, y), pos2(page.right() - h * 0.04, y + h * 0.012)), 0.0, rgb([150, 146, 138]));
                y += h * 0.035;
            }
        }
    }
    painter.rect_stroke(r, 2.0, Stroke::new(2.0_f32, theme::camera()), StrokeKind::Inside);
}

/// Une vignette du panneau : un préréglage d'effet de caméra, ou une trajectoire toute faite (partie 172).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Tuile {
    Effet(Preset),
    Forme(Forme),
    /// Une transition du montage (partie 175).
    Transition(Kind),
    /// Un plan type (partie 176).
    PlanType(PlanType),
}

impl Tuile {
    fn label(self) -> &'static str {
        match self {
            Tuile::Effet(p) => p.label(),
            Tuile::Forme(f) => f.label(),
            Tuile::Transition(k) => k.name(),
            Tuile::PlanType(t) => t.label(),
        }
    }

    fn note(self) -> String {
        match self {
            Tuile::Effet(p) => format!("{} Glisse-le sur la piste Caméra de la timeline, ou clique pour le poser à la tête de lecture.", p.note()),
            Tuile::Forme(f) => format!(
                "{} Glisse-la sur la piste Caméra de la timeline, ou clique pour la poser à la tête de lecture : elle devient le chemin de la caméra du plan, que l'outil Trajectoire retouche dans la vue Espace 3D (poignées, flèches x, y, z).",
                f.note()
            ),
            Tuile::Transition(k) => format!(
                "{} Glisse-la sur une coupe entre deux plans du montage (ou au début, à la fin du film), ou clique pour la poser sur la coupe la plus proche de la tête de lecture.",
                k.description()
            ),
            Tuile::PlanType(t) if t.sur_un_plan() => format!("{} Glisse-le sur un plan du montage, ou clique pour le poser sur le plan courant.", t.note()),
            Tuile::PlanType(t) => format!("{} Glisse-le entre deux plans du montage, ou clique pour le poser juste après le plan courant.", t.note()),
        }
    }

    /// La durée de son mouvement, en images.
    fn duree(self, fps: u32) -> usize {
        match self {
            Tuile::Effet(p) => Params::new(p, fps).duration.max(1),
            Tuile::Forme(_) => ((trajformes::SECONDES * fps.max(1) as f32).round() as usize).max(1),
            Tuile::Transition(k) => 2 * AVANT + images_de_transition(k),
            Tuile::PlanType(t) => ((t.secondes().min(5.0) * fps.max(1) as f32).round() as usize).max(1),
        }
    }

    /// Peinte dans `r` à l'image `rel` de son mouvement.
    fn peindre(self, painter: &egui::Painter, r: Rect, fps: u32, rel: usize) {
        let cam = match self {
            Tuile::Effet(p) => camera_de_vignette(p, fps, rel),
            Tuile::Forme(f) => camera_de_forme(f, rel as f32 / self.duree(fps) as f32),
            Tuile::Transition(k) => return peindre_transition(painter, r, k, rel),
            Tuile::PlanType(t) => return peindre_plan_type(painter, r, t, rel, self.duree(fps)),
        };
        peindre_scene(painter, r, &cam);
    }
}

/// Les vignettes de `tuiles`, côte à côte : chacune joue son mouvement au survol, se glisse vers la timeline (charge
/// `PresetDrag` ou `FormeDrag`) ; rend celle qui est cliquée.
fn vignettes(ui: &mut egui::Ui, tuiles: &[Tuile], fps: u32, maintenant: f32) -> Option<Tuile> {
    let mut pose = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 10.0);
        for &tuile in tuiles {
            let h = VIGNETTE_W * 9.0 / 16.0;
            // (Sous l'image, le nom sur deux lignes au plus : « Caméra à l'épaule (found footage) » ne déborde pas sur la voisine.)
            let (rect, resp) = ui.allocate_exact_size(vec2(VIGNETTE_W, h + 30.0), Sense::click_and_drag());
            let img = Rect::from_min_size(rect.min, vec2(VIGNETTE_W, h));
            let id = resp.id.with("survol");
            let duree = tuile.duree(fps);
            // Au survol, le mouvement joue en boucle depuis le moment où le pointeur est arrivé.
            let rel = if resp.hovered() && !resp.dragged() {
                let debut = ui.ctx().data_mut(|d| *d.get_temp_mut_or_insert_with(id, || maintenant));
                let secondes = duree as f32 / fps.max(1) as f32;
                ui.ctx().request_repaint();
                let t = (maintenant - debut) % (secondes + PAUSE);
                ((t * fps as f32).floor().max(0.0) as usize).min(duree)
            } else {
                ui.ctx().data_mut(|d| d.remove::<f32>(id));
                // (À l'arrêt : un instant parlant, aux deux tiers du mouvement ; une transition, en son milieu.)
                match tuile {
                    Tuile::Transition(k) => AVANT + images_de_transition(k) / 2,
                    Tuile::PlanType(_) => duree / 2,
                    _ => duree * 2 / 3,
                }
            };
            if ui.is_rect_visible(rect) {
                tuile.peindre(ui.painter(), img, fps, rel);
                if resp.hovered() {
                    ui.painter().rect_stroke(img.expand(1.5), 3.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Outside);
                }
                let nom = ui.painter().layout(tuile.label().to_string(), FontId::proportional(11.0), ui.visuals().text_color(), VIGNETTE_W - 4.0);
                ui.painter().with_clip_rect(rect).galley(pos2(rect.left() + 2.0, img.bottom() + 3.0), nom, ui.visuals().text_color());
            }
            if resp.drag_started() {
                match tuile {
                    Tuile::Effet(preset) => egui::DragAndDrop::set_payload(ui.ctx(), PresetDrag { preset }),
                    Tuile::Forme(forme) => egui::DragAndDrop::set_payload(ui.ctx(), FormeDrag { forme }),
                    Tuile::Transition(kind) => egui::DragAndDrop::set_payload(ui.ctx(), TransitionDrag { kind }),
                    Tuile::PlanType(plan) => egui::DragAndDrop::set_payload(ui.ctx(), PlanTypeDrag { plan }),
                }
            }
            let resp = crate::bulles::bulle(resp, tuile.label(), &tuile.note(), "");
            if resp.clicked() {
                pose = Some(tuile);
            }
        }
    });
    pose
}

/// Les deux onglets du panneau (partie 175) : la caméra du plan, et le montage (transitions).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Onglet {
    Camera,
    Montage,
}

fn id_onglet() -> egui::Id {
    egui::Id::new("prereglages_onglet")
}

/// L'onglet ouvert du panneau (gardé d'une séance à l'autre avec la mise en page).
fn onglet(ctx: &egui::Context) -> Onglet {
    match ctx.data_mut(|d| *d.get_persisted_mut_or_default::<u8>(id_onglet())) {
        1 => Onglet::Montage,
        _ => Onglet::Camera,
    }
}

/// Ouvre l'onglet `o` du panneau.
pub(crate) fn choisir_onglet(ctx: &egui::Context, o: Onglet) {
    ctx.data_mut(|d| d.insert_persisted(id_onglet(), if o == Onglet::Montage { 1u8 } else { 0u8 }));
}

impl App {
    /// Le panneau Préréglages.
    pub(crate) fn prereglages_panel(&mut self, ui: &mut egui::Ui) {
        // Les onglets (partie 175) : le panneau est devenu long, la caméra d'un côté, le montage de l'autre.
        let ouvert = onglet(ui.ctx());
        ui.horizontal(|ui| {
            for (o, nom, aide) in [
                (Onglet::Camera, "Caméra", "Les mouvements de caméra, à glisser sur la piste Caméra de la timeline, et les trajectoires toutes faites."),
                (Onglet::Montage, "Montage", "Les transitions, à glisser sur une coupe entre deux plans du montage, et les plans types, à glisser entre deux plans."),
            ] {
                let resp = ui.selectable_label(ouvert == o, nom);
                if crate::bulles::bulle(resp, nom, aide, "").clicked() {
                    choisir_onglet(ui.ctx(), o);
                }
            }
        });
        let fps = self.doc.fps;
        let maintenant = ui.input(|i| i.time) as f32;
        let mut pose = None;
        if ouvert == Onglet::Camera {
            ui.label(egui::RichText::new("Glisse un préréglage sur la piste Caméra de la timeline, ou clique dessus pour le poser à la tête de lecture.").small().weak());
            // Une famille après l'autre (partie 171), sous le titre de la caméra.
            for famille in crate::camfx::Famille::ALL {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("CAMÉRA · {}", famille.label().to_uppercase())).small().strong());
                let tuiles: Vec<Tuile> = famille.presets().map(Tuile::Effet).collect();
                pose = vignettes(ui, &tuiles, fps, maintenant).or(pose);
            }
            // Les trajectoires toutes faites (C17, partie 172) : le chemin de la caméra du plan, à retoucher dans la vue Espace 3D.
            ui.add_space(6.0);
            ui.label(egui::RichText::new("CAMÉRA · TRAJECTOIRES").small().strong());
            let formes: Vec<Tuile> = Forme::ALL.into_iter().map(Tuile::Forme).collect();
            pose = vignettes(ui, &formes, fps, maintenant).or(pose);
        } else {
            ui.label(egui::RichText::new("Glisse une transition sur une coupe du montage, un plan type entre deux plans ; ou clique dessus pour le poser à la tête de lecture.").small().weak());
            // Les transitions du montage (partie 175), sur les coupes.
            ui.add_space(6.0);
            ui.label(egui::RichText::new("MONTAGE · TRANSITIONS").small().strong());
            let transitions: Vec<Tuile> = Kind::ALL.into_iter().map(Tuile::Transition).collect();
            pose = vignettes(ui, &transitions, fps, maintenant).or(pose);
            // Les plans types (partie 176), entre les plans ; le lieu et la date et le générique de début, sur un plan.
            ui.add_space(6.0);
            ui.label(egui::RichText::new("MONTAGE · PLANS TYPES").small().strong());
            let plans: Vec<Tuile> = PlanType::ALL.into_iter().map(Tuile::PlanType).collect();
            pose = vignettes(ui, &plans, fps, maintenant).or(pose);
        }
        match pose {
            Some(Tuile::Effet(pr)) => self.ajouter_effet_camera_a_la_tete(pr),
            Some(Tuile::Forme(f)) => self.poser_trajectoire_toute_faite_a_la_tete(f),
            Some(Tuile::Transition(k)) => self.poser_transition_a_la_tete(k),
            Some(Tuile::PlanType(t)) => self.poser_plan_type_a_la_tete(t),
            None => {}
        }
        // Le préréglage glissé suit le pointeur.
        let glisse = egui::DragAndDrop::payload::<PresetDrag>(ui.ctx())
            .map(|d| (d.preset.label(), egui_phosphor::regular::VIDEO_CAMERA))
            .or_else(|| egui::DragAndDrop::payload::<FormeDrag>(ui.ctx()).map(|d| (d.forme.label(), egui_phosphor::regular::VIDEO_CAMERA)))
            .or_else(|| egui::DragAndDrop::payload::<TransitionDrag>(ui.ctx()).map(|d| (d.kind.name(), egui_phosphor::regular::FILM_STRIP)))
            .or_else(|| egui::DragAndDrop::payload::<PlanTypeDrag>(ui.ctx()).map(|d| (d.plan.label(), egui_phosphor::regular::FILM_SLATE)));
        if let Some((nom, icone)) = glisse {
            if let Some(p) = ui.ctx().pointer_latest_pos() {
                egui::Area::new(egui::Id::new("prereglage_glisse")).order(egui::Order::Tooltip).fixed_pos(p + vec2(14.0, 10.0)).interactable(false).show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{icone} {nom}"));
                    });
                });
            }
        }
    }
}
