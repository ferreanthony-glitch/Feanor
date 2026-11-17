//! On n'enregistre plus (2026-11-10, proposition 25 d'Anthony). FEANOR écrit le projet tout seul, dans son dossier, dès que
//! le stylet se lève : fermer ne demande plus rien, et un plantage ou un arrêt brutal ne perd que le dernier instant.
//!
//! **Ce qui écrit** est l'enregistrement de toujours (`enregistrement.rs`) : le cliché des seuls dessins modifiés sur le fil
//! de l'interface (quelques millisecondes), l'écriture sur un autre fil, chaque fichier remplacé d'un renommage, le manifeste
//! en dernier. Ici on décide seulement **quand** : le projet est modifié depuis `ATTENTE` et le stylet est levé depuis `CALME`
//! (on n'écrit pas entre deux hachures) et le film ne se lit pas ; au plus tard `PLAFOND` après la première modification, au
//! premier stylet levé, même pendant une lecture. Jamais pendant un geste, une retouche de dégradé, un calcul de fond qui doit
//! encore poser son résultat, un export qui prend sa copie du projet : l'écriture attend, elle ne coupe rien.
//!
//! **Pour qui.** Un projet qui a son dossier, qui n'est pas un film partagé (là, enregistrer publie le travail à l'équipe :
//! cela reste un geste voulu) et qui n'est pas un jalon ouvert pour le regarder. Un projet sans dossier garde la question
//! d'avant et sa copie de secours. Le réglage « Enregistrer en continu » (Préférences) rend l'ancienne façon.
//!
//! **Ce qui remplace « Ne pas enregistrer »** : les jalons (`jalons.rs`, `jalonsui.rs`). Un jalon automatique est posé à
//! l'ouverture, avant la première écriture : on peut toujours revenir au projet tel qu'on l'a ouvert, même d'un ancien format.
//! Ctrl+S pose un jalon.
//!
//! **Si l'écriture échoue** (disque plein, dossier retiré), la fenêtre d'erreur de toujours le dit, l'écriture continue se met
//! en pause (elle ne redemande pas chaque seconde) et la copie de secours reprend son rôle ; un enregistrement réussi la relance.

use std::time::{Duration, Instant};

use eframe::egui;

use crate::app::{App, Pending};
use crate::project;

/// Le projet doit être modifié depuis ce temps avant qu'on l'écrive.
const ATTENTE: Duration = Duration::from_millis(1000);
/// Et le stylet levé depuis ce temps (entre deux hachures, on n'écrit pas).
const CALME: Duration = Duration::from_millis(400);
/// Sans jamais dépasser ce temps depuis la première modification : on écrit alors au premier stylet levé.
const PLAFOND: Duration = Duration::from_secs(10);
/// Une zone bitmap qu'on transforme encore : on la laisse tranquille ce temps après le dernier geste, puis l'écriture la pose
/// comme le fait Ctrl+S.
const ZONE_TRANQUILLE: Duration = Duration::from_secs(3);

#[cfg(test)]
thread_local! {
    /// Essais : les trois délais (attente, calme, plafond), pour ne pas attendre de vraies secondes.
    static DELAIS: std::cell::Cell<Option<(Duration, Duration, Duration)>> = const { std::cell::Cell::new(None) };
}

/// Essais : les délais de l'écriture continue sur ce fil (rien : ceux de l'application).
#[cfg(test)]
pub(crate) fn delais_d_essai(d: Option<(Duration, Duration, Duration)>) {
    DELAIS.with(|c| c.set(d));
}

fn delais() -> (Duration, Duration, Duration) {
    #[cfg(test)]
    if let Some(d) = DELAIS.with(|c| c.get()) {
        return d;
    }
    (ATTENTE, CALME, PLAFOND)
}

/// Ce que l'écriture continue retient d'un projet (un par onglet).
#[derive(Default)]
pub(crate) struct Continu {
    /// Depuis quand le projet attend d'être écrit.
    depuis: Option<Instant>,
    /// La dernière image d'écran où un geste tenait la main.
    dernier_geste: Option<Instant>,
    /// En pause, et pourquoi (l'écriture a échoué, le jalon d'ouverture n'a pas pu être posé) : un enregistrement réussi la relance.
    pub pause: Option<String>,
    /// Le jalon qui se pose en ce moment, sur un autre fil (`jalonsui.rs`).
    pub jalon: Option<crate::jalonsui::Pose>,
    /// Un jalon demandé (Ctrl+S) : il se pose dès que le projet est écrit.
    pub jalon_demande: bool,
}

impl Continu {
    /// L'écriture continue vaut-elle pour un projet rangé à `path` ? (`partage` : c'est un film partagé.)
    pub(crate) fn vaut(&self, reglage: bool, path: Option<&std::path::Path>, partage: bool) -> bool {
        reglage && !partage && self.pause.is_none() && path.is_some_and(|p| !crate::jalons::est_dans_un_jalon(p))
    }
}

impl App {
    /// FEANOR écrit ce projet tout seul : il a son dossier, ce n'est ni un film partagé ni un jalon, et rien ne l'a mis en pause.
    pub(crate) fn continu_actif(&self) -> bool {
        self.continu.vaut(self.settings.enregistrer_en_continu, self.path.as_deref(), self.partage.is_some())
    }

    /// Un geste tient la main en ce moment : on n'écrit pas au milieu.
    fn continu_geste(&self, ctx: &egui::Context) -> bool {
        self.stroke.is_some() || self.fond_stroke.is_some() || self.vstroke.is_some() || ctx.input(|i| i.pointer.any_down())
    }

    /// Quelque chose doit finir avant qu'on écrive : une écriture, un jalon, un calcul de fond qui n'a pas posé son résultat, une
    /// retouche de dégradé (le dessin est entre deux états), une question posée. Ce que Ctrl+S attendrait en figeant la fenêtre,
    /// l'écriture continue l'attend sans rien figer.
    fn continu_retenu(&self) -> bool {
        self.saving.is_some()
            || self.continu.jalon.is_some()
            || self.open_job.is_some()
            || self.video_import.is_some()
            || self.save_error.is_some()
            || self.remplacement.is_some()
            || self.pending.is_some()
            || self.groupe.is_some()
            || self.degrade_en_retouche()
            || self.export_job.as_ref().is_some_and(crate::export::ExportJob::needs_project)
            || self.gomme_travaux.occupe()
            || self.aplat_travaux.en_route > 0
            || self.intel_occupe()
            || self.perso.aplat.occupe()
            || self.perso.fil.as_ref().is_some_and(|f| f.en_cours() > 0)
    }

    /// Une zone bitmap transformée attend encore ses pixels : écrire la poserait (comme Ctrl+S).
    fn continu_zone_en_main(&self) -> bool {
        self.bsel.as_ref().and_then(|s| s.lifted.as_ref()).is_some_and(|l| l.stale)
    }

    /// À chaque image d'écran : écrit le projet s'il est temps.
    pub(crate) fn tick_continu(&mut self, ctx: &egui::Context) {
        self.tick_jalon(ctx);
        if !self.modified || !self.continu_actif() {
            self.continu.depuis = None;
            return;
        }
        let maintenant = Instant::now();
        let depuis = *self.continu.depuis.get_or_insert(maintenant);
        let geste = self.continu_geste(ctx);
        if geste {
            self.continu.dernier_geste = Some(maintenant);
        }
        let (attente, calme, plafond) = delais();
        let leve_depuis = self.continu.dernier_geste.map(|t| maintenant.duration_since(t));
        let age = maintenant.duration_since(depuis);
        // (Pendant la lecture non plus, on n'écrit pas tout de suite : une image du film ne saute pas pour un cliché. Mais une
        // lecture laissée en boucle ne retient pas l'écriture au-delà du plafond.)
        let tranquille = leve_depuis.map_or(true, |d| d >= calme) && !self.playing;
        let zone = self.continu_zone_en_main() && leve_depuis.is_some_and(|d| d < ZONE_TRANQUILLE.max(calme));
        let temps = (age >= attente && tranquille) || age >= plafond;
        if temps && !geste && !zone && !self.continu_retenu() {
            self.continu_ecrire();
            return;
        }
        // Rien ne bouge à l'écran : on revient quand même, pour écrire à l'heure.
        let reste = if age < attente { attente - age } else { calme.min(Duration::from_millis(200)) };
        ctx.request_repaint_after(reste.max(Duration::from_millis(30)));
    }

    /// Écrit le projet maintenant, sans rien dire dans la barre d'état. Vrai si l'écriture est partie.
    pub(crate) fn continu_ecrire(&mut self) -> bool {
        self.continu_ecrire_puis(None)
    }

    /// Écrit le projet, puis fait `then` (fermer, quitter…). Vrai si l'écriture est partie.
    pub(crate) fn continu_ecrire_puis(&mut self, then: Option<Pending>) -> bool {
        let Some(p) = self.path.clone() else { return false };
        self.continu.depuis = None;
        self.start_save_avec(project::manifeste_shot(&p), then, true)
    }

    /// Fermer, quitter, ouvrir à la place : avec l'écriture continue, la question « Modifications non enregistrées » ne se pose
    /// pas. Le projet s'écrit, puis l'action suit ; si l'écriture échoue, sa fenêtre le dit et rien ne se ferme. Vrai si
    /// l'affaire est prise en main.
    pub(crate) fn continu_avant(&mut self, action: Pending) -> bool {
        if !self.continu_actif() {
            return false;
        }
        // (Si rien n'est parti, la fenêtre d'erreur de l'enregistrement parle, et le projet reste ouvert.)
        self.continu_ecrire_puis(Some(action));
        true
    }

    /// L'application se ferme (ou l'onglet s'en va) : ce qui n'est pas encore écrit l'est maintenant, en attendant la fin.
    pub(crate) fn continu_avant_de_partir(&mut self) {
        if self.modified && self.continu_actif() && self.stroke.is_none() {
            if self.saving.is_none() {
                self.continu_ecrire();
            }
            let _ = self.wait_save();
        }
    }

    /// L'onglet affiché va être rangé : son projet s'écrit d'abord (l'écriture finit pendant qu'il est caché, `tick_onglets`).
    pub(crate) fn continu_avant_de_ranger(&mut self) {
        if self.modified && self.continu_actif() && self.saving.is_none() && self.save_error.is_none() {
            self.continu_ecrire();
        }
    }

    /// L'écriture du projet vient de finir. Réussie : la pause est levée. Ratée : l'écriture continue se met en pause (elle ne
    /// redemande pas chaque seconde), jusqu'à un enregistrement réussi.
    pub(crate) fn continu_apres_ecriture(&mut self, reussie: bool, erreur: Option<&str>) {
        if reussie {
            self.continu.pause = None;
        } else {
            self.continu.pause = Some(erreur.unwrap_or("l'écriture a échoué").to_string());
            self.continu.jalon_demande = false;
        }
    }

    /// Dans la barre d'état : un mot discret qui dit que le travail est sur le disque, ou pourquoi il ne l'est pas.
    pub(crate) fn continu_status_ui(&self, ui: &mut egui::Ui) {
        if !self.settings.enregistrer_en_continu || self.path.is_none() || self.partage.is_some() || self.sans_projet() {
            return;
        }
        ui.separator();
        if let Some(raison) = &self.continu.pause {
            ui.label(egui::RichText::new("Écriture en pause").small().color(crate::theme::palette().braise))
                .on_hover_text(format!("FEANOR n'écrit plus ce projet tout seul : {raison}. Ctrl+S enregistre et relance l'écriture continue. La copie de secours tourne en attendant."));
            return;
        }
        // (Le mot ne change pas à chaque trait, la barre ne danse pas : « Écriture… » seulement quand quelque chose la retient
        // ou qu'elle dure.)
        let attend = self.continu.depuis.is_some_and(|d| d.elapsed() >= Duration::from_secs(3));
        let dure = self.saving.as_ref().is_some_and(|s| s.dure_depuis() >= Duration::from_secs(1));
        let texte = if attend || dure { "Écriture…" } else { "Enregistré" };
        ui.label(egui::RichText::new(texte).small().weak()).on_hover_text(
            "FEANOR écrit ton travail en continu dans le dossier du projet, dès que le stylet se lève : il n'y a plus rien à enregistrer. \
             Ctrl+S pose un jalon, un moment du projet auquel tu peux revenir (Fichier > Jalons…).",
        );
    }

    /// La case « Enregistrer en continu » des Préférences.
    pub(crate) fn continu_reglage_ui(&mut self, ui: &mut egui::Ui) {
        let r = ui.checkbox(&mut self.settings.enregistrer_en_continu, "Enregistrer en continu").on_hover_text(
            "FEANOR écrit le projet tout seul dans son dossier dès que le stylet se lève, et fermer ne demande plus rien. Ctrl+S pose un \
             jalon (Fichier > Jalons…) ; un jalon est aussi posé à chaque ouverture, pour revenir au projet tel qu'il était. \
             Décoché : comme avant, seul « Enregistrer » écrit le projet. Les films partagés et les projets sans dossier ne sont jamais écrits tout seuls.",
        );
        if r.changed() {
            if let Err(e) = self.settings.save() {
                self.status = format!("Réglages non enregistrés : {e}");
            }
        }
    }
}
