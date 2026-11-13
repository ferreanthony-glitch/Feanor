//! Le catalogue de l'aide animée pour l'espace Vignettes (partie 153, `aideanim.rs` ; ajout d'Anthony du 2026-11-13) : la coupe ou la
//! continuité, un trait à travers les cases, les caméras dans les vignettes, les annotations, le pont avec le Storyboard. Elles jouent à
//! la première ouverture de l'espace Vignettes (ou au premier usage du geste : l'outil Caméra, un outil d'annotation) et ont leur
//! rubrique « Vignettes » dans le registre.
//!
//! Les proportions sont celles de la vraie planche (`board.rs`) : une case de 240 de large, à 16:9 ; 18 d'écart entre deux images d'un
//! plan, 53 à une coupe (`GAP_SAME`, `GAP_CUT`) ; la barre entre deux cases fait le tiers de leur hauteur, pâle dans un plan, à l'encre
//! (et plus épaisse) à une coupe ; le numéro du plan est au-dessus de sa première image. Ici les cases font 38 de large : les écarts
//! gardent leur rapport (18 : 53), un peu agrandis pour se voir.

use eframe::egui::{pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect};

use crate::aideanim::{doux, fondu, mix, mixp, phase, Anim, Chemin, Teintes, Toile};

/// La largeur d'une case dans la vignette, et sa hauteur (16:9).
const CASE_W: f32 = 38.0;
const CASE_H: f32 = CASE_W * 9.0 / 16.0;
/// Les écarts : dans un plan, à une coupe (le rapport de `board::GAP_SAME` à `board::GAP_CUT`).
const ECART: f32 = 6.0;
const ECART_COUPE: f32 = ECART * 53.0 / 18.0;

/// Les animations des Vignettes, dans l'ordre du registre.
pub(crate) static VIGNETTES: &[Anim] = &[
    Anim {
        id: "vignettes.coupe",
        titre: "Coupe ou continuité",
        rubrique: "Vignettes",
        contextes: &["vignettes"],
        duree: 6.0,
        dessin: coupe,
        texte: "Un clic sur la barre entre deux cases d'un même plan coupe : la barre devient noire, l'écart s'ouvre, et la case qui suit commence un plan neuf (son numéro paraît au-dessus). \
                Un second clic réunit les deux plans. \
                Un clic sur la barre devant la case vide pâle fait la case tout de suite, avec la copie du dessin d'avant : c'est la continuité ; la case suivante reste vide.",
    },
    Anim {
        id: "vignettes.trait",
        titre: "Un trait à travers les cases",
        rubrique: "Vignettes",
        contextes: &["vignettes"],
        duree: 5.0,
        dessin: trait_cases,
        texte: "Un trait part du papier, traverse des cases et entre dans une case vide : au lâcher, chaque case garde sa part, et la case vide devient un plan. \
                La part restée hors des cases demeure sur le papier, plus pâle.",
    },
    Anim {
        id: "vignettes.camera",
        titre: "Caméras dans les vignettes",
        rubrique: "Vignettes",
        contextes: &["vignettes.camera"],
        duree: 6.0,
        dessin: cameras,
        texte: "Sur la planche : survole une case, ses cadres se montrent. \
                Glisser un cadre le déplace ; un coin le redimensionne (le zoom). Clic droit sur la case : ajouter ou retirer un cadre. \
                Ctrl + Maj + glisser dans une case : le mouvement de caméra (Alt : un cadre de plus ; {vignettes.camera} tenue : pareil). \
                La case qui suit montre le dernier cadre. Le dessin ne bouge jamais : seuls les cadres bougent. \
                Les réglages fins de la caméra (clés, adoucis, focale) sont dans le Storyboard.",
    },
    Anim {
        id: "vignettes.annotations",
        titre: "Annotations",
        rubrique: "Vignettes",
        contextes: &["vignettes.annotation"],
        duree: 5.6,
        dessin: annotations,
        texte: "Par-dessus la planche, sans effet sur le storyboard : on entoure un détail, on pose une flèche, quelques mots dans la marge, dans la couleur choisie comme n'importe quel trait. \
                Ce qui les distingue, c'est leur calque : l'œil le cache, et un plan ouvert en Storyboard ne les montre pas. \
                Aucune annotation pour l'instant : le premier trait d'annotation crée son calque.\n\
                Lasso d'annotation : glisser dans le vide : lasso (Maj : ajouter). Clic : un trait. Dans la boîte : déplacer ; un coin : agrandir ; hors d'un coin : tourner. Échap : abandonner.",
    },
    Anim {
        id: "vignettes.pont",
        titre: "Le pont avec le Storyboard",
        rubrique: "Vignettes",
        contextes: &["vignettes"],
        duree: 6.0,
        dessin: pont,
        texte: "Vignettes et Storyboard : passer de l'un à l'autre{vue.planches}. La case grandit jusqu'à devenir le dessin du plan dans le Storyboard ; \
                au retour (la même touche, ou le bouton « Retour aux Vignettes »), elle rapetisse à sa place sur la planche et montre ce qu'on y a dessiné. \
                Entrer dans le plan pour l'animer, et en revenir{mode.basculer} : le même passage, vers l'animation.",
    },
];

/// Le rectangle d'une case, à partir de son bord gauche, sur la ligne de la planche.
fn case(x: f32, y: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(CASE_W, CASE_H))
}

/// Le dessin d'une case : un sol et un bonhomme, à `avance` (de 0 à 1) dans la case.
fn dessin_case(c: &Toile, r: Rect, avance: f32, col: Color32) {
    c.pave(r, c.t.cadre);
    c.cadre(r, 1.0, c.t.forte);
    c.ligne(pos2(r.left() + 2.0, r.bottom() - 4.0), pos2(r.right() - 2.0, r.bottom() - 4.0), 0.8, fondu(col, 0.6));
    c.bonhomme(pos2(mix(r.left() + 8.0, r.right() - 8.0, avance), r.bottom() - 4.0), CASE_H * 0.7, 0.3 + 0.6 * avance, col);
}

/// Une case vide pâle, avec son « + ».
fn case_vide(c: &Toile, r: Rect, alpha: f32) {
    c.cadre(r, 1.0, fondu(c.t.forte, 0.6 * alpha));
    let m = r.center();
    c.ligne(m - vec2(4.0, 0.0), m + vec2(4.0, 0.0), 1.0, fondu(c.t.douce, alpha));
    c.ligne(m - vec2(0.0, 4.0), m + vec2(0.0, 4.0), 1.0, fondu(c.t.douce, alpha));
}

/// La barre entre deux cases (au milieu de l'écart) : le tiers de la hauteur, pâle dans un plan, à l'encre à une coupe (`coupe` de 0 à 1).
fn barre(c: &Toile, x: f32, y: f32, coupe: f32) {
    let h = CASE_H / 3.0;
    let m = y + CASE_H / 2.0;
    let col = crate::aidecatalogue::mix_couleur(c.t.forte, c.t.encre, coupe);
    c.ligne(pos2(x, m - h / 2.0), pos2(x, m + h / 2.0), mix(1.5, 2.0, coupe) * 0.7, col);
}

/// Le numéro d'un plan, au-dessus de sa première case.
fn numero(c: &Toile, r: Rect, n: usize, alpha: f32) {
    if alpha > 0.0 {
        c.p.text(c.ecran(pos2(r.left() + 1.0, r.top() - 3.0)), Align2::LEFT_BOTTOM, n.to_string(), FontId::monospace(10.0 * c.k), fondu(c.t.encre, alpha));
    }
}

/// V1. Coupe ou continuité : un clic sur la barre grise coupe (elle devient noire, l'écart s'ouvre, un numéro paraît) ; un second
/// clic réunit ; un clic devant la case vide pâle la fait, avec la copie du dessin d'avant.
fn coupe(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let y = 62.0;
    // La coupe entre la case 1 et la case 2 : ouverte de 1,1 à 1,5, refermée de 2,3 à 2,7.
    let k = doux(t, 1.1, 1.5) * (1.0 - doux(t, 2.3, 2.7));
    let ouvre = (ECART_COUPE - ECART) * k;
    let x0 = 8.0;
    let xs = [x0, x0 + CASE_W + ECART + ouvre, x0 + 2.0 * (CASE_W + ECART) + ouvre, x0 + 3.0 * (CASE_W + ECART) + ouvre, x0 + 4.0 * (CASE_W + ECART) + ouvre];
    for (i, avance) in [0.15, 0.45, 0.75].into_iter().enumerate() {
        dessin_case(&c, case(xs[i], y), avance, th.encre);
    }
    // La case 4 : vide et pâle, puis faite (la copie du dessin d'avant) au clic de 4,0.
    let faite = phase(t, 4.0, 4.4);
    if faite > 0.0 {
        let r4 = case(xs[3], y);
        c.pave(r4, fondu(th.cadre, faite));
        dessin_case(&c, r4, 0.75, fondu(th.encre, faite));
    }
    case_vide(&c, case(xs[3], y), 1.0 - faite);
    if faite > 0.0 {
        case_vide(&c, case(xs[4], y), faite);
    }
    // Les barres.
    barre(&c, xs[1] - (ECART + ouvre) / 2.0, y, k);
    barre(&c, xs[2] - ECART / 2.0, y, 0.0);
    barre(&c, xs[3] - ECART / 2.0, y, 0.0);
    if faite > 0.0 {
        barre(&c, xs[4] - ECART / 2.0, y, 0.0);
    }
    numero(&c, case(xs[0], y), 1, 1.0);
    numero(&c, case(xs[1], y), 2, k);
    // Le curseur : la barre 1-2 (deux clics), puis la barre devant la case vide.
    let b12 = pos2(xs[1] - (ECART + ouvre) / 2.0, y + CASE_H / 2.0);
    let b34 = pos2(xs[3] - ECART / 2.0, y + CASE_H / 2.0);
    let ici = if t < 1.0 { mixp(pos2(120.0, 135.0), b12, doux(t, 0.3, 0.95)) } else if t < 3.2 { b12 } else { mixp(b12, b34, doux(t, 3.2, 3.8)) };
    if t < 5.2 {
        c.curseur(ici, (t > 1.0 && t < 1.15) || (t > 2.2 && t < 2.35) || (t > 3.9 && t < 4.05));
    }
}

/// V2. Un trait à travers les cases : il part du papier, traverse deux cases, entre dans une case vide ; au lâcher, chaque case garde sa
/// part (cernée brièvement de jade), la case vide devient un plan, la part hors des cases reste sur le papier, plus pâle.
fn trait_cases(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let y = 55.0;
    let k = 1.5;
    let (w, h) = (CASE_W * k, CASE_H * k);
    let cases = [Rect::from_min_size(pos2(30.0, y), vec2(w, h)), Rect::from_min_size(pos2(30.0 + w + ECART * k, y), vec2(w, h)), Rect::from_min_size(pos2(30.0 + 2.0 * (w + ECART * k), y), vec2(w, h))];
    let lache = t > 2.6;
    let devenue = phase(t, 2.7, 3.1);
    for (i, rc) in cases.iter().enumerate() {
        if i < 2 {
            c.pave(*rc, th.cadre);
            c.cadre(*rc, 1.0, th.forte);
        } else {
            c.pave(*rc, fondu(th.cadre, devenue));
            c.cadre(*rc, 1.0, fondu(th.forte, 0.5 + 0.5 * devenue));
        }
    }
    numero(&c, cases[0], 1, 1.0);
    numero(&c, cases[2], 2, devenue);
    // Le trait : une vague du papier (à gauche, au-dessus des cases) jusque dans la case vide.
    let chemin = Chemin::bezier((8.0, 30.0), (60.0, 140.0), (130.0, 10.0), (cases[2].center().x + 10.0, cases[2].center().y + 6.0));
    let s = doux(t, 0.3, 2.4);
    let n = 80;
    for i in 0..n {
        let (a, b) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        if b > s {
            break;
        }
        let (qa, qb) = (chemin.point(a).0, chemin.point(b).0);
        let dedans = cases.iter().any(|rc| rc.contains(mixp(qa, qb, 0.5)));
        let col = if lache && !dedans { mix_c(th.encre, th.douce, phase(t, 2.6, 3.2)) } else { th.encre };
        c.ligne(qa, qb, 1.5, col);
    }
    // Au lâcher, chaque case se cerne brièvement de jade.
    let cerne = phase(t, 2.6, 2.9) * (1.0 - phase(t, 3.6, 4.2));
    if cerne > 0.0 {
        for rc in &cases {
            c.cadre(rc.expand(2.0), 1.4, fondu(th.jade, cerne));
        }
    }
    if t < 2.9 {
        let ici = if t < 0.3 { mixp(pos2(30.0, 140.0), chemin.point(0.0).0, doux(t, 0.0, 0.28)) } else { chemin.point(s).0 };
        c.curseur(ici, t > 0.3 && t < 2.5);
    }
}

fn mix_c(a: Color32, b: Color32, s: f32) -> Color32 {
    crate::aidecatalogue::mix_couleur(a, b, s)
}

/// V3. Caméras dans les vignettes : le survol montre le cadre et ses poignées ; glisser le déplace, un coin le recadre ; Ctrl+Maj+glisser
/// pose un cadre B, une fine flèche relie A à B ; la case qui suit montre le dernier cadre. Le dessin ne bouge jamais.
fn cameras(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let k = 2.2;
    let (w, h) = (CASE_W * k, CASE_H * k);
    let r1 = Rect::from_min_size(pos2(18.0, 50.0), vec2(w, h));
    let r2 = Rect::from_min_size(pos2(r1.right() + ECART * k, 50.0), vec2(w, h));
    for rc in [r1, r2] {
        c.pave(rc, th.cadre);
        c.cadre(rc, 1.0, th.forte);
        c.ligne(pos2(rc.left() + 3.0, rc.bottom() - 8.0), pos2(rc.right() - 3.0, rc.bottom() - 8.0), 0.8, th.douce);
        c.arbre(pos2(rc.left() + w * 0.25, rc.bottom() - 8.0), h * 0.6, th.encre);
        c.bonhomme(pos2(rc.left() + w * 0.68, rc.bottom() - 8.0), h * 0.5, 0.4, th.encre);
    }
    numero(&c, r1, 1, 1.0);
    let survol = phase(t, 0.3, 0.6);
    // Le cadre A : déplacé (0,8 → 1,6), recadré par un coin (1,8 → 2,5).
    let a0 = r1.shrink(4.0);
    let deplace = vec2(6.0, 3.0) * doux(t, 0.8, 1.6);
    let resserre = doux(t, 1.8, 2.5);
    let a = Rect::from_min_max(a0.min + deplace, a0.max + deplace - vec2(18.0, 10.0) * resserre);
    c.cadre(a, 1.4, fondu(th.jade, survol));
    for q in [a.left_top(), a.right_top(), a.right_bottom(), a.left_bottom()] {
        if survol > 0.5 {
            c.poignee(q, false);
        }
    }
    // Ctrl+Maj tenues : le cadre B, tiré de 3,0 à 3,8, et la flèche de A à B.
    let tenues = t > 2.8 && t < 4.0;
    c.touche(pos2(30.0, 18.0), "Ctrl", tenues);
    c.touche(pos2(64.0, 18.0), "Maj", tenues);
    let b_s = doux(t, 3.0, 3.8);
    let b = Rect::from_center_size(mixp(a.center(), r1.center() + vec2(10.0, -2.0), b_s), a.size() * mix(1.0, 0.62, b_s));
    if t > 3.0 {
        c.cadre(b, 1.4, th.jade);
        c.fleche(a.center(), b.center(), 0.9, th.jade);
    }
    // La case qui suit montre le dernier cadre.
    let suit = phase(t, 4.2, 4.6);
    if suit > 0.0 {
        let rel = |q: Pos2| r2.min + (q - r1.min);
        c.cadre(Rect::from_min_max(rel(b.min), rel(b.max)), 1.4, fondu(th.jade, suit));
    }
    if t < 4.0 {
        let prise = a0.center();
        let ici = if t < 0.8 { mixp(pos2(220.0, 140.0), prise, doux(t, 0.0, 0.7)) } else if t < 1.7 { prise + deplace } else if t < 1.8 { mixp(prise + deplace, a.right_bottom(), doux(t, 1.7, 1.8)) } else if t < 3.0 { a.right_bottom() } else { b.center() };
        c.curseur(ici, (t > 0.8 && t < 1.6) || (t > 1.8 && t < 2.5) || (t > 3.0 && t < 3.8));
    }
}

/// Un œil (celui du calque) ; `ouvert` de 0 (fermé) à 1.
fn oeil(c: &Toile, m: Pos2, ouvert: f32) {
    let haut: Vec<Pos2> = (0..=12).map(|i| {
        let u = i as f32 / 12.0;
        pos2(m.x - 8.0 + 16.0 * u, m.y - 5.0 * ouvert * (u * std::f32::consts::PI).sin())
    }).collect();
    let bas: Vec<Pos2> = (0..=12).map(|i| {
        let u = i as f32 / 12.0;
        pos2(m.x - 8.0 + 16.0 * u, m.y + 5.0 * (u * std::f32::consts::PI).sin())
    }).collect();
    c.polyligne(&haut, 1.2, c.t.encre);
    c.polyligne(&bas, 1.2, c.t.encre);
    if ouvert > 0.3 {
        c.disque(m, 2.2 * ouvert, c.t.encre);
    }
}

/// V4. Annotations : sur leur calque, par-dessus les cases (un cercle autour d'un détail, une flèche, des notes dans la marge), dans la
/// même encre que le dessin ; l'œil du calque se ferme, elles disparaissent, le storyboard reste intact ; un plan ouvert en Storyboard
/// ne les montre pas.
fn annotations(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let k = 1.45;
    let (w, h) = (CASE_W * k, CASE_H * k);
    let y = 40.0;
    let cases = [Rect::from_min_size(pos2(14.0, y), vec2(w, h)), Rect::from_min_size(pos2(14.0 + w + ECART * k, y), vec2(w, h)), Rect::from_min_size(pos2(14.0 + 2.0 * (w + ECART * k), y), vec2(w, h))];
    for (i, rc) in cases.iter().enumerate() {
        dessin_case(&c, *rc, [0.2, 0.5, 0.8][i], th.encre);
    }
    // L'œil du calque d'annotation, en haut à droite : fermé de 3,8 à 4,4.
    let ouvert = 1.0 - doux(t, 3.8, 4.1) * (1.0 - doux(t, 4.9, 5.2));
    oeil(&c, pos2(222.0, 16.0), ouvert);
    let vues = ouvert;
    // Le cercle autour d'un détail (la tête du bonhomme de la case 2), la flèche, les notes de la marge : à l'encre du dessin.
    let tete = pos2(mix(cases[1].left() + 8.0, cases[1].right() - 8.0, 0.5), cases[1].bottom() - 4.0 - CASE_H * k * 0.7 * 0.82);
    let tour = Chemin::de((0..=36).map(|i| {
        let a = i as f32 / 36.0 * std::f32::consts::TAU * 1.08 - 1.2;
        tete + vec2(a.cos() * 9.0, a.sin() * 8.0)
    }).collect());
    let s1 = doux(t, 0.4, 1.4);
    c.polyligne(&tour.jusqu_a(s1), 1.3, fondu(th.encre, vues));
    let fleche_de = pos2(tete.x + 30.0, cases[1].bottom() + 24.0);
    let s2 = doux(t, 1.6, 2.3);
    if s2 > 0.0 {
        let bout = mixp(fleche_de, tete + vec2(6.0, 9.0), s2);
        c.ligne(fleche_de, bout, 1.3, fondu(th.encre, vues));
        if s2 > 0.95 {
            c.fleche(fleche_de, bout, 1.3, fondu(th.encre, vues));
        }
    }
    // Quelques traits de note dans la marge.
    for (i, l) in [44.0, 30.0].into_iter().enumerate() {
        let s = doux(t, 2.5 + i as f32 * 0.4, 2.9 + i as f32 * 0.4);
        let a = pos2(fleche_de.x + 6.0, fleche_de.y + 4.0 + i as f32 * 7.0);
        if s > 0.0 {
            c.ligne(a, a + vec2(l * s, 0.0), 1.2, fondu(th.encre, vues));
        }
    }
    // Le plan ouvert en Storyboard : le dessin de la case 2, sans les annotations.
    let sb = phase(t, 4.4, 4.8) * (1.0 - phase(t, 5.3, 5.6));
    if sb > 0.0 {
        let grand = Rect::from_min_size(pos2(18.0, 100.0), vec2(CASE_W * 1.4, CASE_H * 1.4));
        c.cadre(grand.expand(2.0), 1.0, fondu(th.jade, sb));
        dessin_case(&c, grand, 0.5, fondu(th.encre, sb));
    }
    if t < 3.6 {
        let ici = if t < 1.6 { tour.point(s1).0 } else if t < 2.4 { mixp(fleche_de, tete + vec2(6.0, 9.0), s2) } else { pos2(fleche_de.x + 6.0 + 40.0 * doux(t, 2.5, 3.3), fleche_de.y + 4.0 + if t > 2.9 { 7.0 } else { 0.0 }) };
        c.curseur(ici, (t > 0.4 && t < 1.4) || (t > 1.6 && t < 2.3) || (t > 2.5 && t < 3.3));
    } else if t < 4.0 {
        c.curseur(mixp(pos2(180.0, 120.0), pos2(222.0, 16.0), doux(t, 3.4, 3.75)), t > 3.75);
    }
}

/// V5. Le pont avec le Storyboard : Maj+Tab, la case grandit jusqu'à devenir le dessin du plan ; un trait s'ajoute ; retour, la case
/// rapetisse à sa place et montre le nouveau trait. Variante courte : Tab, l'animation, et le même retour.
fn pont(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let k = 1.45;
    let (w, h) = (CASE_W * k, CASE_H * k);
    let y = 56.0;
    let cases = [Rect::from_min_size(pos2(14.0, y), vec2(w, h)), Rect::from_min_size(pos2(14.0 + w + ECART * k, y), vec2(w, h)), Rect::from_min_size(pos2(14.0 + 2.0 * (w + ECART * k), y), vec2(w, h))];
    let grand = Rect::from_min_max(pos2(30.0, 14.0), pos2(210.0, 115.0));
    // La case 2 grandit (1,0 → 1,8), reste grande, puis rapetisse (3,7 → 4,5) ; variante courte (Tab) de 4,9 à 5,8.
    let g = doux(t, 1.0, 1.8) * (1.0 - doux(t, 3.7, 4.5));
    let g2 = doux(t, 4.9, 5.2) * (1.0 - doux(t, 5.4, 5.8)) * 0.35;
    let gg = g.max(g2);
    let rc = Rect::from_min_max(mixp(cases[1].min, grand.min, gg), mixp(cases[1].max, grand.max, gg));
    for (i, q) in cases.iter().enumerate() {
        if i != 1 {
            dessin_case(&c, *q, [0.2, 0.5, 0.8][i], fondu(th.encre, 1.0 - gg));
        }
    }
    dessin_case(&c, rc, 0.5, th.encre);
    // Le trait ajouté dans le grand dessin (2,1 → 3,1), qui reste dans la case.
    let trait_ = Chemin::bezier((0.2, 0.3), (0.35, 0.1), (0.55, 0.45), (0.8, 0.25));
    let s = doux(t, 2.1, 3.1);
    let place = |q: Pos2| pos2(mix(rc.left(), rc.right(), q.x), mix(rc.top(), rc.bottom(), q.y));
    let pts: Vec<Pos2> = trait_.jusqu_a(s).into_iter().map(place).collect();
    c.polyligne(&pts, 1.4, th.encre);
    c.touche(pos2(26.0, 140.0), "Maj", (t > 0.5 && t < 1.1) || (t > 3.3 && t < 3.8));
    c.touche(pos2(58.0, 140.0), "Tab", (t > 0.5 && t < 1.1) || (t > 3.3 && t < 3.8) || (t > 4.7 && t < 5.0) || (t > 5.25 && t < 5.45));
    if t > 2.0 && t < 3.2 {
        c.curseur(place(trait_.point(s).0), t < 3.1);
    }
}
