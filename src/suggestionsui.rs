//! Le panneau « Suggestions sonores » (Anthony, 2026-10-19) : SHOT lit le storyboard et propose des sons
//! témoins que l'on écoute, accepte, régénère ou supprime. Le modèle d'une proposition est dans
//! `indications.rs`, la lecture dans `lecturesb.rs` / `lecturerules.rs`, les fournisseurs dans
//! `fournisseurs.rs`, les tâches dans `generation.rs` ; ce module les relie à l'interface.
//!
//! - **Lire le storyboard** : sur tout le film, la séquence courante, le plan courant ou une plage de
//!   plans. Les textes se rassemblent aussitôt ; les vignettes se font sur le fil de l'interface, au plus
//!   8 ms par image d'écran (la règle « sans latence ») ; puis un fil à part lit les plans un par un.
//! - **Générer** : une file d'attente, un fil à la fois (un seul serveur local, une seule carte
//!   graphique). Le fichier est écrit dans le cache de SHOT par le fil de travail.
//! - **Accepter** : le fichier est lu et décodé sur un fil à part, puis copié dans le projet comme un
//!   son importé et posé à l'image voulue sur la piste « Sons proposés » (`indications::piste_pour`) :
//!   une étape d'annulation « Accepter un son proposé ».
//! - **En ligne** : quand un rôle est confié à un service en ligne, une fenêtre dit ce qui partira (les
//!   vignettes et les textes, ou les descriptions, ou les répliques) et demande une confirmation, **une
//!   fois par projet** (`SonsProposes::envoi_en_ligne_confirme`) ; le panneau rappelle en permanence que
//!   les images partent en ligne, quand c'est le cas.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align2, FontId, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::audio::Pcm;
use crate::clessecretes::{Coffre, CoffreWindows};
use crate::dock::Pane;
use crate::fournisseurs::{Controle, DemandeLecture, EntreePlan, LecteurStoryboard};
use crate::fournisseursapi::{Transport, TransportABrancher};
use crate::generation::{dossier_cache, lancer_generation, lancer_lecture, Fabrique, FabriqueReelle, Rapport, Tache, Travail};
use crate::serveurlocal::GestionnaireServeur;
use crate::genreglages::{Mode, Role};
use crate::indications::{debut_montage, piste_pour, Genre, Indication, Statut};
use crate::lecturesb::{entree_plan, plans_de_la_portee, vignette_png, Portee, LARGEUR_VIGNETTE};
use crate::storyboard::{panel_title, timecode};
use crate::theme;

/// Hauteur d'une ligne de la liste, en points.
const ROW_H: f32 = 44.0;
/// Le temps accordé aux vignettes par image d'écran (au moins une est faite à chaque fois).
const BUDGET_VIGNETTES: Duration = Duration::from_millis(8);

/// Ce que la lecture parcourt, tel que le panneau le propose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoixPortee {
    Film,
    Sequence,
    Plan,
    Plage,
}

impl ChoixPortee {
    const ALL: [ChoixPortee; 4] = [ChoixPortee::Film, ChoixPortee::Sequence, ChoixPortee::Plan, ChoixPortee::Plage];

    fn nom(self) -> &'static str {
        match self {
            ChoixPortee::Film => "Tout le film",
            ChoixPortee::Sequence => "La séquence courante",
            ChoixPortee::Plan => "Le plan courant",
            ChoixPortee::Plage => "Des plans choisis",
        }
    }
}

/// Ce qui attend la confirmation d'envoi en ligne.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Lire(Portee),
    Generer(Vec<u64>),
}

/// Les plans dont on a rassemblé les textes, en attente de leurs vignettes.
struct Collecte {
    lecteur: Arc<dyn LecteurStoryboard>,
    entrees: Vec<EntreePlan>,
    suivant: usize,
    controle: Controle,
}

/// Un son accepté : lu et décodé sur un fil à part, posé dès qu'il est prêt.
struct Acceptation {
    id: u64,
    path: PathBuf,
    rx: Receiver<Result<(Vec<u8>, Pcm, String), String>>,
}

/// La fenêtre « Modifier » d'une proposition.
#[derive(Clone, Debug)]
pub(crate) struct Edition {
    pub id: u64,
    pub kind: Genre,
    pub start: usize,
    pub frames: usize,
    pub description: String,
    pub prompt: String,
    pub text: String,
    pub character: String,
    pub voice: String,
}

/// L'état de l'interface des suggestions sonores (un seul champ dans `App`).
pub(crate) struct SuggestionsUi {
    pub fabrique: Arc<dyn Fabrique>,
    pub coffre: Arc<dyn Coffre>,
    pub transport: Arc<dyn Transport>,
    /// Le serveur local lancé par SHOT (Préférences > Génération sonore, ou à la première génération locale).
    pub serveur: Arc<GestionnaireServeur>,
    /// Où les sons générés sont écrits.
    pub dossier: PathBuf,
    pub portee: ChoixPortee,
    /// Des plans choisis : du plan n° `a` au plan n° `b` du film (comptés depuis 1).
    pub plage: (usize, usize),
    pub choisie: Option<u64>,
    pub taches: Vec<Tache>,
    collecte: Option<Collecte>,
    attente: Vec<(Travail, Statut)>,
    acceptations: Vec<Acceptation>,
    pub edition: Option<Edition>,
    pub confirmation: Option<Action>,
    /// Les Préférences > Génération sonore (`genprefsui.rs`).
    pub prefs: crate::genprefsui::GenPrefs,
}

impl Default for SuggestionsUi {
    fn default() -> Self {
        let coffre: Arc<dyn Coffre> = Arc::new(CoffreWindows::new());
        let transport: Arc<dyn Transport> = Arc::new(TransportABrancher);
        // (Les essais ne lancent jamais le vrai serveur : ils n'ont pas besoin de Python.)
        #[cfg(test)]
        let serveur = GestionnaireServeur::inactif();
        #[cfg(not(test))]
        let serveur = GestionnaireServeur::reel();
        SuggestionsUi {
            fabrique: Arc::new(FabriqueReelle { transport: transport.clone(), coffre: coffre.clone(), serveur: serveur.clone() }),
            coffre,
            transport,
            serveur,
            dossier: dossier_cache(),
            portee: ChoixPortee::Film,
            plage: (1, 1),
            choisie: None,
            taches: Vec::new(),
            collecte: None,
            attente: Vec::new(),
            acceptations: Vec::new(),
            edition: None,
            confirmation: None,
            prefs: Default::default(),
        }
    }
}

impl SuggestionsUi {
    pub fn lecture_en_cours(&self) -> bool {
        self.collecte.is_some() || self.taches.iter().any(|t| t.role == Role::Lecture)
    }

    pub fn generation_en_cours(&self) -> bool {
        !self.attente.is_empty() || self.taches.iter().any(|t| t.role == Role::Son)
    }

    /// Quelque chose travaille en arrière-plan.
    pub fn occupe(&self) -> bool {
        self.lecture_en_cours() || self.generation_en_cours() || !self.acceptations.is_empty()
    }

    /// Plus aucune acceptation en attente de pose (essais).
    #[cfg(test)]
    pub fn acceptations_vides(&self) -> bool {
        self.acceptations.is_empty()
    }
}

/// Un nom de fichier lisible pour un son accepté : « Claquement de porte ».
fn nom_de_fichier(titre: &str) -> String {
    let net: String = titre.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '\'' | '’' | '(' | ')') { c } else { ' ' }).collect();
    let net = net.split_whitespace().collect::<Vec<_>>().join(" ");
    if net.is_empty() { "Son proposé".into() } else { net }
}

impl App {
    // ------------------------------------------------------------------ la portée

    /// La portée choisie dans le panneau.
    pub(crate) fn portee_choisie(&self) -> Portee {
        match self.sug.portee {
            ChoixPortee::Film => Portee::Film,
            ChoixPortee::Sequence => Portee::Sequence(self.doc.sequence_of(self.cur_panel.min(self.doc.panels.len() - 1))),
            ChoixPortee::Plan => Portee::Plans(vec![self.doc.panels[self.cur_panel.min(self.doc.panels.len() - 1)].id]),
            ChoixPortee::Plage => {
                let n = self.doc.panels.len();
                let (a, b) = (self.sug.plage.0.clamp(1, n), self.sug.plage.1.clamp(1, n));
                let (a, b) = (a.min(b), a.max(b));
                Portee::Plans(self.doc.panels[a - 1..b].iter().map(|p| p.id).collect())
            }
        }
    }

    // ------------------------------------------------------------------ la confirmation en ligne

    /// Les rôles qu'une action fait travailler.
    fn roles_de(&self, action: &Action) -> Vec<Role> {
        match action {
            Action::Lire(_) => vec![Role::Lecture],
            Action::Generer(ids) => {
                let mut roles = Vec::new();
                for i in ids.iter().filter_map(|id| self.doc.sons_proposes.get(*id)) {
                    let r = if i.kind.est_voix() { Role::Voix } else { Role::Son };
                    if !roles.contains(&r) {
                        roles.push(r);
                    }
                }
                roles
            }
        }
    }

    /// Faut-il d'abord demander la confirmation d'envoi en ligne ? Si oui, la fenêtre est ouverte et
    /// l'action attend (vrai : l'appelant s'arrête là).
    fn exige_confirmation(&mut self, action: Action) -> bool {
        let r = &self.settings.generation_son;
        let en_ligne = self.roles_de(&action).into_iter().any(|role| r.en_ligne(role).is_some());
        if en_ligne && !self.doc.sons_proposes.envoi_en_ligne_confirme {
            self.sug.confirmation = Some(action);
            self.show_pane(Pane::SuggestionsSonores);
            self.status = "Ce projet n'a pas encore confirmé l'envoi vers un service en ligne : confirme ou annule dans la fenêtre.".into();
            return true;
        }
        false
    }

    /// « Confirmer et continuer » : l'envoi en ligne est permis pour ce projet, et l'action reprend.
    pub(crate) fn confirmer_envoi_en_ligne(&mut self) {
        self.doc.sons_proposes.envoi_en_ligne_confirme = true;
        self.modified = true;
        match self.sug.confirmation.take() {
            Some(Action::Lire(p)) => self.lire_storyboard(p),
            Some(Action::Generer(ids)) => self.generer_indications(ids),
            None => {}
        }
    }

    pub(crate) fn refuser_envoi_en_ligne(&mut self) {
        if self.sug.confirmation.take().is_some() {
            self.status = "Rien n'a été envoyé.".into();
        }
    }

    // ------------------------------------------------------------------ lire le storyboard

    /// Lance la lecture du storyboard sur `portee`.
    pub(crate) fn lire_storyboard(&mut self, portee: Portee) {
        if self.sug.lecture_en_cours() {
            self.status = "Une lecture du storyboard est déjà en cours.".into();
            return;
        }
        let plans = plans_de_la_portee(&self.doc, &portee);
        if plans.is_empty() {
            self.status = "Aucun plan à lire.".into();
            return;
        }
        if self.exige_confirmation(Action::Lire(portee)) {
            return;
        }
        let reglages = self.settings.generation_son.clone();
        let lecteur = self.sug.fabrique.lecteur(&reglages);
        let avec_vignettes = reglages.vignettes && reglages.lecture != Mode::Regles;
        let entrees: Vec<EntreePlan> = plans.iter().map(|&i| entree_plan(&self.doc, i)).collect();
        self.status = format!("Lecture du storyboard : {} plan{} ({})…", entrees.len(), if entrees.len() > 1 { "s" } else { "" }, lecteur.nom());
        // Sans vignette, les textes suffisent : la lecture part tout de suite (au prochain `poll`).
        let suivant = if avec_vignettes { 0 } else { entrees.len() };
        self.sug.collecte = Some(Collecte { lecteur, entrees, suivant, controle: Controle::new() });
    }

    // ------------------------------------------------------------------ générer

    /// Met les propositions `ids` dans la file de génération.
    pub(crate) fn generer_indications(&mut self, ids: Vec<u64>) {
        let ids: Vec<u64> = ids
            .into_iter()
            .filter(|id| self.doc.sons_proposes.get(*id).is_some_and(|i| i.status != Statut::EnGeneration && self.doc.panels.iter().any(|p| p.id == i.plan)))
            .collect();
        if ids.is_empty() {
            return;
        }
        if self.exige_confirmation(Action::Generer(ids.clone())) {
            return;
        }
        let fps = self.doc.fps;
        let voix_defaut = self.settings.generation_son.voix_par_defaut().to_string();
        for id in ids {
            let Some(i) = self.doc.sons_proposes.get_mut(id) else { continue };
            let travail = Travail {
                cue: id,
                genre: i.kind,
                prompt: i.prompt_effectif().to_string(),
                secondes: i.secondes(fps).max(0.5),
                texte: i.text.clone(),
                personnage: i.character.clone(),
                voix: if i.voice.trim().is_empty() { voix_defaut.clone() } else { i.voice.clone() },
            };
            let avant = i.status;
            i.status = Statut::EnGeneration;
            self.sug.attente.push((travail, avant));
        }
        self.status = format!("{} son{} à générer…", self.sug.attente.len(), if self.sug.attente.len() > 1 { "s" } else { "" });
    }

    // ------------------------------------------------------------------ annuler

    /// Annule ce qui travaille : la lecture, la file de génération, la tâche en cours. Les
    /// propositions retrouvent leur état d'avant.
    pub(crate) fn annuler_suggestions(&mut self) {
        let mut annulee = false;
        if let Some(c) = self.sug.collecte.take() {
            c.controle.annuler();
            annulee = true;
        }
        for (travail, avant) in std::mem::take(&mut self.sug.attente) {
            if let Some(i) = self.doc.sons_proposes.get_mut(travail.cue).filter(|i| i.status == Statut::EnGeneration) {
                i.status = avant;
            }
            annulee = true;
        }
        for t in &self.sug.taches {
            t.controle.annuler();
            annulee = true;
        }
        if annulee {
            self.status = "Annulation des suggestions sonores…".into();
        }
    }

    /// Un autre projet s'ouvre : tout ce qui travaillait pour l'ancien s'arrête, rien n'est rendu au nouveau.
    pub(crate) fn reset_suggestions(&mut self) {
        if let Some(c) = self.sug.collecte.take() {
            c.controle.annuler();
        }
        for t in self.sug.taches.drain(..) {
            t.controle.annuler();
        }
        self.sug.attente.clear();
        self.sug.acceptations.clear();
        self.sug.choisie = None;
        self.sug.edition = None;
        self.sug.confirmation = None;
        self.sug.plage = (1, 1);
    }

    // ------------------------------------------------------------------ à chaque image d'écran

    /// Reprend les vignettes à faire, les tâches qui avancent, les sons à poser.
    pub(crate) fn poll_suggestions(&mut self, ctx: &egui::Context) {
        let occupe_avant = self.sug.occupe();
        self.poll_collecte();
        self.demarrer_generation();
        self.poll_taches();
        self.poll_acceptations();
        // « Acceptée » suit le son posé dans le montage (annuler la pose le rend « prête »).
        if !self.doc.sons_proposes.is_empty() {
            let doc = &mut self.doc;
            let reels: Vec<(u64, Statut)> = doc.sons_proposes.indications.iter().filter(|i| i.statut_reel(doc) != i.status).map(|i| (i.id, i.statut_reel(doc))).collect();
            for (id, s) in reels {
                if let Some(i) = doc.sons_proposes.get_mut(id) {
                    i.status = s;
                }
            }
        }
        if occupe_avant || self.sug.occupe() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn poll_collecte(&mut self) {
        let Some(mut c) = self.sug.collecte.take() else { return };
        let debut = Instant::now();
        while c.suivant < c.entrees.len() && (c.suivant == 0 || debut.elapsed() < BUDGET_VIGNETTES) {
            let id = c.entrees[c.suivant].plan;
            if let Some(i) = self.doc.panels.iter().position(|p| p.id == id) {
                c.entrees[c.suivant].vignette_png = vignette_png(&self.doc, i, LARGEUR_VIGNETTE);
            }
            c.suivant += 1;
            c.controle.avancer(c.suivant, c.entrees.len(), "Vignettes");
        }
        if c.suivant < c.entrees.len() {
            self.sug.collecte = Some(c);
            return;
        }
        // Tout est rassemblé : un fil à part lit les plans.
        let titre = format!("Lecture du storyboard ({})", c.lecteur.nom());
        let n = c.entrees.len();
        let demande = DemandeLecture { plans: c.entrees, fps: self.doc.fps };
        let rx = lancer_lecture(c.lecteur, demande, c.controle.clone());
        c.controle.avancer(0, n, "");
        self.sug.taches.push(Tache::new(Role::Lecture, titre, c.controle, rx, Default::default()));
    }

    /// Une file de génération qui attend, et rien qui génère : un fil part avec tout ce qui attend.
    fn demarrer_generation(&mut self) {
        if self.sug.attente.is_empty() || self.sug.taches.iter().any(|t| t.role == Role::Son) {
            return;
        }
        let reglages = self.settings.generation_son.clone();
        let (son, voix) = (self.sug.fabrique.son(&reglages), self.sug.fabrique.voix(&reglages));
        let file = std::mem::take(&mut self.sug.attente);
        let avant = file.iter().map(|(t, s)| (t.cue, *s)).collect();
        let travaux: Vec<Travail> = file.into_iter().map(|(t, _)| t).collect();
        let n = travaux.len();
        // (Le titre nomme ceux qui travaillent : les sons, les voix, ou les deux.)
        let (des_voix, des_sons) = (travaux.iter().any(|t| t.genre.est_voix()), travaux.iter().any(|t| !t.genre.est_voix()));
        let noms: Vec<String> = [des_sons.then(|| son.nom()), des_voix.then(|| voix.nom())].into_iter().flatten().collect();
        let titre = format!("Génération des sons ({})", noms.join(", "));
        let controle = Controle::new();
        controle.avancer(0, n, "");
        let rx = lancer_generation(son, voix, travaux, self.sug.dossier.clone(), controle.clone());
        self.sug.taches.push(Tache::new(Role::Son, titre, controle, rx, avant));
    }

    fn poll_taches(&mut self) {
        let mut rapport = Rapport::default();
        let mut finies = Vec::new();
        let mut taches = std::mem::take(&mut self.sug.taches);
        taches.retain_mut(|t| {
            let fini = t.poll(&mut self.doc.sons_proposes, &mut rapport);
            if fini {
                finies.push((t.role, t.titre.clone(), t.totaux, t.prets, t.controle.est_annule()));
            }
            !fini
        });
        // (Des tâches ont pu être ajoutées pendant ce temps : on les garde.)
        taches.append(&mut self.sug.taches);
        self.sug.taches = taches;
        if rapport.change {
            self.modified = true;
        }
        for (role, titre, totaux, prets, annule) in finies {
            self.status = match (role, annule) {
                (_, true) => format!("{titre} : annulé."),
                (Role::Lecture, _) => format!(
                    "{titre} : {} proposition{} ajoutée{}{}.",
                    totaux.ajoutees,
                    if totaux.ajoutees > 1 { "s" } else { "" },
                    if totaux.ajoutees > 1 { "s" } else { "" },
                    if totaux.connues > 0 { format!(", {} déjà connue{}", totaux.connues, if totaux.connues > 1 { "s" } else { "" }) } else { String::new() }
                ),
                _ => format!("{titre} : {prets} son{} prêt{}.", if prets > 1 { "s" } else { "" }, if prets > 1 { "s" } else { "" }),
            };
        }
        if let Some(e) = rapport.erreurs.into_iter().next() {
            self.status = format!("Suggestions sonores : {e}");
        }
    }

    // ------------------------------------------------------------------ accepter

    /// Accepte la proposition `id` : le son est lu et décodé à part, puis posé dans le montage.
    pub(crate) fn accepter_indication(&mut self, id: u64) {
        let Some(i) = self.doc.sons_proposes.get(id) else { return };
        let Some(path) = i.file.clone().filter(|_| matches!(i.statut_reel(&self.doc), Statut::Prete | Statut::Rejetee)) else {
            self.status = "Ce son n'est pas prêt : génère-le d'abord.".into();
            return;
        };
        if self.sug.acceptations.iter().any(|a| a.id == id) {
            return;
        }
        if debut_montage(&self.doc, i).is_none() {
            self.status = "Le plan de cette proposition n'existe plus.".into();
            return;
        }
        let (tx, rx) = channel();
        let p = path.clone();
        std::thread::spawn(move || {
            let result = std::fs::read(&p).map_err(|e| format!("lecture impossible ({e})")).and_then(|bytes| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
                crate::audio::decode(&bytes, &ext).map(|pcm| (bytes, pcm, ext))
            });
            let _ = tx.send(result);
        });
        self.status = "Chargement du son proposé…".into();
        self.sug.acceptations.push(Acceptation { id, path, rx });
    }

    fn poll_acceptations(&mut self) {
        let mut i = 0;
        let mut pretes = Vec::new();
        while i < self.sug.acceptations.len() {
            match self.sug.acceptations[i].rx.try_recv() {
                Ok(r) => pretes.push((self.sug.acceptations.remove(i), r)),
                Err(TryRecvError::Empty) => i += 1,
                Err(TryRecvError::Disconnected) => {
                    self.sug.acceptations.remove(i);
                    self.status = "Le chargement du son proposé s'est interrompu.".into();
                }
            }
        }
        for (a, r) in pretes {
            match r {
                Ok((bytes, pcm, ext)) => self.poser_son_propose(a.id, bytes, pcm, ext),
                Err(e) => self.status = format!("Impossible d'accepter « {} » : {e}", a.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            }
        }
    }

    /// Le son lu entre dans le projet (copié tel quel) et se pose sur la piste « Sons proposés », à
    /// l'image où il est voulu : une étape d'annulation.
    fn poser_son_propose(&mut self, id: u64, bytes: Vec<u8>, pcm: Pcm, ext: String) {
        let Some(ind) = self.doc.sons_proposes.get(id).cloned() else { return };
        let Some(start) = debut_montage(&self.doc, &ind) else {
            self.status = "Le plan de cette proposition n'existe plus.".into();
            return;
        };
        let name = format!("{}.{ext}", nom_de_fichier(&ind.titre()));
        let fps = self.doc.fps;
        let mut clip = None;
        let avant = self.history.context.replace("Accepter un son proposé".into());
        self.audio_op(|d| {
            let sound = d.add_sound(name.clone(), ext, Arc::new(bytes), Arc::new(pcm));
            let len = d.sounds.get(&sound).map_or(1, |s| s.frames(fps));
            let track = piste_pour(d, start, start + len);
            clip = d.add_clip(track, sound, start, 0, len);
            clip.is_some()
        });
        self.history.context = avant;
        let Some(clip) = clip else { return };
        if let Some(i) = self.doc.sons_proposes.get_mut(id) {
            i.clip = Some(clip);
            i.status = Statut::Acceptee;
        }
        self.sel_clip = Some(clip);
        self.modified = true;
        let (piste, len) = self.doc.find_clip(clip).map_or((String::new(), 0), |(t, c)| (self.doc.audio_tracks[t].name.clone(), self.doc.audio_tracks[t].clips[c].length));
        self.status = format!("Son accepté : « {} » ({}), à {}, sur la piste « {piste} ».", ind.titre(), crate::storyboard::duration_label(len, fps), timecode(start, fps));
    }

    // ------------------------------------------------------------------ les autres commandes

    pub(crate) fn rejeter_indication(&mut self, id: u64) {
        if let Some(i) = self.doc.sons_proposes.get_mut(id).filter(|i| matches!(i.status, Statut::Proposee | Statut::Prete | Statut::Rejetee)) {
            i.status = Statut::Rejetee;
            self.modified = true;
        }
    }

    /// Remet une proposition rejetée dans la liste des propositions.
    pub(crate) fn rouvrir_indication(&mut self, id: u64) {
        if let Some(i) = self.doc.sons_proposes.get_mut(id).filter(|i| i.status == Statut::Rejetee) {
            i.status = if i.file.is_some() { Statut::Prete } else { Statut::Proposee };
            self.modified = true;
        }
    }

    pub(crate) fn supprimer_indication(&mut self, id: u64) {
        if self.doc.sons_proposes.get(id).is_some_and(|i| i.status != Statut::EnGeneration) && self.doc.sons_proposes.supprimer(id) {
            if self.sug.choisie == Some(id) {
                self.sug.choisie = None;
            }
            self.modified = true;
        }
    }

    /// Supprime les propositions rejetées.
    pub(crate) fn supprimer_rejetees(&mut self) {
        let ids: Vec<u64> = self.doc.sons_proposes.indications.iter().filter(|i| i.status == Statut::Rejetee).map(|i| i.id).collect();
        for id in &ids {
            self.supprimer_indication(*id);
        }
        if !ids.is_empty() {
            self.status = format!("{} proposition{} rejetée{} supprimée{}.", ids.len(), if ids.len() > 1 { "s" } else { "" }, if ids.len() > 1 { "s" } else { "" }, if ids.len() > 1 { "s" } else { "" });
        }
    }

    /// Écoute (ou arrête) le son généré d'une proposition.
    pub(crate) fn ecouter_indication(&mut self, id: u64) {
        let Some(path) = self.doc.sons_proposes.get(id).and_then(|i| i.file.clone()) else {
            self.status = "Ce son n'est pas encore généré.".into();
            return;
        };
        self.listen_bruitage(&path);
    }

    /// Enregistre la fenêtre « Modifier ».
    pub(crate) fn appliquer_edition(&mut self, e: Edition) {
        let Some(i) = self.doc.sons_proposes.get_mut(e.id) else { return };
        i.kind = e.kind;
        i.start = e.start;
        i.frames = e.frames.clamp(1, crate::indications::IMAGES_MAX);
        i.description = e.description.trim().to_string();
        i.prompt = e.prompt.trim().to_string();
        i.text = e.text.trim().to_string();
        i.character = e.character.trim().to_string();
        i.voice = e.voice.trim().to_string();
        i.edited = true;
        self.modified = true;
    }

    /// Les propositions dans l'ordre du film.
    pub(crate) fn indications_en_ordre(&self) -> Vec<u64> {
        let rang: std::collections::HashMap<u64, usize> = self.doc.panels.iter().enumerate().map(|(k, p)| (p.id, k)).collect();
        let mut v: Vec<&Indication> = self.doc.sons_proposes.indications.iter().filter(|i| rang.contains_key(&i.plan)).collect();
        v.sort_by_key(|i| (rang[&i.plan], i.start, i.id));
        v.into_iter().map(|i| i.id).collect()
    }

    /// Les commandes `suggestion.*` ; vrai si `action` en était une.
    pub(crate) fn suggestion_action(&mut self, action: &str) -> bool {
        match action {
            "suggestion.lire" => {
                self.show_pane(Pane::SuggestionsSonores);
                let portee = self.portee_choisie();
                self.lire_storyboard(portee);
                true
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------------ le panneau

    pub(crate) fn suggestions_panel(&mut self, ui: &mut egui::Ui) {
        let occupe = self.sug.occupe();
        // --- la portée et la lecture
        ui.horizontal_wrapped(|ui| {
            let actuel = self.sug.portee;
            egui::ComboBox::from_id_salt("portee_suggestions").selected_text(actuel.nom()).width(150.0).show_ui(ui, |ui| {
                for choix in ChoixPortee::ALL {
                    ui.selectable_value(&mut self.sug.portee, choix, choix.nom());
                }
            });
            if self.sug.portee == ChoixPortee::Plage {
                let n = self.doc.panels.len().max(1);
                ui.label("du plan");
                ui.add(egui::DragValue::new(&mut self.sug.plage.0).range(1..=n));
                ui.label("au");
                ui.add(egui::DragValue::new(&mut self.sug.plage.1).range(1..=n));
                ui.label(RichText::new("(rang dans le film)").small().weak());
            }
        });
        ui.horizontal(|ui| {
            let lecture = self.sug.lecture_en_cours();
            let bouton = egui::Button::new(format!("{}  Lire le storyboard", ph::BOOK_OPEN_TEXT));
            let resp = ui.add_enabled(!lecture, bouton);
            if crate::bulles::bulle(resp, "Lire le storyboard", "SHOT lit les textes (et les vignettes, si tu l'as choisi) des plans de la portée et propose des bruitages, des ambiances et des voix. Rien n'est posé dans le montage avant que tu acceptes.", &self.raccourci("suggestion.lire")).clicked() {
                let portee = self.portee_choisie();
                self.lire_storyboard(portee);
            }
            if crate::bulles::icone_si(ui, occupe, ph::STOP, "Annuler", "Arrête la lecture ou la génération en cours.", "").clicked() {
                self.annuler_suggestions();
            }
        });
        self.suggestions_fournisseurs(ui);
        self.suggestions_avancement(ui);
        ui.separator();
        // --- la liste
        let ordre = self.indications_en_ordre();
        if ordre.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Aucune suggestion.").strong());
            ui.label("« Lire le storyboard » lit les textes de tes plans — l'action, le dialogue, les notes — et propose les sons qu'ils demandent. Tu les écoutes, tu acceptes ceux que tu gardes : ils se posent sur la piste « Sons proposés ». Ce sont des sons témoins, pour l'animatique.");
            ui.add_space(4.0);
            ui.label(RichText::new("Astuce : une ligne « SFX : porte qui grince, pas dans l'escalier » dans les notes d'un plan devient deux propositions.").small().weak());
            return;
        }
        self.suggestions_barre_globale(ui, &ordre);
        self.suggestions_barre_choix(ui);
        ui.add_space(2.0);
        let fps = self.doc.fps;
        let lignes: Vec<Ligne> = ordre.iter().filter_map(|&id| self.ligne_de(id, fps)).collect();
        let choisie = self.sug.choisie;
        let mut clic: Option<u64> = None;
        let mut double: Option<u64> = None;
        let mut menu: Option<(u64, MenuAction)> = None;
        let hauteur = ui.available_height();
        egui::ScrollArea::vertical().id_salt("liste_suggestions").auto_shrink([false, false]).max_height(hauteur).show_rows(ui, ROW_H, lignes.len(), |ui, plage| {
            for l in &lignes[plage] {
                let resp = dessiner_ligne(ui, l, choisie == Some(l.id));
                if resp.clicked() {
                    clic = Some(l.id);
                }
                if resp.double_clicked() {
                    double = Some(l.id);
                }
                resp.context_menu(|ui| {
                    if let Some(a) = menu_actions(ui, l) {
                        menu = Some((l.id, a));
                    }
                });
            }
        });
        if let Some(id) = clic {
            self.sug.choisie = Some(id);
        }
        if let Some(id) = double {
            self.sug.choisie = Some(id);
            let l = lignes.iter().find(|l| l.id == id);
            if l.is_some_and(|l| l.a_un_fichier) {
                self.ecouter_indication(id);
            } else {
                self.generer_indications(vec![id]);
            }
        }
        if let Some((id, a)) = menu {
            self.sug.choisie = Some(id);
            self.lancer_menu_action(id, a);
        }
    }

    /// Qui fait quoi, en une ligne, et ce qui part en ligne.
    fn suggestions_fournisseurs(&mut self, ui: &mut egui::Ui) {
        let r = &self.settings.generation_son;
        let nom = |role: Role| match r.mode(role) {
            Mode::Regles => "règles".to_string(),
            Mode::Local => "local".to_string(),
            Mode::EnLigne => r.service(role).court().to_string(),
        };
        let texte = format!("Lecture : {} · Sons : {} · Voix : {}", nom(Role::Lecture), nom(Role::Son), nom(Role::Voix));
        let en_ligne: Vec<String> = r
            .roles_en_ligne()
            .into_iter()
            .map(|(role, service)| match role {
                Role::Lecture if r.vignettes => format!("Les vignettes des plans et leurs textes seront envoyés à {}.", service.court()),
                Role::Lecture => format!("Les textes des plans seront envoyés à {} (sans les vignettes).", service.court()),
                Role::Son => format!("Les descriptions des sons seront envoyées à {}.", service.court()),
                Role::Voix => format!("Les répliques seront envoyées à {}.", service.court()),
            })
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(texte).small().weak());
            if ui.small_button(ph::GEAR).on_hover_text("Choisir les fournisseurs (Fichier > Préférences > Génération)").clicked() {
                self.settings.prefs_onglet = crate::prefsonglets::Onglet::Generation;
                self.prefs_window = true;
            }
        });
        if !en_ligne.is_empty() {
            let confirme = self.doc.sons_proposes.envoi_en_ligne_confirme;
            for phrase in en_ligne {
                ui.label(RichText::new(format!("{}  {phrase}", ph::CLOUD_ARROW_UP)).small().color(ui.visuals().warn_fg_color));
            }
            ui.label(RichText::new(if confirme { "Envoi en ligne confirmé pour ce projet." } else { "Une confirmation te sera demandée avant le premier envoi." }).small().weak());
        }
    }

    /// La barre d'avancement de la tâche qui travaille.
    fn suggestions_avancement(&mut self, ui: &mut egui::Ui) {
        let mut lignes: Vec<(String, f32, String)> = Vec::new();
        if let Some(c) = &self.sug.collecte {
            let (fait, total, _) = c.controle.lire();
            lignes.push(("Préparation des vignettes".into(), fait as f32 / total.max(1) as f32, format!("{fait} / {total}")));
        }
        for t in &self.sug.taches {
            let (fait, total, texte) = t.controle.lire();
            let part = if total == 0 { 0.0 } else { fait as f32 / total as f32 };
            let annule = if t.controle.est_annule() { " — annulation…" } else { "" };
            lignes.push((format!("{}{annule}", t.titre), part, if texte.is_empty() { format!("{fait} / {total}") } else { format!("{fait} / {total} · {texte}") }));
        }
        if !self.sug.attente.is_empty() {
            lignes.push(("En attente".into(), 0.0, format!("{} son(s)", self.sug.attente.len())));
        }
        for (titre, part, texte) in lignes {
            ui.label(RichText::new(titre).small());
            ui.add(egui::ProgressBar::new(part).text(RichText::new(texte).small()).desired_height(14.0));
        }
    }

    /// Les commandes pour toute la liste.
    fn suggestions_barre_globale(&mut self, ui: &mut egui::Ui, ordre: &[u64]) {
        let (a_generer, pretes, rejetees) = {
            let s = &self.doc.sons_proposes;
            let compte = |f: &dyn Fn(&Indication) -> bool| ordre.iter().filter_map(|id| s.get(*id)).filter(|i| f(i)).count();
            (compte(&|i| i.status == Statut::Proposee), compte(&|i| i.statut_reel(&self.doc) == Statut::Prete), compte(&|i| i.status == Statut::Rejetee))
        };
        ui.horizontal(|ui| {
            if crate::bulles::icone_si(ui, a_generer > 0, ph::SPARKLE, "Tout générer", "Génère le son de chaque proposition qui n'en a pas encore, l'une après l'autre.", "").clicked() {
                let ids: Vec<u64> = ordre.iter().copied().filter(|id| self.doc.sons_proposes.get(*id).is_some_and(|i| i.status == Statut::Proposee)).collect();
                self.generer_indications(ids);
            }
            if crate::bulles::icone_si(ui, pretes > 0, ph::CHECKS, "Tout accepter", "Pose dans le montage tous les sons prêts, chacun à son image, sur la piste « Sons proposés ».", "").clicked() {
                let ids: Vec<u64> = ordre.iter().copied().filter(|id| self.doc.sons_proposes.get(*id).is_some_and(|i| i.statut_reel(&self.doc) == Statut::Prete)).collect();
                for id in ids {
                    self.accepter_indication(id);
                }
            }
            if crate::bulles::icone_si(ui, rejetees > 0, ph::TRASH, "Supprimer les rejetées", "Retire de la liste les propositions que tu as rejetées.", "").clicked() {
                self.supprimer_rejetees();
            }
            let n = ordre.len();
            ui.label(RichText::new(format!("{n} proposition{}", if n > 1 { "s" } else { "" })).weak());
        });
    }

    /// Les commandes de la proposition choisie.
    fn suggestions_barre_choix(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.sug.choisie else {
            ui.label(RichText::new("Choisis une proposition pour la générer, l'écouter ou l'accepter.").small().weak());
            return;
        };
        let fps = self.doc.fps;
        let Some(l) = self.ligne_de(id, fps) else {
            self.sug.choisie = None;
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(l.genre.icone()).size(16.0));
            ui.label(RichText::new(&l.titre).strong());
        });
        ui.horizontal(|ui| {
            if let Some(a) = barre_actions(ui, &l) {
                self.lancer_menu_action(id, a);
            }
        });
    }

    fn lancer_menu_action(&mut self, id: u64, a: MenuAction) {
        match a {
            MenuAction::Generer => self.generer_indications(vec![id]),
            MenuAction::Ecouter => self.ecouter_indication(id),
            MenuAction::Accepter => self.accepter_indication(id),
            MenuAction::Rejeter => self.rejeter_indication(id),
            MenuAction::Rouvrir => self.rouvrir_indication(id),
            MenuAction::Modifier => {
                if let Some(i) = self.doc.sons_proposes.get(id) {
                    self.sug.edition = Some(Edition { id, kind: i.kind, start: i.start, frames: i.frames, description: i.description.clone(), prompt: i.prompt.clone(), text: i.text.clone(), character: i.character.clone(), voice: i.voice.clone() });
                }
            }
            MenuAction::Supprimer => self.supprimer_indication(id),
        }
    }

    /// Ce que la liste montre d'une proposition.
    fn ligne_de(&self, id: u64, fps: u32) -> Option<Ligne> {
        let i = self.doc.sons_proposes.get(id)?;
        let rang = self.doc.panels.iter().position(|p| p.id == i.plan)?;
        let ecoute = self.brt.lib.ecoute.courant();
        Some(Ligne {
            id,
            genre: i.kind,
            plan: panel_title(&self.doc, rang),
            temps: timecode(i.start, fps),
            titre: i.titre(),
            detail: if i.kind.est_voix() { i.text.clone() } else { i.prompt_effectif().to_string() },
            statut: i.statut_reel(&self.doc),
            a_un_fichier: i.file.is_some(),
            ecoute: i.file.as_deref().is_some_and(|f| ecoute == Some(f)),
        })
    }

    // ------------------------------------------------------------------ les fenêtres

    /// La confirmation d'envoi en ligne et la fenêtre « Modifier » ; à appeler à chaque image.
    pub(crate) fn suggestions_windows(&mut self, ctx: &egui::Context) {
        self.confirmation_window(ctx);
        self.edition_window(ctx);
        self.cle_window(ctx);
    }

    fn confirmation_window(&mut self, ctx: &egui::Context) {
        let Some(action) = self.sug.confirmation.clone() else { return };
        let r = self.settings.generation_son.clone();
        let mut phrases: Vec<String> = Vec::new();
        for role in self.roles_de(&action) {
            if r.en_ligne(role).is_none() {
                continue;
            }
            let phrase = match role {
                Role::Lecture => self.sug.fabrique.lecteur(&r).envoie_en_ligne(),
                Role::Son => self.sug.fabrique.son(&r).envoie_en_ligne(),
                Role::Voix => self.sug.fabrique.voix(&r).envoie_en_ligne(),
            };
            phrases.push(phrase.unwrap_or_else(|| format!("des données de ce projet partent vers {}", r.service(role).court())));
        }
        let mut choix: Option<bool> = None;
        egui::Window::new("Envoyer vers un service en ligne ?").collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.label("Ce que tu as choisi envoie des éléments de ce projet hors de cet ordinateur :");
            for p in &phrases {
                ui.label(format!("•  {p}"));
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Les conditions d'utilisation du service s'appliquent (stockage, réutilisation éventuelle des données, usage commercial des sons) : à vérifier avant un projet confidentiel. La clé est celle de ton compte, rangée dans le Gestionnaire d'identifiants de Windows.").small().weak());
            ui.label(RichText::new("Ta confirmation vaut pour ce projet : elle ne sera plus demandée.").small().weak());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Confirmer et continuer").clicked() {
                    choix = Some(true);
                }
                if ui.button("Annuler").clicked() {
                    choix = Some(false);
                }
            });
        });
        match choix {
            Some(true) => self.confirmer_envoi_en_ligne(),
            Some(false) => self.refuser_envoi_en_ligne(),
            None => {}
        }
    }

    fn edition_window(&mut self, ctx: &egui::Context) {
        let Some(mut e) = self.sug.edition.clone() else { return };
        let mut ouvert = true;
        let mut fin: Option<bool> = None;
        let fps = self.doc.fps;
        egui::Window::new("Modifier la proposition").open(&mut ouvert).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_width(420.0);
            egui::Grid::new("edition_suggestion").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label("Sorte");
                egui::ComboBox::from_id_salt("edition_genre").selected_text(e.kind.nom()).show_ui(ui, |ui| {
                    for g in Genre::ALL {
                        ui.selectable_value(&mut e.kind, g, g.nom());
                    }
                });
                ui.end_row();
                ui.label("Image du plan");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut e.start).range(0..=100_000));
                    ui.label(RichText::new(timecode(e.start, fps)).small().weak());
                });
                ui.end_row();
                ui.label("Durée voulue");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut e.frames).range(1..=crate::indications::IMAGES_MAX).suffix(" i"));
                    ui.label(RichText::new(crate::storyboard::duration_label(e.frames, fps)).small().weak());
                });
                ui.end_row();
                ui.label("Description");
                ui.add(egui::TextEdit::multiline(&mut e.description).desired_rows(2).desired_width(f32::INFINITY));
                ui.end_row();
                if e.kind.est_voix() {
                    ui.label("Personnage");
                    ui.add(egui::TextEdit::singleline(&mut e.character).desired_width(f32::INFINITY));
                    ui.end_row();
                    ui.label("Réplique");
                    ui.add(egui::TextEdit::multiline(&mut e.text).desired_rows(3).desired_width(f32::INFINITY));
                    ui.end_row();
                    ui.label("Voix");
                    ui.add(egui::TextEdit::singleline(&mut e.voice).hint_text("celle des Préférences").desired_width(f32::INFINITY));
                    ui.end_row();
                } else {
                    ui.label("Prompt (anglais)");
                    ui.add(egui::TextEdit::multiline(&mut e.prompt).desired_rows(3).hint_text("ce que le générateur reçoit").desired_width(f32::INFINITY));
                    ui.end_row();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Enregistrer").clicked() {
                    fin = Some(true);
                }
                if ui.button("Annuler").clicked() {
                    fin = Some(false);
                }
            });
        });
        match fin {
            Some(true) => {
                self.appliquer_edition(e);
                self.sug.edition = None;
            }
            Some(false) => self.sug.edition = None,
            None if !ouvert => self.sug.edition = None,
            None => self.sug.edition = Some(e),
        }
    }
}

// ----------------------------------------------------------------------------- une ligne

/// Ce que la liste montre d'une proposition (copié, pour dessiner sans tenir le projet).
#[derive(Clone, Debug)]
struct Ligne {
    id: u64,
    genre: Genre,
    plan: String,
    temps: String,
    titre: String,
    detail: String,
    statut: Statut,
    a_un_fichier: bool,
    ecoute: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuAction {
    Generer,
    Ecouter,
    Accepter,
    Rejeter,
    Rouvrir,
    Modifier,
    Supprimer,
}

fn couleur_statut(ui: &egui::Ui, s: Statut) -> egui::Color32 {
    match s {
        Statut::Proposee => theme::texte_doux(),
        Statut::EnGeneration => ui.visuals().warn_fg_color,
        Statut::Prete | Statut::Acceptee => theme::accent_texte(),
        Statut::Rejetee => theme::texte_doux(),
    }
}

fn dessiner_ligne(ui: &mut egui::Ui, l: &Ligne, choisie: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
    let painter = ui.painter_at(rect);
    if choisie {
        painter.rect_filled(rect, 3.0, ui.visuals().selection.bg_fill.gamma_multiply(0.55));
    } else if resp.hovered() {
        painter.rect_filled(rect, 3.0, theme::eleve());
    }
    let rejetee = l.statut == Statut::Rejetee;
    let encre = if rejetee { theme::texte_doux() } else { theme::texte() };
    painter.text(pos2(rect.left() + 16.0, rect.center().y), Align2::CENTER_CENTER, l.genre.icone(), FontId::proportional(18.0), if rejetee { theme::texte_doux() } else { theme::accent_texte() });
    let gauche = rect.left() + 34.0;
    let etat = if l.ecoute { format!("{}  {}", ph::SPEAKER_HIGH, l.statut.nom()) } else if l.statut == Statut::Acceptee { format!("{}  {}", ph::CHECK, l.statut.nom()) } else { l.statut.nom().to_string() };
    let galerie = painter.layout_no_wrap(etat, FontId::proportional(11.5), couleur_statut(ui, l.statut));
    let largeur_etat = galerie.size().x;
    painter.galley(pos2(rect.right() - 6.0 - largeur_etat, rect.top() + 6.0), galerie, theme::texte_doux());
    let zone = egui::Rect::from_min_max(pos2(gauche, rect.top()), pos2(rect.right() - largeur_etat - 12.0, rect.bottom()));
    let p = painter.with_clip_rect(zone);
    p.text(pos2(gauche, rect.top() + 14.0), Align2::LEFT_CENTER, format!("{} · {}", l.plan, l.temps), FontId::monospace(11.0), theme::texte_doux());
    painter.with_clip_rect(egui::Rect::from_min_max(pos2(gauche, rect.top()), pos2(rect.right() - 6.0, rect.bottom())))
        .text(pos2(gauche, rect.top() + 31.0), Align2::LEFT_CENTER, &l.titre, FontId::proportional(12.5), encre);
    resp.on_hover_text(format!("{} — {}\n{}\n\nClic : choisir · double-clic : écouter (ou générer) · clic droit : toutes les commandes", l.genre.nom(), l.titre, l.detail))
}

/// Les commandes qui valent pour une proposition, selon son état.
fn actions_possibles(l: &Ligne) -> [(MenuAction, bool); 7] {
    let libre = l.statut != Statut::EnGeneration;
    [
        (MenuAction::Generer, libre),
        (MenuAction::Ecouter, l.a_un_fichier),
        (MenuAction::Accepter, matches!(l.statut, Statut::Prete | Statut::Rejetee) && l.a_un_fichier),
        (MenuAction::Rejeter, matches!(l.statut, Statut::Proposee | Statut::Prete)),
        (MenuAction::Rouvrir, l.statut == Statut::Rejetee),
        (MenuAction::Modifier, libre),
        (MenuAction::Supprimer, libre),
    ]
}

fn nom_action(a: MenuAction, l: &Ligne) -> (&'static str, &'static str, &'static str) {
    match a {
        MenuAction::Generer if l.a_un_fichier => (ph::ARROWS_CLOCKWISE, "Régénérer", "Génère un autre son pour cette proposition."),
        MenuAction::Generer => (ph::SPARKLE, "Générer", "Génère le son de cette proposition."),
        MenuAction::Ecouter if l.ecoute => (ph::STOP, "Arrêter l'écoute", "Arrête le son."),
        MenuAction::Ecouter => (ph::PLAY, "Écouter", "Écoute le son généré."),
        MenuAction::Accepter => (ph::CHECK, "Accepter", "Pose le son dans le montage, à l'image voulue, sur la piste « Sons proposés »."),
        MenuAction::Rejeter => (ph::X, "Rejeter", "Écarte cette proposition (elle ne reviendra pas à la prochaine lecture)."),
        MenuAction::Rouvrir => (ph::ARROW_COUNTER_CLOCKWISE, "Reprendre", "Remet cette proposition parmi les propositions."),
        MenuAction::Modifier => (ph::PENCIL_SIMPLE, "Modifier", "Change la description, le prompt, la réplique, la voix ou le moment."),
        MenuAction::Supprimer => (ph::TRASH, "Supprimer", "Retire cette proposition de la liste."),
    }
}

/// Les boutons-icônes de la proposition choisie.
fn barre_actions(ui: &mut egui::Ui, l: &Ligne) -> Option<MenuAction> {
    let mut choix = None;
    for (a, actif) in actions_possibles(l) {
        if a == MenuAction::Rouvrir && l.statut != Statut::Rejetee {
            continue;
        }
        let (icone, nom, aide) = nom_action(a, l);
        if crate::bulles::icone_si(ui, actif, icone, nom, aide, "").clicked() {
            choix = Some(a);
        }
    }
    choix
}

/// Le menu du clic droit : les mêmes commandes, avec leur nom.
fn menu_actions(ui: &mut egui::Ui, l: &Ligne) -> Option<MenuAction> {
    let mut choix = None;
    for (a, actif) in actions_possibles(l) {
        if a == MenuAction::Rouvrir && l.statut != Statut::Rejetee {
            continue;
        }
        let (icone, nom, _) = nom_action(a, l);
        if ui.add_enabled(actif, egui::Button::new(format!("{icone}  {nom}"))).clicked() {
            choix = Some(a);
            ui.close_menu();
        }
    }
    choix
}
