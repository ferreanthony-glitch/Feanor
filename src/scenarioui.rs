//! Importer un scénario : la fenêtre (Fichier > Importer > Scénario…, action
//! `fichier.importer_scenario`, donc aussi dans Ctrl+K). Le fichier est choisi, lu sur un autre fil
//! (avec une barre d'avancement : un scénario de 120 pages est lu en une fraction de seconde, un
//! très gros fichier ne bloque pas l'interface), puis la fenêtre montre ce qui va être créé : les
//! séquences et leurs plans, la durée estimée, les personnages, et les choix (découpage en plans,
//! rythme, destination). Rien n'est touché avant « Importer ».
//!
//! Dans le projet ouvert (à la fin, ou après la séquence du plan courant), l'import est **une seule
//! étape d'annulation**, « Importer un scénario ». Dans un nouveau projet, il remplace le projet
//! ouvert après avoir proposé de l'enregistrer. Voir `scenario`, `scenarioplans`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, vec2, Align2, RichText};

use crate::app::{App, Pending};
use crate::scenario::Script;
use crate::scenarioplans::{self, Dest, Options, Outline, Rhythm, Split};

/// Le nom de l'étape d'annulation.
pub const STEP_NAME: &str = "Importer un scénario";

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
}

#[cfg(test)]
impl ScenarioWindow {
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
        self.scenario_window = Some(ScenarioWindow { path, name, load: Load::Reading { progress, result }, options, dest, built: None });
    }

    /// Un nouveau projet fait du scénario (après la proposition d'enregistrer le projet ouvert) :
    /// il prend le format du dernier « Nouveau projet ».
    pub(crate) fn scenario_new_project(&mut self) {
        let Some(w) = self.scenario_window.take() else { return };
        let Load::Ready(script) = &w.load else { return };
        let outline = scenarioplans::build(script, &w.options);
        if outline.plan_count() == 0 {
            self.status = "Rien à importer : le scénario ne donne aucun plan.".into();
            return;
        }
        let stem = w.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let title = if script.title.trim().is_empty() { stem } else { script.title.clone() };
        let mut doc = crate::nouveau::build_document(&self.settings.new_project.values, &title);
        let Some(applied) = scenarioplans::fill_new_project(&mut doc, &outline, &title, &script.author) else { return };
        let (seqs, plans, seconds) = (applied.sequences, applied.plans, outline.seconds());
        if !self.charger_projet(doc) {
            self.status = "Le scénario n'a pas pu s'ouvrir dans un nouvel onglet : un travail attend l'onglet affiché.".into();
            return;
        }
        self.welcome = false;
        self.modified = true;
        self.status = format!("Scénario « {} » importé dans un nouveau projet : {seqs} séquence(s), {plans} plan(s), environ {}. « Enregistrer sous » pour le garder.", w.name, duration_text(seconds));
    }

    /// L'import dans le projet ouvert : une étape d'annulation, « Importer un scénario ».
    pub(crate) fn scenario_into_project(&mut self, outline: &Outline, dest: Dest, name: &str) -> bool {
        let before = self.history.context.take();
        self.history.context = Some(STEP_NAME.to_string());
        let mut made = None;
        let ok = self.structure_op(|doc, current| {
            let applied = scenarioplans::append_to_project(doc, outline, dest, current)?;
            made = Some(applied);
            Some(applied.first)
        });
        self.history.context = before;
        if let (true, Some(a)) = (ok, made) {
            self.status = format!(
                "Scénario « {name} » importé : {} séquence(s), {} plan(s), environ {}. Ctrl+Z pour annuler.",
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
        egui::Window::new("Importer un scénario")
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
            Act::Import => {
                *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((w.options.clone(), w.dest));
                let outline = w.built.as_ref().map(|(_, o)| o.clone());
                if let Some(outline) = outline {
                    match w.dest {
                        Dest::NewProject => {
                            // Le projet ouvert sera remplacé : la fenêtre reste jusque-là.
                            self.scenario_window = Some(w);
                            self.request(Pending::ScenarioNew);
                            return;
                        }
                        dest => {
                            let name = w.name.clone();
                            if self.scenario_into_project(&outline, dest, &name) {
                                open = false;
                            }
                        }
                    }
                }
            }
        }
        // (Ouverte d'un autre fichier pendant ce temps : la nouvelle fenêtre est gardée.)
        if open && self.scenario_window.is_none() {
            self.scenario_window = Some(w);
        }
    }

    fn scenario_body(&mut self, ui: &mut egui::Ui, w: &mut ScenarioWindow) -> Act {
        let warn = crate::theme::avertissement();
        let mut act = Act::None;
        ui.label(RichText::new(&w.name).strong());
        ui.label(RichText::new(w.path.display().to_string()).small().weak());
        ui.separator();
        match &w.load {
            Load::Reading { progress, .. } => {
                ui.label("Lecture du scénario…");
                ui.add(egui::ProgressBar::new(f32::from_bits(progress.load(Ordering::Relaxed))).show_percentage());
                if ui.button("Annuler").clicked() {
                    act = Act::Close;
                }
                return act;
            }
            Load::Failed(e) => {
                ui.colored_label(warn, e.as_str());
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
        ui.label("Destination");
        let values = &self.settings.new_project.values;
        for d in Dest::ALL {
            ui.radio_value(&mut w.dest, d, d.label());
        }
        match w.dest {
            Dest::NewProject => {
                ui.label(RichText::new(format!("Format du dernier « Nouveau projet » : {}×{} à {} i/s. Le projet ouvert est remplacé (SHOT propose de l'enregistrer s'il a changé).", values.width, values.height, values.fps)).weak().small());
            }
            Dest::End | Dest::AfterCurrent => {
                ui.label(RichText::new("Une seule étape d'annulation. Une séquence ne se coupe pas : après le plan courant, c'est à la fin de sa séquence.").weak().small());
            }
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
                    ui.colored_label(warn, format!("• {m}"));
                }
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            let can = outline.plan_count() > 0;
            if crate::theme::bouton_principal(ui, can, "Importer").clicked() {
                act = Act::Import;
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
    Import,
}
