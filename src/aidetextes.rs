//! Les textes d'explication sans animation (partie 153) : ceux des panneaux qui n'ont pas d'animation à eux, des Préférences et des
//! fenêtres. Ils ont quitté leur fenêtre (Anthony, 2026-11-13 : « les textes explicatifs gris disparaissent des panneaux ») et ne
//! vivent plus que dans Aide > « Registre des explications » (`aideregistre.rs`), rangés par fenêtre.
//!
//! Le contenu est celui qui était écrit dans la fenêtre ; `{commande}` s'écrit « (touche) », lue dans le fichier de raccourcis.

/// Un texte du registre.
pub(crate) struct Texte {
    /// La fenêtre ou le panneau d'où il vient (la rubrique du registre).
    pub fenetre: &'static str,
    /// Ce dont il parle (un réglage, une partie de la fenêtre) ; vide : la fenêtre entière.
    pub sujet: &'static str,
    pub texte: &'static str,
}

/// Tous les textes, dans l'ordre du registre.
pub(crate) static TEXTES: &[Texte] = &[];
