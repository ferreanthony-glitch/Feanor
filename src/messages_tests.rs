//! Messages : les importants deviennent des notifications, un geste ne remplit pas l'historique.

use crate::messages::{ago, important, record};

#[test]
fn only_important_messages_are_notified() {
    assert!(important("Enregistré dans le dossier « Bus » : C:\\Films\\Bus\\Bus.pelure"));
    assert!(important("Échec de l'enregistrement : disque plein"));
    assert!(important("Propagation : 47 images fabriquées en 41 s"));
    assert!(!important("Calque : décalage +10 × +0 px, largeur 100 %"));
    assert!(!important("Flou gaussien : coché."));
}

#[test]
fn a_gesture_updates_its_message_instead_of_piling_up() {
    let mut h = Vec::new();
    record(&mut h, 0.0, "Calque : décalage +1 px");
    record(&mut h, 0.1, "Calque : décalage +2 px");
    record(&mut h, 0.2, "Calque : décalage +3 px");
    assert_eq!(h.len(), 1, "le même geste");
    assert_eq!(h[0].1, "Calque : décalage +3 px");
    record(&mut h, 5.0, "Calque : décalage +9 px");
    record(&mut h, 5.1, "Enregistré dans le dossier « Bus »");
    assert_eq!(h.len(), 3);
    for i in 0..300 {
        record(&mut h, 10.0 + i as f64 * 2.0, &format!("message {i:03} différent"));
    }
    assert_eq!(h.len(), 200, "l'historique garde les 200 derniers");
}

#[test]
fn times_are_said_simply() {
    assert_eq!(ago(5.0), "à l'instant");
    assert_eq!(ago(190.0), "il y a 3 min");
    assert_eq!(ago(3_900.0), "il y a 1 h 05");
}