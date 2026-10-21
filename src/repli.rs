//! Les sections qu'on déplie (passe de simplicité, 2026-10-21) : une petite flèche et un titre discret ; le
//! reste du panneau n'est montré que quand elle est ouverte. Le panneau Réglages (« Plus de réglages »), le
//! panneau Couleur (« Plus de couleurs ») et les effets du panneau Effets s'en servent : même geste partout.
//! L'état est tenu par l'appelant (réglages de l'application, ou la séance seulement).

use eframe::egui::{self, RichText, Sense, Ui};
use egui_phosphor::regular as ph;

/// La rangée de la flèche : un clic sur elle ou sur le titre bascule `ouvert`. Rend vrai quand ça vient de
/// basculer. `explication` : la bulle de la flèche, sous son nom (« Déplier » ou « Replier » le titre).
pub(crate) fn entete(ui: &mut Ui, ouvert: &mut bool, titre: &str, explication: &str) -> bool {
    let (symbole, nom) = if *ouvert { (ph::CARET_DOWN, format!("Replier « {titre} »")) } else { (ph::CARET_RIGHT, format!("Déplier « {titre} »")) };
    let mut bascule = false;
    ui.horizontal(|ui| {
        bascule |= crate::bulles::icone(ui, symbole, &nom, explication, "").clicked();
        let titre = ui.add(egui::Label::new(RichText::new(titre).small().weak()).sense(Sense::click()));
        bascule |= titre.clicked();
    });
    if bascule {
        *ouvert = !*ouvert;
    }
    bascule
}
