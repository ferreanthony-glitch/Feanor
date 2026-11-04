//! Relecture par le réalisateur, côté interface (voir `relecture.rs` pour le principe) :
//!
//! - **Fichier > Exporter > Pour relecture** (`fichier.exporter_relecture`, aussi dans la liste des envois) : la
//!   fenêtre des choix, puis la tâche (`relecturejob.rs`) avec sa barre et son bouton « Arrêter » ;
//! - **Fichier > Importer > Notes de relecture** (`fichier.importer_notes`, ou le fichier `.json` lâché sur la
//!   fenêtre) : le fichier est lu sur un autre fil, la fenêtre dit ce qui sera posé, puis une étape d'annulation
//!   nommée pose les commentaires ;
//! - le dessin du relecteur, montré par-dessus l'image quand son commentaire est choisi.
//!
//! Règle « sans latence » : l'interface ne calcule aucune image de l'export (le projet est copié au départ, les images se
//! calculent sur d'autres fils : `exportrender.rs`) ; la page, le PDF, la lecture et la validation du fichier de notes,
//! le décodage des dessins se font à côté. Le projet reste libre, et l'export continue si l'on change d'onglet.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Align2, Color32, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::commentaires::Annotation;
use crate::envois::Etat;
use crate::export::Progress;
use crate::relecture::{self, Notes, Options, Placement};
use crate::relecturejob::{RelectureJob, Report};
use crate::theme;

/// Ce que la page montre : tout le film, la séquence courante, les plans choisis, ou ceux qui ont changé depuis un envoi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Etendue {
    Film,
    Sequence,
    Choisis,
    Modifies,
}

/// La fenêtre « Exporter pour relecture ».
pub(crate) struct FenetreExport {
    title: String,
    scope: Etendue,
    /// L'envoi dont c'est la relecture (son nom est écrit dans la page) et auquel « modifiés » se compare.
    envoi: Option<u64>,
    opts: Options,
    job: Option<RelectureJob>,
    result: Option<Result<Report, String>>,
}

enum EtatImport {
    Lecture(Receiver<Result<Notes, String>>),
    Pret(Box<Notes>),
    Fait(String),
    Echec(String),
}

/// La fenêtre « Notes de relecture ».
pub(crate) struct FenetreImport {
    path: PathBuf,
    state: EtatImport,
    /// Poser quand même des notes faites pour un autre projet.
    autre_projet_ok: bool,
    /// Poser les notes sans plan sur le plan courant.
    sans_plan_ici: bool,
}

/// La texture du dessin du relecteur montré sur l'image.
struct Calque {
    comment: u64,
    /// Le dessin (identité de l'`Arc`) : un autre dessin refait la texture.
    cle: usize,
    texture: Option<egui::TextureHandle>,
    rx: Option<Receiver<Option<egui::ColorImage>>>,
}

/// L'état de la relecture dans `App` (un seul champ).
pub(crate) struct RelectureUi {
    pub(crate) export: Option<FenetreExport>,
    pub(crate) import: Option<FenetreImport>,
    /// Montrer le dessin du relecteur du commentaire choisi par-dessus l'image.
    pub(crate) montrer_dessin: bool,
    calque: Option<Calque>,
}

impl RelectureUi {
    /// Le projet ne peut pas changer d'onglet (`onglets.rs`, `onglet_occupe`) : l'export copie encore le projet, ou des
    /// notes se lisent. (Une page qui se calcule, elle, continue dans l'onglet caché : `App::relecture_hors_ecran`.)
    pub(crate) fn occupe(&self) -> bool {
        self.rendering() || self.import.as_ref().is_some_and(|i| matches!(i.state, EtatImport::Lecture(_)))
    }

    /// Une page de relecture est en train de s'écrire (dans cet onglet).
    pub(crate) fn export_en_cours(&self) -> bool {
        self.busy()
    }
}

impl Default for RelectureUi {
    fn default() -> Self {
        RelectureUi { export: None, import: None, montrer_dessin: true, calque: None }
    }
}

impl RelectureUi {
    /// SHOT calcule encore les images de l'export : le projet ne doit pas changer.
    pub(crate) fn rendering(&self) -> bool {
        self.export.as_ref().and_then(|w| w.job.as_ref()).is_some_and(RelectureJob::rendering)
    }

    fn busy(&self) -> bool {
        self.export.as_ref().and_then(|w| w.job.as_ref()).is_some_and(RelectureJob::running)
    }

    /// Le dessin du relecteur est prêt à être montré (essais).
    #[cfg(test)]
    pub(crate) fn has_drawing_texture(&self) -> bool {
        self.calque.as_ref().is_some_and(|c| c.texture.is_some())
    }
}

impl FenetreImport {
    /// Le fichier est lu et vérifié (essais).
    #[cfg(test)]
    pub(crate) fn is_ready(&self) -> bool {
        matches!(self.state, EtatImport::Pret(_))
    }

    /// Pourquoi le fichier n'a pas pu être lu (essais).
    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&str> {
        match &self.state {
            EtatImport::Echec(e) => Some(e),
            _ => None,
        }
    }
}

/// Un chemin en court (« …\dossier\nom »), le chemin complet en bulle.
fn short_path(p: &Path) -> String {
    let parts: Vec<String> = p.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    if parts.len() <= 3 {
        p.display().to_string()
    } else {
        format!("…\\{}", parts[parts.len() - 2..].join("\\"))
    }
}

/// Le texte d'une note en une ligne pour une liste.
fn one_line(text: &str, width: usize) -> String {
    crate::commentaires::first_line(text, width)
}

impl App {
    // ------------------------------------------------------------------ commandes

    /// Les commandes de la relecture ; vrai si `action` en était une.
    pub(crate) fn relecture_action(&mut self, action: &str) -> bool {
        match action {
            "fichier.exporter_relecture" => self.open_relecture_export(None),
            "fichier.importer_notes" => self.choose_review_notes(),
            _ => return false,
        }
        true
    }

    /// Ouvre la fenêtre d'export ; `envoi` : la relecture d'un envoi de la liste (seuls les plans modifiés depuis
    /// lui sont proposés d'emblée).
    pub(crate) fn open_relecture_export(&mut self, envoi: Option<u64>) {
        self.end_stroke();
        self.playing = false;
        if self.relecture.busy() {
            return;
        }
        let title = match self.doc.properties.title.trim() {
            "" => self.base_name_or("FEANOR"),
            t => t.to_string(),
        };
        let scope = if envoi.is_some() { Etendue::Modifies } else { Etendue::Film };
        self.relecture.export = Some(FenetreExport { title, scope, envoi, opts: Options::default(), job: None, result: None });
    }

    /// Choisit un fichier de notes (`.json`) et l'ouvre.
    pub(crate) fn choose_review_notes(&mut self) {
        let Some(path) = rfd::FileDialog::new().set_title("Notes de relecture").add_filter("Notes de relecture (.json)", &["json"]).pick_file() else { return };
        self.relecture_open_notes(&path);
    }

    /// Ouvre un fichier de notes : lu et vérifié sur un autre fil, jamais sur celui de l'interface.
    pub(crate) fn relecture_open_notes(&mut self, path: &Path) {
        let (tx, rx) = channel();
        let p = path.to_path_buf();
        std::thread::spawn(move || {
            let _ = tx.send(relecture::read_notes_file(&p));
        });
        self.relecture.import = Some(FenetreImport { path: path.to_path_buf(), state: EtatImport::Lecture(rx), autre_projet_ok: false, sans_plan_ici: false });
    }

    /// Un fichier lâché sur la fenêtre : vrai si c'est un `.json` (SHOT l'ouvre comme des notes de relecture et
    /// dit si ce n'en est pas).
    pub(crate) fn import_dropped_notes(&mut self, path: &Path) -> bool {
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            return false;
        }
        self.relecture_open_notes(path);
        true
    }

    /// L'envoi dont la page de relecture rend compte des notes : celui d'avant `envoi` (le dernier marqué avant lui) ;
    /// sans envoi choisi, le dernier envoi du projet. Rien s'il n'y en a pas.
    pub(crate) fn envoi_precedent(&self, envoi: Option<u64>) -> Option<u64> {
        match envoi {
            Some(m) => self.doc.envois.list.iter().map(|e| e.id).filter(|id| *id < m).max(),
            None => self.doc.envois.dernier().map(|e| e.id),
        }
    }

    /// Les plans (rangs) qui ont changé depuis l'envoi `envoi`, d'après le dernier relevé des plans.
    pub(crate) fn plans_modifies_depuis(&self, envoi: u64) -> Vec<usize> {
        let Some(e) = self.doc.envois.trouve(envoi) else { return Vec::new() };
        self.doc
            .panels
            .iter()
            .enumerate()
            .filter(|(_, p)| match self.envois_ui.empreinte(p.id) {
                Some(d) => crate::envois::etat_du_plan(e, p.id, d) != Etat::Inchange,
                None => false,
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Les plans (rangs) d'une étendue.
    pub(crate) fn plans_de(&self, scope: Etendue, envoi: Option<u64>) -> Vec<usize> {
        let n = self.doc.panels.len();
        match scope {
            Etendue::Film => (0..n).collect(),
            Etendue::Sequence => {
                let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
                (a..b).collect()
            }
            Etendue::Choisis => self.doc.panels.iter().enumerate().filter(|(_, p)| self.board_sel.contains(&p.id)).map(|(i, _)| i).collect(),
            Etendue::Modifies => envoi.map(|e| self.plans_modifies_depuis(e)).unwrap_or_default(),
        }
    }

    // ------------------------------------------------------------------ les fenêtres

    /// À chaque image d'affichage : les deux fenêtres, et l'export qui avance.
    pub(crate) fn relecture_windows(&mut self, ctx: &egui::Context) {
        self.relecture_export_window(ctx);
        self.relecture_import_window(ctx);
    }

    /// Fait avancer l'export de la fenêtre `w` (le projet affiché n'est lu que pour le copier au départ) ; `ctx` :
    /// l'écran à réveiller (rien pour un onglet caché).
    fn relecture_export_avance(&mut self, w: &mut FenetreExport, ctx: Option<&egui::Context>) {
        if let Some(job) = &mut w.job {
            job.step(&self.doc, Duration::from_millis(8));
            let copie = job.rendering();
            match job.progress() {
                Progress::Running(done, total) => {
                    if let Some(ctx) = ctx {
                        if copie {
                            ctx.request_repaint();
                        } else {
                            ctx.request_repaint_after(Duration::from_millis(100));
                        }
                    }
                    self.status = format!("Export pour relecture : image {done} sur {total}");
                }
                Progress::Finishing => {
                    if let Some(ctx) = ctx {
                        ctx.request_repaint_after(Duration::from_millis(60));
                    }
                }
                Progress::Done => {}
            }
        }
        if w.job.as_ref().is_some_and(|j| !j.running()) {
            let job = w.job.take().expect("export");
            let result = job.report().cloned().unwrap_or_else(|| Err("Export annulé.".into()));
            self.status = match &result {
                Ok(r) => format!("Page de relecture exportée : {}", r.html.display()),
                Err(e) => format!("Export pour relecture : {e}"),
            };
            w.result = Some(result);
        }
    }

    /// L'export de relecture d'un onglet caché avance (la fenêtre est celle de cet onglet, échangée le temps d'un appel,
    /// `onglets.rs`) : sa fin se dit dans la barre d'état, le compte rendu attend le retour dans l'onglet.
    pub(crate) fn relecture_hors_ecran(&mut self) {
        let Some(mut w) = self.relecture.export.take() else { return };
        self.relecture_export_avance(&mut w, None);
        self.relecture.export = Some(w);
    }

    fn relecture_export_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.relecture.export.take() else { return };
        self.relecture_export_avance(&mut w, Some(ctx));
        let mut open = true;
        let mut start = false;
        egui::Window::new("Exporter pour relecture")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(500.0)
            .default_pos(ctx.screen_rect().center() - vec2(250.0, 230.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| start = self.relecture_export_body(ui, &mut w));
        if start {
            self.relecture_start_export(&mut w);
        }
        if !open {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
        }
        if open || w.job.as_ref().is_some_and(RelectureJob::running) {
            self.relecture.export = Some(w);
        }
    }

    fn relecture_export_body(&mut self, ui: &mut egui::Ui, w: &mut FenetreExport) -> bool {
        let busy = w.job.is_some();
        ui.label(
            RichText::new(
                "Une page à envoyer à quelqu'un qui n'a pas FEANOR : il lit le storyboard plan par plan, écrit ses notes et dessine dessus, puis vous renvoie un petit fichier. \
                 Fichier > Importer > Notes de relecture les pose en commentaires sur les bons plans.",
            )
            .small()
            .color(theme::texte_doux()),
        );
        ui.separator();
        ui.add_enabled_ui(!busy, |ui| {
            egui::Grid::new("relecture_grille").num_columns(2).spacing([10.0, 7.0]).show(ui, |ui| {
                ui.label("Titre");
                ui.add(egui::TextEdit::singleline(&mut w.title).desired_width(280.0));
                ui.end_row();

                ui.label("Envoi");
                let nom = |id: Option<u64>| id.and_then(|i| self.doc.envois.trouve(i)).map_or("Aucun".to_string(), |e| e.libelle());
                egui::ComboBox::from_id_salt("relecture_envoi").selected_text(nom(w.envoi)).width(280.0).show_ui(ui, |ui| {
                    if ui.selectable_label(w.envoi.is_none(), "Aucun").clicked() {
                        w.envoi = None;
                        if w.scope == Etendue::Modifies {
                            w.scope = Etendue::Film;
                        }
                    }
                    let mut list: Vec<(u64, String)> = self.doc.envois.list.iter().map(|e| (e.id, e.libelle())).collect();
                    list.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
                    for (id, label) in list {
                        if ui.selectable_label(w.envoi == Some(id), label).clicked() {
                            w.envoi = Some(id);
                        }
                    }
                });
                ui.end_row();

                ui.label("Plans");
                ui.vertical(|ui| {
                    let n = self.doc.panels.len();
                    let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
                    ui.radio_value(&mut w.scope, Etendue::Film, format!("Tout le film ({})", relecture::plural(n, "plan", "plans")));
                    ui.radio_value(&mut w.scope, Etendue::Sequence, format!("La séquence courante ({})", relecture::plural(b - a, "plan", "plans")));
                    let choisis = self.plans_de(Etendue::Choisis, None).len();
                    ui.add_enabled_ui(choisis > 0, |ui| {
                        ui.radio_value(&mut w.scope, Etendue::Choisis, format!("Les plans choisis ({choisis})"));
                    });
                    let modifies = w.envoi.map(|e| self.plans_modifies_depuis(e).len());
                    ui.add_enabled_ui(modifies.is_some(), |ui| {
                        let label = match modifies {
                            Some(m) => format!("Seulement les plans modifiés depuis l'envoi ({m})"),
                            None => "Seulement les plans modifiés depuis un envoi".to_string(),
                        };
                        ui.radio_value(&mut w.scope, Etendue::Modifies, label);
                    });
                });
                ui.end_row();

                ui.label("Dans la page");
                ui.vertical(|ui| {
                    ui.checkbox(&mut w.opts.playback, "Lecture animée de chaque plan (page plus lourde)");
                    ui.checkbox(&mut w.opts.comments, "Les commentaires déjà posés, en lecture seule");
                    ui.checkbox(&mut w.opts.pdf, "Aussi un PDF à imprimer, avec des cadres pour écrire à la main");
                });
                ui.end_row();
            });
        });
        ui.separator();
        // Ce qui partirait.
        let panels = self.plans_de(w.scope, w.envoi);
        let boards: usize = panels.iter().map(|&i| crate::board::plan_boards(&self.doc, i).len()).sum();
        if panels.is_empty() {
            let why = if w.scope == Etendue::Modifies && self.envois_ui.empreintes_vides() { "Le relevé des plans est en cours : réessayez dans un instant." } else { "Aucun plan dans cette étendue." };
            ui.colored_label(theme::avertissement(), why);
        } else {
            ui.label(format!("{}, {} de storyboard.", relecture::plural(panels.len(), "plan", "plans"), relecture::plural(boards, "image", "images")));
        }
        if !self.doc.numbering_locked {
            ui.label(RichText::new("La numérotation des plans est suivie : si le film change avant le retour des notes, elles sont retrouvées par l'identifiant du plan, pas par son numéro.").small().color(theme::texte_doux()));
        }
        ui.add_space(4.0);
        let mut start = false;
        match &mut w.job {
            Some(job) => {
                match job.progress() {
                    Progress::Running(done, total) => {
                        ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).text(format!("image {done} / {total}")));
                        if let Some(rest) = job.remaining() {
                            let s = rest.as_secs();
                            ui.label(RichText::new(format!("reste environ {}:{:02}", s / 60, s % 60)).small().weak());
                        }
                    }
                    _ => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Écriture de la page…");
                        });
                    }
                }
                if ui.button("Arrêter").on_hover_text("Arrête l'export et efface ce qu'il a commencé d'écrire.").clicked() {
                    job.cancel();
                }
            }
            None => {
                start = ui.add_enabled(!panels.is_empty() && !w.title.trim().is_empty(), egui::Button::new(format!("{}  Exporter…", ph::EYEGLASSES))).clicked();
            }
        }
        match &w.result {
            Some(Ok(r)) => {
                ui.separator();
                ui.label(RichText::new(format!("Terminé : {}, {}.", relecture::plural(r.plans, "plan", "plans"), relecture::plural(r.stills, "image", "images"))).strong().color(theme::succes()));
                let mo = r.bytes as f64 / 1_048_576.0;
                ui.label(RichText::new(format!("{} ({mo:.1} Mo)", short_path(&r.html))).small()).on_hover_text(r.html.display().to_string());
                if let Some(p) = &r.pdf {
                    ui.label(RichText::new(short_path(p)).small()).on_hover_text(p.display().to_string());
                }
                if mo > 20.0 {
                    ui.colored_label(theme::avertissement(), "Cette page est lourde pour un e-mail : envoyez-la par un lien de partage, ou refaites-la sans la lecture animée.");
                }
                ui.label(RichText::new("Envoyez le fichier HTML : il s'ouvre dans n'importe quel navigateur, sur ordinateur ou sur téléphone, sans internet.").small().weak());
                if ui.button("Montrer dans l'explorateur").clicked() {
                    crate::exportui::reveal_in_explorer(&r.html);
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(theme::avertissement(), e);
            }
            None => {}
        }
        start
    }

    fn relecture_start_export(&mut self, w: &mut FenetreExport) {
        let panels = self.plans_de(w.scope, w.envoi);
        if panels.is_empty() {
            w.result = Some(Err("Aucun plan à exporter.".into()));
            return;
        }
        let stem = crate::editorial::file_safe(&w.title);
        let Some(mut path) = self.dialogue_enregistrer("relecture", "Exporter pour relecture", ("Page HTML", &["html"]), &format!("{stem}_relecture.html")) else { return };
        if path.extension().is_none() {
            path.set_extension("html");
        }
        self.playing = false;
        let project = match self.doc.properties.title.trim() {
            "" => self.base_name_or(&w.title),
            t => t.to_string(),
        };
        let meta = relecture::Meta {
            title: w.title.trim().to_string(),
            project,
            paquet: relecture::random_hex(12),
            envoi: w.envoi.and_then(|i| self.doc.envois.trouve(i)).map(|e| e.nom.clone()),
            now: crate::recovery::now_secs(),
            nonce: relecture::random_hex(24),
            file_stem: format!("notes-{stem}"),
            send_id: w.envoi.filter(|e| self.doc.envois.trouve(*e).is_some()),
            prior_send: self.envoi_precedent(w.envoi),
            changed: w.envoi.map(|e| self.plans_modifies_depuis(e).into_iter().filter_map(|i| self.doc.panels.get(i).map(|p| p.id)).collect()).unwrap_or_default(),
        };
        w.result = None;
        match RelectureJob::start(&self.doc, &panels, &w.opts, meta, &path) {
            Ok(job) => w.job = Some(job.with_exact(self.rejeu_idle.clone())),
            Err(e) => w.result = Some(Err(e)),
        }
    }

    fn relecture_import_window(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.relecture.import.take() else { return };
        if let EtatImport::Lecture(rx) = &w.state {
            match rx.try_recv() {
                Ok(Ok(notes)) => w.state = EtatImport::Pret(Box::new(notes)),
                Ok(Err(e)) => w.state = EtatImport::Echec(e),
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => w.state = EtatImport::Echec("lecture interrompue".into()),
            }
        }
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Notes de relecture").open(&mut open).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(460.0);
            let file = w.path.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
            match &w.state {
                EtatImport::Lecture(_) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(format!("Lecture de « {file} »…"));
                    });
                }
                EtatImport::Echec(e) => {
                    ui.label(RichText::new(&file).small().color(theme::texte_doux()));
                    ui.colored_label(theme::erreur(), e);
                }
                EtatImport::Fait(message) => {
                    ui.label(RichText::new(message).strong().color(theme::succes()));
                    ui.label(RichText::new("Les notes sont des commentaires de la timeline (panneau Commentaires, fanions jaunes). Ctrl+Z les retire d'un coup.").small().color(theme::texte_doux()));
                }
                EtatImport::Pret(notes) => {
                    let places = relecture::place(&self.doc, notes);
                    let loose = w.sans_plan_ici.then_some(self.cur_panel);
                    let sum = relecture::preview(&self.doc, notes, &places, loose);
                    let name = self.doc.properties.title.trim().to_string();
                    let here = if name.is_empty() { self.base_name_or("") } else { name };
                    let other = !notes.project.is_empty() && !here.is_empty() && !notes.project.eq_ignore_ascii_case(&here);
                    ui.label(RichText::new(format!("{} — {}", notes.reviewer, if notes.project.is_empty() { "projet sans nom".to_string() } else { format!("« {} »", notes.project) })).strong());
                    if notes.created > 0 {
                        ui.label(RichText::new(format!("Notes du {}", crate::envois::date_courte(notes.created))).small().color(theme::texte_doux()));
                    }
                    ui.add_space(4.0);
                    let drawn = notes.notes.iter().filter(|n| n.drawing.is_some()).count();
                    ui.label(format!("{}, dont {} avec un dessin.", relecture::plural(notes.notes.len(), "note", "notes"), drawn));
                    ui.label(format!("{} à poser sur {}.", relecture::plural(sum.imported, "note", "notes"), relecture::plural(sum.plans, "plan", "plans")));
                    if sum.by_number > 0 {
                        ui.label(RichText::new(format!("{} retrouvée{} par son numéro de plan (l'identifiant n'existe plus).", relecture::plural(sum.by_number, "note", "notes"), if sum.by_number > 1 { "s" } else { "" })).small().color(theme::texte_doux()));
                    }
                    if sum.replies > 0 {
                        ui.label(RichText::new(format!("{} aux notes de l'envoi d'avant : elles rejoignent le fil de leur note.", relecture::plural(sum.replies, "réponse", "réponses"))).small().color(theme::texte_doux()));
                    }
                    if sum.already > 0 {
                        ui.label(RichText::new(format!("{} déjà importée{} : pas de doublon.", relecture::plural(sum.already, "note", "notes"), if sum.already > 1 { "s" } else { "" })).small().color(theme::texte_doux()));
                    }
                    if notes.ignored > 0 || notes.drawings_refused > 0 {
                        ui.colored_label(theme::avertissement(), format!("{} ignorée(s), {} refusé(s) dans le fichier.", notes.ignored, relecture::plural(notes.drawings_refused, "dessin", "dessins")));
                    }
                    if !sum.unmatched.is_empty() {
                        ui.colored_label(theme::avertissement(), format!("{} sans plan (plan supprimé ou inconnu) :", relecture::plural(sum.unmatched.len(), "note", "notes")));
                        egui::ScrollArea::vertical().max_height(110.0).id_salt("relecture_sans_plan").show(ui, |ui| {
                            for &k in &sum.unmatched {
                                let n = &notes.notes[k];
                                let who = if !n.title.is_empty() { n.title.clone() } else if !n.number.is_empty() { format!("Plan {}", n.number) } else { "?".into() };
                                ui.label(RichText::new(format!("• {who} : {}", one_line(&n.text, 70))).small());
                            }
                        });
                        ui.checkbox(&mut w.sans_plan_ici, "Les poser sur le plan courant, avec leur provenance");
                    }
                    if other {
                        ui.separator();
                        ui.colored_label(theme::avertissement(), format!("Ces notes viennent du projet « {} », pas de « {here} ». Les plans sont cherchés par leur identifiant : sur un autre projet, elles tomberaient au mauvais endroit.", notes.project));
                        ui.checkbox(&mut w.autre_projet_ok, "Importer quand même");
                    }
                    ui.add_space(6.0);
                    let ready = sum.new_things() > 0 && (!other || w.autre_projet_ok);
                    ui.horizontal(|ui| {
                        apply = ui.add_enabled(ready, egui::Button::new(format!("{}  Importer", ph::CHATS))).clicked();
                        if sum.new_things() == 0 && sum.already > 0 {
                            ui.label(RichText::new("Rien de nouveau.").color(theme::texte_doux()));
                        }
                    });
                }
            }
        });
        if apply {
            if let EtatImport::Pret(notes) = &w.state {
                let message = self.relecture_apply_notes(notes, w.sans_plan_ici);
                w.state = EtatImport::Fait(message);
            }
        }
        if open {
            self.relecture.import = Some(w);
        }
    }

    /// Pose les notes : **une** étape d'annulation nommée, puis les commentaires. Rend le message du compte rendu.
    pub(crate) fn relecture_apply_notes(&mut self, notes: &Notes, loose_here: bool) -> String {
        self.end_stroke();
        self.playing = false;
        let places: Vec<Placement> = relecture::place(&self.doc, notes);
        let loose = loose_here.then_some(self.cur_panel);
        if relecture::preview(&self.doc, notes, &places, loose).new_things() == 0 {
            return "Rien à importer : toutes ces notes sont déjà dans le projet.".into();
        }
        self.comment_step("Importer les notes de relecture");
        let sum = relecture::apply(&mut self.doc, notes, &places, loose, crate::recovery::now_secs());
        self.modified = true;
        // Le premier commentaire posé est choisi : le panneau Commentaires l'a sous les yeux.
        if let Some(id) = self.doc.comments.iter().rev().find(|c| c.note.is_some()).map(|c| c.id) {
            self.comment_ui.sel = Some(id);
            self.comment_ui.scroll_to = Some(id);
        }
        self.show_pane(crate::dock::Pane::Comments);
        let message = sum.message();
        self.status = format!("Notes de relecture : {message}");
        message
    }

    // ------------------------------------------------------------------ le dessin du relecteur

    /// Dans l'éditeur d'un commentaire qui a un dessin : l'interrupteur « Montrer » et « Retirer ».
    pub(crate) fn relecture_drawing_row(&mut self, ui: &mut egui::Ui, id: u64) {
        ui.horizontal(|ui| {
            let mut on = self.relecture.montrer_dessin;
            bulles::interrupteur(
                ui,
                &mut on,
                ph::PENCIL_SIMPLE_LINE,
                "Montrer le dessin du relecteur",
                "Le dessin fait par-dessus l'image, montré sur le dessin quand ce commentaire est choisi. Éteint, il se cache (il reste dans le commentaire).",
                "",
            );
            self.relecture.montrer_dessin = on;
            ui.label(RichText::new("Dessin du relecteur").small().color(theme::texte_doux()));
            if bulles::icone(ui, ph::TRASH, "Retirer le dessin", "Enlève le dessin de ce commentaire (Ctrl+Z le rétablit).", "").clicked() {
                self.comment_step("Retirer le dessin du commentaire");
                if let Some(c) = self.doc.comment_mut(id) {
                    c.annotation = None;
                }
            }
        });
    }

    /// Le dessin du relecteur par-dessus l'image, quand son commentaire est choisi et que la tête de lecture est sur
    /// l'image de storyboard qu'il vise. La texture se décode sur un autre fil.
    pub(crate) fn paint_review_drawing(&mut self, ctx: &egui::Context, painter: &egui::Painter) {
        let wanted = self.comment_ui.sel.and_then(|id| self.doc.comment(id)).filter(|_| self.relecture.montrer_dessin && !self.in_card).and_then(|c| {
            let art = c.annotation.clone()?;
            let (panel, frame) = self.doc.comment_spot(c)?;
            let (_, start, end) = crate::board::plan_boards(&self.doc, panel).into_iter().rev().find(|b| b.1 <= frame)?;
            (panel == self.cur_panel && self.cur_frame >= start && self.cur_frame < end).then_some((c.id, art))
        });
        let Some((id, art)) = wanted else {
            self.relecture.calque = None;
            return;
        };
        let cle = std::sync::Arc::as_ptr(&art.0) as usize;
        if self.relecture.calque.as_ref().is_none_or(|c| c.comment != id || c.cle != cle) {
            self.relecture.calque = Some(Calque { comment: id, cle, texture: None, rx: Some(decode_in_background(art))});
        }
        let Some(calque) = self.relecture.calque.as_mut() else { return };
        if let Some(rx) = &calque.rx {
            match rx.try_recv() {
                Ok(Some(image)) => {
                    calque.texture = Some(ctx.load_texture("dessin_relecteur", image, egui::TextureOptions::LINEAR));
                    calque.rx = None;
                }
                Ok(None) | Err(TryRecvError::Disconnected) => calque.rx = None,
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(40)),
            }
        }
        let Some(texture) = calque.texture.clone() else { return };
        let map = self.doc.panel_camera(self.cur_panel, self.cur_frame).frame_map(self.doc.frame);
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        let [fw, fh] = self.doc.frame.map(|v| v as f32);
        let corners = [(0.0, 0.0), (fw, 0.0), (fw, fh), (0.0, fh)].map(|(u, v)| map.at(u, v).map(|p| self.view.to_screen(rect, size, p)));
        if corners.iter().any(Option::is_none) {
            return;
        }
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for (p, uv) in corners.into_iter().flatten().zip([pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)]) {
            mesh.vertices.push(egui::epaint::Vertex { pos: p, uv, color: Color32::WHITE });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// Décode un PNG de dessin sur un fil à part ; `None` s'il est illisible.
fn decode_in_background(art: Annotation) -> Receiver<Option<egui::ColorImage>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        // (Même sous limites que pour l'import : le dessin vient d'un fichier de projet, qui peut avoir été abîmé.)
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(&art.0[..]), image::ImageFormat::Png);
        reader.limits(limits);
        let image = reader.decode().ok().map(|i| {
            let rgba = i.to_rgba8();
            egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw())
        });
        let _ = tx.send(image);
    });
    rx
}
