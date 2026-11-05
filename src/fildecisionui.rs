//! Fil de décision par plan, côté interface (modèle : `fildecision.rs`) : le statut de chaque note, le fil d'un plan
//! à travers ses versions, la vue d'ensemble (compteurs, filtre, note suivante à traiter), la pastille de statut sur
//! les vignettes et dans le montage, les retakes. Tout vit dans le panneau Commentaires (`commentairesui.rs`).
//! Chaque changement est une étape d'annulation qui porte son nom.

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::commentaires::{Comment, View};
use crate::fildecision::{PlanMark, Status};
use crate::theme;

/// L'icône d'un statut.
pub(crate) fn status_icon(s: Status) -> &'static str {
    match s {
        Status::Todo => ph::CIRCLE,
        Status::Done => ph::CHECK_CIRCLE,
        Status::Refused => ph::X_CIRCLE,
        Status::Discussion => ph::CHAT_CIRCLE_DOTS,
    }
}

/// La couleur d'un statut (celles du thème : encre, succès, accent ; le refus se lit à son icône, le rouge `alerte` est réservé aux erreurs bloquantes).
pub(crate) fn status_color(s: Status) -> Color32 {
    match s {
        Status::Todo => theme::avertissement(),
        Status::Done => theme::succes(),
        Status::Refused => theme::texte_fort(),
        Status::Discussion => theme::accent_texte(),
    }
}

/// La case de statut d'une ligne de la liste : une case vide (à faire), ou l'icône du statut à sa couleur.
pub(crate) fn paint_status_box(painter: &Painter, rect: Rect, status: Status, hovered: bool) {
    let frame = Rect::from_center_size(rect.center(), vec2(14.0, 14.0));
    match status {
        Status::Todo => {
            painter.rect_stroke(frame, 3.0, Stroke::new(1.0_f32, if hovered { theme::texte() } else { theme::texte_doux() }), StrokeKind::Inside);
        }
        s => {
            painter.text(frame.center(), Align2::CENTER_CENTER, status_icon(s), FontId::proportional(15.0), status_color(s));
        }
    }
}

/// Sous la case de statut : le retake et le conflit, en petits symboles.
pub(crate) fn paint_row_marks(painter: &Painter, check: Rect, c: &Comment, doc: &crate::document::Document) {
    let mut x = check.right() - 4.0;
    let y = check.bottom() + 8.0;
    if !doc.conflicts_of(c.id).is_empty() {
        painter.text(pos2(x, y), Align2::RIGHT_CENTER, ph::LIGHTNING, FontId::proportional(12.0), theme::braise());
        x -= 14.0;
    }
    if c.follow.retake.is_some() {
        painter.text(pos2(x, y), Align2::RIGHT_CENTER, ph::ARROW_COUNTER_CLOCKWISE, FontId::proportional(12.0), theme::texte_doux());
    }
}

/// La pastille de notes à traiter centrée en `centre` : le nombre de notes à faire ou en discussion, à l'accent ; à
/// la couleur d'erreur quand deux notes à traiter sont en conflit. Même forme que la pastille « modifié depuis
/// l'envoi » (`envoisui::peindre_repere`).
pub(crate) fn peindre_marque(painter: &Painter, centre: Pos2, m: &PlanMark, rayon: f32) {
    // (Un conflit n'est pas une erreur : même pastille, avec un anneau braise, jamais un aplat rouge ni braise.)
    painter.circle_filled(centre, rayon, theme::accent_texte());
    painter.circle_stroke(centre, rayon, if m.conflict { Stroke::new(2.0_f32, theme::braise()) } else { Stroke::new(1.0_f32, theme::encre()) });
    let n = m.counts.open();
    let texte = if n > 9 { "9+".to_string() } else { n.to_string() };
    painter.text(centre, Align2::CENTER_CENTER, texte, FontId::proportional(rayon * 1.2), theme::encre());
}

/// Peint la pastille de notes sur le bloc d'un plan de la bande, sous celle de « modifié depuis l'envoi », et rend sa zone
/// de survol. Rien sur un bloc trop petit (la bande zoomée très loin) : le panneau Commentaires a tout.
pub(crate) fn peindre_marque_du_bloc(painter: &Painter, bloc: Rect, m: &PlanMark) -> Option<Rect> {
    if bloc.width() < 44.0 || bloc.height() < 76.0 {
        return None;
    }
    let r = crate::envoisui::REPERE;
    let centre = pos2(bloc.right() - 18.0, bloc.top() + 18.0 + r * 2.0 + 4.0);
    peindre_marque(painter, centre, m, r);
    Some(Rect::from_center_size(centre, vec2(r * 2.0 + 2.0, r * 2.0 + 2.0)))
}

/// La bulle de la pastille : « 2 notes à traiter (1 à faire, 1 en discussion) ».
pub(crate) fn marque_phrase(m: &PlanMark) -> String {
    let n = m.counts.open();
    let mut parts: Vec<String> = Vec::new();
    for s in [Status::Todo, Status::Discussion] {
        let k = m.counts.get(s);
        if k > 0 {
            parts.push(format!("{k} {}", s.label().to_lowercase()));
        }
    }
    let mut t = format!("{n} note{} à traiter ({})", if n > 1 { "s" } else { "" }, parts.join(", "));
    if m.conflict {
        t.push_str(" · deux notes à traiter sont en conflit");
    }
    if m.retake {
        t.push_str(" · retake demandé");
    }
    t
}

/// Un compteur de statut qui sert de filtre : un petit cadre à la couleur du statut quand ses notes sont montrées, gris
/// et sans cadre quand elles sont cachées. (Peint à la main : le texte garde la couleur de son statut.)
fn chip(ui: &mut egui::Ui, s: Status, shown: bool, n: usize) -> egui::Response {
    let color = if shown { status_color(s) } else { theme::texte_doux() };
    let galley = ui.painter().layout_no_wrap(format!("{} {} {}", status_icon(s), s.label(), n), FontId::proportional(11.0), color);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(12.0, 6.0), Sense::click());
    if shown {
        ui.painter().rect_filled(rect, 4.0, theme::eleve());
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, color), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
    }
    ui.painter().galley(rect.min + vec2(6.0, 3.0), galley, color);
    resp
}

fn who(c: &Comment) -> String {
    if c.author.trim().is_empty() {
        "Sans nom".to_string()
    } else {
        c.author.trim().to_string()
    }
}

fn date(secs: u64) -> String {
    if secs == 0 {
        String::new()
    } else {
        crate::projetui::date_label(secs)
    }
}

impl App {
    // ------------------------------------------------------------------ commandes

    /// Change le statut de la note `id` : une étape d'annulation qui porte son nom. « Refusé » sans raison est refusé
    /// (rien ne change, la barre d'état le dit).
    pub(crate) fn set_note_status(&mut self, id: u64, status: Status, reason: &str) {
        match self.doc.note_status_changes(id, status, reason) {
            Err(e) => self.status = e,
            Ok(false) => {}
            Ok(true) => {
                self.comment_step(match status {
                    Status::Todo => "Note à faire",
                    Status::Done => "Note faite",
                    Status::Refused => "Note refusée",
                    Status::Discussion => "Note en discussion",
                });
                let _ = self.doc.set_note_status(id, status, reason, crate::recovery::now_secs());
                self.status = format!("Note : {}.", status.label().to_lowercase());
            }
        }
    }

    /// Le statut voulu depuis un menu ou une commande : « Refusé » ouvre d'abord la raison dans l'éditeur.
    pub(crate) fn ask_note_status(&mut self, id: u64, status: Status) {
        if self.doc.comment(id).is_none() {
            return;
        }
        if status == Status::Refused {
            let reason = self.doc.comment(id).map(|c| c.follow.reason.clone()).unwrap_or_default();
            self.comment_ui.refusing = Some((id, reason));
            self.select_comment(id, false);
            self.show_pane(crate::dock::Pane::Comments);
            self.status = "Refuser une note : écris la raison dans le panneau Commentaires.".into();
        } else {
            self.comment_ui.refusing = None;
            self.set_note_status(id, status, "");
        }
    }

    /// Pose ou retire le retake de la note.
    pub(crate) fn toggle_retake(&mut self, id: u64) {
        let Some(has) = self.doc.comment(id).map(|c| c.follow.retake.is_some()) else { return };
        self.comment_step(if has { "Retirer le retake" } else { "Note de retake" });
        self.doc.set_retake(id, if has { None } else { Some("") });
        if !has {
            self.comment_ui.focus_motive = true;
            self.select_comment(id, false);
            self.show_pane(crate::dock::Pane::Comments);
        }
    }

    /// Va à la note à traiter qui suit (à faire ou en discussion), en revenant au début en fin de film.
    pub(crate) fn next_note_to_treat(&mut self) {
        let sel = self.comment_ui.sel;
        match self.doc.next_to_treat(self.global_frame(), sel, self.comment_ui.filter.send) {
            Some(id) => {
                self.select_comment(id, true);
                self.show_pane(crate::dock::Pane::Comments);
                self.comment_status(id);
            }
            None => self.status = "Plus aucune note à traiter.".into(),
        }
    }

    /// Écrit la liste des retakes (CSV) là où l'utilisateur le choisit.
    pub(crate) fn export_retakes(&mut self) {
        if self.doc.retake_rows().is_empty() {
            self.status = "Aucun retake à exporter : marque une note « retake » d'abord.".into();
            return;
        }
        let name = format!("{}_retakes.csv", self.base_name());
        let Some(mut path) = rfd::FileDialog::new().set_title("Liste des retakes").set_file_name(name).add_filter("Tableur (CSV)", &["csv"]).save_file() else { return };
        if path.extension().is_none() {
            path.set_extension("csv");
        }
        let n = self.doc.retake_rows().len();
        self.status = match crate::export::write_atomic(&path, &self.doc.retakes_csv()) {
            Ok(()) => format!("{n} retake{} écrit{} dans {}.", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }, path.display()),
            Err(e) => format!("Les retakes n'ont pas été écrits : {e}"),
        };
    }

    /// Les commandes du fil ; vrai si `action` en était une.
    pub(crate) fn notes_action(&mut self, action: &str) -> bool {
        let needs = |s: &mut App, f: &dyn Fn(&mut App, u64)| match s.comment_ui.sel.filter(|id| s.doc.comment(*id).is_some()) {
            Some(id) => f(s, id),
            None => s.status = "Aucune note choisie.".into(),
        };
        match action {
            "notes.suivante_a_traiter" => self.next_note_to_treat(),
            "notes.fil" => {
                self.comment_ui.view = View::Fil;
                self.show_pane(crate::dock::Pane::Comments);
            }
            "notes.statut_a_faire" => needs(self, &|s, id| s.ask_note_status(id, Status::Todo)),
            "notes.statut_fait" => needs(self, &|s, id| s.ask_note_status(id, Status::Done)),
            "notes.statut_refuse" => needs(self, &|s, id| s.ask_note_status(id, Status::Refused)),
            "notes.statut_discussion" => needs(self, &|s, id| s.ask_note_status(id, Status::Discussion)),
            "notes.retake" => needs(self, &|s, id| s.toggle_retake(id)),
            "notes.conflit" => needs(self, &|s, _| {
                s.comment_ui.conflict_pick = true;
                s.show_pane(crate::dock::Pane::Comments);
            }),
            "notes.retakes_exporter" => self.export_retakes(),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ le haut du panneau

    /// Les icônes de commande à côté de « Nouveau commentaire » : la note suivante à traiter et la liste des retakes.
    pub(crate) fn notes_commands(&mut self, ui: &mut egui::Ui) {
        self.command_button(ui, ph::SKIP_FORWARD, "notes.suivante_a_traiter", "Note suivante à traiter", "Va à la prochaine note à faire ou en discussion, plan après plan (revient au début du film en fin de course).", None);
        self.command_button(ui, ph::FILE_CSV, "notes.retakes_exporter", "Liste des retakes (CSV)", "Écrit un tableur des notes marquées « retake » : plan, version, motif, statut.", None);
    }

    /// La vue d'ensemble : liste du film ou fil du plan, envoi, compteurs qui servent de filtre.
    pub(crate) fn notes_overview(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::bulle;
        ui.horizontal_wrapped(|ui| {
            let liste = ui.selectable_label(self.comment_ui.view == View::Liste, "Tout le film");
            if bulle(liste, "Tout le film", "Toutes les notes, dans l'ordre du film.", "").clicked() {
                self.comment_ui.view = View::Liste;
            }
            let fil = ui.selectable_label(self.comment_ui.view == View::Fil, "Fil du plan");
            if bulle(fil, "Fil du plan", "Les notes du plan courant dans l'ordre où elles sont venues, à travers ses versions, avec les réponses.", &self.raccourci("notes.fil")).clicked() {
                self.comment_ui.view = View::Fil;
            }
            if !self.doc.envois.list.is_empty() {
                let current = match self.comment_ui.filter.send.and_then(|s| self.doc.envois.trouve(s)) {
                    Some(e) => e.nom.clone(),
                    None => "Tous les envois".to_string(),
                };
                egui::ComboBox::from_id_salt("notes_envoi").selected_text(current).width(130.0).show_ui(ui, |ui| {
                    if ui.selectable_label(self.comment_ui.filter.send.is_none(), "Tous les envois").clicked() {
                        self.comment_ui.filter.send = None;
                    }
                    let mut list: Vec<(u64, String)> = self.doc.envois.list.iter().map(|e| (e.id, e.nom.clone())).collect();
                    list.sort_by_key(|(id, _)| std::cmp::Reverse(*id));
                    for (id, nom) in list {
                        let n = self.doc.note_counts(Some(id));
                        let label = format!("{nom} · {} à traiter sur {}", n.open(), n.total());
                        if ui.selectable_label(self.comment_ui.filter.send == Some(id), label).clicked() {
                            self.comment_ui.filter.send = Some(id);
                        }
                    }
                });
            }
        });
        // Les compteurs : un par statut ; un clic montre ou cache les notes de ce statut (liste et règles), clic droit : seulement celui-là.
        let counts = self.doc.note_counts(self.comment_ui.filter.send);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for s in Status::ALL {
                let shown = self.comment_ui.filter.shown[s.index()];
                let n = counts.get(s);
                let resp = chip(ui, s, shown, n);
                let resp = bulle(resp, s.label(), if shown { "Cliquer pour cacher ces notes de la liste et des règles. Clic droit : seulement celles-là." } else { "Cliquer pour montrer ces notes. Clic droit : seulement celles-là." }, "");
                if resp.clicked() {
                    self.comment_ui.filter.shown[s.index()] = !shown;
                }
                if resp.secondary_clicked() {
                    self.comment_ui.filter.shown = [false; 4];
                    self.comment_ui.filter.shown[s.index()] = true;
                }
            }
            if !self.comment_ui.filter.is_all() && ui.small_button("Tout").clicked() {
                self.comment_ui.filter = crate::fildecision::Filter::all();
            }
        });
    }

    // ------------------------------------------------------------------ le fil du plan

    /// Les notes du plan courant, de la plus ancienne à la plus récente, à travers ses versions.
    pub(crate) fn thread_view(&mut self, ui: &mut egui::Ui) {
        let panel = self.panel().id;
        let ids: Vec<u64> = self.doc.thread_of(panel).into_iter().filter(|id| self.doc.comment(*id).is_some_and(|c| self.comment_ui.filter.matches(c))).collect();
        let all = self.doc.thread_of(panel).len();
        let title = crate::storyboard::panel_title(&self.doc, self.cur_panel);
        let versions = self.doc.nombre_de_versions(panel);
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).strong());
            if versions >= 2 {
                ui.label(RichText::new(format!("{versions} versions, V{} affichée", self.doc.numero_de_version(panel))).small().weak());
            }
            ui.label(RichText::new(format!("{} / {all}", ids.len())).small().weak());
        });
        if ids.is_empty() {
            ui.add_space(6.0);
            let text = if all == 0 { "Aucune note sur ce plan. Les notes d'un plan restent ici même quand on change de version." } else { "Aucune note avec ce filtre." };
            ui.label(RichText::new(text).small().weak());
            return;
        }
        let mut clicked: Option<u64> = None;
        for id in ids {
            let Some(c) = self.doc.comment(id).cloned() else { continue };
            if self.thread_card(ui, &c) {
                clicked = Some(id);
            }
        }
        if let Some(id) = clicked {
            self.select_comment(id, true);
            self.comment_ui.scroll_to = None;
        }
    }

    /// Une note du fil : statut, auteur, date, version et envoi, texte, raison, retake, conflits, réponses. Rend vrai si on
    /// l'a cliquée ; le clic droit ouvre son menu.
    fn thread_card(&mut self, ui: &mut egui::Ui, c: &Comment) -> bool {
        let id = c.id;
        let status = c.status();
        let selected = self.comment_ui.sel == Some(id);
        let fill = if selected { crate::theme::accent().gamma_multiply(0.45) } else { theme::eleve() };
        let dim = if status.is_open() { 1.0 } else { 0.7 };
        let inner = egui::Frame::new().fill(fill).corner_radius(4).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{} {}", status_icon(status), status.label())).strong().color(status_color(status)));
                ui.label(RichText::new(who(c)).strong().color(theme::texte().gamma_multiply(dim)));
                let d = date(c.created);
                if !d.is_empty() {
                    ui.label(RichText::new(d).small().weak());
                }
            });
            let mut meta: Vec<String> = vec![c.source().label().to_string()];
            if c.follow.plan_version > 0 && self.doc.panels.iter().find(|p| p.id == c.panel).is_some_and(|_| self.doc.nombre_de_versions(c.panel) >= 2 || c.follow.plan_version >= 2) {
                meta.push(format!("V{}", c.follow.plan_version));
            }
            if let Some(e) = c.follow.send.and_then(|s| self.doc.envois.trouve(s)) {
                meta.push(format!("envoi « {} »", e.nom));
            }
            if let Some((_, f)) = self.doc.comment_spot(c) {
                meta.push(format!("image {}", self.frame_label(f)));
            }
            ui.label(RichText::new(meta.join(" · ")).small().color(theme::texte_doux()));
            if c.text.trim().is_empty() {
                ui.label(RichText::new("(pas encore de texte)").italics().weak());
            } else {
                ui.label(RichText::new(c.text.trim()).color(theme::texte().gamma_multiply(dim)));
            }
            if status == Status::Refused {
                let raison = if c.follow.reason.is_empty() { "(sans raison)".to_string() } else { c.follow.reason.clone() };
                ui.label(RichText::new(format!("Raison du refus : {raison}")).italics().color(theme::texte_doux()));
            }
            if let Some(m) = &c.follow.retake {
                let motif = if m.is_empty() { String::new() } else { format!(" : {m}") };
                crate::theme::avertir_petit(ui, format!("{} Retake{motif}", ph::ARROW_COUNTER_CLOCKWISE));
            }
            for o in self.doc.conflicts_of(id) {
                if let Some(other) = self.doc.comment(o) {
                    let line = crate::commentaires::first_line(&other.text, 60);
                    crate::theme::avertir_petit(ui, format!("{} En conflit avec {} ({}) : {line}", ph::LIGHTNING, who(other), other.status().label().to_lowercase()));
                }
            }
            for r in &c.follow.replies {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(14.0);
                    ui.label(RichText::new(format!("{} {}", ph::ARROW_BEND_DOWN_RIGHT, if r.author.trim().is_empty() { "Sans nom" } else { r.author.trim() })).small().strong());
                    ui.label(RichText::new(&r.text).small());
                    let d = date(r.created);
                    if !d.is_empty() {
                        ui.label(RichText::new(d).small().weak());
                    }
                });
            }
        });
        let rect = inner.response.rect;
        let resp = ui.interact(rect, ui.id().with(("carte_fil", id)), Sense::click());
        resp.context_menu(|ui| self.comment_menu(ui, id));
        if self.comment_ui.scroll_to == Some(id) {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            self.comment_ui.scroll_to = None;
        }
        ui.add_space(4.0);
        resp.clicked()
    }

    // ------------------------------------------------------------------ l'éditeur : statut, retake, conflits, réponses

    /// Sous le texte de la note choisie : le statut (et la raison d'un refus), d'où elle vient, le retake, les conflits, le fil
    /// de réponses.
    pub(crate) fn follow_editor(&mut self, ui: &mut egui::Ui, c: &Comment) {
        let id = c.id;
        let status = c.status();
        // Un refus en cours pour une autre note est abandonné.
        if self.comment_ui.refusing.as_ref().is_some_and(|(r, _)| *r != id) {
            self.comment_ui.refusing = None;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Statut").small().weak());
            let shown = format!("{} {}", status_icon(status), status.label());
            egui::ComboBox::from_id_salt(("statut_note", id)).selected_text(RichText::new(shown).color(status_color(status))).show_ui(ui, |ui| {
                for s in Status::ALL {
                    let label = RichText::new(format!("{}  {}", status_icon(s), s.label())).color(status_color(s));
                    if ui.selectable_label(s == status, label).clicked() {
                        self.ask_note_status(id, s);
                    }
                }
            });
            if c.created != 0 {
                ui.label(RichText::new(format!("Créé le {}", date(c.created))).small().weak());
            }
        });
        // La raison d'un refus : à écrire avant que le statut change.
        if let Some((rid, mut reason)) = self.comment_ui.refusing.take() {
            if rid == id {
                let mut keep = true;
                ui.label(RichText::new("Raison du refus (obligatoire)").small().color(theme::texte_fort()));
                let resp = ui.add(egui::TextEdit::multiline(&mut reason).id(egui::Id::new(("raison_refus", id))).desired_rows(2).desired_width(f32::INFINITY).hint_text("Pourquoi cette note est refusée…"));
                if self.comment_ui.focus_motive {
                    resp.request_focus();
                    self.comment_ui.focus_motive = false;
                }
                ui.horizontal(|ui| {
                    if ui.add_enabled(!reason.trim().is_empty(), egui::Button::new("Refuser la note")).clicked() {
                        self.set_note_status(id, Status::Refused, &reason);
                        keep = false;
                    }
                    if ui.button("Annuler").clicked() {
                        keep = false;
                    }
                });
                if keep {
                    self.comment_ui.refusing = Some((rid, reason));
                }
            }
        } else if status == Status::Refused {
            let raison = if c.follow.reason.is_empty() { "(sans raison)".to_string() } else { c.follow.reason.clone() };
            ui.label(RichText::new(format!("Raison : {raison}")).small().italics().color(theme::texte_doux()));
        }
        // D'où elle vient, l'envoi et la version qu'elle concerne.
        let mut origine = vec![format!("Source : {}", c.source().label())];
        if let Some(e) = &c.follow.external {
            origine.push(format!("n° {e}"));
        }
        if c.follow.plan_version > 0 {
            origine.push(format!("V{}", c.follow.plan_version));
        }
        ui.label(RichText::new(origine.join(" · ")).small().weak());
        if !self.doc.envois.list.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Envoi").small().weak());
                let current = c.follow.send.and_then(|s| self.doc.envois.trouve(s)).map(|e| e.nom.clone()).unwrap_or_else(|| "Aucun".to_string());
                egui::ComboBox::from_id_salt(("envoi_note", id)).selected_text(current).show_ui(ui, |ui| {
                    if ui.selectable_label(c.follow.send.is_none(), "Aucun").clicked() {
                        self.set_note_send(id, None);
                    }
                    let mut list: Vec<(u64, String)> = self.doc.envois.list.iter().map(|e| (e.id, e.nom.clone())).collect();
                    list.sort_by_key(|(i, _)| std::cmp::Reverse(*i));
                    for (sid, nom) in list {
                        if ui.selectable_label(c.follow.send == Some(sid), nom).clicked() {
                            self.set_note_send(id, Some(sid));
                        }
                    }
                });
            });
        }
        // Retake.
        ui.horizontal(|ui| {
            let mut retake = c.follow.retake.is_some();
            if ui.checkbox(&mut retake, "Retake").changed() {
                self.toggle_retake(id);
            }
            if let Some(motif) = &c.follow.retake {
                let mut text = motif.clone();
                let resp = ui.add(egui::TextEdit::singleline(&mut text).id(egui::Id::new(("motif_retake", id))).desired_width(ui.available_width()).hint_text("Motif du retake"));
                if std::mem::take(&mut self.comment_ui.focus_motive) {
                    resp.request_focus();
                }
                if self.edited_as(&resp, "Motif du retake") {
                    if let Some(m) = self.doc.comment_mut(id) {
                        m.follow.retake = Some(text.chars().take(crate::fildecision::REASON_MAX).collect());
                    }
                }
            }
        });
        // Conflits.
        let conflicts = self.doc.conflicts_of(id);
        for o in &conflicts {
            let Some(other) = self.doc.comment(*o).cloned() else { continue };
            ui.horizontal(|ui| {
                ui.label(RichText::new(ph::LIGHTNING).color(theme::braise()));
                let line = crate::commentaires::first_line(&other.text, 40);
                let label = format!("En conflit avec {} ({}) : {line}", who(&other), other.status().label().to_lowercase());
                if ui.add(egui::Label::new(RichText::new(label).small()).sense(Sense::click())).clicked() {
                    self.select_comment(*o, true);
                }
                if ui.small_button(ph::X).on_hover_text("Ce n'est plus un conflit").clicked() {
                    self.comment_step("Fin du conflit entre deux notes");
                    self.doc.unlink_conflict(id, *o);
                }
            });
        }
        if self.comment_ui.conflict_pick {
            let mut picked: Option<u64> = None;
            let mut close = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new("En conflit avec").small().weak());
                let mut others: Vec<(bool, usize, u64)> = self
                    .doc
                    .comments
                    .iter()
                    .filter(|o| o.id != id && !conflicts.contains(&o.id))
                    .map(|o| (o.panel != c.panel, self.doc.panels.iter().position(|p| p.id == o.panel).unwrap_or(usize::MAX), o.id))
                    .collect();
                others.sort_unstable();
                egui::ComboBox::from_id_salt(("conflit_note", id)).selected_text("Choisir une note…").width(220.0).show_ui(ui, |ui| {
                    if others.is_empty() {
                        ui.label(RichText::new("Aucune autre note.").small().weak());
                    }
                    for (_, rank, oid) in others.iter().take(60) {
                        let Some(o) = self.doc.comment(*oid) else { continue };
                        let plan = if *rank == usize::MAX { "Plan disparu".to_string() } else { crate::storyboard::panel_title(&self.doc, *rank) };
                        let label = format!("{plan} · {} : {}", who(o), crate::commentaires::first_line(&o.text, 36));
                        if ui.selectable_label(false, label).clicked() {
                            picked = Some(*oid);
                        }
                    }
                });
                if ui.small_button("Fermer").clicked() {
                    close = true;
                }
            });
            if let Some(o) = picked {
                self.comment_step("Deux notes en conflit");
                self.doc.link_conflict(id, o);
                self.comment_ui.conflict_pick = false;
                self.status = "Deux notes marquées en conflit : le fil les montre ensemble.".into();
            } else if close {
                self.comment_ui.conflict_pick = false;
            }
        } else if ui.small_button(format!("{} En conflit avec…", ph::LIGHTNING)).on_hover_text("Marquer cette note comme contradictoire avec une autre : à toi de décider laquelle compte.").clicked() {
            self.comment_ui.conflict_pick = true;
        }
        // Le fil de réponses.
        if !c.follow.replies.is_empty() {
            ui.add_space(2.0);
            for r in &c.follow.replies {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{} {}", ph::ARROW_BEND_DOWN_RIGHT, if r.author.trim().is_empty() { "Sans nom" } else { r.author.trim() })).small().strong());
                    ui.label(RichText::new(&r.text).small());
                    let d = date(r.created);
                    if !d.is_empty() {
                        ui.label(RichText::new(d).small().weak());
                    }
                });
            }
        }
        ui.horizontal(|ui| {
            let resp = ui.add(egui::TextEdit::singleline(&mut self.comment_ui.reply).id(egui::Id::new(("reponse_note", id))).desired_width(ui.available_width() - 70.0).hint_text("Répondre dans le fil…"));
            let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let send = ui.add_enabled(!self.comment_ui.reply.trim().is_empty(), egui::Button::new("Répondre")).clicked() || (enter && !self.comment_ui.reply.trim().is_empty());
            if send {
                let text = std::mem::take(&mut self.comment_ui.reply);
                let author = crate::commentaires::author_name(&self.settings.comment_author);
                self.comment_step("Réponse à une note");
                self.doc.add_reply(id, &author, &text, crate::recovery::now_secs());
                self.status = "Réponse ajoutée au fil de la note.".into();
            }
        });
    }

    /// Change l'envoi qu'une note concerne.
    fn set_note_send(&mut self, id: u64, send: Option<u64>) {
        if self.doc.comment(id).is_none_or(|c| c.follow.send == send) {
            return;
        }
        self.comment_step("Envoi de la note");
        if let Some(c) = self.doc.comment_mut(id) {
            c.follow.send = send;
        }
    }
}
