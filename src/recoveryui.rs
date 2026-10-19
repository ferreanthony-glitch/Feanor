//! Copie de secours dans l'application (4a) : copie régulière, fenêtre de récupération au
//! démarrage, réglage du délai, indication dans la barre d'état.

use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::app::App;
use crate::recovery::{now_secs, Orphan, Snapshot};

fn warn() -> Color32 {
    crate::theme::avertissement()
}

/// « il y a 3 min », « il y a 2 h », « il y a 4 jours ».
pub fn ago(secs: u64) -> String {
    let d = now_secs().saturating_sub(secs);
    match d {
        0..=59 => "à l'instant".into(),
        60..=3599 => format!("il y a {} min", d / 60),
        3600..=86_399 => format!("il y a {} h", d / 3600),
        _ => {
            let days = d / 86_400;
            format!("il y a {days} jour{}", if days > 1 { "s" } else { "" })
        }
    }
}

impl App {
    /// Délai entre deux copies (aucun : copie désactivée).
    fn autosave_interval(&self) -> Option<Duration> {
        (self.settings.autosave_minutes > 0).then(|| Duration::from_secs(60 * self.settings.autosave_minutes as u64))
    }

    /// À chaque image : copie de secours si le délai est passé et que le projet a changé.
    pub(crate) fn tick_recovery(&mut self, ctx: &egui::Context) {
        let interval = self.autosave_interval();
        let title = self.base_name_or("Sans titre");
        // Pas au milieu d'un geste : le document n'est pas dans un état stable.
        // (Ni pendant un enregistrement : la copie recopierait des dessins que le dossier du projet n'a
        // pas encore, `enregistrement.rs`.)
        let hold = self.stroke.is_some() || ctx.input(|i| i.pointer.any_down()) || self.export_job.is_some() || self.save_en_cours();
        let Some(rec) = &mut self.recovery else { return };
        let snap = Snapshot {
            doc: &self.doc,
            serial: self.history.serial(),
            modified: self.modified,
            title,
            project: self.path.clone(),
            project_written: &self.written,
        };
        rec.tick(snap, interval, hold);
        if let Some(e) = rec.error.take() {
            self.status = format!("Copie de secours impossible : {e}");
        }
        // Rien ne bouge à l'écran : on revient quand même pour la prochaine copie.
        if rec.busy() {
            ctx.request_repaint_after(Duration::from_millis(200));
        } else if let (Some(i), true) = (interval, self.modified) {
            ctx.request_repaint_after(i.min(Duration::from_secs(10)));
        }
    }

    /// Réglage du délai (menu Fichier).
    pub(crate) fn autosave_menu_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Copie de secours toutes les");
            let r = ui.add(egui::DragValue::new(&mut self.settings.autosave_minutes).range(0..=30).suffix(" min"));
            if r.changed() {
                let _ = self.settings.save();
            }
        })
        .response
        .on_hover_text(
            "SHOT copie le projet à part (dans %LOCALAPPDATA%\\SHOT\\secours), sans toucher au projet \
             lui-même : seul « Enregistrer » l'écrit. Après un plantage, SHOT propose d'ouvrir la copie. \
             0 = jamais.",
        );
    }

    /// Dernière copie, dans la barre d'état.
    pub(crate) fn recovery_status_ui(&self, ui: &mut egui::Ui) {
        let Some(copied) = self.recovery.as_ref().and_then(|r| r.info.copied) else { return };
        ui.separator();
        ui.label(RichText::new(format!("Secours {}", ago(copied))).small().weak())
            .on_hover_text("Copie de secours du projet. Le projet lui-même n'est écrit que par « Enregistrer ».");
    }

    /// Fenêtre « SHOT s'est fermé sans enregistrer », au démarrage.
    pub(crate) fn recovery_window(&mut self, ctx: &egui::Context) {
        if self.orphans.is_empty() {
            return;
        }
        let mut open = None;
        let mut delete = None;
        let mut later = false;
        egui::Modal::new(egui::Id::new("recuperation")).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.heading("SHOT s'est fermé sans enregistrer");
            ui.label("Une copie de secours a été trouvée :");
            ui.add_space(6.0);
            for (i, o) in self.orphans.iter().enumerate() {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(&o.info.title).strong());
                    let when = o.info.copied.map_or("date inconnue".to_string(), |c| format!("copie faite {}", ago(c)));
                    let place = o
                        .info
                        .project
                        .as_ref()
                        .map_or("projet jamais enregistré".to_string(), |p| p.display().to_string());
                    ui.label(RichText::new(format!("{when} · {place}")).small());
                    if o.project_is_newer() {
                        ui.colored_label(warn(),"Le projet a été enregistré après cette copie : elle n'apporte sans doute rien.");
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Ouvrir la copie").clicked() {
                            open = Some(i);
                        }
                        if ui.button("Supprimer").clicked() {
                            delete = Some(i);
                        }
                    });
                });
            }
            ui.add_space(6.0);
            later = ui
                .button("Plus tard")
                .on_hover_text("Les copies restent ; SHOT les proposera au prochain démarrage.")
                .clicked();
        });
        if let Some(i) = open {
            let o = self.orphans.remove(i);
            self.open_orphan(o);
            // Une seule copie ouverte à la fois : les autres attendront le prochain démarrage.
            self.orphans.clear();
        } else if let Some(i) = delete {
            let o = self.orphans.remove(i);
            let _ = std::fs::remove_dir_all(&o.dir);
        } else if later {
            self.orphans.clear();
        }
    }

    pub(crate) fn open_orphan(&mut self, o: Orphan) {
        match crate::project::load(&o.manifest) {
            Ok(loaded) => {
                self.load_doc(loaded.doc);
                // « Enregistrer » écrira dans le projet d'origine, s'il existe encore.
                self.path = o.info.project.clone().filter(|p| p.parent().is_some_and(|d| d.exists()));
                self.modified = true;
                let _ = std::fs::remove_dir_all(&o.dir);
                self.status = format!(
                    "Copie de secours de « {} » ouverte. Enregistre pour la garder{}.",
                    o.info.title,
                    if self.path.is_some() { " dans le projet" } else { " (« Enregistrer sous »)" }
                );
            }
            Err(e) => self.status = format!("Copie de secours illisible : {e}"),
        }
    }
}
