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
        Etat { ouverte: false, opt: Options { legendes: true, bloc_titre: true, ..Options::default() }, etendue: Etendue::Film, choix: ChoixResolution::Dpi150, largeur: 4000, message: None, resume: None }
    }
}

type Cle = (Options, Etendue, u64, u64, (u32, usize, usize), Vec<u64>, usize, Vec<(u64, bool)>);

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

    /// Les options de l'export posées sur celles de l'écran : le sens de lecture et les textes montrés, et l'opacité du passe-partout (celle de
    /// la barre du bas de la planche). Que le passe-partout soit allumé à l'écran n'y change rien : c'est la case de la fenêtre qui décide.
    pub(crate) fn planche_options(&self, opt: &Options) -> Options {
        Options { sens: self.layout.board_sens, textes: self.layout.board_textes, numeros: opt.numeros && self.settings.vignettes.numeros_visibles, marie_opacite: self.settings.vignettes.passe_partout_cases_opacite.min(100), debord: self.settings.vignettes.debord_fraction(), ..opt.clone() }
    }

    /// Les options et l'étendue du bouton « Exporter » : celles de la fenêtre, avec les deux cases retenues dans les réglages de l'application
    /// (« Avec les annotations », « Passe-partout »).
    pub(crate) fn planche_options_de_depart(&self) -> (Options, Etendue) {
        let e = &self.vignettes.planche;
        let mut opt = e.opt.clone();
        opt.resolution = e.resolution();
        (Options { annotations: self.settings.export_annotations, marie: self.settings.vignettes.export_passe_partout, ..opt }, e.etendue)
    }

    /// La case « Passe-partout » de la fenêtre : retenue dans les réglages de l'application (pas dans le projet).
    pub(crate) fn planche_regler_passe_partout(&mut self, oui: bool) {
        self.settings.vignettes.export_passe_partout = oui;
        self.settings_a_ecrire = true;
    }

    /// Le bloc de titre de la planche des plans `plans` (`entetepdf.rs`) : la série, l'épisode, la version (le dernier envoi), la date ; la séquence quand
    /// tous les plans sont de la même.
    pub(crate) fn planche_entete(&self, plans: &[usize]) -> crate::entetepdf::Entete {
        let base = crate::entetepdf::Entete::de_projet(&self.doc, self.doc.envois.dernier(), &crate::incrustation::date_text(crate::incrustation::now_secs()));
        let ids = self.doc.sequence_ids();
        let mut seqs: Vec<u64> = plans.iter().filter_map(|&p| ids.get(p).copied()).collect();
        seqs.sort_unstable();
        seqs.dedup();
        match seqs.as_slice() {
            [seq] => base.pour_sequence(&self.doc.sequence_label(*seq)),
            _ => base,
        }
    }

    /// La tâche d'export de la planche des plans `plans` vers `out` (rien n'est calculé ici).
    pub(crate) fn planche_job(&self, plans: &[usize], opt: &Options, out: &Path) -> Result<ExportJob, String> {
        if plans.is_empty() {
            return Err("Aucun plan dans cette étendue.".into());
        }
        // (Le sens de lecture et les textes montrés sont ceux de l'écran : l'export range les cases comme la planche.)
        let opt = &Options { entete: self.planche_entete(plans), ..self.planche_options(opt) };
        let p = planche::construire(&self.doc, plans, &self.layout.board_settings(), opt, &self.doc.annotations);
        let (s, dims, reduit) = planche::dimensions(p.cadre, opt.resolution);
        let n = p.cartes.len();
        let tailles: Arc<Vec<[u32; 2]>> = Arc::new((0..n).map(|k| planche::taille_vignette(&p, k, s)).collect());
        let frames = p.images();
        let frame = self.doc.frame;
        // (Les cadres de caméra ont la couleur choisie dans Préférences > Vignettes, comme à l'écran.)
        let (opt2, titre) = (Options { couleur_cadre: self.settings.vignettes.couleur_cadre, ..opt.clone() }, self.base_name_or("Planche"));
        // Le dessin sur toute l'enveloppe des plans à mouvement de caméra : préparé ici (un cliché léger), rendu sur le fil de l'export.
        let travaux = self.planche_tuiles_a_rendre(&p, s, opt);
        let (chemin, p2) = (out.to_path_buf(), Arc::new(p));
        let message = planche::message_de_fin(opt.sortie, n, dims, reduit.as_deref(), out);
        let job = ExportJob::collect_with(
            format!("Planche des vignettes ({n} vignette{})", if n > 1 { "s" } else { "" }),
            "Vignette",
            out,
            frames,
            move |k, px| rgb_sur_blanc(px, frame, tailles[k]),
            move |imgs: Vec<image::RgbImage>| {
                // (Le calcul des effets des tuiles se fait ici : l'interface garde la main, comme avec les fils de rendu.)
                crate::parallele::baisser_la_priorite_ici();
                let tuiles = rendre_les_tuiles(travaux);
                let img = planche::composer_avec_tuiles(&p2, &imgs, &tuiles, &opt2, s)?;
                drop(imgs);
                let bytes = planche::encoder_avec(&p2, &img, opt2.sortie, &titre, s, opt2.bloc_titre.then_some(&opt2.entete))?;
                crate::export::write_atomic(&chemin, &bytes)
            },
            || {},
        );
        // (« Avec les annotations » : les calques d'annotation de la planche — dessinés par `composer` — et ceux des plans, posés par-dessus
        // chaque vignette — `panel_plan_at_avec`.)
        Ok(job.pour_planche(message).avec_annotations(opt.annotations))
    }

    /// Pour chaque case : de quoi rendre ce qu'elle montre hors de son image (`TuileATravailler::rendre`) : sa tuile si sa caméra bouge (les
    /// cadres ou le passe-partout sont demandés), le dessin sur toute l'enveloppe ; sinon sa marge. Le passe-partout cache les marges : rien à
    /// en rendre. Un cliché léger du plan : le dessin n'est pas copié.
    fn planche_tuiles_a_rendre(&self, p: &planche::Planche, s: f32, opt: &Options) -> Vec<Option<TuileATravailler>> {
        let marge_de = |c: &planche::Carte| if opt.marie { 0 } else { (opt.debord * c.place.thumb.width() * s).round() as usize };
        p.cartes
            .iter()
            .enumerate()
            .map(|(k, c)| {
                let Some(m) = c.mouvement.as_ref() else {
                    let marge = marge_de(c);
                    if marge == 0 {
                        return None;
                    }
                    let plan = crate::render::panel_plan_at(&self.doc, c.plan_reel, c.place.frame, Some(self.doc.paper));
                    let doc = crate::thumbs::snapshot(&self.doc, &plan);
                    let t = planche::taille_vignette(p, k, s);
                    let cam = self.doc.panel_camera(c.plan_reel, c.place.frame);
                    return Some(TuileATravailler { doc, plan, cam, taille: [t[0] as usize, t[1] as usize], masque: None, marge });
                };
                let z = &m.tuile.zone;
                let r = m.tuile.rect;
                let taille = [((r.width() * s).round() as usize).clamp(1, 16_000), ((r.height() * s).round() as usize).clamp(1, 16_000)];
                let cadre = [z.taille.x.round().clamp(1.0, 16384.0) as usize, z.taille.y.round().clamp(1.0, 16384.0) as usize];
                let plan = crate::render::panel_plan_at(&self.doc, c.plan_reel, c.place.frame, Some(self.doc.paper));
                let mut doc = crate::thumbs::snapshot(&self.doc, &plan);
                doc.frame = cadre;
                let moitie = vec2(cadre[0] as f32, cadre[1] as f32) * 0.5;
                let masque = Some(m.tuile.masque.iter().map(|q| ((*q - z.centre) + moitie).to_pos2()).collect());
                let cam = crate::camera::Camera { x: z.centre.x, y: z.centre.y, zoom: 1.0, angle: 0.0, dolly: 0.0, tilt: 0.0, pan: 0.0 };
                Some(TuileATravailler { doc, plan, cam, taille, masque, marge: marge_de(c) })
            })
            .collect()
    }

    /// (Essais.) Les tuiles et les marges que l'export de la planche poserait, rendues ici.
    #[cfg(test)]
    pub(crate) fn planche_tuiles_pour_essai(&self, p: &planche::Planche, s: f32, opt: &Options) -> Vec<Option<planche::TuileImage>> {
        rendre_les_tuiles(self.planche_tuiles_a_rendre(p, s, opt))
    }

    /// Les plans de l'étendue choisie.
    fn planche_plans(&self, e: Etendue) -> Vec<usize> {
        // « Tout » : ce que la planche montre, dans l'ordre où elle le range, avec les titres des séquences.
        if e == Etendue::Film {
            return crate::planchesequences::plans_affiches(&self.doc);
        }
        self.plans_de(e, None)
    }

    /// Le bouton « Exporter » : où écrire, puis la tâche de fond.
    fn planche_exporter(&mut self) {
        if !self.export_libre() {
            return;
        }
        let (opt, etendue) = self.planche_options_de_depart();
        let plans = self.planche_plans(etendue);
        let ext = opt.sortie.extension();
        let base = self.base_name();
        let (nom_filtre, exts): (&str, &[&str]) = match opt.sortie {
            Sortie::Png => ("Image PNG", &["png"]),
            Sortie::Jpeg => ("Image JPEG", &["jpg", "jpeg"]),
            _ => ("Document PDF", &["pdf"]),
        };
        let Some(mut path) = self.dialogue_enregistrer("planche", "Exporter la planche des vignettes", (nom_filtre, exts), &format!("{base}_planche.{ext}")) else { return };
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
        ui.label(RichText::new("Toute la planche des vignettes en une grande image, ou en PDF, comme à l'écran. Les annotations n'y sont que si tu les coches.").small().color(theme::texte_doux()));
        ui.separator();
        let n_plans = self.doc.panels.len();
        let choisis = self.planche_plans(Etendue::Choisis).len();
        let (a, b) = self.doc.sequence_run(self.cur_panel.min(n_plans.saturating_sub(1)));
        // (Les calques d'annotation cachés de la planche n'y sont pas.)
        let annotations = self.doc.annotations.ordre_de_peinture().len();
        let mut avec_annotations = self.settings.export_annotations;
        let mut avec_passe_partout = self.settings.vignettes.export_passe_partout;
        let opacite_passe_partout = self.settings.vignettes.passe_partout_cases_opacite.min(100);
        let mut e = std::mem::take(&mut self.vignettes.planche);
        egui::Grid::new("planche_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
            ui.label("Plans");
            ui.vertical(|ui| {
                let montres = crate::planchesequences::plans_affiches(&self.doc).len();
                if montres == n_plans {
                    ui.radio_value(&mut e.etendue, Etendue::Film, format!("Tout le film ({n_plans})"));
                } else {
                    ui.radio_value(&mut e.etendue, Etendue::Film, format!("Ce que la planche montre ({montres} plans sur {n_plans})"));
                }
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
                bulles::bulle(ui.checkbox(&mut o.notes, "Notes"), "Notes", "Les textes sous les images : une ligne par champ coché dans le panneau Planche, coupée par « … » si elle est trop longue.", "");
                bulles::bulle(
                    ui.checkbox(&mut o.legendes, "Légendes : nom du plan et durée"),
                    "Légendes",
                    "Au-dessus de chaque vignette : le nom du plan selon le modèle de noms du projet (SQ01_PL012A, _C02 pour les cases d'après) et sa durée. Elles tiennent lieu du numéro seul.",
                    "",
                );
                bulles::bulle(
                    ui.checkbox(&mut o.bloc_titre, "Bloc de titre en haut des pages du PDF"),
                    "Bloc de titre",
                    "En haut de chaque page d'un PDF : la série, l'épisode, la séquence, la version (le nom du dernier envoi), la date et la page n / N. Sans effet pour une image.",
                    "",
                );
                bulles::bulle(ui.checkbox(&mut o.barres, "Barres de coupe"), "Barres de coupe", "Les barres entre deux vignettes : grises dans un même plan, noires à une coupe.", "");
                bulles::bulle(ui.checkbox(&mut o.cadres, "Cadres de caméra"), "Cadres de caméra", "Là où la caméra bouge : les cadres A, B, C… en bleu fin, avec leurs lettres, la flèche ou les coins qui les relient.", "");
                // Une seule case pour la planche et les plans (comme dans la fenêtre d'export du film) : les calques d'annotation de la planche
                // (par-dessus la planche) et ceux des plans (par-dessus chaque vignette). Décochée d'office, retenue par poste de travail.
                bulles::bulle(
                    ui.checkbox(&mut avec_annotations, "Avec les annotations"),
                    "Avec les annotations",
                    &format!("Les calques d'annotation de la planche ({annotations} annotation(s) visible(s)) et ceux des plans, par-dessus. Décochée d'office : elles ne font pas partie du storyboard. Retenu d'une séance à l'autre."),
                    "",
                );
                // Le passe-partout autour des cases : sa propre case, indépendante de l'interrupteur de l'écran ; seule son opacité est celle de
                // l'écran (barre du bas de la planche). Décochée d'office, retenue d'une séance à l'autre.
                ui.horizontal(|ui| {
                    bulles::bulle(
                        ui.checkbox(&mut avec_passe_partout, "Passe-partout"),
                        "Passe-partout",
                        &format!("Couvre tout ce qui est hors de l'intérieur des vignettes : seules les images se voient. Il a l'opacité réglée à l'écran ({opacite_passe_partout} %). Décoché d'office, indépendant du bouton de l'écran ; retenu d'une séance à l'autre."),
                        "",
                    );
                    if avec_passe_partout {
                        crate::champcouleur::srgb(ui, &mut o.marie_couleur);
                    }
                });
            });
            ui.end_row();
        });
        ui.separator();
        // « Avec les annotations » : retenu par poste de travail.
        if avec_annotations != self.settings.export_annotations {
            self.settings.export_annotations = avec_annotations;
            self.settings_a_ecrire = true;
        }
        e.opt.annotations = avec_annotations;
        // « Passe-partout » : retenu dans les réglages de l'application (pas dans le projet).
        if avec_passe_partout != self.settings.vignettes.export_passe_partout {
            self.planche_regler_passe_partout(avec_passe_partout);
        }
        e.opt.marie = avec_passe_partout;
        // Ce qui partirait.
        let mut opt = e.opt.clone();
        opt.resolution = e.resolution();
        (opt.sens, opt.textes) = (self.layout.board_sens, self.layout.board_textes);
        let cle: Cle = (opt.clone(), e.etendue, self.history.serial(), self.doc_epoch, (self.layout.card_size.to_bits(), self.layout.board_columns, self.layout.board_rows), self.board_sel.clone(), self.cur_panel, self.doc.annotations.affichage.ordre.clone());
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
            crate::theme::avertir(ui, "Aucun plan dans cette étendue.");
        }
        if let Some(r) = reduit {
            crate::theme::avertir(ui, r);
        }
        if let Some(m) = &e.message {
            crate::theme::avertir(ui, m);
        }
        let libre = !matches!(&self.export_job, Some(j) if j.outcome.is_none());
        let mut lancer = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if bulles::icone_si(ui, pret && libre, ph::EXPORT, "Exporter la planche…", "Choisir le fichier, puis l'écrire en tâche de fond : le projet reste libre pendant ce temps.", &self.raccourci("fichier.exporter_planche")).clicked() {
                lancer = true;
            }
            if !libre {
                crate::theme::avertir_petit(ui, "Un export est déjà en cours.");
            }
        });
        self.vignettes.planche = e;
        lancer
    }
}

/// Les tuiles et les marges d'un export de la planche, sur le fil de l'export, **exactes** : effets, remplissages et fonds se calculent ici, comme les images des cases, dans une
/// réserve privée (`fxcache::avec_privee`), sans rien demander au fil de fond de l'écran ni lui prendre. Un fichier ne montre jamais le repli de l'écran.
fn rendre_les_tuiles(travaux: Vec<Option<TuileATravailler>>) -> Vec<Option<planche::TuileImage>> {
    crate::fxcache::avec_privee(crate::fxcache::limite_octets().max(96 << 20), || {
        travaux
            .into_iter()
            .map(|t| {
                crate::fxcache::privee_nouvelle_image();
                t.and_then(TuileATravailler::rendre)
            })
            .collect()
    })
}

/// Ce qu'il faut pour rendre, dans l'export de la planche, la tuile d'un plan à mouvement de caméra (`masque` : son enveloppe) ou la marge d'une case.
struct TuileATravailler {
    doc: crate::document::Document,
    plan: crate::render::Plan,
    cam: crate::camera::Camera,
    taille: [usize; 2],
    masque: Option<Vec<eframe::egui::Pos2>>,
    marge: usize,
}

impl TuileATravailler {
    /// La tuile (le papier dans l'enveloppe, le trait qui déborde dehors) ou la marge seule, en pixels prémultipliés.
    fn rendre(self) -> Option<planche::TuileImage> {
        let img = match &self.masque {
            Some(masque) => crate::render::thumbnail_marge_until(&self.doc, &self.plan, &self.cam, self.taille[0], self.taille[1], self.marge, Some(masque), &|| false),
            None => Some(crate::render::marge_seule(&self.doc, &self.plan, &self.cam, self.taille[0], self.taille[1], self.marge)),
        }?;
        let rvba = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        Some(planche::TuileImage { taille: [img.size[0] as u32, img.size[1] as u32], marge: self.marge as u32, rvba })
    }
}
