//! Déplacer la sélection aux flèches : un pixel, dix avec Maj ; Ctrl et Alt gardent leurs
//! raccourcis.

use eframe::egui::{vec2, Key, Modifiers};

use crate::nudge::{arrow_step, BIG_STEP};

#[test]
fn arrows_move_by_one_pixel_and_by_ten_with_shift() {
    // Anthony : « déplacement au pixel, déplacement par 10 pixels quand Maj est maintenu ».
    assert_eq!(arrow_step(Key::ArrowLeft, Modifiers::NONE), Some(vec2(-1.0, 0.0)));
    assert_eq!(arrow_step(Key::ArrowRight, Modifiers::NONE), Some(vec2(1.0, 0.0)));
    assert_eq!(arrow_step(Key::ArrowUp, Modifiers::NONE), Some(vec2(0.0, -1.0)), "vers le haut : y diminue");
    assert_eq!(arrow_step(Key::ArrowDown, Modifiers::NONE), Some(vec2(0.0, 1.0)));
    assert_eq!(arrow_step(Key::ArrowRight, Modifiers::SHIFT), Some(vec2(BIG_STEP, 0.0)));
    assert_eq!(BIG_STEP, 10.0);
}

#[test]
fn ctrl_and_alt_arrows_keep_their_shortcuts() {
    // Ctrl + flèches : instances et calques ; Ctrl + Maj : signets.
    assert_eq!(arrow_step(Key::ArrowLeft, Modifiers::CTRL | Modifiers::COMMAND), None);
    assert_eq!(arrow_step(Key::ArrowLeft, Modifiers::CTRL | Modifiers::COMMAND | Modifiers::SHIFT), None);
    assert_eq!(arrow_step(Key::ArrowUp, Modifiers::ALT), None);
    assert_eq!(arrow_step(Key::A, Modifiers::NONE), None, "une autre touche ne déplace rien");
}
