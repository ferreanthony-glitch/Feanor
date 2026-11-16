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

/// La rubrique des réglages des brosses : le registre y montre aussi ce que règle chaque catégorie (`Categorie::help`, qui reste
/// l'info-bulle de la catégorie).
pub(crate) const BROSSES: &str = "Réglages : les brosses";

const fn t(fenetre: &'static str, sujet: &'static str, texte: &'static str) -> Texte {
    Texte { fenetre, sujet, texte }
}

/// Tous les textes du registre : ceux des panneaux (ici), puis ceux des Préférences et des fenêtres (`aidefenetres.rs`).
pub(crate) fn tous() -> impl Iterator<Item = &'static Texte> {
    TEXTES.iter().chain(crate::aidefenetres::FENETRES.iter())
}

/// La rubrique des exports : le registre y montre ce que fait chaque export de la fenêtre « Exporter » (`prefsui::FAMILLES`, qui
/// reste l'info-bulle de son bouton).
pub(crate) const EXPORTS: &str = "Exporter";

/// Les textes des panneaux, dans l'ordre du registre.
pub(crate) static TEXTES: &[Texte] = &[
    // ------------------------------------------------------------------ Réglages (les textes sans animation)
    t(BROSSES, "Connexions", "Chaque réglage peut suivre l'appui, la vitesse, la direction du trait, un fondu ou le hasard, par une courbe (par exemple un réglage qui varie avec la pression ou la vitesse)."),
    t(BROSSES, "Couleur A→B", "La couleur d'arrière-plan (B) se mêle à celle d'avant-plan (A) selon la source."),
    t(BROSSES, "Mélange", "Modèle propre à FEANOR : à comparer avec tes outils de peinture."),
    t(BROSSES, "Toucher du stylet", "Réponse à la pression commune à tous les pinceaux, appliquée avant leurs réglages. « Ancien » : le toucher d'avant l'étude, pour comparer."),
    t(
        "Réglages : l'aplat",
        "Ce que fait l'outil sur l'aplat",
        "Aplat de <calque> : pot de peinture : clic : remplir la zone fermée par le trait, sous le trait. Dégradé : glisser : un dégradé dans la zone fermée par le trait, sous le trait (Alt : tout l'aplat). \
         Cutter : clic : retirer la zone fermée de l'aplat. Gomme : la gomme coupe l'aplat. \
         Sélection : lasso, rectangle, clic : prendre les pixels de l'aplat, la boîte les déplace, les met à l'échelle, les tourne (Ctrl+A : tout, sur plusieurs clés aussi). \
         Pinceau et crayon : peindre sous le trait, dans la forme (recoud l'aplat coupé). Clic sur le calque : revenir au trait. (La barre d'état le redit.)",
    ),
    t("Réglages : Transformer", "Clés de position", "Aucune clé : le calque garde sa pose fixe. Pose une clé, change d'image, déplace le calque."),
    t("Réglages : Transformer", "Arrivée, départ", "Combien d'images le calque met à ralentir en arrivant à cette clé, à accélérer en repartant (0 : vitesse constante)."),
];
