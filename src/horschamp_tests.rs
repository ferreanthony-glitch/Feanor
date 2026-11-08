//! Essais des ajustements des Vignettes du 2026-10-30 : la continuité tout de suite et sans cascade, tout le papier dessinable (ce qui est hors
//! du champ d'une case reste sur la planche), les numéros, la pastille de version hors cadre, les lignes de texte, les réglages, le zoom
//! d'ouverture, la barre du bas et le passe-partout des cases.

use eframe::egui::{pos2, vec2, Modifiers, Pos2, Rect};

use crate::document::{Document, LayerKind};
use crate::horschamp::{champs, proprietaire, trancher, Pt};
use crate::perf_tests::Banc;
use crate::vector::VPoint;
use crate::vignettes_tests::{barre, case_vide, clic_avec, dernier_nom, encre, glisser, ouvert, pas_d_annulation, quatre_plans, vague, vignette};

fn coupes(b: &Banc) -> Vec<bool> {
    let l = b.app.board_layout();
    l.bars.iter().take(l.cards.len() - 1).map(|x| x.cut).collect()
}

fn rien_dans_les_plans(b: &Banc) -> bool {
    (0..b.app.doc.panels.len()).all(|i| !encre(&b.app.doc, i))
}

fn en_bitmap(mut doc: Document) -> Document {
    for p in &mut doc.panels {
        for l in &mut p.layers {
            l.kind = LayerKind::Bitmap;
        }
    }
    doc
}

// ------------------------------------------------------------------------------ la continuité

#[test]
fn clicking_the_bar_before_the_empty_case_makes_the_continuity_case_at_once_and_the_next_case_stays_a_cut() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 5);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    let last = b.app.doc.panels.len() - 1;
    let (frames, marks) = (b.app.doc.panels[last].frame_count(), b.app.doc.panels[last].marks.len());
    let n = pas_d_annulation(&b);
    assert!(b.app.board_layout().bars.last().unwrap().cut, "la barre devant la case vide est noire");
    let zone = barre(&b, 5);
    clic_avec(&mut b, zone.center(), Modifiers::NONE);
    // Sans trait : la case est faite, avec la copie du dessin d'avant.
    assert_eq!(b.app.doc.panels.len(), 4, "une image de plus, pas un plan de plus");
    let p = &b.app.doc.panels[last];
    assert_eq!(p.marks.len(), marks + 1);
    assert!(p.frame_count() > frames);
    let debut = p.marks.last().unwrap().frame;
    assert!(p.layers.iter().any(|l| l.cel_at(debut).is_some_and(|c| b.app.doc.cel_a_du_trait(c))), "la case montre déjà le dessin hérité");
    assert_eq!(pas_d_annulation(&b), n + 1);
    assert_eq!(dernier_nom(&b), "Continuité avec le plan précédent");
    // Pas de cascade : la case vide d'après est une coupe, et dessiner dedans fait un plan.
    assert!(b.app.board_layout().bars.last().unwrap().cut, "la suite reste une coupe");
    assert!(!b.app.vignettes.empty_joins);
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert_eq!(b.app.doc.panels.len(), 5, "un plan neuf : la continuité ne vaut que pour la case d'avant");
    // Annuler : le plan, puis la continuité.
    b.app.run_action("edition.annuler");
    b.app.run_action("edition.annuler");
    b.chauffer(2);
    assert_eq!(b.app.doc.panels[last].marks.len(), marks);
    assert_eq!(pas_d_annulation(&b), n);
}

#[test]
fn drawing_in_the_pale_case_with_the_old_flag_does_not_cascade() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 5);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    b.app.vignettes.empty_joins = true;
    let e = case_vide(&b, 0);
    glisser(&mut b, &vague(e.center() - vec2(20.0, 0.0), e.center() + vec2(20.0, 0.0), 8));
    assert!(!b.app.vignettes.empty_joins, "le drapeau retombe : la case d'après est une coupe");
    assert!(b.app.board_layout().bars.last().unwrap().cut);
}

#[test]
fn joining_two_plans_by_the_bar_shows_the_left_drawing_in_the_right_case_without_a_stroke() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 0);
    glisser(&mut b, &vague(v.center() - vec2(30.0, 0.0), v.center() + vec2(30.0, 0.0), 10));
    let a_len = b.app.doc.panels[0].frame_count();
    let avant = coupes(&b);
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 2));
    b.chauffer(2);
    let p = &b.app.doc.panels[0];
    assert!(p.layers.iter().any(|l| l.cel_at(a_len).is_some_and(|c| b.app.doc.cel_a_du_trait(c))), "la case de droite montre le dessin de la précédente");
    // Seule cette barre a changé : les suivantes restent ce qu'elles étaient.
    let apres = coupes(&b);
    assert_eq!(apres.len(), avant.len());
    assert!(avant[2] && !apres[2], "la barre cliquée est grise");
    assert_eq!(apres[..2], avant[..2]);
    assert_eq!(apres[3..], avant[3..]);
    // Décocher dans l'autre sens : recouper ne touche que cette barre.
    let layout = b.app.board_layout();
    assert!(b.app.board_toggle_cut(&layout, 2));
    b.chauffer(2);
    assert_eq!(coupes(&b), avant);
}

// ------------------------------------------------------------------------------ le champ d'une case

fn pt(x: f32, y: f32) -> Pt {
    Pt { layer: VPoint { x, y, r: 1.0 }, board: pos2(x, y), pressure: 1.0, time: 0.0 }
}

#[test]
fn a_path_is_cut_where_it_leaves_a_field_and_no_point_is_lost() {
    let champs = [Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0)), Rect::from_min_max(pos2(20.0, 0.0), pos2(30.0, 10.0))];
    let recoit = [true, true];
    let owner = |p: Pos2| proprietaire(&champs, &recoit, 0, p);
    let chemin: Vec<Pt> = (0..=30).map(|i| pt(i as f32, 5.0)).collect();
    let runs = trancher(&chemin, &owner);
    let qui: Vec<Option<usize>> = runs.iter().map(|r| r.0).collect();
    assert_eq!(qui, vec![Some(0), None, Some(1)]);
    // Le premier tronçon finit à la limite (10), le suivant commence juste après ; tous les points de départ sont là.
    assert!((runs[0].1.last().unwrap().board.x - 10.0).abs() < 0.1);
    assert!((runs[2].1[0].board.x - 20.0).abs() < 0.1);
    let n: usize = runs.iter().map(|r| r.1.len()).sum();
    assert!(n >= chemin.len(), "{n}");
    // Là où deux champs se recouvrent, la case d'où l'on part garde le point ; sinon la première.
    let recouvrants = [Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 10.0)), Rect::from_min_max(pos2(5.0, 0.0), pos2(15.0, 10.0))];
    assert_eq!(proprietaire(&recouvrants, &[true, true], 0, pos2(7.0, 5.0)), Some(0));
    assert_eq!(proprietaire(&recouvrants, &[true, true], 1, pos2(7.0, 5.0)), Some(1));
    assert_eq!(proprietaire(&recouvrants, &[true, false], 0, pos2(12.0, 5.0)), None, "une case qui ne peut pas recevoir n'a pas de part");
}

fn hors_cadres(b: &Banc) -> usize {
    let a = &b.app.doc.annotations;
    // (Partie 127 : un trait bitmap laisse ses pixels sur la couche bitmap de la planche.)
    a.traces.iter().filter(|t| a.calque(t.calque).is_some_and(|c| c.planche)).count() + a.bitmap.morceaux.len()
}

fn points_de_depart_hors_champ(b: &Banc) -> (Pos2, Pos2) {
    // Un coin du papier, loin de toute case : on le vérifie.
    let view = b.app.vignettes.view;
    let a = pos2(view.left() + 6.0, view.top() + 6.0);
    let layout = b.app.board_layout();
    let champs = champs(&layout, b.app.settings.vignettes.debord_fraction());
    let (p, q) = (b.app.vignettes.xf.to_board(a), b.app.vignettes.xf.to_board(a + vec2(80.0, 40.0)));
    assert!(champs.iter().all(|c| !c.contains(p) && !c.contains(q)), "le coin est hors de tout champ");
    (a, a + vec2(80.0, 40.0))
}

#[test]
fn a_stroke_outside_every_field_stays_on_the_board_in_no_plan_in_one_undo_step_and_is_saved_and_reread() {
    for bitmap in [false, true] {
        let doc = if bitmap { en_bitmap(quatre_plans()) } else { quatre_plans() };
        let mut b = ouvert(doc, 4);
        let (de, a) = points_de_depart_hors_champ(&b);
        let n = pas_d_annulation(&b);
        glisser(&mut b, &vague(de, a, 12));
        assert!(rien_dans_les_plans(&b), "bitmap {bitmap} : aucun plan n'a le trait");
        assert_eq!(hors_cadres(&b), 1, "bitmap {bitmap} : le trait est sur la planche");
        assert_eq!(b.app.doc.panels.len(), 4);
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap} : une seule étape");
        if bitmap {
            // Partie 127 : ses pixels sur la couche bitmap de la planche (relue : `planchebitmap_tests`, `vignettes_bitmap_tests`), et dans l'export.
            assert!(b.app.doc.annotations.traces.is_empty());
            let opt = crate::planche::Options::default();
            let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
            let p = crate::planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
            assert_eq!(p.annotations.bitmap.morceaux.len(), 1);
            b.app.run_action("edition.annuler");
            b.chauffer(2);
            assert_eq!(hors_cadres(&b), 0);
            assert!(rien_dans_les_plans(&b));
            continue;
        }
        // Enregistré et relu : mêmes traits, même calque.
        let octets = crate::annotations::to_bytes(&b.app.doc).expect("écrit").expect("un fichier");
        let relu = crate::annotations::parse(std::str::from_utf8(&octets).unwrap()).expect("relu");
        let (a, r) = (&b.app.doc.annotations, &relu);
        assert_eq!((r.calques.len(), r.traces.len()), (a.calques.len(), a.traces.len()), "bitmap {bitmap}");
        assert_eq!((r.traces[0].color, r.traces[0].alpha, r.traces[0].anchor, r.traces[0].points.len()), (a.traces[0].color, a.traces[0].alpha, a.traces[0].anchor, a.traces[0].points.len()));
        assert!(r.traces[0].points.iter().zip(&a.traces[0].points).all(|(x, y)| (x.x - y.x).abs() < 1e-3 && (x.r - y.r).abs() < 1e-3), "mêmes points à l'arrondi près");
        assert!(relu.calques.iter().any(|c| c.planche));
        // Dans l'export de la planche, même sans l'option des annotations ; pas dans les plans.
        let opt = crate::planche::Options::default();
        let tous: Vec<usize> = (0..b.app.doc.panels.len()).collect();
        let p = crate::planche::construire(&b.app.doc, &tous, &b.app.layout.board_settings(), &opt, &b.app.doc.annotations);
        assert_eq!(p.annotations.traces.len(), 1, "bitmap {bitmap}");
        // Annuler : plus rien.
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert_eq!(hors_cadres(&b), 0);
        assert!(rien_dans_les_plans(&b));
        assert_eq!(pas_d_annulation(&b), n);
    }
}

#[test]
fn a_stroke_across_the_edge_of_a_case_splits_there_plan_part_stays_the_rest_goes_to_the_board_one_undo_step() {
    for bitmap in [false, true] {
        let doc = if bitmap { en_bitmap(quatre_plans()) } else { quatre_plans() };
        let mut b = ouvert(doc, 4);
        let v = vignette(&b, 0);
        let loin = pos2(b.app.vignettes.view.left() + 4.0, v.center().y);
        let n = pas_d_annulation(&b);
        glisser(&mut b, &vague(v.center(), loin, 16));
        assert!(encre(&b.app.doc, 0), "bitmap {bitmap} : la partie dans le champ reste dans le plan");
        assert!(hors_cadres(&b) >= 1, "bitmap {bitmap} : le reste est sur la planche");
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap} : une seule étape pour tout le trait");
        // Aucun point du trait de la planche n'est dans le champ de la case du plan.
        let layout = b.app.board_layout();
        let champ = champs(&layout, b.app.settings.vignettes.debord_fraction())[0];
        let a = &b.app.doc.annotations;
        for (i, t) in a.traces.iter().enumerate() {
            if a.calque(t.calque).is_some_and(|c| c.planche) {
                for p in a.points_planche(&layout, i).unwrap() {
                    assert!(!champ.shrink(1.0).contains(p.pos()), "bitmap {bitmap} : {p:?} est dans le champ {champ:?}");
                }
            }
        }
        // (Et aucun pixel de la couche bitmap de la planche.)
        for m in &a.bitmap.morceaux {
            let v = m.vers_planche(&layout).expect("sa case");
            crate::planchebitmap::chaque_pixel(&m.pixels, |x, y, _| {
                let p = v.point(pos2(x as f32 + 0.5, y as f32 + 0.5));
                assert!(!champ.shrink(1.0).contains(p), "bitmap : le pixel {p:?} est dans le champ {champ:?}");
            });
        }
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert!(!encre(&b.app.doc, 0) && hors_cadres(&b) == 0, "bitmap {bitmap} : tout est parti d'un coup");
        assert_eq!(pas_d_annulation(&b), n);
    }
}

#[test]
fn a_stroke_across_two_neighbour_cases_gives_each_plan_its_part_in_one_undo_step() {
    for bitmap in [false, true] {
        let doc = if bitmap { en_bitmap(quatre_plans()) } else { quatre_plans() };
        let mut b = ouvert(doc, 4);
        let (v4, v5) = (vignette(&b, 4), vignette(&b, 5));
        let n = pas_d_annulation(&b);
        glisser(&mut b, &vague(v4.center(), v5.center(), 24));
        assert!(encre(&b.app.doc, 2), "bitmap {bitmap} : le plan 3 a sa part");
        assert!(encre(&b.app.doc, 3), "bitmap {bitmap} : le plan 4 a sa part");
        assert_eq!(pas_d_annulation(&b), n + 1, "bitmap {bitmap} : une seule étape");
        b.app.run_action("edition.annuler");
        b.chauffer(2);
        assert!(!encre(&b.app.doc, 2) && !encre(&b.app.doc, 3), "bitmap {bitmap}");
        assert_eq!(pas_d_annulation(&b), n);
    }
}

#[test]
fn a_stroke_inside_its_case_costs_nothing_more_and_leaves_the_board_alone() {
    let mut b = ouvert(quatre_plans(), 4);
    let v = vignette(&b, 0);
    let n = pas_d_annulation(&b);
    glisser(&mut b, &vague(v.center() - vec2(20.0, 0.0), v.center() + vec2(20.0, 0.0), 8));
    assert!(encre(&b.app.doc, 0));
    assert_eq!(hors_cadres(&b), 0);
    assert_eq!(pas_d_annulation(&b), n + 1);
    assert!(b.app.doc.annotations.rien_a_ecrire(), "rien à écrire dans le projet pour la planche");
}

#[test]
fn the_eraser_outside_every_case_erases_the_board_drawing_it_touches() {
    let mut b = ouvert(quatre_plans(), 4);
    let (de, a) = points_de_depart_hors_champ(&b);
    glisser(&mut b, &vague(de, a, 12));
    assert_eq!(hors_cadres(&b), 1);
    b.app.tool = crate::app::Tool::Eraser;
    glisser(&mut b, &[de + vec2(30.0, 15.0), de + vec2(40.0, 20.0), de + vec2(50.0, 22.0)]);
    assert_eq!(hors_cadres(&b), 0, "la gomme a pris le trait de la planche");
}

// ------------------------------------------------------------------------------ numéros, textes, pastille

#[test]
fn the_plan_number_follows_the_zoom_within_bounds_and_the_setting_scales_it() {
    use crate::vignettesprefs::taille_numero;
    assert_eq!(taille_numero(0.01, 1.0), 15.0, "très loin : 15 points au moins");
    assert_eq!(taille_numero(50.0, 1.0), 72.0, "très près : 72 points au plus");
    let a = taille_numero(0.3, 1.0);
    let b = taille_numero(0.6, 1.0);
    assert!(b >= a);
    assert!(taille_numero(0.01, 2.0) > taille_numero(0.01, 1.0));
    assert!(taille_numero(0.01, 0.5) < taille_numero(0.01, 1.0));
}

#[test]
fn the_version_badge_is_outside_the_frame_next_to_the_plan_number() {
    let mut doc = quatre_plans();
    assert!(doc.nouvelle_version(0, 10).is_some());
    let mut b = ouvert(doc, 4);
    let plan = b.app.doc.panels[0].id;
    let zone = b.app.pile_versions.etiquettes.iter().find(|(_, p)| *p == plan).map(|(r, _)| *r).expect("l'étiquette");
    let case = vignette(&b, 0);
    assert!(!zone.intersects(case), "hors de la case : {zone:?} {case:?}");
    assert!(zone.bottom() <= case.top() + 1.0, "au-dessus du cadre, près du numéro");
    // Éteinte dans les Préférences : plus d'étiquette.
    b.app.settings.vignettes.pastille_version = false;
    b.chauffer(3);
    assert!(b.app.pile_versions.etiquettes.iter().all(|(_, p)| *p != plan));
}

#[test]
fn texts_are_off_by_default_and_each_checked_line_is_shown_with_its_label_and_colour_even_empty() {
    use crate::boardsens::{Champ, Textes};
    assert_eq!(crate::workspace::Layout::default().board_textes, Textes::aucun(), "aucune ligne d'origine");
    assert_eq!(crate::vignettesreglages::ReglagesVignettes::default().textes_defaut, Textes::aucun());
    let mut b = crate::boardjapon_tests::banc_vignettes(quatre_plans(), crate::boardsens::Sens::Lignes, 4);
    b.app.layout.board_textes = Textes { action: true, dialogue: true, notes: true };
    b.chauffer(4);
    // Trois lignes sous la case, vides : chacune a son intitulé.
    assert!(proche_lignes(&b, 3));
    for champ in Champ::TOUS {
        assert!(b.textes.iter().any(|t| t.starts_with(champ.etiquette())), "{champ:?} : sa ligne est visible même vide : {:?}", b.textes);
    }
}

fn proche_lignes(b: &Banc, n: usize) -> bool {
    (b.app.board_layout().cards[0].note.height() - n as f32 * crate::board::NOTE_LINE_H).abs() < 0.5
}

// ------------------------------------------------------------------------------ réglages

#[test]
fn the_new_vignette_settings_have_their_defaults_and_an_old_settings_file_still_reads() {
    use crate::vignettesreglages::{ReglagesVignettes, COULEURS_TEXTES_DEFAUT};
    let d = ReglagesVignettes::default();
    assert_eq!((d.taille_numeros, d.fond_numeros, d.pastille_version, d.afficher_duree), (100, true, true, false));
    assert_eq!((d.ouverture_colonnes, d.ouverture_rangees), (6, 2));
    assert_eq!(d.textes_defaut.lignes(), 0);
    assert_eq!(d.couleurs_textes, COULEURS_TEXTES_DEFAUT);
    assert_eq!((d.epaisseur_cadre, d.couleur_filet, d.opacite_hors_champ), (1.0, None, 100));
    assert_eq!((d.passe_partout_cases, d.passe_partout_cases_opacite), (false, 100));
    // Un fichier d'avant : seuls les anciens réglages, les nouveaux restent à leur défaut.
    let vieux: ReglagesVignettes = toml::from_str("debord_pour_cent = 20\naimant_points = 3").expect("lu");
    assert_eq!((vieux.debord, vieux.aimant), (20, 3));
    assert_eq!((vieux.ouverture_colonnes, vieux.taille_numeros), (6, 100));
    // Des valeurs hors bornes sont ramenées.
    let fou = ReglagesVignettes { taille_numeros: 9000, ouverture_colonnes: 0, ouverture_rangees: 200, opacite_hors_champ: 250, ..Default::default() }.nettoyes();
    assert_eq!((fou.taille_numeros, fou.ouverture_colonnes, fou.ouverture_rangees, fou.opacite_hors_champ), (300, 1, 12, 100));
    // Aller-retour.
    let t = toml::to_string(&ReglagesVignettes { ouverture_colonnes: 4, ouverture_rangees: 3, ..Default::default() }).unwrap();
    let relu: ReglagesVignettes = toml::from_str(&t).unwrap();
    assert_eq!((relu.ouverture_colonnes, relu.ouverture_rangees), (4, 3));
}

// ------------------------------------------------------------------------------ zoom d'ouverture, barre du bas, passe-partout

fn beaucoup_de_plans(n: usize) -> Document {
    let mut doc = Document::with_frame(160, 90, 24);
    let mut last = 0;
    for _ in 1..n {
        last = doc.add_scene_after(last);
    }
    doc.into_plans();
    doc
}

#[test]
fn a_project_opens_on_two_rows_of_six_cases_and_the_zoom_stays_when_coming_back() {
    let mut b = crate::perf_tests::banc_taille(beaucoup_de_plans(30), false, 1600, 900);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.vignettes.ouverture_a_faire = true;
    b.app.layout.board_columns = 6;
    b.app.layout.board_rows = 0;
    // (Le cadrage d'ouverture suit la vue pendant une dizaine d'images, le temps que les panneaux se rangent.)
    b.chauffer(24);
    let view = b.app.vignettes.view;
    let layout = b.app.board_layout();
    for k in 0..12 {
        let r = b.app.vignettes.xf.rect(layout.cards[k].thumb);
        assert!(view.contains_rect(r), "la case {k} est entière dans la vue : {r:?} {view:?}");
    }
    let zoom = b.app.board_zoom;
    assert!(zoom > 0.0);
    // Les six cases d'une rangée prennent la largeur de la vue (c'est elle qui fixe le zoom), pas toute la planche (30 plans).
    let (g, d) = (b.app.vignettes.xf.rect(layout.cards[0].thumb), b.app.vignettes.xf.rect(layout.cards[5].thumb));
    assert!((d.right() - g.left()) > view.width() * 0.6, "six cases sur la largeur : {} pour {}", d.right() - g.left(), view.width());
    // Revenir sur l'onglet (changer d'espace et revenir) ne recadre pas.
    b.app.set_espace(crate::workspace::Espace::Storyboard);
    b.chauffer(3);
    b.app.set_espace(crate::workspace::Espace::Vignettes);
    b.chauffer(4);
    assert_eq!(b.app.board_zoom, zoom);
    // Le bouton « 6 × 2 » recadre à la demande.
    b.app.board_zoom = 0.3;
    b.app.vignettes.ouverture_a_faire = false;
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = true;
    b.chauffer(3);
    let zoom = b.app.board_zoom;
    assert!(zoom > 0.0);
    // Un autre réglage : 3 × 1.
    b.app.settings.vignettes.ouverture_colonnes = 3;
    b.app.settings.vignettes.ouverture_rangees = 1;
    b.app.board_zoom = 0.0;
    b.app.vignettes.ouverture_a_faire = true;
    b.chauffer(3);
    assert!(b.app.board_zoom > zoom, "moins de cases : plus gros");
}

#[test]
fn the_bottom_bar_is_under_the_board_and_the_cases_passe_partout_switch_and_opacity_are_kept() {
    let mut b = ouvert(quatre_plans(), 4);
    let (barre_basse, view) = (b.app.vignettes.barre_basse, b.app.vignettes.view);
    assert!((barre_basse.height() - crate::vignettesprefs::BARRE_BASSE_H).abs() < 0.5);
    assert!((view.bottom() - barre_basse.top()).abs() < 0.5, "la planche s'arrête où la barre commence");
    assert!(!b.app.vignettes.annot.marie, "éteint d'origine");
    b.app.run_action("vignettes.marie_louise");
    assert!(b.app.vignettes.annot.marie);
    assert!(b.app.settings.vignettes.passe_partout_cases, "retenu dans les réglages");
    // Un état retenu se retrouve à la séance suivante.
    let v = crate::boardui::Vignettes::avec_passe_partout(b.app.settings.vignettes.passe_partout_cases);
    assert!(v.annot.marie);
    // L'opacité est celle des réglages (voile à 0 % : rien n'est couvert).
    b.app.settings.vignettes.passe_partout_cases_opacite = 0;
    b.chauffer(2);
    b.app.run_action("vignettes.marie_louise");
    assert!(!b.app.settings.vignettes.passe_partout_cases);
}

#[test]
fn the_opening_frame_has_equal_side_margins_whole_first_row_with_its_numbers_just_under_the_top_and_the_6_x_2_button_gives_the_same() {
    let mut b = crate::perf_tests::banc_taille(beaucoup_de_plans(30), false, 1600, 900);
    b.app.board_view = crate::app::BoardView::Panels;
    b.app.vignettes.ouverture_a_faire = true;
    b.app.layout.board_columns = 6;
    b.app.layout.board_rows = 0;
    b.chauffer(24);
    let (view, xf) = (b.app.vignettes.view, b.app.vignettes.xf);
    let layout = b.app.board_layout();
    for k in 0..6 {
        let (thumb, numero) = (xf.rect(layout.cards[k].thumb), xf.rect(layout.cards[k].label()));
        assert!(view.contains_rect(thumb), "l'image {k} est entière dans la vue : {thumb:?} {view:?}");
        assert!(view.contains_rect(numero), "le numéro {k} est entier dans la vue : {numero:?} {view:?}");
    }
    let gauche = xf.rect(layout.cards[0].thumb).left() - view.left();
    let droite = view.right() - xf.rect(layout.cards[5].thumb).right();
    assert!((gauche - droite).abs() < 12.0, "marges égales à gauche et à droite : {gauche} contre {droite}");
    assert!(gauche > 2.0, "la première colonne ne touche pas le bord : {gauche}");
    let haut = xf.rect(layout.cards[0].label()).top() - view.top();
    assert!((0.0..=24.0).contains(&haut), "la première rangée, numéros compris, juste sous le haut : {haut}");
    // Le bouton « 6 × 2 » donne exactement le même cadrage.
    let (zoom, pan) = (b.app.board_zoom, b.app.board_pan);
    b.app.board_zoom = 0.0;
    b.app.board_pan = eframe::egui::vec2(300.0, 300.0);
    b.app.vignettes.ouverture_a_faire = true;
    b.chauffer(24);
    assert!((b.app.board_zoom - zoom).abs() < 1e-3 && (b.app.board_pan - pan).length() < 1.0, "{} {:?} contre {zoom} {pan:?}", b.app.board_zoom, b.app.board_pan);
}

#[test]
fn the_view_has_one_zoom_command_in_the_bottom_bar_showing_the_real_zoom() {
    let mut b = ouvert(quatre_plans(), 4);
    b.garder_textes = true;
    b.app.board_zoom = 0.77;
    b.chauffer(4);
    let textes = b.textes.clone();
    assert!(textes.iter().any(|t| t.contains("77")), "la barre du bas montre le vrai zoom : {textes:?}");
    // Plus de pourcentage de zoom dans la barre du haut (la seule commande de zoom est en bas) : un seul « 77 % ».
    assert_eq!(textes.iter().filter(|t| t.contains("77 %") || t.contains("77%")).count(), 1, "{textes:?}");
}

#[test]
fn a_new_project_reads_the_default_text_lines_from_the_preferences_none_by_default() {
    use crate::boardsens::Textes;
    let mut b = ouvert(quatre_plans(), 4);
    b.app.layout.board_textes = Textes { action: false, dialogue: true, notes: true };
    let d = crate::nouveauui::NewDocDialog::open(&b.app.settings.new_project);
    assert!(b.app.create_project(&d));
    assert_eq!(b.app.layout.board_textes, Textes::aucun(), "d'origine : aucune ligne dans un projet neuf");
    b.app.settings.vignettes.textes_defaut = Textes { action: true, dialogue: false, notes: true };
    let d = crate::nouveauui::NewDocDialog::open(&b.app.settings.new_project);
    assert!(b.app.create_project(&d));
    assert_eq!(b.app.layout.board_textes, Textes { action: true, dialogue: false, notes: true });
    // Ouvrir un projet existant ne touche pas au choix fait.
    b.app.layout.board_textes = Textes { action: false, dialogue: false, notes: true };
    let doc = quatre_plans();
    assert!(b.app.charger_projet(doc));
    assert_eq!(b.app.layout.board_textes, Textes { action: false, dialogue: false, notes: true });
}
