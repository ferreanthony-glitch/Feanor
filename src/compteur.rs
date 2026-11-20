//! Le compteur des bandes du bas (partie 195, Anthony : « ctrl + clic : bascule entre le temps et les numéros d'images ») : celui de la
//! bande du Storyboard (« 0:00:00 / 0:22:00 », la position dans le film et sa durée) et celui de la timeline de l'Animation (l'image
//! courante et la dernière du plan). **Ctrl + clic** sur l'un d'eux passe du temps aux numéros d'images, et retour ; le choix est
//! retenu dans les réglages de l'application (`compteur_en_images`, `compteur_timeline_en_temps`).

use crate::app::App;
use crate::cues::TimeDisplay;

/// Le texte du compteur du Storyboard : l'image `g` (comptée depuis 0) du film de `total` images, en temps (« 0:02:15 / 0:22:00 ») ou
/// en numéros d'images, l'image 0 étant la première (« image 63 / 528 »).
pub fn texte(g: usize, total: usize, fps: u32, en_images: bool) -> String {
    if en_images {
        format!("image {} / {}", g + 1, total.max(1))
    } else {
        format!("{} / {}", crate::storyboard::timecode(g, fps), crate::storyboard::timecode(total, fps))
    }
}

impl App {
    /// Ctrl + clic sur le compteur du Storyboard : le temps ou les numéros d'images, retenu dans les réglages.
    pub(crate) fn basculer_compteur(&mut self) {
        self.settings.compteur_en_images = !self.settings.compteur_en_images;
        let _ = self.settings.save();
        self.status = if self.settings.compteur_en_images { "Compteur en numéros d'images (Ctrl + clic : le temps).".into() } else { "Compteur en temps (Ctrl + clic : les numéros d'images).".into() };
    }

    /// Ctrl + clic sur le compteur de la timeline : les numéros d'images ou le time code, retenu dans les réglages.
    pub(crate) fn basculer_compteur_timeline(&mut self) {
        self.time_display = if self.time_display == TimeDisplay::TimeCode { TimeDisplay::Image } else { TimeDisplay::TimeCode };
        self.settings.compteur_timeline_en_temps = self.time_display == TimeDisplay::TimeCode;
        let _ = self.settings.save();
        self.status = if self.settings.compteur_timeline_en_temps { "Timeline en time code (Ctrl + clic : les numéros d'images).".into() } else { "Timeline en numéros d'images (Ctrl + clic : le time code).".into() };
    }
}
