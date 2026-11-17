//! Retour d'Anthony, 2026-11-10, sur son projet « afa » (format 52) après la fusion de « La caméra se souvient » (partie 131) : « la planche
//! ne prend plus le trait, j'ai beau essayer de dessiner mais rien n'apparaît sauf à certains endroits hors des cases, mais le trait disparaît
//! immédiatement après avoir lâché le stylet ». Rejoué sur une copie de son projet : le chemin de son manifeste dans `FEANOR_PROJET_ESSAI`
//! (sans elle, l'essai ne fait rien). Les captures vont dans `FEANOR_CAPTURE_DIR`.

use eframe::egui::{pos2, Event, Modifiers, Pos2};

use crate::document::Document;
use crate::perf_tests::Banc;
use crate::traits_continus_tests::{des_plans, planche, relever, repos, sauver, tracer};
use crate::vignettes_tests::appui;

fn projet() -> Option<Document> {
    let p = std::env::var_os("FEANOR_PROJET_ESSAI")?;
    let l = crate::project::load(std::path::Path::new(&p)).expect("le projet s'ouvre");
    println!("ouverture : {} avertissement(s) {:?}, {} dessin(s) manquant(s)", l.warnings.len(), l.warnings, l.manquants.len());
    Some(l.doc)
}

/// (éléments vectoriels, tuiles bitmap dessinées) des dessins du plan `i`.
fn dessins(doc: &Document, i: usize) -> (usize, usize) {
    let mut vus = Vec::new();
    let (mut v, mut t) = (0, 0);
    for id in doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        if vus.contains(id) {
            continue;
        }
        vus.push(*id);
        if let Some(c) = doc.cels.get(id) {
            v += c.vector.as_ref().map_or(0, |d| d.elements.len());
            t += c.raster.drawn_ids().into_iter().count() + c.raster.outer.len();
            // (La peinture de l'aplat sous le trait compte aussi.)
            t += c.fond.as_ref().map_or(0, |s| s.peinture.raster.drawn_ids().into_iter().count() + s.peinture.raster.outer.len());
        }
    }
    (v, t)
}

/// Ce que contient une étape d'annulation, en bref.
fn resume(e: &crate::history::Edit) -> String {
    use crate::history::Edit;
    match e {
        Edit::Group(v) => format!("Groupe[{}]", v.iter().map(resume).collect::<Vec<_>>().join(", ")),
        Edit::FondTiles { tiles, coupe, .. } => {
            let pleines = tiles.iter().filter(|(_, t)| t.as_ref().is_some_and(|t| t.iter().any(|c| c.a() > 0))).count();
            format!("FondTiles({} tuiles dont {pleines} peintes avant, coupe {coupe})", tiles.len())
        }
        Edit::Tiles { tiles, .. } => format!("Tiles({})", tiles.len()),
        _ => "autre".into(),
    }
}

/// Les pixels peints dans l'aplat sous le trait des dessins du plan `i`.
fn peinture_aplat(doc: &Document, i: usize) -> usize {
    let mut vus = Vec::new();
    let mut n = 0;
    for id in doc.panels[i].layers.iter().flat_map(|l| l.frames.iter().flatten()) {
        if vus.contains(id) {
            continue;
        }
        vus.push(*id);
        if let Some(s) = doc.cels.get(id).and_then(|c| c.fond.as_ref()) {
            n += s.peinture.raster.to_flat().iter().filter(|c| c.a() > 0).count();
        }
    }
    n
}

/// (traits du calque « Hors cadres », morceaux de la couche bitmap de la planche).
fn papier(b: &Banc) -> (usize, usize) {
    let a = &b.app.doc.annotations;
    (a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count(), a.bitmap.morceaux.len())
}

fn calque_courant(b: &Banc) -> String {
    b.app.layers().get(b.app.cur_layer).map(|l| format!("« {} » ({:?})", l.name, l.kind)).unwrap_or_else(|| "aucun".into())
}

/// Un trait droit, point par point, avec une capture juste avant le lâcher et une juste après.
fn tracer_et_voir(b: &mut Banc, de: Pos2, a: Pos2, nom: &str) {
    let n = ((a - de).length() / 6.0).ceil().max(2.0) as usize;
    b.image(vec![Event::PointerMoved(de)], 1.0 / 60.0);
    b.image(vec![appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=n {
        b.image(vec![Event::PointerMoved(de + (a - de) * (k as f32 / n as f32))], 1.0 / 120.0);
    }
    let pendant = b.capturer().unwrap();
    sauver(&pendant, &format!("{nom}-pendant"));
    println!("  pendant le geste : {:?}", relever(&pendant, de, a));
    b.image(vec![appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
    let lache = b.capturer().unwrap();
    sauver(&lache, &format!("{nom}-lache"));
    println!("  juste après le lâcher : {:?}", relever(&lache, de, a));
}

#[test]
fn anthony_afa_la_planche_prend_le_trait() {
    let Some(doc) = projet() else { return println!("FEANOR_PROJET_ESSAI non défini : rien à rejouer") };
    let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.brush.size = 44.0;
    repos(&mut b);
    sauver(&b.capturer().unwrap(), "afa-avant");
    let l = b.app.board_layout();
    println!(
        "{} cases ; plan courant {} ; calque courant {} ; hors cadres montré : {:?} ; zoom {}",
        l.cards.len(),
        b.app.cur_panel,
        calque_courant(&b),
        b.app.doc.annotations.hors_cadres_montre(),
        b.app.board_zoom
    );
    for k in 0..l.cards.len().min(9) {
        let c = &l.cards[k];
        println!("  case {k} : plan {} (id {}) image {} rect écran {:?}", c.panel, c.id, c.frame, b.app.vignettes.xf.rect(c.thumb));
    }
    for k in 0..7usize {
        let l = b.app.board_layout();
        let Some(c) = l.cards.get(k) else { break };
        let (i, r) = (c.panel, b.app.vignettes.xf.rect(c.thumb));
        let y = r.top() + r.height() * 0.35;
        let (de, a) = (pos2(r.left() + r.width() * 0.2, y), pos2(r.right() - r.width() * 0.2, y));
        let (d0, p0, e0) = (dessins(&b.app.doc, i), papier(&b), b.app.history.undo_len());
        println!("case {k} (plan {i}) : de {de:?} à {a:?}, calque courant {}", calque_courant(&b));
        tracer_et_voir(&mut b, de, a, &format!("afa-case-{k}"));
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("afa-case-{k}-repos"));
        println!("  au repos : {:?}", relever(&rgba, de, a));
        println!(
            "  dessins du plan {:?} -> {:?} ; papier {:?} -> {:?} ; étapes {} -> {} ; calque courant {} ; état « {} »",
            d0,
            dessins(&b.app.doc, i),
            p0,
            papier(&b),
            e0,
            b.app.history.undo_len(),
            calque_courant(&b),
            b.app.status
        );
    }
    // Le papier : en haut de la vue, loin des cases, puis juste à droite de la première case.
    let view = b.app.vignettes.view;
    let l = b.app.board_layout();
    let r0 = b.app.vignettes.xf.rect(l.cards[0].thumb);
    let essais = [
        ("papier-haut", pos2(view.left() + 40.0, view.top() + 30.0), pos2(view.left() + 300.0, view.top() + 30.0)),
        ("papier-droite", pos2(r0.right() + 6.0, r0.top() + r0.height() * 0.5), pos2(r0.right() + 6.0, r0.bottom() + r0.height() * 0.5)),
    ];
    for (nom, de, a) in essais {
        let (p0, e0) = (papier(&b), b.app.history.undo_len());
        println!("{nom} : de {de:?} à {a:?}");
        tracer_et_voir(&mut b, de, a, &format!("afa-{nom}"));
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("afa-{nom}-repos"));
        println!("  au repos : {:?}", relever(&rgba, de, a));
        println!("  papier {:?} -> {:?} ; étapes {} -> {} ; état « {} »", p0, papier(&b), e0, b.app.history.undo_len(), b.app.status);
    }
}

/// L'hypothèse : l'Aplat (le fond, `fond.rs`) est la cible du dessin. Le pinceau ne peint alors que dans la forme déduite d'un trait déjà
/// là ; là où il n'y en a pas, rien n'est posé et le trait est jeté au lâcher (« Aplat : dessine d'abord le trait… »). On rejoue ce cas sur
/// la copie de son projet : un calque à fond rendu courant, l'Aplat visé.
#[test]
fn anthony_afa_aplat_vise_la_planche_ne_prend_plus_le_trait() {
    let Some(doc) = projet() else { return println!("FEANOR_PROJET_ESSAI non défini : rien à rejouer") };
    let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.brush.size = 44.0;
    // Le premier plan dont un calque a un fond (comme dans son projet : Perso 3, Perso 2…).
    let mut vise = None;
    'rech: for (pi, p) in b.app.doc.panels.iter().enumerate() {
        for (li, l) in p.layers.iter().enumerate() {
            if l.fond.is_some() {
                vise = Some((pi, li, l.id, l.name.clone()));
                break 'rech;
            }
        }
    }
    let Some((pi, li, lid, lnom)) = vise else { return println!("aucun calque à fond dans ce projet") };
    b.app.select_board(pi, 0);
    b.app.cur_layer = li;
    b.app.aplat_cible = Some(lid);
    repos(&mut b);
    println!("plan {pi} calque « {lnom} » (id {lid}) ; aplat visé : {} ; calque courant {}", b.app.aplat_en_cible(), calque_courant(&b));
    // La case de ce plan, puis le papier.
    let l = b.app.board_layout();
    let k = l.cards.iter().position(|c| c.panel == pi).unwrap_or(0);
    let r = b.app.vignettes.xf.rect(l.cards[k].thumb);
    let y = r.top() + r.height() * 0.6;
    let (d0, e0) = (dessins(&b.app.doc, pi), b.app.history.undo_len());
    println!("case {k} (plan {pi}) : dessin par-dessus le personnage");
    stylet_et_voir(&mut b, pos2(r.left() + r.width() * 0.2, y), pos2(r.right() - r.width() * 0.2, y), "aplat-case");
    repos(&mut b);
    println!("  dessins du plan {:?} -> {:?} ; étapes {} -> {} ; aplat visé {} ; état « {} »", d0, dessins(&b.app.doc, pi), e0, b.app.history.undo_len(), b.app.aplat_en_cible(), b.app.status);
    // Le papier, loin de toute case.
    let view = b.app.vignettes.view;
    let (p0, e1) = (papier(&b), b.app.history.undo_len());
    println!("papier, loin des cases :");
    stylet_et_voir(&mut b, pos2(view.right() - 420.0, view.top() + 40.0), pos2(view.right() - 200.0, view.top() + 90.0), "aplat-papier");
    repos(&mut b);
    println!("  papier {:?} -> {:?} ; étapes {} -> {} ; aplat visé {} ; état « {} »", p0, papier(&b), e1, b.app.history.undo_len(), b.app.aplat_en_cible(), b.app.status);
    // Et une fois l'Aplat relâché (clic sur le trait) : la case reprend-elle le trait ?
    b.app.aplat_cible = None;
    repos(&mut b);
    let (d1, e2) = (dessins(&b.app.doc, pi), b.app.history.undo_len());
    println!("Aplat relâché, même case :");
    stylet_et_voir(&mut b, pos2(r.left() + r.width() * 0.2, y + 12.0), pos2(r.right() - r.width() * 0.2, y + 12.0), "aplat-relache-case");
    repos(&mut b);
    println!("  dessins du plan {:?} -> {:?} ; étapes {} -> {} ; aplat visé {} ; état « {} »", d1, dessins(&b.app.doc, pi), e2, b.app.history.undo_len(), b.app.aplat_en_cible(), b.app.status);
}

/// Ses vidéos : sur le papier, **loin des cases**, le trait se voit pendant le geste et disparaît au lâcher. On le pose de plus en plus loin
/// de la planche (en largeurs de case), à droite de tout ce qu'elle montre, cases pâles comprises.
#[test]
fn anthony_afa_loin_des_cases() {
    let Some(doc) = projet() else { return println!("FEANOR_PROJET_ESSAI non défini : rien à rejouer") };
    let Some(mut b) = planche(doc, 6, 0.45) else { return println!("pas de carte graphique") };
    b.app.brush.size = 44.0;
    repos(&mut b);
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let case = xf.rect(l.cards[0].thumb);
    let droite = l.cards.iter().map(|c| xf.rect(c.thumb).right()).chain(l.empty.iter().map(|r| xf.rect(*r).right())).fold(f32::MIN, f32::max);
    let view = b.app.vignettes.view;
    println!("case {:.0} px de large ; planche jusqu'à x = {droite:.0} ; vue jusqu'à x = {:.0}", case.width(), view.right());
    sauver(&b.capturer().unwrap(), "loin-avant");
    for d in [1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0] {
        let x = droite + d * case.width();
        if x + 10.0 > view.right() {
            println!("à {d} largeurs : hors de la vue");
            continue;
        }
        let (de, a) = (pos2(x, case.top()), pos2(x + 8.0, case.bottom() + case.height()));
        let (p0, e0) = (papier(&b), b.app.history.undo_len());
        let nom = format!("loin-{d}");
        stylet_et_voir(&mut b, de, a, &nom);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("{nom}-repos"));
        let r = relever(&rgba, de, a);
        println!(
            "à {d} largeurs de case : papier {:?} -> {:?} ; étapes {} -> {} ; au repos trou {} sur {} ; état « {} »",
            p0,
            papier(&b),
            e0,
            b.app.history.undo_len(),
            r.trou,
            r.longueur,
            b.app.status
        );
    }
}

/// L'hypothèse qui explique tout : « Peint sur l'aplat » allumé. Chaque trait du pinceau va dans l'aplat (créé s'il n'y en a pas), là où le
/// dessin a déjà un trait ; ailleurs, rien. Une case pâle naît, vide ; le papier montre le trait pendant le geste, puis plus rien.
#[test]
fn anthony_afa_peint_sur_l_aplat() {
    let Some(doc) = projet() else { return println!("FEANOR_PROJET_ESSAI non défini : rien à rejouer") };
    let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.brush.size = 44.0;
    b.app.paint_on_aplat = true;
    repos(&mut b);
    let n0 = b.app.doc.panels.len();
    // Les cases 0, 3, 4 (un trait par-dessus leur dessin), puis la première case pâle, puis le papier.
    for k in [0usize, 3, 4] {
        let l = b.app.board_layout();
        let (i, r) = (l.cards[k].panel, b.app.vignettes.xf.rect(l.cards[k].thumb));
        let y = r.top() + r.height() * 0.6;
        let (d0, e0) = (dessins(&b.app.doc, i), b.app.history.undo_len());
        println!("case {k} (plan {i}) : peinture d'aplat du plan avant {} px", peinture_aplat(&b.app.doc, i));
        stylet_et_voir(&mut b, pos2(r.left() + r.width() * 0.2, y), pos2(r.right() - r.width() * 0.2, y), &format!("surplat-case-{k}"));
        repos(&mut b);
        println!("  dessins du plan {:?} -> {:?} ; étapes {} -> {} ; état « {} »", d0, dessins(&b.app.doc, i), e0, b.app.history.undo_len(), b.app.status);
        println!("  étapes : {:?} ; peinture d'aplat du plan {} px ; calque courant {}", b.app.history.names().0.iter().rev().take(3).collect::<Vec<_>>(), peinture_aplat(&b.app.doc, i), calque_courant(&b));
        println!("  contenu des étapes : {:?}", b.app.history.etapes().map(resume).collect::<Vec<_>>());
        // Partie 133 : rien n'est créé pour un trait qui ne laisse rien (avant : un aplat et un dessin vide, deux étapes).
        if b.app.history.undo_len() == e0 {
            assert!(b.app.status == crate::aplatsur::SANS_TRAIT || b.app.status == crate::aplatsur::HORS_DE_LA_FORME, "{}", b.app.status);
        } else {
            assert_ne!(dessins(&b.app.doc, i), d0, "une étape, c'est que le trait a laissé quelque chose");
        }
    }
    let l = b.app.board_layout();
    if let Some(e) = l.empty.first() {
        let r = b.app.vignettes.xf.rect(*e);
        let y = r.top() + r.height() * 0.5;
        let e0 = b.app.history.undo_len();
        println!("première case pâle :");
        stylet_et_voir(&mut b, pos2(r.left() + r.width() * 0.2, y), pos2(r.right() - r.width() * 0.2, y), "surplat-pale");
        repos(&mut b);
        let n = b.app.doc.panels.len();
        println!("  plans {n0} -> {n} ; dessins du plan neuf {:?} ; étapes {} -> {} ; état « {} »", dessins(&b.app.doc, n - 1), e0, b.app.history.undo_len(), b.app.status);
        assert_eq!(n, n0, "partie 133 : pas de plan vide");
        assert_eq!(b.app.history.undo_len(), e0);
    }
    let view = b.app.vignettes.view;
    let (p0, e0) = (papier(&b), b.app.history.undo_len());
    println!("papier :");
    stylet_et_voir(&mut b, pos2(view.right() - 420.0, view.top() + 40.0), pos2(view.right() - 200.0, view.top() + 90.0), "surplat-papier");
    repos(&mut b);
    let rgba = b.capturer().unwrap();
    sauver(&rgba, "surplat-papier-repos");
    println!("  au repos : {:?}", relever(&rgba, pos2(view.right() - 420.0, view.top() + 40.0), pos2(view.right() - 200.0, view.top() + 90.0)));
    println!("  papier {:?} -> {:?} ; étapes {} -> {} ; état « {} »", p0, papier(&b), e0, b.app.history.undo_len(), b.app.status);
    assert_eq!(papier(&b).0 + papier(&b).1, p0.0 + p0.1 + 1, "partie 133 : le trait posé sur le papier y reste");
    assert_eq!(b.app.history.undo_len(), e0 + 1);
}

// ------------------------------------------------------------------------------ partie 133, sans son projet

/// Un point du papier, loin de toute case : le coin haut gauche de la vue (vérifié).
fn coin_du_papier(b: &Banc) -> Pos2 {
    let view = b.app.vignettes.view;
    let a = pos2(view.left() + 6.0, view.top() + 6.0);
    let l = b.app.board_layout();
    let p = b.app.vignettes.xf.to_board(a);
    assert!(!crate::horschamp::champs(&l, b.app.settings.vignettes.debord_fraction()).iter().any(|c| c.contains(p)), "le coin est hors de tout champ");
    a
}

fn aplats(b: &Banc) -> usize {
    b.app.doc.panels.iter().flat_map(|p| p.layers.iter()).filter(|l| l.fond.is_some()).count()
}

/// « Peint sur l'aplat » allumé : un trait posé sur le papier va au papier, entier, comme d'habitude (le papier n'a pas d'aplat). Avant, il
/// se voyait pendant le geste et disparaissait au lâcher.
#[test]
fn with_paint_on_aplat_a_stroke_put_down_on_the_paper_stays_on_the_paper() {
    for bitmap in [false, true] {
        let Some(mut b) = planche(des_plans(6, bitmap), 6, 1.0) else { return println!("pas de carte graphique") };
        let sorte = if bitmap { "bitmap" } else { "vecteur" };
        b.app.paint_on_aplat = true;
        let l = b.app.board_layout();
        let (de, a) = (coin_du_papier(&b), b.app.vignettes.xf.rect(l.cards[1].thumb).center());
        let (etapes, p0, f0) = (b.app.history.undo_len(), papier(&b), aplats(&b));
        tracer(&mut b, de, a);
        repos(&mut b);
        let p1 = papier(&b);
        assert_eq!(p1.0 + p1.1, p0.0 + p0.1 + 1, "{sorte} : le trait est sur le papier ({})", b.app.status);
        assert_eq!(b.app.history.undo_len(), etapes + 1, "{sorte} : une étape");
        assert_eq!(aplats(&b), f0, "{sorte} : aucun aplat créé");
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("sur-aplat-papier-{sorte}"));
        crate::traits_continus_tests::continu(&rgba, de, a, &format!("le papier, « Peint sur l'aplat » allumé, {sorte}"));
    }
}

/// « Peint sur l'aplat » allumé : dans une case sans trait, rien ne reste, donc rien n'est créé (ni aplat, ni étape), et la barre d'état le
/// dit ; poser le stylet dans une case pâle ne fait plus naître un plan vide. Éteint, tout dessine.
#[test]
fn with_paint_on_aplat_an_empty_case_or_a_pale_case_gets_nothing_and_the_status_says_why() {
    let Some(mut b) = planche(des_plans(2, false), 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.paint_on_aplat = true;
    let l = b.app.board_layout();
    let xf = b.app.vignettes.xf;
    let (c0, pale) = (xf.rect(l.cards[0].thumb), xf.rect(*l.empty.first().expect("une case pâle")));
    let dans = |r: eframe::egui::Rect| (pos2(r.left() + r.width() * 0.2, r.center().y), pos2(r.right() - r.width() * 0.2, r.center().y));
    // Une case dessinée, sans trait à cette image.
    let (etapes, f0) = (b.app.history.undo_len(), aplats(&b));
    let (de, a) = dans(c0);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.history.undo_len(), etapes, "pas d'étape");
    assert_eq!(aplats(&b), f0, "pas d'aplat créé pour rien");
    assert!(!crate::vignettes_tests::encre(&b.app.doc, 0));
    assert_eq!(b.app.status, crate::aplatsur::SANS_TRAIT);
    // Une case pâle : pas de plan vide.
    let (de, a) = dans(pale);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 2, "pas de plan vide");
    assert_eq!(b.app.history.undo_len(), etapes, "pas d'étape");
    assert_eq!(b.app.status, crate::aplatsur::SANS_TRAIT);
    // Éteint : la case dessine, la case pâle fait son plan.
    b.app.paint_on_aplat = false;
    let (de, a) = dans(c0);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert!(crate::vignettes_tests::encre(&b.app.doc, 0), "éteint, le trait est dessiné");
    let (de, a) = dans(pale);
    tracer(&mut b, de, a);
    repos(&mut b);
    assert_eq!(b.app.doc.panels.len(), 3, "éteint, la case pâle fait son plan");
}

/// Un trait droit au stylet (événements tactiles avec leur appui, puis la souris que le système y ajoute), capture avant et après le lâcher.
fn stylet_et_voir(b: &mut Banc, de: Pos2, a: Pos2, nom: &str) {
    use eframe::egui::{TouchDeviceId, TouchId, TouchPhase};
    let touch = |phase, p: Pos2| Event::Touch { device_id: TouchDeviceId(0), id: TouchId(0), phase, pos: p, force: Some(0.6) };
    let n = ((a - de).length() / 6.0).ceil().max(2.0) as usize;
    b.image(vec![touch(TouchPhase::Start, de), Event::PointerMoved(de), appui(de, true, Modifiers::NONE)], 1.0 / 60.0);
    for k in 1..=n {
        let p = de + (a - de) * (k as f32 / n as f32);
        b.image(vec![touch(TouchPhase::Move, p), Event::PointerMoved(p)], 1.0 / 120.0);
    }
    let pendant = b.capturer().unwrap();
    sauver(&pendant, &format!("{nom}-pendant"));
    println!("  pendant le geste : {:?}", relever(&pendant, de, a));
    b.image(vec![touch(TouchPhase::End, a), appui(a, false, Modifiers::NONE)], 1.0 / 60.0);
    let lache = b.capturer().unwrap();
    sauver(&lache, &format!("{nom}-lache"));
    println!("  juste après le lâcher : {:?}", relever(&lache, de, a));
}

fn traces_nouvelles(b: &Banc, depuis: usize) -> String {
    let a = &b.app.doc.annotations;
    a.traces.iter().skip(depuis).map(|t| format!("[dessus {} ancre {:?} brosse {} calque {:?} points {}]", t.dessus, t.anchor, t.brosse.is_some(), t.calque, t.points.len())).collect::<Vec<_>>().join(" ")
}

/// Ses réglages du 10 novembre : passe-partout des cases coché, à l'opacité 0 ; le stylet.
#[test]
fn anthony_afa_au_stylet_avec_passe_partout() {
    let Some(doc) = projet() else { return println!("FEANOR_PROJET_ESSAI non défini : rien à rejouer") };
    let Some(mut b) = planche(doc, 6, 1.0) else { return println!("pas de carte graphique") };
    b.app.brush.size = 44.0;
    b.app.settings.vignettes.passe_partout_cases = true;
    b.app.settings.vignettes.passe_partout_cases_opacite = 0;
    b.app.vignettes.annot.marie = true;
    repos(&mut b);
    sauver(&b.capturer().unwrap(), "pp-avant");
    let l = b.app.board_layout();
    let ann = l.annulees.first().map(|c| b.app.vignettes.xf.rect(c.thumb));
    println!("{} cases, {} annulée(s) {:?} ; film {} ; calque courant {}", l.cards.len(), l.annulees.len(), ann, b.app.vignettes.annot.marie, calque_courant(&b));
    for k in 0..4usize {
        let l = b.app.board_layout();
        let Some(c) = l.cards.get(k) else { break };
        let (i, r) = (c.panel, b.app.vignettes.xf.rect(c.thumb));
        let y = r.top() + r.height() * 0.6;
        let (de, a) = (pos2(r.left() + r.width() * 0.2, y), pos2(r.right() - r.width() * 0.2, y));
        let (d0, p0, e0, t0) = (dessins(&b.app.doc, i), papier(&b), b.app.history.undo_len(), b.app.doc.annotations.traces.len());
        println!("pp case {k} (plan {i}) : de {de:?} à {a:?}, calque courant {}", calque_courant(&b));
        stylet_et_voir(&mut b, de, a, &format!("pp-case-{k}"));
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("pp-case-{k}-repos"));
        println!("  au repos : {:?}", relever(&rgba, de, a));
        println!(
            "  dessins du plan {:?} -> {:?} ; papier {:?} -> {:?} ; étapes {} -> {} ; calque courant {} ; état « {} » ; traces {}",
            d0,
            dessins(&b.app.doc, i),
            p0,
            papier(&b),
            e0,
            b.app.history.undo_len(),
            calque_courant(&b),
            b.app.status,
            traces_nouvelles(&b, t0)
        );
    }
    let view = b.app.vignettes.view;
    let mut essais = vec![("pp-papier-haut-droite", pos2(view.right() - 400.0, view.top() + 40.0), pos2(view.right() - 200.0, view.top() + 90.0))];
    if let Some(r) = ann {
        essais.push(("pp-papier-vers-annulee", pos2(r.center().x - 20.0, r.top() - 70.0), pos2(r.center().x + 30.0, r.bottom() + 15.0)));
        essais.push(("pp-papier-sous-annulee", pos2(r.left(), r.bottom() + 40.0), pos2(r.right(), r.bottom() + 60.0)));
    }
    for (nom, de, a) in essais {
        let (p0, e0, t0) = (papier(&b), b.app.history.undo_len(), b.app.doc.annotations.traces.len());
        println!("{nom} : de {de:?} à {a:?}");
        stylet_et_voir(&mut b, de, a, nom);
        repos(&mut b);
        let rgba = b.capturer().unwrap();
        sauver(&rgba, &format!("{nom}-repos"));
        println!("  au repos : {:?}", relever(&rgba, de, a));
        println!("  papier {:?} -> {:?} ; étapes {} -> {} ; état « {} » ; traces {}", p0, papier(&b), e0, b.app.history.undo_len(), b.app.status, traces_nouvelles(&b, t0));
    }
}
