//! Messages qui restent (Anthony, 2026-10-18, axe d'ergonomie 3) : chaque message de la barre
//! d'état rejoint un historique (clic sur le message pour l'ouvrir, survol pour le lire en
//! entier) ; les avis importants (enregistrement, ouverture, erreurs, fin de propagation, export)
//! s'affichent aussi quelques secondes en bas à droite, où un clic les ferme.

use eframe::egui::{self, Align2, Color32, RichText, Vec2};

use crate::app::App;

/// Taille de l'historique.
const KEEP: usize = 200;
/// Durée d'une notification, en secondes.
const SHOWN: f64 = 6.0;

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

    /// Le message de la barre d'état : son texte entier au survol, l'historique au clic.
    pub(crate) fn status_message(&mut self, ui: &mut egui::Ui) {
        let resp = ui.add(egui::Label::new(&self.status).truncate().sense(egui::Sense::click()));
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
                        let frame = egui::Frame::popup(ui.style()).fill(crate::theme::ELEVE).stroke(egui::Stroke::new(1.0_f32, crate::theme::ACCENT));
                        let resp = frame
                            .show(ui, |ui| {
                                ui.set_max_width(420.0);
                                ui.add(egui::Label::new(RichText::new(msg).color(Color32::WHITE)).wrap());
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
