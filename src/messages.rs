//! Messages qui restent (Anthony, 2026-10-18, axe d'ergonomie 3) : chaque message de la barre
//! d'état rejoint un historique (clic sur le message pour l'ouvrir, survol pour le lire en
//! entier) ; les avis importants (enregistrement, ouverture, erreurs, fin de propagation, export)
//! s'affichent aussi quelques secondes en bas à droite, où un clic les ferme.

use eframe::egui::{self, Align2, RichText, Vec2};

use crate::app::App;

/// Taille de l'historique.
const KEEP: usize = 200;
/// Durée d'une notification, en secondes.
const SHOWN: f64 = 6.0;

/// Ce qu'un bouton à côté d'un message de la barre d'état peut faire.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ActionMessage {
    /// « Transformer en calque d'animation » (le calque vidéo courant).
    VideoEnAnimation,
    /// Le dossier d'un export fini.
    MontrerDossier(std::path::PathBuf),
    /// Met l'ancien `crayonnes.json` de ce dossier de projet de côté (renommé, jamais effacé).
    CrayonnesDeCote(std::path::PathBuf),
    /// Le remet à son nom.
    CrayonnesRemettre(std::path::PathBuf),
}

impl ActionMessage {
    pub(crate) fn libelle(&self) -> &'static str {
        match self {
            ActionMessage::VideoEnAnimation => "Transformer en calque d'animation",
            ActionMessage::MontrerDossier(_) => "Ouvrir le dossier",
            ActionMessage::CrayonnesDeCote(_) => "Mettre de côté",
            ActionMessage::CrayonnesRemettre(_) => "Annuler",
        }
    }

    pub(crate) fn aide(&self) -> &'static str {
        match self {
            ActionMessage::VideoEnAnimation => "Chaque image de la vidéo devient un dessin : le calque se dessine alors comme les autres (annulable).",
            ActionMessage::MontrerDossier(_) => "Ouvre l'explorateur sur ce qui vient d'être exporté.",
            ActionMessage::CrayonnesDeCote(_) => "Renomme crayonnes.json en crayonnes.json.ancien dans le dossier du projet : cet avis ne revient plus. Rien n'est effacé.",
            ActionMessage::CrayonnesRemettre(_) => "Remet crayonnes.json à son nom d'origine.",
        }
    }
}

/// Le message mérite-t-il une notification ? (Ceux qui disent qu'une chose importante a réussi
/// ou échoué ; les messages de geste, qui changent à chaque image, non.)
pub fn important(msg: &str) -> bool {
    const WORDS: [&str; 12] = [
        "Enregistré", "Échec", "échec", "mpossible", "erreur", "Erreur", "Propagation :", "Ouvert", "Export", "exporté", "Projet fermé", "Rien n'a changé",
    ];
    WORDS.iter().any(|w| msg.contains(w))
}

/// Le nouveau message `msg` à l'instant `now` : il remplace le dernier s'il le prolonge (même
/// début, moins d'une seconde après : un geste qui met sa valeur à jour), sinon il s'ajoute.
pub fn record(history: &mut Vec<(f64, String)>, now: f64, msg: &str) {
    let head = |s: &str| s.chars().take(12).collect::<String>();
    match history.last_mut() {
        Some((t, last)) if now - *t < 1.0 && head(last) == head(msg) => {
            *t = now;
            *last = msg.to_string();
        }
        _ => history.push((now, msg.to_string())),
    }
    if history.len() > KEEP {
        history.remove(0);
    }
}

/// Longueur au-delà de laquelle la barre d'état ne montre plus qu'un début de phrase.
const COURT: usize = 70;

/// Ce que la barre d'état montre d'un message : lui-même s'il est court ; sinon son début, coupé avant la première
/// parenthèse, le premier point ou le premier point-virgule qui vient après quelques mots, suivi de « … ». Le détail
/// n'est pas perdu : le message entier est dans la bulle de la barre et dans l'historique (simplification des barres,
/// 2026-10-21 : une seule astuce courte, le reste dans les bulles).
pub fn resume(msg: &str) -> String {
    let msg = msg.trim();
    if msg.chars().count() <= COURT {
        return msg.to_string();
    }
    // Les coupures qu'on cherche, la plus tôt gagne, pourvu qu'il reste assez de texte avant elle.
    let minimum = 12;
    let coupure = [" (", ". ", " ; ", " — "]
        .iter()
        .filter_map(|c| msg.char_indices().find(|(i, _)| msg[*i..].starts_with(c) && msg[..*i].chars().count() >= minimum).map(|(i, _)| i))
        .min();
    match coupure {
        Some(i) => format!("{}…", msg[..i].trim_end_matches([' ', ',', ';', ':'])),
        None => msg.to_string(),
    }
}

/// « il y a 3 min » pour un message d'il y a `secs` secondes.
pub fn ago(secs: f64) -> String {
    match secs as u64 {
        s if s < 60 => "à l'instant".into(),
        s if s < 3600 => format!("il y a {} min", s / 60),
        s => format!("il y a {} h {:02}", s / 3600, (s % 3600) / 60),
    }
}

impl App {
    /// Une fois par image : un message nouveau rejoint l'historique, et les importants deviennent
    /// des notifications.
    pub(crate) fn track_messages(&mut self, ctx: &egui::Context) {
        if self.status == self.last_status {
            return;
        }
        self.last_status = self.status.clone();
        if self.status.is_empty() {
            return;
        }
        let now = ctx.input(|i| i.time);
        record(&mut self.messages, now, &self.status);
        if important(&self.status) {
            self.toasts.push((now, self.status.clone()));
            if self.toasts.len() > 3 {
                self.toasts.remove(0);
            }
        }
    }

    /// Pose le message `msg` dans la barre d'état avec un bouton qui fait `action` (le bouton ne vaut que tant que ce message est
    /// celui de la barre : le message suivant l'efface).
    pub(crate) fn status_avec_action(&mut self, msg: String, action: ActionMessage) {
        self.status = msg.clone();
        self.ctx.data_mut(|d| d.insert_temp(egui::Id::new("action_du_message"), (msg, action)));
    }

    /// Le bouton du message actuel de la barre d'état.
    pub(crate) fn action_du_message(&self) -> Option<ActionMessage> {
        self.action_pour(&self.status)
    }

    fn action_pour(&self, msg: &str) -> Option<ActionMessage> {
        let (pour, action) = self.ctx.data(|d| d.get_temp::<(String, ActionMessage)>(egui::Id::new("action_du_message")))?;
        (pour == msg).then_some(action)
    }

    /// Fait ce que dit le bouton d'un message.
    pub(crate) fn lancer_action_message(&mut self, action: ActionMessage) {
        match action {
            ActionMessage::VideoEnAnimation => self.video_to_animation(),
            ActionMessage::MontrerDossier(p) => crate::exportui::reveal_in_explorer(&p),
            ActionMessage::CrayonnesDeCote(dir) => self.crayonnes_de_cote(&dir),
            ActionMessage::CrayonnesRemettre(dir) => self.crayonnes_remettre(&dir),
        }
    }

    /// Le message de la barre d'état : une ligne courte (`resume`), son texte entier au survol, l'historique au clic.
    pub(crate) fn status_message(&mut self, ui: &mut egui::Ui) {
        // Le bouton du message (s'il en a un) passe avant le texte : une phrase longue ne le pousse pas hors de la barre.
        if let Some(action) = self.action_du_message() {
            let r = crate::bulles::bulle(ui.small_button(action.libelle()), action.libelle(), action.aide(), "");
            if r.clicked() {
                self.lancer_action_message(action);
            }
        }
        let court = resume(&self.status);
        let resp = ui.add(egui::Label::new(court).truncate().sense(egui::Sense::click()));
        if resp.on_hover_text(format!("{}\n\nClic : les messages précédents.", self.status)).clicked() {
            self.messages_window = !self.messages_window;
        }
    }

    /// Les notifications en bas à droite, et la fenêtre de l'historique.
    pub(crate) fn messages_ui(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|(t, _)| now - t < SHOWN);
        if !self.toasts.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            let mut closed = None;
            egui::Area::new(egui::Id::new("notifications"))
                .order(egui::Order::Foreground)
                .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -36.0))
                .show(ctx, |ui| {
                    for (k, (_, msg)) in self.toasts.iter().enumerate().rev() {
                        let frame = egui::Frame::popup(ui.style()).fill(crate::theme::eleve()).stroke(egui::Stroke::new(1.0_f32, crate::theme::accent_texte()));
                        let resp = frame
                            .show(ui, |ui| {
                                ui.set_max_width(420.0);
                                ui.add(egui::Label::new(RichText::new(msg).color(crate::theme::texte_fort())).wrap());
                            })
                            .response
                            .interact(egui::Sense::click());
                        if resp.on_hover_text("Clic : fermer").clicked() {
                            closed = Some(k);
                        }
                        ui.add_space(4.0);
                    }
                });
            if let Some(k) = closed {
                self.toasts.remove(k);
            }
        }
        if !self.messages_window {
            return;
        }
        let mut open = true;
        egui::Window::new("Messages")
            .open(&mut open)
            .default_size([520.0, 320.0])
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -36.0))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if self.messages.is_empty() {
                        ui.label(RichText::new("Aucun message pour l'instant.").weak());
                    }
                    for (t, msg) in self.messages.iter().rev() {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(ago(now - t)).small().weak());
                            ui.label(msg);
                        });
                        ui.separator();
                    }
                });
            });
        self.messages_window = open;
    }
}
