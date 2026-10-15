//! Rôles des images (13g-2).

use crate::roles::{self, Role, Roles};

fn of(list: &[(usize, Role)]) -> Roles {
    list.iter().copied().collect()
}

#[test]
fn roles_follow_inserted_and_removed_images() {
    let mut r = of(&[(1, Role::Key), (4, Role::Inbetween), (8, Role::Breakdown)]);
    roles::shift(&mut r, 3, 2, false);
    assert_eq!(r, of(&[(1, Role::Key), (6, Role::Inbetween), (10, Role::Breakdown)]));
    // Images 5 à 7 retirées : le rôle de l'image 6 part, celui de 10 recule de 3.
    roles::shift(&mut r, 5, 3, true);
    assert_eq!(r, of(&[(1, Role::Key), (7, Role::Breakdown)]));
}

#[test]
fn roles_follow_reversed_and_moved_images() {
    let mut r = of(&[(2, Role::Key), (3, Role::Inbetween), (9, Role::SpecialDone)]);
    roles::reverse(&mut r, 2, 6);
    assert_eq!(r, of(&[(5, Role::Key), (4, Role::Inbetween), (9, Role::SpecialDone)]));
    // Images 4 et 5 déplacées à l'image 0 : retirées, puis posées devant.
    let row = roles::take(&r, 4, 6);
    roles::shift(&mut r, 4, 2, true);
    roles::shift(&mut r, 0, 2, false);
    roles::put(&mut r, 0, 2, &row);
    assert_eq!(r, of(&[(0, Role::Inbetween), (1, Role::Key), (9, Role::SpecialDone)]));
}

#[test]
fn a_cut_plan_takes_the_roles_after_the_cut() {
    let mut r = of(&[(1, Role::Key), (5, Role::Breakdown), (7, Role::Inbetween)]);
    let tail = roles::split_off(&mut r, 5);
    assert_eq!(r, of(&[(1, Role::Key)]));
    assert_eq!(tail, of(&[(0, Role::Breakdown), (2, Role::Inbetween)]));
}

#[test]
fn roles_follow_plans_cut_and_joined_and_inserted_images() {
    use crate::document::Document;
    let mut doc = Document::new(16, 16, 24);
    doc.panels[0].set_length(10);
    doc.panels[0].layers[0].roles = of(&[(2, Role::Key), (7, Role::Breakdown)]);
    doc.split_plan(0, 5).expect("coupé");
    assert_eq!(doc.panels[0].layers[0].roles, of(&[(2, Role::Key)]));
    assert_eq!(doc.panels[1].layers[0].roles, of(&[(2, Role::Breakdown)]));
    doc.join_next_plan(0).expect("réunis");
    assert_eq!(doc.panels[0].layers[0].roles, of(&[(2, Role::Key), (7, Role::Breakdown)]));
    // Trois images insérées à l'image 4 (Nouveau dessin, exposition de plus…).
    doc.panels[0].layers[0].insert(4, None, 3);
    assert_eq!(doc.panels[0].layers[0].roles, of(&[(2, Role::Key), (10, Role::Breakdown)]));
}

#[test]
fn roles_are_saved_with_the_layer() {
    use crate::document::{Document, LayerKind};
    let mut doc = Document::new(16, 16, 24);
    let id = doc.new_cel(LayerKind::Bitmap);
    let l = &mut doc.panels[0].layers[0];
    l.frames = vec![Some(id), None, None];
    l.roles = of(&[(0, Role::Key), (2, Role::Special)]);
    let dir = std::env::temp_dir().join(format!("pelure-test-roles-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = dir.join("R.pelure");
    crate::project::save(&doc, &manifest, &mut Default::default()).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("\"intervalle_special\""), "{text}");
    let back = crate::project::load(&manifest).unwrap().doc;
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(back.panels[0].layers[0].roles, of(&[(0, Role::Key), (2, Role::Special)]), "même sur une image vide");
}
