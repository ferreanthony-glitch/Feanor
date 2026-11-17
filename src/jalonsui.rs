//! Les jalons dans l'application (2026-11-10, proposition 25 d'Anthony) : le jalon de l'ouverture, Ctrl+S, la fenêtre
//! « Jalons » et le retour à un jalon. Les fichiers sont l'affaire de `jalons.rs`, l'écriture continue celle de `continu.rs`.
//!
//! - **À l'ouverture** d'un projet que FEANOR écrit en continu, un jalon automatique est posé sur un autre fil, avant la
//!   première écriture (`continu_retenu` l'attend) : le projet tel qu'on l'a ouvert reste là, même si l'écriture le passe à un
//!   format plus récent. Rien n'est posé si le projet n'a pas changé depuis son dernier jalon. S'il ne peut pas être posé,
//!   l'écriture continue se met en pause et le dit.
//! - **Ctrl+S** écrit ce qui reste à écrire, puis pose un jalon ; la barre d'état propose de le nommer.
//! - **Revenir à un jalon** : tout est écrit, un jalon de l'état d'aujourd'hui est posé, l'onglet se ferme, les fichiers sont
//!   remis (`jalons::restaurer`), le projet se rouvre. Rien de ce que la séance tenait en mémoire ne survit au retour : c'est une
//!   ouverture comme une autre.

use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use eframe::egui::{self, RichText};

use crate::app::App;
use crate::jalons::{self, Jalon};

/// Pourquoi un jalon se pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Raison {
    /// Le projet vient de s'ouvrir.
    Ouverture,
    /// Ctrl+S.
    Demande,
}

/// Un jalon qui se pose sur un autre fil.
pub(crate) struct Pose {
    fil: JoinHandle<Result<Jalon, String>>,
    raison: Raison,
}

/// La fenêtre « Jalons » d'un projet.
#[derive(Default)]
pub(crate) struct Fenetre {
    /// Les jalons lus sur le disque, du plus récent au plus ancien.
    liste: Vec<Jalon>,
    /// Le nom en cours de saisie de chacun (par dossier).
    noms: std::collections::HashMap<PathBuf, String>,
    /// Le jalon auquel on a demandé de revenir : la fenêtre demande confirmation.
    retour: Option<PathBuf>,
    /// Le jalon dont le nom prend le curseur à la prochaine image.
    focus: Option<PathBuf>,
}

impl Fenetre {
    fn relire(&mut self, projet: &Path) {
        self.liste = jalons::lister(projet);
        self.noms = self.liste.iter().map(|j| (j.dossier.clone(), j.fiche.nom.clone())).collect();
        if self.retour.as_ref().is_some_and(|r| !self.liste.iter().any(|j| &j.dossier == r)) {
            self.retour = None;
        }
    }
}

/// « 10/11/2026 à 19:42 ».
fn date(j: &Jalon) -> String {
    crate::remplacer::date_locale(Some(j.fiche.quand))
}

impl App {
    /// Le dossier où le projet affiché est écrit, s'il en a un (pas celui d'un ancien fichier importé : rien n'y est écrit).
    fn dossier_ecrit(&self) -> Option<PathBuf> {
        self.path.as_deref().and_then(Path::parent).map(Path::to_path_buf)
    }

    /// Lance la pose d'un jalon sur un autre fil. Le projet ne s'écrit pas pendant ce temps (`continu_retenu`, `attendre_le_jalon`).
    fn jalon_lancer(&mut self, nom: &str, auto: bool, raison: Raison) {
        let Some(manifeste) = self.path.clone() else { return };
        let (nom, reveil) = (nom.to_string(), self.ctx.clone());
        let fil = std::thread::Builder::new().name("jalon".into()).spawn(move || {
            crate::parallele::fil_et_ses_fils_sous_la_normale();
            let pose = jalons::poser(&manifeste, &nom, auto, crate::recovery::now_secs()).map(|(j, _)| j);
            if let (Ok(_), Some(projet)) = (&pose, manifeste.parent()) {
                jalons::elaguer(projet);
            }
            reveil.request_repaint();
            pose
        });
        match fil {
            Ok(fil) => self.continu.jalon = Some(Pose { fil, raison }),
            Err(e) => self.jalon_fini(raison, Err(e.to_string())),
        }
    }

    /// Le projet vient de s'ouvrir : son jalon d'ouverture, s'il a changé depuis le dernier.
    pub(crate) fn jalon_d_ouverture(&mut self) {
        if !self.continu_actif() || self.continu.jalon.is_some() {
            return;
        }
        let Some(manifeste) = self.path.clone() else { return };
        if !jalons::a_change_depuis_le_dernier(&manifeste) {
            return;
        }
        self.jalon_lancer("À l'ouverture", true, Raison::Ouverture);
    }

    /// Attend la fin du jalon qui se pose (une écriture du projet va partir : elle ne doit pas lui passer dessous).
    pub(crate) fn attendre_le_jalon(&mut self) {
        if let Some(Pose { fil, raison }) = self.continu.jalon.take() {
            let pose = fil.join().unwrap_or_else(|_| Err("la pose du jalon a planté".into()));
            self.jalon_fini(raison, pose);
        }
    }

    /// Le jalon est posé, ou ne l'a pas pu être.
    fn jalon_fini(&mut self, raison: Raison, pose: Result<Jalon, String>) {
        match (raison, pose) {
            (Raison::Ouverture, Ok(_)) => {}
            (Raison::Ouverture, Err(e)) => {
                self.continu.pause = Some(format!("le jalon d'ouverture n'a pas pu être posé ({e})"));
                self.status = format!("Jalon d'ouverture impossible : {e}. FEANOR n'écrit pas ce projet tout seul ; Ctrl+S l'enregistre.");
            }
            (Raison::Demande, Ok(j)) => {
                let (_, _, _, h, mi) = crate::remplacer::heure_locale(j.fiche.quand);
                let msg = format!("Jalon posé à {h:02}:{mi:02}. Tu peux y revenir par Fichier > Jalons…");
                self.status_avec_action(msg, crate::messages::ActionMessage::NommerJalon(j.dossier.clone()));
                if let (Some(f), Some(projet)) = (self.jalons_ui.as_mut(), j.dossier.parent().and_then(Path::parent)) {
                    f.relire(projet);
                }
            }
            (Raison::Demande, Err(e)) => self.status = format!("Jalon impossible : {e}"),
        }
    }

    /// À chaque image d'écran : le jalon qui se posait est adopté ; celui que Ctrl+S a demandé part dès que le projet est écrit.
    pub(crate) fn tick_jalon(&mut self, ctx: &egui::Context) {
        if self.continu.jalon.as_ref().is_some_and(|p| p.fil.is_finished()) {
            self.attendre_le_jalon();
        }
        if self.continu.jalon.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        if !self.continu.jalon_demande {
            return;
        }
        if self.save_error.is_some() || !self.continu_actif() {
            // (L'écriture a échoué : sa fenêtre le dit, le jalon ne garderait pas ce qu'on voit.)
            self.continu.jalon_demande = false;
            return;
        }
        if self.saving.is_some() {
            return;
        }
        if self.modified {
            // (Modifié depuis la demande, ou l'écriture attendait un calcul : on écrit d'abord.)
            if self.stroke.is_none() {
                self.continu_ecrire();
            }
            return;
        }
        self.continu.jalon_demande = false;
        self.jalon_lancer("", false, Raison::Demande);
    }

    /// Ctrl+S quand FEANOR écrit en continu : ce qui reste à écrire s'écrit, puis un jalon est posé.
    pub(crate) fn jalon_demander(&mut self) {
        self.end_stroke();
        if self.continu.jalon_demande {
            return;
        }
        self.continu.jalon_demande = true;
        self.status = "Jalon…".into();
        if self.modified && self.saving.is_none() {
            self.continu_ecrire();
        }
    }

    /// Ouvre la fenêtre « Jalons » du projet affiché.
    pub(crate) fn jalons_ouvrir(&mut self, focus: Option<PathBuf>) {
        let Some(projet) = self.dossier_ecrit() else {
            self.status = "Les jalons viennent avec le dossier du projet : enregistre-le d'abord (Fichier > Enregistrer sous…).".into();
            return;
        };
        let mut f = Fenetre { focus, ..Default::default() };
        f.relire(&projet);
        self.jalons_ui = Some(f);
    }

    /// Revient au jalon `dossier` : tout est écrit, l'état d'aujourd'hui est gardé dans un jalon, puis le projet reprend les
    /// fichiers du jalon et se rouvre.
    pub(crate) fn jalon_revenir(&mut self, dossier: &Path) {
        let (Some(manifeste), Some(projet)) = (self.path.clone(), self.dossier_ecrit()) else { return };
        let Some(cible) = jalons::lister(&projet).into_iter().find(|j| j.dossier == dossier) else {
            self.status = "Ce jalon n'est plus sur le disque.".into();
            return;
        };
        if self.partage.is_some() || jalons::est_dans_un_jalon(&manifeste) {
            self.status = "On ne revient pas à un jalon dans un film partagé, ni depuis un jalon ouvert.".into();
            return;
        }
        if let Some(quoi) = self.onglet_occupe() {
            self.status = format!("Attends la fin de {quoi} avant de revenir à un jalon.");
            return;
        }
        self.end_stroke();
        self.attendre_le_jalon();
        // 1. Tout ce qui est en mémoire va sur le disque : le jalon de l'état d'aujourd'hui doit l'avoir.
        if self.modified && self.saving.is_none() && !self.start_save_avec(crate::project::manifeste_shot(&manifeste), None, true) {
            return;
        }
        if !self.wait_save() || self.modified {
            self.status = "Retour au jalon annulé : le projet n'a pas pu être écrit d'abord.".into();
            return;
        }
        let manifeste = self.path.clone().unwrap_or(manifeste);
        // 2. Un jalon de l'état d'aujourd'hui, toujours : un retour se défait.
        let titre = cible.titre();
        if let Err(e) = jalons::poser(&manifeste, &format!("Avant le retour à « {titre} »"), true, crate::recovery::now_secs()) {
            self.status = format!("Retour au jalon impossible : l'état d'aujourd'hui n'a pas pu être gardé ({e}). Rien n'a changé.");
            return;
        }
        // 3. L'onglet se ferme (rien de la séance ne doit regarder des fichiers qui changent), les fichiers sont remis, le projet se rouvre.
        self.jalons_ui = None;
        self.fermer_l_onglet_affiche();
        let retour = jalons::restaurer(&projet, &cible.dossier);
        jalons::elaguer(&projet);
        if !self.onglet_vierge() && !self.sans_projet() {
            // (Le voisin est passé à l'écran : le projet se rouvre dans un onglet à lui, jamais à la place d'un autre.)
            self.nouvel_onglet();
        }
        // (Le manifeste a pu reprendre son ancien nom : un jalon d'avant le passage de `.pelure` à `.shot`.)
        self.open_path(crate::project::manifeste_a_ouvrir(&manifeste));
        self.status = match retour {
            Ok(()) => format!("Revenu au jalon « {titre} » du {}. L'état d'avant est gardé dans un jalon.", date(&cible)),
            Err(e) => format!("Le retour au jalon « {titre} » n'a pas pu se finir : {e}. L'état d'avant est gardé dans le jalon « Avant le retour à « {titre} » »."),
        };
    }

    /// La fenêtre « Jalons ».
    pub(crate) fn jalons_fenetre(&mut self, ctx: &egui::Context) {
        let Some(mut f) = self.jalons_ui.take() else { return };
        let Some(projet) = self.dossier_ecrit() else { return };
        let mut ouverte = true;
        let (mut poser, mut revenir, mut supprimer, mut renommer): (bool, Option<PathBuf>, Option<PathBuf>, Option<(PathBuf, String)>) = (false, None, None, None);
        let continu = self.continu_actif();
        let raccourci = self.raccourci("fichier.enregistrer");
        egui::Window::new("Jalons").open(&mut ouverte).collapsible(false).resizable(true).default_width(520.0).show(ctx, |ui| {
            ui.label("Un jalon garde le projet tel qu'il est à un moment. Tu peux y revenir : l'état d'aujourd'hui est alors gardé lui aussi.");
            ui.label(RichText::new("FEANOR en pose un à chaque ouverture et garde les dix derniers, avec tes trente derniers jalons sans nom. Un jalon que tu nommes reste jusqu'à ce que tu le supprimes.").small().weak());
            ui.add_space(6.0);
            let libelle = if raccourci.is_empty() { "Poser un jalon maintenant".to_string() } else { format!("Poser un jalon maintenant ({raccourci})") };
            if ui.add_enabled(continu, egui::Button::new(libelle)).on_disabled_hover_text("FEANOR n'écrit pas ce projet en continu (film partagé, jalon ouvert, réglage décoché ou écriture en pause).").clicked() {
                poser = true;
            }
            ui.add_space(6.0);
            if f.liste.is_empty() {
                ui.label(RichText::new("Aucun jalon pour l'instant.").weak());
            }
            egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                for j in &f.liste {
                    ui.separator();
                    ui.horizontal(|ui| {
                        let nom = f.noms.entry(j.dossier.clone()).or_default();
                        let champ = ui.add(egui::TextEdit::singleline(nom).hint_text(j.titre()).desired_width(220.0)).on_hover_text("Le nom du jalon. Un jalon que tu nommes n'est jamais retiré tout seul.");
                        if f.focus.as_ref() == Some(&j.dossier) {
                            champ.request_focus();
                            f.focus = None;
                        }
                        if champ.lost_focus() && nom.trim() != j.fiche.nom.trim() {
                            renommer = Some((j.dossier.clone(), nom.clone()));
                        }
                        ui.label(RichText::new(date(j)).small());
                        if j.fiche.auto {
                            ui.label(RichText::new("automatique").small().weak()).on_hover_text("Posé par FEANOR. Seuls les dix jalons automatiques les plus récents sont gardés.");
                        }
                    });
                    ui.horizontal(|ui| {
                        if f.retour.as_ref() == Some(&j.dossier) {
                            ui.label(format!("Le projet reprend son état du {}. Celui d'aujourd'hui est gardé dans un jalon.", date(j)));
                            if ui.button("Revenir").clicked() {
                                revenir = Some(j.dossier.clone());
                            }
                            if ui.button("Annuler").clicked() {
                                f.retour = None;
                            }
                        } else {
                            if ui.add_enabled(continu, egui::Button::new("Revenir à ce jalon…")).clicked() {
                                f.retour = Some(j.dossier.clone());
                            }
                            if ui.button("Supprimer").on_hover_text("Supprime ce jalon. Le projet lui-même ne change pas.").clicked() {
                                supprimer = Some(j.dossier.clone());
                            }
                        }
                    });
                }
            });
        });
        if let Some((dossier, nom)) = renommer {
            match jalons::renommer(&dossier, &nom) {
                Ok(_) => self.status = "Jalon renommé.".into(),
                Err(e) => self.status = format!("Jalon : renommage impossible ({e})"),
            }
            f.relire(&projet);
        }
        if let Some(dossier) = supprimer {
            match jalons::supprimer(&dossier) {
                Ok(()) => self.status = "Jalon supprimé.".into(),
                Err(e) => self.status = format!("Jalon : suppression impossible ({e})"),
            }
            f.relire(&projet);
        }
        if ouverte {
            self.jalons_ui = Some(f);
        }
        if poser {
            self.jalon_demander();
        }
        if let Some(dossier) = revenir {
            self.jalon_revenir(&dossier);
        }
    }
}
