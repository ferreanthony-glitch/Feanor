//! Aide > À propos de FEANOR… (`aide.a_propos`) : une petite fenêtre modale avec le lockup de la marque (le mot, le Joyau
//! jade devant le mot, la devise « Forge your story » alignée sur le R), la version, les polices de marque et la mention de
//! ffmpeg (partie 124 : licence LGPL, dépliant « Licence et sources de ffmpeg » avec le texte de la licence, la configuration
//! exacte du ffmpeg livré, le ffmpeg que ce poste utilise, le dossier `ffmpeg-licence` de la version portable). Échap, un
//! clic à côté ou « Fermer » la ferment.
//!
//! La date de fabrication n'y est pas : la compilation ne l'écrit nulle part sans coûter une
//! recompilation à chaque fois.

use std::path::PathBuf;

use eframe::egui::{self, pos2, vec2, Align, Layout, RichText, Sense};

use crate::app::App;
use crate::logo::Joyau;
use crate::marque::{self, Mise};

/// Hauteur des capitales du mot dans la fenêtre, en points (le Joyau fait alors plus de 32 points).
const CAPITALES: f32 = 40.0;
/// Largeur de la fenêtre, en points.
const LARGEUR: f32 = 320.0;
/// Hauteur du texte de la licence déplié, en points (il défile).
const HAUTEUR_LICENCE: f32 = 220.0;

/// « Version 0.1.0 », d'après Cargo.toml.
pub(crate) fn version_text() -> String {
    format!("Version {}", env!("CARGO_PKG_VERSION"))
}

/// La ligne sur les polices de marque (licence SIL OFL 1.1, fichiers dans `assets/polices/`).
pub(crate) fn polices_text() -> &'static str {
    "Polices : Big Shoulders Display, IBM Plex (licence SIL OFL 1.1)"
}

/// La ligne sur ffmpeg, le programme séparé qui lit et écrit les vidéos et les sons (livré avec FEANOR ; obligation de la LGPL).
pub(crate) fn ffmpeg_text() -> &'static str {
    "Vidéo et son : FFmpeg 8.1 (licence GNU LGPL v2.1 ou suivante), programme séparé, construit sans composant GPL"
}

/// Le texte de la licence du ffmpeg livré avec FEANOR : la GNU Lesser General Public License v2.1 (`assets/licences/`).
pub(crate) const LICENCE_FFMPEG: &str = include_str!("../assets/licences/ffmpeg-LGPL-2.1.txt");
/// La ligne de configuration exacte du ffmpeg livré avec FEANOR (`Logiciel\ffmpeg\allege\construire.ps1`).
pub(crate) const CONFIGURATION_FFMPEG: &str = include_str!("../assets/licences/ffmpeg-configure.txt");
/// Le dossier, à côté de `feanor.exe` dans la version portable, qui porte la licence, la configuration et l'origine des sources.
pub(crate) const DOSSIER_LICENCE_FFMPEG: &str = "ffmpeg-licence";
/// Où sont les sources du ffmpeg livré (archive officielle, inchangée).
pub(crate) const SOURCES_FFMPEG: &str = "https://ffmpeg.org/releases/ffmpeg-8.1.3.tar.xz";

/// Ce que dit le dépliant sur ffmpeg.
pub(crate) fn ffmpeg_explication() -> &'static str {
    "FEANOR appelle ffmpeg comme un programme à part pour lire et écrire les vidéos et les sons ; il ne l'intègre pas. Le ffmpeg \
     livré avec FEANOR est FFmpeg 8.1.3, construit depuis les sources officielles, sans composant GPL ni bibliothèque de codec \
     extérieure, sous la GNU Lesser General Public License v2.1 ou suivante (texte ci-dessous). Ses sources sont l'archive \
     officielle de ffmpeg.org, à laquelle FEANOR ne change rien ; la version portable les cite dans son dossier « ffmpeg-licence ». \
     L'encodeur H.264 est celui de Windows (Media Foundation)."
}

/// Le dossier `ffmpeg-licence` à côté du programme, s'il y est (version portable).
pub(crate) fn dossier_licence_ffmpeg() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.join(DOSSIER_LICENCE_FFMPEG);
    dir.is_dir().then_some(dir)
}

impl App {
    /// La fenêtre « À propos de FEANOR ».
    pub(crate) fn about_window_ui(&mut self, ctx: &egui::Context) {
        if !self.about_window {
            return;
        }
        let mut close = false;
        // Le ffmpeg que ce poste utilise, cherché une fois par ouverture de la fenêtre (il faut sonder le disque).
        let cle_ffmpeg = egui::Id::new("a_propos_ffmpeg_trouve");
        let saved = self.settings.ffmpeg.clone();
        let trouve: Option<PathBuf> = ctx.data_mut(|d| d.get_temp_mut_or_insert_with(cle_ffmpeg, || crate::export::find_ffmpeg(saved.as_deref())).clone());
        let modal = egui::Modal::new(egui::Id::new("a_propos_feanor")).show(ctx, |ui| {
            ui.set_width(LARGEUR);
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                let p = crate::theme::palette();
                // (Le lockup rétrécit plutôt que d'être coupé.)
                let capitales = marque::ajuster(ctx, ui.available_width() - 8.0, CAPITALES, Mise::Devant);
                let mesure = marque::disposer(ctx, pos2(0.0, 0.0), capitales, true, Mise::Devant).rect;
                let (zone, _) = ui.allocate_exact_size(vec2(mesure.width(), mesure.height()), Sense::hover());
                let texture = marque::texture_joyau(ctx, "apropos", marque::cote_joyau(capitales), Joyau::Jade, None);
                let origine = pos2(zone.left() - mesure.min.x, zone.top() - mesure.min.y);
                marque::peindre(ui.painter(), ctx, &texture, origine, capitales, p.texte_fort, Some(p.texte_doux), Mise::Devant);
                ui.add_space(14.0);
                ui.label(RichText::new(version_text()).strong());
                ui.label(RichText::new(polices_text()).small().weak());
                ui.label(RichText::new(ffmpeg_text()).small().weak());
                ui.add_space(6.0);
            });
            // La licence de ffmpeg (LGPL) : son texte, la configuration exacte, le ffmpeg de ce poste, le dossier de la version portable.
            egui::CollapsingHeader::new(RichText::new("Licence et sources de ffmpeg").small()).id_salt("a_propos_ffmpeg").show(ui, |ui| {
                ui.label(RichText::new(ffmpeg_explication()).small());
                let a_cote = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
                match &trouve {
                    Some(p) if a_cote.as_deref().is_some_and(|d| p.parent() == Some(d)) => {
                        ui.label(RichText::new(format!("Sur ce poste : {} (celui livré avec FEANOR).", p.display())).small().weak());
                    }
                    Some(p) => {
                        ui.label(RichText::new(format!("Sur ce poste : {} (un autre ffmpeg, qui a sa propre licence ; Préférences > Export).", p.display())).small().weak());
                    }
                    None => {
                        ui.label(RichText::new("Sur ce poste : aucun ffmpeg trouvé (Préférences > Export).").small().weak());
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if let Some(dossier) = dossier_licence_ffmpeg() {
                        if ui.small_button("Ouvrir le dossier ffmpeg-licence").on_hover_text(dossier.display().to_string()).clicked() {
                            crate::exportui::reveal_in_explorer(&dossier);
                        }
                    }
                    if ui.small_button("Les sources (ffmpeg.org)").on_hover_text(SOURCES_FFMPEG).clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(SOURCES_FFMPEG));
                    }
                });
                egui::ScrollArea::vertical().id_salt("a_propos_licence_ffmpeg").max_height(HAUTEUR_LICENCE).show(ui, |ui| {
                    ui.label(RichText::new(LICENCE_FFMPEG).small().monospace());
                    ui.add_space(6.0);
                    ui.label(RichText::new("Configuration exacte du ffmpeg livré :").small().strong());
                    ui.label(RichText::new(CONFIGURATION_FFMPEG.trim()).small().weak());
                });
            });
            ui.add_space(4.0);
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Fermer").clicked() {
                    close = true;
                }
            });
        });
        if close || modal.should_close() {
            self.about_window = false;
            ctx.data_mut(|d| d.remove_temp::<Option<PathBuf>>(cle_ffmpeg));
        }
    }
}