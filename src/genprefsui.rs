//! Fichier > Préférences > Génération sonore (Anthony, 2026-10-19) : pour chacun des trois rôles —
//! lire le storyboard, générer les bruitages et ambiances, générer les voix témoin — le fournisseur,
//! **local** (le serveur de cet ordinateur) ou **service en ligne**.
//!
//! - Local : l'adresse (toujours `127.0.0.1`) et le port, et un bouton « Tester » qui dit
//!   « non installé » tant que le serveur de la phase 2 n'est pas là.
//! - En ligne : le service, « Saisir la clé… » (rangée dans le Gestionnaire d'identifiants de Windows,
//!   `clessecretes.rs` : jamais dans `reglages.toml`), « Tester la connexion ».
//! - Une note sur les licences : les conditions d'utilisation de chaque service et de chaque modèle
//!   (usage commercial des sons) sont à vérifier par l'utilisateur.
//!
//! Les essais de connexion se font sur un fil à part : la fenêtre ne se fige jamais.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, TryRecvError};

use eframe::egui::{self, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::clessecretes::{Cle, Service};
use crate::fournisseurs::Controle;
use crate::genreglages::{Mode, Role};

/// L'état de la partie « Génération sonore » des Préférences.
#[derive(Default)]
pub(crate) struct GenPrefs {
    /// La fenêtre de saisie d'une clé est ouverte pour ce service.
    pub cle_pour: Option<Service>,
    /// Ce qui est tapé dans la fenêtre : effacé dès qu'elle se ferme.
    pub(crate) saisie: String,
    /// Le dernier mot de la fenêtre de saisie (erreur du coffre, par exemple).
    pub message_cle: String,
    /// Quels services ont une clé (lu une fois dans le coffre, puis tenu à jour).
    presence: HashMap<Service, bool>,
    presence_lue: bool,
    /// Un essai de connexion en cours (le rôle, ou rien pour le serveur local), et ses résultats.
    essai: Option<(Option<Role>, Receiver<String>)>,
    pub resultats: HashMap<Option<Role>, String>,
}

impl GenPrefs {
    /// Un essai de connexion est en cours.
    pub(crate) fn essai_en_cours(&self) -> bool {
        self.essai.is_some()
    }

    /// Remplace la saisie par des zéros avant de la lâcher.
    fn effacer_saisie(&mut self) {
        let mut v = std::mem::take(&mut self.saisie).into_bytes();
        v.iter_mut().for_each(|b| *b = 0);
    }
}

impl App {
    /// A-t-on une clé pour ce service ? (Lu dans le coffre à la première question.)
    fn cle_presente(&mut self, service: Service) -> bool {
        if !self.sug.prefs.presence_lue {
            self.sug.prefs.presence_lue = true;
            for s in Service::ALL {
                let present = self.sug.coffre.presente(s);
                self.sug.prefs.presence.insert(s, present);
            }
        }
        self.sug.prefs.presence.get(&service).copied().unwrap_or(false)
    }

    /// Lance un essai de connexion sur un fil à part : `role` = `None` pour le serveur local.
    pub(crate) fn lancer_essai(&mut self, role: Option<Role>) {
        if self.sug.prefs.essai.is_some() {
            return;
        }
        let (tx, rx) = channel();
        let r = self.settings.generation_son.clone();
        match role {
            None => {
                std::thread::spawn(move || {
                    let _ = tx.send(crate::generation::tester_local(r.port_local, &Controle::new()));
                });
            }
            Some(role) => {
                let (coffre, transport, service) = (self.sug.coffre.clone(), self.sug.transport.clone(), r.service(role));
                std::thread::spawn(move || {
                    let _ = tx.send(crate::fournisseursapi::tester(service, coffre.as_ref(), transport.as_ref(), &Controle::new()));
                });
            }
        }
        self.sug.prefs.resultats.insert(role, "Essai en cours…".into());
        self.sug.prefs.essai = Some((role, rx));
    }

    pub(crate) fn suivre_essai(&mut self, ctx: &egui::Context) {
        let Some((role, rx)) = &self.sug.prefs.essai else { return };
        let role = *role;
        match rx.try_recv() {
            Ok(texte) => {
                self.sug.prefs.resultats.insert(role, texte);
                self.sug.prefs.essai = None;
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(std::time::Duration::from_millis(100)),
            Err(TryRecvError::Disconnected) => {
                self.sug.prefs.resultats.insert(role, "L'essai s'est interrompu.".into());
                self.sug.prefs.essai = None;
            }
        }
    }

    /// La section « Génération sonore » de la fenêtre Préférences.
    pub(crate) fn generation_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.suivre_essai(&ctx);
        egui::CollapsingHeader::new(RichText::new("Génération sonore").strong()).id_salt("prefs_generation_son").default_open(false).show(ui, |ui| self.generation_prefs_corps(ui));
    }

    /// Le contenu de la section : les trois rôles, le serveur local, la note de licences.
    pub(crate) fn generation_prefs_corps(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Le panneau Suggestions sonores lit le storyboard et propose des sons témoins. Chaque rôle a son fournisseur : le serveur local de cet ordinateur, ou un service en ligne.").small().weak());
        let mut change = false;
        for role in Role::ALL {
            ui.add_space(4.0);
            ui.label(RichText::new(role.nom()).strong());
            let actuel = self.settings.generation_son.mode(role);
            ui.horizontal(|ui| {
                let modes: &[Mode] = if role == Role::Lecture { &[Mode::Regles, Mode::Local, Mode::EnLigne] } else { &[Mode::Local, Mode::EnLigne] };
                for &m in modes {
                    if ui.selectable_label(actuel == m, m.nom()).clicked() && actuel != m {
                        self.settings.generation_son.definir(role, m);
                        change = true;
                    }
                }
            });
            match self.settings.generation_son.mode(role) {
                Mode::Regles => {
                    ui.label(RichText::new("Lit l'action, le dialogue et les notes des plans avec des mots-clés, sans modèle : rien à installer, rien n'est envoyé.").small().weak());
                }
                Mode::Local => {
                    ui.label(RichText::new(format!("{} par le serveur local (phase 2), sur 127.0.0.1.", role.modele_local())).small().weak());
                }
                Mode::EnLigne => change |= self.service_prefs_ui(ui, role),
            }
            if role == Role::Lecture && self.settings.generation_son.lecture != Mode::Regles {
                let r = ui.checkbox(&mut self.settings.generation_son.vignettes, "Joindre la vignette de chaque plan").on_hover_text("Le modèle voit l'image du plan en plus de ses textes. Avec un service en ligne, ces images partent hors de cet ordinateur.");
                change |= r.changed();
            }
            if role == Role::Voix {
                ui.horizontal(|ui| {
                    ui.label("Voix par défaut");
                    let (champ, indice) = if self.settings.generation_son.voix == Mode::EnLigne { (&mut self.settings.generation_son.voix_en_ligne, "identifiant d'une voix ElevenLabs") } else { (&mut self.settings.generation_son.voix_locale, "ff_siwis") };
                    let r = ui.add(egui::TextEdit::singleline(champ).hint_text(indice).desired_width(180.0));
                    if r.lost_focus() {
                        change = true;
                    }
                });
            }
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Serveur local").strong());
        ui.horizontal(|ui| {
            ui.label("Adresse 127.0.0.1, port");
            let r = ui.add(egui::DragValue::new(&mut self.settings.generation_son.port_local).range(1..=65535));
            if r.lost_focus() || r.drag_stopped() {
                change = true;
            }
            let occupe = self.sug.prefs.essai_en_cours();
            if ui.add_enabled(!occupe, egui::Button::new("Tester")).on_hover_text("Demande au serveur local s'il répond et quels modèles il a. Tant qu'il n'est pas installé : « non installé ».").clicked() {
                self.lancer_essai(None);
            }
        });
        if let Some(t) = self.sug.prefs.resultats.get(&None) {
            ui.label(RichText::new(t).small());
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Licences : les conditions d'utilisation de chaque service et de chaque modèle (en particulier l'usage commercial des sons générés et le sort des données envoyées) sont à vérifier par toi. Ce sont ici des sons témoins, pour l'animatique.").small().weak());
        if change {
            self.settings.generation_son = std::mem::take(&mut self.settings.generation_son).sanitized();
            let _ = self.settings.save();
        }
    }

    /// Le service en ligne d'un rôle : le choix, la clé, l'essai. Vrai si un réglage a changé.
    fn service_prefs_ui(&mut self, ui: &mut egui::Ui, role: Role) -> bool {
        let mut change = false;
        let service = self.settings.generation_son.service(role);
        if role.services().len() > 1 {
            ui.horizontal(|ui| {
                ui.label("Service");
                egui::ComboBox::from_id_salt(("service_generation", role.nom())).selected_text(service.nom()).show_ui(ui, |ui| {
                    for &s in role.services() {
                        if ui.selectable_label(service == s, s.nom()).clicked() && service != s {
                            self.settings.generation_son.definir_service(role, s);
                            change = true;
                        }
                    }
                });
            });
        } else {
            ui.label(RichText::new(format!("Service : {}", service.nom())).small());
        }
        if role == Role::Lecture {
            ui.horizontal(|ui| {
                ui.label("Modèle");
                let r = ui.add(egui::TextEdit::singleline(&mut self.settings.generation_son.modele_claude).desired_width(180.0));
                if r.lost_focus() {
                    change = true;
                }
            });
        }
        let service = self.settings.generation_son.service(role);
        let presente = self.cle_presente(service);
        ui.horizontal(|ui| {
            if ui.button(format!("{}  Saisir la clé…", ph::KEY)).on_hover_text("La clé est rangée dans le Gestionnaire d'identifiants de Windows, jamais dans un fichier de SHOT.").clicked() {
                self.sug.prefs.cle_pour = Some(service);
                self.sug.prefs.message_cle.clear();
            }
            let occupe = self.sug.prefs.essai_en_cours();
            if ui.add_enabled(!occupe, egui::Button::new("Tester la connexion")).clicked() {
                self.lancer_essai(Some(role));
            }
            ui.label(RichText::new(if presente { "clé enregistrée" } else { "aucune clé" }).small().weak());
        });
        if let Some(t) = self.sug.prefs.resultats.get(&Some(role)) {
            ui.label(RichText::new(t).small());
        }
        change
    }

    /// La fenêtre où l'on tape une clé. Ce qui est tapé n'est jamais montré, jamais écrit dans un
    /// fichier, et effacé en fermant.
    pub(crate) fn cle_window(&mut self, ctx: &egui::Context) {
        let Some(service) = self.sug.prefs.cle_pour else { return };
        let mut ouvert = true;
        let mut action: Option<u8> = None;
        egui::Window::new(format!("Clé de {}", service.nom())).open(&mut ouvert).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label("Colle la clé (le « API key » de ton compte). Elle est rangée dans le Gestionnaire d'identifiants de Windows ; SHOT ne l'écrit dans aucun fichier et ne l'affiche plus.");
            let r = ui.add(egui::TextEdit::singleline(&mut self.sug.prefs.saisie).password(true).hint_text("clé").desired_width(f32::INFINITY));
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                action = Some(0);
            }
            if !self.sug.prefs.message_cle.is_empty() {
                ui.label(RichText::new(self.sug.prefs.message_cle.clone()).small().color(ui.visuals().warn_fg_color));
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.sug.prefs.saisie.trim().is_empty(), egui::Button::new("Enregistrer")).clicked() {
                    action = Some(0);
                }
                if ui.button("Effacer la clé").on_hover_text("Retire la clé du Gestionnaire d'identifiants.").clicked() {
                    action = Some(1);
                }
                if ui.button("Annuler").clicked() {
                    action = Some(2);
                }
            });
        });
        let fermer = match action {
            Some(0) => self.valider_cle(service),
            Some(1) => self.effacer_cle(service),
            Some(_) => true,
            None => !ouvert,
        };
        if fermer {
            self.fermer_cle();
        }
    }

    /// Range la clé tapée dans le coffre. Vrai si c'est fait (la fenêtre se ferme) ; sinon le coffre dit pourquoi.
    pub(crate) fn valider_cle(&mut self, service: Service) -> bool {
        if self.sug.prefs.saisie.trim().is_empty() {
            self.sug.prefs.message_cle = "La clé est vide.".into();
            return false;
        }
        let cle = Cle::new(&self.sug.prefs.saisie);
        let r = self.sug.coffre.ecrire(service, &cle);
        drop(cle);
        match r {
            Ok(()) => {
                self.sug.prefs.presence.insert(service, true);
                self.status = format!("Clé de {} enregistrée dans le Gestionnaire d'identifiants.", service.court());
                self.fermer_cle();
                true
            }
            Err(e) => {
                self.sug.prefs.message_cle = e;
                false
            }
        }
    }

    pub(crate) fn effacer_cle(&mut self, service: Service) -> bool {
        match self.sug.coffre.effacer(service) {
            Ok(()) => {
                self.sug.prefs.presence.insert(service, false);
                self.status = format!("Clé de {} effacée.", service.court());
                self.fermer_cle();
                true
            }
            Err(e) => {
                self.sug.prefs.message_cle = e;
                false
            }
        }
    }

    /// Ferme la fenêtre de saisie : ce qui était tapé est effacé.
    fn fermer_cle(&mut self) {
        self.sug.prefs.effacer_saisie();
        self.sug.prefs.cle_pour = None;
    }
}
