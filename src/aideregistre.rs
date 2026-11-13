//! Aide > « Registre des explications » (partie 153) : toutes les animations de l'aide (`aideanim.rs`), rangées par outil, chacune
//! rejouable d'un clic sur sa vignette, avec son texte ; puis les textes des panneaux, des Préférences et des fenêtres, sans
//! animation, rangés par fenêtre (`aidetextes.rs`). C'est le seul endroit où ces textes vivent désormais.

use eframe::egui::{self, vec2, RichText, Sense};

use crate::aideanim::{self, peindre, Anim, Teintes};
use crate::app::App;

/// La largeur d'une vignette du registre.
const VIGNETTE: f32 = 176.0;
/// Le temps laissé sur la dernière image d'une animation rejouée dans le registre, avant le repos.
const TENUE: f64 = 0.6;

/// Les rubriques des animations, dans l'ordre du catalogue (chacune une fois).
pub(crate) fn rubriques() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for a in aideanim::toutes() {
        if !out.contains(&a.rubrique) {
            out.push(a.rubrique);
        }
    }
    out
}

/// Les fenêtres des textes sans animation, dans l'ordre du catalogue (chacune une fois).
pub(crate) fn fenetres() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for t in crate::aidetextes::TEXTES {
        if !out.contains(&t.fenetre) {
            out.push(t.fenetre);
        }
    }
    out
}

impl App {
    /// La fenêtre du registre (ouverte par `aide.registre`).
    pub(crate) fn aide_registre_fenetre(&mut self, ctx: &egui::Context) {
        if !self.confort.aide.registre {
            return;
        }
        let mut ouverte = true;
        egui::Window::new("Registre des explications")
            .open(&mut ouverte)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .default_height(640.0)
            .show(ctx, |ui| self.aide_registre_contenu(ui));
        if !ouverte {
            self.confort.aide.registre = false;
            self.confort.aide.registre_joue = None;
        }
    }

    fn aide_registre_contenu(&mut self, ui: &mut egui::Ui) {
        let maintenant = ui.input(|i| i.time);
        // L'animation rejouée s'arrête à sa fin : plus de repaint ensuite.
        if let Some((id, debut)) = self.confort.aide.registre_joue {
            if aideanim::anim(id).is_none_or(|a| maintenant - debut > a.duree as f64 + TENUE) {
                self.confort.aide.registre_joue = None;
            } else {
                ui.ctx().request_repaint();
            }
        }
        let afficher = self.settings.afficher_explications;
        ui.horizontal(|ui| {
            let mut on = afficher;
            let resp = ui.checkbox(&mut on, "Afficher les explications à chaque prise d'un outil");
            if crate::bulles::bulle(resp, "Afficher les explications", "Cochée : l'animation d'un outil rejoue chaque fois qu'on le prend. Décochée : une seule fois, la première ; le bouton ▶ des Réglages la rejoue.", &self.raccourci("aide.explications")).changed() {
                self.aide_basculer_explications();
            }
        });
        ui.separator();
        let contexte = self.aide_contexte();
        let teintes = Teintes::du_theme();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("Les outils et les gestes").strong());
            for rubrique in rubriques() {
                let anims: Vec<&'static Anim> = aideanim::toutes().filter(|a| a.rubrique == rubrique).collect();
                egui::CollapsingHeader::new(RichText::new(rubrique).strong()).id_salt(("registre", rubrique)).default_open(true).show(ui, |ui| {
                    for a in anims {
                        self.aide_registre_ligne(ui, a, maintenant, &teintes, a.contextes.contains(&contexte));
                    }
                });
            }
            ui.add_space(10.0);
            ui.label(RichText::new("Les panneaux et les fenêtres").strong());
            for fenetre in fenetres() {
                egui::CollapsingHeader::new(RichText::new(fenetre).strong()).id_salt(("registre_fenetre", fenetre)).default_open(true).show(ui, |ui| {
                    for t in crate::aidetextes::TEXTES.iter().filter(|t| t.fenetre == fenetre) {
                        if !t.sujet.is_empty() {
                            ui.label(RichText::new(t.sujet).strong());
                        }
                        ui.label(aideanim::avec_touches(t.texte, |a| self.raccourci(a)));
                        ui.add_space(4.0);
                    }
                });
            }
        });
    }

    /// Une animation du registre : sa vignette (un clic la rejoue) et son texte.
    fn aide_registre_ligne(&mut self, ui: &mut egui::Ui, a: &'static Anim, maintenant: f64, teintes: &Teintes, de_l_outil: bool) {
        ui.horizontal_top(|ui| {
            let taille = vec2(VIGNETTE, VIGNETTE * aideanim::HAUTEUR / aideanim::LARGEUR);
            let (zone, resp) = ui.allocate_exact_size(taille + vec2(6.0, 6.0), Sense::click());
            let vignette = zone.shrink(3.0);
            let joue = self.confort.aide.registre_joue.filter(|(id, _)| *id == a.id).map(|(_, d)| (maintenant - d) as f32);
            peindre(ui.painter(), vignette, a, joue.unwrap_or(a.duree), teintes);
            if joue.is_none() {
                // Le bouton ▶ sur la vignette au repos.
                let c = vignette.right_bottom() - vec2(12.0, 12.0);
                ui.painter().circle_filled(c, 9.0, teintes.cadre);
                ui.painter().text(c, egui::Align2::CENTER_CENTER, egui_phosphor::regular::PLAY, egui::FontId::proportional(11.0), teintes.jade);
            }
            if crate::bulles::bulle(resp, "Rejouer", "Un clic rejoue l'animation.", "").clicked() {
                self.confort.aide.registre_joue = Some((a.id, maintenant));
                ui.ctx().request_repaint();
            }
            ui.vertical(|ui| {
                let titre = RichText::new(a.titre).strong();
                ui.label(if de_l_outil { titre.color(crate::theme::accent_texte()) } else { titre });
                for paragraphe in aideanim::avec_touches(a.texte, |x| self.raccourci(x)).split('\n') {
                    ui.label(paragraphe);
                }
            });
        });
        ui.add_space(6.0);
    }
}
