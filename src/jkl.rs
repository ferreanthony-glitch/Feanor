//! J, K, L du montage : la lecture à vitesse variable, les gestes habituels des monteurs (partie 104 de `docs/INTERFACE.md`).
//!
//! - **L** lit en avant ; un nouvel appui accélère (x2, x4, x8).
//! - **J** lit en arrière, de même.
//! - **K** arrête.
//! - **K tenue + L** (ou **J**) : une image en avant (ou en arrière), pour chaque appui.
//! - L pendant une lecture arrière la ralentit d'un cran, puis l'arrête (J de même pendant une lecture avant) : ce que fait un
//!   logiciel de montage.
//!
//! Le son suit à la vitesse normale (x1 en avant) ; à toute autre vitesse il est coupé net et revient quand on repasse à x1
//! (`soundtrack::start_audio_playback`). Les touches ne jouent que dans l'espace Montage (`contexte = "montage"` dans
//! `keymap.toml`, `Keymap::find_in`) et jamais pendant une saisie de texte (`App::handle_shortcuts`). Rien de tout cela ne touche au
//! film : seule la tête de lecture bouge.

use crate::app::App;
use crate::workspace::Espace;

/// Les vitesses, en multiples de la cadence du film.
pub(crate) const PALIERS: [f32; 4] = [1.0, 2.0, 4.0, 8.0];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Touche {
    J,
    K,
    L,
}

fn rang(v: f32) -> usize {
    PALIERS.iter().position(|p| (p - v.abs()).abs() < 1e-3).unwrap_or(0)
}

/// La vitesse (signée : négative en arrière, 0 à l'arrêt) après un appui sur `touche`.
pub(crate) fn apres(vitesse: f32, touche: Touche) -> f32 {
    let sens = match touche {
        Touche::K => return 0.0,
        Touche::L => 1.0,
        Touche::J => -1.0,
    };
    if vitesse == 0.0 {
        sens
    } else if vitesse.signum() == sens {
        sens * PALIERS[(rang(vitesse) + 1).min(PALIERS.len() - 1)]
    } else {
        // L'autre sens que celui qu'on lit : on ralentit d'un cran, puis on s'arrête.
        match rang(vitesse) {
            0 => 0.0,
            i => vitesse.signum() * PALIERS[i - 1],
        }
    }
}

/// Ce que dit la barre d'état.
pub(crate) fn texte(vitesse: f32) -> String {
    if vitesse == 0.0 {
        "Lecture arrêtée.".into()
    } else {
        let sens = if vitesse < 0.0 { "arrière" } else { "avant" };
        format!("Lecture en {sens}, x{}.", vitesse.abs())
    }
}

impl App {
    /// Un appui sur J, K ou L (les actions `montage.lecture_*`). Sans effet hors du Montage.
    pub(crate) fn lecture_touche(&mut self, touche: Touche) {
        if self.espace != Espace::Montage {
            return;
        }
        // K tenue : J et L avancent image par image.
        let k_tenue = touche != Touche::K && self.ctx.input(|i| self.keymap.held_any(i, "montage.lecture_arret"));
        if k_tenue {
            self.playing = false;
            self.stop_audio();
            self.play_speed = 1.0;
            self.step_global(if touche == Touche::L { 1 } else { -1 });
            return;
        }
        let actuelle = if self.playing { self.play_speed } else { 0.0 };
        self.regler_vitesse(apres(actuelle, touche));
    }

    /// Lit à la vitesse signée `vitesse` (0 : arrête).
    pub(crate) fn regler_vitesse(&mut self, vitesse: f32) {
        if vitesse == 0.0 {
            self.playing = false;
            self.stop_audio();
            self.play_speed = 1.0;
            self.status = texte(0.0);
            return;
        }
        let etait_lu = self.playing;
        let avant = self.play_speed;
        if !etait_lu {
            self.end_stroke();
            self.play_montage = true;
            // Au bout du film, dans le sens de la lecture : on repart de l'autre bout.
            let (g, total) = (self.global_frame(), self.doc.total_frames());
            if vitesse > 0.0 && g + 1 >= total {
                self.goto_global(0);
            } else if vitesse < 0.0 && g == 0 {
                self.goto_global(total.saturating_sub(1));
            }
            self.playing = true;
            self.last_time = None;
            self.play_accum = 0.0;
        }
        self.play_speed = vitesse;
        if vitesse == 1.0 {
            // Vitesse normale en avant : le son revient, à l'image où l'on est.
            if !etait_lu || avant != 1.0 {
                self.stop_audio();
                self.start_audio_playback();
                self.last_time = None;
                self.play_accum = 0.0;
            }
        } else {
            self.stop_audio();
        }
        self.status = texte(vitesse);
    }
}
