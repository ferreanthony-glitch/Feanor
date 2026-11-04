//! Fenêtre « Kitsu » (Fichier > Exporter > Montage & échange, actions `fichier.kitsu_publier` et `fichier.kitsu_retours`) : la connexion, la production
//! (projet, type de tâche, statuts), la publication et la lecture des retours. Le protocole est dans `kitsu.rs`.
//!
//! Règles : le pont est **éteint d'origine** et rien ne part sans un clic ; le mot de passe n'est qu'en mémoire (vidé après la connexion) ; tout ce qui
//! parle au réseau se fait sur un fil à part (« sans latence ») avec « Arrêter » ; les prévisualisations (vignettes, MP4 par plan) sont fabriquées par
//! les tâches d'export habituelles, **seulement pour les plans qui ont changé** depuis la dernière publication.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, vec2, Align2, Color32, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::document::Document;
use crate::editorial::{self, Media};
use crate::editorialjob::EditorialJob;
use crate::export::{Encoders, ExportJob, Progress};
use crate::exportstills::{self, StillsPer, StillsSettings};
use crate::kitsu::{self, Client, Element, Erreur, Fichiers, Ids, Plan, Progres, Rapport, RetourPlan, StatutKitsu, TypeTache};
use crate::relectureui::Etendue;
use crate::theme;

/// Ce que rend un fil.
pub(crate) enum Sortie {
    Jeton { jeton: String, nouveau: bool },
    Listes { projets: Vec<Element>, types: Vec<TypeTache>, statuts: Vec<StatutKitsu> },
    Apercu(Box<Plan>),
    Publie(Rapport),
    Retours(Vec<RetourPlan>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Genre {
    Connexion,
    Listes,
    Apercu,
    Publication,
    Retours,
}

impl Genre {
    fn nom(self) -> &'static str {
        match self {
            Genre::Connexion => "Connexion",
            Genre::Listes => "Lecture des listes",
            Genre::Apercu => "Aperçu",
            Genre::Publication => "Publication",
            Genre::Retours => "Lecture des retours",
        }
    }
}

pub(crate) struct Travail {
    genre: Genre,
    rx: Receiver<Result<Sortie, Erreur>>,
    annule: Arc<AtomicBool>,
    prog: Arc<Progres>,
    /// Le dossier temporaire des prévisualisations, effacé à la fin.
    menage: Option<PathBuf>,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Section {
    #[default]
    Connexion,
    Publier,
    Retours,
}

/// L'aperçu d'une publication, et ce pour quoi il vaut.
pub(crate) struct ApercuPret {
    plan: Plan,
    cle: (u64, u64, bool, bool, bool),
}

/// Tout l'état du pont. `session` (jeton, listes) survit à la fenêtre ; le reste se vide quand le projet change.
#[derive(Default)]
pub(crate) struct Kitsu {
    pub(crate) ouvert: bool,
    pub(crate) section: Section,
    epoch: u64,
    mot_de_passe: String,
    jeton: Option<(String, String)>,
    projets: Vec<Element>,
    types: Vec<TypeTache>,
    statuts: Vec<StatutKitsu>,
    message: Option<Result<String, String>>,
    travail: Option<Travail>,
    apercu: Option<ApercuPret>,
    rendu: Option<Rendu>,
    /// Statut de la tâche storyboard lu dans Kitsu, par identifiant de plan : (nom, couleur).
    pub(crate) marques: HashMap<u64, (String, Option<[u8; 3]>)>,
    marques_epoch: Option<u64>,
    /// Le jeton gardé a déjà été cherché (une fois par ouverture de la fenêtre).
    jeton_cherche: bool,
}

impl Kitsu {
    /// Le projet est copié pour les prévisualisations (quelques images d'écran) : `App::modal_open`.
    pub(crate) fn rendering(&self) -> bool {
        self.rendu.as_ref().is_some_and(Rendu::needs_project)
    }

    /// Un fil ou un rendu tourne (« Quitter » le dit).
    pub(crate) fn busy(&self) -> bool {
        self.travail.is_some() || self.rendu.is_some()
    }

    /// Pour les essais : la session comme après « Tester la connexion ».
    #[cfg(test)]
    pub(crate) fn poser_session_pour_essai(&mut self, serveur: &str, jeton: &str) {
        self.jeton = Some((serveur.to_string(), jeton.to_string()));
    }

    #[cfg(test)]
    pub(crate) fn message_pour_essai(&self) -> String {
        match &self.message {
            Some(Ok(m)) | Some(Err(m)) => m.clone(),
            None => String::new(),
        }
    }

    fn arreter(&mut self) {
        if let Some(r) = &mut self.rendu {
            r.cancel();
        }
        self.rendu = None;
        if let Some(t) = &self.travail {
            t.annule.store(true, Ordering::Relaxed);
        }
    }

    fn lancer<F>(&mut self, ctx: &egui::Context, genre: Genre, epoch: u64, menage: Option<PathBuf>, f: F)
    where
        F: FnOnce(&AtomicBool, &Progres) -> Result<Sortie, Erreur> + Send + 'static,
    {
        let annule = Arc::new(AtomicBool::new(false));
        let prog = Arc::new(Progres::default());
        prog.dire(genre.nom());
        let (tx, rx) = channel();
        let (a2, p2, wake) = (annule.clone(), prog.clone(), ctx.clone());
        let started = std::thread::Builder::new().name("kitsu".into()).spawn(move || {
            let r = f(&a2, &p2);
            let _ = tx.send(r);
            wake.request_repaint();
        });
        match started {
            Ok(_) => {
                self.message = None;
                self.travail = Some(Travail { genre, rx, annule, prog, menage, epoch });
            }
            Err(e) => {
                self.message = Some(Err(format!("Le fil de travail n'a pas pu démarrer : {e}")));
                if let Some(d) = menage {
                    let _ = std::fs::remove_dir_all(d);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------- les prévisualisations

#[derive(Clone, Copy, PartialEq, Eq)]
enum Etape {
    Vignettes,
    Videos,
}

enum Courant {
    Export(ExportJob),
    Montage(EditorialJob),
}

/// Fabrique les vignettes et les vidéos des plans à prévisualiser, avec les tâches d'export habituelles, dans un dossier temporaire.
struct Rendu {
    dir: PathBuf,
    plan: Plan,
    etapes: VecDeque<Etape>,
    courant: Option<(Courant, Etape)>,
    snapshot: Option<Arc<Document>>,
    ffmpeg: Option<(PathBuf, Encoders)>,
    exact: Vec<crate::document::CelId>,
    frame: [usize; 2],
    /// (identifiant du plan, nom du fichier sans extension).
    vignettes: Vec<(u64, exportstills::Still)>,
    video: Option<(editorial::Plan, editorial::Options, Vec<(usize, u64)>)>,
    fichiers: Fichiers,
    total: usize,
}

static NUMERO: AtomicUsize = AtomicUsize::new(0);

impl Rendu {
    fn demarrer(doc: &Document, plan: Plan, vignettes: bool, videos: bool, cur: usize, ffmpeg: Option<(PathBuf, Encoders)>, exact: Vec<crate::document::CelId>) -> Result<Rendu, String> {
        let racine = crate::donnees::local().unwrap_or_else(std::env::temp_dir).join("Kitsu");
        let dir = racine.join(format!("{}-{}", std::process::id(), NUMERO.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
        let voulus: Vec<u64> = plan.apercus_a_faire();
        let rangs: Vec<(usize, u64)> = doc.panels.iter().enumerate().filter(|(_, p)| voulus.contains(&p.id)).map(|(i, p)| (i, p.id)).collect();
        let mut etapes = VecDeque::new();
        let mut items = Vec::new();
        if vignettes {
            let plans: Vec<usize> = rangs.iter().map(|(i, _)| *i).collect();
            for s in exportstills::items(doc, &plans, StillsPer::Plan) {
                if let Some((_, id)) = rangs.iter().find(|(i, _)| *i == s.frame.0) {
                    items.push((*id, s));
                }
            }
            etapes.push_back(Etape::Vignettes);
        }
        let mut video = None;
        if videos {
            let (_, enc) = ffmpeg.as_ref().ok_or("ffmpeg est introuvable ou inutilisable : il fabrique les vidéos (Préférences, ou décoche les vidéos).")?;
            if enc.h264.is_none() {
                return Err("Ce ffmpeg ne sait pas encoder en H.264.".into());
            }
            let opts = editorial::Options { edl: false, xml: false, otio: false, fcpxml: false, markers: false, sound: false, media: Media::H264, half: true, whole_film: true, ..editorial::Options::default() };
            let mut p = editorial::build_plan(doc, &opts, cur, "kitsu")?;
            p.clips.retain(|c| rangs.iter().any(|(i, _)| *i == c.panel));
            if !p.clips.is_empty() {
                video = Some((p, opts, rangs.clone()));
                etapes.push_back(Etape::Videos);
            }
        }
        let total = etapes.len();
        Ok(Rendu { dir, plan, etapes, courant: None, snapshot: None, ffmpeg, exact, frame: doc.frame, vignettes: items, video, fichiers: Fichiers::default(), total })
    }

    fn needs_project(&self) -> bool {
        self.snapshot.is_none()
            && match &self.courant {
                Some((Courant::Export(j), _)) => j.needs_project(),
                Some((Courant::Montage(j), _)) => j.needs_project(),
                None => !self.etapes.is_empty(),
            }
    }

    fn status(&self) -> (String, f32) {
        let fait = self.total - self.etapes.len() - usize::from(self.courant.is_some());
        let (nom, part) = match &self.courant {
            Some((Courant::Export(j), _)) => (
                "vignettes",
                match j.progress() {
                    Progress::Running(d, t) => d as f32 / t.max(1) as f32,
                    _ => 0.99,
                },
            ),
            Some((Courant::Montage(j), _)) => {
                let (d, t) = j.frames();
                ("vidéos par plan", d as f32 / t.max(1) as f32)
            }
            None => ("préparation", 0.0),
        };
        (format!("Prévisualisations, étape {} sur {} : {nom}", fait + 1, self.total.max(1)), (fait as f32 + part) / self.total.max(1) as f32)
    }

    fn cancel(&mut self) {
        match self.courant.take() {
            Some((Courant::Export(mut j), _)) => j.cancel(),
            Some((Courant::Montage(mut j), _)) => j.cancel(),
            None => {}
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }

    /// Avance : `Some(Ok(()))` quand tout est fabriqué, `Some(Err)` en cas d'échec, `None` tant que ça travaille.
    fn step(&mut self, doc: &Document, budget: Duration) -> Option<Result<(), String>> {
        loop {
            if self.courant.is_none() {
                let Some(e) = self.etapes.pop_front() else { return Some(Ok(())) };
                match self.lancer(e) {
                    Ok(c) => self.courant = Some((c, e)),
                    Err(err) => return Some(Err(err)),
                }
            }
            let mut fini: Option<Result<(), String>> = None;
            if let Some((c, _)) = &mut self.courant {
                match c {
                    Courant::Export(j) => {
                        j.step(doc, budget);
                        if self.snapshot.is_none() {
                            self.snapshot = j.snapshot();
                        }
                        fini = j.outcome.as_ref().map(|o| o.clone().map(|_| ()));
                    }
                    Courant::Montage(j) => {
                        j.step(doc, budget);
                        if self.snapshot.is_none() {
                            self.snapshot = j.snapshot();
                        }
                        fini = j.outcome.as_ref().map(|o| o.clone().map(|_| ()));
                    }
                }
            }
            match fini {
                None => return None,
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(())) => {
                    let Some((c, e)) = self.courant.take() else { return None };
                    self.recolter(&c, e);
                }
            }
        }
    }

    fn lancer(&mut self, e: Etape) -> Result<Courant, String> {
        match e {
            Etape::Vignettes => {
                let dir = self.dir.join("vignettes");
                std::fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let items: Vec<exportstills::Still> = self.vignettes.iter().map(|(_, s)| exportstills::Still { frame: s.frame, name: s.name.clone() }).collect();
                let mut j = exportstills::job(&dir, &items, self.frame, &StillsSettings::default(), false);
                j.set_exact(self.exact.clone());
                Ok(Courant::Export(match &self.snapshot {
                    Some(s) => j.with_snapshot(s.clone()),
                    None => j,
                }))
            }
            Etape::Videos => {
                let (plan, opts, _) = self.video.take().ok_or("vidéos déjà faites")?;
                let (ff, enc) = self.ffmpeg.clone().ok_or("ffmpeg manquant")?;
                let dir = self.dir.join("videos");
                std::fs::create_dir_all(&dir).map_err(|e| format!("{} : {e}", dir.display()))?;
                let j = EditorialJob::start(plan, opts, ff, enc, &dir)?.with_exact(self.exact.clone());
                Ok(Courant::Montage(match &self.snapshot {
                    Some(s) => j.with_snapshot(s.clone()),
                    None => j,
                }))
            }
        }
    }

    fn recolter(&mut self, c: &Courant, e: Etape) {
        match (c, e) {
            (Courant::Export(_), Etape::Vignettes) => {
                for (id, s) in &self.vignettes {
                    let p = self.dir.join("vignettes").join(format!("{}.png", s.name));
                    if p.is_file() {
                        self.fichiers.vignettes.insert(*id, p);
                    }
                }
            }
            (Courant::Montage(j), Etape::Videos) => {
                let rangs: Vec<(usize, u64)> = self.plan.cibles.iter().map(|c| (c.rang, c.panel)).collect();
                for clip in &j.plan.clips {
                    let p = j.dir.join(&clip.file);
                    if let (true, Some((_, id))) = (p.is_file(), rangs.iter().find(|(r, _)| *r == clip.panel)) {
                        self.fichiers.videos.insert(*id, p);
                    }
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------------- les réglages

/// « #RRGGBB » en couleur.
fn couleur_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    Some([u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?])
}

fn choix(ui: &mut egui::Ui, id: &str, valeur: &mut String, options: &[(String, String)], vide: &str, largeur: f32) -> bool {
    let texte = match options.iter().find(|(i, _)| i == valeur) {
        Some((_, n)) => n.clone(),
        None if valeur.is_empty() => vide.to_string(),
        None => "(déjà choisi)".to_string(),
    };
    let mut change = false;
    egui::ComboBox::from_id_salt(id).selected_text(texte).width(largeur).show_ui(ui, |ui| {
        for (i, n) in options {
            if ui.selectable_label(valeur == i, n).clicked() {
                *valeur = i.clone();
                change = true;
            }
        }
    });
    change
}

impl App {
    /// Les identifiants Kitsu du projet : le dossier où est `kitsu.json`.
    fn kitsu_dossier(&self) -> Option<PathBuf> {
        self.path.as_ref().and_then(|p| p.parent()).filter(|d| d.is_dir()).map(Path::to_path_buf)
    }

    pub(crate) fn open_kitsu(&mut self, section: Section) {
        self.end_stroke();
        self.kitsu.ouvert = true;
        self.kitsu.section = section;
        self.kitsu.jeton_cherche = false;
    }

    /// Dans la fiche du plan : le statut de la tâche storyboard dans Kitsu, s'il a été lu.
    pub(crate) fn kitsu_etiquette(&self, ui: &mut egui::Ui, rang: usize) {
        let Some(p) = self.doc.panels.get(rang) else { return };
        let Some((nom, couleur)) = self.kitsu.marques.get(&p.id) else { return };
        if nom.is_empty() {
            return;
        }
        let c = couleur.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or_else(theme::texte_doux);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Kitsu").small().color(theme::texte_doux()));
            ui.label(RichText::new(format!("● {nom}")).small().color(c)).on_hover_text("Statut de la tâche storyboard de ce plan dans Kitsu, lu la dernière fois que tu as cliqué sur « Lire les retours ».");
        });
    }

    /// Les statuts déjà lus (`kitsu.json`) reviennent à l'ouverture d'un projet ; rien ne part sur le réseau.
    fn kitsu_marques_charger(&mut self) {
        if self.kitsu.marques_epoch == Some(self.doc_epoch) {
            return;
        }
        self.kitsu.marques_epoch = Some(self.doc_epoch);
        self.kitsu.marques.clear();
        let Some(dossier) = self.kitsu_dossier() else { return };
        if let Ok(Some(ids)) = Ids::lire(&dossier) {
            for (k, p) in &ids.plans {
                if let (Ok(id), false) = (k.parse::<u64>(), p.statut.is_empty()) {
                    self.kitsu.marques.insert(id, (p.statut.clone(), couleur_hex(&p.statut_couleur)));
                }
            }
        }
    }

    /// Le projet a changé (autre onglet, autre fichier) : ce qui valait pour l'ancien est abandonné.
    fn kitsu_changement_de_projet(&mut self) {
        if self.kitsu.epoch == self.doc_epoch {
            return;
        }
        self.kitsu.epoch = self.doc_epoch;
        self.kitsu.arreter();
        self.kitsu.apercu = None;
        self.kitsu.message = None;
    }

    pub(crate) fn kitsu_window(&mut self, ctx: &egui::Context) {
        self.kitsu_marques_charger();
        self.kitsu_changement_de_projet();
        if !self.kitsu.ouvert && !self.kitsu.busy() {
            return;
        }
        let mut k = std::mem::take(&mut self.kitsu);
        self.kitsu_poll(&mut k, ctx);
        self.kitsu_rendu(&mut k, ctx);
        if k.travail.is_some() || k.rendu.is_some() {
            ctx.request_repaint_after(Duration::from_millis(if k.rendering() { 16 } else { 100 }));
        }
        if k.ouvert {
            // Les empreintes des plans servent à ne prévisualiser que ce qui a changé : on demande leur calcul tant que la fenêtre est ouverte.
            let mut ouverte = true;
            egui::Window::new("Kitsu")
                .open(&mut ouverte)
                .resizable(true)
                .collapsible(false)
                .default_width(560.0)
                .default_pos(ctx.screen_rect().center() - vec2(280.0, 300.0))
                .pivot(Align2::LEFT_TOP)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().max_height((ctx.screen_rect().height() - 120.0).max(240.0)).show(ui, |ui| self.kitsu_body(ui, &mut k, ctx));
                });
            if !ouverte {
                k.ouvert = false;
                if let Err(e) = self.settings.save() {
                    self.status = format!("Réglages non enregistrés : {e}");
                }
            }
        }
        self.kitsu = k;
    }

    fn kitsu_poll(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let Some(t) = k.travail.take() else { return };
        let res = match t.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => {
                self.status = format!("Kitsu : {}", t.prog.etat().0);
                k.travail = Some(t);
                return;
            }
            Err(TryRecvError::Disconnected) => Err(Erreur::Reseau("le fil de travail s'est arrêté".into())),
        };
        if let Some(d) = &t.menage {
            let _ = std::fs::remove_dir_all(d);
        }
        if t.epoch != self.doc_epoch {
            // Le projet affiché n'est plus celui pour lequel le fil travaillait : on ne pose rien dedans.
            k.message = Some(Ok("Le projet a changé pendant le travail : résultat laissé de côté.".into()));
            return;
        }
        match res {
            Err(Erreur::Annule) => k.message = Some(Ok("Arrêté. Ce qui était déjà fait dans Kitsu y reste (rien ne sera doublé la prochaine fois).".into())),
            Err(e) => {
                if e == Erreur::NonAutorise {
                    k.jeton = None;
                    kitsu::jeton::oublier();
                }
                self.status = format!("Kitsu : {e}");
                k.message = Some(Err(format!("{} : {e}", t.genre.nom())));
            }
            Ok(Sortie::Jeton { jeton, nouveau }) => {
                let serveur = kitsu::serveur_valide(&self.settings.kitsu.serveur).unwrap_or_default();
                if nouveau {
                    k.mot_de_passe.clear();
                    if self.settings.kitsu.rester_connecte {
                        if let Err(e) = kitsu::jeton::garder(&serveur, &jeton) {
                            k.message = Some(Err(format!("Connecté, mais le jeton n'a pas pu être gardé : {e}")));
                        }
                    }
                }
                if !matches!(k.message, Some(Err(_))) {
                    k.message = Some(Ok(format!("Connecté à {serveur}.")));
                }
                k.jeton = Some((serveur, jeton));
            }
            Ok(Sortie::Listes { projets, types, statuts }) => {
                self.settings.kitsu.remplir_defauts(&statuts);
                if self.settings.kitsu.tache_id.is_empty() {
                    if let Some(t) = types.iter().find(|t| t.nom.eq_ignore_ascii_case("storyboard")) {
                        self.settings.kitsu.tache_id = t.id.clone();
                        self.settings.kitsu.tache_nom = t.nom.clone();
                    }
                }
                k.message = Some(Ok(format!("{} projet(s), {} type(s) de tâche, {} statut(s) lus.", projets.len(), types.len(), statuts.len())));
                k.projets = projets;
                k.types = types;
                k.statuts = statuts;
                if let Err(e) = self.settings.save() {
                    self.status = format!("Réglages non enregistrés : {e}");
                }
            }
            Ok(Sortie::Apercu(plan)) => {
                let cle = self.kitsu_cle();
                k.message = Some(Ok(plan.phrase()));
                k.apercu = Some(ApercuPret { plan: *plan, cle });
            }
            Ok(Sortie::Publie(r)) => {
                k.apercu = None;
                self.status = r.phrase();
                let mut m = r.phrase();
                for a in r.avertissements.iter().take(5) {
                    m.push_str(&format!("\n• {a}"));
                }
                k.message = Some(Ok(m));
                self.kitsu_marques_depuis_fichier(k);
            }
            Ok(Sortie::Retours(retours)) => {
                self.kitsu_appliquer_retours(k, &retours);
            }
        }
        ctx.request_repaint();
    }

    fn kitsu_marques_depuis_fichier(&mut self, k: &mut Kitsu) {
        k.marques_epoch = None;
        // Recharge depuis `kitsu.json` (écrit par le fil).
        let Some(dossier) = self.kitsu_dossier() else { return };
        k.marques.clear();
        if let Ok(Some(ids)) = Ids::lire(&dossier) {
            for (key, p) in &ids.plans {
                if let (Ok(id), false) = (key.parse::<u64>(), p.statut.is_empty()) {
                    k.marques.insert(id, (p.statut.clone(), couleur_hex(&p.statut_couleur)));
                }
            }
        }
        k.marques_epoch = Some(self.doc_epoch);
    }

    /// Pose les notes lues : **une** étape d'annulation, puis les commentaires (jamais deux fois le même, jamais d'écrasement du statut décidé ici).
    fn kitsu_appliquer_retours(&mut self, k: &mut Kitsu, retours: &[RetourPlan]) {
        for r in retours {
            if r.statut_nom.is_empty() {
                k.marques.remove(&r.panel);
            } else {
                k.marques.insert(r.panel, (r.statut_nom.clone(), couleur_hex(&r.statut_couleur)));
            }
        }
        self.end_stroke();
        self.playing = false;
        let resume = kitsu::previsualiser(&self.doc, retours);
        if resume.nouveau() == 0 {
            k.message = Some(Ok(format!("{} plan(s) lus : {}", retours.len(), resume.phrase())));
            self.status = "Kitsu : rien de nouveau.".into();
            return;
        }
        self.comment_step("Importer les notes de Kitsu");
        let resume = kitsu::appliquer(&mut self.doc, retours, &self.settings.kitsu, crate::recovery::now_secs());
        self.modified = true;
        if let Some(id) = self.doc.comments.iter().rev().find(|c| c.follow.source == crate::fildecision::Source::Kitsu).map(|c| c.id) {
            self.comment_ui.sel = Some(id);
            self.comment_ui.scroll_to = Some(id);
        }
        self.show_pane(crate::dock::Pane::Comments);
        let m = format!("{} plan(s) lus : {}", retours.len(), resume.phrase());
        self.status = format!("Kitsu : {m}");
        k.message = Some(Ok(m));
    }

    /// Ce pour quoi un aperçu vaut : le projet (version de l'historique), l'étendue, les sortes de prévisualisation.
    fn kitsu_cle(&self) -> (u64, u64, bool, bool, bool) {
        let r = &self.settings.kitsu;
        (self.doc_epoch, self.history.serial(), r.film_entier, r.vignettes, r.videos)
    }

    /// Le rendu des prévisualisations avance ; fini, il lance la publication.
    fn kitsu_rendu(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let Some(r) = &mut k.rendu else { return };
        match r.step(&self.doc, Duration::from_millis(8)) {
            None => {
                self.status = format!("Kitsu : {}", r.status().0);
            }
            Some(Err(e)) => {
                if let Some(mut r) = k.rendu.take() {
                    r.cancel();
                }
                k.message = Some(Err(format!("Prévisualisations : {e}")));
            }
            Some(Ok(())) => {
                let Some(r) = k.rendu.take() else { return };
                let Rendu { dir, plan, fichiers, .. } = r;
                self.kitsu_lancer_publication(k, ctx, plan, fichiers, Some(dir));
            }
        }
    }

    /// Les conditions d'une connexion : pont allumé, adresse valable, jeton. Rend l'adresse et le jeton.
    fn kitsu_pret(&self, k: &Kitsu) -> Result<(String, String), String> {
        let r = &self.settings.kitsu;
        if !r.actif {
            return Err("Allume d'abord le pont Kitsu (case « Activer le pont Kitsu »).".into());
        }
        let serveur = kitsu::serveur_valide(&r.serveur)?;
        match &k.jeton {
            Some((s, j)) if *s == serveur => Ok((serveur, j.clone())),
            _ => Err("Connecte-toi d'abord (« Tester la connexion »).".into()),
        }
    }

    pub(crate) fn kitsu_connecter(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let r = self.settings.kitsu.clone();
        let serveur = match kitsu::serveur_valide(&r.serveur) {
            Ok(s) => s,
            Err(e) => {
                k.message = Some(Err(e));
                return;
            }
        };
        let mdp = k.mot_de_passe.clone();
        let jeton = k.jeton.clone().filter(|(s, _)| *s == serveur).map(|(_, j)| j);
        let epoch = self.doc_epoch;
        k.lancer(ctx, Genre::Connexion, epoch, None, move |annule, prog| {
            if !mdp.is_empty() {
                prog.dire("Connexion");
                let j = Client::connexion(&serveur, &r.email, &mdp, annule)?;
                return Ok(Sortie::Jeton { jeton: j, nouveau: true });
            }
            let Some(j) = jeton else { return Err(Erreur::Reglage("Tape le mot de passe (il n'est jamais écrit sur le disque).".into())) };
            prog.dire("Vérification de la session");
            let c = Client { serveur: serveur.clone(), jeton: Some(j.clone()), annule };
            c.verifier_jeton()?;
            Ok(Sortie::Jeton { jeton: j, nouveau: false })
        });
    }

    fn kitsu_charger_listes(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let (serveur, jeton) = match self.kitsu_pret(k) {
            Ok(x) => x,
            Err(e) => {
                k.message = Some(Err(e));
                return;
            }
        };
        let epoch = self.doc_epoch;
        k.lancer(ctx, Genre::Listes, epoch, None, move |annule, _| {
            let c = Client { serveur, jeton: Some(jeton), annule };
            Ok(Sortie::Listes { projets: c.projets()?, types: c.types_de_tache()?, statuts: c.statuts()? })
        });
    }

    pub(crate) fn kitsu_apercu(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let (serveur, jeton) = match self.kitsu_pret(k) {
            Ok(x) => x,
            Err(e) => {
                k.message = Some(Err(e));
                return;
            }
        };
        let r = self.settings.kitsu.clone();
        if let Some(m) = r.manque_pour_publier() {
            k.message = Some(Err(m));
            return;
        }
        let Some(dossier) = self.kitsu_dossier() else {
            k.message = Some(Err("Enregistre d'abord le projet : les identifiants Kitsu sont gardés dans son dossier (kitsu.json).".into()));
            return;
        };
        if self.doc.panels.is_empty() {
            k.message = Some(Err("Le projet n'a aucun plan.".into()));
            return;
        }
        if !self.envois_empreintes_a_jour() {
            k.message = Some(Err("Les empreintes des plans se calculent encore (elles disent quels plans ont changé) : réessaie dans un instant.".into()));
            return;
        }
        let plans = self.plans_de(if r.film_entier { Etendue::Film } else { Etendue::Sequence }, None);
        let cibles = kitsu::cibles(&self.doc, &plans, self.settings.liste_plans.depart, self.envois_empreintes());
        let episode = kitsu::nom_episode(&self.doc);
        let avec = r.vignettes || r.videos;
        let epoch = self.doc_epoch;
        k.lancer(ctx, Genre::Apercu, epoch, None, move |annule, prog| {
            let c = Client { serveur: serveur.clone(), jeton: Some(jeton), annule };
            prog.dire("Lecture de ce que Kitsu contient");
            let distant = c.distant(&r.projet_id)?;
            let ids = charger_ids(&dossier, &serveur, &r.projet_id)?;
            Ok(Sortie::Apercu(Box::new(kitsu::calculer(cibles, episode, &ids, &distant, avec))))
        });
    }

    pub(crate) fn kitsu_publier(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let Some(a) = k.apercu.take() else { return };
        if a.cle != self.kitsu_cle() {
            k.message = Some(Err("Le projet ou les choix ont changé depuis l'aperçu : refais l'aperçu.".into()));
            return;
        }
        let r = self.settings.kitsu.clone();
        if let Err(e) = self.settings.save() {
            self.status = format!("Réglages non enregistrés : {e}");
        }
        let plan = a.plan;
        if plan.apercus_a_faire().is_empty() || !(r.vignettes || r.videos) {
            self.kitsu_lancer_publication(k, ctx, plan, Fichiers::default(), None);
            return;
        }
        let ffmpeg = match self.ffmpeg_info().cloned() {
            Some((p, Ok(enc))) => Some((p, enc)),
            _ => None,
        };
        self.playing = false;
        match Rendu::demarrer(&self.doc, plan, r.vignettes, r.videos, self.cur_panel, ffmpeg, self.rejeu_idle.clone()) {
            Ok(rendu) => {
                k.rendu = Some(rendu);
                k.message = None;
            }
            Err(e) => k.message = Some(Err(e)),
        }
    }

    fn kitsu_lancer_publication(&mut self, k: &mut Kitsu, ctx: &egui::Context, plan: Plan, fichiers: Fichiers, menage: Option<PathBuf>) {
        let (serveur, jeton) = match self.kitsu_pret(k) {
            Ok(x) => x,
            Err(e) => {
                k.message = Some(Err(e));
                if let Some(d) = menage {
                    let _ = std::fs::remove_dir_all(d);
                }
                return;
            }
        };
        let Some(dossier) = self.kitsu_dossier() else {
            k.message = Some(Err("Enregistre d'abord le projet.".into()));
            return;
        };
        let r = self.settings.kitsu.clone();
        let epoch = self.doc_epoch;
        k.lancer(ctx, Genre::Publication, epoch, menage, move |annule, prog| {
            let c = Client { serveur: serveur.clone(), jeton: Some(jeton), annule };
            let mut ids = charger_ids(&dossier, &serveur, &r.projet_id)?;
            let rapport = kitsu::executer(&c, &r, &plan, &fichiers, &mut ids, Some(&dossier), prog)?;
            Ok(Sortie::Publie(rapport))
        });
    }

    pub(crate) fn kitsu_retours(&mut self, k: &mut Kitsu, ctx: &egui::Context) {
        let (serveur, jeton) = match self.kitsu_pret(k) {
            Ok(x) => x,
            Err(e) => {
                k.message = Some(Err(e));
                return;
            }
        };
        let r = self.settings.kitsu.clone();
        if r.tache_id.is_empty() {
            k.message = Some(Err("Choisis le type de tâche du storyboard.".into()));
            return;
        }
        let Some(dossier) = self.kitsu_dossier() else {
            k.message = Some(Err("Enregistre d'abord le projet.".into()));
            return;
        };
        let panels: Vec<u64> = self.doc.panels.iter().map(|p| p.id).collect();
        let epoch = self.doc_epoch;
        k.lancer(ctx, Genre::Retours, epoch, None, move |annule, prog| {
            let c = Client { serveur: serveur.clone(), jeton: Some(jeton), annule };
            let mut ids = charger_ids(&dossier, &serveur, &r.projet_id)?;
            if ids.plans.is_empty() {
                return Err(Erreur::Reglage("Rien n'a encore été publié dans Kitsu depuis ce projet : publie d'abord.".into()));
            }
            let retours = kitsu::lire_retours(&c, &r, &ids, &panels, prog)?;
            kitsu::noter_statuts(&mut ids, &retours);
            ids.ecrire(&dossier).map_err(Erreur::Reglage)?;
            Ok(Sortie::Retours(retours))
        });
    }

    // ------------------------------------------------------------------------------------------------ l'écran

    fn kitsu_body(&mut self, ui: &mut egui::Ui, k: &mut Kitsu, ctx: &egui::Context) {
        let avert = theme::avertissement();
        if !k.jeton_cherche {
            k.jeton_cherche = true;
            if self.settings.kitsu.rester_connecte && k.jeton.is_none() {
                if let Ok(s) = kitsu::serveur_valide(&self.settings.kitsu.serveur) {
                    if let Some(j) = kitsu::jeton::relire(&s) {
                        k.jeton = Some((s, j));
                    }
                }
            }
        }
        let occupe = k.travail.is_some() || k.rendu.is_some();
        ui.label(
            RichText::new("Publie les plans dans Kitsu (le logiciel libre de suivi de production) et lis les retours de la production : statuts et commentaires, qui deviennent des notes du fil de décision. Éteint d'origine : rien ne part sur Internet tant que tu n'as pas allumé le pont et cliqué.")
                .small()
                .color(theme::texte_doux()),
        );
        ui.add_space(4.0);
        ui.checkbox(&mut self.settings.kitsu.actif, "Activer le pont Kitsu").on_hover_text("Éteint, aucun bouton de cette fenêtre ne parle à Kitsu.");
        let actif = self.settings.kitsu.actif;
        ui.separator();

        let section = k.section;
        // ---- connexion
        egui::CollapsingHeader::new(RichText::new("Connexion").strong()).default_open(section == Section::Connexion).show(ui, |ui| {
            ui.add_enabled_ui(!occupe, |ui| {
                egui::Grid::new("kitsu_connexion").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    ui.label("Serveur");
                    let r = &mut self.settings.kitsu;
                    ui.add(egui::TextEdit::singleline(&mut r.serveur).desired_width(360.0).hint_text("https://kitsu.monstudio.com"));
                    ui.end_row();
                    ui.label("E-mail");
                    ui.add(egui::TextEdit::singleline(&mut r.email).desired_width(360.0));
                    ui.end_row();
                    ui.label("Mot de passe");
                    ui.add(egui::TextEdit::singleline(&mut k.mot_de_passe).password(true).desired_width(360.0))
                        .on_hover_text("Gardé en mémoire le temps de la connexion seulement : jamais écrit sur le disque, vidé dès que la connexion réussit.");
                    ui.end_row();
                });
            });
            if !self.settings.kitsu.serveur.trim().is_empty() {
                if let Err(e) = kitsu::serveur_valide(&self.settings.kitsu.serveur) {
                    ui.colored_label(avert, e);
                }
            }
            let avant = self.settings.kitsu.rester_connecte;
            ui.checkbox(&mut self.settings.kitsu.rester_connecte, "Rester connecté").on_hover_text("Garde le jeton de connexion (pas le mot de passe) sur ce poste, chiffré par Windows pour ton compte : tu n'auras pas à retaper le mot de passe. Décoché, il est effacé.");
            if avant && !self.settings.kitsu.rester_connecte {
                kitsu::jeton::oublier();
            }
            ui.horizontal(|ui| {
                let ok = actif && !occupe && !self.settings.kitsu.serveur.trim().is_empty();
                if bulles::icone_si(ui, ok, ph::PLUGS_CONNECTED, "Tester la connexion", "Se connecte à Kitsu avec l'e-mail et le mot de passe (ou vérifie la session gardée). C'est le seul bouton ici qui envoie le mot de passe, et seulement à l'adresse du serveur ci-dessus.", "").clicked() {
                    self.kitsu_connecter(k, ctx);
                }
                if bulles::icone_si(ui, k.jeton.is_some() && !occupe, ph::PLUGS, "Se déconnecter", "Oublie la session (et le jeton gardé).", "").clicked() {
                    k.jeton = None;
                    kitsu::jeton::oublier();
                    k.message = Some(Ok("Déconnecté.".into()));
                }
                match &k.jeton {
                    Some((s, _)) => ui.label(RichText::new(format!("Connecté à {s}")).small().color(theme::succes())),
                    None => ui.label(RichText::new("Pas connecté").small().color(theme::texte_doux())),
                };
            });
        });

        // ---- production
        egui::CollapsingHeader::new(RichText::new("Production").strong()).default_open(false).show(ui, |ui| {
            ui.horizontal(|ui| {
                if bulles::icone_si(ui, actif && k.jeton.is_some() && !occupe, ph::ARROWS_CLOCKWISE, "Lire les listes de Kitsu", "Lit les projets, les types de tâche et les statuts de ton Kitsu pour les proposer ci-dessous.", "").clicked() {
                    self.kitsu_charger_listes(k, ctx);
                }
                ui.label(RichText::new(format!("{} projet(s), {} type(s) de tâche, {} statut(s) lus", k.projets.len(), k.types.len(), k.statuts.len())).small().color(theme::texte_doux()));
            });
            let r = &mut self.settings.kitsu;
            let projets: Vec<(String, String)> = k.projets.iter().map(|p| (p.id.clone(), p.nom.clone())).collect();
            let types: Vec<(String, String)> = k.types.iter().filter(|t| t.pour.is_empty() || t.pour.eq_ignore_ascii_case("shot")).map(|t| (t.id.clone(), t.nom.clone())).collect();
            let statuts: Vec<(String, String)> = k.statuts.iter().map(|s| (s.id.clone(), s.nom.clone())).collect();
            egui::Grid::new("kitsu_production").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Projet Kitsu");
                let vide_p = if r.projet_nom.is_empty() { "à choisir".to_string() } else { r.projet_nom.clone() };
                if choix(ui, "kitsu_projet", &mut r.projet_id, &projets, &vide_p, 300.0) {
                    r.projet_nom = k.projets.iter().find(|p| p.id == r.projet_id).map(|p| p.nom.clone()).unwrap_or_default();
                }
                ui.end_row();
                ui.label("Tâche du storyboard");
                let vide_t = if r.tache_nom.is_empty() { "à choisir".to_string() } else { r.tache_nom.clone() };
                if choix(ui, "kitsu_tache", &mut r.tache_id, &types, &vide_t, 300.0) {
                    r.tache_nom = k.types.iter().find(|t| t.id == r.tache_id).map(|t| t.nom.clone()).unwrap_or_default();
                }
                ui.end_row();
                ui.label("À la publication");
                choix(ui, "kitsu_st_pub", &mut r.statut_publication, &statuts, "à choisir", 300.0);
                ui.end_row();
            });
            ui.add_space(2.0);
            ui.label(RichText::new("Correspondance des statuts (Kitsu → note de FEANOR)").small().strong());
            ui.label(RichText::new("Un commentaire lu dans Kitsu devient une note dont le statut vient de la tâche à ce moment-là. Une note déjà dans FEANOR n'est jamais modifiée.").small().color(theme::texte_doux()));
            egui::Grid::new("kitsu_statuts").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                for (s, nom) in [(crate::fildecision::Status::Todo, "À faire"), (crate::fildecision::Status::Discussion, "En discussion"), (crate::fildecision::Status::Done, "Fait"), (crate::fildecision::Status::Refused, "Refusé")] {
                    ui.label(nom);
                    let mut v = r.statut_de(s).to_string();
                    if choix(ui, &format!("kitsu_st_{nom}"), &mut v, &statuts, "à choisir", 300.0) {
                        r.regler_statut(s, v);
                    }
                    ui.end_row();
                }
            });
        });

        // ---- publier
        egui::CollapsingHeader::new(RichText::new("Publier").strong()).default_open(section == Section::Publier).show(ui, |ui| {
            let n = self.doc.panels.len();
            let (a, b) = self.doc.sequence_run(self.cur_panel.min(n.saturating_sub(1)));
            let ffmpeg_ok = matches!(self.ffmpeg_info(), Some((_, Ok(_))));
            ui.add_enabled_ui(!occupe, |ui| {
                let r = &mut self.settings.kitsu;
                ui.horizontal(|ui| {
                    ui.radio_value(&mut r.film_entier, true, format!("Tout le film ({n})"));
                    ui.radio_value(&mut r.film_entier, false, format!("La séquence courante ({})", b - a));
                });
                ui.checkbox(&mut r.vignettes, "Une vignette (PNG) par plan changé, en prévisualisation");
                ui.add_enabled_ui(ffmpeg_ok, |ui| {
                    ui.checkbox(&mut r.videos, "Une vidéo MP4 par plan changé, en prévisualisation");
                });
                if !ffmpeg_ok {
                    r.videos = false;
                }
            });
            if self.kitsu_dossier().is_none() {
                ui.colored_label(avert, "Enregistre le projet : les identifiants Kitsu sont gardés dans son dossier (kitsu.json).");
            }
            if !self.doc.numbering_locked {
                ui.label(RichText::new("La numérotation des plans est suivie : les noms changent si un plan s'ajoute ou se déplace. Verrouille-la avant de publier, pour que Kitsu retrouve ses plans.").small().color(avert));
            }
            let empreintes = self.envois_empreintes_a_jour();
            if !empreintes && (self.settings.kitsu.vignettes || self.settings.kitsu.videos) {
                ui.label(RichText::new("Calcul des empreintes des plans (pour n'envoyer que ce qui a changé)…").small().color(theme::texte_doux()));
                ctx.request_repaint_after(Duration::from_millis(300));
            }
            ui.horizontal(|ui| {
                let ok = actif && k.jeton.is_some() && !occupe && self.kitsu_dossier().is_some() && empreintes;
                if bulles::icone_si(ui, ok, ph::EYE, "Aperçu (rien n'est envoyé)", "Compare le projet à ce que Kitsu contient et dit ce qui serait fait : plans à créer, à mettre à jour, prévisualisations. Rien n'est écrit dans Kitsu.", "").clicked() {
                    self.kitsu_apercu(k, ctx);
                }
                let pret = k.apercu.as_ref().is_some_and(|a| a.cle == self.kitsu_cle() && !a.plan.est_vide());
                if bulles::icone_si(ui, pret && actif && !occupe, ph::CLOUD_ARROW_UP, "Publier dans Kitsu", "Crée ce qui manque, met à jour ce qui diffère, puis poste un commentaire avec la prévisualisation sur la tâche des plans changés. Publier de nouveau ne double rien.", "").clicked() {
                    self.kitsu_publier(k, ctx);
                }
                if bulles::icone_si(ui, occupe, ph::X, "Arrêter", "Arrête tout de suite. Ce qui est déjà dans Kitsu y reste, rien ne sera doublé la prochaine fois.", "").clicked() {
                    k.arreter();
                }
            });
            match &k.apercu {
                Some(a) if a.cle == self.kitsu_cle() => {
                    ui.label(RichText::new(a.plan.phrase()).strong());
                    if a.plan.est_vide() {
                        ui.label(RichText::new("Kitsu est déjà à jour.").small().color(theme::succes()));
                    }
                    egui::CollapsingHeader::new("Le détail").default_open(false).show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                            for p in &a.plan.plans {
                                let c = &a.plan.cibles[p.cible];
                                let (t, col) = match p.action {
                                    kitsu::Action::Creer => ("à créer", theme::avertissement()),
                                    kitsu::Action::MettreAJour => ("à mettre à jour", theme::avertissement()),
                                    kitsu::Action::Rien => ("à jour", theme::texte_doux()),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&c.nom).monospace().small());
                                    ui.label(RichText::new(t).small().color(col));
                                    if p.apercu {
                                        ui.label(RichText::new("+ prévisualisation").small().color(theme::texte_doux()));
                                    }
                                });
                            }
                        });
                    });
                }
                Some(_) => {
                    ui.label(RichText::new("Le projet ou les choix ont changé depuis l'aperçu : refais l'aperçu.").small().color(avert));
                }
                None => {}
            }
        });

        // ---- retours
        egui::CollapsingHeader::new(RichText::new("Retours de Kitsu").strong()).default_open(section == Section::Retours).show(ui, |ui| {
            ui.horizontal(|ui| {
                let ok = actif && k.jeton.is_some() && !occupe && self.kitsu_dossier().is_some();
                if bulles::icone_si(ui, ok, ph::DOWNLOAD_SIMPLE, "Lire les retours", "Lit le statut et les commentaires de la tâche storyboard de chaque plan déjà publié. Les commentaires deviennent des notes du fil de décision (une seule étape d'annulation), sans doublon.", "").clicked() {
                    self.kitsu_retours(k, ctx);
                }
                ui.label(RichText::new("Ne change rien dans Kitsu.").small().color(theme::texte_doux()));
            });
            if !k.marques.is_empty() {
                egui::CollapsingHeader::new(format!("Statuts lus ({})", k.marques.len())).default_open(false).show(ui, |ui| {
                    for (i, p) in self.doc.panels.iter().enumerate() {
                        if let Some((nom, c)) = k.marques.get(&p.id) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(self.doc.nom_de_plan(i)).monospace().small());
                                ui.label(RichText::new(format!("● {nom}")).small().color(c.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or_else(theme::texte_doux)));
                            });
                        }
                    }
                });
            }
        });

        ui.separator();
        // ---- l'état
        if let Some(r) = &k.rendu {
            let (t, p) = r.status();
            ui.add(crate::barre::Barre::new(p).text(t));
        } else if let Some(t) = &k.travail {
            let (texte, part) = t.prog.etat();
            if t.genre == Genre::Publication || t.genre == Genre::Retours {
                ui.add(crate::barre::Barre::new(part).text(texte));
            } else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(texte);
                });
            }
            if bulles::icone(ui, ph::X, "Arrêter", "Arrête tout de suite.", "").clicked() {
                k.arreter();
            }
        }
        match &k.message {
            Some(Ok(m)) => {
                ui.label(RichText::new(m).color(theme::succes()));
            }
            Some(Err(e)) => {
                ui.colored_label(avert, e);
            }
            None => {}
        }
    }
}

fn charger_ids(dossier: &Path, serveur: &str, projet: &str) -> Result<Ids, Erreur> {
    Ok(Ids::lire(dossier).map_err(Erreur::Reglage)?.unwrap_or_default().pour(serveur, projet))
}
