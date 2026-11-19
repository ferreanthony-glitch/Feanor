//! Le décor 3D d'une séquence, côté interface (partie 180, `docs/INTERFACE.md`) : Fichier > Importer > « Décor 3D de la
//! séquence (glTF, .glb)… », la fenêtre « Ce décor est lourd », « Enlever le décor 3D de la séquence », la retouche de sa place et
//! de son aspect dans le panneau Espace 3D, le décor dans la vue Espace 3D, et le budget de cette machine (Préférences >
//! Performance).
//!
//! **Rien n'attend sur le fil de l'interface** : la lecture, la mesure, l'étalonnage de la machine, l'allègement et la
//! simplification tournent sur d'autres fils ; l'écran montre l'avancement et reste libre. Le fichier d'origine n'est jamais
//! modifié : seule la copie gardée dans le projet (`decors/<id>.glb`) est allégée ou simplifiée.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use eframe::egui::{self, pos2, Align2, Color32, FontId, Rect, RichText, Stroke};

use crate::app::App;
use crate::decor3d::{Allege, Analyse, Decor3d, Modele3d, Options, Place3d, Verdict};
use crate::decor3dbudget::{grand_nombre, BudgetMachine};
use crate::decor3dsimplifier::Suivi;
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, View, V3};

/// Ce que l'interface du décor retient (un par projet ouvert).
#[derive(Default)]
pub(crate) struct Etat3d {
    /// La lecture et la mesure d'un fichier, sur un autre fil.
    lecture: Option<JoinHandle<Result<Analyse, String>>>,
    /// Un fichier lu, qui attend la fin de l'étalonnage de la machine pour être jugé.
    lu: Option<Analyse>,
    /// La fenêtre « Ce décor est lourd ».
    lourd: Option<Lourd>,
    /// Les modèles déjà comparés au budget de cette machine (`Modele3d::uid`).
    vus: BTreeSet<u64>,
    /// La retouche en cours de la place ou de l'aspect : les décors d'avant (pour l'étape d'annulation, au lâcher).
    retouche: Option<crate::decor3d::Decors3d>,
}

impl Etat3d {
    /// La fenêtre « Ce décor est lourd » est ouverte (essais).
    #[cfg(test)]
    pub(crate) fn fenetre_ouverte(&self) -> bool {
        self.lourd.is_some()
    }

    /// Un import attend (lecture, étalonnage) (essais).
    #[cfg(test)]
    pub(crate) fn en_attente(&self) -> bool {
        self.lecture.is_some() || self.lu.is_some()
    }
}

/// Le but de la simplification.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cible {
    Moitie,
    Quart,
    Budget,
}

struct Calcul {
    options: Options,
    suivi: Arc<Suivi>,
    fil: Option<JoinHandle<Result<Allege, String>>>,
    fini: Option<Result<Allege, String>>,
}

struct Lourd {
    analyse: Analyse,
    raisons: Vec<String>,
    tel_quel: bool,
    alleger: bool,
    cote: u32,
    simplifier: bool,
    cible: Cible,
    calcul: Option<Calcul>,
    /// Un décor déjà dans le projet (ouvert sur cet ordinateur-ci) : sa séquence.
    existant: Option<u64>,
}

/// Le titre de la fenêtre.
pub(crate) const TITRE_LOURD: &str = "Ce décor est lourd";

impl App {
    /// La clé de cette machine (`decor3dbudget::cle_machine`).
    fn decor3d_cle(&self) -> String {
        crate::decor3dbudget::cle_machine(self.gpu.as_ref().map(|g| g.render_state().adapter.get_info()).as_ref())
    }

    /// Le budget gardé, s'il vaut pour cette machine.
    pub(crate) fn decor3d_budget(&self) -> Option<&BudgetMachine> {
        let cle = self.decor3d_cle();
        self.settings.decor3d.as_ref().filter(|b| b.cle == cle)
    }

    /// Au démarrage : le budget de cette machine, s'il a été mesuré ici, sert à l'écran et à l'export.
    pub(crate) fn decor3d_budget_au_demarrage(&mut self) {
        crate::decor3d::poser_budget(self.decor3d_budget().cloned());
    }

    /// Lance l'étalonnage de cette machine (rien s'il tourne déjà).
    fn decor3d_etalonner(&mut self) {
        let carte = self.gpu.as_ref().map(|g| g.render_state().clone());
        let max = carte.as_ref().map_or(16384, |rs| rs.device.limits().max_texture_dimension_2d);
        let feuille = [(self.doc.width as u32).min(max), (self.doc.height as u32).min(max)];
        crate::decor3dbudget::lancer(carte, feuille, self.doc.frame);
        self.status = "Décor 3D : FEANOR mesure ce que cet ordinateur dessine sans ralentir (quelques secondes)…".into();
    }

    /// « Remesurer » (Préférences > Performance, diagnostic des performances, palette).
    pub(crate) fn decor3d_remesurer(&mut self) {
        self.decor3d_etalonner();
    }

    /// Fichier > Importer > « Décor 3D de la séquence (glTF, .glb)… ».
    pub(crate) fn decor3d_importer(&mut self) {
        if self.sans_projet() || self.content_refuses() {
            return;
        }
        if self.decor3d.lecture.is_some() || self.decor3d.lourd.is_some() {
            self.status = "Un décor 3D est déjà en cours d'import.".into();
            return;
        }
        let Some(chemin) = rfd::FileDialog::new().set_title("Importer le décor 3D de la séquence").add_filter("glTF 2.0", &["glb", "gltf"]).pick_file() else { return };
        self.decor3d_importer_chemin(chemin);
    }

    /// Lit et mesure `chemin` sur un autre fil ; la suite à `decor3d_tick`.
    pub(crate) fn decor3d_importer_chemin(&mut self, chemin: PathBuf) {
        let carte = self.gpu.as_ref().map(|g| g.render_state().clone());
        let (feuille, cadre) = ([self.doc.width, self.doc.height], self.doc.frame);
        self.decor3d.lecture = Some(std::thread::spawn(move || {
            let mut a = crate::decor3d::analyser(&chemin)?;
            // Le temps réel d'une image de ce décor sur la carte de cette machine (s'il n'est pas d'emblée hors budget).
            if let Some(rs) = carte.filter(|_| a.mesures.triangles <= 4_000_000) {
                a.cout_ms = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mesurer_cout(&rs, &a.scene, feuille, cadre))).ok().flatten();
            }
            Ok(a)
        }));
        if self.decor3d_budget().is_none() {
            self.decor3d_etalonner();
        } else {
            self.status = "Décor 3D : lecture du fichier…".into();
        }
    }

    /// À chaque image d'écran : l'étalonnage, la lecture, la fenêtre, la vérification des décors du projet.
    pub(crate) fn decor3d_tick(&mut self, ctx: &egui::Context) {
        if let Some(b) = crate::decor3dbudget::fini() {
            self.status = format!("Décor 3D sur cet ordinateur : {}.", b.texte());
            self.settings.decor3d = Some(b);
            let _ = self.settings.save();
            crate::decor3d::poser_budget(self.settings.decor3d.clone());
            // (Un nouveau budget : les décors du projet sont jugés de nouveau.)
            self.decor3d.vus.clear();
        }
        if crate::decor3dbudget::en_cours() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if self.decor3d.lecture.as_ref().is_some_and(|h| h.is_finished()) {
            let r = self.decor3d.lecture.take().and_then(|h| h.join().ok()).unwrap_or_else(|| Err("lecture interrompue".into()));
            match r {
                Ok(a) => self.decor3d.lu = Some(a),
                Err(e) => self.status = format!("Décor 3D refusé : {e}"),
            }
        } else if self.decor3d.lecture.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        // Un fichier lu se juge quand le budget de la machine est là.
        if self.decor3d.lu.is_some() {
            if let Some(b) = self.decor3d_budget().cloned() {
                let a = self.decor3d.lu.take().expect("fichier lu");
                self.decor3d_juger(a, &b, None);
            } else if !crate::decor3dbudget::en_cours() {
                self.decor3d_etalonner();
            }
        }
        // Les décors du projet (ouvert sur cet ordinateur, ou rétablis) : comparés au budget d'ici une fois.
        let nouveaux: Vec<(u64, Decor3d)> = self.doc.decors3d.par_sequence.iter().filter(|(_, d)| !self.decor3d.vus.contains(&d.modele.uid)).map(|(s, d)| (*s, d.clone())).collect();
        if !nouveaux.is_empty() && self.decor3d.lourd.is_none() {
            match self.decor3d_budget().cloned() {
                Some(b) => {
                    for (seq, d) in nouveaux {
                        self.decor3d.vus.insert(d.modele.uid);
                        let mesures = mesures_de(&d.modele);
                        if self.decor3d.lourd.is_none() && matches!(crate::decor3d::verdict(&mesures, &b, None), Verdict::Lourd { tel_quel: false, .. }) {
                            let a = Analyse { nom: d.nom.clone(), origine: d.origine.clone(), glb: d.modele.octets.clone(), scene: d.modele.scene.clone(), mesures, cout_ms: None };
                            self.status = format!("Le décor 3D « {} » est plus lourd que ce que cet ordinateur dessine sans ralentir : il est masqué en attendant ton choix.", d.nom);
                            crate::decor3d::masquer(d.modele.uid, true);
                            self.decor3d_juger(a, &b, Some(seq));
                        }
                    }
                }
                None if !crate::decor3dbudget::en_cours() => self.decor3d_etalonner(),
                None => {}
            }
        }
        self.decor3d_fenetre(ctx);
    }

    /// Le fichier lu, comparé au budget : importé tout de suite, ou la fenêtre « Ce décor est lourd ».
    fn decor3d_juger(&mut self, a: Analyse, b: &BudgetMachine, existant: Option<u64>) {
        match crate::decor3d::verdict(&a.mesures, b, a.cout_ms) {
            Verdict::Direct => self.decor3d_poser(a.nom.clone(), a.origine.clone(), a.glb.clone(), a.scene.clone(), Vec::new()),
            Verdict::Lourd { raisons, tel_quel, simplifier } => {
                let trop_textures = a.mesures.octets_textures > b.textures_mo << 20 || a.mesures.plus_grand_cote > 4096;
                self.decor3d.lourd = Some(Lourd { analyse: a, raisons, tel_quel, alleger: true, cote: if trop_textures { 2048 } else { 4096 }, simplifier, cible: Cible::Budget, calcul: None, existant });
            }
        }
    }

    /// Pose le décor (une étape d'annulation) pour la séquence du plan courant.
    fn decor3d_poser(&mut self, nom: String, origine: String, glb: Arc<Vec<u8>>, scene: Arc<crate::decor3dgltf::Scene3d>, allege: Vec<String>) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let monde = Monde::of(&self.doc);
        let place = Place3d::d_origine(&scene, &monde);
        let (lo, hi) = scene.bornes();
        let triangles = scene.triangles();
        let modele = Arc::new(Modele3d::new(scene, glb));
        self.decor3d.vus.insert(modele.uid);
        let snap = self.snapshot();
        let id = self.doc.alloc();
        let ancien = self.doc.decors3d.par_sequence.insert(seq, Decor3d { id, nom: nom.clone(), origine, allege: allege.clone(), place, aspect: Default::default(), modele });
        let before = self.history.context.replace(if ancien.is_some() { "Remplacer le décor 3D".into() } else { "Importer le décor 3D".into() });
        self.commit_structure(snap);
        self.history.context = before;
        let taille = [0, 1, 2].map(|k| hi[k] - lo[k]);
        let plus_grand = taille.iter().cloned().fold(0.0f32, f32::max);
        self.status = format!(
            "Décor 3D « {nom} » posé pour la séquence : {:.2} × {:.2} × {:.2} m (largeur, hauteur, profondeur), {} triangles{}{}.",
            taille[0],
            taille[1],
            taille[2],
            grand_nombre(triangles as u64),
            if plus_grand > 500.0 {
                " — très grand : vérifie son échelle"
            } else if plus_grand < 0.05 {
                " — très petit : vérifie son échelle"
            } else {
                ""
            },
            if allege.is_empty() { String::new() } else { format!(" ; {}", allege.join(", ")) }
        );
    }

    /// « Enlever le décor 3D de la séquence » (une étape d'annulation).
    pub(crate) fn decor3d_enlever(&mut self) {
        let seq = self.doc.sequence_of(self.cur_panel);
        if !self.doc.decors3d.par_sequence.contains_key(&seq) {
            self.status = "Cette séquence n'a pas de décor 3D.".into();
            return;
        }
        if self.content_refuses() {
            return;
        }
        let snap = self.snapshot();
        let ancien = self.doc.decors3d.par_sequence.remove(&seq);
        let before = self.history.context.replace("Enlever le décor 3D".into());
        self.commit_structure(snap);
        self.history.context = before;
        self.status = format!("Décor 3D « {} » enlevé de la séquence (Ctrl+Z le rend).", ancien.map(|d| d.nom).unwrap_or_default());
    }

    // ------------------------------------------------------------------------------------- la fenêtre « lourd »

    fn decor3d_fenetre(&mut self, ctx: &egui::Context) {
        let Some(mut l) = self.decor3d.lourd.take() else { return };
        let budget = self.decor3d_budget().cloned();
        let options = l.options(budget.as_ref());
        // Le calcul d'après suit les choix : un autre choix arrête le calcul en cours et en lance un neuf.
        if l.calcul.as_ref().is_none_or(|c| c.options != options) {
            if let Some(c) = l.calcul.take() {
                c.suivi.annule.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if options.cote_max.is_some() || options.triangles.is_some() {
                let suivi = Arc::new(Suivi::default());
                let (a, s) = (l.analyse.clone(), suivi.clone());
                let fil = std::thread::spawn(move || crate::decor3d::alleger(&a, options, &s));
                l.calcul = Some(Calcul { options, suivi, fil: Some(fil), fini: None });
            }
        }
        if let Some(c) = l.calcul.as_mut() {
            if c.fil.as_ref().is_some_and(|h| h.is_finished()) {
                c.fini = Some(c.fil.take().and_then(|h| h.join().ok()).unwrap_or_else(|| Err("calcul interrompu".into())));
            } else if c.fil.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        let mut choix: Option<&'static str> = None;
        let mut pret: Option<Allege> = None;
        let mm = l.analyse.mesures.clone();
        egui::Window::new(TITRE_LOURD).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(RichText::new(format!("« {} » ({})", l.analyse.nom, l.analyse.origine)).strong());
            egui::Grid::new("decor3d_mesures").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                let mut ligne = |a: &str, b: String| {
                    ui.label(a);
                    ui.label(b);
                    ui.end_row();
                };
                ligne("Fichier", crate::decor3d::octets_lisibles(mm.octets_fichier));
                ligne("Triangles", grand_nombre(mm.triangles as u64));
                ligne("Sommets", grand_nombre(mm.sommets as u64));
                ligne("Matières", mm.matieres.to_string());
                ligne("Textures", format!("{} ({} en mémoire, plus grand côté {} px)", mm.textures, crate::decor3d::octets_lisibles(mm.octets_textures), mm.plus_grand_cote));
                if mm.cartes_inutiles + mm.uv_en_plus + mm.tangentes + mm.couleurs_sommets + mm.animations + mm.peaux + mm.morphoses > 0 {
                    ligne(
                        "Inutilisé par FEANOR",
                        format!(
                            "{} carte(s) de normales, rugosité, occlusion ou émission ({}) ; {} primitive(s) avec d'autres coordonnées, tangentes ou couleurs de sommets ; {} animation(s), {} squelette(s), {} morphose(s)",
                            mm.cartes_inutiles,
                            crate::decor3d::octets_lisibles(mm.octets_cartes_inutiles),
                            mm.uv_en_plus.max(mm.tangentes).max(mm.couleurs_sommets),
                            mm.animations,
                            mm.peaux,
                            mm.morphoses
                        ),
                    );
                }
                if let Some(ms) = l.analyse.cout_ms {
                    ligne("Une image sur la carte", format!("{ms:.1} ms"));
                }
            });
            if let Some(b) = &budget {
                ui.label(RichText::new(format!("Cet ordinateur : {} (pour rester fluide).", b.texte())).small().weak());
            }
            ui.add_space(4.0);
            for r in &l.raisons {
                ui.label(format!("• {r}"));
            }
            ui.separator();
            ui.checkbox(&mut l.alleger, "Alléger (la forme ne change pas)");
            ui.add_enabled_ui(l.alleger, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Textures au plus :");
                    ui.radio_value(&mut l.cote, 4096, "4096 px");
                    ui.radio_value(&mut l.cote, 2048, "2048 px");
                    ui.radio_value(&mut l.cote, 1024, "1024 px");
                });
                ui.label(RichText::new("Retire aussi ce que FEANOR ne montre pas : cartes de normales, de rugosité, d'occlusion et d'émission, coordonnées en plus, tangentes, couleurs de sommets, animations, squelettes et morphoses (ils compteront pour les personnages, à l'étape 2 : le fichier d'origine les garde). Sommets soudés, indices sur 16 bits quand ils tiennent.").small().weak());
            });
            ui.checkbox(&mut l.simplifier, "Simplifier la géométrie (modifie la forme)");
            ui.add_enabled_ui(l.simplifier, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut l.cible, Cible::Moitie, "50 %");
                    ui.radio_value(&mut l.cible, Cible::Quart, "25 %");
                    let b = budget.as_ref().map_or("le budget".to_string(), |b| grand_nombre(b.triangles));
                    ui.radio_value(&mut l.cible, Cible::Budget, format!("jusqu'à {b} triangles"));
                });
                ui.label(RichText::new("Contraction d'arêtes guidée par l'erreur : les bords, les coutures de texture et les frontières de matière restent.").small().weak());
            });
            ui.separator();
            match l.calcul.as_ref() {
                None => {
                    ui.label(RichText::new("Coche « Alléger » ou « Simplifier » pour voir le résultat.").weak());
                }
                Some(c) => match &c.fini {
                    None => {
                        ui.add(egui::ProgressBar::new(c.suivi.fraction()).text("Calcul de l'allègement…").animate(true));
                    }
                    Some(Err(e)) => {
                        ui.colored_label(Color32::from_rgb(220, 120, 90), format!("Allègement impossible : {e}"));
                    }
                    Some(Ok(r)) => {
                        let apres = &r.mesures;
                        ui.label(format!(
                            "Avant → après : fichier {} → {} ; textures en mémoire {} → {} ; triangles {} → {}.",
                            crate::decor3d::octets_lisibles(mm.octets_fichier),
                            crate::decor3d::octets_lisibles(apres.octets_fichier),
                            crate::decor3d::octets_lisibles(mm.octets_textures),
                            crate::decor3d::octets_lisibles(apres.octets_textures),
                            grand_nombre(mm.triangles as u64),
                            grand_nombre(apres.triangles as u64),
                        ));
                        let tient = budget.as_ref().is_none_or(|b| matches!(crate::decor3d::verdict(apres, b, None), Verdict::Direct | Verdict::Lourd { tel_quel: true, .. }));
                        if tient {
                            pret = Some(r.clone());
                        } else {
                            ui.colored_label(Color32::from_rgb(220, 160, 90), "Encore au-delà de ce que cet ordinateur dessine sans ralentir : allège ou simplifie davantage.");
                        }
                    }
                },
            }
            ui.horizontal(|ui| {
                let verbe = match (l.simplifier, l.alleger) {
                    (true, true) => "Simplifier, alléger et importer",
                    (true, false) => "Simplifier et importer",
                    _ => "Alléger et importer",
                };
                let verbe = if l.existant.is_some() { verbe.replace("importer", "remplacer") } else { verbe.to_string() };
                if ui.add_enabled(pret.is_some(), egui::Button::new(verbe)).clicked() {
                    choix = Some("alleger");
                }
                if l.tel_quel && ui.button(if l.existant.is_some() { "Garder tel quel" } else { "Importer tel quel" }).clicked() {
                    choix = Some("tel_quel");
                }
                if ui.button(if l.existant.is_some() { "Le masquer sur cet ordinateur" } else { "Annuler" }).clicked() {
                    choix = Some("annuler");
                }
            });
        });
        match choix {
            Some("alleger") => {
                if let Some(r) = pret {
                    match l.existant {
                        Some(seq) => self.decor3d_remplacer(seq, r),
                        None => self.decor3d_poser(l.analyse.nom.clone(), l.analyse.origine.clone(), r.glb.clone(), r.scene.clone(), r.notes.clone()),
                    }
                }
            }
            Some("tel_quel") => match l.existant {
                Some(_) => {
                    if let Some(d) = self.doc.decors3d.par_sequence.values().find(|d| Arc::ptr_eq(&d.modele.scene, &l.analyse.scene)) {
                        crate::decor3d::masquer(d.modele.uid, false);
                    }
                }
                None => self.decor3d_poser(l.analyse.nom.clone(), l.analyse.origine.clone(), l.analyse.glb.clone(), l.analyse.scene.clone(), Vec::new()),
            },
            Some(_) => {
                if let Some(c) = l.calcul {
                    c.suivi.annule.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if l.existant.is_none() {
                    self.status = "Import du décor 3D annulé.".into();
                }
            }
            None => self.decor3d.lourd = Some(l),
        }
    }

    /// Le décor de la séquence `seq`, remplacé par sa version allégée (une étape d'annulation) ; son nom et sa place restent.
    fn decor3d_remplacer(&mut self, seq: u64, r: Allege) {
        let Some(mut d) = self.doc.decors3d.par_sequence.get(&seq).cloned() else { return };
        let snap = self.snapshot();
        d.modele = Arc::new(Modele3d::new(r.scene.clone(), r.glb.clone()));
        d.id = self.doc.alloc();
        d.allege.extend(r.notes.iter().cloned());
        self.decor3d.vus.insert(d.modele.uid);
        let nom = d.nom.clone();
        self.doc.decors3d.par_sequence.insert(seq, d);
        let before = self.history.context.replace("Alléger le décor 3D".into());
        self.commit_structure(snap);
        self.history.context = before;
        self.status = format!("Décor 3D « {nom} » allégé : {}.", r.notes.join(", "));
    }

    // ---------------------------------------------------------------------------------- la vue Espace 3D

    /// Le décor de la séquence dans une vue Espace 3D (entre le quadrillage et les cartes) : dessiné par la carte graphique ;
    /// sans elle, sa boîte en fil de fer et une légende. `cle` : la prise (une par vue).
    pub(crate) fn e3_peindre_decor(&self, painter: &egui::Painter, draw: Rect, view: &View, cle: u64) {
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else { return };
        if crate::decor3d::masque(d.modele.uid) {
            return;
        }
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        match self.gpu.as_ref() {
            Some(g) => {
                let qualite = if d.aspect.ombres { crate::decor3d::qualite(&d.modele.scene) } else { crate::decor3dbudget::Qualite::Sans };
                let cb = crate::decor3dgpu::VueDecor {
                    cle,
                    dessins: vec![crate::decor3dgpu::Dessin { id: d.modele.uid, scene: d.modele.scene.clone(), matrice: d.place.matrice(), aspect: d.aspect }],
                    vue: crate::decor3dgpu::Vue::de_view(view, center),
                    taille_pts: [draw.width(), draw.height()],
                    cible_egui: g.render_state().target_format,
                    qualite,
                };
                painter.add(eframe::egui_wgpu::Callback::new_paint_callback(draw, cb));
            }
            None => {
                // Sans carte graphique : la boîte du décor, en fil de fer.
                let (lo, hi) = d.modele.scene.bornes();
                let mx = d.place.matrice();
                let coin = |i: usize| m::point(&mx, [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }));
                let proj = |p: V3| view.project(p, center).map(|q| pos2(draw.left() + q[0], draw.top() + q[1]));
                let trait_ = Stroke::new(1.0_f32, Color32::from_rgb(150, 170, 200));
                for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                    if let (Some(p), Some(q)) = (proj(coin(a)), proj(coin(b))) {
                        painter.line_segment([p, q], trait_);
                    }
                }
                painter.text(draw.left_bottom() + egui::vec2(8.0, -22.0), Align2::LEFT_BOTTOM, format!("Décor 3D « {} » : sa boîte (sans carte graphique, la vue ne le dessine pas)", d.nom), FontId::proportional(11.0), Color32::from_gray(170));
            }
        }
    }

    // ------------------------------------------------------------------------------- le panneau Espace 3D

    /// Le décor de la séquence dans le panneau Espace 3D : son nom, ce qui a été allégé, sa place (position, rotation autour de
    /// la verticale, échelle), son aspect, « Remettre au sol », « Enlever ».
    pub(crate) fn decor3d_reglages(&mut self, ui: &mut egui::Ui) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let Some(mut d) = self.doc.decors3d.par_sequence.get(&seq).cloned() else {
            ui.add_space(6.0);
            if ui.button("Importer le décor 3D de la séquence…").on_hover_text("Un modèle glTF (.glb, .gltf) que tous les plans de la séquence montrent, derrière les calques.").clicked() {
                self.run_action("fichier.importer_decor3d");
            }
            return;
        };
        ui.add_space(6.0);
        let avant = d.clone();
        let budget = self.decor3d_budget().cloned();
        let mut enlever = false;
        egui::CollapsingHeader::new(RichText::new(format!("Décor 3D : {}", d.nom)).strong()).id_salt("decor3d_reglages").default_open(true).show(ui, |ui| {
            if !d.origine.is_empty() {
                ui.label(RichText::new(format!("Fichier d'origine : {}", d.origine)).small().weak());
            }
            if !d.allege.is_empty() {
                ui.label(RichText::new(format!("Allégé : {}", d.allege.join(", "))).small().weak());
            }
            egui::Grid::new("decor3d_place").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                ui.label("Position (m)");
                ui.horizontal(|ui| {
                    for v in d.place.position.iter_mut() {
                        ui.add(egui::DragValue::new(v).speed(0.02).max_decimals(3));
                    }
                });
                ui.end_row();
                ui.label("Rotation (°)");
                ui.add(egui::DragValue::new(&mut d.place.rotation_y).speed(0.5).suffix("°"));
                ui.end_row();
                ui.label("Échelle");
                ui.add(egui::DragValue::new(&mut d.place.echelle).speed(0.005).range(0.001..=1000.0));
                ui.end_row();
                ui.label("Soleil");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut d.aspect.soleil_azimut).speed(1.0).suffix("° autour"));
                    ui.add(egui::DragValue::new(&mut d.aspect.soleil_hauteur).speed(0.5).range(5.0..=89.0).suffix("° haut"));
                });
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut d.aspect.facettes, "Facettes");
                ui.checkbox(&mut d.aspect.ciel, "Ciel");
                ui.checkbox(&mut d.aspect.brume, "Brume");
                ui.checkbox(&mut d.aspect.ombres, "Ombres");
            });
            if let Some(b) = &budget {
                let q = if d.aspect.ombres { b.qualite_pour(d.modele.scene.triangles() as u64) } else { crate::decor3dbudget::Qualite::Sans };
                ui.label(RichText::new(format!("{} triangles · sur cet ordinateur : {}", grand_nombre(d.modele.scene.triangles() as u64), q.nom())).small().weak());
            }
            ui.horizontal(|ui| {
                if ui.button("Remettre au sol").on_hover_text("Le bas du décor sur le sol, centré, derrière la feuille ; rotation et échelle d'origine.").clicked() {
                    d.place = Place3d::d_origine(&d.modele.scene, &Monde::of(&self.doc));
                }
                if ui.button("Enlever").clicked() {
                    enlever = true;
                }
            });
        });
        if enlever {
            self.run_action("decor3d.enlever");
            return;
        }
        if d.place != avant.place || d.aspect != avant.aspect {
            if self.decor3d.retouche.is_none() {
                self.decor3d.retouche = Some(self.doc.decors3d.clone());
            }
            self.doc.decors3d.par_sequence.insert(seq, d);
            self.modified = true;
        }
        // Le geste fini (rien n'est tenu, aucun champ n'est en saisie) : une étape d'annulation pour toute la retouche.
        let tenu = ui.input(|i| i.pointer.any_down()) || ui.ctx().memory(|mem| mem.focused().is_some());
        if !tenu {
            if let Some(avant) = self.decor3d.retouche.take() {
                let maintenant = std::mem::replace(&mut self.doc.decors3d, avant);
                let snap = self.snapshot();
                self.doc.decors3d = maintenant;
                let before = self.history.context.replace("Déplacer le décor 3D".into());
                self.commit_structure(snap);
                self.history.context = before;
            }
        }
    }

    /// Préférences > Performance : le budget 3D de cette machine et « Remesurer ».
    pub(crate) fn decor3d_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let mut remesurer = false;
        ui.horizontal_wrapped(|ui| {
            let texte = match self.decor3d_budget() {
                Some(b) => format!("Décor 3D sur cet ordinateur : {} (mesuré le {}).", b.texte(), date_lisible(b.date)),
                None if crate::decor3dbudget::en_cours() => "Décor 3D sur cet ordinateur : mesure en cours…".to_string(),
                None => "Décor 3D sur cet ordinateur : pas encore mesuré (au premier décor importé ou ouvert).".to_string(),
            };
            ui.label(texte);
            let r = ui.add_enabled(!crate::decor3dbudget::en_cours(), egui::Button::new("Remesurer"));
            remesurer = r.on_hover_text("Mesure de nouveau ce que la carte graphique et le processeur de cet ordinateur dessinent sans ralentir (quelques secondes, en arrière-plan).").clicked();
        });
        if remesurer {
            self.decor3d_remesurer();
        }
    }
}

impl Lourd {
    fn options(&self, budget: Option<&BudgetMachine>) -> Options {
        let t = self.analyse.mesures.triangles;
        let triangles = self.simplifier.then(|| match self.cible {
            Cible::Moitie => t / 2,
            Cible::Quart => t / 4,
            Cible::Budget => budget.map_or(t / 2, |b| (b.triangles as usize).min(t)),
        });
        Options { cote_max: self.alleger.then_some(self.cote), triangles }
    }
}

/// Les mesures d'un décor déjà chargé (ouvert avec le projet).
pub(crate) fn mesures_de(m: &Modele3d) -> crate::decor3dgltf::Mesures {
    let s = &m.scene;
    crate::decor3dgltf::Mesures {
        octets_fichier: m.octets.len() as u64,
        triangles: s.triangles(),
        sommets: s.sommets(),
        matieres: s.primitives.len(),
        textures: s.textures.len(),
        octets_textures: s.octets_textures(),
        plus_grand_cote: s.textures.iter().map(|t| t.largeur.max(t.hauteur)).max().unwrap_or(0),
        ..Default::default()
    }
}

/// Le temps d'une image du décor sur la carte, à la taille de la feuille, posé à sa place d'origine (millisecondes).
fn mesurer_cout(rs: &eframe::egui_wgpu::RenderState, scene: &Arc<crate::decor3dgltf::Scene3d>, feuille: [usize; 2], cadre: [usize; 2]) -> Option<f32> {
    let max = rs.device.limits().max_texture_dimension_2d as usize;
    let feuille = [feuille[0].min(max), feuille[1].min(max)];
    let monde = Monde::new(feuille, cadre);
    let prise = crate::decor3drendu::Prise::de_la_feuille(&monde, monde.eye_reference())?;
    let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    let d = [crate::decor3dgpu::Dessin { id: 1, scene: scene.clone(), matrice: Place3d::d_origine(scene, &monde).matrice(), aspect: Default::default() }];
    crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &prise, [feuille[0] as u32, feuille[1] as u32], true, crate::decor3dbudget::Qualite::Sans, 4).map(|r| r.1)
}

/// « 19/11/2026 ».
fn date_lisible(secondes: u64) -> String {
    let jours = (secondes / 86_400) as i64;
    // (Calendrier civil depuis le 1er janvier 1970 : jours depuis l'époque vers année, mois, jour.)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + ere * 400 + i64::from(mo <= 2);
    format!("{d:02}/{mo:02}/{y}")
}
