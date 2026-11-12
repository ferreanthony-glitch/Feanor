//! Essais des vues multiples de l'Espace 3D qui visent ce qui est né de la relecture de 16f24d9 (`espace3dvues.rs`,
//! `docs/INTERFACE.md` partie 144.8) : le bouton « Regarder par la caméra » du panneau Réglages, et le quadrillage commun à
//! la vue seule et aux vues multiples.

use eframe::egui::{self, pos2, vec2, Rect};

use crate::document::{Document, Layer, LayerKind};
use crate::espace3dvues::{Disposition, Ortho, PointDeVue};
use crate::perf_tests::{banc_taille, Banc};

fn space() -> Banc {
    let mut doc = Document::new(320, 180, 24);
    doc.panels[0].layers = vec![Layer::new("Carte", LayerKind::Bitmap)];
    let mut b = banc_taille(doc, false, 1280, 720);
    b.chauffer(2);
    b.app.run_action("espace3d.basculer");
    b.chauffer(4);
    b
}

/// Le panneau Réglages de l'Espace 3D, peint seul dans une image.
fn reglages(b: &mut Banc) {
    let ctx = b.ctx.clone();
    let app = &mut b.app;
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 1400.0))), ..Default::default() };
    let _ = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.espace3d_options_panel(ui));
    });
}

#[test]
fn le_bouton_des_reglages_suit_la_vue_active() {
    let mut b = space();
    b.app.run_action("espace3d.vues_quatre");
    b.chauffer(3);
    // La vue active (la perspective, en bas à droite) ne regarde pas par la caméra : le bouton est relevé.
    assert!(!b.app.e3_par_la_camera());
    // Ce que fait son clic (`e3_basculer_par_la_camera`, la même fonction pour la barre et les Réglages) : la vue active passe
    // à la caméra du plan, la vue seule ne change pas.
    let active = b.app.espace3d.vues.active;
    b.app.e3_basculer_par_la_camera();
    assert_eq!(b.app.espace3d.vues.cadres[active].point, PointDeVue::Camera);
    assert!(b.app.e3_par_la_camera() && !b.app.espace3d.through_camera, "allumé pour la vue active, la vue seule intacte");
    // Une autre vue devient active : l'état du bouton la suit.
    b.app.espace3d.vues.active = 0;
    assert!(!b.app.e3_par_la_camera());
    b.app.e3_basculer_par_la_camera();
    assert_eq!(b.app.espace3d.vues.cadres[0].point, PointDeVue::Camera);
    b.app.e3_basculer_par_la_camera();
    assert_eq!(b.app.espace3d.vues.cadres[0].point, PointDeVue::Perspective, "un second clic la remet en perspective");
    // Le panneau Réglages se peint avec plusieurs vues (et son bouton passe par la même fonction : `espace3dui.rs`).
    reglages(&mut b);
    // En une vue : la vue seule, comme avant.
    b.app.e3_vues_poser(Disposition::Une);
    let avant = b.app.espace3d.through_camera;
    b.app.e3_basculer_par_la_camera();
    assert_eq!(b.app.espace3d.through_camera, !avant);
    assert_eq!(b.app.e3_par_la_camera(), !avant);
}

#[test]
fn la_vue_seule_et_chaque_vue_ont_le_meme_quadrillage() {
    let b = space();
    // Le sol d'une perspective : celui de la vue seule (`grid_lines`), posé à la hauteur du sol (`e3_hauteur_sol`).
    let sol = b.app.e3_hauteur_sol();
    let a = b.app.e3_quadrillage([1.0, 0.0, -2.0], 8.0, None);
    let brut = crate::espace3dview::grid_lines([1.0, 0.0, -2.0], 8.0);
    assert_eq!(a.len(), brut.len());
    assert!(a.iter().zip(&brut).all(|(x, y)| (x.a[1] - y.a[1] - sol).abs() < 1e-6 && x.a[0] == y.a[0] && x.b[2] == y.b[2]));
    // La vue de dessus : le sol, à la même hauteur ; la vue de face : le plan de la feuille (z = 0).
    let o = Ortho { target: [0.0; 3], span: 10.0 };
    let dessus = b.app.e3_quadrillage(o.target, o.span, Some((PointDeVue::Dessus, o, 1.5)));
    assert!(!dessus.is_empty() && dessus.iter().all(|l| (l.a[1] - sol).abs() < 1e-6 && (l.b[1] - sol).abs() < 1e-6));
    let face = b.app.e3_quadrillage(o.target, o.span, Some((PointDeVue::Face, o, 1.5)));
    assert!(!face.is_empty() && face.iter().all(|l| l.a[2] == 0.0 && l.b[2] == 0.0));
}
