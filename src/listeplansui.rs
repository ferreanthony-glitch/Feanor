//! Fenêtre « Liste des plans » (Fichier > Exporter, action `fichier.exporter_liste_plans`) : une ligne par plan, en CSV ou en JSON, pour un logiciel de
//! suivi de production (profil Kitsu ou générique). Le contenu : `listeplans.rs`.
//!
//! Règle « sans latence » : le fichier est de quelques kilo-octets, il s'écrit tout de suite ; les vignettes (facultatives) se calculent dans la tâche
//! d'export habituelle (`exportstills::job_avec_fin`) : le fichier n'est écrit qu'une fois toutes les vignettes là, et « Arrêter » efface les vignettes.

use std::path::{Path, PathBuf};

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::exportstills::{self, StillsPer, StillsSettings};
use crate::listeplans::{self, Format, Profil, Separateur};
use crate::relectureui::Etendue;
use crate::theme;

pub(crate) struct ListeWindow {
    pub(crate) scope: Etendue,
    /// Dernier résultat (succès ou erreur), en une phrase.
    pub(crate) result: Option<Result<String, String>>,
    /// Ce que montre la fenêtre, recalculé seulement quand un choix ou le projet change (jamais à chaque image d'écran).
    cache: Option<(Cle, Vec<listeplans::Ligne>, crate::nommage::Verification)>,
}

type Cle = (Etendue, i64, bool, u64, u64, String, usize, Vec<u64>);

impl App {
    /// Fichier > Exporter > Liste des plans.
    pub(crate) fn open_liste_plans(&mut self) {
        self.end_stroke();
        self.liste_plans = Some(ListeWindow { scope: Etendue::Film, result: None, cache: None });
    }

    /// Les lignes de la liste pour l'étendue et les réglages de la fenêtre.
    pub(crate) fn liste_lignes(&self, scope: Etendue) -> Vec<listeplans::Ligne> {
        let r = &self.settings.liste_plans;
        let envoi = self.doc.envois.dernier().map(|e| e.id);
        let plans = self.plans_de(scope, envoi);
        let (_, etats) = self.fiche_etats(envoi);
        listeplans::construire(&self.doc, &plans, r.depart, etats.as_ref(), r.vignettes)
    }

    /// Écrit la liste dans `path` (et ses vignettes à côté si elles sont demandées). Rend ce qu'il faut dire.
    pub(crate) fn liste_ecrire(&mut self, scope: Etendue, path: &Path) -> Result<String, String> {
        let r = self.settings.liste_plans.clone();
        let lignes = self.liste_lignes(scope);
        if lignes.is_empty() {
            return Err("Aucun plan dans cette étendue.".into());
        }
        let contenu = listeplans::texte(&self.doc, &lignes, &r);
        if !r.vignettes {
            crate::export::write_atomic(path, contenu.as_bytes())?;
            return Ok(format!("Liste des plans écrite : {} ({} plan(s)).", path.display(), lignes.len()));
        }
        let dossier = path.parent().unwrap_or(Path::new(".")).join("vignettes");
        let plans: Vec<usize> = lignes.iter().map(|l| l.panel).collect();
        let items = exportstills::items(&self.doc, &plans, StillsPer::Plan);
        let cible: PathBuf = path.to_path_buf();
        let n = lignes.len();
        let job = exportstills::job_avec_fin(&dossier, &items, self.doc.frame, &StillsSettings::default(), false, move || crate::export::write_atomic(&cible, contenu.as_bytes())).avec_message(format!("Liste des plans écrite ({n} plan(s)) avec ses vignettes : {}", path.display()));
        if !self.export_libre() {
            return Err("Un export est déjà en cours : attends sa fin, ou arrête-le.".into());
        }
        self.start_job(job);
        Ok("Vignettes en cours de calcul : la liste s'écrit à la fin.".into())
    }

    pub(crate) fn liste_plans_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.liste_plans.take() else { return };
        let mut ouverte = true;
        let mut exporter = false;
        egui::Window::new("Liste des plans")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(480.0)
            .default_pos(ctx.screen_rect().center() - vec2(240.0, 230.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| exporter = self.liste_body(ui, &mut w));
        if exporter {
            w.result = None;
            let r = self.settings.liste_plans.clone();
            let base = self.base_name_or("SHOT");
            let nom = listeplans::nom_de_fichier(&self.doc, &base, r.format);
            let mut d = rfd::FileDialog::new().set_title("Enregistrer la liste des plans").set_file_name(nom);
            d = match r.format {
                Format::Csv => d.add_filter("CSV (.csv)", &["csv"]),
                Format::Json => d.add_filter("JSON (.json)", &["json"]),
            };
            if let Some(dir) = self.path.as_ref().or(self.import_source.as_ref()).and_then(|p| p.parent()).filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            if let Some(mut path) = d.save_file() {
                if path.extension().is_none() {
                    path.set_extension(r.format.extension());
                }
                let res = self.liste_ecrire(w.scope, &path);
                let (Ok(m) | Err(m)) = &res;
                self.status = m.clone();
                w.result = Some(res);
                if let Err(e) = self.settings.save() {
                    self.status = format!("Réglages non enregistrés : {e}");
                }
            }
        }
        if ouverte {
            self.liste_plans = Some(w);
        }
    }

    fn liste_body(&mut self, ui: &mut egui::Ui, w: &mut ListeWindow) -> bool {
        let avert = theme::avertissement();
        ui.label(
            RichText::new("Une ligne par plan pour le suivi de production : épisode, séquence, nom du plan (selon le modèle de noms), description, dialogue, images, image d'entrée et de sortie, durée, cadence, version, vignette. Pour importer les plans dans Kitsu ou un tableur de production.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        let n = self.doc.panels.len();
        let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
        let choisis = self.plans_de(Etendue::Choisis, None).len();
        let r = &mut self.settings.liste_plans;
        egui::Grid::new("liste_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
            ui.label("Plans");
            ui.vertical(|ui| {
                ui.radio_value(&mut w.scope, Etendue::Film, format!("Tout le film ({n})"));
                ui.radio_value(&mut w.scope, Etendue::Sequence, format!("La séquence courante ({})", b - a));
                ui.add_enabled_ui(choisis > 0, |ui| {
                    ui.radio_value(&mut w.scope, Etendue::Choisis, format!("Les plans choisis ({choisis})"));
                });
            });
            ui.end_row();

            ui.label("En-têtes");
            ui.horizontal(|ui| {
                bulles::choix(ui, &mut r.profil, Profil::Kitsu, ph::TABLE, "Kitsu", "Les en-têtes de l'import de plans de Kitsu (logiciel libre de suivi de production) : Episode, Sequence, Name, Description, Nb Frames, Frame In, Frame Out, FPS. À confirmer sur ta version de Kitsu : son aperçu d'import permet de rattacher les colonnes.", "");
                bulles::choix(ui, &mut r.profil, Profil::Generique, ph::LIST_BULLETS, "Générique", "Des en-têtes anglais clairs, avec en plus le dialogue, la durée en secondes, le statut (vide), la version, la vignette et « modifié depuis l'envoi ».", "");
            });
            ui.end_row();

            ui.label("Format");
            ui.horizontal(|ui| {
                bulles::choix(ui, &mut r.format, Format::Csv, ph::FILE_CSV, "CSV (.csv)", "Un fichier texte UTF-8 que Kitsu et tous les tableurs lisent.", "");
                bulles::choix(ui, &mut r.format, Format::Json, ph::BRACKETS_CURLY, "JSON (.json)", "Les mêmes lignes en JSON, avec la production décrite en tête, pour un script ou un outil maison.", "");
                if r.format == Format::Csv {
                    ui.add_space(8.0);
                    ui.label("Séparateur");
                    ui.radio_value(&mut r.separateur, Separateur::Virgule, "virgule").on_hover_text("Kitsu, tableurs en anglais.");
                    ui.radio_value(&mut r.separateur, Separateur::PointVirgule, "point-virgule").on_hover_text("Tableurs en français.");
                }
            });
            ui.end_row();

            ui.label("Image d'entrée");
            ui.horizontal(|ui| {
                for v in [1001, 1, 0] {
                    ui.radio_value(&mut r.depart, v, v.to_string()).on_hover_text(if v == 1001 { "L'usage des studios : le premier plan commence à l'image 1001." } else { "Numéro de la première image du premier plan." });
                }
                ui.add(egui::DragValue::new(&mut r.depart).range(0..=1_000_000).speed(1.0)).on_hover_text("Un autre numéro de départ. L'image de sortie est incluse : sortie = entrée + images − 1.");
            });
            ui.end_row();

            ui.label("Vignettes");
            ui.checkbox(&mut r.vignettes, "Une vignette par plan, dans un dossier « vignettes » à côté").on_hover_text("Une image PNG par plan, nommée comme le plan ; la colonne « thumbnail » (profil générique) porte son chemin relatif.");
            ui.end_row();
        });
        ui.separator();
        let cle: Cle = (w.scope, self.settings.liste_plans.depart, self.settings.liste_plans.vignettes, self.history.serial(), self.doc_epoch, self.doc.properties.modele_noms.clone(), self.cur_panel, self.board_sel.clone());
        if w.cache.as_ref().map(|c| &c.0) != Some(&cle) {
            let v = crate::nommage::verifier(&self.doc, &self.doc.properties.modele_noms);
            w.cache = Some((cle, self.liste_lignes(w.scope), v));
        }
        let (lignes, v) = w.cache.as_ref().map(|c| (c.1.len(), c.2.clone())).unwrap_or_default();
        ui.label(format!("{lignes} plan(s), {} colonne(s).", self.settings.liste_plans.profil.en_tetes().len()));
        if self.settings.liste_plans.profil == Profil::Kitsu && self.settings.liste_plans.vignettes {
            ui.label(RichText::new("Le profil Kitsu n'a pas de colonne pour les vignettes : elles sont écrites à côté, sans être citées.").small().color(avert));
        }
        for a in &v.avertissements {
            ui.label(RichText::new(a).small().color(avert));
        }
        if !self.doc.numbering_locked {
            ui.label(RichText::new("La numérotation des plans est suivie : les noms changent si un plan s'ajoute ou se déplace. Verrouille-la pour un suivi de production.").small().color(avert));
        }
        let mut go = false;
        ui.horizontal(|ui| {
            go = bulles::icone_si(ui, lignes > 0, ph::TABLE, "Écrire la liste…", "Choisir le fichier, puis écrire la liste (les vignettes, si tu les veux, se calculent en tâche de fond).", "").clicked();
        });
        match &w.result {
            Some(Ok(m)) => {
                ui.label(RichText::new(m).small().color(theme::succes()));
            }
            Some(Err(e)) => {
                ui.colored_label(avert, e);
            }
            None => {}
        }
        go
    }
}
