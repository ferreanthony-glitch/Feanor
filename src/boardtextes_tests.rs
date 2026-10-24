//! Essais des textes sous les cases de la planche des Vignettes (Anthony, 2026-10-24) : chaque champ coché dans le panneau Planche fait
//! exactement une ligne (coupée par « … »), aucun champ coché : aucune ligne et la place sous les cases se resserre ; un clic sur la
//! ligne Notes l'écrit, comme l'icône.

use std::collections::HashMap;

use eframe::egui::{epaint::Shape, Pos2};

use crate::board::{self, BoardSettings, Hit, Options, LABEL_H, NOTE_LINE_H, ROW_GAP};
use crate::boardjapon_tests::{banc_vignettes, clic, ecran, plan_avec_textes};
use crate::boardsens::{Champ, Sens, Textes};
use crate::vignettes_camera_tests::{deux_cadres, plan_court, sans_chevauchement, K};

fn textes(action: bool, dialogue: bool, notes: bool) -> Textes {
    Textes { action, dialogue, notes }
}

fn proche(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-2
}

#[test]
fn each_checked_field_is_one_line_and_none_checked_means_no_text_zone() {
    let doc = plan_court();
    for (t, n) in [(textes(false, false, false), 0usize), (textes(false, false, true), 1), (textes(true, false, true), 2), (textes(true, true, true), 3), (textes(false, true, false), 1)] {
        for sens in Sens::TOUS {
            let o = Options { sens, textes: t, ..Default::default() };
            let l = board::layout_with(&doc, &BoardSettings { card_w: 170.0, columns: 6, rows: 0 }, &HashMap::new(), false, &o);
            for (k, c) in l.cards.iter().enumerate() {
                assert!(proche(c.note.height(), n as f32 * NOTE_LINE_H), "{n} champs ({sens:?}) : {n} lignes, pas plus : {:?}", c.note);
                assert_eq!(c.a_du_texte(), n > 0, "{n} champs ({sens:?}) : la zone n'existe que s'il y a un champ : {:?}", c.note);
                // Sans champ : rien à toucher à la place de la zone.
                if n == 0 {
                    assert_ne!(board::hit(&l, c.note.center(), 0.0), Hit::Note(k), "pas de zone de note sans champ");
                }
            }
        }
        // La rangée (en lignes) est aussi haute que ses lignes : sans champ, plus d'espace sous les cases.
        let s = BoardSettings { card_w: 170.0, columns: 2, rows: 0 };
        let l = board::layout_with(&doc, &s, &HashMap::new(), false, &Options { textes: t, ..Default::default() });
        let attendu = LABEL_H + l.cards[0].thumb.height() + if n == 0 { 0.0 } else { board::note_gap(170.0) + n as f32 * NOTE_LINE_H } + ROW_GAP;
        assert!(proche(l.cards[2].thumb.top() - l.cards[0].thumb.top(), attendu), "{n} champs : la rangée suivante est à {} (attendu {attendu})", l.cards[2].thumb.top() - l.cards[0].thumb.top());
    }
}

#[test]
fn a_move_takes_the_text_lines_of_the_checked_fields_too() {
    for t in [textes(false, false, false), textes(true, false, true), textes(true, true, true)] {
        for sens in Sens::TOUS {
            let mut doc = plan_court();
            deux_cadres(&mut doc, 1, 160.0, 55.0);
            let l = board::layout_with(&doc, &BoardSettings { card_w: 170.0, columns: 4, rows: 0 }, &HashMap::new(), false, &Options { sens, textes: t, ..Default::default() });
            let c = l.cards[K];
            assert!(proche(c.note.height(), t.lignes() as f32 * NOTE_LINE_H), "{sens:?} : {} lignes sous le mouvement", t.lignes());
            sans_chevauchement(&l);
        }
    }
}

#[test]
fn on_screen_a_field_is_one_clipped_line_with_a_label_only_when_two_or_more_are_shown() {
    let mut b = banc_vignettes(plan_avec_textes(), Sens::Lignes, 4);
    b.app.doc.panels[1].action = "mot ".repeat(200);
    for (t, etiquettes) in [(textes(false, false, true), false), (textes(true, false, true), true), (textes(true, true, true), true)] {
        b.app.layout.board_textes = t;
        b.chauffer(4);
        let galleys: Vec<_> = b.formes.iter().filter_map(|s| if let Shape::Text(x) = &s.shape { Some(x.galley.clone()) } else { None }).collect();
        for champ in t.champs() {
            let g = galleys
                .iter()
                .find(|g| match champ {
                    Champ::Action => g.job.text.contains("mot mot"),
                    Champ::Dialogue => g.job.text.contains("Où vas-tu"),
                    Champ::Notes => g.job.text.contains("La clé"),
                })
                .unwrap_or_else(|| panic!("{champ:?} est peint : {:?}", b.textes));
            assert_eq!(g.rows.len(), 1, "{champ:?} : une seule ligne");
            assert_eq!(g.job.text.starts_with(champ.etiquette()), etiquettes, "{champ:?} : l'étiquette seulement avec plusieurs champs : {:?}", g.job.text);
        }
        let c = b.app.board_layout().cards[3];
        assert!(proche(c.note.height(), t.lignes() as f32 * NOTE_LINE_H));
    }
    // L'action trop longue finit par « … ».
    b.app.layout.board_textes = textes(true, false, false);
    b.chauffer(4);
    let g = b.formes.iter().filter_map(|s| if let Shape::Text(x) = &s.shape { Some(x.galley.clone()) } else { None }).find(|g| g.job.text.contains("mot mot")).unwrap();
    assert!(g.rows[0].text().ends_with('…'), "{:?}", g.rows[0].text());
    // Aucun champ coché : aucun texte, aucune ligne, plus d'icône d'écriture.
    b.app.layout.board_textes = textes(false, false, false);
    b.chauffer(4);
    for morceau in ["La clé", "Il court", "Où vas-tu", "mot mot"] {
        assert!(b.textes.iter().all(|t| !t.contains(morceau)), "« {morceau} » ne se peint plus : {:?}", b.textes);
    }
    let l = b.app.board_layout();
    assert!(l.cards.iter().all(|c| !c.a_du_texte()));
    let c = l.cards[3];
    let icone = b.app.note_icone_rect(&c);
    clic(&mut b, icone.center());
    assert!(b.app.vignettes.note.is_none(), "sans zone de texte, l'icône n'écrit rien");
}

#[test]
fn a_click_on_the_notes_line_writes_the_note_like_the_icon_and_the_other_lines_do_not() {
    for (sens, t) in [(Sens::Lignes, textes(true, false, true)), (Sens::Lignes, textes(true, true, true)), (Sens::Lignes, textes(false, false, true)), (Sens::Japonais, textes(true, false, true))] {
        let mut b = banc_vignettes(plan_avec_textes(), sens, 4);
        b.app.layout.board_textes = t;
        b.chauffer(4);
        let c = b.app.board_layout().cards[3];
        // Une ligne qui n'est pas Notes : un appui dessine dans la case, comme avant (il n'écrit pas la note).
        let autre = t.champs().into_iter().position(|x| x != Champ::Notes);
        if let Some(i) = autre {
            let p: Pos2 = ecran(&b, c.ligne_de_texte(i)).center();
            clic(&mut b, p);
            assert!(b.app.vignettes.note.is_none(), "{sens:?} {t:?} : la ligne {i} (pas Notes) n'écrit pas la note");
        }
        // La ligne Notes : un clic l'écrit, la note de la case, avec son texte dans le tampon.
        let i = t.ligne_du(Champ::Notes).expect("Notes est cochée");
        let p: Pos2 = ecran(&b, c.ligne_de_texte(i)).center();
        let n = b.app.doc.panels[1].notes.clone();
        clic(&mut b, p);
        let e = b.app.vignettes.note.as_ref().unwrap_or_else(|| panic!("{sens:?} {t:?} : un clic sur la ligne Notes écrit la note"));
        assert_eq!(e.id, c.id);
        assert_eq!(e.buffer, n, "le tampon porte la note de la case");
        // L'icône, elle, fait comme avant : elle écrit aussi la note.
        b.app.vignettes.note = None;
        let icone = b.app.note_icone_rect(&c);
        clic(&mut b, icone.center());
        assert_eq!(b.app.vignettes.note.as_ref().map(|e| e.id), Some(c.id), "{sens:?} {t:?} : l'icône écrit la note");
    }
}

#[test]
fn the_note_command_says_what_to_check_when_no_field_is_shown() {
    let mut b = banc_vignettes(plan_avec_textes(), Sens::Lignes, 4);
    b.app.layout.board_textes = textes(false, false, false);
    b.chauffer(3);
    b.app.run_action("vignettes.note");
    assert!(b.app.vignettes.note.is_none());
    assert!(b.app.status.contains("Coche un champ"), "{}", b.app.status);
}

#[test]
fn the_export_has_one_line_per_field_and_no_text_zone_without_the_option() {
    use crate::planche::{construire, Options as Export};
    let mut doc = plan_avec_textes();
    doc.panels[1].action = "mot ".repeat(200);
    let s = BoardSettings { card_w: 170.0, columns: 4, rows: 0 };
    let tout: Vec<usize> = (0..doc.panels.len()).collect();
    for sens in Sens::TOUS {
        let o = Export { sens, textes: textes(true, true, true), ..Export::default() };
        let p = construire(&doc, &tout, &s, &o, &doc.annotations);
        let c = p.cartes.iter().find(|c| c.place.plan == doc.panels[1].id).unwrap();
        assert!(proche(c.place.note.height(), 3.0 * NOTE_LINE_H), "{sens:?} : trois champs, trois lignes");
        assert_eq!(c.champs.len(), 3);
        assert!(c.champs.iter().all(|(_, _, lignes)| lignes.len() == 1), "{sens:?} : une seule ligne par champ : {:?}", c.champs);
        assert!(c.champs[0].2[0].ends_with('…'), "{sens:?} : l'action trop longue est coupée : {:?}", c.champs[0].2);
        // Sans l'option Notes : aucune ligne, la place se resserre.
        let sans = construire(&doc, &tout, &s, &Export { notes: false, ..o.clone() }, &doc.annotations);
        assert!(sans.cartes.iter().all(|c| !c.place.a_du_texte() && c.champs.is_empty()), "{sens:?} : pas de zone de texte sans l'option");
        assert!(sans.cadre.size().x * sans.cadre.size().y < p.cadre.size().x * p.cadre.size().y, "{sens:?} : la planche se resserre");
    }
}
