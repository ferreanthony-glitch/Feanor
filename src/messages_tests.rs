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
fn the_status_bar_shows_one_short_tip_and_the_bubble_keeps_the_detail() {
    use crate::messages::resume;
    // Un message court reste tel quel.
    assert_eq!(resume("Enregistré."), "Enregistré.");
    assert_eq!(resume("Vignettes : dessine dans les cases."), "Vignettes : dessine dans les cases.");
    // Un long message est coupé avant sa première parenthèse, son premier point-virgule ou son premier point.
    let long = "Vignettes : dessine dans les cases (Ctrl + Maj + glisser un cadre : mouvement de caméra ; Tab : retour au storyboard).";
    assert_eq!(resume(long), "Vignettes : dessine dans les cases…");
    let sans_parenthese = "Espace 3D : clic sur une carte (un calque) ou sur la caméra pour la choisir, glisser pour la déplacer, clic droit pour tourner la vue.";
    assert_eq!(resume(sans_parenthese), "Espace 3D : clic sur une carte…");
    assert_eq!(resume("Calque renommé en « Fond de ciel ». Annule avec Ctrl+Z si ce n'était pas voulu et que tu veux revenir."), "Calque renommé en « Fond de ciel »…");
    // Un long message sans coupure naturelle n'est pas mutilé (l'étiquette de la barre le tronque à la largeur).
    let plat = "Un très long message sans aucune ponctuation qui permette de le couper proprement ni nulle part ailleurs";
    assert_eq!(resume(plat), plat);
    // Une coupure trop tôt (moins de douze caractères) ne compte pas.
    let tot = "Oui (vraiment) c'est une très longue phrase qui continue encore et encore sans fin visible ici.";
    assert!(resume(tot).starts_with("Oui (vraiment)"), "{}", resume(tot));
    // Chaque message d'espace de travail tient dans la barre sans coupure.
    for e in crate::workspace::Espace::TOUS {
        let (_ctx, app) = {
            let ctx = eframe::egui::Context::default();
            let dir = std::env::temp_dir().join(format!("shot-messages-{}-{:?}", std::process::id(), std::thread::current().id()));
            let _ = std::fs::create_dir_all(&dir);
            crate::donnees::bac_a_sable(&dir);
            let app = crate::app::App::sans_fenetre(&ctx, None);
            (ctx, app)
        };
        let mut app = app;
        app.set_espace(e);
        assert!(app.status.chars().count() <= 70, "{e:?} : « {} »", app.status);
        assert_eq!(resume(&app.status), app.status.trim());
    }
}

#[test]
fn times_are_said_simply() {
    assert_eq!(ago(5.0), "à l'instant");
    assert_eq!(ago(190.0), "il y a 3 min");
    assert_eq!(ago(3_900.0), "il y a 1 h 05");
}