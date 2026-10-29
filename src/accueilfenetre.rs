//! La fenêtre d'accueil compacte (Anthony, 2026-10-29 : « Voilà ce que je veux en page d'ouverture. Inutile d'afficher le
//! logiciel vide derrière, juste cette fenêtre. »), `docs/INTERFACE.md` partie 74.
//!
//! Sans projet à rouvrir, SHOT ne montre qu'**une petite fenêtre** (`TAILLE_ACCUEIL`, centrée, barre de titre de Windows,
//! taille fixe) : ni menus, ni barres, ni panneaux, ni barre d'état. Dès qu'un projet s'ouvre, la fenêtre passe à sa taille de
//! travail ; quand le dernier projet se ferme, elle revient à l'accueil.
//!
//! - **Au lancement** : `demarre_sur_l_accueil` devine, avant de créer la fenêtre, si un projet va s'ouvrir (fichier donné,
//!   « Rouvrir le dernier projet », séance d'avant). Non : la fenêtre naît compacte, centrée (`eframe`, `centered`). Oui : elle
//!   naît à sa taille de travail, comme avant, et l'accueil ne se voit jamais. Si la supposition était fausse (le projet à
//!   rouvrir a disparu du disque), `regler_la_fenetre` rétrécit la fenêtre.
//! - **En route** : à chaque image, `regler_la_fenetre` compare le mode voulu (`accueil_compact`) au mode de la fenêtre et
//!   envoie les `ViewportCommand` des changements (agrandir : maximisée, ou à la taille et à la place qu'elle avait avant
//!   d'être compacte ; rétrécir : en deux temps, d'abord sortir du plein écran et de la maximisation, puis taille, taille
//!   fixe et position, centrée sur l'endroit où la fenêtre se trouvait).
//! - **Mémoire de la taille** : eframe n'en retient aucune (la fonction `persistence` n'est pas activée, et `persist_window`
//!   est coupé pour que cela reste vrai) ; SHOT retient seulement, le temps de la séance, la taille et la place de la fenêtre
//!   de travail pour la rendre au prochain projet. La taille compacte n'est donc jamais prise pour une taille de travail.

use std::path::PathBuf;

use eframe::egui::{self, vec2, Pos2, Vec2, ViewportCommand};

use crate::app::App;
use crate::nouveauui::NewDocDialog;
use crate::settings::Settings;

/// Taille intérieure de la fenêtre d'accueil, en points.
pub(crate) const TAILLE_ACCUEIL: [f32; 2] = [1000.0, 620.0];

/// Taille intérieure minimale de la fenêtre de travail, en points.
pub(crate) const TAILLE_MIN_TRAVAIL: [f32; 2] = [960.0, 600.0];

/// Dans quel état est la fenêtre : compacte (l'accueil seul) ou de travail (les projets).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeFenetre {
    Accueil,
    Travail,
}

/// La taille et la place de la fenêtre de travail, gardées le temps de la séance.
#[derive(Clone, Copy, Debug)]
struct Retour {
    maximisee: bool,
    position: Pos2,
    taille: Vec2,
}

/// L'état de la fenêtre native, côté application.
pub(crate) struct Fenetre {
    pub mode: ModeFenetre,
    retour: Option<Retour>,
    /// Le retour à l'accueil est à finir à l'image suivante : taille, taille fixe, position.
    etape_deux: bool,
    /// Où centrer la fenêtre compacte (le centre de la fenêtre de travail qu'elle remplace).
    centre: Option<Pos2>,
    /// L'épaisseur du cadre de Windows (barre de titre, bords), pour centrer la fenêtre entière.
    cadre: Vec2,
}

impl Default for Fenetre {
    fn default() -> Self {
        Fenetre { mode: ModeFenetre::Travail, retour: None, etape_deux: false, centre: None, cadre: vec2(16.0, 39.0) }
    }
}

/// L'état de l'écran d'accueil : le formulaire « Nouveau projet », le projet récent choisi.
#[derive(Default)]
pub(crate) struct AccueilEtat {
    /// Le formulaire posé dans le cadre « Nouveau projet » (créé à la première image de l'accueil).
    pub form: Option<NewDocDialog>,
    /// Le curseur doit aller dans le champ du nom (Ctrl+N).
    pub focus_nom: bool,
    /// Le projet récent choisi (un clic), que Entrée ouvre.
    pub selection: Option<PathBuf>,
}

/// SHOT va-t-il démarrer sur l'accueil compact ? Non quand un projet est donné ou va être rouvert ; oui quand une copie de secours
/// attend (elle se propose dans l'accueil).
pub(crate) fn demarre_sur_l_accueil(projet_donne: bool, reglages: &Settings, copies_de_secours: bool) -> bool {
    if projet_donne {
        return false;
    }
    if copies_de_secours {
        return true;
    }
    let rouvre = reglages.reopen_last && !reglages.session_vide && (!reglages.session_tabs.is_empty() || !reglages.recent_projects.is_empty());
    !rouvre
}

/// La touche pressée dans cette image n'est pas du texte : une touche de fonction, ou Ctrl avec une touche qu'un champ de texte
/// ne prend pas pour lui (A, C, V, X, Y, Z, les flèches et la suppression restent au champ).
pub(crate) fn raccourci_hors_saisie(ctx: &egui::Context) -> bool {
    use egui::Key;
    const FONCTIONS: [Key; 12] = [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12];
    const DU_CHAMP: [Key; 14] = [Key::A, Key::C, Key::V, Key::X, Key::Y, Key::Z, Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End, Key::Backspace, Key::Delete];
    ctx.input(|i| {
        let fonction = FONCTIONS.iter().any(|k| i.key_pressed(*k));
        let ctrl = i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, key, modifiers, .. } if modifiers.command && !DU_CHAMP.contains(key)));
        fonction || ctrl
    })
}

impl App {
    /// L'accueil est-il ce que la fenêtre montre ? Aucun projet n'est ouvert dans l'onglet affiché, et aucun n'arrive (un projet
    /// qui s'ouvre, la séance d'avant ou un projet donné au lancement qui revient : sinon l'accueil clignote devant lui).
    pub(crate) fn accueil_visible(&self) -> bool {
        self.sans_projet() && self.open_job.is_none() && self.session_file.is_empty() && self.reopen_probe.is_none()
    }

    /// La fenêtre est-elle l'accueil seul, compacte ? L'accueil est montré et il n'y a pas d'autres onglets (sinon la bande des
    /// onglets reste et la fenêtre garde sa taille de travail).
    pub(crate) fn accueil_compact(&self) -> bool {
        self.accueil_visible() && self.onglets.len() <= 1
    }

    /// Une fois par image : la fenêtre a la taille de ce qu'elle montre.
    pub(crate) fn regler_la_fenetre(&mut self, ctx: &egui::Context) {
        let veut = if self.accueil_compact() { ModeFenetre::Accueil } else { ModeFenetre::Travail };
        if veut == self.fenetre.mode {
            if veut == ModeFenetre::Accueil && self.fenetre.etape_deux {
                self.fenetre.etape_deux = false;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(TAILLE_ACCUEIL.into()));
                ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(TAILLE_ACCUEIL.into()));
                ctx.send_viewport_cmd(ViewportCommand::Resizable(false));
                if let Some(c) = self.fenetre.centre.take() {
                    let entiere = Vec2::from(TAILLE_ACCUEIL) + self.fenetre.cadre;
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(c - entiere / 2.0));
                }
                ctx.request_repaint();
            }
            return;
        }
        match veut {
            ModeFenetre::Accueil => {
                let (maximisee, plein, dehors, dedans) = ctx.input(|i| {
                    let v = i.viewport();
                    (v.maximized, v.fullscreen, v.outer_rect, v.inner_rect)
                });
                // (La taille de travail est gardée pour le prochain projet ; pas en plein écran, dont la taille n'est pas la sienne.)
                if !plein.unwrap_or(false) {
                    self.fenetre.retour = Some(Retour {
                        maximisee: maximisee.unwrap_or(true),
                        position: dehors.map_or(Pos2::ZERO, |r| r.min),
                        taille: dedans.map_or(vec2(1500.0, 920.0), |r| r.size()),
                    });
                }
                self.fenetre.centre = dehors.map(|r| r.center());
                if let (Some(o), Some(i)) = (dehors, dedans) {
                    let c = o.size() - i.size();
                    if c.x >= 0.0 && c.y >= 0.0 && c.x < 100.0 && c.y < 150.0 {
                        self.fenetre.cadre = c;
                    }
                }
                ctx.send_viewport_cmd(ViewportCommand::Fullscreen(false));
                ctx.send_viewport_cmd(ViewportCommand::Maximized(false));
                self.fenetre.etape_deux = true;
                // (La maximisation du lancement, `App::frame`, n'a plus lieu d'être.)
                self.maximized_once = 4;
            }
            ModeFenetre::Travail => {
                self.fenetre.etape_deux = false;
                self.fenetre.centre = None;
                ctx.send_viewport_cmd(ViewportCommand::Resizable(true));
                ctx.send_viewport_cmd(ViewportCommand::MinInnerSize(TAILLE_MIN_TRAVAIL.into()));
                match self.fenetre.retour {
                    Some(r) if !r.maximisee => {
                        ctx.send_viewport_cmd(ViewportCommand::InnerSize(r.taille));
                        ctx.send_viewport_cmd(ViewportCommand::OuterPosition(r.position));
                    }
                    _ => ctx.send_viewport_cmd(ViewportCommand::Maximized(true)),
                }
            }
        }
        self.fenetre.mode = veut;
        ctx.request_repaint();
    }
}
