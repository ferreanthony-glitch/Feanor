//! Le journal des différences dans l'interface (`journal.rs` pour le modèle et les formats, `journaljob.rs` pour la tâche) : une petite
//! fenêtre avec les deux choix d'envoi (« De » et « À » : un autre envoi, ou l'état actuel), les comptes, la liste des plans qui ont
//! changé et « Exporter… » (PDF et / ou page HTML).
//!
//! # Zéro latence
//!
//! Rien de lourd ici. La comparaison des deux listes de plans (des nombres) se refait seulement quand quelque chose a changé (clé du
//! cache). « Exporter » prépare une `Demande` (quelques copies de plans, les tuiles sont partagées) puis un fil calcule les images et
//! écrit les fichiers, avec une barre d'avancement et « Arrêter » qui ne laisse aucun fichier (`journaljob.rs`).
//!
//! Entrées : l'action `envoi.journal` (Ctrl+K, Storyboard > Envois, barre du montage > Plus…, famille « Images & PDF » de la fenêtre
//! Exporter), l'icône de chaque ligne de la fenêtre « Envois » (depuis cet envoi jusqu'à aujourd'hui) et, dans le paquet de livraison,
//! la case « Journal des différences » (décochée d'origine).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::comparer;
use crate::envois::{Envoi, Envois};
use crate::envoisui::Releve;
use crate::journal::{self, Diff, Genre, Journal, PlanVu};
use crate::journaljob::{Cote, Demande, JournalJob, Rapport};
use crate::theme;

/// Ce qu'on a calculé pour la fenêtre, et pour quelle situation (si la clé change, on recalcule).
type Cle = (Option<u64>, Option<u64>, u64, Option<Releve>, usize, usize);

pub(crate) struct JournalFenetre {
    /// Le premier côté (un envoi) et le second (`None` : l'état actuel).
    pub(crate) a: Option<u64>,
    pub(crate) b: Option<u64>,
    pub(crate) pdf: bool,
    pub(crate) html: bool,
    pub(crate) job: Option<JournalJob>,
    pub(crate) resultat: Option<Result<Rapport, String>>,
    /// Le projet (`App::doc_epoch`) pour lequel la fenêtre a été ouverte : elle se ferme si l'on change de projet (hors tâche en cours).
    epoch: u64,
    cache: Option<(Cle, Result<Journal, String>)>,
}

impl JournalFenetre {
    pub(crate) fn busy(&self) -> bool {
        self.job.as_ref().is_some_and(JournalJob::running)
    }
}

/// Les deux côtés proposés d'abord : le dernier envoi et l'état actuel s'il y a des plans modifiés depuis ; sinon l'avant-dernier et le
/// dernier. `modifies` : des plans ne sont plus comme au dernier envoi. `(None, None)` : pas d'envoi.
pub fn defauts(envois: &Envois, modifies: bool) -> (Option<u64>, Option<u64>) {
    let mut ids: Vec<u64> = envois.list.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    match ids.as_slice() {
        [] => (None, None),
        [.., dernier] if modifies => (Some(*dernier), None),
        [.., avant, dernier] => (Some(*avant), Some(*dernier)),
        [seul] => (Some(*seul), None),
    }
}

/// La comparaison faite, avec les envois entiers (pour relire leurs instantanés).
pub(crate) struct Base {
    pub(crate) journal: Journal,
    pub(crate) a: Envoi,
    pub(crate) b: Option<Envoi>,
}

impl App {
    // ------------------------------------------------------------------ la comparaison

    /// Compare l'envoi `a` à l'envoi `b` (ou à l'état actuel si `b` est `None`). Deux envois dans le désordre sont remis dans l'ordre.
    pub(crate) fn journal_base(&self, a: u64, b: Option<u64>) -> Result<Base, String> {
        let (mut a, mut b) = (a, b);
        if let Some(bb) = b {
            if bb == a {
                return Err("Choisis deux états différents.".into());
            }
            if bb < a {
                (a, b) = (bb, Some(a));
            }
        }
        let ea = self.doc.envois.trouve(a).ok_or("Cet envoi n'existe plus.")?.clone();
        let eb = match b {
            Some(id) => Some(self.doc.envois.trouve(id).ok_or("Cet envoi n'existe plus.")?.clone()),
            None => None,
        };
        // Le côté « État actuel » : les empreintes que le suivi des plans a calculées sur un autre fil (`envoisui.rs`).
        let mut actuels = Vec::new();
        if eb.is_none() {
            let attente = "Le relevé des plans est en cours : réessaie dans un instant.";
            if self.envois_ui.fait_pour() != Some(self.envois_releve()) {
                return Err(attente.into());
            }
            for (i, p) in self.doc.panels.iter().enumerate() {
                let empreinte = self.envois_ui.empreinte(p.id).ok_or(attente)?;
                actuels.push(PlanVu { id: p.id, titre: crate::storyboard::panel_title(&self.doc, i), empreinte, version: self.doc.numero_de_version(p.id), images: p.frame_count() });
            }
        }
        let journal = journal::faire(&self.doc, &ea, eb.as_ref(), actuels, crate::recovery::now_secs());
        Ok(Base { journal, a: ea, b: eb })
    }

    /// De quoi lancer la tâche : la comparaison, les copies des plans d'aujourd'hui qu'il faut montrer, et le dossier des instantanés.
    pub(crate) fn journal_demande(&self, a: u64, b: Option<u64>, html: Option<PathBuf>, pdf: Option<PathBuf>) -> Result<Demande, String> {
        let base = self.journal_base(a, b)?;
        let dossier = self.doc.envois.dossier.clone().or_else(|| self.path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf));
        for e in std::iter::once(&base.a).chain(base.b.as_ref()) {
            if self.doc.envois.pending.iter().any(|p| p.id == e.id) {
                return Err(format!("l'envoi « {} » s'enregistre encore : réessaie dans un instant.", e.nom));
            }
        }
        let dossier = dossier.ok_or("Enregistre d'abord le projet : l'état d'un envoi est gardé dans son dossier.")?;
        let a = Cote::Envoi { dossier: dossier.clone(), envoi: Box::new(base.a.clone()) };
        let b = match &base.b {
            Some(e) => Cote::Envoi { dossier, envoi: Box::new(e.clone()) },
            None => {
                // L'état d'aujourd'hui : une petite copie des plans à montrer (modifiés ou ajoutés), sans recopier un pixel.
                let mut docs = HashMap::new();
                for l in base.journal.diff.lignes.iter().filter(|l| matches!(l.genre, Genre::Modifie | Genre::Ajoute)) {
                    if let Some(p) = self.doc.panels.iter().find(|p| p.id == l.id) {
                        docs.insert(l.id, comparer::petit_projet(&self.doc, p));
                    }
                }
                Cote::Docs(docs)
            }
        };
        Ok(Demande { journal: base.journal, a, b, html, pdf })
    }

    /// Lance le journal vers `base` (sans extension) : `<base>.html` et / ou `<base>.pdf`. La fenêtre garde la tâche.
    pub(crate) fn journal_exporter_vers(&mut self, ctx: &egui::Context, base: &Path) {
        let Some(mut w) = self.journal.take() else { return };
        w.resultat = None;
        match w.a {
            None => w.resultat = Some(Err("Il n'y a pas encore d'envoi à comparer.".into())),
            Some(a) => match self.journal_demande(a, w.b, w.html.then(|| base.with_extension("html")), w.pdf.then(|| base.with_extension("pdf"))) {
                Ok(d) => match JournalJob::start(d, Some(ctx.clone())) {
                    Ok(j) => w.job = Some(j),
                    Err(e) => w.resultat = Some(Err(e)),
                },
                Err(e) => w.resultat = Some(Err(e)),
            },
        }
        self.journal = Some(w);
    }

    // ------------------------------------------------------------------ la fenêtre

    /// Ouvre la fenêtre. `depuis` : un envoi précis comme premier côté (l'état actuel au second) ; sinon les côtés proposés d'origine.
    pub(crate) fn journal_ouvrir(&mut self, depuis: Option<u64>) {
        if self.doc.envois.is_empty() {
            self.status = "Aucun envoi : « Marquer un envoi… » d'abord, puis le journal dira ce qui a changé depuis.".into();
            return;
        }
        let (a, b) = match depuis {
            Some(id) => (Some(id), None),
            None => defauts(&self.doc.envois, !self.envois_ui.etats_vides()),
        };
        // Une tâche tourne : on garde la fenêtre telle quelle.
        if self.journal.as_ref().is_some_and(JournalFenetre::busy) {
            return;
        }
        let (pdf, html) = self.journal.as_ref().map_or((true, true), |w| (w.pdf, w.html));
        self.journal = Some(JournalFenetre { a, b, pdf, html, job: None, resultat: None, epoch: self.doc_epoch, cache: None });
    }

    /// Les deux côtés que le journal du paquet de livraison compare : les mêmes que ceux que la fenêtre propose d'abord.
    pub(crate) fn journal_defauts(&self) -> (Option<u64>, Option<u64>) {
        defauts(&self.doc.envois, !self.envois_ui.etats_vides())
    }

    pub(crate) fn journal_fenetre(&mut self, ctx: &egui::Context) {
        let Some(mut w) = self.journal.take() else { return };
        // Un autre projet est devant (onglet, ouverture) : les envois choisis ne sont plus les siens.
        if w.epoch != self.doc_epoch && !w.busy() {
            return;
        }
        if let Some(job) = &mut w.job {
            job.poll();
            if job.running() {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            } else if let Some(res) = job.outcome.take() {
                self.status = match &res {
                    Ok(r) => format!("Journal des différences écrit ({} modifié{}, {} ajouté{}, {} retiré{}).", r.modifies, if r.modifies > 1 { "s" } else { "" }, r.ajoutes, if r.ajoutes > 1 { "s" } else { "" }, r.retires, if r.retires > 1 { "s" } else { "" }),
                    Err(e) => format!("Journal des différences : {e}"),
                };
                w.resultat = Some(res);
                w.job = None;
            }
        }
        // La comparaison, refaite seulement si quelque chose a changé.
        let cle: Cle = (w.a, w.b, self.history.serial(), self.envois_ui.fait_pour(), self.doc.envois.list.len(), self.doc.comments.len());
        if w.cache.as_ref().map(|c| c.0) != Some(cle) {
            let r = match w.a {
                Some(a) => self.journal_base(a, w.b).map(|b| b.journal),
                None => Err("Aucun envoi.".into()),
            };
            w.cache = Some((cle, r));
        }
        let mut ouverte = true;
        let mut exporter = false;
        let mut aller: Option<u64> = None;
        egui::Window::new("Journal des différences")
            .open(&mut ouverte)
            .collapsible(false)
            .default_width(520.0)
            .default_pos(ctx.screen_rect().center() - vec2(260.0, 280.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| exporter = self.journal_corps(ui, &mut w, &mut aller));
        if let Some(id) = aller {
            if let Some(i) = self.doc.panels.iter().position(|p| p.id == id) {
                self.select_panel(i);
                self.board_sel = vec![id];
            }
        }
        if exporter {
            if let Some(base) = self.journal_choisir_fichier(&w) {
                self.journal = Some(w);
                self.journal_exporter_vers(ctx, &base);
                return;
            }
        }
        if !ouverte {
            if let Some(job) = &mut w.job {
                job.cancel();
            }
        }
        if ouverte || w.busy() {
            self.journal = Some(w);
        }
    }

    /// La boîte « Enregistrer sous » : le nom sans extension, les deux fichiers s'écrivent à côté.
    fn journal_choisir_fichier(&self, w: &JournalFenetre) -> Option<PathBuf> {
        let Some(Ok(j)) = w.cache.as_ref().map(|c| &c.1) else { return None };
        let nom = j.nom_de_fichier(&crate::paquet::base_du_film(&self.doc), &j.de.split(',').next().unwrap_or(""), &j.vers.split(',').next().unwrap_or(""));
        let mut d = rfd::FileDialog::new().set_title("Enregistrer le journal des différences").set_file_name(format!("{nom}.pdf"));
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        d.save_file().map(|p| p.with_extension(""))
    }

    fn journal_corps(&mut self, ui: &mut egui::Ui, w: &mut JournalFenetre, aller: &mut Option<u64>) -> bool {
        let avert = theme::avertissement();
        let occupe = w.job.is_some();
        ui.label(
            RichText::new("Ce qui a changé entre deux envois : seulement les plans modifiés, côte à côte (avant / après), avec la part de l'image qui diffère et la durée ; les plans ajoutés, retirés ou déplacés ; les notes traitées entre les deux. En PDF et en page HTML à joindre à l'envoi.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.separator();
        let mut envois: Vec<(u64, String)> = self.doc.envois.list.iter().map(|e| (e.id, e.libelle())).collect();
        envois.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
        let libelle = |id: Option<u64>| id.and_then(|i| envois.iter().find(|(e, _)| *e == i)).map_or_else(|| "État actuel".to_string(), |(_, l)| l.clone());
        ui.add_enabled_ui(!occupe, |ui| {
            egui::Grid::new("journal_cotes").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("De");
                egui::ComboBox::from_id_salt("journal_de").selected_text(libelle(w.a)).width(340.0).show_ui(ui, |ui| {
                    for (id, l) in &envois {
                        ui.selectable_value(&mut w.a, Some(*id), l);
                    }
                });
                ui.end_row();
                ui.label("À");
                egui::ComboBox::from_id_salt("journal_a").selected_text(libelle(w.b)).width(340.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut w.b, None, "État actuel");
                    for (id, l) in &envois {
                        ui.selectable_value(&mut w.b, Some(*id), l);
                    }
                });
                ui.end_row();
            });
        });
        ui.separator();
        let mut prete = false;
        match w.cache.as_ref().map(|c| &c.1) {
            Some(Ok(j)) => {
                prete = true;
                ui.label(RichText::new(format!("{} → {}", j.de, j.vers)).small().color(theme::texte_doux()));
                ui.label(RichText::new(j.resume()).strong());
                if !j.notes.is_empty() {
                    let r = j.resume_notes();
                    ui.label(format!("{} traitée{} entre les deux{}.", crate::relecture::plural(j.notes.len(), "note", "notes"), if j.notes.len() > 1 { "s" } else { "" }, if r.is_empty() { String::new() } else { format!(" : {r}") }));
                }
                self.journal_liste(ui, &j.diff, aller);
            }
            Some(Err(e)) => {
                ui.colored_label(avert, e);
            }
            None => {}
        }
        ui.separator();
        ui.add_enabled_ui(!occupe, |ui| {
            ui.horizontal(|ui| {
                ui.label("Formats");
                ui.checkbox(&mut w.pdf, "PDF");
                ui.checkbox(&mut w.html, "Page HTML");
            });
        });
        let mut go = false;
        match &mut w.job {
            Some(job) => {
                let (fait, total) = job.progress();
                let texte = if job.running() { format!("Images {fait} sur {total}") } else { "Écriture…".to_string() };
                ui.add(crate::barre::Barre::new(if total == 0 { 0.0 } else { fait as f32 / total as f32 }).text(texte));
                if bulles::icone(ui, ph::X, "Arrêter", "Arrête le journal : aucun fichier ne reste.", "").clicked() {
                    job.cancel();
                }
            }
            None => {
                let ok = prete && (w.pdf || w.html);
                if bulles::icone_si(ui, ok, ph::EXPORT, "Exporter le journal…", "Choisis où écrire le PDF et la page HTML (même nom, deux extensions). Le calcul des images se fait en tâche de fond.", "").clicked() {
                    go = true;
                }
            }
        }
        match &w.resultat {
            Some(Ok(r)) => {
                ui.separator();
                ui.label(RichText::new(format!("Terminé : {:.1} Mo.", r.octets as f64 / 1_048_576.0)).strong().color(theme::succes()));
                ui.label(RichText::new(format!("{} modifiés, {} ajoutés, {} retirés, {} déplacés, {} notes traitées.", r.modifies, r.ajoutes, r.retires, r.deplaces, r.notes)).small());
                for p in r.html.iter().chain(r.pdf.iter()) {
                    ui.label(RichText::new(p.display().to_string()).small());
                }
                if let Some(p) = r.pdf.as_ref().or(r.html.as_ref()) {
                    if bulles::icone(ui, ph::FOLDER_OPEN, "Montrer dans l'explorateur", "Ouvre le dossier du journal.", "").clicked() {
                        crate::exportui::reveal_in_explorer(p);
                    }
                }
            }
            Some(Err(e)) => {
                ui.separator();
                ui.colored_label(avert, e);
            }
            None => {}
        }
        go
    }

    /// La liste des plans qui ont changé : un clic va au plan.
    fn journal_liste(&self, ui: &mut egui::Ui, d: &Diff, aller: &mut Option<u64>) {
        if d.est_vide() {
            return;
        }
        egui::ScrollArea::vertical().id_salt("journal_liste").max_height(260.0).show(ui, |ui| {
            for l in &d.lignes {
                let sorte = match l.genre {
                    Genre::Modifie => "modifié",
                    Genre::Ajoute => "ajouté",
                    Genre::Retire => "retiré",
                    Genre::Deplace => "déplacé",
                };
                let mut texte = format!("{}  ({sorte})", l.nom);
                if l.genre == Genre::Modifie {
                    let dur = journal::durees_texte(l.images_a, l.images_b, self.doc.fps, "→");
                    if l.images_a != l.images_b {
                        texte.push_str(&format!("  · {dur}"));
                    }
                    if l.deplace {
                        texte.push_str("  · déplacé");
                    }
                }
                let present = self.doc.panels.iter().any(|p| p.id == l.id);
                if ui.add_enabled(present, egui::SelectableLabel::new(false, texte)).clicked() {
                    *aller = Some(l.id);
                }
            }
        });
    }
}
