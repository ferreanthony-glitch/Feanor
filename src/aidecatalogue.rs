//! Le catalogue de l'aide animée, pour les outils (partie 153, `aideanim.rs`) : une animation par geste, et le texte qui était dans
//! le panneau. Les six premières sont celles qu'Anthony a validées (Trajectoire sur la feuille, Trajectoire dans l'espace, Maj
//! depuis un bout, boîte de sélection, caméra, dégradé) ; les temps de leurs choréographies sont les siens.
//!
//! Chaque dessin reçoit le temps en secondes et peint dans le repère 240 × 150 de la `Toile`.

use eframe::egui::{epaint::Mesh, pos2, vec2, Color32, Painter, Pos2, Rect, Shape};

use crate::aideanim::{doux, fondu, mix, mixp, par_les_marques, phase, Anim, Chemin, Teintes, Toile};

/// Les animations des outils, dans l'ordre du registre.
pub(crate) static OUTILS: &[Anim] = &[
    Anim {
        id: "trajectoire.tracer",
        titre: "Tracer, marquer, suivre",
        rubrique: "Trajectoire",
        contextes: &["trajectoire"],
        duree: 6.0,
        dessin: trajectoire_feuille,
        texte: "Trace le chemin du calque d'un trait : le calque le suit depuis la tête de lecture jusqu'à la fin du plan, en partant de là où il est. \
                Puis de petits traits en travers du chemin : les marques. Le temps est le même entre deux marques ; serrées, le calque va lentement ; espacées, il va vite. \
                Sur la timeline, glisser la première clé, la dernière ou une marque les recale dans le temps.",
    },
    Anim {
        id: "trajectoire.prolonger",
        titre: "Maj depuis un bout : prolonger",
        rubrique: "Trajectoire",
        contextes: &["trajectoire", "trajectoire.espace"],
        duree: 5.6,
        dessin: maj_prolonger,
        texte: "Tirer sur le chemin le tord là où on le prend, comme une corde (Maj : le déplacer en bloc). \
                Maj + un trait depuis un bout du chemin le prolonge, devant ou derrière. \
                Glisser une marque le long du chemin la déplace ; la tirer loin du chemin la retire. Un trait long retrace le chemin, les marques restent. \
                Sur la timeline, glisser la première clé, la dernière ou une marque les recale dans le temps ; Suppr sur une marque la retire. \
                Transformer déplace le calque et son chemin ensemble.",
    },
    Anim {
        id: "trajectoire.espace",
        titre: "Trajectoire dans l'espace",
        rubrique: "Trajectoire",
        contextes: &["trajectoire.espace"],
        duree: 6.0,
        dessin: trajectoire_espace,
        texte: "Dans la vue Espace 3D, d'un plan coché « Plan en Espace 3D » (panneau Espace 3D) : sur la feuille, hors de cette vue, l'outil trace le chemin du calque à plat. \
                Trace d'un trait, sur une carte ou sur le sol, le chemin de ce qui est choisi : chaque carte choisie, et la caméra si elle l'est, le suit depuis la tête de lecture jusqu'à la fin du plan, en partant de là où elle est. \
                Puis de petits traits en travers du chemin : les marques. Le temps est le même entre deux marques ; serrées, ça va lentement ; espacées, vite. \
                Ctrl+clic choisit ou lâche une carte, la caméra.\n\
                Tirer sur le chemin le tord là où on le prend (Maj : le déplacer en bloc). Maj + un trait depuis un bout du chemin le prolonge, devant ou derrière, sur le même plan. \
                Glisser une marque le long du chemin la déplace ; la tirer loin du chemin la retire. Un trait long retrace le chemin, sur la surface qu'il touche ; les marques restent. \
                Sur la timeline, glisser la première clé, la dernière ou une marque d'une carte les recale dans le temps. \
                La caméra ne reçoit aucune clé : son chemin s'ajoute à celles qu'elle a.",
    },
    Anim {
        id: "selection.boite",
        titre: "La boîte : déplacer, agrandir, tourner",
        rubrique: "Sélection",
        contextes: &["selection", "selection.bitmap", "selection.couleur"],
        duree: 6.0,
        dessin: boite,
        texte: "Boîte : glisser dedans pour déplacer (Maj : un seul axe) ; un coin ou le milieu d'un côté pour mettre à l'échelle, la poignée opposée restant en place \
                (Maj : proportions ; Ctrl : l'autre point fixe, réglé par « Point fixe ») ; juste hors d'un coin pour tourner autour du pivot ou du coin opposé \
                (réglé par « Rotation » ; Maj : par 15°) ; un bord entre les poignées pour incliner ; le point rond pour déplacer le pivot. \
                Flèches : déplacer d'un pixel (Maj : dix).\n\
                Calque vectoriel : clic : un élément. Glisser dans le vide : lasso. Maj : ajouter / retirer. Ctrl+Maj : prendre sur tous les calques du plan (pas seulement les calques choisis).",
    },
    Anim {
        id: "camera.cadrer",
        titre: "Cadrer, poser une clé, jouer",
        rubrique: "Caméra",
        contextes: &["camera"],
        duree: 6.0,
        dessin: camera,
        texte: "Glisser dans le cadre : déplacer (Maj : à l'horizontale ou à la verticale). Coin : zoomer. Poignée ronde : tourner (Maj : par 15°). \
                Un plan sans clé : caméra au centre, cadre entier. \
                Losanges jaunes de la piste Caméra : clic pour choisir (Ctrl : ajouter, Maj : la suite), glisser pour déplacer, glisser dans le vide de la piste pour en choisir plusieurs au rectangle \
                (Maj : ajouter, Ctrl : basculer), Ctrl+C / Ctrl+V à l'image courante, Suppr. \
                Aides au cadrage : menu « Cadrage » au-dessus du dessin.",
    },
    Anim {
        id: "degrade.axe",
        titre: "Tirer l'axe et valider",
        rubrique: "Dégradé",
        contextes: &["degrade"],
        duree: 5.0,
        dessin: degrade,
        texte: "Glisser : tracer l'axe (Maj : par 15°). Ensuite l'axe reste : tire ses poignées ou change les réglages, jusqu'à Entrée, Échap ou un autre outil. \
                Dans la sélection, sinon dans la zone fermée où l'on appuie ; Alt : tout le calque.",
    },
];

// ------------------------------------------------------------------------------------------------------ Trajectoire

/// Le chemin des deux trajectoires sur la feuille : `M30,115 C70,25 150,135 210,45`.
fn chemin_feuille() -> Chemin {
    Chemin::bezier((30.0, 115.0), (70.0, 25.0), (150.0, 135.0), (210.0, 45.0))
}

/// Une marque : un petit trait en travers du chemin, à la fraction `s`.
fn marque(c: &Toile, chemin: &Chemin, s: f32, couleur: Color32) {
    let (p, d) = chemin.point(s);
    let n = vec2(-d.y, d.x) * 6.0;
    c.ligne(p - n, p + n, 1.4, couleur);
}

/// 1. Trajectoire sur la feuille : le curseur trace le chemin, pose trois marques serrées au milieu, puis la carte suit, plus lente au milieu.
fn trajectoire_feuille(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let chemin = chemin_feuille();
    let marques = [0.3, 0.4, 0.5];
    // Le trait du chemin.
    let trace = doux(t, 0.3, 1.8);
    c.polyligne(&chemin.jusqu_a(trace), 1.6, th.encre);
    // Les marques, l'une après l'autre.
    let mut curseur = chemin.point(trace).0;
    let mut appuye = t > 0.3 && t < 1.8;
    for (i, &m) in marques.iter().enumerate() {
        let a = 1.9 + i as f32 * 0.34;
        if t >= a + 0.2 {
            marque(&c, &chemin, m, th.encre);
        }
        if t >= a - 0.1 && t < a + 0.34 {
            let (q, d) = chemin.point(m);
            let n = vec2(-d.y, d.x) * 6.0;
            curseur = mixp(q - n, q + n, phase(t, a, a + 0.2));
            appuye = t >= a;
        }
    }
    if t < 0.3 {
        curseur = chemin.point(0.0).0 + vec2(-8.0, 6.0) * (1.0 - phase(t, 0.0, 0.3));
    }
    // La carte : à son départ, puis le long du chemin, à temps égaux entre les marques ; des points gris à chaque image.
    let u = phase(t, 3.1, 5.7);
    let images = 16;
    for i in 0..=images {
        let ui = i as f32 / images as f32;
        if t >= 3.1 && ui <= u + 1e-4 {
            c.disque(chemin.point(par_les_marques(ui, &marques)).0, 1.5, th.forte);
        }
    }
    let (q, _) = chemin.point(par_les_marques(u, &marques));
    c.carte(q, 18.0, 13.0, false);
    if t < 3.0 {
        c.curseur(curseur, appuye);
    }
}

/// 3. Maj depuis un bout : un chemin existe ; Maj, le curseur va au bout, un cercle jade s'y ouvre, le prolongement se trace en jade puis
/// prend l'encre ; la carte parcourt tout le chemin d'une traite.
fn maj_prolonger(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let chemin = Chemin::bezier((30.0, 115.0), (60.0, 50.0), (110.0, 120.0), (150.0, 70.0));
    let suite = Chemin::bezier((150.0, 70.0), (170.0, 45.0), (195.0, 40.0), (215.0, 52.0));
    c.polyligne(&chemin.pts, 1.6, th.encre);
    let maj = t > 0.3 && t < 2.9;
    c.touche(pos2(26.0, 18.0), "Maj", maj);
    let bout = chemin.point(1.0).0;
    // Le cercle qui s'ouvre sur le bout.
    let ouvert = doux(t, 1.2, 1.5) * (1.0 - doux(t, 2.9, 3.2));
    if ouvert > 0.0 {
        c.cercle(bout, 7.0 * ouvert, 1.4, th.jade);
    }
    // Le prolongement, en jade, puis à l'encre.
    let s = doux(t, 1.6, 2.8);
    let couleur = mix_couleur(th.jade, th.encre, phase(t, 2.9, 3.3));
    c.polyligne(&suite.jusqu_a(s), 1.6, couleur);
    // Le curseur.
    if t < 3.0 {
        let depart = pos2(120.0, 30.0);
        let ici = if t < 1.6 { mixp(depart, bout, doux(t, 0.4, 1.2)) } else { suite.point(s).0 };
        c.curseur(ici, t >= 1.6 && t < 2.8);
    }
    // La carte parcourt tout le chemin d'une traite.
    let u = doux(t, 3.4, 5.4);
    let l1 = chemin.longueur();
    let l2 = suite.longueur();
    let d = u * (l1 + l2);
    let q = if d <= l1 { chemin.point(d / l1).0 } else { suite.point((d - l1) / l2).0 };
    c.carte(q, 18.0, 13.0, false);
}

/// Une couleur entre deux autres.
pub(crate) fn mix_couleur(a: Color32, b: Color32, s: f32) -> Color32 {
    let m = |x: u8, y: u8| mix(x as f32, y as f32, s.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()), m(a.a(), b.a()))
}

/// Le sol en perspective de l'Espace 3D : `gx` de -1 à 1 (gauche, droite), `gz` de 0 (devant) à 1 (au fond). Rend le point et
/// l'éloignement (1 devant, 4 au fond).
pub(crate) fn sol(gx: f32, gz: f32) -> (Pos2, f32) {
    let d = 1.0 + 3.0 * gz;
    (pos2(120.0 + gx * 120.0 / d, 52.0 + 96.0 / d), d)
}

/// La grille du sol.
pub(crate) fn grille_sol(c: &Toile) {
    for i in 0..=8 {
        let gx = -1.6 + i as f32 * 0.4;
        c.ligne(sol(gx, 0.0).0, sol(gx, 1.0).0, 0.8, c.t.forte);
    }
    for j in 0..=5 {
        let gz = j as f32 / 5.0;
        c.ligne(sol(-1.6, gz).0, sol(1.6, gz).0, 0.8, c.t.forte);
    }
}

/// Une carte debout sur le sol, à sa taille selon l'éloignement.
fn carte_debout(c: &Toile, gx: f32, gz: f32, choisie: bool) {
    let (pied, d) = sol(gx, gz);
    let (w, h) = (34.0 / d, 46.0 / d);
    c.carte(pos2(pied.x, pied.y - h / 2.0), w, h, choisie);
}

/// 2. Trajectoire dans l'espace : un sol en grille ; Ctrl s'allume, un clic choisit une carte debout ; le curseur trace le chemin sur le
/// sol ; la carte le suit, sa taille change avec la distance.
fn trajectoire_espace(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    grille_sol(&c);
    // Le chemin, sur le sol (dans le repère du sol).
    let au_sol = Chemin::bezier((-0.6, 0.15), (-0.1, 0.05), (0.2, 0.9), (0.75, 0.75));
    let ecran = |q: Pos2| sol(q.x, q.y).0;
    let ctrl = t > 0.2 && t < 1.2;
    c.touche(pos2(28.0, 18.0), "Ctrl", ctrl);
    let choisie = t > 0.9;
    let trace = doux(t, 1.3, 2.8);
    let pts: Vec<Pos2> = au_sol.jusqu_a(trace).into_iter().map(ecran).collect();
    c.polyligne(&pts, 1.6, th.encre);
    // La carte : au départ, puis le long du chemin.
    let u = doux(t, 3.1, 5.6);
    let (q, _) = au_sol.point(u);
    carte_debout(&c, q.x, q.y, choisie);
    // Le curseur : vers la carte, clic, puis le tracé.
    if t < 3.0 {
        let (pied, d) = sol(-0.6, 0.15);
        let sur_la_carte = pos2(pied.x, pied.y - 23.0 / d);
        let ici = if t < 1.3 { mixp(pos2(70.0, 30.0), sur_la_carte, doux(t, 0.5, 0.85)) } else { ecran(au_sol.point(trace).0) };
        let appuye = (t > 0.85 && t < 1.05) || (t >= 1.3 && t < 2.8);
        c.curseur(ici, appuye);
    }
}

// ------------------------------------------------------------------------------------------------------ Sélection

/// 4. La boîte de sélection : un personnage dans sa boîte ; glisser dedans la déplace, tirer un coin l'agrandit, juste hors d'un coin
/// elle tourne ; puis tout revient.
fn boite(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let boite = Rect::from_min_max(pos2(92.0, 42.0), pos2(148.0, 118.0));
    let retour = 1.0 - doux(t, 4.8, 5.7);
    let deplace = vec2(34.0, -8.0) * doux(t, 0.7, 1.5) * retour;
    let echelle = 1.0 + 0.3 * doux(t, 2.2, 3.0) * retour;
    let angle = 0.38 * doux(t, 3.6, 4.5) * retour;
    let coin = boite.left_top();
    // Mettre à l'échelle autour du coin opposé, tourner autour du centre, déplacer.
    let centre = coin + (boite.center() - coin) * echelle;
    let tf = |q: Pos2| -> Pos2 {
        let s = coin + (q - coin) * echelle;
        let v = s - centre;
        let (sn, cs) = angle.sin_cos();
        centre + vec2(v.x * cs - v.y * sn, v.x * sn + v.y * cs) + deplace
    };
    // Le personnage, transformé point par point.
    let h = 62.0;
    let pied = pos2(120.0, 114.0);
    let tete = pos2(pied.x, pied.y - h * 0.82);
    let tour: Vec<Pos2> = (0..=20).map(|i| {
        let a = i as f32 / 20.0 * std::f32::consts::TAU;
        tf(tete + vec2(a.cos(), a.sin()) * h * 0.14)
    }).collect();
    c.polyligne(&tour, 1.3, th.encre);
    let cou = pos2(pied.x, pied.y - h * 0.66);
    let bassin = pos2(pied.x, pied.y - h * 0.32);
    for (a, b) in [
        (cou, bassin),
        (cou + vec2(0.0, 3.0), cou + vec2(-11.0, 16.0)),
        (cou + vec2(0.0, 3.0), cou + vec2(11.0, 15.0)),
        (bassin, pied + vec2(-7.0, 0.0)),
        (bassin, pied + vec2(7.0, 0.0)),
    ] {
        c.ligne(tf(a), tf(b), 1.3, th.encre);
    }
    // La boîte et ses quatre poignées.
    let coins = [boite.left_top(), boite.right_top(), boite.right_bottom(), boite.left_bottom()].map(tf);
    let mut ferme = coins.to_vec();
    ferme.push(coins[0]);
    c.pointilles(&ferme, 1.2, th.jade);
    let tenue_coin = t > 2.15 && t < 3.1;
    for (i, q) in coins.iter().enumerate() {
        c.poignee(*q, tenue_coin && i == 2);
    }
    // Le curseur.
    if t < 5.0 {
        let dedans = tf(pos2(126.0, 86.0));
        let hors_du_coin = coins[1] + vec2(9.0, -9.0);
        let ici = if t < 1.7 {
            if t < 0.6 { mixp(pos2(200.0, 130.0), dedans, doux(t, 0.2, 0.6)) } else { dedans }
        } else if t < 3.2 {
            if t < 2.15 { mixp(tf(pos2(126.0, 86.0)), coins[2], doux(t, 1.7, 2.1)) } else { coins[2] }
        } else if t < 3.6 {
            mixp(coins[2], hors_du_coin, doux(t, 3.2, 3.55))
        } else {
            hors_du_coin
        };
        let appuye = (t > 0.6 && t < 1.6) || tenue_coin || (t > 3.6 && t < 4.6);
        if t > 3.3 && t < 4.7 {
            c.fleche_courbe(hors_du_coin + vec2(-2.0, 2.0), 9.0, th.jade);
        }
        c.curseur(ici, appuye);
    }
}

// ------------------------------------------------------------------------------------------------------ Caméra

/// Un décor simple : un horizon, un arbre, un personnage.
pub(crate) fn decor(c: &Toile) {
    c.ligne(pos2(8.0, 94.0), pos2(232.0, 94.0), 1.0, c.t.douce);
    c.arbre(pos2(62.0, 94.0), 52.0, c.t.encre);
    c.bonhomme(pos2(154.0, 112.0), 40.0, 0.4, c.t.encre);
}

/// 5. Caméra : glisser le cadre le déplace, tirer un coin le resserre ; un losange de clé apparaît sur la piste ; la tête de lecture
/// avance, le cadre va de la première clé à la seconde.
fn camera(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    decor(&c);
    let a = Rect::from_center_size(pos2(118.0, 70.0), vec2(150.0, 84.0));
    // Le geste : déplacer (0,3 → 1,4), resserrer par le coin (1,6 → 2,6) ; B est le cadre obtenu.
    let deplace = vec2(22.0, 8.0);
    let b = Rect::from_center_size(a.center() + deplace, vec2(96.0, 54.0));
    let apres_glisser = Rect::from_center_size(a.center() + deplace * doux(t, 0.5, 1.4), a.size());
    let cadre = if t < 1.6 {
        apres_glisser
    } else if t < 3.0 {
        let s = doux(t, 1.7, 2.6);
        Rect::from_center_size(mixp(apres_glisser.center(), b.center(), s), mixp(a.size().to_pos2(), b.size().to_pos2(), s).to_vec2())
    } else if t < 3.3 {
        // La tête de lecture revient à la première clé : le cadre de la première clé.
        let s = doux(t, 3.0, 3.3);
        Rect::from_center_size(mixp(b.center(), a.center(), s), mixp(b.size().to_pos2(), a.size().to_pos2(), s).to_vec2())
    } else {
        let s = doux(t, 3.4, 5.6);
        Rect::from_center_size(mixp(a.center(), b.center(), s), mixp(a.size().to_pos2(), b.size().to_pos2(), s).to_vec2())
    };
    c.cadre(cadre, 1.5, th.jade);
    for q in [cadre.left_top(), cadre.right_top(), cadre.right_bottom(), cadre.left_bottom()] {
        c.poignee(q, false);
    }
    // La piste, ses clés, la tête de lecture.
    let (x1, x2, y) = (40.0, 196.0, 136.0);
    c.ligne(pos2(20.0, y), pos2(220.0, y), 1.0, th.forte);
    c.losange(pos2(x1, y), 4.0, th.jade);
    if t > 2.8 {
        c.losange(pos2(x2, y), 4.0 * doux(t, 2.8, 3.0), th.jade);
    }
    // (Elle va d'abord à l'image de la seconde clé, que le geste pose ; elle revient à la première, puis joue.)
    let tete = if t < 3.0 { mix(x1, x2, doux(t, 0.0, 0.4)) } else if t < 3.3 { mix(x2, x1, doux(t, 3.0, 3.3)) } else { mix(x1, x2, doux(t, 3.4, 5.6)) };
    c.ligne(pos2(tete, y - 7.0), pos2(tete, y + 7.0), 1.2, th.encre);
    // Le curseur.
    if t < 2.8 {
        let prise = pos2(a.center().x + 30.0, a.center().y - 10.0);
        let ici = if t < 0.5 {
            mixp(pos2(220.0, 20.0), prise, doux(t, 0.1, 0.45))
        } else if t < 1.5 {
            prise + deplace * doux(t, 0.5, 1.4)
        } else if t < 1.7 {
            mixp(prise + deplace, apres_glisser.right_bottom(), doux(t, 1.5, 1.7))
        } else {
            let s = doux(t, 1.7, 2.6);
            mixp(apres_glisser.right_bottom(), b.right_bottom(), s)
        };
        c.curseur(ici, (t > 0.5 && t < 1.4) || (t > 1.7 && t < 2.6));
    }
}

// ------------------------------------------------------------------------------------------------------ Dégradé

/// Un remplissage du dégradé dans `zone` : de l'encre (au départ de l'axe) au fond (à son bout), en dégradé linéaire le long de l'axe.
pub(crate) fn remplir_degrade(c: &Toile, zone: Rect, depart: Pos2, bout: Pos2, force: f32) {
    let axe = bout - depart;
    let l2 = axe.length_sq().max(1.0);
    let (nx, ny) = (24, 14);
    let mut mesh = Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let q = pos2(mix(zone.left(), zone.right(), i as f32 / nx as f32), mix(zone.top(), zone.bottom(), j as f32 / ny as f32));
            let s = ((q - depart).dot(axe) / l2).clamp(0.0, 1.0);
            let col = fondu(mix_couleur(c.t.encre, c.t.fond, s), force * 0.85);
            mesh.colored_vertex(c.ecran(q), col);
        }
    }
    let w = (nx + 1) as u32;
    for j in 0..ny as u32 {
        for i in 0..nx as u32 {
            let k = j * w + i;
            mesh.add_triangle(k, k + 1, k + w);
            mesh.add_triangle(k + 1, k + w + 1, k + w);
        }
    }
    c.p.add(Shape::mesh(mesh));
}

/// 6. Dégradé : le curseur tire l'axe avec ses deux poignées, le remplissage paraît ; la poignée de fin s'oriente ; Entrée : les poignées
/// s'en vont.
fn degrade(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let zone = Rect::from_min_max(pos2(40.0, 30.0), pos2(200.0, 120.0));
    let depart = pos2(72.0, 75.0);
    let bout_droit = pos2(170.0, 75.0);
    let bout_tourne = pos2(150.0, 112.0);
    let tire = doux(t, 0.6, 1.6);
    let oriente = doux(t, 2.2, 3.3);
    let bout = mixp(mixp(depart, bout_droit, tire), bout_tourne, oriente);
    if t > 0.6 {
        remplir_degrade(&c, zone, depart, if tire > 0.02 { bout } else { depart + vec2(1.0, 0.0) }, phase(t, 0.6, 0.9));
    }
    c.cadre(zone, 1.4, th.encre);
    // L'axe et ses poignées, jusqu'à Entrée.
    let visibles = 1.0 - phase(t, 4.0, 4.4);
    if t > 0.6 && visibles > 0.0 {
        let col = fondu(th.jade, visibles);
        c.ligne(depart, bout, 1.2, col);
        c.cercle(depart, 4.0, 1.4, col);
        c.cercle(bout, 4.0, 1.4, col);
        if t > 2.0 && t < 3.4 {
            c.disque(bout, 2.4, col);
        }
    }
    c.touche(pos2(212.0, 138.0), "Entrée", t > 3.7 && t < 4.4);
    if t < 3.6 {
        let ici = if t < 0.6 {
            mixp(pos2(30.0, 140.0), depart, doux(t, 0.1, 0.55))
        } else if t < 1.9 {
            mixp(depart, bout_droit, tire)
        } else if t < 2.2 {
            bout_droit
        } else {
            bout
        };
        c.curseur(ici, (t > 0.6 && t < 1.6) || (t > 2.0 && t < 3.3));
    }
}
