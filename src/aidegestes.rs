//! Le catalogue de l'aide animée, suite (partie 153, `aideanim.rs`) : les gestes des autres outils, une animation par geste, avec le
//! texte qui était dans leur panneau. Même style que les six premières (`aidecatalogue.rs`).

use eframe::egui::{pos2, vec2, Color32, Painter, Pos2, Rect};

use crate::aidecatalogue::{decor, grille_sol, mix_couleur, remplir_degrade, sol};
use crate::aideanim::{doux, fondu, mix, mixp, phase, Anim, Chemin, Teintes, Toile};

/// Les animations des autres outils, dans l'ordre du registre.
pub(crate) static GESTES: &[Anim] = &[
    Anim {
        id: "trajectoire.tordre",
        titre: "Tordre le chemin, déplacer ou retirer une marque",
        rubrique: "Trajectoire",
        contextes: &["trajectoire"],
        duree: 5.4,
        dessin: tordre,
        texte: "Tirer sur le chemin le tord là où on le prend, comme une corde (Maj : le déplacer en bloc). \
                Glisser une marque le long du chemin la déplace ; la tirer loin du chemin la retire. Un trait long retrace le chemin, les marques restent. \
                Sur la timeline, glisser la première clé, la dernière ou une marque les recale dans le temps ; Suppr sur une marque la retire. \
                Transformer déplace le calque et son chemin ensemble.",
    },
    Anim {
        id: "selection.lasso",
        titre: "Le lasso sur un calque bitmap",
        rubrique: "Sélection",
        contextes: &["selection.bitmap"],
        duree: 5.0,
        dessin: lasso,
        texte: "Glisser dans le vide : lasso ou rectangle (Maj : ajouter). Un clic sur un dessin choisit sa forme (Ctrl+Maj + clic : en ajouter une autre, sur tous les calques). \
                Échap ou autre outil : poser la zone.",
    },
    Anim {
        id: "selection.couleur",
        titre: "La sélection par couleur",
        rubrique: "Sélection",
        contextes: &["selection.couleur"],
        duree: 4.6,
        dessin: par_couleur,
        texte: "Calque vectoriel : clic sur un trait ou une zone : tous les éléments de sa couleur dans ce dessin. Maj : ajouter.\n\
                Calque bitmap : clic : la zone de cette couleur. Maj : ajouter. Poignées : échelle et rotation ; flèches pour déplacer. Échap ou autre outil : poser.",
    },
    Anim {
        id: "selection.plusieurs",
        titre: "Plusieurs calques dans une seule boîte",
        rubrique: "Sélection",
        contextes: &["selection.multi", "transformer"],
        duree: 5.0,
        dessin: plusieurs,
        texte: "Une seule boîte entoure tout : chaque geste (déplacer, échelle, tourner, incliner, flèches, champs) s'applique à tous les calques autour du même pivot, en une étape d'annulation. \
                Un clic sur un trait le prend ; Maj : ajouter ou retirer ; Ctrl+Maj : prendre sur tous les calques du plan (leurs clés se choisissent aussi dans le montage). \
                Les calques cachés ou verrouillés sont laissés de côté.\n\
                Outil Transformer, plusieurs calques choisis : un coin agrandit tous les calques en gardant les proportions ; l'étirement par un côté se fait sur un seul calque. \
                Ctrl+R remet tous les calques choisis en place. Les calques cachés ou verrouillés restent où ils sont.",
    },
    Anim {
        id: "dessin.pression",
        titre: "La pression du stylet change l'épaisseur",
        rubrique: "Dessin",
        contextes: &["pinceau", "crayon", "gomme.bitmap"],
        duree: 4.4,
        dessin: pression,
        texte: "La pression vient du stylet, par le pilote du stylet. À la souris, elle vaut 100 %.\n\
                Sur un calque vectoriel, le trait du pinceau reste modifiable avec l'outil Sélection{outil.selection}, même texturé (pointe en image, grain du papier) ; celui du crayon aussi.",
    },
    Anim {
        id: "dessin.melangeur",
        titre: "Le mélangeur étale",
        rubrique: "Dessin",
        contextes: &["melangeur"],
        duree: 4.6,
        dessin: melangeur,
        texte: "Mélangeur : la brosse du pinceau, en « Lisser » : elle étale ce qui est peint sans poser de couleur. « Couleur et mélange » règle l'entraînement.",
    },
    Anim {
        id: "dessin.gomme",
        titre: "La gomme coupe un trait vectoriel",
        rubrique: "Dessin",
        contextes: &["gomme"],
        duree: 4.2,
        dessin: gomme,
        texte: "Sur un calque vectoriel, la gomme coupe les traits. Les zones remplies s'effacent en mode « traits entiers » ou par la sélection.",
    },
    Anim {
        id: "dessin.cutter",
        titre: "Le Cutter coupe aux croisements",
        rubrique: "Dessin",
        contextes: &["cutter"],
        duree: 4.6,
        dessin: cutter,
        texte: "Glisse en travers des bouts de traits à enlever, ou clique sur l'un d'eux. \
                Calque vectoriel : chaque trait touché est coupé là où il croise les autres traits (et lui-même) ; seuls les morceaux touchés partent. \
                Un trait qui ne croise rien part en entier. Ctrl+Z annule.",
    },
    Anim {
        id: "camera.paliers",
        titre: "Paliers et adoucis",
        rubrique: "Caméra",
        contextes: &["camera"],
        duree: 5.0,
        dessin: paliers,
        texte: "Mouvement par paliers : le mouvement montré par paliers, aucune clé n'est touchée. Sans mouvement (moins de deux clés), rien ne change. \
                L'outil Caméra arrêté montre le mouvement doux, pour le régler. Lance la lecture ou change d'outil pour voir les paliers.\n\
                Adoucis : place-toi sur une clé de caméra pour régler ses adoucis.",
    },
    Anim {
        id: "camera.effets",
        titre: "Effets de caméra sur une plage d'images",
        rubrique: "Caméra",
        contextes: &["camera"],
        duree: 5.0,
        dessin: effets,
        texte: "Choisir un effet : il se voit tout de suite, sans clé. Choisir des images dans la timeline avant pour viser une plage.",
    },
    Anim {
        id: "deformer.enveloppe",
        titre: "L'enveloppe",
        rubrique: "Déformer",
        contextes: &["deformer"],
        duree: 4.6,
        dessin: enveloppe,
        texte: "Clic sur le dessin : son enveloppe apparaît (sur un calque vectoriel, la sélection s'il y en a une ; sur un calque bitmap, la zone choisie). \
                Tire un coin ou une poignée ronde pour courber les côtés, l'intérieur pour déplacer. Ctrl+Z annule chaque geste.\n\
                Plusieurs calques choisis : clic sur le dessin : un seul cadre entoure ce que contiennent les calques choisis. Tire un coin (ou une poignée ronde de l'enveloppe), l'intérieur pour déplacer. \
                Les calques cachés ou verrouillés sont laissés de côté. Ctrl+Z annule chaque geste.",
    },
    Anim {
        id: "deformer.perspective",
        titre: "La perspective",
        rubrique: "Déformer",
        contextes: &["deformer"],
        duree: 4.4,
        dessin: perspective,
        texte: "Clic sur le dessin : son cadre apparaît (sur un calque vectoriel, la sélection s'il y en a une). Tire un coin pour mettre en perspective, l'intérieur pour déplacer. \
                Ctrl+Z annule chaque geste.",
    },
    Anim {
        id: "degrade.pastilles",
        titre: "Les pastilles sur la bande",
        rubrique: "Dégradé",
        contextes: &["degrade"],
        duree: 5.2,
        dessin: pastilles,
        texte: "Les deux pastilles de couleur suivent les carrés avant-plan et arrière-plan du panneau Couleur ; touche à une pastille pour les détacher.\n\
                Clic sous la bande : ajouter une couleur. Clic au-dessus : ajouter une opacité. Glisser : déplacer ; tirer vers le bas ou le haut : supprimer. \
                Losange : milieu (double-clic : au centre).",
    },
    Anim {
        id: "formes.tracer",
        titre: "Droite, courbe, S, polyligne",
        rubrique: "Formes",
        contextes: &["formes"],
        duree: 6.0,
        dessin: formes,
        texte: "Droite : glisser pour tracer. Maj : angle par pas de 15°.\n\
                Courbe : glisser pour tracer, puis glisser pour courber : la courbe passe par le pointeur.\n\
                Courbe en S : glisser pour tracer, puis deux glissers pour courber (le second fait le S). Entrée : poser, Échap : annuler.\n\
                Polyligne : un clic par point (Maj : par 15°). Double-clic ou Entrée : terminer. Clic sur le premier point : fermer. Retour arrière : retirer un point. Échap : annuler.\n\
                Rectangle, ellipse : glisser pour tracer. Maj : carré ou cercle.",
    },
    Anim {
        id: "sculpter.grab_pull",
        titre: "Grab et Pull",
        rubrique: "Sculpter",
        contextes: &["sculpter"],
        duree: 5.6,
        dessin: sculpter,
        texte: "Grab : appui : les points du cercle sont pris ; glisser les déplace, plus ou moins selon leur distance au centre.\n\
                Pull : glisser : les points sous le cercle suivent le mouvement, comme un doigt qui étire le trait.\n\
                Plusieurs clés choisies du calque : la même déformation s'applique à chacune (une seule étape d'annulation).",
    },
    Anim {
        id: "outils.pot",
        titre: "Le pot de peinture",
        rubrique: "Autres outils",
        contextes: &["remplissage"],
        duree: 4.0,
        dessin: pot,
        texte: "Sur un calque vectoriel, sur un trait ou une zone déjà peinte : change sa couleur.\n\
                Astuce : trait sur un calque, couleurs sur un calque en dessous, contours pris sur tous les calques.",
    },
    Anim {
        id: "outils.texte",
        titre: "Le texte",
        rubrique: "Autres outils",
        contextes: &["texte"],
        duree: 4.4,
        dessin: texte,
        texte: "Clic : nouveau texte (sur un texte d'un calque vectoriel : le modifier). Ctrl+Entrée : poser ; Échap : annuler. \
                Sur un calque vectoriel, le texte reste modifiable (Sélection : déplacer, agrandir, tourner). Une police absente du poste est remplacée par celle de FEANOR.",
    },
    Anim {
        id: "outils.pipette",
        titre: "La pipette (Alt + clic)",
        rubrique: "Autres outils",
        contextes: &["pipette"],
        duree: 4.0,
        dessin: pipette,
        texte: "Clique sur le dessin pour prélever une couleur. Avec les autres outils : Alt + clic.",
    },
    Anim {
        id: "outils.main",
        titre: "La main (Espace + glisser)",
        rubrique: "Autres outils",
        contextes: &["main"],
        duree: 4.4,
        dessin: main,
        texte: "Glisse pour déplacer la vue. Sinon : Espace + glisser, ou bouton du milieu.",
    },
    Anim {
        id: "espace3d.ensemble",
        titre: "Plusieurs cartes choisies ensemble",
        rubrique: "Espace 3D",
        contextes: &["espace3d"],
        duree: 4.8,
        dessin: e3_ensemble,
        texte: "Plusieurs cartes choisies : elles se déplacent, tournent et grandissent ensemble ; choisis-en une seule pour ses réglages.",
    },
    Anim {
        id: "espace3d.carte",
        titre: "Une carte et ses axes de rotation",
        rubrique: "Espace 3D",
        contextes: &["espace3d"],
        duree: 4.8,
        dessin: e3_carte,
        texte: "Rotation Z : le tour dans le plan de la carte (antihoraire) ; X et Y inclinent la carte.",
    },
    Anim {
        id: "espace3d.camera",
        titre: "La caméra libre",
        rubrique: "Espace 3D",
        contextes: &["espace3d"],
        duree: 5.0,
        dessin: e3_camera,
        texte: "Plan en Espace 3D : la caméra est libre. Elle tourne tout autour (panoramique sans borne), passe derrière les cartes, bascule jusqu'à la verticale \
                (90° : plongée ou contre-plongée totale), et l'image la suit.",
    },
];

// ------------------------------------------------------------------------------------------------------ Trajectoire

/// Trajectoire : tirer sur le chemin le tord comme une corde ; glisser une marque la déplace ; la tirer loin du chemin la retire.
fn tordre(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let base = Chemin::bezier((30.0, 100.0), (80.0, 60.0), (150.0, 110.0), (210.0, 60.0));
    // Le tord : un creux autour du milieu, qui suit le curseur.
    let tire = doux(t, 0.5, 1.5) * 26.0;
    let pts: Vec<Pos2> = base
        .pts
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let s = i as f32 / (base.pts.len() - 1) as f32;
            let w = (-((s - 0.5) / 0.18).powi(2)).exp();
            *q + vec2(0.0, -tire * w)
        })
        .collect();
    let chemin = Chemin::de(pts);
    c.polyligne(&chemin.pts, 1.6, th.encre);
    // Les marques : la première glisse le long du chemin, la troisième part loin et s'efface.
    let m1 = mix(0.25, 0.38, doux(t, 2.1, 3.0));
    let m3 = 0.75;
    let loin = doux(t, 3.6, 4.5);
    for (s, ecart, alpha) in [(m1, 0.0, 1.0), (0.55, 0.0, 1.0), (m3, loin * 30.0, 1.0 - phase(t, 4.4, 4.7))] {
        let (q, d) = chemin.point(s);
        let n = vec2(-d.y, d.x);
        let q = q + vec2(0.0, ecart);
        c.ligne(q - n * 6.0, q + n * 6.0, 1.4, fondu(th.encre, alpha));
    }
    c.carte(chemin.point(0.0).0, 18.0, 13.0, false);
    let prise_milieu = chemin.point(0.5).0;
    let (q1, _) = chemin.point(m1);
    let (q3, _) = chemin.point(m3);
    let ici = if t < 0.5 {
        mixp(pos2(120.0, 140.0), prise_milieu, doux(t, 0.0, 0.45))
    } else if t < 1.7 {
        prise_milieu
    } else if t < 2.1 {
        mixp(prise_milieu, chemin.point(0.25).0, doux(t, 1.7, 2.05))
    } else if t < 3.2 {
        q1
    } else if t < 3.6 {
        mixp(q1, q3, doux(t, 3.2, 3.55))
    } else {
        q3 + vec2(0.0, loin * 30.0)
    };
    if t < 4.9 {
        c.curseur(ici, (t > 0.5 && t < 1.5) || (t > 2.1 && t < 3.0) || (t > 3.6 && t < 4.5));
    }
}

// ------------------------------------------------------------------------------------------------------ Sélection

/// Une tache de peinture (bitmap) : un disque bosselé plein.
fn tache(c: &Toile, centre: Pos2, rayon: f32, col: Color32) {
    let pts: Vec<Pos2> = (0..24)
        .map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            let r = rayon * (1.0 + 0.12 * (3.0 * a).sin());
            centre + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    c.plein(&pts, col);
}

/// Le lasso sur un calque bitmap : le curseur entoure une tache, la zone se ferme en pointillés, glisser dedans la déplace.
fn lasso(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let centre = pos2(110.0, 75.0);
    let deplace = vec2(40.0, -6.0) * doux(t, 3.0, 4.2);
    tache(&c, pos2(185.0, 110.0), 14.0, th.douce);
    tache(&c, centre + deplace, 22.0, th.encre);
    let boucle: Vec<Pos2> = (0..=40)
        .map(|i| {
            let a = i as f32 / 40.0 * std::f32::consts::TAU - 1.6;
            centre + vec2(a.cos() * 38.0, a.sin() * 32.0) * (1.0 + 0.06 * (5.0 * a).sin())
        })
        .collect();
    let s = doux(t, 0.4, 2.2);
    let n = ((boucle.len() - 1) as f32 * s).round() as usize;
    let vus: Vec<Pos2> = boucle[..=n.min(boucle.len() - 1)].iter().map(|q| *q + deplace).collect();
    if t < 2.3 {
        c.polyligne(&vus, 1.2, th.jade);
    } else {
        c.pointilles(&vus, 1.2, th.jade);
    }
    if t < 4.6 {
        let ici = if t < 0.4 { mixp(pos2(30.0, 30.0), boucle[0], doux(t, 0.0, 0.35)) } else if t < 2.5 { vus[vus.len() - 1] } else if t < 3.0 { mixp(boucle[0], centre, doux(t, 2.5, 2.9)) } else { centre + deplace };
        c.curseur(ici, (t > 0.4 && t < 2.2) || (t > 3.0 && t < 4.2));
    }
}

/// La sélection par couleur : un clic sur une tache prend toutes celles de sa couleur ; Maj + clic en ajoute une autre.
fn par_couleur(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let encre = [pos2(60.0, 60.0), pos2(170.0, 100.0)];
    let douce = pos2(120.0, 105.0);
    for q in encre {
        tache(&c, q, 18.0, th.encre);
    }
    tache(&c, douce, 16.0, th.douce);
    let entoure = |q: Pos2, rayon: f32, s: f32| {
        if s > 0.0 {
            c.cadre_pointille(Rect::from_center_size(q, vec2(2.0, 2.0) * (rayon + 5.0)), 1.2, fondu(th.jade, s));
        }
    };
    for q in encre {
        entoure(q, 18.0, phase(t, 1.1, 1.4));
    }
    entoure(douce, 16.0, phase(t, 3.0, 3.3));
    c.touche(pos2(26.0, 18.0), "Maj", t > 2.2 && t < 3.4);
    if t < 4.2 {
        let ici = if t < 1.0 { mixp(pos2(210.0, 30.0), encre[0], doux(t, 0.2, 0.9)) } else if t < 2.4 { encre[0] } else { mixp(encre[0], douce, doux(t, 2.4, 2.9)) };
        c.curseur(ici, (t > 0.95 && t < 1.15) || (t > 2.9 && t < 3.1));
    }
}

/// Plusieurs calques dans une seule boîte : deux cartes (deux calques), une boîte autour des deux ; glisser déplace tout, un coin agrandit tout.
fn plusieurs(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let retour = 1.0 - doux(t, 4.0, 4.8);
    let deplace = vec2(26.0, 6.0) * doux(t, 0.6, 1.6) * retour;
    let echelle = 1.0 + 0.25 * doux(t, 2.2, 3.2) * retour;
    let boite = Rect::from_min_max(pos2(70.0, 40.0), pos2(170.0, 112.0));
    let tf = |q: Pos2| boite.left_top() + (q - boite.left_top()) * echelle + deplace;
    for (centre, w, h) in [(pos2(98.0, 64.0), 40.0, 30.0), (pos2(140.0, 90.0), 44.0, 32.0)] {
        let a = tf(centre - vec2(w, h) / 2.0);
        let b = tf(centre + vec2(w, h) / 2.0);
        c.carte(mixp(a, b, 0.5), b.x - a.x, b.y - a.y, false);
    }
    let coins = [boite.left_top(), boite.right_top(), boite.right_bottom(), boite.left_bottom()].map(tf);
    let mut ferme = coins.to_vec();
    ferme.push(coins[0]);
    c.pointilles(&ferme, 1.2, th.jade);
    for (i, q) in coins.iter().enumerate() {
        c.poignee(*q, t > 2.15 && t < 3.2 && i == 2);
    }
    if t < 4.0 {
        let dedans = tf(pos2(120.0, 76.0));
        let ici = if t < 0.6 { mixp(pos2(220.0, 140.0), dedans, doux(t, 0.1, 0.55)) } else if t < 1.8 { dedans } else if t < 2.15 { mixp(dedans, coins[2], doux(t, 1.8, 2.1)) } else { coins[2] };
        c.curseur(ici, (t > 0.6 && t < 1.6) || (t > 2.15 && t < 3.2));
    }
}

// ------------------------------------------------------------------------------------------------------ Dessin

/// Un trait d'épaisseur variable : `largeur(s)` pour la fraction `s` du chemin, jusqu'à `fin`.
fn trait_variable(c: &Toile, chemin: &Chemin, fin: f32, largeur: impl Fn(f32) -> f32, col: Color32) {
    let n = 60;
    for i in 0..n {
        let (a, b) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        if b > fin {
            break;
        }
        c.ligne(chemin.point(a).0, chemin.point(b).0, largeur((a + b) / 2.0), col);
    }
}

/// La pression change l'épaisseur : le trait s'épaissit quand la jauge monte, s'amincit quand elle descend.
fn pression(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let chemin = Chemin::bezier((30.0, 95.0), (80.0, 40.0), (150.0, 130.0), (200.0, 70.0));
    let s = doux(t, 0.4, 3.2);
    let appui = |x: f32| (x * std::f32::consts::PI).sin().max(0.08);
    trait_variable(&c, &chemin, s, |x| 1.0 + 7.0 * appui(x), th.encre);
    // La jauge de l'appui, à droite.
    let jauge = Rect::from_min_max(pos2(222.0, 30.0), pos2(228.0, 120.0));
    c.cadre(jauge, 1.0, th.forte);
    let niveau = if t > 0.4 && t < 3.2 { appui(s) } else { 0.0 };
    c.pave(Rect::from_min_max(pos2(jauge.left(), jauge.bottom() - jauge.height() * niveau), jauge.max), th.jade);
    if t < 3.8 {
        let ici = if t < 0.4 { mixp(pos2(20.0, 30.0), chemin.point(0.0).0, doux(t, 0.0, 0.35)) } else { chemin.point(s).0 };
        c.curseur(ici, t > 0.4 && t < 3.2);
    }
}

/// Le mélangeur étale : deux pavés de peinture ; le curseur passe de l'un à l'autre et traîne la couleur derrière lui.
fn melangeur(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    c.pave(Rect::from_min_max(pos2(30.0, 45.0), pos2(115.0, 105.0)), th.encre);
    c.pave(Rect::from_min_max(pos2(115.0, 45.0), pos2(205.0, 105.0)), th.forte);
    let chemin = Chemin::bezier((70.0, 75.0), (110.0, 60.0), (140.0, 90.0), (190.0, 72.0));
    let s = doux(t, 0.6, 3.4);
    let n = 40;
    for i in 0..=n {
        let u = i as f32 / n as f32;
        if u > s {
            break;
        }
        let (q, _) = chemin.point(u);
        // La couleur emportée : l'encre, qui se mêle peu à peu à celle du lieu.
        let lieu = if q.x < 115.0 { th.encre } else { th.forte };
        let emporte = (1.0 - (u - 0.3).max(0.0) * 1.6).clamp(0.0, 1.0);
        c.disque(q, 7.0, mix_couleur(lieu, th.encre, emporte));
    }
    if t < 4.0 {
        let ici = if t < 0.6 { mixp(pos2(60.0, 130.0), chemin.point(0.0).0, doux(t, 0.1, 0.55)) } else { chemin.point(s).0 };
        c.cercle(ici, 8.0, 1.0, th.jade);
        c.curseur(ici, t > 0.6 && t < 3.4);
    }
}

/// La gomme coupe un trait vectoriel : elle passe en travers, le trait est coupé en deux morceaux.
fn gomme(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let chemin = Chemin::bezier((25.0, 80.0), (80.0, 40.0), (160.0, 120.0), (215.0, 75.0));
    let x = mix(120.0, 120.0, 0.0);
    let y = mix(25.0, 130.0, doux(t, 1.0, 2.2));
    let coupe = t > 1.5;
    if coupe {
        let trou = 0.07;
        c.polyligne(&chemin.entre(0.0, 0.5 - trou), 1.8, th.encre);
        c.polyligne(&chemin.entre(0.5 + trou, 1.0), 1.8, th.encre);
        let a = phase(t, 1.6, 1.9) * (1.0 - phase(t, 3.0, 3.4));
        c.disque(chemin.point(0.5 - trou).0, 2.5, fondu(th.jade, a));
        c.disque(chemin.point(0.5 + trou).0, 2.5, fondu(th.jade, a));
    } else {
        c.polyligne(&chemin.pts, 1.8, th.encre);
    }
    if t < 3.8 {
        let ici = if t < 1.0 { mixp(pos2(200.0, 20.0), pos2(x, 25.0), doux(t, 0.1, 0.9)) } else { pos2(x, y) };
        c.cercle(ici, 10.0, 1.0, th.forte);
        c.curseur(ici, t > 1.0 && t < 2.2);
    }
}

/// Le Cutter coupe aux croisements : trois traits qui se croisent ; un geste en travers d'un bout l'enlève jusqu'au croisement.
fn cutter(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    c.ligne(pos2(40.0, 40.0), pos2(200.0, 115.0), 1.6, th.encre);
    c.ligne(pos2(40.0, 115.0), pos2(200.0, 40.0), 1.6, th.encre);
    // Le trait horizontal : son bout droit, au-delà du croisement, est le morceau qui part.
    let croise = pos2(160.0, 58.0);
    c.ligne(pos2(30.0, 58.0), croise, 1.6, th.encre);
    let part = 1.0 - phase(t, 2.2, 2.8);
    c.ligne(croise, pos2(215.0, 58.0), 1.6, fondu(if t > 1.9 { th.jade } else { th.encre }, part));
    let geste = Chemin::droite((190.0, 40.0), (196.0, 76.0));
    let s = doux(t, 1.2, 1.9);
    if t > 1.2 && t < 2.4 {
        c.polyligne(&geste.jusqu_a(s), 1.0, th.jade);
    }
    if t < 3.4 {
        let ici = if t < 1.2 { mixp(pos2(220.0, 140.0), geste.point(0.0).0, doux(t, 0.2, 1.1)) } else { geste.point(s).0 };
        c.curseur(ici, t > 1.2 && t < 1.9);
    }
}

// ------------------------------------------------------------------------------------------------------ Caméra

/// Paliers : le cadre va d'une clé à l'autre ; en haut, le mouvement doux, en bas, le même par paliers.
fn paliers(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let u = doux(t, 0.6, 4.4);
    let marches = 5.0;
    let u_palier = (u * marches).floor() / marches;
    for (y, v, col) in [(40.0, u, th.douce), (100.0, u_palier, th.jade)] {
        c.ligne(pos2(30.0, y), pos2(210.0, y), 0.8, th.forte);
        let x = mix(40.0, 200.0, v);
        c.cadre(Rect::from_center_size(pos2(x, y), vec2(34.0, 22.0)), 1.4, col);
    }
    // Les positions des paliers.
    for k in 0..=marches as usize {
        c.disque(pos2(mix(40.0, 200.0, k as f32 / marches), 100.0 + 16.0), 1.5, th.forte);
    }
    let (y, x1, x2) = (138.0, 40.0, 200.0);
    c.ligne(pos2(20.0, y), pos2(220.0, y), 1.0, th.forte);
    c.losange(pos2(x1, y), 4.0, th.jade);
    c.losange(pos2(x2, y), 4.0, th.jade);
    let tete = mix(x1, x2, u);
    c.ligne(pos2(tete, y - 7.0), pos2(tete, y + 7.0), 1.2, th.encre);
}

/// Effets de caméra : une plage d'images choisie sur la piste ; quand la tête de lecture y passe, le cadre tremble.
fn effets(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    decor(&c);
    let (y, a, b) = (136.0, 90.0, 160.0);
    c.ligne(pos2(20.0, y), pos2(220.0, y), 1.0, th.forte);
    // La plage choisie, à la souris.
    let choisie = doux(t, 0.4, 1.3);
    if choisie > 0.0 {
        c.pave(Rect::from_min_max(pos2(a, y - 5.0), pos2(mix(a, b, choisie), y + 5.0)), fondu(th.jade, 0.45));
    }
    let tete = mix(30.0, 210.0, phase(t, 1.6, 4.8));
    c.ligne(pos2(tete, y - 7.0), pos2(tete, y + 7.0), 1.2, th.encre);
    let dans = tete > a && tete < b;
    let tremble = if dans { vec2((t * 37.0).sin() * 3.0, (t * 29.0).cos() * 2.0) } else { vec2(0.0, 0.0) };
    c.cadre(Rect::from_center_size(pos2(118.0, 70.0) + tremble, vec2(150.0, 84.0)), 1.5, th.jade);
    if t < 1.5 {
        let ici = if t < 0.4 { mixp(pos2(200.0, 100.0), pos2(a, y), doux(t, 0.0, 0.35)) } else { pos2(mix(a, b, choisie), y) };
        c.curseur(ici, t > 0.4 && t < 1.3);
    }
}

// ------------------------------------------------------------------------------------------------------ Déformer

/// Le dessin à déformer : un bonhomme dans son cadre ; `f` place chaque point (de 0 à 1 dans le cadre, en x et en y).
fn dessin_deforme(c: &Toile, f: &dyn Fn(f32, f32) -> Pos2) {
    // Les quatre côtés, en points serrés pour qu'ils puissent se courber.
    let mut bord = Vec::new();
    for i in 0..=16 {
        bord.push(f(i as f32 / 16.0, 0.0));
    }
    for i in 0..=16 {
        bord.push(f(1.0, i as f32 / 16.0));
    }
    for i in 0..=16 {
        bord.push(f(1.0 - i as f32 / 16.0, 1.0));
    }
    for i in 0..=16 {
        bord.push(f(0.0, 1.0 - i as f32 / 16.0));
    }
    c.pointilles(&bord, 1.0, c.t.jade);
    // Un bonhomme.
    let tete: Vec<Pos2> = (0..=16).map(|i| {
        let a = i as f32 / 16.0 * std::f32::consts::TAU;
        f(0.5 + 0.1 * a.cos(), 0.25 + 0.1 * a.sin())
    }).collect();
    c.polyligne(&tete, 1.3, c.t.encre);
    for (a, b) in [((0.5, 0.35), (0.5, 0.68)), ((0.5, 0.42), (0.3, 0.6)), ((0.5, 0.42), (0.7, 0.6)), ((0.5, 0.68), (0.38, 0.95)), ((0.5, 0.68), (0.62, 0.95))] {
        let pts: Vec<Pos2> = (0..=8).map(|i| {
            let s = i as f32 / 8.0;
            f(mix(a.0, b.0, s), mix(a.1, b.1, s))
        }).collect();
        c.polyligne(&pts, 1.3, c.t.encre);
    }
}

/// L'enveloppe : le cadre du dessin a des poignées rondes au milieu des côtés ; en tirer une courbe le côté.
fn enveloppe(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let cadre = Rect::from_min_max(pos2(80.0, 30.0), pos2(160.0, 125.0));
    let bosse = 26.0 * doux(t, 0.9, 2.2) * (1.0 - doux(t, 3.6, 4.3));
    let f = |x: f32, y: f32| {
        let q = pos2(mix(cadre.left(), cadre.right(), x), mix(cadre.top(), cadre.bottom(), y));
        // Le côté droit se courbe vers la droite, de plus en plus près de son milieu ; l'intérieur suit.
        q + vec2(bosse * x * (std::f32::consts::PI * y).sin(), 0.0)
    };
    dessin_deforme(&c, &f);
    for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
        c.poignee(f(x, y), false);
    }
    let milieu = f(1.0, 0.5);
    for q in [f(0.5, 0.0), f(0.0, 0.5), f(0.5, 1.0)] {
        c.cercle(q, 3.0, 1.2, th.jade);
    }
    c.cercle(milieu, 3.0, 1.2, th.jade);
    let tenue = t > 0.9 && t < 2.2;
    if tenue {
        c.disque(milieu, 2.0, th.jade);
    }
    if t < 3.0 {
        let ici = if t < 0.9 { mixp(pos2(220.0, 140.0), f(1.0, 0.5), doux(t, 0.1, 0.85)) } else { milieu };
        c.curseur(ici, tenue);
    }
}

/// La perspective : tirer un coin du cadre met le dessin en perspective.
fn perspective(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let (a, b, cc, d) = (pos2(80.0, 30.0), pos2(160.0, 30.0), pos2(160.0, 125.0), pos2(80.0, 125.0));
    let s = doux(t, 0.9, 2.3) * (1.0 - doux(t, 3.4, 4.1));
    let b2 = b + vec2(-24.0, 22.0) * s;
    let c2 = cc + vec2(-24.0, -10.0) * s;
    let f = |x: f32, y: f32| {
        let haut = mixp(a, b2, x);
        let bas = mixp(d, c2, x);
        mixp(haut, bas, y)
    };
    dessin_deforme(&c, &f);
    for q in [a, b2, c2, d] {
        c.poignee(q, false);
    }
    if t < 2.8 {
        let ici = if t < 0.9 { mixp(pos2(220.0, 20.0), b, doux(t, 0.1, 0.85)) } else { b2 };
        c.curseur(ici, t > 0.9 && t < 2.3);
    }
}

// ------------------------------------------------------------------------------------------------------ Dégradé

/// Les pastilles sur la bande : un clic sous la bande ajoute une couleur ; glisser la déplace ; tirer vers le bas la supprime.
fn pastilles(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let bande = Rect::from_min_max(pos2(30.0, 60.0), pos2(210.0, 84.0));
    remplir_degrade(&c, bande, bande.left_center(), bande.right_center(), 1.0);
    c.cadre(bande, 1.0, th.forte);
    let pastille = |x: f32, y: f32, plein: Color32, tenue: bool| {
        let q = pos2(x, y);
        c.plein(&[q + vec2(0.0, -6.0), q + vec2(5.0, 0.0), q + vec2(5.0, 7.0), q + vec2(-5.0, 7.0), q + vec2(-5.0, 0.0)], plein);
        c.ferme(&[q + vec2(0.0, -6.0), q + vec2(5.0, 0.0), q + vec2(5.0, 7.0), q + vec2(-5.0, 7.0), q + vec2(-5.0, 0.0)], 1.2, if tenue { th.jade } else { th.forte });
    };
    pastille(bande.left(), 92.0, th.encre, false);
    pastille(bande.right(), 92.0, th.fond, false);
    // Le milieu, en losange au-dessus des pastilles.
    c.losange(pos2(120.0, 96.0), 3.0, th.forte);
    // La pastille neuve : posée au clic (1,0), glissée (1,6 → 2,6), tirée vers le bas et supprimée (3,2 → 4,0).
    let x = mix(90.0, 150.0, doux(t, 1.6, 2.6));
    let y = 92.0 + 34.0 * doux(t, 3.2, 4.0);
    let la = t > 1.0 && t < 4.0;
    if la {
        pastille(x, y, fondu(th.douce, 1.0 - phase(t, 3.7, 4.0)), true);
    }
    if t < 4.5 {
        let ici = if t < 1.0 { mixp(pos2(120.0, 140.0), pos2(90.0, 92.0), doux(t, 0.2, 0.9)) } else { pos2(x, y) };
        c.curseur(ici, (t > 0.95 && t < 1.1) || (t > 1.6 && t < 2.6) || (t > 3.2 && t < 4.0));
    }
}

// ------------------------------------------------------------------------------------------------------ Formes

/// Les formes : une droite (Maj : par 15°), une courbe, une courbe en S, une polyligne, chacune dans son coin.
fn formes(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let mut ici = pos2(120.0, 75.0);
    let mut appuye = false;
    // La droite, en haut à gauche, Maj tenue : un angle de 30°.
    let s = doux(t, 0.2, 1.2);
    let (a, b) = (pos2(20.0, 60.0), pos2(100.0, 60.0 - 80.0 * 0.577));
    if t > 0.2 {
        c.ligne(a, mixp(a, b, s), 1.6, th.encre);
    }
    c.touche(pos2(110.0, 66.0), "Maj", t > 0.2 && t < 1.3);
    if t < 1.4 {
        ici = mixp(a, b, s);
        appuye = t > 0.2 && t < 1.2;
    }
    // La courbe, en haut à droite : tracée, puis courbée par le pointeur.
    let (a2, b2) = (pos2(140.0, 55.0), pos2(225.0, 55.0));
    if t > 1.4 {
        let tire = doux(t, 1.4, 2.0);
        let courbe = doux(t, 2.1, 2.9) * -36.0;
        let fin = mixp(a2, b2, tire);
        let milieu = mixp(a2, fin, 0.5) + vec2(0.0, courbe * 2.0);
        let ch = Chemin::bezier((a2.x, a2.y), (milieu.x, milieu.y), (milieu.x, milieu.y), (fin.x, fin.y));
        c.polyligne(&ch.pts, 1.6, th.encre);
        if t < 3.0 {
            ici = if t < 2.1 { fin } else { mixp(a2, b2, 0.5) + vec2(0.0, courbe) };
            appuye = (t > 1.4 && t < 2.0) || (t > 2.1 && t < 2.9);
        }
    }
    // La courbe en S, en bas à gauche.
    if t > 3.0 {
        let s = doux(t, 3.0, 4.2);
        let ch = Chemin::bezier((20.0, 120.0), (45.0, 80.0 - 0.0), (75.0, 150.0), (100.0, 105.0));
        c.polyligne(&ch.jusqu_a(s), 1.6, th.encre);
        if t < 4.3 {
            ici = ch.point(s).0;
            appuye = t < 4.2;
        }
    }
    // La polyligne, en bas à droite : un clic par point.
    let points = [pos2(140.0, 130.0), pos2(160.0, 95.0), pos2(190.0, 120.0), pos2(220.0, 92.0)];
    if t > 4.3 {
        let n = (((t - 4.3) / 0.35).floor() as usize + 1).min(points.len());
        c.polyligne(&points[..n], 1.6, th.encre);
        for q in &points[..n] {
            c.disque(*q, 1.8, th.jade);
        }
        if t < 5.8 {
            ici = points[n - 1];
            appuye = (t - 4.3) % 0.35 < 0.12;
        }
    }
    if t < 5.8 {
        c.curseur(ici, appuye);
    }
}

// ------------------------------------------------------------------------------------------------------ Sculpter

/// Grab et Pull : un cercle prend les points du trait et les déplace, plus fort au centre ; puis un doigt étire un autre trait.
fn sculpter(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    // Grab, en haut.
    let base = Chemin::droite((20.0, 50.0), (220.0, 50.0));
    let centre = pos2(110.0, 50.0);
    let tire = vec2(0.0, -24.0) * doux(t, 0.7, 1.9) * (1.0 - doux(t, 4.9, 5.4));
    let pris = t > 0.6 && t < 2.0;
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let q = base.point(i as f32 / 60.0).0;
            let d = (q.distance(centre) / 26.0).min(1.0);
            q + tire * (1.0 - d * d)
        })
        .collect();
    c.polyligne(&pts, 1.6, th.encre);
    if pris {
        for q in pts.iter().filter(|q| q.distance(centre + tire) < 26.0) {
            c.disque(*q, 1.2, th.jade);
        }
    }
    // Pull, en bas : le trait suit le doigt qui glisse.
    let ch = Chemin::droite((20.0, 110.0), (220.0, 110.0));
    let s = doux(t, 2.6, 4.2);
    let doigt = pos2(mix(70.0, 170.0, s), 110.0 - 18.0 * (s * std::f32::consts::PI).sin());
    let pts2: Vec<Pos2> = (0..=60)
        .map(|i| {
            let q = ch.point(i as f32 / 60.0).0;
            let d = ((q.x - doigt.x).abs() / 30.0).min(1.0);
            let poids = if t > 2.6 { 1.0 - d * d } else { 0.0 };
            pos2(q.x, mix(q.y, doigt.y, poids * (1.0 - doux(t, 4.9, 5.4))))
        })
        .collect();
    c.polyligne(&pts2, 1.6, th.encre);
    let ici = if t < 0.6 { mixp(pos2(200.0, 140.0), centre, doux(t, 0.0, 0.55)) } else if t < 2.2 { centre + tire } else if t < 2.6 { mixp(centre + tire, pos2(70.0, 110.0), doux(t, 2.2, 2.55)) } else { doigt };
    if t < 4.6 {
        c.cercle(ici, 26.0, 1.0, th.forte);
        c.curseur(ici, pris || (t > 2.6 && t < 4.2));
    }
}

// ------------------------------------------------------------------------------------------------------ Autres outils

/// Le pot de peinture : un clic dans la zone fermée la remplit.
fn pot(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let zone: Vec<Pos2> = (0..32)
        .map(|i| {
            let a = i as f32 / 32.0 * std::f32::consts::TAU;
            pos2(120.0 + 60.0 * a.cos(), 75.0 + 40.0 * a.sin())
        })
        .collect();
    let rempli = phase(t, 1.2, 1.7);
    if rempli > 0.0 {
        c.plein(&zone, fondu(th.forte, rempli));
    }
    c.ferme(&zone, 1.6, th.encre);
    if t < 3.2 {
        let ici = mixp(pos2(220.0, 140.0), pos2(120.0, 80.0), doux(t, 0.2, 1.0));
        c.curseur(ici, t > 1.05 && t < 1.25);
    }
}

/// Le texte : un clic pose le curseur de texte, les lignes s'écrivent (des barres, sans mots), Ctrl+Entrée le pose.
fn texte(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let depart = pos2(50.0, 55.0);
    if t > 0.9 {
        // Les lignes de texte, comme des barres qui s'allongent.
        let longueurs = [110.0, 80.0];
        let ecrit = phase(t, 1.1, 3.0) * (longueurs[0] + longueurs[1]);
        let l1 = ecrit.min(longueurs[0]);
        let l2 = (ecrit - longueurs[0]).max(0.0);
        c.pave(Rect::from_min_size(depart + vec2(0.0, -6.0), vec2(l1, 8.0)), th.encre);
        if l2 > 0.0 {
            c.pave(Rect::from_min_size(depart + vec2(0.0, 12.0), vec2(l2, 8.0)), th.encre);
        }
        let pose = t > 3.4;
        if !pose {
            let caret = if l2 > 0.0 { depart + vec2(l2 + 2.0, 12.0) } else { depart + vec2(l1 + 2.0, -6.0) };
            c.ligne(caret, caret + vec2(0.0, 9.0), 1.2, th.jade);
            c.cadre_pointille(Rect::from_min_size(depart + vec2(-6.0, -12.0), vec2(124.0, 38.0)), 1.0, th.jade);
        }
    }
    c.touche(pos2(180.0, 128.0), "Ctrl+Entrée", t > 3.2 && t < 3.8);
    if t < 1.2 {
        c.curseur(mixp(pos2(200.0, 30.0), depart, doux(t, 0.1, 0.8)), t > 0.8 && t < 0.95);
    }
}

/// La pipette : Alt tenue, un clic sur une couleur du dessin, la couleur courante la prend.
fn pipette(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    let (a, b) = (pos2(80.0, 70.0), pos2(150.0, 70.0));
    tache(&c, a, 24.0, th.encre);
    tache(&c, b, 24.0, th.forte);
    // La couleur courante, en bas à droite.
    let courante = if t > 2.1 { th.forte } else { th.encre };
    let carre = Rect::from_min_size(pos2(200.0, 115.0), vec2(22.0, 22.0));
    c.pave(carre, courante);
    c.cadre(carre, 1.2, if t > 2.1 && t < 2.8 { th.jade } else { th.douce });
    c.touche(pos2(26.0, 18.0), "Alt", t > 1.2 && t < 2.4);
    if t < 3.4 {
        let ici = mixp(pos2(60.0, 130.0), b, doux(t, 0.3, 1.6));
        c.curseur(ici, t > 1.9 && t < 2.1);
    }
}

/// La main : Espace tenue, glisser déplace toute la vue.
fn main(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let fond = Toile::new(p, r, th);
    fond.fond();
    let decale = vec2(-40.0, 10.0) * doux(t, 1.0, 3.0);
    let pp = p.with_clip_rect(r.intersect(p.clip_rect()));
    let c = Toile::new(&pp, r.translate(decale * (r.width() / crate::aideanim::LARGEUR)), th);
    decor(&c);
    fond.touche(pos2(30.0, 18.0), "Espace", t > 0.7 && t < 3.2);
    if t < 3.8 {
        let ici = pos2(150.0, 70.0) + decale;
        fond.curseur(if t < 0.9 { mixp(pos2(220.0, 140.0), pos2(150.0, 70.0), doux(t, 0.0, 0.85)) } else { ici }, t > 1.0 && t < 3.0);
    }
}

// ------------------------------------------------------------------------------------------------------ Espace 3D

/// Le sol de l'Espace 3D vu sous un angle `a` (radians, autour de l'axe vertical) : le point du sol (`gx`, `gz`, de -1 à 1).
fn sol_tourne(gx: f32, gz: f32, a: f32) -> Pos2 {
    let (sn, cs) = a.sin_cos();
    let (x, z) = (gx * cs - gz * sn, gx * sn + gz * cs);
    sol(x, (z + 1.0) / 2.0).0
}

/// Plusieurs cartes choisies ensemble : deux cartes debout sur le sol, cernées de jade ; glisser l'une les emmène toutes les deux, du même
/// pas (elles grandissent en venant devant), puis elles reviennent.
fn e3_ensemble(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    for i in 0..=6 {
        let g = -1.0 + i as f32 / 3.0;
        c.ligne(sol_tourne(g, -1.0, 0.0), sol_tourne(g, 1.0, 0.0), 0.8, th.forte);
        c.ligne(sol_tourne(-1.0, g, 0.0), sol_tourne(1.0, g, 0.0), 0.8, th.forte);
    }
    let pas = doux(t, 0.9, 2.8) * (1.0 - doux(t, 3.6, 4.5));
    let decale = (0.35 * pas, -0.6 * pas);
    let mut prise = pos2(0.0, 0.0);
    for (k, (gx, gz)) in [(-0.5, 0.4), (0.2, 0.1)].into_iter().enumerate() {
        let pied = sol_tourne(gx + decale.0, gz + decale.1, 0.0);
        // (L'éloignement, lu sur la hauteur du pied : `sol` place un point à 52 + 96 / d.)
        let d = (96.0 / (pied.y - 52.0).max(1.0)).clamp(1.0, 4.0);
        let centre = pos2(pied.x, pied.y - 20.0 / d);
        c.carte(centre, 28.0 / d, 38.0 / d, true);
        if k == 0 {
            prise = centre;
        }
    }
    if t < 3.4 {
        let ici = if t < 0.9 { mixp(pos2(30.0, 130.0), prise, doux(t, 0.1, 0.85)) } else { prise };
        c.curseur(ici, t > 0.9 && t < 2.8);
    }
}

/// Une carte et ses axes de rotation : un anneau en travers de la carte ; le tirer la tourne (elle s'amincit de profil).
fn e3_carte(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    grille_sol(&c);
    let a = 1.0 * doux(t, 1.0, 3.2) * (1.0 - doux(t, 3.8, 4.5));
    let centre = pos2(120.0, 70.0);
    let (w, h) = (54.0 * a.cos().abs().max(0.12), 70.0);
    c.carte(centre, w, h, true);
    // L'anneau de l'axe vertical : une ellipse aplatie autour de la carte.
    let anneau: Vec<Pos2> = (0..=40).map(|i| {
        let u = i as f32 / 40.0 * std::f32::consts::TAU;
        centre + vec2(44.0 * u.cos(), 12.0 * u.sin()) + vec2(0.0, 38.0)
    }).collect();
    c.polyligne(&anneau, 1.2, th.jade);
    let prise = centre + vec2(44.0 * (a + 0.4).cos(), 12.0 * (a + 0.4).sin() + 38.0);
    c.disque(prise, 3.0, th.jade);
    if t < 4.0 {
        c.curseur(if t < 1.0 { mixp(pos2(220.0, 30.0), prise, doux(t, 0.1, 0.9)) } else { prise }, t > 1.0 && t < 3.2);
    }
}

/// La caméra libre : la caméra tourne tout autour des cartes, passe derrière, et l'image filmée la suit.
fn e3_camera(p: &Painter, r: Rect, t: f32, th: &Teintes) {
    let c = Toile::new(p, r, th);
    c.fond();
    // Vu de dessus : deux cartes au centre, la caméra fait le tour.
    let centre = pos2(90.0, 78.0);
    c.ligne(centre + vec2(-14.0, -6.0), centre + vec2(10.0, -6.0), 2.0, th.encre);
    c.ligne(centre + vec2(-6.0, 10.0), centre + vec2(16.0, 10.0), 2.0, th.douce);
    let a = std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * 0.75 * doux(t, 0.6, 4.4);
    let cam = centre + vec2(a.cos(), a.sin()) * 52.0;
    let vers = (centre - cam).normalized();
    let cote = vec2(-vers.y, vers.x);
    c.ferme(&[cam - vers * 6.0 + cote * 6.0, cam - vers * 6.0 - cote * 6.0, cam + vers * 6.0 - cote * 3.0, cam + vers * 6.0 + cote * 3.0], 1.4, th.jade);
    c.ligne(cam + vers * 6.0 + cote * 3.0, cam + vers * 40.0 + cote * 20.0, 0.8, fondu(th.jade, 0.6));
    c.ligne(cam + vers * 6.0 - cote * 3.0, cam + vers * 40.0 - cote * 20.0, 0.8, fondu(th.jade, 0.6));
    c.cercle(centre, 52.0, 0.8, th.forte);
    // L'image filmée, à droite : l'ordre des cartes s'inverse quand la caméra passe derrière.
    let image = Rect::from_min_max(pos2(170.0, 45.0), pos2(228.0, 90.0));
    c.cadre(image, 1.2, th.jade);
    let devant = a.sin() > 0.0;
    let x = image.center().x + 14.0 * a.cos();
    let (premiere, seconde) = if devant { (th.encre, th.douce) } else { (th.douce, th.encre) };
    c.pave(Rect::from_center_size(pos2(x - 6.0, image.center().y), vec2(16.0, 24.0)), seconde);
    c.pave(Rect::from_center_size(pos2(x + 6.0, image.center().y + 4.0), vec2(16.0, 24.0)), premiere);
}
