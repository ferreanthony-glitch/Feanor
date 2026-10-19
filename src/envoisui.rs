//! Les envois dans l'interface (`envois.rs` pour le modèle et le fichier) : marquer un envoi, la liste des
//! envois.
//!
//! **Marquer un envoi** est une commande explicite, pas un geste : elle copie d'un coup tous les plans et
//! tous les dessins (les tuiles d'un bitmap sont partagées, une copie ne recopie aucun pixel ; seul un
//! dessin vectoriel est copié en entier : quelques millisecondes, mesurées dans `envois_tests`), puis **un
//! fil** hache les dessins, bâtit l'instantané et rend la main ; le projet est enregistré dès que
//! l'instantané est prêt (les fichiers de `versions/` s'écrivent sur l'autre fil de l'enregistrement,
//! `enregistrement.rs`). Rien de cela ne bloque l'interface.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, vec2, Align2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::empreinte::Donnees;
use crate::envois::{self, Copie, Demande, NouvelEnvoi, PlanCopie, Resultat};
use crate::theme;

/// Un envoi en cours de marquage : le fil bâtit l'instantané.
struct Marquage {
    rx: Receiver<Result<Resultat, String>>,
}

/// La fenêtre « Marquer un envoi ».
pub(crate) struct NouveauEnvoi {
    nom: String,
    date: String,
    focus: bool,
}

/// L'état de l'interface des envois (un seul champ dans `App`).
#[derive(Default)]
pub(crate) struct EnvoisUi {
    /// Le projet (`App::doc_epoch`) pour lequel il vaut.
    epoch: u64,
    marquage: Option<Marquage>,
    pub(crate) nouveau: Option<NouveauEnvoi>,
    pub(crate) liste: bool,
    /// Envoi dont on confirme le retrait de la liste.
    retrait: Option<u64>,
    /// Durée de la dernière copie d'un marquage sur le fil de l'interface (profil).
    pub(crate) derniere_copie: Option<Duration>,
}

impl EnvoisUi {
    /// Un fil travaille (essais : attendre).
    #[cfg(test)]
    pub(crate) fn occupe(&self) -> bool {
        self.marquage.is_some()
    }

    /// Rien à faire (essais).
    #[cfg(test)]
    pub(crate) fn au_repos(&self) -> bool {
        self.marquage.is_none()
    }
}

/// Un numéro pour nommer le fichier d'un instantané (les envois de deux dossiers qui se mêlent ne se
/// confondent pas).
fn unique() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    (t ^ (t >> 32) ^ u64::from(std::process::id()).rotate_left(16)) as u32
}

impl App {
    /// Les plans à examiner, copiés : leur titre et leur version d'aujourd'hui.
    fn envois_plans(&self) -> Vec<PlanCopie> {
        self.doc
            .panels
            .iter()
            .enumerate()
            .map(|(i, p)| PlanCopie { panel: p.clone(), titre: crate::storyboard::panel_title(&self.doc, i), version: self.doc.numero_de_version(p.id) })
            .collect()
    }

    /// Une image d'écran des envois : le résultat d'un marquage, s'il est arrivé.
    pub(crate) fn envois_tick(&mut self, ctx: &egui::Context) {
        let _t = crate::perf::section("envois");
        if self.envois_ui.epoch != self.doc_epoch {
            self.envois_ui = EnvoisUi { epoch: self.doc_epoch, ..Default::default() };
        }
        if let Some(m) = self.envois_ui.marquage.take() {
            match m.rx.try_recv() {
                Ok(Ok(r)) => self.envois_fini(ctx, r),
                Ok(Err(e)) => self.status = format!("Envoi : échec ({e})."),
                Err(TryRecvError::Empty) => {
                    self.envois_ui.marquage = Some(m);
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
                Err(TryRecvError::Disconnected) => self.status = "Envoi : le calcul s'est arrêté.".into(),
            }
        }
    }

    // ------------------------------------------------------------------ marquer un envoi

    /// Marque un envoi : l'état de chaque plan, tout de suite (copie), puis l'instantané sur un fil.
    pub(crate) fn envois_marquer(&mut self, ctx: &egui::Context, nom: &str, date: u64) {
        if self.envois_ui.marquage.is_some() {
            self.status = "Un envoi est déjà en préparation.".into();
            return;
        }
        self.end_stroke();
        let start = Instant::now();
        let plans = self.envois_plans();
        let mut cels = HashMap::new();
        for p in &self.doc.panels {
            for id in crate::empreinte::dessins_du_plan(p) {
                if let Some(c) = self.doc.cels.get(&id) {
                    cels.entry(id).or_insert_with(|| Copie { version: c.version, donnees: Donnees::de(c) });
                }
            }
        }
        let note_le = crate::recovery::now_secs();
        let demande = Demande {
            plans,
            cels,
            connus: HashMap::new(),
            tete: crate::project::Tete::de(&self.doc),
            envoi: Some(NouvelEnvoi { id: self.doc.envois.prochain_numero(), nom: nom.trim().to_string(), date, note_le, unique: unique() }),
        };
        self.envois_ui.derniere_copie = Some(start.elapsed());
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = ctx.clone();
        let started = std::thread::Builder::new().name("envoi".into()).spawn(move || {
            let r = envois::construire(demande);
            let _ = tx.send(r);
            wake.request_repaint();
        });
        match started {
            Ok(_) => {
                self.envois_ui.marquage = Some(Marquage { rx });
                self.status = format!("Envoi « {} » : état des plans en cours de relevé…", nom.trim());
            }
            Err(e) => self.status = format!("Envoi impossible : {e}"),
        }
    }

    /// L'instantané est prêt : l'envoi entre dans le projet, et le projet est enregistré.
    fn envois_fini(&mut self, ctx: &egui::Context, r: Resultat) {
        let Some((envoi, instantane)) = r.envoi else { return };
        let (n, titre) = (envoi.plans.len(), envoi.libelle());
        self.doc.envois.ajouter(envoi, instantane);
        self.modified = true;
        if self.path.is_some() {
            self.save();
            self.status = format!("Envoi « {titre} » marqué : {n} plan{} notés. Le projet s'enregistre.", if n > 1 { "s" } else { "" });
        } else {
            self.status = format!("Envoi « {titre} » marqué : {n} plan{} notés. Enregistre le projet pour le garder.", if n > 1 { "s" } else { "" });
        }
        ctx.request_repaint();
    }

    /// Ouvre la fenêtre « Marquer un envoi », avec un nom et la date du jour.
    pub(crate) fn envois_ouvrir_marquage(&mut self) {
        let n = self.doc.envois.prochain_numero();
        self.envois_ui.nouveau = Some(NouveauEnvoi { nom: format!("v{n} au réal"), date: envois::date_courte(crate::recovery::now_secs()), focus: true });
    }

    /// Les commandes `envoi.*` ; vrai si `action` en était une.
    pub(crate) fn envois_action(&mut self, action: &str) -> bool {
        match action {
            "envoi.marquer" => self.envois_ouvrir_marquage(),
            "envoi.liste" => self.envois_ui.liste = !self.envois_ui.liste,
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ les fenêtres

    /// Les fenêtres des envois : le marquage et la liste.
    pub(crate) fn envois_windows(&mut self, ctx: &egui::Context) {
        self.envois_fenetre_marquage(ctx);
        self.envois_fenetre_liste(ctx);
    }

    fn envois_fenetre_marquage(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.envois_ui.nouveau.take() else { return };
        let mut fermer = false;
        let mut valider = false;
        let date = envois::lire_date(&dlg.date);
        let nom_ok = !dlg.nom.trim().is_empty();
        egui::Window::new("Marquer un envoi").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| {
            ui.set_max_width(380.0);
            let dernier = self.doc.envois.dernier().map(|e| e.libelle());
            ui.label(format!(
                "SHOT note l'état des {} plans du film tel qu'il est maintenant. Les plans qui changeront ensuite pourront être repérés.",
                self.doc.panels.len()
            ));
            if let Some(d) = dernier {
                ui.label(RichText::new(format!("Dernier envoi : {d}.")).color(theme::texte_doux()));
            }
            ui.add_space(4.0);
            egui::Grid::new("envoi_champs").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Nom");
                let nom = ui.add(egui::TextEdit::singleline(&mut dlg.nom).desired_width(240.0).hint_text("v2 au réal"));
                if std::mem::take(&mut dlg.focus) {
                    nom.request_focus();
                }
                if nom.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && nom_ok && date.is_some() {
                    valider = true;
                }
                ui.end_row();
                ui.label("Date");
                let champ = ui.add(egui::TextEdit::singleline(&mut dlg.date).desired_width(110.0).hint_text("JJ/MM/AAAA"));
                if champ.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && nom_ok && date.is_some() {
                    valider = true;
                }
                if date.is_none() {
                    ui.label(RichText::new("jour/mois/année").color(theme::erreur()));
                }
                ui.end_row();
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = nom_ok && date.is_some();
                let r = ui.add_enabled(ok, egui::Button::new(format!("{}  Marquer l'envoi", ph::PAPER_PLANE_TILT)));
                if r.clicked() {
                    valider = true;
                }
                if ui.button("Annuler").clicked() {
                    fermer = true;
                }
            });
            if self.path.is_none() {
                ui.label(RichText::new("Le projet n'est pas encore enregistré : l'envoi sera gardé au premier enregistrement.").color(theme::texte_doux()));
            }
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            fermer = true;
        }
        if valider {
            if let Some(d) = date.filter(|_| nom_ok) {
                let nom = dlg.nom.clone();
                self.envois_marquer(ctx, &nom, d);
                return;
            }
        }
        if !fermer {
            self.envois_ui.nouveau = Some(dlg);
        }
    }

    fn envois_fenetre_liste(&mut self, ctx: &egui::Context) {
        if !self.envois_ui.liste {
            return;
        }
        let mut ouverte = true;
        let mut retirer: Option<u64> = None;
        egui::Window::new("Envois").open(&mut ouverte).default_width(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if bulles::icone(ui, ph::PAPER_PLANE_TILT, "Marquer un envoi…", "Noter l'état de tous les plans.", &self.raccourci("envoi.marquer")).clicked() {
                    self.envois_ouvrir_marquage();
                }
            });
            ui.separator();
            if self.doc.envois.is_empty() {
                ui.label("Aucun envoi pour l'instant. « Marquer un envoi » note l'état de tous les plans (par exemple « v2 au réal »).");
                return;
            }
            ui.label(RichText::new("Les envois").strong());
            let mut lignes: Vec<&envois::Envoi> = self.doc.envois.list.iter().collect();
            lignes.sort_by_key(|e| std::cmp::Reverse((e.date, e.id)));
            egui::ScrollArea::vertical().id_salt("envois_liste").max_height(260.0).show(ui, |ui| {
                for e in lignes {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(e.libelle()).strong());
                        ui.label(RichText::new(format!("{} plans", e.plans.len())).color(theme::texte_doux()));
                        if bulles::icone(ui, ph::TRASH, "Retirer de la liste", "L'envoi sort de la liste. Ses fichiers dans « versions » ne sont jamais effacés.", "").clicked() {
                            retirer = Some(e.id);
                        }
                    });
                }
            });
            if let Some(id) = self.envois_ui.retrait {
                ui.separator();
                let nom = self.doc.envois.trouve(id).map(|e| e.libelle()).unwrap_or_default();
                ui.label(format!("Retirer « {nom} » de la liste ?"));
                ui.horizontal(|ui| {
                    if ui.button("Retirer").clicked() {
                        self.doc.envois.retirer(id);
                        self.modified = true;
                        self.envois_ui.retrait = None;
                        self.status = "Envoi retiré de la liste (ses fichiers restent dans « versions »).".into();
                    }
                    if ui.button("Garder").clicked() {
                        self.envois_ui.retrait = None;
                    }
                });
            }
        });
        if let Some(id) = retirer {
            self.envois_ui.retrait = Some(id);
        }
        self.envois_ui.liste = ouverte;
    }
}
