//! Fenêtre « Color script » (Fichier > Exporter, action `fichier.exporter_colorscript`) : l'étendue, une image par plan ou par case,
//! laquelle (première ou du milieu), la taille et l'écart, le fond, les titres et numéros, la bande des couleurs dominantes, PNG ou
//! PDF. Même tâche de fond que tous les exports (`ExportJob::collect_with`) : l'interface ne calcule aucune image. Le calcul :
//! `colorscript.rs`.

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::colorscript::{self, Cellule, Options, Prise, Sortie};
use crate::export::ExportJob;
use crate::plancheui::rgb_sur_blanc;
use crate::relectureui::Etendue;
use crate::theme;

/// Ce que la fenêtre garde d'une ouverture à l'autre (de l'onglet).
pub(crate) struct Etat {
    pub ouverte: bool,
    pub opt: Options,
    pub etendue: Etendue,
    pub message: Option<String>,
    resume: Option<(Cle, usize, [u32; 2], Option<String>)>,
}

impl Default for Etat {
    fn default() -> Self {
        Etat { ouverte: false, opt: Options::default(), etendue: Etendue::Film, message: None, resume: None }
    }
}

type Cle = (Options, Etendue, u64, u64, Vec<u64>, usize);

impl App {
    /// Fichier > Exporter > Color script.
    pub(crate) fn open_colorscript(&mut self) {
        self.end_stroke();
        let e = &mut self.vignettes.colorscript;
        e.ouverte = true;
        e.message = None;
        if self.board_sel.len() > 1 {
            self.vignettes.colorscript.etendue = Etendue::Choisis;
        } else if self.vignettes.colorscript.etendue == Etendue::Choisis {
            self.vignettes.colorscript.etendue = Etendue::Film;
        }
    }

    /// La tâche d'export du color script des plans `plans` vers `out` (rien n'est calculé ici).
    pub(crate) fn colorscript_job(&self, plans: &[usize], opt: &Options, out: &Path) -> Result<ExportJob, String> {
        let spec = colorscript::construire(&self.doc, plans, opt);
        let n = spec.nombre();
        if n == 0 {
            return Err("Aucun plan dans cette étendue.".into());
        }
        let frames = spec.images();
        let frame = self.doc.frame;
        let cellule = spec.cellule;
        let bande = spec.options.bande;
        let (sortie, titre) = (spec.options.sortie, self.base_name_or("Color script"));
        let mut message = format!("Color script exporté ({n} image{}, {} × {} pixels) : {}", if n > 1 { "s" } else { "" }, spec.taille[0], spec.taille[1], out.display());
        if let Some(r) = &spec.reduit {
            message += &format!(" {r}");
        }
        let (chemin, spec) = (out.to_path_buf(), Arc::new(spec));
        let job = ExportJob::collect_with(
            format!("Color script ({n} image{})", if n > 1 { "s" } else { "" }),
            "Image",
            out,
            frames,
            move |_, px| -> Result<Cellule, String> {
                let img = rgb_sur_blanc(px, frame, cellule)?;
                let palette = if bande { colorscript::dominantes(&img, 5) } else { Vec::new() };
                Ok((img, palette))
            },
            move |cellules: Vec<Cellule>| {
                let img = colorscript::composer(&spec, &cellules)?;
                drop(cellules);
                let bytes = colorscript::encoder(&img, sortie, &titre)?;
                crate::export::write_atomic(&chemin, &bytes)
            },
            || {},
        );
        Ok(job.pour_planche(message))
    }

    fn colorscript_exporter(&mut self) {
        if !self.export_libre() {
            return;
        }
        let (opt, etendue) = (self.vignettes.colorscript.opt.clone(), self.vignettes.colorscript.etendue);
        let plans = self.plans_de(etendue, None);
        let ext = opt.sortie.extension();
        let base = self.base_name();
        let dialogue = rfd::FileDialog::new().set_title("Exporter le color script").set_file_name(format!("{base}_colorscript.{ext}"));
        let dialogue = match opt.sortie {
            Sortie::Png => dialogue.add_filter("Image PNG", &["png"]),
            Sortie::Pdf => dialogue.add_filter("Document PDF", &["pdf"]),
        };
        let Some(mut path) = dialogue.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        match self.colorscript_job(&plans, &opt, &path) {
            Ok(job) => {
                self.vignettes.colorscript.message = None;
                self.start_job(job);
            }
            Err(e) => self.vignettes.colorscript.message = Some(e),
        }
    }

    /// À chaque image d'écran : la fenêtre.
    pub(crate) fn colorscript_fenetre(&mut self, ctx: &egui::Context) {
        if !self.vignettes.colorscript.ouverte {
            return;
        }
        let mut ouverte = true;
        let mut lancer = false;
        egui::Window::new("Color script")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(430.0)
            .default_pos(ctx.screen_rect().center() - vec2(215.0, 250.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| lancer = self.colorscript_corps(ui));
        if lancer {
            self.colorscript_exporter();
        }
        if !ouverte {
            self.vignettes.colorscript.ouverte = false;
        }
    }

    fn colorscript_corps(&mut self, ui: &mut egui::Ui) -> bool {
        let avert = theme::avertissement();
        ui.label(RichText::new("Une petite image par plan, rangées par séquence en rangées serrées sur un fond uni : la couleur du film d'un coup d'œil.").small().color(theme::texte_doux()));
        ui.separator();
        let n_plans = self.doc.panels.len();
        let choisis = self.plans_de(Etendue::Choisis, None).len();
        let (a, b) = self.doc.sequence_run(self.cur_panel.min(n_plans.saturating_sub(1)));
        let mut e = std::mem::take(&mut self.vignettes.colorscript);
        egui::Grid::new("colorscript_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
            ui.label("Plans");
            ui.vertical(|ui| {
                ui.radio_value(&mut e.etendue, Etendue::Film, format!("Tout le film ({n_plans})"));
                ui.radio_value(&mut e.etendue, Etendue::Sequence, format!("La séquence courante ({})", b - a));
                ui.add_enabled_ui(choisis > 1, |ui| {
                    ui.radio_value(&mut e.etendue, Etendue::Choisis, format!("Les plans choisis ({choisis})"));
                });
            });
            ui.end_row();
            ui.label("Images");
            ui.vertical(|ui| {
                ui.radio_value(&mut e.opt.par_case, false, "Une par plan");
                ui.radio_value(&mut e.opt.par_case, true, "Une par case (le plan et chacune de ses marques d'image)");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut e.opt.prise, Prise::Premiere, "La première");
                    ui.radio_value(&mut e.opt.prise, Prise::Milieu, "Celle du milieu");
                });
            });
            ui.end_row();
            ui.label("Taille");
            ui.horizontal(|ui| {
                bulles::etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Largeur d'une image", "En pixels.");
                ui.add(egui::DragValue::new(&mut e.opt.largeur).range(24..=2000).suffix(" px").speed(2.0));
                bulles::etiquette(ui, ph::ARROWS_HORIZONTAL, "Écart", "Entre deux images, en pixels.");
                ui.add(egui::DragValue::new(&mut e.opt.espace).range(0..=200).suffix(" px"));
                bulles::etiquette(ui, ph::COLUMNS, "Images par rangée", "Au plus : une séquence plus longue passe à la rangée suivante.");
                ui.add(egui::DragValue::new(&mut e.opt.par_ligne).range(1..=200));
            });
            ui.end_row();
            ui.label("Fond");
            ui.color_edit_button_srgb(&mut e.opt.fond);
            ui.end_row();
            ui.label("Dessus");
            ui.vertical(|ui| {
                bulles::bulle(ui.checkbox(&mut e.opt.titres, "Titres des séquences"), "Titres des séquences", "Le nom de chaque séquence, au-dessus de ses images.", "");
                bulles::bulle(ui.checkbox(&mut e.opt.numeros, "Numéros de plan"), "Numéros de plan", "Un petit numéro en haut à gauche de chaque image de début de plan.", "");
                bulles::bulle(
                    ui.checkbox(&mut e.opt.bande, "Bande des couleurs dominantes"),
                    "Bande des couleurs dominantes",
                    "Sous chaque image, jusqu'à cinq couleurs, chacune large comme la part de l'image qu'elle représente. Éteinte d'office.",
                    "",
                );
            });
            ui.end_row();
            ui.label("Fichier");
            ui.horizontal(|ui| {
                ui.radio_value(&mut e.opt.sortie, Sortie::Png, "Image PNG");
                ui.radio_value(&mut e.opt.sortie, Sortie::Pdf, "PDF, une grande page");
            });
            ui.end_row();
        });
        ui.separator();
        let cle: Cle = (e.opt.clone(), e.etendue, self.history.serial(), self.doc_epoch, self.board_sel.clone(), self.cur_panel);
        if e.resume.as_ref().map(|r| &r.0) != Some(&cle) {
            let plans = self.plans_de(e.etendue, None);
            let spec = colorscript::construire(&self.doc, &plans, &e.opt);
            e.resume = Some((cle, spec.nombre(), spec.taille, spec.reduit.clone()));
        }
        let (n, dims, reduit) = e.resume.as_ref().map_or((0, [0, 0], None), |r| (r.1, r.2, r.3.clone()));
        let pret = n > 0;
        if pret {
            ui.label(format!("{n} image{}, image de {} × {} pixels.", if n > 1 { "s" } else { "" }, dims[0], dims[1]));
        } else {
            ui.colored_label(avert, "Aucun plan dans cette étendue.");
        }
        if let Some(r) = reduit {
            ui.colored_label(avert, r);
        }
        if let Some(m) = &e.message {
            ui.colored_label(avert, m);
        }
        let libre = !matches!(&self.export_job, Some(j) if j.outcome.is_none());
        let mut lancer = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if bulles::icone_si(ui, pret && libre, ph::EXPORT, "Exporter le color script…", "Choisir le fichier, puis l'écrire en tâche de fond : le projet reste libre pendant ce temps.", &self.raccourci("fichier.exporter_colorscript")).clicked() {
                lancer = true;
            }
            if !libre {
                ui.label(RichText::new("Un export est déjà en cours.").small().color(avert));
            }
        });
        self.vignettes.colorscript = e;
        lancer
    }
}
