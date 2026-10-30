//! Le parcours « Premier jour » (passe « premier contact », 2026-10-30, `docs/INTERFACE.md` partie 89) : **une seule**
//! visite rapide, proposée une fois, à la place de la pluie de bulles du premier lancement.
//!
//! - **La proposition** est discrète : une ligne au bas de l'accueil (« Visite rapide (2 min) · Plus tard · Ne plus proposer »),
//!   ou une petite carte en bas à droite si un projet s'ouvre tout seul au lancement. Elle ne vient que sur une machine neuve
//!   (aucun conseil vu, aucun diagnostic des performances), jamais dans les essais, jamais par-dessus une fenêtre.
//! - **« Plus tard »** la rend au lancement suivant (deux propositions au plus en tout) ; **« Ne plus proposer »** l'éteint pour de bon. Dans les deux
//!   cas, Aide > Visite rapide « Premier jour »… la relance quand on veut.
//! - **Le parcours** a cinq étapes, dans l'ordre du travail : créer un projet, dessiner des vignettes, mettre une durée, écouter
//!   l'animatique, exporter. Chaque étape est une petite carte (un bouton qui ouvre l'endroit, « Suivant », « Quitter la visite ») ;
//!   la dernière offre aussi « Régler SHOT pour cette machine… » (le diagnostic des performances, qui ne s'ouvre plus tout seul).
//!   L'étape « créer un projet » se coche d'elle-même quand un projet est ouvert.
//! - **Retenu** dans les réglages de l'application (`reglages.toml`, clé `premier_jour`) : la réponse et l'étape où l'on en est. Rien
//!   n'est écrit dans le projet, et la visite ne change rien au projet : elle ouvre des endroits, c'est tout.
//! - La carte attend : elle ne se montre pas par-dessus une fenêtre ou une boîte de dialogue (elle revient après), et les conseils
//!   contextuels (`conseils.rs`) attendent qu'elle soit répondue ou finie.

use eframe::egui::{self, Align2, RichText};
use egui_phosphor::regular as ph;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::theme;

/// Combien de propositions au plus en tout : la première, puis une de plus après un « Plus tard ».
pub const PROPOSITIONS_MAX: u8 = 2;

/// Où en est le parcours sur ce poste.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Etat {
    /// Jamais répondu.
    #[default]
    Inconnu,
    /// « Plus tard » : la proposition revient au lancement suivant (`PROPOSITIONS_MAX` fois au plus).
    PlusTard,
    /// « Ne plus proposer » : plus jamais d'office.
    NePlusProposer,
    /// Le parcours est commencé : la carte de l'étape se montre (même après un redémarrage).
    EnCours,
    /// Terminé ou quitté.
    Fini,
}

/// Ce que ce poste a retenu du parcours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reglage {
    #[serde(default)]
    pub etat: Etat,
    /// L'étape en cours (0 à 4) quand le parcours est commencé.
    #[serde(default)]
    pub etape: usize,
    /// Combien de fois la proposition a été faite puis repoussée.
    #[serde(default)]
    pub propositions: u8,
}

impl Reglage {
    /// Rien de retenu : la clé n'est pas écrite dans le fichier de réglages.
    pub fn est_vide(&self) -> bool {
        *self == Reglage::default()
    }

    /// La proposition se fait-elle au lancement ? `machine_neuve` : aucun conseil vu et aucun diagnostic (une machine où SHOT
    /// a déjà servi n'a pas besoin de visite). « Plus tard » revient, deux fois au plus.
    pub fn a_proposer(&self, machine_neuve: bool) -> bool {
        match self.etat {
            Etat::Inconnu => machine_neuve,
            Etat::PlusTard => self.propositions < PROPOSITIONS_MAX,
            Etat::NePlusProposer | Etat::EnCours | Etat::Fini => false,
        }
    }
}

/// Une étape du parcours.
pub struct Etape {
    pub titre: &'static str,
    pub texte: &'static str,
    /// Le bouton qui ouvre l'endroit, et la commande qu'il lance.
    pub bouton: &'static str,
    pub icone: &'static str,
    pub action: &'static str,
}

/// Les cinq étapes, dans l'ordre du travail.
pub const ETAPES: [Etape; 5] = [
    Etape {
        titre: "Créer un projet",
        texte: "Donne un nom à ton projet puis « Créer » : SHOT te propose un format, une cadence et un dossier.",
        bouton: "Nouveau projet…",
        icone: ph::FILE_PLUS,
        action: "fichier.nouveau",
    },
    Etape {
        titre: "Dessiner des vignettes",
        texte: "Dessine directement dans les cases de la planche : chaque case est un plan.",
        bouton: "Ouvrir les Vignettes",
        icone: ph::SQUARES_FOUR,
        action: "espace.vignettes",
    },
    Etape {
        titre: "Mettre une durée",
        texte: "Dans le Montage, tire le bord d'un plan pour changer sa durée ; elle se règle aussi dans la Fiche du plan.",
        bouton: "Ouvrir le Montage",
        icone: ph::FILM_STRIP,
        action: "espace.montage",
    },
    Etape {
        titre: "Écouter l'animatique",
        texte: "Lance la lecture : les plans défilent à leur durée, avec le son du projet.",
        bouton: "Lecture / pause",
        icone: ph::PLAY,
        action: "lecture.basculer",
    },
    Etape {
        titre: "Exporter",
        texte: "Une vidéo, des images ou un PDF pour montrer ton travail : choisis un format et « Exporter ».",
        bouton: "Exporter…",
        icone: ph::EXPORT,
        action: "fichier.exporter",
    },
];

/// Ce que l'on peut répondre à la proposition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reponse {
    Visite,
    PlusTard,
    NePlusProposer,
}

impl App {
    /// Le parcours est-il commencé (la carte d'une étape se montre) ?
    pub(crate) fn visite_en_cours(&self) -> bool {
        self.settings.premier_jour.etat == Etat::EnCours
    }

    /// La proposition attend-elle une réponse (au lancement, pas encore répondue dans cette séance) ?
    pub(crate) fn visite_offerte(&self) -> bool {
        self.confort.visite_offre
    }

    /// Quelque chose de la visite est-il à l'écran ou en attente ? Les conseils contextuels laissent la place.
    pub(crate) fn visite_occupe(&self) -> bool {
        self.visite_en_cours() || self.visite_offerte()
    }

    /// Réponse à la proposition. La visite commence à l'étape 1 (ou 2 si un projet est déjà ouvert).
    pub(crate) fn visite_repondre(&mut self, r: Reponse) {
        self.confort.visite_offre = false;
        let p = &mut self.settings.premier_jour;
        match r {
            Reponse::Visite => {
                p.etat = Etat::EnCours;
                p.etape = 0;
            }
            Reponse::PlusTard => {
                p.etat = Etat::PlusTard;
                p.propositions = p.propositions.saturating_add(1);
            }
            Reponse::NePlusProposer => p.etat = Etat::NePlusProposer,
        }
        let _ = self.settings.save();
    }

    /// Aide > Visite rapide « Premier jour »… : le parcours repart du début, à tout moment.
    pub(crate) fn visite_lancer(&mut self) {
        self.visite_repondre(Reponse::Visite);
        self.status = "Visite rapide : cinq étapes, la carte en bas à droite te guide.".into();
    }

    /// Quitte le parcours (terminé ou abandonné) : plus rien d'office, Aide le relance.
    pub(crate) fn visite_finir(&mut self) {
        let p = &mut self.settings.premier_jour;
        p.etat = Etat::Fini;
        p.etape = 0;
        let _ = self.settings.save();
    }

    /// Une fois par image : l'étape « créer un projet » se coche quand un projet est ouvert.
    pub(crate) fn visite_suivi(&mut self) {
        if self.visite_en_cours() && self.settings.premier_jour.etape == 0 && !self.sans_projet() {
            self.settings.premier_jour.etape = 1;
            let _ = self.settings.save();
        }
    }

    /// Une fenêtre de l'utilisateur est-elle ouverte (en plus des boîtes de dialogue de `modal_open`) : Préférences, palette de
    /// commandes, liste des raccourcis, diagnostic, réglage du stylet, messages, réglages de la pelure ou des miroirs, groupes ?
    /// Les bulles de la visite et des conseils ne se posent jamais par-dessus.
    pub(crate) fn fenetre_ouverte(&self) -> bool {
        self.prefs_window
            || self.palette.is_some()
            || self.show_help
            || self.diag.ouvert
            || self.confort.stylet.is_some()
            || self.messages_window
            || self.onion_window
            || self.flips_window
            || self.properties_window
            || self.groups_window
    }

    /// La carte peut-elle se montrer : rien d'autre ne tient l'écran (accueil, fenêtre, boîte de dialogue, diagnostic).
    fn visite_libre(&self) -> bool {
        !self.welcome_shown() && !self.modal_open() && !self.fenetre_ouverte() && !self.diag.offre
    }

    /// La ligne de l'accueil (sans projet) : la proposition, ou l'étape 1 du parcours. Dessinée dans `ui`, en bas de la page.
    pub(crate) fn visite_ligne_accueil(&mut self, ui: &mut egui::Ui) {
        if !self.confort.conseils_actifs && !self.visite_en_cours() {
            return;
        }
        let mut reponse = None;
        if self.visite_offerte() {
            // (Le parent écrit de droite à gauche : le dernier posé est le premier à gauche.)
            if crate::bulles::bulle(ui.button("Ne plus proposer"), "Ne plus proposer", "La visite ne se propose plus d'elle-même. Elle reste dans le menu Aide.", "").clicked() {
                reponse = Some(Reponse::NePlusProposer);
            }
            if crate::bulles::bulle(ui.button("Plus tard"), "Plus tard", "La visite se reproposera au prochain lancement (Aide > Visite rapide « Premier jour »… à tout moment).", "").clicked() {
                reponse = Some(Reponse::PlusTard);
            }
            if crate::bulles::bulle(ui.button(RichText::new("Visite rapide (2 min)").strong()), "Visite rapide", "Cinq étapes : créer un projet, dessiner des vignettes, mettre une durée, écouter l'animatique, exporter.", "").clicked() {
                reponse = Some(Reponse::Visite);
            }
            ui.label(RichText::new(format!("{}  Première fois sur SHOT ?", ph::COMPASS)).color(theme::texte_doux()));
        } else if self.visite_en_cours() {
            let e = &ETAPES[0];
            let mut quitter = false;
            if crate::bulles::bulle(ui.button("Quitter la visite"), "Quitter la visite", "Ferme la visite. Aide > Visite rapide « Premier jour »… la relance.", "").clicked() {
                quitter = true;
            }
            ui.label(RichText::new("Donne un nom au projet puis « Créer ».").color(theme::texte_doux()));
            ui.label(RichText::new(format!("{}  Visite rapide, étape 1 sur {} : {}.", ph::COMPASS, ETAPES.len(), e.titre)).color(theme::accent_texte()));
            if quitter {
                self.visite_finir();
            }
        }
        if let Some(r) = reponse {
            self.visite_repondre(r);
        }
    }

    /// Chaque image d'écran : la carte du parcours (la proposition, ou l'étape), quand l'écran est libre.
    pub(crate) fn visite_ui(&mut self, ctx: &egui::Context) {
        self.visite_suivi();
        if !self.visite_occupe() || !self.visite_libre() {
            return;
        }
        let offre = self.visite_offerte();
        if offre && !self.confort.conseils_actifs {
            return;
        }
        let etape = self.settings.premier_jour.etape.min(ETAPES.len() - 1);
        let mut reponse = None;
        let (mut lancer, mut suivant, mut precedent, mut quitter, mut regler) = (false, false, false, false, false);
        egui::Area::new(egui::Id::new("visite_premier_jour")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -120.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).fill(theme::eleve()).stroke(egui::Stroke::new(1.0_f32, theme::accent_texte())).show(ui, |ui| {
                ui.set_max_width(380.0);
                if offre {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(ph::COMPASS).size(16.0).color(theme::accent_texte()));
                        ui.label(RichText::new("Première fois sur SHOT ?").strong().color(theme::texte_fort()));
                    });
                    ui.add(egui::Label::new(RichText::new("Une visite rapide de deux minutes : créer un projet, dessiner des vignettes, mettre une durée, écouter l'animatique, exporter.").color(theme::texte_doux())).wrap());
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if crate::bulles::bulle(ui.button(RichText::new("Visite rapide (2 min)").strong()), "Visite rapide", "Cinq étapes, une petite carte à chaque fois.", "").clicked() {
                            reponse = Some(Reponse::Visite);
                        }
                        if crate::bulles::bulle(ui.button("Plus tard"), "Plus tard", "La visite se reproposera au prochain lancement.", "").clicked() {
                            reponse = Some(Reponse::PlusTard);
                        }
                        if crate::bulles::bulle(ui.button("Ne plus proposer"), "Ne plus proposer", "La visite ne se propose plus d'elle-même. Aide > Visite rapide « Premier jour »… la relance.", "").clicked() {
                            reponse = Some(Reponse::NePlusProposer);
                        }
                    });
                    return;
                }
                let e = &ETAPES[etape];
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ph::COMPASS).size(16.0).color(theme::accent_texte()));
                    ui.label(RichText::new(format!("Visite rapide · étape {} sur {} : {}", etape + 1, ETAPES.len(), e.titre)).strong().color(theme::texte_fort()));
                });
                ui.add(egui::Label::new(RichText::new(e.texte).color(theme::texte_doux())).wrap());
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if crate::bulles::bulle(ui.button(RichText::new(format!("{}  {}", e.icone, e.bouton)).strong()), e.bouton, "Ouvre l'endroit de cette étape ; la visite reste là.", "").clicked() {
                        lancer = true;
                    }
                    if etape > 1 && crate::bulles::bulle(ui.button("Précédent"), "Précédent", "Revient à l'étape d'avant.", "").clicked() {
                        precedent = true;
                    }
                    let dernier = etape + 1 == ETAPES.len();
                    if crate::bulles::bulle(ui.button(if dernier { "Terminer" } else { "Suivant" }), if dernier { "Terminer" } else { "Suivant" }, "Passe à l'étape suivante.", "").clicked() {
                        suivant = true;
                    }
                    if !dernier && crate::bulles::bulle(ui.button("Quitter la visite"), "Quitter la visite", "Ferme la visite. Aide > Visite rapide « Premier jour »… la relance.", "").clicked() {
                        quitter = true;
                    }
                });
                if etape + 1 == ETAPES.len() {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Pour finir : SHOT peut régler ses performances sur cet ordinateur.").small().color(theme::texte_doux()));
                    if crate::bulles::bulle(ui.button("Régler SHOT pour cette machine…"), "Régler SHOT pour cette machine", "Détecte le matériel, le mesure une quinzaine de secondes et propose des réglages de performance. Rien n'est appliqué sans ton accord. Aussi dans Aide.", "").clicked() {
                        regler = true;
                    }
                }
            });
        });
        if let Some(r) = reponse {
            self.visite_repondre(r);
        }
        if lancer {
            self.run_action(ETAPES[etape].action);
        }
        if precedent {
            self.settings.premier_jour.etape = etape.saturating_sub(1).max(1);
            let _ = self.settings.save();
        }
        if suivant {
            if etape + 1 >= ETAPES.len() {
                self.visite_finir();
                self.status = "Visite terminée : Aide > Visite rapide « Premier jour »… la relance.".into();
            } else {
                self.settings.premier_jour.etape = etape + 1;
                let _ = self.settings.save();
            }
        }
        if quitter {
            self.visite_finir();
        }
        if regler {
            self.run_action("aide.diagnostic");
        }
    }

    /// Préférences > Général > Conseils et visite : la visite se refait, les conseils se revoient.
    pub(crate) fn visite_prefs_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let b = ui.button("Refaire la visite rapide");
            if crate::bulles::bulle(b, "Visite rapide « Premier jour »", "Les cinq étapes : créer un projet, dessiner des vignettes, mettre une durée, écouter l'animatique, exporter.", &self.raccourci("aide.visite")).clicked() {
                self.visite_lancer();
            }
        });
    }
}
