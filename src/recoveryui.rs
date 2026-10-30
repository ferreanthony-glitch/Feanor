//! Copie de secours dans l'application (4a) : copie régulière, fenêtre de récupération au
//! démarrage, réglage du délai, indication dans la barre d'état.

use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::app::App;
use crate::recovery::{now_secs, Orphan, Snapshot};

/// Ce que la proposition de récupération a demandé : une copie à ouvrir, toutes, une à supprimer, « Plus tard ».
#[derive(Default)]
pub(crate) struct RecoveryChoix {
    pub open: Option<usize>,
    pub open_all: bool,
    pub delete: Option<usize>,
    pub later: bool,
}

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
    pub(crate) fn autosave_interval(&self) -> Option<Duration> {
        (self.settings.autosave_minutes > 0).then(|| Duration::from_secs(60 * self.settings.autosave_minutes as u64))
    }

    /// À chaque image : copie de secours si le délai est passé et que le projet a changé.
    pub(crate) fn tick_recovery(&mut self, ctx: &egui::Context) {
        let interval = self.autosave_interval();
        let title = self.base_name_or("Sans titre");
        // Pas au milieu d'un geste : le document n'est pas dans un état stable.
        // (Ni pendant un enregistrement : la copie recopierait des dessins que le dossier du projet n'a
        // pas encore, `enregistrement.rs`.)
        let hold = self.stroke.is_some() || self.degrade_en_retouche() || ctx.input(|i| i.pointer.any_down()) || self.export_job.is_some() || self.save_en_cours();
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
        // (Une copie demandée tout de suite attend la fin de celle qui s'écrit : on revient vite pour la lancer.)
        if rec.busy() || rec.en_attente() {
            ctx.request_repaint_after(Duration::from_millis(200));
        } else if let (Some(i), true) = (interval, self.modified) {
            ctx.request_repaint_after(i.min(Duration::from_secs(10)));
        }
    }

    /// La carte graphique de l'affichage est perdue (`plantage::note_device_lost`) : SHOT tourne encore, mais son
    /// affichage peut fermer SHOT à tout moment. Une copie de secours de chaque projet modifié part tout de suite,
    /// sans attendre le délai, et la barre d'état dit quoi faire.
    pub(crate) fn secours_carte_perdue(&mut self) {
        if self.modified {
            let title = self.base_name_or("Sans titre");
            if let Some(rec) = &mut self.recovery {
                let snap = Snapshot {
                    doc: &self.doc,
                    serial: self.history.serial(),
                    modified: true,
                    title,
                    project: self.path.clone(),
                    project_written: &self.written,
                };
                // (Une copie s'écrit déjà : celle-ci part dès sa fin, `Recovery::tick`.)
                rec.start_copy(snap);
            }
        }
        // Les onglets cachés : leur copie part aussi (délai nul), ou dès la fin de celle qui s'écrit.
        for i in 0..self.onglets.len() {
            if i != self.actif {
                if let Some(rec) = self.onglets[i].etat.as_mut().and_then(|e| e.recovery.as_mut()) {
                    rec.demander();
                }
                self.tick_secours_cache(i, Some(Duration::ZERO));
            }
        }
        self.status = "La carte graphique ne répond plus (pilote réinitialisé ?) : SHOT risque de se fermer. Une copie de secours \
                       part maintenant ; enregistre, puis relance SHOT."
            .into();
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
        // (Sans projet ouvert, la proposition est dans la fenêtre d'accueil, en haut du cadre « Projets récents » : `accueil.rs`.)
        if self.accueil_visible() {
            return;
        }
        let mut choix = RecoveryChoix::default();
        egui::Modal::new(egui::Id::new("recuperation")).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.heading("SHOT s'est fermé sans enregistrer");
            choix = self.recovery_contenu(ui);
        });
        self.recovery_appliquer(choix);
    }

    /// Les copies de secours trouvées, chacune avec « Ouvrir la copie » et « Supprimer », puis « Tout ouvrir » et « Plus tard ».
    /// Le même contenu dans la fenêtre de récupération et dans la fenêtre d'accueil (rien n'est fait ici : le choix est rendu).
    pub(crate) fn recovery_contenu(&self, ui: &mut egui::Ui) -> RecoveryChoix {
        let mut c = RecoveryChoix::default();
        ui.label(if self.orphans.len() > 1 { "Des copies de secours ont été trouvées, une par projet :" } else { "Une copie de secours a été trouvée :" });
        ui.add_space(6.0);
        for (i, o) in self.orphans.iter().enumerate() {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(&o.info.title).strong());
                let when = o.info.copied.map_or("date inconnue".to_string(), |c| format!("copie faite {}", ago(c)));
                let place = o.info.project.as_ref().map_or("projet jamais enregistré".to_string(), |p| p.display().to_string());
                ui.add(egui::Label::new(RichText::new(format!("{when} · {place}")).small()).truncate()).on_hover_text(format!("{when} · {place}"));
                if o.project_is_newer() {
                    ui.colored_label(warn(), "Le projet a été enregistré après cette copie : elle n'apporte sans doute rien.");
                }
                ui.horizontal(|ui| {
                    if ui.button("Ouvrir la copie").clicked() {
                        c.open = Some(i);
                    }
                    if ui.button("Supprimer").clicked() {
                        c.delete = Some(i);
                    }
                });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if self.orphans.len() > 1 && ui.button("Tout ouvrir").on_hover_text("Chaque copie dans son onglet.").clicked() {
                c.open_all = true;
            }
            c.later = ui.button("Plus tard").on_hover_text("Les copies restent ; SHOT les proposera au prochain démarrage.").clicked();
        });
        c
    }

    /// Ce que `recovery_contenu` a demandé.
    pub(crate) fn recovery_appliquer(&mut self, choix: RecoveryChoix) {
        let RecoveryChoix { open, open_all, delete, later } = choix;
        if open_all {
            // Chaque copie dans son onglet.
            for o in std::mem::take(&mut self.orphans) {
                self.open_orphan(o);
            }
        } else if let Some(i) = open {
            // (Les autres copies restent proposées : chacune s'ouvre dans son onglet.)
            let o = self.orphans.remove(i);
            self.open_orphan(o);
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
                if !self.charger_projet(loaded.doc) {
                    self.status = "La copie de secours n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
                    return;
                }
                // « Enregistrer » écrira dans le projet d'origine, s'il existe encore (sous son nom d'aujourd'hui : un
                // `.pelure` converti depuis est son `.shot` ; un `.pelure` se convertit à l'enregistrement).
                self.path = o.info.project.as_deref().map(crate::project::manifeste_a_ouvrir).filter(|p| p.parent().is_some_and(|d| d.exists()));
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
