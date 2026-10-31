//! Fenêtre « Fiche technique » (Fichier > Exporter, action `fichier.exporter_fiche`) : les plans, les
//! colonnes (choisies, dans l'ordre), le format (classeur XLSX ou CSV), le time code de départ, puis
//! l'écriture en tâche de fond (`fichejob.rs`) ou la copie dans le presse-papiers. Ce que contient la
//! fiche : `fiche.rs` ; les formats : `ficheformats.rs`.
//!
//! Règle « sans latence » : l'interface ne calcule aucune vignette. Le projet est copié au départ (quelques images
//! d'écran, pendant lesquelles il ne bouge pas : `FicheWindow::rendering`, lu par `App::modal_open`), puis les
//! images se calculent sur d'autres fils (`exportrender.rs`) ; le reste (PNG, classeur, fichiers) est à côté. Le
//! projet est libre et l'export continue si l'on change d'onglet. Le presse-papiers n'est touché que sur un clic du
//! bouton « Copier ».

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::editorial;
use crate::envois::{self, Etat};
use crate::export::Progress;
use crate::fiche::{self, Colonne, Entree, Fiche};
use crate::fichejob::{FicheJob, Report};
use crate::ficheformats::{self, Format, Separateur};
use crate::relectureui::Etendue;
use crate::theme;

/// Ce que le résumé a compté (rangé avec la clé qui a servi à le calculer).
type Cle = (Etendue, Option<u64>, bool, usize, u64, u64, Vec<u64>, bool);

/// L'état de la fenêtre.
pub(crate) struct FicheWindow {
    /// Toutes les colonnes dans l'ordre de la fenêtre, avec leur case : les cochées, dans cet ordre, font la fiche.
    pub(crate) ordre: Vec<(Colonne, bool)>,
    pub(crate) format: Format,
    pub(crate) sep: Separateur,
    pub(crate) par_image: bool,
    pub(crate) start_tc: String,
    pub(crate) scope: Etendue,
    /// L'envoi de référence (colonne « Modifié depuis l'envoi » et étendue « modifiés »).
    pub(crate) envoi: Option<u64>,
    pub(crate) titre: String,
    pub(crate) job: Option<FicheJob>,
    pub(crate) result: Option<Result<Report, String>>,
    /// Ce que le dernier clic sur « Copier » a fait.
    pub(crate) copie: Option<String>,
    resume: Option<(Cle, usize, usize)>,
}

impl FicheWindow {
    /// La fiche copie encore le projet : il ne doit pas changer, ni d'onglet.
    pub(crate) fn rendering(&self) -> bool {
        self.job.as_ref().is_some_and(FicheJob::rendering)
    }

    /// Une fiche s'écrit : on ne ferme pas l'onglet (le fichier est en cours d'écriture).
    pub(crate) fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(FicheJob::running)
    }

    fn colonnes(&self) -> Vec<Colonne> {
        self.ordre.iter().filter(|(_, on)| *on).map(|(c, _)| *c).collect()
    }

    /// Les réglages à garder d'une séance à l'autre.
    fn reglages(&self, dossier: Option<&Path>, avant: &fiche::Reglages) -> fiche::Reglages {
        fiche::Reglages {
            colonnes: self.colonnes().iter().map(|c| c.id().to_string()).collect(),
            format: self.format,
            separateur: self.sep,
            start_tc: self.start_tc.trim().to_string(),
            par_image: self.par_image,
            dossier: dossier.map(Path::to_path_buf).or_else(|| avant.dossier.clone()),
        }
    }
}

/// Les colonnes d'une fenêtre neuve : celles des réglages (dans leur ordre), puis les autres, décochées.
fn ordre_depuis(ids: &[String]) -> Vec<(Colonne, bool)> {
    let mut choisies = fiche::colonnes_depuis_ids(ids);
    if choisies.is_empty() {
        choisies = fiche::colonnes_par_defaut();
    }
    let mut ordre: Vec<(Colonne, bool)> = choisies.iter().map(|c| (*c, true)).collect();
    ordre.extend(Colonne::TOUTES.iter().filter(|c| !choisies.contains(c)).map(|c| (*c, false)));
    ordre
}

/// Un chemin en court (« …\dossier\nom »), le chemin complet en bulle.
fn court(p: &Path) -> String {
    let parts: Vec<String> = p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.len() <= 3 {
        p.display().to_string()
    } else {
        format!("…\\{}", parts[parts.len() - 2..].join("\\"))
    }
}

/// Ce que la fenêtre demande à l'application après un tour de dessin.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Demande {
    Rien,
    Exporter,
    Copier,
}

impl App {
    /// Fichier > Exporter > Fiche technique.
    pub(crate) fn open_fiche(&mut self) {
        self.end_stroke();
        if self.fiche.as_ref().is_some_and(FicheWindow::busy) {
            return;
        }
        let r = self.settings.fiche.clone();
        let titre = match self.doc.properties.title.trim() {
            "" => self.base_name_or("SHOT"),
            t => t.to_string(),
        };
        let start_tc = if editorial::parse_timecode(&r.start_tc, self.doc.fps).is_some() { r.start_tc.clone() } else { "01:00:00:00".to_string() };
        self.fiche = Some(FicheWindow {
            ordre: ordre_depuis(&r.colonnes),
            format: r.format,
            sep: r.separateur,
            par_image: r.par_image,
            start_tc,
            scope: Etendue::Film,
            envoi: self.doc.envois.dernier().map(|e| e.id),
            titre,
            job: None,
            result: None,
            copie: None,
            resume: None,
        });
    }

    /// L'envoi de référence et l'état des plans à son égard (rien : le relevé n'est pas prêt).
    pub(crate) fn fiche_etats(&self, envoi: Option<u64>) -> (Option<String>, Option<HashMap<u64, Etat>>) {
        let Some(e) = envoi.and_then(|i| self.doc.envois.trouve(i)).or_else(|| self.doc.envois.dernier()) else { return (None, None) };
        if self.envois_ui.empreintes_vides() {
            return (Some(e.nom.clone()), None);
        }
        let etats = self
            .doc
            .panels
            .iter()
            .filter_map(|p| self.envois_ui.empreinte(p.id).map(|d| (p.id, envois::etat_du_plan(e, p.id, d))))
            .filter(|(_, s)| *s != Etat::Inchange)
            .collect();
        (Some(e.nom.clone()), Some(etats))
    }

    /// La fiche que la fenêtre décrit ; `vignettes` : avec la colonne des vignettes (pas pour le presse-papiers).
    pub(crate) fn fiche_construire(&self, w: &FicheWindow, vignettes: bool) -> Result<Fiche, String> {
        let colonnes: Vec<Colonne> = w.colonnes().into_iter().filter(|c| vignettes || *c != Colonne::Vignette).collect();
        let fps = self.doc.fps;
        let start = editorial::parse_timecode(&w.start_tc, fps).ok_or_else(|| format!("Time code de départ illisible : « {} » (attendu HH:MM:SS:FF, FF de 0 à {}).", w.start_tc.trim(), fps.max(1) - 1))?;
        let plans = self.plans_de(w.scope, w.envoi);
        let (envoi, etats) = self.fiche_etats(w.envoi);
        fiche::build(&self.doc, &Entree { plans: &plans, colonnes: &colonnes, par_image: w.par_image, start_tc: start, etats: etats.as_ref(), envoi: envoi.as_deref() })
    }

    fn fiche_enregistre_reglages(&mut self, w: &FicheWindow, dossier: Option<&Path>) {
        let r = w.reglages(dossier, &self.settings.fiche);
        if r != self.settings.fiche {
            self.settings.fiche = r;
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }

    /// Le bouton « Exporter » : la fiche est vérifiée, puis l'utilisateur choisit le fichier.
    fn fiche_exporter(&mut self, w: &mut FicheWindow) {
        w.result = None;
        w.copie = None;
        if let Err(e) = self.fiche_construire(w, true) {
            w.result = Some(Err(e));
            return;
        }
        let ext = w.format.extension();
        let filtre = match w.format {
            Format::Xlsx => ("Classeur (.xlsx)", ["xlsx"]),
            Format::Csv => ("CSV (.csv)", ["csv"]),
        };
        let Some(mut path) = self.dialogue_enregistrer("fiche", "Enregistrer la fiche technique", (filtre.0, &filtre.1), &format!("{}_fiche.{ext}", editorial::file_safe(&w.titre))) else { return };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        self.fiche_lancer(w, &path);
    }

    /// Écrit la fiche dans `path` en tâche de fond.
    pub(crate) fn fiche_lancer(&mut self, w: &mut FicheWindow, path: &Path) {
        w.result = None;
        w.copie = None;
        let fiche = match self.fiche_construire(w, true) {
            Ok(f) => f,
            Err(e) => {
                w.result = Some(Err(e));
                return;
            }
        };
        self.fiche_enregistre_reglages(w, path.parent());
        self.playing = false;
        match FicheJob::start(&self.doc, fiche, w.format, w.sep, path) {
            Ok(job) => w.job = Some(job.with_exact(self.rejeu_idle.clone())),
            Err(e) => w.result = Some(Err(e)),
        }
    }

    /// Le bouton « Copier » : le tableau, séparé par des tabulations, dans le presse-papiers.
    pub(crate) fn fiche_copier(&mut self, ctx: &egui::Context, w: &mut FicheWindow) {
        w.result = None;
        match self.fiche_construire(w, false) {
            Ok(f) => {
                let texte = ficheformats::presse_papiers(&f);
                ctx.copy_text(texte);
                w.copie = Some(format!("Copié : {} lignes, séparées par des tabulations. Colle dans la case A1 d'un tableur.", f.lignes.len()));
                self.status = "Fiche technique copiée dans le presse-papiers.".into();
                self.fiche_enregistre_reglages(w, None);
            }
            Err(e) => w.result = Some(Err(e)),
        }
    }

    /// À chaque image d'affichage : la fenêtre, et l'export qui avance.
    pub(crate) fn fiche_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.fiche.take() else { return };
        if let Some(job) = &mut w.job {
            job.step(&self.doc, Duration::from_millis(8));
            let copie = job.rendering();
            match job.progress() {
                Progress::Running(done, total) => {
                    if copie {
                        ctx.request_repaint();
                    } else {
                        ctx.request_repaint_after(Duration::from_millis(100));
                    }
                    self.status = format!("Fiche technique : vignette {} sur {total}", done + 1);
                }
                Progress::Finishing => ctx.request_repaint_after(Duration::from_millis(60)),
                Progress::Done => {}
            }
        }
        if w.job.as_ref().is_some_and(|j| !j.running()) {
            let job = w.job.take().expect("export");
            let resultat = job.report().cloned().unwrap_or_else(|| Err("Export annulé.".into()));
            self.status = match &resultat {
                Ok(r) => format!("Fiche technique écrite : {}", r.fichier.display()),
                Err(e) => format!("Fiche technique : {e}"),
            };
            w.result = Some(resultat);
        }
        let mut ouverte = true;
        let mut demande = Demande::Rien;
        egui::Window::new("Fiche technique")
            .open(&mut ouverte)
            .resizable(true)
            .collapsible(false)
            .default_width(560.0)
            .default_pos(ctx.screen_rect().center() - vec2(280.0, 280.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| demande = self.fiche_body(ui, &mut w));
        match demande {
            Demande::Exporter => self.fiche_exporter(&mut w),
            Demande::Copier => self.fiche_copier(ctx, &mut w),
            Demande::Rien => {}
        }
        if !ouverte {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
            self.fiche_enregistre_reglages(&w, None);
        }
        if ouverte || w.busy() {
            self.fiche = Some(w);
        }
    }

    fn fiche_body(&mut self, ui: &mut egui::Ui, w: &mut FicheWindow) -> Demande {
        let avert = theme::avertissement();
        let occupe = w.job.is_some();
        let fps = self.doc.fps;
        ui.label(
            RichText::new("Le découpage du film en tableau, un plan par ligne : durées, time codes, cadrage, mouvement de caméra, textes, sons, commentaires, vignette… pour la production, le layout et le montage.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        ui.add_enabled_ui(!occupe, |ui| {
            egui::Grid::new("fiche_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
                ui.label("Plans");
                ui.vertical(|ui| {
                    let n = self.doc.panels.len();
                    let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
                    ui.radio_value(&mut w.scope, Etendue::Film, format!("Tout le film ({n})"));
                    ui.radio_value(&mut w.scope, Etendue::Sequence, format!("La séquence courante ({})", b - a));
                    let choisis = self.plans_de(Etendue::Choisis, None).len();
                    ui.add_enabled_ui(choisis > 0, |ui| {
                        ui.radio_value(&mut w.scope, Etendue::Choisis, format!("Les plans choisis ({choisis})"));
                    });
                    let modifies = w.envoi.map(|e| self.plans_modifies_depuis(e).len());
                    ui.add_enabled_ui(modifies.is_some(), |ui| {
                        let texte = match modifies {
                            Some(m) => format!("Seulement les plans modifiés depuis l'envoi ({m})"),
                            None => "Seulement les plans modifiés depuis un envoi".to_string(),
                        };
                        ui.radio_value(&mut w.scope, Etendue::Modifies, texte);
                    });
                });
                ui.end_row();

                if !self.doc.envois.is_empty() {
                    ui.label("Envoi");
                    let nom = |id: Option<u64>| id.and_then(|i| self.doc.envois.trouve(i)).map_or("Aucun".to_string(), |e| e.libelle());
                    egui::ComboBox::from_id_salt("fiche_envoi").selected_text(nom(w.envoi)).width(280.0).show_ui(ui, |ui| {
                        let mut liste: Vec<(u64, String)> = self.doc.envois.list.iter().map(|e| (e.id, e.libelle())).collect();
                        liste.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
                        for (id, texte) in liste {
                            if ui.selectable_label(w.envoi == Some(id), texte).clicked() {
                                w.envoi = Some(id);
                            }
                        }
                    })
                    .response
                    .on_hover_text("L'envoi auquel se comparent les colonnes « Modifié depuis l'envoi » et « Dernier envoi », et l'étendue « modifiés ».");
                    ui.end_row();
                }

                ui.label("Lignes");
                ui.vertical(|ui| {
                    ui.radio_value(&mut w.par_image, false, "Une ligne par plan");
                    ui.radio_value(&mut w.par_image, true, "Une ligne par image de storyboard (le plan et ses marques d'image)");
                });
                ui.end_row();

                ui.label("Time code de départ");
                ui.horizontal(|ui| {
                    let valide = editorial::parse_timecode(&w.start_tc, fps).is_some();
                    let edit = egui::TextEdit::singleline(&mut w.start_tc).desired_width(100.0).hint_text("01:00:00:00");
                    let edit = if valide { edit } else { edit.text_color(avert) };
                    ui.add(edit).on_hover_text(format!("Le time code de la première image du film : HH:MM:SS:FF, FF de 0 à {} (sans saut d'images). Un plan garde son time code même si l'on n'écrit que quelques plans.", fps.max(1) - 1));
                });
                ui.end_row();

                ui.label("Format");
                ui.horizontal(|ui| {
                    bulles::choix(ui, &mut w.format, Format::Xlsx, ph::FILE_XLS, "Classeur (.xlsx)", "Un classeur avec un en-tête figé, des colonnes à la bonne largeur et les vignettes dans les cellules. S'ouvre dans n'importe quel tableur.", "");
                    bulles::choix(ui, &mut w.format, Format::Csv, ph::FILE_CSV, "Texte CSV (.csv)", "Un fichier texte (UTF-8) que tous les tableurs lisent. Les vignettes sont des chemins vers des images.", "");
                    if w.format == Format::Csv {
                        ui.add_space(8.0);
                        ui.label("Séparateur");
                        ui.radio_value(&mut w.sep, Separateur::PointVirgule, "point-virgule").on_hover_text("Pour un tableur en français.");
                        ui.radio_value(&mut w.sep, Separateur::Virgule, "virgule").on_hover_text("Pour un tableur en anglais.");
                    }
                });
                ui.end_row();
            });
            ui.add_space(4.0);
            self.fiche_colonnes(ui, w);
        });
        ui.separator();

        // Ce qui partirait.
        let cle: Cle = (w.scope, w.envoi, w.par_image, self.cur_panel, self.history.serial(), self.doc_epoch, self.board_sel.clone(), w.colonnes().contains(&Colonne::Vignette));
        if w.resume.as_ref().map(|r| &r.0) != Some(&cle) {
            let plans = self.plans_de(w.scope, w.envoi);
            let lignes = if w.par_image { plans.iter().map(|&i| crate::board::plan_boards(&self.doc, i).len()).sum() } else { plans.len() };
            w.resume = Some((cle.clone(), plans.len(), lignes));
        }
        let (plans, lignes) = w.resume.as_ref().map_or((0, 0), |r| (r.1, r.2));
        if plans == 0 {
            let pourquoi = if w.scope == Etendue::Modifies && self.envois_ui.empreintes_vides() { "Le relevé des plans est en cours : réessaie dans un instant." } else { "Aucun plan dans cette étendue." };
            ui.colored_label(avert, pourquoi);
        } else {
            let vignettes = if cle.7 { format!(", {lignes} vignette(s) à dessiner") } else { String::new() };
            ui.label(format!("{plans} plan(s), {lignes} ligne(s), {} colonne(s){vignettes}.", w.colonnes().len()));
        }
        if w.colonnes().is_empty() {
            ui.colored_label(avert, "Coche au moins une colonne.");
        }
        if editorial::parse_timecode(&w.start_tc, fps).is_none() {
            ui.colored_label(avert, "Le time code de départ est illisible (HH:MM:SS:FF).");
        }
        if !self.doc.numbering_locked {
            ui.label(RichText::new("La numérotation des plans est suivie : les numéros et les références de la fiche changent si un plan s'ajoute ou se déplace. Réglages du projet > Numérotation des plans > Verrouillée les fige.").small().color(avert));
        }
        if w.colonnes().contains(&Colonne::Modifie) && !self.doc.envois.is_empty() && self.envois_ui.empreintes_vides() {
            ui.label(RichText::new("Le relevé des plans depuis l'envoi n'est pas prêt : la colonne « Modifié depuis l'envoi » serait vide.").small().color(avert));
        }
        ui.add_space(4.0);

        let mut demande = Demande::Rien;
        let pret = plans > 0 && !w.colonnes().is_empty() && editorial::parse_timecode(&w.start_tc, fps).is_some();
        match &mut w.job {
            Some(job) => {
                match job.progress() {
                    Progress::Running(fait, total) => {
                        ui.add(egui::ProgressBar::new(fait as f32 / total.max(1) as f32).text(format!("vignette {fait} / {total}")));
                        if let Some(reste) = job.remaining() {
                            let s = reste.as_secs();
                            ui.label(RichText::new(format!("reste environ {}:{:02}", s / 60, s % 60)).small().weak());
                        }
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Écriture de la fiche…");
                        });
                    }
                }
                if bulles::icone(ui, ph::X, "Arrêter", "Arrête l'export et efface ce qu'il a commencé d'écrire. La fiche d'avant, s'il y en avait une, reste.", "").clicked() {
                    job.cancel();
                }
            }
            None => {
                ui.horizontal(|ui| {
                    if bulles::icone_si(ui, pret, ph::TABLE, "Exporter la fiche…", "Choisir le fichier, puis l'écrire en tâche de fond. Les vignettes vont dans un dossier « vignettes » à côté.", "").clicked() {
                        demande = Demande::Exporter;
                    }
                    if bulles::icone_si(ui, pret, ph::CLIPBOARD_TEXT, "Copier dans le presse-papiers", "Le tableau en texte séparé par des tabulations, à coller dans un tableur (sans la colonne des vignettes). Rien n'est copié avant ce clic.", "").clicked() {
                        demande = Demande::Copier;
                    }
                });
            }
        }
        if let Some(c) = &w.copie {
            ui.label(RichText::new(c).small().color(theme::succes()));
        }
        match &w.result {
            Some(Ok(r)) => {
                ui.separator();
                ui.label(RichText::new(format!("Terminé : {} ligne(s){}.", r.lignes, if r.vignettes > 0 { format!(", {} vignette(s)", r.vignettes) } else { String::new() })).strong().color(theme::succes()));
                let ko = r.octets as f64 / 1024.0;
                ui.label(RichText::new(format!("{} ({ko:.0} Ko)", court(&r.fichier))).small()).on_hover_text(r.fichier.display().to_string());
                if let Some(d) = &r.dossier_vignettes {
                    ui.label(RichText::new(format!("Vignettes : {}", court(d))).small()).on_hover_text(d.display().to_string());
                }
                for a in &r.avertissements {
                    ui.colored_label(avert, a);
                }
                if bulles::icone(ui, ph::FOLDER_OPEN, "Montrer dans l'explorateur", "Ouvre le dossier de la fiche.", "").clicked() {
                    crate::exportui::reveal_in_explorer(&r.fichier);
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(avert, e);
            }
            None => {}
        }
        demande
    }

    /// La liste des colonnes : cocher, ordonner.
    fn fiche_colonnes(&mut self, ui: &mut egui::Ui, w: &mut FicheWindow) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Colonnes").strong());
            ui.label(RichText::new("(dans cet ordre)").small().color(theme::texte_doux()));
            ui.add_space(8.0);
            if bulles::icone(ui, ph::CHECKS, "Tout cocher", "Toutes les colonnes.", "").clicked() {
                w.ordre.iter_mut().for_each(|c| c.1 = true);
            }
            if bulles::icone(ui, ph::SQUARE, "Tout décocher", "Aucune colonne : à toi de choisir.", "").clicked() {
                w.ordre.iter_mut().for_each(|c| c.1 = false);
            }
            if bulles::icone(ui, ph::ARROW_COUNTER_CLOCKWISE, "Colonnes d'origine", "Les colonnes et leur ordre proposés d'office.", "").clicked() {
                w.ordre = ordre_depuis(&[]);
            }
        });
        let mut deplacer: Option<(usize, i32)> = None;
        egui::ScrollArea::vertical().max_height(250.0).auto_shrink([false, true]).show(ui, |ui| {
            for i in 0..w.ordre.len() {
                let (c, mut on) = w.ordre[i];
                ui.horizontal(|ui| {
                    let r = ui.checkbox(&mut on, c.titre());
                    bulles::bulle(r, c.titre(), c.aide(), "");
                    if on {
                        let premier = w.ordre.iter().position(|x| x.1) == Some(i);
                        let dernier = w.ordre.iter().rposition(|x| x.1) == Some(i);
                        ui.add_space(6.0);
                        if bulles::icone_si(ui, !premier, ph::ARROW_UP, "Monter", "Avant la colonne cochée précédente.", "").clicked() {
                            deplacer = Some((i, -1));
                        }
                        if bulles::icone_si(ui, !dernier, ph::ARROW_DOWN, "Descendre", "Après la colonne cochée suivante.", "").clicked() {
                            deplacer = Some((i, 1));
                        }
                    }
                });
                w.ordre[i].1 = on;
            }
        });
        if let Some((i, sens)) = deplacer {
            // On saute les colonnes décochées : la colonne prend la place de la cochée voisine.
            let voisine = if sens < 0 { (0..i).rev().find(|&j| w.ordre[j].1) } else { (i + 1..w.ordre.len()).find(|&j| w.ordre[j].1) };
            if let Some(j) = voisine {
                let c = w.ordre.remove(i);
                w.ordre.insert(j, c);
            }
        }
    }
}
