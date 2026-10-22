//! Fenêtre « Planche des vignettes » (Fichier > Exporter, action `fichier.exporter_planche`) : l'étendue, le format, la
//! résolution, ce qu'on y met (numéros, notes, barres, cadres de caméra, annotations, marie-louise), puis l'export en tâche de
//! fond. Le calcul : `planche.rs`.
//!
//! Règle « sans latence » : l'interface ne calcule aucune image. L'export prend le même chemin que tous les autres
//! (`ExportJob::collect_with` : le projet est copié au départ, les images des vignettes se calculent sur d'autres fils, la planche
//! se compose et s'écrit sur un fil à part, d'abord sous un nom en `.part`) ; sa progression, son bouton « Arrêter » (qui efface ce
//! qui était commencé) et son compte rendu sont ceux de la fenêtre « Export en cours ».

use std::path::Path;
use std::sync::Arc;

use eframe::egui::{self, vec2, Align2, Color32, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::export::ExportJob;
use crate::planche::{self, Options, Resolution, Sortie};
use crate::relectureui::Etendue;
use crate::theme;

/// Le choix de résolution de la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoixResolution {
    Dpi150,
    Dpi300,
    Largeur,
}

/// Ce que la fenêtre garde d'une ouverture à l'autre (de l'onglet).
pub(crate) struct Etat {
    pub ouverte: bool,
    pub opt: Options,
    pub etendue: Etendue,
    pub choix: ChoixResolution,
    pub largeur: u32,
    /// Ce que le dernier lancement a dit (erreur de départ).
    pub message: Option<String>,
    resume: Option<(Cle, usize, [u32; 2], Option<String>)>,
}

impl Default for Etat {
    fn default() -> Self {
        Etat { ouverte: false, opt: Options::default(), etendue: Etendue::Film, choix: ChoixResolution::Dpi150, largeur: 4000, message: None, resume: None }
    }
}

type Cle = (Options, Etendue, u64, u64, (u32, usize, usize), Vec<u64>, usize);

impl Etat {
    /// La résolution que dit la fenêtre.
    pub fn resolution(&self) -> Resolution {
        match self.choix {
            ChoixResolution::Dpi150 => Resolution::Dpi(150),
            ChoixResolution::Dpi300 => Resolution::Dpi(300),
            ChoixResolution::Largeur => Resolution::Largeur(self.largeur),
        }
    }
}

/// Une image du film (couleurs prémultipliées) posée sur le papier blanc, ramenée à `cible` pixels.
pub(crate) fn rgb_sur_blanc(px: &[Color32], size: [usize; 2], cible: [u32; 2]) -> Result<image::RgbImage, String> {
    let [w, h] = size;
    if w == 0 || h == 0 || px.len() < w * h {
        return Err("image invalide".into());
    }
    let rgb: Vec<u8> = px
        .iter()
        .take(w * h)
        .flat_map(|c| {
            let blanc = 255 - c.a();
            [c.r().saturating_add(blanc), c.g().saturating_add(blanc), c.b().saturating_add(blanc)]
        })
        .collect();
    let img = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("image invalide")?;
    Ok(if img.dimensions() != (cible[0].max(1), cible[1].max(1)) { image::imageops::resize(&img, cible[0].max(1), cible[1].max(1), image::imageops::FilterType::Triangle) } else { img })
}

impl App {
    /// Fichier > Exporter > Planche des vignettes.
    pub(crate) fn open_planche(&mut self) {
        self.end_stroke();
        let e = &mut self.vignettes.planche;
        e.ouverte = true;
        e.message = None;
        if self.board_sel.len() > 1 {
            self.vignettes.planche.etendue = Etendue::Choisis;
        } else if self.vignettes.planche.etendue == Etendue::Choisis {
            self.vignettes.planche.etendue = Etendue::Film;
        }
    }

    /// La tâche d'export de la planche des plans `plans` vers `out` (rien n'est calculé ici).
    pub(crate) fn planche_job(&self, plans: &[usize], opt: &Options, out: &Path) -> Result<ExportJob, String> {
        if plans.is_empty() {
            return Err("Aucun plan dans cette étendue.".into());
        }
        let p = planche::construire(&self.doc, plans, &self.layout.board_settings(), opt, &self.doc.annotations);
        let (s, dims, reduit) = planche::dimensions(p.cadre, opt.resolution);
        let n = p.cartes.len();
        let tailles: Arc<Vec<[u32; 2]>> = Arc::new((0..n).map(|k| planche::taille_vignette(&p, k, s)).collect());
        let frames = p.images();
        let frame = self.doc.frame;
        let (opt2, titre) = (opt.clone(), self.base_name_or("Planche"));
        let (chemin, p2) = (out.to_path_buf(), Arc::new(p));
        let message = planche::message_de_fin(opt.sortie, n, dims, reduit.as_deref(), out);
        let job = ExportJob::collect_with(
            format!("Planche des vignettes ({n} vignette{})", if n > 1 { "s" } else { "" }),
            "Vignette",
            out,
            frames,
            move |k, px| rgb_sur_blanc(px, frame, tailles[k]),
            move |imgs: Vec<image::RgbImage>| {
                let img = planche::composer(&p2, &imgs, &opt2, s)?;
                drop(imgs);
                let bytes = planche::encoder(&p2, &img, opt2.sortie, &titre, s)?;
                crate::export::write_atomic(&chemin, &bytes)
            },
            || {},
        );
        Ok(job.pour_planche(message))
    }

    /// Les plans de l'étendue choisie.
    fn planche_plans(&self, e: Etendue) -> Vec<usize> {
        self.plans_de(e, None)
    }

    /// Le bouton « Exporter » : où écrire, puis la tâche de fond.
    fn planche_exporter(&mut self) {
        if !self.export_libre() {
            return;
        }
        let (opt, etendue) = {
            let e = &self.vignettes.planche;
            let mut opt = e.opt.clone();
            opt.resolution = e.resolution();
            (opt, e.etendue)
        };
        let plans = self.planche_plans(etendue);
        let ext = opt.sortie.extension();
        let base = self.base_name();
        let mut dialogue = rfd::FileDialog::new().set_title("Exporter la planche des vignettes").set_file_name(format!("{base}_planche.{ext}"));
        dialogue = match opt.sortie {
            Sortie::Png => dialogue.add_filter("Image PNG", &["png"]),
            Sortie::Jpeg => dialogue.add_filter("Image JPEG", &["jpg", "jpeg"]),
            _ => dialogue.add_filter("Document PDF", &["pdf"]),
        };
        let Some(mut path) = dialogue.save_file() else { return };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        match self.planche_job(&plans, &opt, &path) {
            Ok(job) => {
                self.vignettes.planche.message = None;
                self.start_job(job);
            }
            Err(e) => self.vignettes.planche.message = Some(e),
        }
    }

    /// À chaque image d'écran : la fenêtre.
    pub(crate) fn planche_fenetre(&mut self, ctx: &egui::Context) {
        if !self.vignettes.planche.ouverte {
            return;
        }
        let mut ouverte = true;
        let mut lancer = false;
        egui::Window::new("Planche des vignettes")
            .open(&mut ouverte)
            .resizable(false)
            .collapsible(false)
            .default_width(430.0)
            .default_pos(ctx.screen_rect().center() - vec2(215.0, 250.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| lancer = self.planche_corps(ui));
        if lancer {
            self.planche_exporter();
        }
        if !ouverte {
            self.vignettes.planche.ouverte = false;
        }
    }

    fn planche_corps(&mut self, ui: &mut egui::Ui) -> bool {
        let avert = theme::avertissement();
        ui.label(RichText::new("Toute la planche des vignettes en une grande image, ou en PDF, comme à l'écran. Les annotations n'y sont que si tu les coches.").small().color(theme::texte_doux()));
        ui.separator();
        let n_plans = self.doc.panels.len();
        let choisis = self.planche_plans(Etendue::Choisis).len();
        let (a, b) = self.doc.sequence_run(self.cur_panel.min(n_plans.saturating_sub(1)));
        let annotations = self.doc.annotations.traces.len();
        let mut e = std::mem::take(&mut self.vignettes.planche);
        egui::Grid::new("planche_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
            ui.label("Plans");
            ui.vertical(|ui| {
                ui.radio_value(&mut e.etendue, Etendue::Film, format!("Tout le film ({n_plans})"));
                ui.radio_value(&mut e.etendue, Etendue::Sequence, format!("La séquence courante ({})", b - a));
                ui.add_enabled_ui(choisis > 1, |ui| {
                    ui.radio_value(&mut e.etendue, Etendue::Choisis, format!("Les plans choisis ({choisis})"));
                });
            });
            ui.end_row();
            ui.label("Fichier");
            ui.vertical(|ui| {
                for s in Sortie::TOUTES {
                    ui.radio_value(&mut e.opt.sortie, s, s.nom());
                }
            });
            ui.end_row();
            ui.label("Résolution");
            ui.horizontal(|ui| {
                ui.radio_value(&mut e.choix, ChoixResolution::Dpi150, "150 dpi");
                ui.radio_value(&mut e.choix, ChoixResolution::Dpi300, "300 dpi");
                ui.radio_value(&mut e.choix, ChoixResolution::Largeur, "Largeur");
                ui.add_enabled(e.choix == ChoixResolution::Largeur, egui::DragValue::new(&mut e.largeur).range(200..=40_000).suffix(" px").speed(20.0));
            });
            ui.end_row();
            ui.label("Dessus");
            ui.vertical(|ui| {
                let o = &mut e.opt;
                bulles::bulle(ui.checkbox(&mut o.numeros, "Numéros de plan"), "Numéros de plan", "Le numéro du plan, au-dessus de sa première vignette.", "");
                bulles::bulle(ui.checkbox(&mut o.notes, "Notes"), "Notes", "Les bandes de note sous les images (trois lignes au plus).", "");
                bulles::bulle(ui.checkbox(&mut o.barres, "Barres de coupe"), "Barres de coupe", "Les barres entre deux vignettes : grises dans un même plan, noires à une coupe.", "");
                bulles::bulle(ui.checkbox(&mut o.cadres, "Cadres de caméra"), "Cadres de caméra", "Là où la caméra bouge : les cadres A, B, C… en bleu fin, avec leurs lettres, la flèche ou les coins qui les relient.", "");
                ui.add_enabled_ui(annotations > 0, |ui| {
                    let t = if annotations > 0 { format!("Annotations ({annotations})") } else { "Annotations (aucune)".into() };
                    bulles::bulle(ui.checkbox(&mut o.annotations, t), "Annotations", "Les traits du stylo d'annotation, par-dessus la planche. Décochées d'office : elles ne font pas partie du storyboard.", "");
                });
                ui.horizontal(|ui| {
                    bulles::bulle(ui.checkbox(&mut o.marie, "Marie-louise"), "Marie-louise", "Un passe-partout couvre tout ce qui est hors de l'intérieur des vignettes : seules les images se voient, les traits qui débordent aussi sont masqués.", "");
                    if o.marie {
                        ui.color_edit_button_srgb(&mut o.marie_couleur);
                    }
                });
            });
            ui.end_row();
        });
        ui.separator();
        // Ce qui partirait.
        let mut opt = e.opt.clone();
        opt.resolution = e.resolution();
        let cle: Cle = (opt.clone(), e.etendue, self.history.serial(), self.doc_epoch, (self.layout.card_size.to_bits(), self.layout.board_columns, self.layout.board_rows), self.board_sel.clone(), self.cur_panel);
        if e.resume.as_ref().map(|r| &r.0) != Some(&cle) {
            let plans = self.planche_plans(e.etendue);
            let p = planche::construire(&self.doc, &plans, &self.layout.board_settings(), &opt, &self.doc.annotations);
            let (_, dims, reduit) = planche::dimensions(p.cadre, opt.resolution);
            e.resume = Some((cle, p.cartes.len(), dims, reduit));
        }
        let (n, dims, reduit) = e.resume.as_ref().map_or((0, [0, 0], None), |r| (r.1, r.2, r.3.clone()));
        let pret = n > 0;
        if pret {
            ui.label(format!("{n} vignette{}, image de {} × {} pixels.", if n > 1 { "s" } else { "" }, dims[0], dims[1]));
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
            if bulles::icone_si(ui, pret && libre, ph::EXPORT, "Exporter la planche…", "Choisir le fichier, puis l'écrire en tâche de fond : le projet reste libre pendant ce temps.", &self.raccourci("fichier.exporter_planche")).clicked() {
                lancer = true;
            }
            if !libre {
                ui.label(RichText::new("Un export est déjà en cours.").small().color(avert));
            }
        });
        self.vignettes.planche = e;
        lancer
    }
}
