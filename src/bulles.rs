//! Boutons en icônes et leurs bulles d'aide (12d) : les commandes des barres et des panneaux
//! n'ont plus de texte ; quand la souris reste une seconde sur l'une d'elles, une bulle donne son
//! nom, son raccourci (lu dans le fichier de raccourcis) et une petite description.
//!
//! Les fonctions libres prennent le texte du raccourci ; les méthodes d'`App` prennent
//! l'identifiant de l'action et y lisent le raccourci.

use eframe::egui::{self, vec2, Response, RichText, Ui};

use crate::app::App;
use crate::theme;

/// Temps que la souris doit rester sur un bouton avant sa bulle, en secondes.
pub const DELAI: f32 = 1.0;
/// Taille des icônes des barres et des panneaux.
pub const TAILLE: f32 = 15.0;
/// Largeur la plus grande d'une bulle (la description passe à la ligne au-delà).
const LARGEUR: f32 = 280.0;

/// Contenu d'une bulle : le nom en gras et le raccourci sur la première ligne, la description
/// dessous, plus discrète.
pub(crate) fn contenu(ui: &mut Ui, nom: &str, description: &str, raccourci: &str) {
    ui.set_max_width(LARGEUR);
    ui.horizontal(|ui| {
        ui.label(RichText::new(nom).strong());
        if !raccourci.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new(raccourci).monospace().color(theme::texte_doux()));
        }
    });
    if !description.is_empty() {
        ui.label(RichText::new(description).color(theme::texte_doux()));
    }
}

/// Ajoute la bulle à un bouton (aussi quand il est grisé).
pub(crate) fn bulle(resp: Response, nom: &str, description: &str, raccourci: &str) -> Response {
    resp.on_hover_ui(|ui| contenu(ui, nom, description, raccourci))
        .on_disabled_hover_ui(|ui| contenu(ui, nom, description, raccourci))
}

fn texte(symbole: &str) -> RichText {
    RichText::new(symbole).size(TAILLE)
}

/// L'encre d'un bouton qui montre un état : quand il est allumé (`on`), la couleur lisible sur
/// l'accent (`theme::sur_accent`), dans tous les thèmes. egui dessine un `Button::selected` avec la
/// couleur de texte du survol, pas celle de la sélection : sur l'accent clair du thème Gris sombre,
/// l'icône restait claire sur clair. Tout bouton fait à la main avec `.selected(on)` passe par ici
/// (les `SelectableLabel` d'egui prennent déjà la bonne couleur).
pub(crate) fn encre_etat(texte: RichText, on: bool) -> RichText {
    if on {
        texte.color(theme::sur_accent())
    } else {
        texte
    }
}

/// L'icône d'un bouton d'état, à sa taille d'icône.
pub(crate) fn symbole_etat(symbole: &str, on: bool) -> RichText {
    encre_etat(texte(symbole), on)
}

/// Icône à la place du nom d'un réglage (devant un curseur ou une valeur), avec sa bulle.
pub(crate) fn etiquette(ui: &mut Ui, symbole: &str, nom: &str, description: &str) -> Response {
    let resp = ui.label(texte(symbole).color(theme::texte_doux()));
    bulle(resp, nom, description, "")
}

/// Bouton icône.
pub(crate) fn icone(ui: &mut Ui, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    icone_si(ui, true, symbole, nom, description, raccourci)
}

/// Bouton icône, grisé si `actif` est faux.
pub(crate) fn icone_si(ui: &mut Ui, actif: bool, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    let resp = ui.add_enabled(actif, egui::Button::new(texte(symbole)).min_size(vec2(24.0, 0.0)));
    bulle(resp, nom, description, raccourci)
}

/// Bouton icône qui montre un état : enfoncé quand `on`.
pub(crate) fn etat(ui: &mut Ui, on: bool, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    etat_si(ui, true, on, symbole, nom, description, raccourci)
}

/// Bouton icône qui montre un état, grisé si `actif` est faux.
pub(crate) fn etat_si(ui: &mut Ui, actif: bool, on: bool, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    let resp = ui.add_enabled(actif, egui::Button::new(symbole_etat(symbole, on)).selected(on).min_size(vec2(24.0, 0.0)));
    bulle(resp, nom, description, raccourci)
}

/// Interrupteur : bouton icône enfoncé quand `valeur` est vraie, qui la bascule au clic.
pub(crate) fn interrupteur(ui: &mut Ui, valeur: &mut bool, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    let mut resp = etat(ui, *valeur, symbole, nom, description, raccourci);
    if resp.clicked() {
        *valeur = !*valeur;
        resp.mark_changed();
    }
    resp
}

/// `interrupteur` d'une commande liée à `action` : clic droit, lui assigner un raccourci.
pub(crate) fn interrupteur_a(ui: &mut Ui, valeur: &mut bool, symbole: &str, nom: &str, description: &str, raccourci: &str, action: &str) -> Response {
    let resp = interrupteur(ui, valeur, symbole, nom, description, raccourci);
    crate::raccourcisui::shortcut_menu(&resp, action, raccourci);
    resp
}

/// `choix` d'une commande liée à `action` : clic droit, lui assigner un raccourci.
#[allow(clippy::too_many_arguments)]
pub(crate) fn choix_a<T: PartialEq>(ui: &mut Ui, valeur: &mut T, cette: T, symbole: &str, nom: &str, description: &str, raccourci: &str, action: &str) -> Response {
    let resp = choix(ui, valeur, cette, symbole, nom, description, raccourci);
    crate::raccourcisui::shortcut_menu(&resp, action, raccourci);
    resp
}

/// Choix entre des valeurs : un bouton icône par valeur, enfoncé quand c'est la valeur courante.
pub(crate) fn choix<T: PartialEq>(ui: &mut Ui, valeur: &mut T, cette: T, symbole: &str, nom: &str, description: &str, raccourci: &str) -> Response {
    let mut resp = etat(ui, *valeur == cette, symbole, nom, description, raccourci);
    if resp.clicked() && *valeur != cette {
        *valeur = cette;
        resp.mark_changed();
    }
    resp
}

impl App {
    /// Raccourci d'une action, tel que le fichier de raccourcis le donne (vide si aucun).
    pub(crate) fn raccourci(&self, action: &str) -> String {
        if action.is_empty() {
            String::new()
        } else {
            self.keymap.label(action)
        }
    }

    /// Bulle d'une commande liée à une action ; clic droit : lui assigner un raccourci.
    pub(crate) fn aide(&self, resp: Response, nom: &str, description: &str, action: &str) -> Response {
        let r = self.raccourci(action);
        crate::raccourcisui::shortcut_menu(&resp, action, &r);
        bulle(resp, nom, description, &r)
    }

    /// Bouton icône d'une commande ; `action` : son raccourci, montré dans la bulle (clic droit :
    /// l'assigner).
    pub(crate) fn bouton(&self, ui: &mut Ui, symbole: &str, nom: &str, description: &str, action: &str) -> Response {
        let r = self.raccourci(action);
        let resp = icone(ui, symbole, nom, description, &r);
        crate::raccourcisui::shortcut_menu(&resp, action, &r);
        resp
    }

    /// Bouton icône d'une commande, grisé si `actif` est faux.
    pub(crate) fn bouton_si(&self, ui: &mut Ui, actif: bool, symbole: &str, nom: &str, description: &str, action: &str) -> Response {
        let r = self.raccourci(action);
        let resp = icone_si(ui, actif, symbole, nom, description, &r);
        crate::raccourcisui::shortcut_menu(&resp, action, &r);
        resp
    }

    /// Bouton icône d'une commande qui montre un état (enfoncé quand `on`).
    pub(crate) fn bouton_etat(&self, ui: &mut Ui, on: bool, symbole: &str, nom: &str, description: &str, action: &str) -> Response {
        let r = self.raccourci(action);
        let resp = etat(ui, on, symbole, nom, description, &r);
        crate::raccourcisui::shortcut_menu(&resp, action, &r);
        resp
    }
}
