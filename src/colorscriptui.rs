//! Fenêtre « Color script » (Fichier > Exporter, action `fichier.exporter_colorscript`) : l'étendue, la présentation (planche, bande continue,
//! bande de couleurs seule), l'image de chaque plan, la sortie (PNG, un PNG par séquence, PDF d'impression), les repères, les données (CSV ou
//! JSON) ; les réglages rares (tailles, fond, bande des couleurs dominantes, version du PDF) sont repliés. Même tâche de fond que tous les
//! exports (`ExportJob::collect_with`) : l'interface ne calcule aucune image. Le calcul : `colorscript.rs`.

use std::path::Path;

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::colorscript::{self, CouleurPlan, Donnees, Meta, Options, Papier, Presentation, Prise, Sortie, Spec};
use crate::export::ExportJob;
use crate::relectureui::Etendue;
use crate::theme;

/// Ce que la fenêtre garde d'une ouverture à l'autre (de l'onglet).
pub(crate) struct Etat {
    pub ouverte: bool,
    pub opt: Options,
    pub etendue: Etendue,
    pub message: Option<String>,
    resume: Option<(Cle, Resume)>,
}

/// Ce que la fenêtre dit du résultat : images, fichiers, taille de la première image, réduction éventuelle.
#[derive(Clone, Debug)]
struct Resume {
    images: usize,
    fichiers: usize,
    taille: [u32; 2],
    reduit: Option<String>,
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

    /// La tâche d'export du color script des plans `plans` vers `out` (rien n'est calculé ici). `out` : le fichier de l'image d'ensemble ou du PDF ;
    /// pour un PNG par séquence, le dossier. Les données sont écrites à côté (`colorscript::chemins`).
    pub(crate) fn colorscript_job(&self, plans: &[usize], opt: &Options, out: &Path) -> Result<ExportJob, String> {
        let specs = colorscript::specs(&self.doc, plans, opt);
        let n: usize = specs.iter().map(Spec::nombre).sum();
        if n == 0 {
            return Err("Aucun plan dans cette étendue.".into());
        }
        let (cibles, donnees) = colorscript::chemins(&self.doc, plans, opt, out);
        let pluriel = |k: usize| if k > 1 { "s" } else { "" };
        let mut message = if opt.sortie == Sortie::PngParSequence {
            format!("Color script exporté ({n} image{}, {} fichier{} PNG) : {}", pluriel(n), cibles.len(), pluriel(cibles.len()), out.display())
        } else {
            format!("Color script exporté ({n} image{}, {} × {} pixels) : {}", pluriel(n), specs[0].taille[0], specs[0].taille[1], out.display())
        };
        if let Some(d) = &donnees {
            message += &format!(" Couleurs écrites dans {}.", d.display());
        }
        for r in specs.iter().filter_map(|s| s.reduit.as_ref()).take(1) {
            message += &format!(" {r}");
        }
        let meta = Meta::pour(&self.doc, opt);
        colorscript::job(self.doc.frame, specs, meta, cibles, donnees, message)
    }

    fn colorscript_exporter(&mut self) {
        if !self.export_libre() {
            return;
        }
        let (opt, etendue) = (self.vignettes.colorscript.opt.clone(), self.vignettes.colorscript.etendue);
        let plans = self.plans_de(etendue, None);
        let ext = opt.sortie.extension();
        let chemin = if opt.sortie == Sortie::PngParSequence {
            self.dialogue_dossier("colorscript", "Dossier des images du color script (celles du même nom sont remplacées)", None)
        } else {
            let (nom_filtre, exts): (&str, &[&str]) = match opt.sortie {
                Sortie::Pdf => ("Document PDF", &["pdf"]),
                _ => ("Image PNG", &["png"]),
            };
            let nom = colorscript::noms_de_fichiers(&self.doc, &plans, &opt).into_iter().next().unwrap_or_else(|| "color_script".into());
            self.dialogue_enregistrer("colorscript", "Exporter le color script", (nom_filtre, exts), &format!("{nom}.{ext}")).map(|mut p| {
                if p.extension().is_none() {
                    p.set_extension(ext);
                }
                p
            })
        };
        let Some(path) = chemin else { return };
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
            .default_width(460.0)
            .default_pos(ctx.screen_rect().center() - vec2(230.0, 270.0))
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
        ui.label(RichText::new("La couleur du film d'un coup d'œil : une planche d'images par séquence, une bande continue, ou juste les couleurs de chaque plan.").small().color(theme::texte_doux()));
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
            ui.label("Présentation");
            ui.vertical(|ui| {
                bulles::bulle(ui.radio_value(&mut e.opt.presentation, Presentation::Planche, "Planche"), "Planche", "Une petite image par plan, rangées par séquence sur un fond uni.", "");
                bulles::bulle(
                    ui.radio_value(&mut e.opt.presentation, Presentation::Bande, "Bande continue"),
                    "Bande continue",
                    "Une colonne par plan sur une seule ligne, large comme la durée du plan (ou toutes égales). L'image est écrasée dans sa colonne.",
                    "",
                );
                bulles::bulle(
                    ui.radio_value(&mut e.opt.presentation, Presentation::Couleurs, "Bande de couleurs seule"),
                    "Bande de couleurs seule",
                    "Sans vignette : une couleur par plan. La progression des couleurs du film d'un coup d'œil.",
                    "",
                );
            });
            ui.end_row();
            ui.label("Image d'un plan");
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    bulles::bulle(ui.radio_value(&mut e.opt.prise, Prise::Premiere, "La première"), "La première", "La première image du plan.", "");
                    bulles::bulle(ui.radio_value(&mut e.opt.prise, Prise::Milieu, "Celle du milieu"), "Celle du milieu", "L'image du milieu du plan.", "");
                    bulles::bulle(ui.radio_value(&mut e.opt.prise, Prise::Moyenne, "La moyenne"), "La moyenne", "La moyenne de cinq images au plus, réparties sur la durée du plan. Plus lent.", "");
                    bulles::bulle(
                        ui.radio_value(&mut e.opt.prise, Prise::PremiereCle, "La première clé"),
                        "La première clé",
                        "La première image où un dessin commence sur un calque visible : un plan qui commence vide prend l'image où son premier dessin arrive.",
                        "",
                    );
                });
                ui.horizontal(|ui| {
                    ui.radio_value(&mut e.opt.par_case, false, "Une par plan");
                    ui.radio_value(&mut e.opt.par_case, true, "Une par case");
                });
            });
            ui.end_row();
            if e.opt.presentation != Presentation::Planche {
                ui.label("Colonnes");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut e.opt.proportionnelle, true, "Selon la durée");
                    ui.radio_value(&mut e.opt.proportionnelle, false, "Toutes égales");
                });
                ui.end_row();
            }
            if e.opt.presentation == Presentation::Couleurs {
                ui.label("Couleur d'un plan");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut e.opt.couleur, CouleurPlan::Dominante, "La dominante");
                    ui.radio_value(&mut e.opt.couleur, CouleurPlan::Moyenne, "La moyenne");
                });
                ui.end_row();
            }
            ui.label("Repères");
            ui.horizontal_wrapped(|ui| {
                bulles::bulle(ui.checkbox(&mut e.opt.titres, "Titres des séquences"), "Titres des séquences", "Le nom de chaque séquence, au-dessus de ses images.", "");
                bulles::bulle(ui.checkbox(&mut e.opt.numeros, "Numéro de plan"), "Numéros de plan", "Un petit numéro en haut à gauche de chaque image de début de plan (sauf là où il n'y a pas la place).", "");
                bulles::bulle(ui.checkbox(&mut e.opt.minutage, "Minutage"), "Minutage", "Le temps d'entrée de chaque plan dans le film (minutes:secondes).", "");
            });
            ui.end_row();
            ui.label("Fichier");
            ui.vertical(|ui| {
                ui.radio_value(&mut e.opt.sortie, Sortie::Png, "Un PNG d'ensemble");
                ui.radio_value(&mut e.opt.sortie, Sortie::PngParSequence, "Un PNG par séquence");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut e.opt.sortie, Sortie::Pdf, "Un PDF à imprimer");
                    ui.add_enabled_ui(e.opt.sortie == Sortie::Pdf, |ui| {
                        egui::ComboBox::from_id_salt("colorscript_papier").selected_text(e.opt.papier.nom()).width(110.0).show_ui(ui, |ui| {
                            for p in [Papier::A3, Papier::A4] {
                                ui.selectable_value(&mut e.opt.papier, p, p.nom());
                            }
                        });
                    });
                });
            });
            ui.end_row();
            ui.label("Couleurs");
            ui.horizontal(|ui| {
                bulles::bulle(
                    ui.radio_value(&mut e.opt.donnees, Donnees::Aucune, "Pas de fichier"),
                    "Pas de fichier de couleurs",
                    "Seule l'image est écrite.",
                    "",
                );
                bulles::bulle(
                    ui.radio_value(&mut e.opt.donnees, Donnees::Csv, "CSV"),
                    "Couleurs en CSV",
                    "À côté de l'image : pour chaque image, la séquence, le plan, son entrée en secondes, sa durée, sa couleur moyenne et ses cinq couleurs dominantes (hex et part).",
                    "",
                );
                bulles::bulle(ui.radio_value(&mut e.opt.donnees, Donnees::Json, "JSON"), "Couleurs en JSON", "La même chose en JSON, à côté de l'image.", "");
            });
            ui.end_row();
        });
        egui::CollapsingHeader::new("Réglages").id_salt("colorscript_reglages").show(ui, |ui| {
            egui::Grid::new("colorscript_reglages_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
                ui.label("Taille");
                ui.horizontal(|ui| {
                    if e.opt.presentation == Presentation::Planche {
                        bulles::etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Largeur d'une image", "En pixels.");
                        ui.add(egui::DragValue::new(&mut e.opt.largeur).range(24..=2000).suffix(" px").speed(2.0));
                    } else {
                        bulles::etiquette(ui, ph::ARROWS_OUT_SIMPLE, "Largeur de la bande", "En pixels, tous les plans ensemble.");
                        ui.add(egui::DragValue::new(&mut e.opt.bande_largeur).range(100..=15_900).suffix(" px").speed(10.0));
                        bulles::etiquette(ui, ph::ARROWS_VERTICAL, "Hauteur de la bande", "En pixels.");
                        ui.add(egui::DragValue::new(&mut e.opt.bande_hauteur).range(8..=4000).suffix(" px").speed(2.0));
                    }
                    bulles::etiquette(ui, ph::ARROWS_HORIZONTAL, "Écart", "Entre deux images, ou entre deux séquences d'une bande, en pixels.");
                    ui.add(egui::DragValue::new(&mut e.opt.espace).range(0..=200).suffix(" px"));
                    if e.opt.presentation == Presentation::Planche {
                        bulles::etiquette(ui, ph::COLUMNS, "Images par rangée", "Au plus : une séquence plus longue passe à la rangée suivante.");
                        ui.add(egui::DragValue::new(&mut e.opt.par_ligne).range(1..=200));
                    }
                });
                ui.end_row();
                ui.label("Fond");
                crate::champcouleur::srgb(ui, &mut e.opt.fond);
                ui.end_row();
                if e.opt.presentation != Presentation::Couleurs {
                    ui.label("Dessous");
                    bulles::bulle(
                        ui.checkbox(&mut e.opt.bande, "Bande des couleurs dominantes"),
                        "Bande des couleurs dominantes",
                        "Sous chaque image, jusqu'à cinq couleurs, chacune large comme la part de l'image qu'elle représente. Éteinte d'office.",
                        "",
                    );
                    ui.end_row();
                }
                ui.label("Version du PDF");
                ui.add(egui::TextEdit::singleline(&mut e.opt.version).hint_text("le dernier envoi").desired_width(160.0));
                ui.end_row();
            });
        });
        ui.separator();
        let cle: Cle = (e.opt.clone(), e.etendue, self.history.serial(), self.doc_epoch, self.board_sel.clone(), self.cur_panel);
        if e.resume.as_ref().map(|r| &r.0) != Some(&cle) {
            let plans = self.plans_de(e.etendue, None);
            let specs = colorscript::specs(&self.doc, &plans, &e.opt);
            let images = specs.iter().map(Spec::nombre).sum();
            let (taille, reduit) = specs.first().map_or(([0, 0], None), |s| (s.taille, s.reduit.clone()));
            e.resume = Some((cle, Resume { images, fichiers: specs.len(), taille, reduit }));
        }
        let r = e.resume.as_ref().map_or(Resume { images: 0, fichiers: 0, taille: [0, 0], reduit: None }, |r| r.1.clone());
        let pret = r.images > 0;
        if pret {
            let s = if r.images > 1 { "s" } else { "" };
            if e.opt.sortie == Sortie::PngParSequence {
                ui.label(format!("{} image{s}, {} fichier{} PNG.", r.images, r.fichiers, if r.fichiers > 1 { "s" } else { "" }));
            } else {
                ui.label(format!("{} image{s}, image de {} × {} pixels.", r.images, r.taille[0], r.taille[1]));
            }
        } else {
            crate::theme::avertir(ui, "Aucun plan dans cette étendue.");
        }
        if let Some(m) = r.reduit {
            crate::theme::avertir(ui, m);
        }
        if let Some(m) = &e.message {
            crate::theme::avertir(ui, m);
        }
        let libre = !matches!(&self.export_job, Some(j) if j.outcome.is_none());
        let mut lancer = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if bulles::icone_si(ui, pret && libre, ph::EXPORT, "Exporter le color script…", "Choisir le fichier (ou le dossier), puis l'écrire en tâche de fond : le projet reste libre pendant ce temps.", &self.raccourci("fichier.exporter_colorscript")).clicked() {
                lancer = true;
            }
            if !libre {
                crate::theme::avertir_petit(ui, "Un export est déjà en cours.");
            }
        });
        self.vignettes.colorscript = e;
        lancer
    }
}
