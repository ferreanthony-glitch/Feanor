//! Partie 164 : la Couleur, les Effets et la ligne du calque, plus visuels (Anthony : « des panneaux moins dans les chiffres, plus
//! visuels, plus facilement compréhensibles, qui donnent envie d'être manipulés » ; sur la maquette : « si l'on doit interagir avec
//! les graphiques pour modifier directement la scène c'est super »). Ce module tient ce qui ne dépend pas de l'application : le titre
//! du repli « Précis » (une seule apparence partout, parties 162, 163 et 164), les taches de couleur « avant · maintenant », les
//! vignettes des effets (ce qu'ils produisent, peint au `Painter`), et la « force » d'un effet (le réglage que tire la barre de
//! l'avant | après). Les méthodes qui écrivent dans le projet sont dans `colorui.rs` et `fxui.rs`. (Les pastilles des modes de fusion
//! et l'opacité sur la miniature de la ligne du calque, partie 164.3, sont retirées à la demande d'Anthony, partie 165.3 : la ligne a
//! retrouvé « 100 % » et « Normal ▾ ».)
//!
//! Aucune image : tout est peint. Les vignettes des effets sont des schémas (un trait sur un papier). L'avant | après, lui, montre le
//! vrai calque depuis la partie 165 (`avantapres165.rs`, calculé en petit au fil de fond du cache des effets) ; le schéma reste son
//! repli (calque vide, aperçu pas encore arrivé).

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui};

use crate::fx::EffectKind;
use crate::theme;

// ------------------------------------------------------------------ 164.4 : le titre du repli « Précis »

/// Le titre d'un repli « Précis » : un petit triangle peint (▸ fermé, ▾ ouvert ; peint plutôt qu'écrit, pour qu'aucune police ne le
/// change en carré vide), puis `texte`, en petit et en gris doux. Il se clique ; sa bulle est posée par l'appelant. Le même pour les
/// panneaux des parties 162, 163 et 164.
pub(crate) fn titre_precis(ui: &mut Ui, ouvert: bool, texte: &str) -> Response {
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(texte.to_string(), font, theme::texte_doux());
    let cote = 8.0;
    let taille = vec2(cote + 5.0 + galley.size().x, galley.size().y.max(cote) + 2.0);
    let (rect, resp) = ui.allocate_exact_size(taille, Sense::click());
    if ui.is_rect_visible(rect) {
        let encre = if resp.hovered() { theme::texte() } else { theme::texte_doux() };
        let c = pos2(rect.left() + cote * 0.5, rect.center().y);
        let h = cote * 0.5;
        let pts = if ouvert {
            vec![pos2(c.x - h, c.y - h * 0.55), pos2(c.x + h, c.y - h * 0.55), pos2(c.x, c.y + h * 0.65)]
        } else {
            vec![pos2(c.x - h * 0.55, c.y - h), pos2(c.x + h * 0.65, c.y), pos2(c.x - h * 0.55, c.y + h)]
        };
        ui.painter().add(Shape::convex_polygon(pts, encre, Stroke::NONE));
        ui.painter().galley(pos2(rect.left() + cote + 5.0, rect.center().y - galley.size().y * 0.5), galley, encre);
    }
    resp
}

// ------------------------------------------------------------------ 164.1 : les taches de couleur

/// Le contour d'une tache de peinture dans `r` : une ellipse un peu irrégulière (toujours la même, `graine` la varie).
pub(crate) fn contour_tache(r: Rect, graine: u32) -> Vec<Pos2> {
    let n = 28;
    let c = r.center();
    let (rx, ry) = (r.width() * 0.5, r.height() * 0.5);
    (0..n)
        .map(|k| {
            let t = k as f32 / n as f32 * std::f32::consts::TAU;
            // (Deux ondulations lentes : la tache n'est pas une ellipse parfaite, mais reste convexe.)
            let g = graine as f32;
            let d = 1.0 - 0.06 * (2.0 * t + g).sin().abs() - 0.04 * (3.0 * t + 2.0 * g).cos().max(0.0);
            pos2(c.x + rx * d * t.cos(), c.y + ry * d * t.sin())
        })
        .collect()
}

/// Peint une tache de la couleur `c` (bordée du contour du thème quand elle est `vive`, la couleur d'aujourd'hui).
pub(crate) fn tache(p: &Painter, r: Rect, c: Option<[u8; 3]>, graine: u32, vive: bool) {
    let pts = contour_tache(r, graine);
    match c {
        Some(c) => {
            let bord = if vive { Stroke::new(1.5_f32, theme::contour()) } else { Stroke::new(1.0_f32, theme::bord_fort()) };
            p.add(Shape::convex_polygon(pts, Color32::from_rgb(c[0], c[1], c[2]), bord));
        }
        // Pas encore d'ancienne couleur : la place de la tache, en pointillés.
        None => {
            let mut boucle = pts.clone();
            boucle.push(pts[0]);
            p.add(Shape::dashed_line(&boucle, Stroke::new(1.0_f32, theme::texte_doux()), 3.0, 3.0));
        }
    }
}

/// Ce que retient la Couleur pour « avant · maintenant » : la couleur d'avant-plan vue posée à la dernière image calme (`vue`), et
/// celle d'avant elle (`avant`). Un geste (glisser la roue, taper un code) ne compte qu'une fois, quand il est fini.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct AvantMaintenant {
    pub vue: Option<[u8; 3]>,
    pub avant: Option<[u8; 3]>,
}

impl AvantMaintenant {
    /// À une image calme (rien d'appuyé, aucune saisie) : si la couleur d'avant-plan a changé depuis la dernière, l'ancienne devient
    /// « avant ».
    pub(crate) fn suivre(&mut self, couleur: [u8; 3], calme: bool) {
        if !calme {
            return;
        }
        match self.vue {
            Some(v) if v != couleur => {
                self.avant = Some(v);
                self.vue = Some(couleur);
            }
            None => self.vue = Some(couleur),
            _ => {}
        }
    }
}

// ------------------------------------------------------------------ 164.2 : les effets

/// Les effets montrés en vignettes pour « Ajouter » (ceux que FEANOR sait faire et qui se voient le mieux en petit), dans l'ordre de la
/// maquette puis les autres. Tous les autres restent dans le menu « Tous les effets », sous Précis.
pub(crate) const EN_VIGNETTES: [EffectKind; 9] = [
    EffectKind::Glow,
    EffectKind::GaussianBlur,
    EffectKind::Grain,
    EffectKind::Flash,
    EffectKind::Vignette,
    EffectKind::DropShadow,
    EffectKind::BlackWhite,
    EffectKind::Bloom,
    EffectKind::Boil,
];

/// Le nom court d'un effet sous sa vignette.
pub(crate) fn nom_court(kind: EffectKind) -> &'static str {
    match kind {
        EffectKind::GaussianBlur => "Flou",
        EffectKind::Grain => "Grain",
        EffectKind::DropShadow => "Ombre",
        EffectKind::BlackWhite => "Noir et blanc",
        EffectKind::Bloom => "Lueur diffuse",
        EffectKind::Boil => "Tremblement",
        k => k.label(),
    }
}

/// Le réglage d'un effet que tire la barre de l'avant | après (sa « force »), et la plage que couvre la barre (dans les bornes du
/// réglage ; au-delà, on règle dans Précis). Aucun pour un effet sans réglage de force (un négatif, une onde…).
pub(crate) fn force_de(kind: EffectKind) -> Option<(usize, f32, f32)> {
    let params = kind.params();
    let (k, lo, hi) = match kind {
        EffectKind::GaussianBlur => (0, 0.0, 40.0),
        EffectKind::MotionBlur => (0, 0.0, 120.0),
        EffectKind::Glow | EffectKind::Bloom => (1, 0.0, 200.0),
        EffectKind::DropShadow => (3, 0.0, 100.0),
        EffectKind::Boil => (0, 0.0, 10.0),
        _ => {
            let k = params.iter().position(|p| matches!(p.name, "Quantité" | "Intensité" | "Force"))?;
            // (Depuis zéro : un pincement va de -100 à 100, la barre ne va que de « sans effet » à « le plus fort ».)
            (k, params[k].min.max(0.0), params[k].max)
        }
    };
    let p = params.get(k)?;
    Some((k, lo.max(p.min), hi.min(p.max)))
}

/// La force (0 à 1) d'une valeur `v` du réglage de force.
pub(crate) fn fraction(v: f32, lo: f32, hi: f32) -> f32 {
    if hi <= lo {
        return 0.0;
    }
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// La valeur du réglage de force pour la barre posée à `x` dans `r` : la barre partage l'avant (à gauche) de l'après (à droite) ;
/// plus l'après est large, plus l'effet est fort.
pub(crate) fn force_a(x: f32, r: Rect) -> f32 {
    (1.0 - (x - r.left()) / r.width().max(1.0)).clamp(0.0, 1.0)
}

/// Le papier et le trait d'une vignette (une image simple : un trait sombre en arc sur un papier clair).
const PAPIER: Color32 = Color32::from_gray(222);
const ENCRE: Color32 = Color32::from_gray(60);

fn arc(r: Rect) -> Vec<Pos2> {
    let n = 18;
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let x = r.left() + r.width() * (0.12 + 0.76 * t);
            let y = r.bottom() - r.height() * (0.2 + 0.55 * (std::f32::consts::PI * t).sin());
            pos2(x, y)
        })
        .collect()
}

fn ligne(p: &Painter, pts: &[Pos2], s: Stroke) {
    p.add(Shape::line(pts.to_vec(), s));
}

/// Peint ce que produit l'effet `kind` à la force `f` (0 à 1) dans `r` : la vignette d'« Ajouter » (f vaut alors sa force d'origine)
/// et chaque moitié de l'avant | après (f = 0 : l'avant).
pub(crate) fn dessin_effet(p: &Painter, r: Rect, kind: EffectKind, f: f32) {
    let f = f.clamp(0.0, 1.0);
    let p = p.with_clip_rect(r.intersect(p.clip_rect()));
    let mince = (r.height() / 22.0).max(1.0);
    match kind {
        // La lueur : un point clair sur un fond sombre, son halo grandit avec la force.
        EffectKind::Glow | EffectKind::Bloom => {
            p.rect_filled(r, 0.0, Color32::from_gray(64));
            let c = r.center();
            let base = r.height() * 0.16;
            let large = if kind == EffectKind::Bloom { 2.6 } else { 1.8 };
            for k in (1..=5).rev() {
                let t = k as f32 / 5.0;
                let a = (f * 70.0 * (1.0 - t * 0.8)) as u8;
                p.circle_filled(c, base * (1.0 + large * f * t), Color32::from_rgba_unmultiplied(255, 210, 122, a));
            }
            p.circle_filled(c, base, Color32::from_rgb(255, 236, 190));
        }
        EffectKind::GaussianBlur | EffectKind::MotionBlur => {
            p.rect_filled(r, 0.0, PAPIER);
            let pts = arc(r);
            let n = 6;
            for k in (0..n).rev() {
                let t = k as f32 / (n - 1) as f32;
                let w = mince * (1.5 + 10.0 * f * t);
                let a = (255.0 * (1.0 - f * 0.85).max(0.12) * (1.0 - t * 0.7)) as u8;
                ligne(&p, &pts, Stroke::new(w, Color32::from_rgba_unmultiplied(60, 60, 60, if k == 0 { ((1.0 - f * 0.7) * 255.0) as u8 } else { a / 3 })));
            }
        }
        EffectKind::Grain | EffectKind::Noise => {
            p.rect_filled(r, 0.0, PAPIER);
            ligne(&p, &arc(r), Stroke::new(mince * 1.5, ENCRE));
            // (Des points toujours aux mêmes places : leur nombre suit la force.)
            let n = (f * 60.0) as u32;
            for k in 0..n {
                let h = k.wrapping_mul(2_654_435_761);
                let x = r.left() + r.width() * ((h % 1000) as f32 / 1000.0);
                let y = r.top() + r.height() * (((h / 1000) % 1000) as f32 / 1000.0);
                let g = if k % 2 == 0 { 90 } else { 160 };
                p.circle_filled(pos2(x, y), mince * 0.7, Color32::from_gray(g));
            }
        }
        // Le flash : l'image passe au blanc.
        EffectKind::Flash => {
            p.rect_filled(r, 0.0, PAPIER);
            ligne(&p, &arc(r), Stroke::new(mince * 1.5, ENCRE));
            p.rect_filled(r, 0.0, Color32::from_white_alpha((f * 235.0) as u8));
        }
        // Le vignettage : les bords s'assombrissent.
        EffectKind::Vignette => {
            p.rect_filled(r, 0.0, PAPIER);
            ligne(&p, &arc(r), Stroke::new(mince * 1.5, ENCRE));
            let c = r.center();
            let n = 32;
            let mut mesh = egui::Mesh::default();
            let sombre = Color32::from_black_alpha((f * 200.0) as u8);
            for k in 0..n {
                let t = k as f32 / n as f32 * std::f32::consts::TAU;
                let d = vec2(t.cos(), t.sin());
                let dedans = c + vec2(d.x * r.width() * 0.32, d.y * r.height() * 0.30);
                // (Le bord du cadre, dans cette direction.)
                let s = (r.width() * 0.5 / d.x.abs().max(1e-3)).min(r.height() * 0.5 / d.y.abs().max(1e-3));
                let dehors = c + d * s * 1.01;
                mesh.colored_vertex(dedans, Color32::TRANSPARENT);
                mesh.colored_vertex(dehors, sombre);
            }
            for k in 0..n as u32 {
                let (a, b) = (2 * k, 2 * ((k + 1) % n as u32));
                mesh.add_triangle(a, a + 1, b);
                mesh.add_triangle(a + 1, b + 1, b);
            }
            p.add(Shape::mesh(mesh));
        }
        // L'ombre portée : le trait et son ombre décalée.
        EffectKind::DropShadow => {
            p.rect_filled(r, 0.0, PAPIER);
            let pts = arc(r);
            let off = vec2(r.width(), r.height()) * 0.06;
            let ombre: Vec<Pos2> = pts.iter().map(|q| *q + off).collect();
            ligne(&p, &ombre, Stroke::new(mince * 3.0, Color32::from_black_alpha((f * 140.0) as u8)));
            ligne(&p, &pts, Stroke::new(mince * 1.5, ENCRE));
        }
        // Noir et blanc : une tache de couleur qui se désature.
        EffectKind::BlackWhite => {
            p.rect_filled(r, 0.0, PAPIER);
            let gris = |c: [u8; 3]| {
                let l = 0.3 * c[0] as f32 + 0.59 * c[1] as f32 + 0.11 * c[2] as f32;
                let m = |v: u8| (v as f32 + (l - v as f32) * f).round().clamp(0.0, 255.0) as u8;
                Color32::from_rgb(m(c[0]), m(c[1]), m(c[2]))
            };
            p.circle_filled(pos2(r.left() + r.width() * 0.38, r.center().y), r.height() * 0.3, gris([210, 60, 50]));
            p.circle_filled(pos2(r.left() + r.width() * 0.62, r.center().y), r.height() * 0.3, gris([60, 150, 220]));
        }
        // Le tremblement du trait : le trait en trois variantes, plus écartées avec la force.
        EffectKind::Boil => {
            p.rect_filled(r, 0.0, PAPIER);
            let pts = arc(r);
            for (k, d) in [(-1.0_f32, 0.6_f32), (1.0, -0.4), (0.0, 0.0)] {
                let o = vec2(k, d) * r.height() * 0.07 * f;
                let v: Vec<Pos2> = pts.iter().enumerate().map(|(i, q)| *q + o * if i % 2 == 0 { 1.0 } else { -0.6 }).collect();
                ligne(&p, &v, Stroke::new(mince * 1.2, if k == 0.0 { ENCRE } else { Color32::from_rgba_unmultiplied(60, 60, 60, (f * 160.0) as u8) }));
            }
        }
        // Les autres (dans l'avant | après d'un effet posé sans dessin à lui) : le trait, voilé de la couleur d'accent selon la force.
        _ => {
            p.rect_filled(r, 0.0, PAPIER);
            ligne(&p, &arc(r), Stroke::new(mince * 1.5, ENCRE));
            p.rect_filled(r, 0.0, theme::accent().gamma_multiply(0.45 * f));
        }
    }
}

/// La force d'origine d'un effet (celle de sa vignette d'« Ajouter »).
pub(crate) fn force_d_origine(kind: EffectKind) -> f32 {
    match force_de(kind) {
        Some((k, lo, hi)) => fraction(kind.params()[k].default, lo, hi).max(0.35),
        None => 0.7,
    }
}

/// Peint l'avant | après de l'effet `kind` à la force `f` dans `r` : à gauche de la barre, l'avant ; à droite, l'après ; la barre
/// (de l'accent) et sa poignée ; « avant » et « nom · n % » écrits en bas.
pub(crate) fn avant_apres(p: &Painter, r: Rect, kind: EffectKind, f: f32, nom: &str) {
    let x = r.left() + r.width() * (1.0 - f.clamp(0.0, 1.0));
    let gauche = Rect::from_min_max(r.min, pos2(x, r.bottom()));
    let droite = Rect::from_min_max(pos2(x, r.top()), r.max);
    if gauche.width() > 0.5 {
        dessin_effet(&p.with_clip_rect(gauche.intersect(p.clip_rect())), r, kind, 0.0);
    }
    if droite.width() > 0.5 {
        dessin_effet(&p.with_clip_rect(droite.intersect(p.clip_rect())), r, kind, f);
    }
    barre_avant_apres(p, r, f, nom);
}

/// Partie 165 : l'avant | après du vrai calque. `vue` : les deux images (le calque sans l'effet, avec lui), de l'aperçu calculé en
/// arrière-plan (`avantapres165`), posées à `vue.place` (relative) sur le papier du projet `papier` ; la barre et les textes par-dessus,
/// comme pour le schéma.
pub(crate) fn avant_apres_reel(p: &Painter, r: Rect, vue: &crate::avantapres165::Vue, papier: [u8; 3], f: f32, nom: &str) {
    let x = r.left() + r.width() * (1.0 - f.clamp(0.0, 1.0));
    let p = p.with_clip_rect(r.intersect(p.clip_rect()));
    p.rect_filled(r, 0.0, Color32::from_rgb(papier[0], papier[1], papier[2]));
    let place = Rect::from_min_max(r.lerp_inside(vue.place.min.to_vec2()), r.lerp_inside(vue.place.max.to_vec2()));
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    let gauche = Rect::from_min_max(r.min, pos2(x, r.bottom()));
    let droite = Rect::from_min_max(pos2(x, r.top()), r.max);
    if gauche.width() > 0.5 {
        p.with_clip_rect(gauche.intersect(p.clip_rect())).image(vue.avant, place, uv, Color32::WHITE);
    }
    if droite.width() > 0.5 {
        p.with_clip_rect(droite.intersect(p.clip_rect())).image(vue.apres, place, uv, Color32::WHITE);
    }
    barre_avant_apres(&p, r, f, nom);
}

/// Le cadre, la barre (de l'accent) et sa poignée à la force `f`, et « avant » / « nom · n % » écrits en bas : le même pour le schéma
/// et pour le vrai calque.
fn barre_avant_apres(p: &Painter, r: Rect, f: f32, nom: &str) {
    let x = r.left() + r.width() * (1.0 - f.clamp(0.0, 1.0));
    p.rect_stroke(r, 3.0, Stroke::new(1.0_f32, theme::bord_fort()), StrokeKind::Inside);
    let barre = Rect::from_center_size(pos2(x, r.center().y), vec2(3.0, r.height()));
    p.rect_filled(barre, 0.0, theme::accent());
    p.circle_filled(pos2(x, r.center().y), 6.5, theme::accent());
    p.circle_stroke(pos2(x, r.center().y), 6.5, Stroke::new(1.0_f32, theme::contour()));
    // (Les deux textes sur une bande sombre en bas : lisibles sur le papier clair comme sur le fond de la lueur.)
    let bande = Rect::from_min_max(pos2(r.left(), r.bottom() - 15.0), r.max);
    p.rect_filled(bande, 0.0, Color32::from_black_alpha(110));
    let police = FontId::proportional(10.5);
    p.text(pos2(r.left() + 5.0, bande.center().y), Align2::LEFT_CENTER, "avant", police.clone(), Color32::from_gray(225));
    p.text(pos2(r.right() - 5.0, bande.center().y), Align2::RIGHT_CENTER, format!("{nom} · {:.0} %", f * 100.0), police, Color32::WHITE);
}
