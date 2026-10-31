//! Fenêtre « Paquet de livraison » (Fichier > Exporter, action `fichier.exporter_paquet`) : ce qu'il contient, le dossier, le modèle de chemin avec son aperçu,
//! puis la tâche de fond (`paquet.rs`) avec sa progression et « Arrêter » (qui efface tout ce qui a été écrit).
//!
//! Règle « sans latence » : l'interface ne calcule rien. Le projet est copié une fois au départ (quelques images d'écran : `PaquetWindow::rendering`, lu par
//! `App::modal_open`), puis tout se fait sur d'autres fils ; le projet est libre et le paquet continue si l'on change d'onglet.

use std::path::Path;
use std::time::Duration;

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::paquet::{self, PaquetJob, Rapport};
use crate::theme;

pub(crate) struct PaquetWindow {
    pub(crate) job: Option<PaquetJob>,
    pub(crate) result: Option<Result<Rapport, String>>,
    /// La vérification des noms, recalculée seulement quand le projet ou le modèle change.
    cache: Option<((u64, u64, String), crate::nommage::Verification)>,
}

impl PaquetWindow {
    pub(crate) fn rendering(&self) -> bool {
        self.job.as_ref().is_some_and(PaquetJob::needs_project)
    }

    pub(crate) fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(PaquetJob::running)
    }
}

fn court(p: &Path) -> String {
    let parts: Vec<String> = p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.len() <= 3 {
        p.display().to_string()
    } else {
        format!("…\\{}", parts[parts.len() - 2..].join("\\"))
    }
}

impl App {
    /// Fichier > Exporter > Paquet de livraison.
    pub(crate) fn open_paquet(&mut self) {
        self.end_stroke();
        self.playing = false;
        if self.paquet.as_ref().is_some_and(PaquetWindow::busy) {
            return;
        }
        self.paquet = Some(PaquetWindow { job: None, result: None, cache: None });
    }

    fn paquet_lancer(&mut self, w: &mut PaquetWindow) {
        w.result = None;
        let r = self.settings.paquet.clone();
        let Some(dossier) = r.dossier.clone() else {
            w.result = Some(Err("Choisis d'abord le dossier où ranger le paquet.".into()));
            return;
        };
        let ffmpeg = match self.ffmpeg_info().cloned() {
            Some((p, Ok(enc))) => Some((p, enc)),
            _ => None,
        };
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
        self.playing = false;
        match PaquetJob::start(&self.doc, &r, self.cur_panel, ffmpeg, &dossier, self.rejeu_idle.clone()) {
            Ok(job) if r.journal => {
                // Le journal des différences : les deux côtés proposés d'origine (`journalui.rs`), préparés ici, calculés sur un autre fil.
                let (a, b) = self.journal_defauts();
                match a.ok_or_else(|| "Il n'y a pas encore d'envoi : le journal des différences n'a rien à comparer.".to_string()).and_then(|a| self.journal_demande(a, b, None, None)) {
                    Ok(d) => w.job = Some(job.avec_journal(d)),
                    // (Le paquet abandonné efface son dossier de travail en se fermant.)
                    Err(e) => w.result = Some(Err(e)),
                }
            }
            Ok(job) => w.job = Some(job),
            Err(e) => w.result = Some(Err(e)),
        }
    }

    pub(crate) fn paquet_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.paquet.take() else { return };
        if let Some(job) = &mut w.job {
            job.step(&self.doc, Duration::from_millis(8));
            if job.running() {
                if job.needs_project() {
                    ctx.request_repaint();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                self.status = format!("Paquet de livraison : {}", job.status().0);
            }
        }
        if w.job.as_ref().is_some_and(|j| !j.running()) {
            let mut job = w.job.take().expect("paquet");
            let res = job.outcome.take().unwrap_or_else(|| Err("Export annulé.".into()));
            self.status = match &res {
                Ok(r) => format!("Paquet de livraison écrit : {} ({} fichiers)", r.racine.display(), r.fichiers),
                Err(e) => format!("Paquet de livraison : {e}"),
            };
            w.result = Some(res);
        }
        let mut ouverte = true;
        let mut lancer = false;
        egui::Window::new("Paquet de livraison")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(540.0)
            .default_pos(ctx.screen_rect().center() - vec2(270.0, 270.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| lancer = self.paquet_body(ui, &mut w));
        if lancer {
            self.paquet_lancer(&mut w);
        }
        if !ouverte {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
        }
        if ouverte || w.busy() {
            self.paquet = Some(w);
        }
    }

    fn paquet_body(&mut self, ui: &mut egui::Ui, w: &mut PaquetWindow) -> bool {
        let avert = theme::avertissement();
        let occupe = w.job.is_some();
        ui.label(
            RichText::new("Tout ce qu'il faut livrer, en un clic : une vidéo par plan, les vignettes, le PDF du storyboard, l'animatique, la liste des plans, et un manifeste (manifest.json) avec l'empreinte de chaque fichier. Les fichiers sont rangés selon ton modèle de chemin ; tout est écrit à l'écart puis rangé d'un coup : si tu arrêtes, rien ne reste.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        let ffmpeg_ok = matches!(self.ffmpeg_info(), Some((_, Ok(_))));
        let n = self.doc.panels.len();
        let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
        let projet_dossier = self.dossier_du_projet();
        let journal_possible = !self.doc.envois.is_empty();
        let r = &mut self.settings.paquet;
        if !journal_possible {
            r.journal = false;
        }
        ui.add_enabled_ui(!occupe, |ui| {
            egui::Grid::new("paquet_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
                ui.label("Contenu");
                ui.vertical(|ui| {
                    ui.checkbox(&mut r.videos, "Une vidéo par plan").on_hover_text("Un fichier vidéo par plan (H.264, ou ProRes si tu le coches ci-dessous), sans transitions, nommé comme le plan.");
                    ui.add_enabled_ui(r.videos, |ui| {
                        ui.checkbox(&mut r.prores, "   en ProRes (plus lourd, pour le montage)");
                    });
                    ui.checkbox(&mut r.vignettes, "Les vignettes par plan (PNG)");
                    ui.checkbox(&mut r.pdf, "Le PDF du storyboard");
                    ui.checkbox(&mut r.animatique, "L'animatique complète (MP4 avec le son)");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut r.liste_csv, "La liste des plans : CSV");
                        ui.checkbox(&mut r.liste_json, "JSON");
                    });
                    ui.add_enabled_ui(journal_possible, |ui| {
                        ui.checkbox(&mut r.journal, "Le journal des différences (PDF et page HTML)")
                            .on_hover_text("Ce qui a changé depuis le dernier envoi (ou entre les deux derniers s'il n'y a rien de neuf) : les plans modifiés avant / après, ajoutés, retirés, déplacés, et les notes traitées. Demande au moins un envoi.");
                    });
                    ui.label(RichText::new("Le manifeste est toujours écrit.").small().weak());
                });
                ui.end_row();

                ui.label("Plans");
                ui.vertical(|ui| {
                    ui.radio_value(&mut r.film_entier, true, format!("Tout le film ({n})"));
                    ui.radio_value(&mut r.film_entier, false, format!("La séquence courante ({})", b - a));
                });
                ui.end_row();

                ui.label("Dossier");
                ui.horizontal(|ui| {
                    match &r.dossier {
                        Some(d) => ui.label(court(d)).on_hover_text(d.display().to_string()),
                        None => ui.colored_label(avert, "à choisir"),
                    };
                    if ui.button("Choisir…").clicked() {
                        if let Some(p) = crate::dialogues::choisir_dossier("paquet", "Dossier où ranger le paquet de livraison", projet_dossier.as_deref(), r.dossier.as_deref()) {
                            r.dossier = Some(p);
                        }
                    }
                });
                ui.end_row();

                ui.label("Rangement");
                ui.vertical(|ui| {
                    let mut choisi: Option<&str> = None;
                    egui::ComboBox::from_id_salt("paquet_chemins").selected_text("Modèles prêts").width(260.0).show_ui(ui, |ui| {
                        for (nom, m) in paquet::CHEMINS_PRETS {
                            if ui.selectable_label(r.modele_chemin.trim() == *m, format!("{nom}   {m}")).clicked() {
                                choisi = Some(*m);
                            }
                        }
                    });
                    if let Some(m) = choisi {
                        r.modele_chemin = m.to_string();
                    }
                    let resp = ui.add(egui::TextEdit::singleline(&mut r.modele_chemin).desired_width(400.0).font(egui::TextStyle::Monospace).hint_text(paquet::CHEMIN_DEFAUT));
                    bulles::bulle(resp, "Modèle de chemin", "Les dossiers du paquet, avec {projet}, {episode}, {sequence} et {date} (comme le modèle de noms) et des « / ». Un morceau dont le jeton est vide est laissé de côté.", "");
                });
                ui.end_row();

                ui.label("Si un fichier existe");
                ui.checkbox(&mut r.remplacer, "Remplacer les fichiers qui existent déjà").on_hover_text("Décoché, le paquet refuse de commencer à ranger si un fichier est déjà là. Coché, l'ancien est remis en place si le paquet échoue.");
                ui.end_row();
            });
        });
        ui.separator();

        // L'aperçu du rangement.
        let r = self.settings.paquet.clone();
        let apercu = (|| -> Result<(String, String), String> {
            let cur = self.cur_panel.min(self.doc.panels.len().saturating_sub(1));
            let g = paquet::dossier_de(&self.doc, r.chemin(), None)?;
            let p = paquet::dossier_de(&self.doc, r.chemin(), Some(cur))?;
            Ok((g.display().to_string(), p.display().to_string()))
        })();
        match &apercu {
            Ok((g, p)) => {
                ui.label(RichText::new("Aperçu").strong());
                let base = paquet::base_du_film(&self.doc);
                ui.label(RichText::new(format!("{}\\{}_storyboard.pdf, …_plans.csv, manifest.json", if g.is_empty() { "." } else { g }, base)).monospace().small());
                ui.label(RichText::new(format!("{}\\{}.mp4", if p.is_empty() { "." } else { p }, self.doc.nom_de_plan(self.cur_panel.min(self.doc.panels.len().saturating_sub(1))))).monospace().small());
            }
            Err(e) => {
                ui.colored_label(avert, e);
            }
        }
        if r.vide() {
            ui.colored_label(avert, "Coche au moins un contenu.");
        }
        if r.a_besoin_de_ffmpeg() && !ffmpeg_ok {
            ui.colored_label(avert, "ffmpeg est introuvable ou inutilisable : il fabrique les vidéos et l'animatique (Préférences, ou décoche-les).");
        }
        if !self.doc.numbering_locked {
            ui.label(RichText::new("La numérotation des plans est suivie : les noms changent si un plan s'ajoute ou se déplace.").small().color(avert));
        }
        let cle = (self.history.serial(), self.doc_epoch, self.doc.properties.modele_noms.clone());
        if w.cache.as_ref().map(|c| &c.0) != Some(&cle) {
            w.cache = Some((cle, crate::nommage::verifier(&self.doc, &self.doc.properties.modele_noms)));
        }
        let v = w.cache.as_ref().map(|c| c.1.clone()).unwrap_or_default();
        for a in &v.avertissements {
            ui.label(RichText::new(a).small().color(avert));
        }
        ui.add_space(4.0);

        let mut go = false;
        match &mut w.job {
            Some(job) => {
                let (texte, part) = job.status();
                ui.add(egui::ProgressBar::new(part).text(texte));
                if bulles::icone(ui, ph::X, "Arrêter", "Arrête le paquet et efface tout ce qu'il a commencé d'écrire : rien ne reste dans le dossier choisi.", "").clicked() {
                    job.cancel();
                }
            }
            None => {
                let pret = !r.vide() && r.dossier.is_some() && apercu.is_ok() && (!r.a_besoin_de_ffmpeg() || ffmpeg_ok);
                if bulles::icone_si(ui, pret, ph::PACKAGE, "Faire le paquet", "Calcule tout en tâche de fond, puis range les fichiers selon le modèle de chemin et écrit le manifeste.", "").clicked() {
                    go = true;
                }
            }
        }
        match &w.result {
            Some(Ok(r)) => {
                ui.separator();
                ui.label(RichText::new(format!("Terminé : {} fichier(s), {:.1} Mo.", r.fichiers, r.octets as f64 / 1_048_576.0)).strong().color(theme::succes()));
                ui.label(RichText::new(court(&r.racine)).small()).on_hover_text(r.racine.display().to_string());
                if bulles::icone(ui, ph::FOLDER_OPEN, "Montrer dans l'explorateur", "Ouvre le dossier du paquet.", "").clicked() {
                    crate::exportui::reveal_in_explorer(&r.manifeste);
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(avert, e);
            }
            None => {}
        }
        go
    }
}
