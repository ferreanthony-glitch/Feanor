//! « Aller à un plan » : par son nom, par séquence/plan, sinon par son rang.

use crate::aller::find_plan;

fn plans() -> Vec<(String, String)> {
    [("1", "1"), ("1", "2"), ("1", "12A"), ("2", "1"), ("2", "3")].iter().map(|(s, p)| (s.to_string(), p.to_string())).collect()
}

#[test]
fn a_plan_is_found_by_its_name_first_in_the_current_sequence() {
    let p = plans();
    assert_eq!(find_plan(&p, "12a", "1"), Some(2), "sans tenir compte des majuscules");
    assert_eq!(find_plan(&p, "1", "1"), Some(0));
    assert_eq!(find_plan(&p, "1", "2"), Some(3), "le plan 1 de la séquence courante d'abord");
    assert_eq!(find_plan(&p, " 3 ", "1"), Some(4), "le seul plan nommé 3");
}

#[test]
fn sequence_slash_plan_then_rank() {
    let p = plans();
    assert_eq!(find_plan(&p, "2/1", "1"), Some(3));
    assert_eq!(find_plan(&p, "2 3", "1"), Some(4));
    // Aucun plan nommé 5 : le cinquième plan du storyboard.
    assert_eq!(find_plan(&p, "5", "1"), Some(4));
    assert_eq!(find_plan(&p, "9", "1"), None);
    assert_eq!(find_plan(&p, "", "1"), None);
    assert_eq!(find_plan(&p, "zz", "1"), None);
}
