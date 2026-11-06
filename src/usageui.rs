//! Le compteur d'usage, côté application (`usage.rs` porte le compte) : compter chaque commande à son départ, écrire le
//! compte au repos, et la fenêtre « Mes habitudes » (Aide > Mes habitudes…, `aide.habitudes`).
//!
//! Où une commande est comptée :
//! - `App::run_action` : toute commande lancée par son identifiant. Celui qui la lance dit par où (`Etat::source`) : le
//!   clavier (`handle_shortcuts`), un menu (`action_item`), la palette, l'anneau sous le stylet ; sinon « autre ».
//! - un clic sur une icône de commande (`raccourcisui::shortcut_menu`, par où passent les boutons des barres, des panneaux et
//!   les outils) : noté par [`clic`], lu à l'image suivante. Si le clic a aussi lancé la commande par `run_action`, les deux
//!   ne font qu'un (`usage::DOUBLON`).
//!
//! Une touche tenue qui se répète ne compte qu'une fois (son premier appui).
//!
//! **Sans latence.** Compter, c'est un nombre de plus en mémoire. Le fichier (quelques kilo-octets) s'écrit au plus une fois
//! par minute, jamais pendant qu'un bouton de la souris ou la pointe est appuyé, et à la fermeture.

use eframe::egui::{self, Id, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::app::{App, Tool};
use crate::usage::{self, Source, Usage};

/// Clé des données d'egui où un bouton dessiné sans accès à l'application note son clic.
const CLICS: &str = "feanor_usage_clics";
/// Délai entre deux écritures du compte, en secondes.
const ECRITURE: f64 = 60.0;
/// Combien de commandes la fenêtre montre parmi les plus utilisées.
const MONTREES: usize = 15;
/// En dessous de ce nombre de jours, « jamais utilisée » ne veut encore rien dire.
const JOURS_UTILES: i64 = 7;

/// Ce que l'application garde du compteur.
#[derive(Default)]
pub(crate) struct Etat {
    /// Le compte de ce poste et le journal des dernières commandes.
    pub compte: Usage,
    /// Par où part la prochaine commande : posé par celui qui la lance, repris par `usage_noter`.
    pub source: Option<Source>,
    /// La prochaine commande est la répétition d'une touche tenue : elle n'est pas comptée.
    pub muet: bool,
    /// L'horloge d'egui à cette image.
    pub instant: f64,
    /// La fenêtre « Mes habitudes » est ouverte.
    pub fenetre: bool,
    dernier_ecrit: f64,
    confirmer: bool,
}

impl Etat {
    /// L'état au lancement, avec le compte lu sur le poste.
    pub(crate) fn avec(compte: Usage) -> Etat {
        Etat { compte, ..Default::default() }
    }
}

/// Un clic sur l'icône d'une commande (lu à l'image suivante, `App::usage_suivre`).
pub(crate) fn clic(ctx: &egui::Context, action: &str) {
    if action.is_empty() {
        return;
    }
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(Id::new(CLICS)).push(action.to_string()));
}

fn aujourd_hui() -> String {
    crate::incrustation::date_text(crate::incrustation::now_secs())
}

impl App {
    /// Le compte de ce poste, lu au lancement.
    pub(crate) fn usage_lire() -> Usage {
        crate::donnees::roaming().and_then(|d| std::fs::read_to_string(d.join(usage::FICHIER)).ok()).map(|t| Usage::lire(&t)).unwrap_or_default()
    }

    /// `run_action` : cette commande part. Le journal la retient toujours ; le compte, si le compteur est allumé.
    pub(crate) fn usage_noter(&mut self, action: &str) {
        let source = self.usage.source.take().unwrap_or(Source::Autre);
        if std::mem::take(&mut self.usage.muet) {
            return;
        }
        self.usage.compte.noter(action, source, &aujourd_hui(), self.usage.instant, self.settings.usage_actif);
    }

    /// À chaque image, avant les panneaux : l'heure, les clics d'icônes de l'image d'avant, et l'écriture du compte au repos.
    pub(crate) fn usage_suivre(&mut self, ctx: &egui::Context) {
        self.usage.instant = ctx.input(|i| i.time);
        let clics = ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<String>>(Id::new(CLICS))));
        if !clics.is_empty() {
            let jour = aujourd_hui();
            for action in clics {
                self.usage.compte.noter(&action, Source::Icone, &jour, self.usage.instant, self.settings.usage_actif);
            }
        }
        if self.usage.compte.a_ecrire && self.usage.instant - self.usage.dernier_ecrit >= ECRITURE && !ctx.input(|i| i.pointer.any_down()) {
            self.usage_ecrire();
        }
    }

    /// Écrit le compte à côté des réglages (au repos, à la fermeture, après une remise à zéro).
    pub(crate) fn usage_ecrire(&mut self) {
        if !std::mem::take(&mut self.usage.compte.a_ecrire) {
            return;
        }
        self.usage.dernier_ecrit = self.usage.instant;
        let Some(dossier) = crate::donnees::roaming() else { return };
        let _ = std::fs::create_dir_all(&dossier);
        if let Err(e) = crate::project::write_atomic(&dossier.join(usage::FICHIER), self.usage.compte.ecrire().as_bytes()) {
            self.status = format!("Mes habitudes : le compte n'a pas pu être écrit ({e}).");
        }
    }

    /// Les commandes qu'on voit dans les barres : les outils et les boutons des barres. C'est parmi elles que « jamais
    /// utilisée » a un sens (une commande qu'on ne voit nulle part ne gêne personne).
    /// Rangées par endroit : les outils, puis chaque barre (une commande présente à deux endroits n'est dite qu'au premier).
    fn commandes_visibles() -> Vec<(&'static str, Vec<&'static str>)> {
        use crate::barres::Genre;
        let outils: Vec<&'static str> = Tool::ALL.iter().map(|t| t.action()).collect();
        let mut vues = outils.clone();
        let mut groupes = vec![("Outils", outils)];
        for (titre, barre) in [("Barre des commandes", crate::barres::COMMANDES), ("Barre du montage", crate::barres::MONTAGE), ("Barre de la timeline", crate::barres::TIMELINE)] {
            let mut groupe = Vec::new();
            for c in barre {
                if matches!(c.genre, Genre::Bouton | Genre::Etat) && !c.action.is_empty() && !vues.contains(&c.action) {
                    vues.push(c.action);
                    groupe.push(c.action);
                }
            }
            groupes.push((titre, groupe));
        }
        groupes
    }

    /// La fenêtre « Mes habitudes » (Aide > Mes habitudes…).
    pub(crate) fn habitudes_fenetre(&mut self, ctx: &egui::Context) {
        if !self.usage.fenetre {
            return;
        }
        let mut ouverte = true;
        egui::Window::new("Mes habitudes").open(&mut ouverte).collapsible(false).resizable(true).default_width(540.0).default_height(580.0).show(ctx, |ui| self.habitudes_contenu(ui));
        if !ouverte {
            self.usage.fenetre = false;
            self.usage.confirmer = false;
        }
    }

    fn habitudes_contenu(&mut self, ui: &mut egui::Ui) {
        let jour = aujourd_hui();
        let jours = self.usage.compte.jours(&jour);
        let total = self.usage.compte.total();
        // L'en-tête : depuis quand, et la promesse.
        let depuis = match jours {
            Some(0) => format!("{total} commandes comptées depuis aujourd'hui."),
            Some(1) => format!("{total} commandes comptées depuis hier."),
            Some(n) => format!("{total} commandes comptées depuis le {} ({n} jours).", usage::jour_en_clair(&self.usage.compte.depuis)),
            None => "Rien n'est encore compté.".to_string(),
        };
        ui.label(RichText::new(depuis).strong());
        ui.label(RichText::new("Le compte reste sur cet ordinateur : FEANOR ne l'envoie nulle part. Il ne retient que des noms de commandes et des nombres.").small().weak());
        ui.horizontal(|ui| {
            let resp = ui.checkbox(&mut self.settings.usage_actif, "Compter mes commandes");
            let resp = crate::bulles::bulle(resp, "Compter mes commandes", "Chaque commande lancée est comptée, avec son chemin : clavier, icône, menu, anneau, palette. Décoché : plus rien n'est compté, le compte déjà fait reste.", "");
            if resp.changed() {
                self.settings_a_ecrire = true;
            }
            if self.usage.confirmer {
                if ui.button("Oui, remettre à zéro").clicked() {
                    self.usage.compte.effacer();
                    self.usage_ecrire();
                    self.usage.confirmer = false;
                    self.status = "Mes habitudes : le compte est remis à zéro.".into();
                }
                if ui.button("Non").clicked() {
                    self.usage.confirmer = false;
                }
            } else if ui.add_enabled(total > 0, egui::Button::new("Remettre à zéro…")).on_hover_text("Efface tout le compte. Le comptage repart d'aujourd'hui.").clicked() {
                self.usage.confirmer = true;
            }
        });
        ui.separator();

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("Les plus utilisées").strong());
            let lignes: Vec<(String, u32, String, Option<Source>)> = self.usage.compte.plus_utilisees(MONTREES).into_iter().map(|(a, c)| (a.to_string(), c.total(), c.detail(), c.chemin_principal())).collect();
            if lignes.is_empty() {
                ui.label(RichText::new("Dessine, change d'outil, lance des commandes : elles apparaîtront ici.").weak());
            }
            egui::Grid::new("habitudes_plus_utilisees").num_columns(3).spacing([14.0, 4.0]).striped(true).show(ui, |ui| {
                for (action, n, detail, principal) in &lignes {
                    // (L'icône et le nom de l'anneau : les mêmes partout.)
                    let part = self.anneau_part(action);
                    let raccourci = self.raccourci(action);
                    let dans_l_anneau = self.settings.anneau.commandes.contains(action);
                    let resp = ui.add(egui::Label::new(format!("{}  {}", part.icone, part.nom)).sense(Sense::click()));
                    // Clic droit : lui donner une touche, ou l'envoyer dans l'anneau (sans compter ce clic comme un usage).
                    crate::raccourcisui::menu_de_commande(&resp, action, &raccourci);
                    ui.label(RichText::new(n.to_string()).strong());
                    let mut suite = detail.clone();
                    // Une commande qui part surtout d'un menu, d'une icône ou de la palette, et qui n'a pas de touche : à lui en donner une.
                    if raccourci.is_empty() && matches!(principal, Some(Source::Menu | Source::Icone | Source::Palette | Source::Autre)) {
                        suite += "  ·  sans raccourci";
                    }
                    if dans_l_anneau {
                        suite += &format!("  ·  {} dans l'anneau", ph::COMPASS);
                    }
                    ui.label(RichText::new(suite).small().weak());
                    ui.end_row();
                }
            });
            if !lignes.is_empty() {
                ui.label(RichText::new("Clic droit sur une commande : lui donner une touche, ou l'envoyer dans l'anneau.").small().weak());
            }

            ui.add_space(10.0);
            ui.label(RichText::new("Jamais utilisées").strong());
            // Par endroit : le titre, combien il en a, et celles qui n'ont jamais servi.
            let groupes: Vec<(&'static str, usize, Vec<String>)> = Self::commandes_visibles()
                .into_iter()
                .map(|(titre, commandes)| {
                    let jamais = self.usage.compte.jamais(commandes.iter().copied()).into_iter().map(|a| self.anneau_part(a)).map(|p| format!("{}  {}", p.icone, p.nom)).collect();
                    (titre, commandes.len(), jamais)
                })
                .collect();
            let jamais: usize = groupes.iter().map(|g| g.2.len()).sum();
            match jours {
                None => {
                    ui.label(RichText::new("Le comptage n'a pas commencé.").weak());
                }
                Some(n) if n < JOURS_UTILES => {
                    ui.label(RichText::new(format!("Trop tôt pour conclure : {} de comptage sur les {JOURS_UTILES} qu'il faut au moins. Pour l'instant, {jamais} outils et boutons des barres n'ont pas servi.", if n <= 1 { "un jour".to_string() } else { format!("{n} jours") })).weak());
                }
                Some(_) if jamais == 0 => {
                    ui.label(RichText::new("Tous les outils et les boutons des barres ont servi au moins une fois.").weak());
                }
                Some(n) => {
                    ui.label(RichText::new(format!("{jamais} outils et boutons des barres n'ont pas servi en {n} jours. Bons candidats à cacher.")).small().weak());
                    for (titre, tous, commandes) in &groupes {
                        if commandes.is_empty() {
                            continue;
                        }
                        // (Les outils ouverts d'office : c'est la barre qu'on a sous la main en dessinant.)
                        egui::CollapsingHeader::new(format!("{titre} : {} sur {tous}", commandes.len())).id_salt(("habitudes_jamais", *titre)).default_open(*titre == "Outils").show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for texte in commandes {
                                    ui.label(texte);
                                    ui.add_space(8.0);
                                }
                            });
                        });
                    }
                }
            }
        });
    }
}
