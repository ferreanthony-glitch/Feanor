//! Noter une friction, côté application (`friction.rs` porte la note) : la touche (`aide.noter_friction`), la capture de la
//! fenêtre, le champ de texte, l'écriture sur un fil de fond.
//!
//! Le geste : la touche, une ligne si on veut, Entrée. On reste dans le dessin.
//! 1. À la touche, ce qu'on sait de l'instant est figé (heure, projet, espace, outil, plan, dernières commandes) et une
//!    capture de la fenêtre est demandée. **Le champ ne se montre pas encore** : la capture montre l'écran tel qu'il était.
//! 2. La capture arrivée (ou 0,6 s plus tard, sans elle), un champ de texte paraît en haut de la fenêtre, avec la main.
//! 3. Entrée : la note part. Échap : rien n'est gardé.
//!
//! **Sans latence.** Rien n'est écrit sur le fil de l'interface : la capture est encodée et la note ajoutée à son fichier sur
//! un fil de fond ; la barre d'état dit où quand c'est fait, ou pourquoi ça n'a pas marché.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, Align2, ColorImage, Event, Id, Key, Modifiers, Order, RichText, UserData, ViewportCommand};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::friction::{self, Avant, Ecriture, Note};

/// La commande qui note une friction, et celle qui ouvre le dossier des notes.
pub(crate) const ACTION: &str = "aide.noter_friction";
pub(crate) const ACTION_DOSSIER: &str = "aide.ouvrir_frictions";
/// Combien de temps on attend la capture avant de montrer le champ sans elle, en secondes.
const ATTENTE: f64 = 0.6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Repos,
    /// La capture est demandée : rien de plus n'est dessiné tant qu'elle n'est pas là.
    Capture,
    /// Le champ de texte est à l'écran.
    Saisie,
}

/// Ce que l'application garde d'une note en cours.
#[derive(Default)]
pub(crate) struct Etat {
    phase: Phase,
    depuis: f64,
    /// Ce qui est tapé.
    pub texte: String,
    image: Option<Arc<ColorImage>>,
    /// L'instant figé à la touche.
    note: Note,
    /// Le champ doit prendre la main à sa première image.
    focus: bool,
    /// La réponse du fil qui écrit la dernière note.
    retour: Option<Receiver<Result<PathBuf, String>>>,
    /// Le nombre de notes prises pendant cette séance.
    pub notees: usize,
}

impl Etat {
    /// Le champ de texte est-il à l'écran ?
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn en_saisie(&self) -> bool {
        self.phase == Phase::Saisie
    }

    /// Une note est-elle en cours (capture demandée ou champ à l'écran) ?
    pub(crate) fn en_cours(&self) -> bool {
        self.phase != Phase::Repos
    }
}

/// Le dossier des notes : `<Documents>\SHOT\Frictions`. (Les essais écrivent dans leur bac à sable.)
pub(crate) fn dossier() -> PathBuf {
    if cfg!(test) {
        crate::donnees::local().unwrap_or_else(std::env::temp_dir).join(friction::DOSSIER)
    } else {
        crate::nouveau::default_folder().join("SHOT").join(friction::DOSSIER)
    }
}

impl App {
    /// `aide.noter_friction` : fige l'instant et demande la capture de la fenêtre.
    pub(crate) fn friction_commencer(&mut self) {
        if self.friction.en_cours() {
            self.friction.focus = true;
            return;
        }
        let instant = self.usage.instant;
        let secs = crate::incrustation::now_secs();
        let (annee, mois, jour, heure, minute) = crate::remplacer::heure_locale(secs);
        // Ce qu'on venait de faire, sans la commande qui prend la note.
        let avant: Vec<Avant> = self
            .usage
            .compte
            .recentes
            .iter()
            .rev()
            .filter(|r| r.action != ACTION)
            .take(friction::AVANT)
            .map(|r| Avant { nom: crate::anneauui::nom_court(&crate::monpanneauui::infos(&self.ctx, &r.action).nom), chemin: r.source.nom(), il_y_a: instant - r.instant })
            .collect();
        // (Le nom du fichier du projet seulement, jamais son chemin.)
        let projet = match self.path.as_ref().and_then(|p| p.file_name()) {
            Some(nom) => nom.to_string_lossy().into_owned(),
            None if self.sans_projet() => String::new(),
            None => "projet pas encore enregistré".to_string(),
        };
        self.friction.note = Note {
            quand: (annee, mois, jour, heure, minute, (secs % 60) as u32),
            texte: String::new(),
            projet,
            espace: self.espace.nom().to_string(),
            outil: self.tool.label().to_string(),
            plan: if self.sans_projet() { String::new() } else { format!("{} sur {}", self.cur_panel + 1, self.doc.panels.len()) },
            avant,
            capture: false,
            version: format!("FEANOR {}", env!("CARGO_PKG_VERSION")),
        };
        self.friction.texte.clear();
        self.friction.image = None;
        self.friction.phase = Phase::Capture;
        self.friction.depuis = instant;
        self.friction.focus = true;
        self.ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
        self.ctx.request_repaint();
    }

    /// `aide.ouvrir_frictions` : le dossier des notes dans l'Explorateur.
    pub(crate) fn friction_ouvrir_le_dossier(&mut self) {
        let d = dossier();
        let fichier = d.join(friction::FICHIER);
        if fichier.exists() {
            crate::exportui::reveal_in_explorer(&fichier);
        } else {
            self.status = format!("Aucune friction notée pour l'instant. Elles iront dans {}.", d.display());
        }
    }

    /// Échap abandonne la note en cours et ne fait rien d'autre : à appeler avant les raccourcis.
    pub(crate) fn friction_echap(&mut self, ctx: &egui::Context) {
        if self.friction.en_cours() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.friction_abandonner();
        }
    }

    fn friction_abandonner(&mut self) {
        self.friction.phase = Phase::Repos;
        self.friction.texte.clear();
        self.friction.image = None;
        self.status = "Friction abandonnée : rien n'est noté.".into();
    }

    /// La note part : la capture et le texte sont écrits sur un fil de fond.
    fn friction_enregistrer(&mut self) {
        let mut note = std::mem::take(&mut self.friction.note);
        note.texte = std::mem::take(&mut self.friction.texte);
        let image = self.friction.image.take();
        let dossier = dossier();
        let (envoi, retour) = channel();
        std::thread::spawn(move || {
            // (La copie des pixels se fait ici, pas sur le fil de l'interface.)
            let image = image.map(|i| (i.as_raw().to_vec(), i.size[0] as u32, i.size[1] as u32));
            let _ = envoi.send(Ecriture { dossier, note, image }.faire());
        });
        self.friction.retour = Some(retour);
        self.friction.phase = Phase::Repos;
        self.friction.notees += 1;
        self.status = "Friction notée.".into();
    }

    /// Une fois par image, après les panneaux : la capture attendue, le champ de texte, la réponse du fil d'écriture.
    pub(crate) fn friction_fenetre(&mut self, ctx: &egui::Context) {
        match self.friction.retour.as_ref().map(|r| r.try_recv()) {
            None => {}
            Some(Ok(Ok(fichier))) => {
                self.status = format!("Friction notée dans {}.", fichier.display());
                self.friction.retour = None;
            }
            Some(Ok(Err(e))) => {
                self.status = format!("La friction n'a pas pu être notée : {e}");
                self.friction.retour = None;
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => ctx.request_repaint_after(Duration::from_millis(100)),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => self.friction.retour = None,
        }
        match self.friction.phase {
            Phase::Repos => {}
            Phase::Capture => {
                let recue = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    })
                });
                if recue.is_some() || ctx.input(|i| i.time) - self.friction.depuis > ATTENTE {
                    self.friction.image = recue;
                    self.friction.phase = Phase::Saisie;
                }
                ctx.request_repaint();
            }
            Phase::Saisie => self.friction_saisie(ctx),
        }
    }

    fn friction_saisie(&mut self, ctx: &egui::Context) {
        let (mut noter, mut abandonner) = (false, false);
        let capture = self.friction.image.is_some();
        egui::Area::new(Id::new("friction_saisie")).order(Order::Foreground).anchor(Align2::CENTER_TOP, [0.0, 90.0]).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(470.0);
                ui.label(RichText::new(format!("{}  Qu'est-ce qui t'a freiné ?", ph::NOTE_PENCIL)).strong());
                let champ = ui.add(egui::TextEdit::singleline(&mut self.friction.texte).desired_width(f32::INFINITY).hint_text("Une ligne suffit. Tu peux aussi laisser vide."));
                if std::mem::take(&mut self.friction.focus) {
                    champ.request_focus();
                }
                if champ.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    noter = true;
                }
                let suite = if capture { "La fenêtre est capturée telle qu'elle était, avec tes dernières commandes." } else { "Sans capture de la fenêtre. Tes dernières commandes sont notées." };
                ui.label(RichText::new(suite).small().weak());
                ui.horizontal(|ui| {
                    if ui.button("Noter").clicked() {
                        noter = true;
                    }
                    if ui.button("Abandonner").clicked() {
                        abandonner = true;
                    }
                    ui.label(RichText::new("Entrée : noter. Échap : abandonner.").small().weak());
                });
            });
        });
        if noter {
            self.friction_enregistrer();
        } else if abandonner {
            self.friction_abandonner();
        }
    }
}
