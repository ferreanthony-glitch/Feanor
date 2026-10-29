//! Les sortes de calques et le menu du « + » du panneau Calques (passe de simplicité du 2026-10-21) :
//! un seul bouton pour créer un calque, qui ouvre un petit menu avec, pour chaque sorte, une icône et
//! une phrase (dessin vectoriel, dessin bitmap, calque de couleur, calque d'effet, vidéo, dossier).
//! Les mêmes entrées servent au menu Calques, au clic droit dans le vide du panneau et à la barre des
//! commandes : la création n'est plus éparpillée. Chaque entrée lance l'action qu'elle avait déjà
//! (`calque.nouveau_vectoriel`…) : les raccourcis et la palette de commandes ne changent pas.
//!
//! La même icône, sans le mot, dit la sorte de chaque ligne du panneau (`Sorte::of`).

use eframe::egui::{self, text::LayoutJob, vec2, Color32, FontId, Response, RichText, TextFormat};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::document::{Layer, LayerKind};

/// Les sortes de calques qu'on peut créer, et que le panneau montre par une icône.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sorte {
    Vecto,
    Bitmap,
    Couleur,
    Effet,
    /// Des notes de réalisateur sur le plan, jamais dans un export (`calqueannot.rs`).
    Annotation,
    /// Un fond d'une seule couleur sur toute la feuille, avec des dessins par-dessus (`couleurunie.rs`).
    Unie,
    Video,
    Dossier,
}

impl Sorte {
    /// Dans l'ordre du menu du « + ».
    pub(crate) const TOUTES: [Sorte; 8] = [Sorte::Vecto, Sorte::Bitmap, Sorte::Couleur, Sorte::Unie, Sorte::Effet, Sorte::Annotation, Sorte::Video, Sorte::Dossier];

    /// La sorte d'un calque, telle que la ligne du panneau la montre : un dossier, un calque d'effet, un calque
    /// d'annotation, un calque de couleur ou une vidéo avant le vectoriel et le bitmap.
    pub(crate) fn of(l: &Layer) -> Sorte {
        if l.folder {
            Sorte::Dossier
        } else if l.effet.is_some() {
            Sorte::Effet
        } else if l.annotation {
            Sorte::Annotation
        } else if l.couleur.is_some() {
            Sorte::Couleur
        } else if l.unie.is_some() {
            Sorte::Unie
        } else if l.video.is_some() {
            Sorte::Video
        } else if l.kind == LayerKind::Vector {
            Sorte::Vecto
        } else {
            Sorte::Bitmap
        }
    }

    pub(crate) fn icone(self) -> &'static str {
        match self {
            Sorte::Vecto => ph::BEZIER_CURVE,
            Sorte::Bitmap => ph::PAINT_BRUSH_BROAD,
            Sorte::Couleur => ph::PALETTE,
            Sorte::Effet => ph::SPARKLE,
            Sorte::Annotation => ph::MARKER_CIRCLE,
            Sorte::Unie => ph::SQUARE,
            Sorte::Video => ph::FILM_STRIP,
            Sorte::Dossier => ph::FOLDER_PLUS,
        }
    }

    /// L'icône d'une ligne du panneau : celle du menu, sauf le dossier (le « + » du menu n'a pas de sens sur une ligne).
    pub(crate) fn icone_ligne(self) -> &'static str {
        if self == Sorte::Dossier {
            ph::FOLDER
        } else {
            self.icone()
        }
    }

    /// Le nom, dans le menu et dans la bulle de l'icône d'une ligne.
    pub(crate) fn nom(self) -> &'static str {
        match self {
            Sorte::Vecto => "Dessin vectoriel",
            Sorte::Bitmap => "Dessin bitmap",
            Sorte::Couleur => "Calque de couleur",
            Sorte::Effet => "Calque d'effet",
            Sorte::Annotation => "Calque d'annotation",
            Sorte::Unie => "Couleur unie",
            Sorte::Video => "Vidéo",
            Sorte::Dossier => "Dossier",
        }
    }

    /// Une phrase.
    pub(crate) fn phrase(self) -> &'static str {
        match self {
            Sorte::Vecto => "Des traits qui restent modifiables.",
            Sorte::Bitmap => "De la peinture en pixels, pinceaux texturés.",
            Sorte::Couleur => "Pour colorier sous le trait, sans l'abîmer.",
            Sorte::Effet => "Un effet sur tout ce qui est dessous : lueur, flash, grain…",
            Sorte::Annotation => "Des notes par-dessus le plan, jamais dans les exports.",
            Sorte::Unie => "Un fond d'une seule couleur sur toute la feuille ; on dessine par-dessus.",
            Sorte::Video => "Une vidéo posée comme calque, pour dessiner par-dessus.",
            Sorte::Dossier => "Pour ranger des calques ensemble.",
        }
    }

    /// L'action que lance l'entrée du menu (le calque d'effet ouvre à la place ses préréglages).
    pub(crate) fn action(self) -> Option<&'static str> {
        match self {
            Sorte::Vecto => Some("calque.nouveau_vectoriel"),
            Sorte::Bitmap => Some("calque.nouveau_bitmap"),
            Sorte::Couleur => Some("calque.nouveau_couleur"),
            Sorte::Effet => None,
            Sorte::Annotation => Some("calque.nouveau_annotation"),
            Sorte::Unie => Some("calque.nouveau_couleur_unie"),
            Sorte::Video => Some("fichier.importer_video"),
            Sorte::Dossier => Some("dossier.nouveau"),
        }
    }

    /// Les actions qui créent un calque depuis le menu du « + » (essais : aucune ne se perd). Le calque d'effet vide
    /// est dans le sous-menu des préréglages du calque d'effet (`prereglagesui.rs`).
    #[cfg(test)]
    pub(crate) fn actions_du_menu() -> Vec<&'static str> {
        let mut v: Vec<&'static str> = Sorte::TOUTES.iter().filter_map(|s| s.action()).collect();
        v.extend(["effet.vide", ACTION_AVEC_NOM]);
        v
    }
}

/// « Nom, groupe, place… » : la fenêtre « Nouveau calque ».
pub(crate) const ACTION_AVEC_NOM: &str = "calque.nouveau";

/// Une entrée de menu à deux lignes : l'icône et le nom, la phrase dessous en plus discret.
fn entree(ui: &egui::Ui, icone: &str, teinte: Color32, nom: &str, phrase: &str) -> LayoutJob {
    let mut job = LayoutJob::default();
    let visuals = ui.visuals();
    job.append(&format!("{icone}  "), 0.0, TextFormat { font_id: FontId::proportional(16.0), color: teinte, ..Default::default() });
    job.append(nom, 0.0, TextFormat { font_id: FontId::proportional(13.5), color: visuals.text_color(), ..Default::default() });
    job.append("\n", 0.0, TextFormat::default());
    job.append(phrase, 0.0, TextFormat { font_id: FontId::proportional(11.0), color: visuals.weak_text_color(), ..Default::default() });
    job
}

/// La couleur d'une sorte : celle de la bande de ses lignes dans le panneau.
pub(crate) fn teinte(ui: &egui::Ui, s: Sorte) -> Color32 {
    match s {
        Sorte::Vecto => crate::theme::couleur_calque(LayerKind::Vector, false),
        Sorte::Bitmap | Sorte::Video => crate::theme::couleur_calque(LayerKind::Bitmap, false),
        Sorte::Couleur => crate::theme::couleur_calque(LayerKind::Bitmap, true),
        Sorte::Effet => crate::theme::palette().dessin_cfx,
        Sorte::Annotation => crate::theme::palette().dessin_annotation,
        Sorte::Unie => crate::theme::palette().dessin_unie,
        Sorte::Dossier => ui.visuals().selection.bg_fill,
    }
}

impl App {
    /// Le bouton « + » du panneau Calques : un menu des sortes de calques à créer.
    pub(crate) fn layer_plus_button(&mut self, ui: &mut egui::Ui) {
        let button = egui::Button::new(RichText::new(ph::PLUS).size(crate::bulles::TAILLE)).min_size(vec2(24.0, 0.0));
        let resp = egui::menu::menu_custom_button(ui, button, |ui| self.layer_new_menu(ui)).response;
        crate::bulles::bulle(resp, "Nouveau calque", "Dessin vectoriel ou bitmap, calque de couleur, calque d'effet, vidéo, dossier.", "");
    }

    /// Le menu « Nouveau calque » : une entrée par sorte (icône, nom, une phrase), puis la fenêtre avec le nom, le
    /// groupe et la place. Aussi dans le menu Calques et le clic droit dans le vide du panneau.
    pub(crate) fn layer_new_menu(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(250.0);
        for s in Sorte::TOUTES {
            let job = entree(ui, s.icone(), teinte(ui, s), s.nom(), s.phrase());
            match s.action() {
                Some(action) => {
                    let shortcut = self.keymap.label(action);
                    let resp = ui.add(egui::Button::new(job).shortcut_text(&shortcut));
                    self.entree_creation(ui, &resp, action);
                }
                // Le calque d'effet : un sous-menu de préréglages (impact frame, lueur, flash…).
                None => {
                    ui.menu_button(job, |ui| self.effect_preset_menu(ui));
                }
            }
        }
        ui.separator();
        self.action_item(ui, "Nom, groupe, place…", ACTION_AVEC_NOM);
    }

    /// Clic sur une entrée de création : l'action ; clic droit : lui assigner un raccourci.
    fn entree_creation(&mut self, ui: &mut egui::Ui, resp: &Response, action: &str) {
        if resp.clicked() {
            ui.close_menu();
            self.run_action(action);
        } else if resp.secondary_clicked() {
            ui.close_menu();
            self.start_assign(action);
        }
    }
}
