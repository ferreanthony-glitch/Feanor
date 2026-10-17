//! Outils à ressort (`springtools`) : une pression brève change d'outil, une pression tenue (ou
//! un outil utilisé pendant qu'on tient la touche) le rend au relâché.

use crate::springtools::{back_on_release, HOLD};

#[test]
fn a_held_tool_key_brings_the_previous_tool_back() {
    assert!(!back_on_release(0.1, false), "une pression brève : on garde le nouvel outil");
    assert!(back_on_release(HOLD, false), "tenue assez longtemps : on revient");
    assert!(back_on_release(0.1, true), "l'outil a servi pendant qu'on tenait la touche : on revient");
}