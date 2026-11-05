//! Importer un scénario, et en tirer des plans : deux fenêtres, la même (Anthony, 2026-10-30 : « garder la
//! lecture du scénario par SHOT pour le traduire en plans comme une option disponible à partir du panneau,
//! mais pas native à l'import »).
//!
//! - **Importer** (Fichier > Importer > Scénario…, action `fichier.importer_scenario`, donc aussi dans Ctrl+K) :
//!   le fichier est choisi, lu sur un autre fil (avec une barre d'avancement : un scénario de 120 pages est lu en
//!   une fraction de seconde, un très gros fichier ne bloque pas l'interface), puis la fenêtre dit ce qui a été
//!   lu. « Importer » le **garde dans le projet** (`scenariogarde`) et ouvre le panneau Scénario ; aucun plan
//!   n'est créé. Une seule étape d'annulation, « Importer un scénario ». Si le projet a déjà un scénario, la
//!   fenêtre demande s'il est remplacé (l'ancien fichier est gardé à côté). Sans projet ouvert, un nouveau projet
//!   le reçoit.
//! - **Créer les plans depuis le scénario…** (bouton du panneau Scénario, action `scenario.creer_plans`) : la
//!   fenêtre de découpage d'avant (choix du découpage, du rythme, de la destination), qui part du scénario déjà
//!   gardé, sans redemander de fichier. Les plans s'ajoutent à la fin du projet ou après la séquence du plan
//!   courant, sans toucher aux plans qui existent (seul un projet encore vierge est remplacé) ; une seule étape
//!   d'annulation, « Créer les plans du scénario ». Ces plans sont liés au texte (surlignage et clic du panneau).
//!   Voir `scenario`, `scenarioplans`, `scenariopanneau`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::{App, Pending};
use crate::dock::Pane;
use crate::scenario::Script;
use crate::scenariogarde::Gardee;
use crate::scenarioplans::{self, Dest, Options, Outline, Rhythm, Split};

/// Le nom de l'étape d'annulation de l'import (le scénario est gardé dans le projet).
pub const STEP_NAME: &str = "Importer un scénario";
/// Le nom de l'étape d'annulation de la création des plans.
pub const PLANS_STEP_NAME: &str = "Créer les plans du scénario";

/// Ce que la fenêtre fait : garder le scénario dans le projet, ou en créer les plans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Garder,
    Plans,
}

type Reading = Arc<Mutex<Option<Result<Script, String>>>>;

enum Load {
    /// Lecture en cours sur un autre fil.
    Reading { progress: Arc<AtomicU32>, result: Reading },
    Ready(Box<Script>),
    Failed(String),
}

/// L'état de la fenêtre.
pub(crate) struct ScenarioWindow {
    path: PathBuf,
    name: String,
    load: Load,
    options: Options,
    dest: Dest,
    /// Ce que les choix actuels donnent (recalculé quand ils changent).
    built: Option<(Options, Outline)>,
    mode: Mode,
    /// « Remplacer le scénario du projet ? » est posée.
    confirm: bool,
}

#[cfg(test)]
impl ScenarioWindow {
    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    pub(crate) fn asks_to_replace(&self) -> bool {
        self.confirm
    }

    /// Le scénario est lu (et prêt à être importé).
    pub(crate) fn ready(&self) -> bool {
        matches!(self.load, Load::Ready(_))
    }

    /// Pourquoi la lecture a échoué, s'il y a lieu.
    pub(crate) fn failure(&self) -> Option<&str> {
        match &self.load {
            Load::Failed(e) => Some(e),
            _ => None,
        }
    }
}

/// Les derniers choix, gardés le temps de la séance.
static LAST: Mutex<Option<(Options, Dest)>> = Mutex::new(None);

/// « 2 min 05 s », « 48 s ».
pub fn duration_text(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{} h {:02} min", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{} min {:02} s", s / 60, s % 60)
    } else {
        format!("{s} s")
    }
}

impl App {
    /// Fichier > Importer > Scénario… : choisit le fichier, puis ouvre la fenêtre.
    pub(crate) fn open_scenario_window(&mut self) {
        if self.scenario_window.as_ref().is_some_and(|w| matches!(w.load, Load::Reading { .. })) {
            return;
        }
        let picked = rfd::FileDialog::new()
            .set_title("Scénario à importer")
            .add_filter("Scénarios (.fountain, .txt, .fdx, .docx)", &["fountain", "spmd", "txt", "fdx", "docx"])
            .add_filter("Tous les fichiers", &["*"])
            .pick_file();
        if let Some(path) = picked {
            self.read_scenario(path);
        }
    }

    /// Lance la lecture de `path` sur un autre fil et ouvre la fenêtre.
    pub(crate) fn read_scenario(&mut self, path: PathBuf) {
        let progress = Arc::new(AtomicU32::new(0));
        let result: Reading = Arc::new(Mutex::new(None));
        let (p, r, file) = (progress.clone(), result.clone(), path.clone());
        std::thread::spawn(move || {
            let report = |f: f32| p.store(f.to_bits(), Ordering::Relaxed);
            let read = crate::scenario::read_file(&file, &report);
            *r.lock().unwrap_or_else(|e| e.into_inner()) = Some(read);
        });
        let (options, dest) = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or((Options::default(), if self.welcome { Dest::NewProject } else { Dest::End }));
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        self.scenario_window = Some(ScenarioWindow { path, name, load: Load::Reading { progress, result }, options, dest, built: None, mode: Mode::Garder, confirm: false });
    }

    /// « Créer les plans depuis le scénario… » : la fenêtre de découpage, à partir du scénario gardé dans le projet.
    pub(crate) fn open_plans_window(&mut self) {
        let Some(g) = self.doc.scenario.clone() else {
            self.status = "Ce projet n'a pas de scénario : importe-en un d'abord (Fichier > Importer > Scénario).".into();
            return;
        };
        if self.scenario_window.as_ref().is_some_and(|w| matches!(w.load, Load::Reading { .. })) {
            return;
        }
        let (options, dest) = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or((Options::default(), Dest::End));
        // (Les plans s'ajoutent au projet ouvert : jamais « dans un nouveau projet » depuis le panneau.)
        let dest = if dest == Dest::NewProject { Dest::End } else { dest };
        self.scenario_window = Some(ScenarioWindow {
            path: PathBuf::from(&g.nom),
            name: g.nom.clone(),
            load: Load::Ready(Box::new(g.script.clone())),
            options,
            dest,
            built: None,
            mode: Mode::Plans,
            confirm: false,
        });
    }

    /// Un nouveau projet garde le scénario (après la proposition d'enregistrer le projet ouvert) : il prend le format du
    /// dernier « Nouveau projet », le titre et l'auteur du scénario. Aucun plan n'est créé (un plan vide, comme tout projet neuf).
    pub(crate) fn scenario_new_project(&mut self) {
        let Some(w) = self.scenario_window.take() else { return };
        let Load::Ready(script) = &w.load else { return };
        let stem = w.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let title = if script.title.trim().is_empty() { stem } else { script.title.clone() };
        let mut doc = crate::nouveau::build_document(&self.settings.new_project.values, &title);
        if !script.title.trim().is_empty() {
            doc.properties.title = script.title.trim().to_string();
        }
        if !script.author.trim().is_empty() {
            doc.properties.author = script.author.trim().to_string();
        }
        doc.scenario = Some(std::sync::Arc::new(Gardee::new(&w.name, (**script).clone(), Vec::new())));
        self.numerotation_du_projet_neuf(&mut doc);
        if !self.charger_projet(doc) {
            self.status = "Le scénario n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
            return;
        }
        self.welcome = false;
        self.modified = true;
        self.show_pane(Pane::Scenario);
        self.status = format!("Scénario « {} » gardé dans un nouveau projet. « Enregistrer sous » pour le conserver.", w.name);
    }

    /// L'import dans le projet ouvert : le scénario est gardé dans le projet (une étape d'annulation, « Importer un scénario »),
    /// le panneau Scénario s'ouvre. Aucun plan n'est créé.
    pub(crate) fn scenario_keep(&mut self, script: &Script, name: &str) -> bool {
        let g = std::sync::Arc::new(Gardee::new(name, script.clone(), Vec::new()));
        let before = self.history.context.take();
        self.history.context = Some(STEP_NAME.to_string());
        let ok = self.structure_op(|doc, current| {
            doc.scenario = Some(g);
            Some(current)
        });
        self.history.context = before;
        if ok {
            self.show_pane(Pane::Scenario);
            self.status = format!("Scénario « {name} » gardé dans le projet : {} scène(s). Ctrl+Z pour annuler.", script.headings());
        }
        ok
    }

    /// La création des plans dans le projet ouvert : une étape d'annulation, « Créer les plans du scénario ». Les plans créés sont
    /// liés au texte du scénario gardé (s'il est toujours celui d'où ils viennent).
    pub(crate) fn scenario_into_project(&mut self, outline: &Outline, dest: Dest, name: &str, script: &Script) -> bool {
        let before = self.history.context.take();
        self.history.context = Some(PLANS_STEP_NAME.to_string());
        let mut made = None;
        let ok = self.structure_op(|doc, current| {
            let applied = scenarioplans::append_to_project(doc, outline, dest, current)?;
            let first = applied.first;
            if let Some(g) = doc.scenario.clone().filter(|g| g.script == *script) {
                doc.scenario = Some(std::sync::Arc::new(g.avec_liens(applied.liens.clone())));
            }
            made = Some(applied);
            Some(first)
        });
        self.history.context = before;
        if let (true, Some(a)) = (ok, made) {
            self.status = format!(
                "Plans du scénario « {name} » créés : {} séquence(s), {} plan(s), environ {}. Ctrl+Z pour annuler.",
                a.sequences,
                a.plans,
                duration_text(outline.seconds())
            );
        }
        ok
    }

    /// À chaque image d'affichage : la lecture qui avance, la fenêtre.
    pub(crate) fn scenario_window_ui(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.scenario_window.take() else { return };
        // Lecture finie ?
        let mut finished = None;
        if let Load::Reading { progress, result } = &w.load {
            finished = result.lock().unwrap_or_else(|e| e.into_inner()).take();
            if finished.is_none() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
                self.status = format!("Lecture du scénario « {} » : {:.0} %", w.name, f32::from_bits(progress.load(Ordering::Relaxed)) * 100.0);
            }
        }
        match finished {
            Some(Ok(script)) => {
                self.status = format!("Scénario « {} » lu : {} scène(s).", w.name, script.headings());
                w.load = Load::Ready(Box::new(script));
            }
            Some(Err(e)) => {
                self.status = format!("Scénario illisible : {e}");
                w.load = Load::Failed(e);
            }
            None => {}
        }
        // Les choix donnent des séquences et des plans ; on ne les recalcule que s'ils changent.
        if let Load::Ready(script) = &w.load {
            if w.built.as_ref().is_none_or(|(o, _)| *o != w.options) {
                w.built = Some((w.options.clone(), scenarioplans::build(script, &w.options)));
            }
        }

        let mut open = true;
        let mut action = Act::None;
        let title = if w.mode == Mode::Plans { "Créer les plans depuis le scénario" } else { "Importer un scénario" };
        egui::Window::new(title)
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(560.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| action = self.scenario_body(ui, &mut w));
        match action {
            Act::None => {}
            Act::Close => open = false,
            Act::Another => {
                open = false;
                self.open_scenario_window();
            }
            // Garder le scénario dans le projet : s'il en a déjà un, la fenêtre demande d'abord s'il est remplacé.
            Act::Keep | Act::KeepConfirmed => {
                let script = match &w.load {
                    Load::Ready(s) => Some((**s).clone()),
                    _ => None,
                };
                if let Some(script) = script {
                    if self.welcome {
                        // Sans projet ouvert : un nouveau projet le garde (il remplace le projet vierge, après la proposition d'enregistrer).
                        self.scenario_window = Some(w);
                        self.request(Pending::ScenarioNew);
                        return;
                    }
                    if matches!(action, Act::Keep) && self.doc.scenario.is_some() {
                        w.confirm = true;
                    } else {
                        let name = w.name.clone();
                        if self.scenario_keep(&script, &name) {
                            open = false;
                        }
                    }
                }
            }
            Act::Create => {
                *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((w.options.clone(), w.dest));
                let outline = w.built.as_ref().map(|(_, o)| o.clone());
                let script = match &w.load {
                    Load::Ready(s) => Some((**s).clone()),
                    _ => None,
                };
                if let (Some(outline), Some(script)) = (outline, script) {
                    let dest = if w.dest == Dest::NewProject { Dest::End } else { w.dest };
                    let name = w.name.clone();
                    if self.scenario_into_project(&outline, dest, &name, &script) {
                        open = false;
                    }
                }
            }
            Act::Unconfirm => w.confirm = false,
        }
        // (Ouverte d'un autre fichier pendant ce temps : la nouvelle fenêtre est gardée.)
        if open && self.scenario_window.is_none() {
            self.scenario_window = Some(w);
        }
    }

    fn scenario_body(&mut self, ui: &mut egui::Ui, w: &mut ScenarioWindow) -> Act {
        let mut act = Act::None;
        ui.label(RichText::new(&w.name).strong());
        ui.label(RichText::new(w.path.display().to_string()).small().weak());
        ui.separator();
        match &w.load {
            Load::Reading { progress, .. } => {
                ui.label("Lecture du scénario…");
                ui.add(crate::barre::Barre::new(f32::from_bits(progress.load(Ordering::Relaxed))).show_percentage());
                if ui.button("Annuler").clicked() {
                    act = Act::Close;
                }
                return act;
            }
            Load::Failed(e) => {
                crate::theme::avertir(ui, e.as_str());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Choisir un autre fichier…").clicked() {
                        act = Act::Another;
                    }
                    if ui.button("Fermer").clicked() {
                        act = Act::Close;
                    }
                });
                return act;
            }
            Load::Ready(_) => {}
        }
        if w.mode == Mode::Garder {
            return self.scenario_keep_body(ui, w);
        }
        let Load::Ready(script) = &w.load else { return act };
        let Some((_, outline)) = &w.built else { return act };

        // ------------------------------------------------------------------ ce qui a été lu
        let mut head = format!("{} : {} scène(s)", script.format.label(), script.headings());
        if !script.title.is_empty() {
            head = format!("« {} » — {head}", script.title);
        }
        if !script.author.is_empty() {
            head.push_str(&format!(", de {}", script.author));
        }
        ui.label(head);
        ui.label(
            RichText::new(format!(
                "Va créer {} séquence(s) et {} plan(s), environ {}, {} personnage(s) qui parlent.",
                outline.sequences.len(),
                outline.plan_count(),
                duration_text(outline.seconds()),
                outline.characters.len()
            ))
            .strong(),
        );
        ui.separator();

        // ------------------------------------------------------------------ découpage et rythme
        ui.label("Découpage en plans");
        ui.horizontal_wrapped(|ui| {
            for s in Split::ALL {
                ui.radio_value(&mut w.options.split, s, s.label()).on_hover_text(s.help());
            }
        });
        ui.checkbox(&mut w.options.scene_in_notes, "Noter le décor de la scène (« INT. CUISINE – NUIT ») dans les notes de son premier plan");
        egui::CollapsingHeader::new("Rythme (durée estimée des plans)").default_open(false).show(ui, |ui| {
            let r = &mut w.options.rhythm;
            egui::Grid::new("grille_rythme_scenario").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
                ui.label("Répliques");
                ui.add(egui::DragValue::new(&mut r.words_per_second).speed(0.05).range(0.2..=20.0).suffix(" mots par seconde"));
                ui.end_row();
                ui.label("Action");
                ui.add(
                    egui::DragValue::new(&mut r.seconds_per_action_line)
                        .speed(0.05)
                        .range(0.1..=30.0)
                        .suffix(format!(" s par ligne de {} signes", scenarioplans::CHARS_PER_LINE)),
                );
                ui.end_row();
                ui.label("Au plus court");
                ui.add(egui::DragValue::new(&mut r.min_seconds).speed(0.05).range(0.1..=60.0).suffix(" s"));
                ui.end_row();
                ui.label("Au plus long");
                ui.add(egui::DragValue::new(&mut r.max_seconds).speed(0.5).range(1.0..=600.0).suffix(" s"));
                ui.end_row();
            });
            if ui.button("Valeurs d'origine").clicked() {
                *r = Rhythm::default();
            }
            ui.label(RichText::new("Un plan dure la somme de ses répliques (mots ÷ vitesse) et de son action (lignes × durée par ligne), entre le plus court et le plus long.").weak().small());
        });
        ui.separator();

        // ------------------------------------------------------------------ destination
        ui.label("Où poser les plans");
        for d in [Dest::End, Dest::AfterCurrent] {
            ui.radio_value(&mut w.dest, d, d.label());
        }
        ui.label(RichText::new("Une seule étape d'annulation. Une séquence ne se coupe pas : après le plan courant, c'est à la fin de sa séquence.").weak().small());
        if scenarioplans::is_blank(&self.doc) {
            ui.label(RichText::new("Le projet est encore vierge (un seul plan vide) : les plans du scénario le remplacent.").small());
        } else {
            ui.label(RichText::new(format!("Le projet a déjà {} plan(s) : ceux du scénario s'ajoutent, aucun plan existant n'est touché.", self.doc.panels.len())).small());
        }
        if self.doc.scenario.as_ref().is_some_and(|g| !g.liens.is_empty()) {
            ui.label(RichText::new("Des plans ont déjà été créés depuis ce scénario : ils gardent leur place, mais le panneau Scénario ne les relie plus au texte (seuls les nouveaux le sont).").weak().small());
        }
        ui.separator();

        // ------------------------------------------------------------------ séquences
        ui.label(format!("Séquences ({})", outline.sequences.len()));
        let row_h = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical().id_salt("sequences_scenario").max_height(150.0).auto_shrink([false, true]).show_rows(ui, row_h, outline.sequences.len(), |ui, range| {
            for s in &outline.sequences[range] {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(s.name.as_str()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(format!("{} plan(s) · {}", s.plans.len(), duration_text(s.seconds()))).weak().small());
                    });
                });
            }
        });
        if !outline.characters.is_empty() {
            ui.add_space(4.0);
            ui.label("Personnages");
            let mut by_lines: Vec<&crate::scenario::Character> = outline.characters.iter().collect();
            by_lines.sort_by(|a, b| b.lines.cmp(&a.lines));
            let shown: Vec<String> = by_lines.iter().take(24).map(|c| format!("{} ({})", c.name, c.lines)).collect();
            let more = by_lines.len().saturating_sub(24);
            let mut text = shown.join(" · ");
            if more > 0 {
                text.push_str(&format!(" · … et {more} autre(s)"));
            }
            ui.label(RichText::new(text).small());
            ui.label(RichText::new("Notés dans les notes du projet (Réglages du projet), pour les voix provisoires.").weak().small());
        }
        if !script.warnings.is_empty() {
            ui.add_space(4.0);
            egui::CollapsingHeader::new(format!("Remarques ({})", script.warnings.len())).default_open(true).show(ui, |ui| {
                for m in &script.warnings {
                    crate::theme::avertir(ui, format!("• {m}"));
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let can = outline.plan_count() > 0;
            if ui.add_enabled(can, egui::Button::new("Créer les plans")).clicked() {
                act = Act::Create;
            }
            if ui.button("Annuler").clicked() {
                act = Act::Close;
            }
        });
        act
    }

    /// La fenêtre d'import : ce qui a été lu, et « Importer » (garder dans le projet, sans créer de plan).
    fn scenario_keep_body(&mut self, ui: &mut egui::Ui, w: &mut ScenarioWindow) -> Act {
        let mut act = Act::None;
        let Load::Ready(script) = &w.load else { return act };
        let mut head = format!("{} : {} scène(s)", script.format.label(), script.headings());
        if !script.title.is_empty() {
            head = format!("« {} » — {head}", script.title);
        }
        if !script.author.is_empty() {
            head.push_str(&format!(", de {}", script.author));
        }
        ui.label(head);
        let speakers = crate::scenario::characters(&script.elements).len();
        ui.label(format!("{} élément(s) lus, {speakers} personnage(s) qui parlent.", script.elements.len()));
        ui.add_space(4.0);
        ui.label(RichText::new("Le scénario est gardé dans le projet et se lit dans le panneau Scénario. Aucun plan n'est créé : le bouton « Créer les plans depuis le scénario… » de ce panneau le fait, quand tu le décides.").strong());
        if self.welcome {
            ui.label(RichText::new("Un nouveau projet le reçoit (FEANOR propose d'abord d'enregistrer le projet ouvert s'il a changé).").weak().small());
        }
        if !script.warnings.is_empty() {
            ui.add_space(4.0);
            egui::CollapsingHeader::new(format!("Remarques ({})", script.warnings.len())).default_open(true).show(ui, |ui| {
                for m in &script.warnings {
                    crate::theme::avertir(ui, format!("• {m}"));
                }
            });
        }
        ui.separator();
        if w.confirm {
            let nom = self.doc.scenario.as_ref().map(|g| g.nom.clone()).unwrap_or_default();
            crate::theme::avertir(ui, format!("Ce projet a déjà un scénario (« {nom} »). Le remplacer par celui-ci ?"));
            ui.label(RichText::new("L'ancien fichier n'est pas effacé : il reste à côté, dans le dossier du projet (scenario.ancien-….json). Les plans déjà créés depuis l'ancien scénario ne sont pas touchés, mais ne sont plus reliés au texte.").weak().small());
            ui.horizontal(|ui| {
                if ui.button("Remplacer le scénario").clicked() {
                    act = Act::KeepConfirmed;
                }
                if ui.button("Garder l'actuel").clicked() {
                    act = Act::Unconfirm;
                }
            });
            return act;
        }
        ui.horizontal(|ui| {
            if ui.button("Importer").clicked() {
                act = Act::Keep;
            }
            if ui.button("Annuler").clicked() {
                act = Act::Close;
            }
            if ui.button("Autre fichier…").clicked() {
                act = Act::Another;
            }
        });
        act
    }
}

enum Act {
    None,
    Close,
    Another,
    /// Importer : garder dans le projet (demande avant de remplacer un scénario).
    Keep,
    KeepConfirmed,
    Unconfirm,
    /// Créer les plans.
    Create,
}
