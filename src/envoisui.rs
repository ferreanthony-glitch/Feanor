//! Les envois dans l'interface (`envois.rs` pour le modèle et le fichier) : marquer un envoi, voir quels
//! plans ont changé depuis le dernier, sauter de l'un à l'autre.
//!
//! # Zéro latence
//!
//! Savoir si un plan a changé demande l'empreinte de ses dessins (`empreinte.rs`) : des millions de
//! pixels. Rien de cela ne se fait sur le fil de l'interface.
//!
//! - **Sans envoi, aucun travail** : un projet qui n'a jamais marqué d'envoi ne paie rien (pas de relevé,
//!   pas de fil, pas de repeinte).
//! - **Un relevé par image d'écran** (`stamp`) : le numéro de l'historique et la somme des versions des
//!   dessins, un parcours de `doc.cels` (quelques microsecondes pour des milliers de dessins). Tant qu'il
//!   change, on attend.
//! - **Après 0,6 s de calme** (souris relâchée, pas de lecture), la préparation copie les plans (`Panel`)
//!   et, **au plus 1,5 ms par image d'écran**, les dessins dont la version a changé (les tuiles d'un
//!   bitmap sont partagées : une copie ne recopie aucun pixel ; seul un dessin vectoriel est copié en
//!   entier). Puis **un fil** hache les dessins et les plans (`envois::construire`).
//! - À l'arrivée du résultat, l'état de chaque plan se compare au dernier envoi (un nombre contre un
//!   nombre). Un plan qui revient à son état d'envoi (annuler) perd son repère.
//! - Au repos (rien n'a changé depuis le dernier relevé analysé), aucune repeinte n'est demandée.
//!
//! **Marquer un envoi** est une commande explicite, pas un geste : elle copie tous les plans d'un coup, puis les
//! dessins par morceaux de trois millisecondes au plus, image d'écran après image d'écran (un épisode de 200 plans
//! en demandait 90 à 110 d'un seul coup) ; le fil fait le reste, et le projet est enregistré dès que l'instantané
//! est prêt. Si le projet change pendant la copie, elle recommence : l'instantané montre le film d'un seul instant.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align2, FontId, Rect, RichText, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::document::CelId;
use crate::empreinte::Donnees;
use crate::envois::{self, Copie, Demande, Etat, NouvelEnvoi, PlanCopie, Resultat};
use crate::theme;

/// Secondes de calme (rien ne change, souris relâchée) avant qu'un relevé commence.
const CALME: f64 = 0.6;
/// Temps accordé par image d'écran à la copie des dessins d'un relevé.
const BUDGET: Duration = Duration::from_micros(1_500);
/// Temps accordé par image d'écran à la copie d'un marquage d'envoi (une commande explicite : l'écran ne doit pas saccader, pas non plus
/// attendre plus d'une ou deux secondes pour 4 000 dessins).
const BUDGET_MARQUAGE: Duration = Duration::from_millis(3);
// Essais : la copie d'un marquage ne prend qu'un dessin par image d'écran, sur ce fil-ci (de quoi changer le projet pendant qu'elle dure).
#[cfg(test)]
thread_local! {
    pub(crate) static MARQUAGE_UN_DESSIN_PAR_IMAGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn budget_marquage() -> Duration {
    #[cfg(test)]
    if MARQUAGE_UN_DESSIN_PAR_IMAGE.with(|c| c.get()) {
        return Duration::ZERO;
    }
    BUDGET_MARQUAGE
}
/// Combien de fois la copie d'un marquage recommence (le projet change pendant qu'elle dure) avant qu'on y renonce.
const MARQUAGE_REPRISES_MAX: u32 = 5;

/// Rayon du repère « modifié » dans la bande et sur la planche.
pub(crate) const REPERE: f32 = 7.0;

/// Ce qui permet de savoir que le projet a bougé depuis le dernier relevé.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Releve {
    serial: u64,
    versions: u64,
    dessins: usize,
    plans: usize,
}

/// La copie en cours des plans et des dessins d'un relevé.
struct Preparation {
    releve: Releve,
    plans: Vec<PlanCopie>,
    cels: HashMap<CelId, Copie>,
    restants: Vec<CelId>,
}

/// La copie en cours d'un marquage d'envoi (`App::envois_marquer_copier`).
struct PrepMarquage {
    releve: Releve,
    /// Les plans déjà copiés (les plans.len() premiers) ; la numérotation, les noms et les séquences de tous, pris d'un coup au début.
    plans: Vec<PlanCopie>,
    numeros: crate::numerotation::Numeros,
    noms: Vec<String>,
    sequences: Vec<u64>,
    cels: HashMap<CelId, Copie>,
    restants: Vec<CelId>,
    nom: String,
    date: u64,
    /// La variante choisie de chaque séquence au moment du marquage (`variantes.rs`).
    variantes: Vec<envois::VarianteEnvoyee>,
    /// Combien de fois le projet a changé pendant la copie (elle a recommencé).
    #[allow(dead_code)]
    reprises: u32,
    /// Le plus long morceau de copie sur le fil de l'interface.
    pire: Duration,
}

/// Le fil qui hache.
struct Travail {
    rx: Receiver<Result<Resultat, String>>,
    releve: Releve,
    generation: u64,
}

/// Un envoi en cours de marquage : le fil bâtit l'instantané.
struct Marquage {
    rx: Receiver<Result<Resultat, String>>,
    releve: Releve,
    nom: String,
    /// La variante choisie de chaque séquence au moment du marquage (`variantes.rs`).
    variantes: Vec<envois::VarianteEnvoyee>,
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
    releve: Option<Releve>,
    calme_depuis: f64,
    /// Le relevé que le dernier résultat a analysé.
    fait_pour: Option<Releve>,
    /// Numéro du dessin → (version, empreinte) : ce qu'on sait déjà.
    connus: HashMap<CelId, (u64, u64)>,
    /// Plan → empreinte de son contenu, au dernier relevé analysé.
    digests: HashMap<u64, u64>,
    /// Les plans qui ne sont plus comme au dernier envoi.
    etats: HashMap<u64, Etat>,
    /// Le dernier envoi (numéro, libellé) auquel `etats` se compare, et ce qui a servi à le calculer.
    reference: Option<(u64, String)>,
    signature: (usize, u64),
    prep: Option<Preparation>,
    travail: Option<Travail>,
    marquage: Option<Marquage>,
    /// La copie d'un marquage, en cours (elle précède `marquage`).
    marquage_prep: Option<PrepMarquage>,
    generation: u64,
    pub(crate) nouveau: Option<NouveauEnvoi>,
    pub(crate) liste: bool,
    /// Envoi dont on confirme le retrait de la liste.
    retrait: Option<u64>,
    /// Durée de la dernière copie d'un marquage sur le fil de l'interface (profil).
    pub(crate) derniere_copie: Option<Duration>,
}

impl EnvoisUi {
    /// Le plan `plan` n'est plus comme au dernier envoi.
    pub(crate) fn etat(&self, plan: u64) -> Option<Etat> {
        self.etats.get(&plan).copied()
    }

    /// L'empreinte du plan `plan` au dernier relevé analysé.
    pub(crate) fn empreinte(&self, plan: u64) -> Option<u64> {
        self.digests.get(&plan).copied()
    }

    /// Aucun relevé des plans n'est encore analysé.
    pub(crate) fn empreintes_vides(&self) -> bool {
        self.digests.is_empty()
    }

    /// Le dernier relevé que le suivi a analysé (le journal des différences veut des empreintes à jour).
    pub(crate) fn fait_pour(&self) -> Option<Releve> {
        self.fait_pour
    }

    /// Aucun plan n'est repéré modifié ou ajouté depuis le dernier envoi.
    pub(crate) fn etats_vides(&self) -> bool {
        self.etats.is_empty()
    }

    /// Rien à faire : le dernier relevé est analysé et le projet n'a pas bougé depuis.
    #[cfg(test)]
    pub(crate) fn au_repos(&self) -> bool {
        self.travail.is_none() && self.marquage.is_none() && self.marquage_prep.is_none() && self.prep.is_none() && self.releve == self.fait_pour
    }

    /// Un fil travaille (essais : attendre).
    #[cfg(test)]
    pub(crate) fn occupe(&self) -> bool {
        self.travail.is_some() || self.marquage.is_some() || self.marquage_prep.is_some() || self.prep.is_some()
    }

    /// Essais : (dessins déjà copiés, dessins à copier) de la copie d'un marquage en cours.
    #[cfg(test)]
    pub(crate) fn marquage_avancement(&self) -> Option<(usize, usize)> {
        self.marquage_prep.as_ref().map(|p| (p.cels.len(), p.restants.len()))
    }

    /// Un relevé des plans a été analysé (essais).
    #[cfg(test)]
    pub(crate) fn suivi_fait(&self) -> bool {
        self.fait_pour.is_some()
    }
}

/// Un numéro pour nommer le fichier d'un instantané (les envois de deux dossiers qui se mêlent ne se
/// confondent pas).
fn unique() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
    (t ^ (t >> 32) ^ u64::from(std::process::id()).rotate_left(16)) as u32
}

impl App {
    // ------------------------------------------------------------------ le suivi

    /// Les empreintes des plans sont celles du projet tel qu'il est (le pont Kitsu s'en sert).
    pub(crate) fn envois_empreintes_a_jour(&self) -> bool {
        self.envois_ui.epoch == self.doc_epoch && self.envois_ui.travail.is_none() && self.envois_ui.prep.is_none() && self.envois_ui.fait_pour == Some(self.envois_releve())
    }

    /// Plan (identifiant) → empreinte de son contenu, au dernier relevé analysé.
    pub(crate) fn envois_empreintes(&self) -> &HashMap<u64, u64> {
        &self.envois_ui.digests
    }

    pub(crate) fn envois_releve(&self) -> Releve {
        Releve {
            serial: self.history.serial(),
            versions: self.doc.cels.values().fold(0u64, |a, c| a.wrapping_add(c.version)),
            dessins: self.doc.cels.len(),
            plans: self.doc.panels.len(),
        }
    }

    /// Les plans à examiner, copiés : leur titre et leur version d'aujourd'hui.
    fn envois_plans(&self) -> Vec<PlanCopie> {
        // (Les noms et les titres de tous les plans en une passe : pas le carré du nombre de plans.)
        let numeros = crate::numerotation::Numeros::of(&self.doc);
        let noms = self.doc.noms_de_tous_les_plans();
        let sequences = self.doc.sequence_ids();
        self.doc
            .panels
            .iter()
            .enumerate()
            .map(|(i, p)| PlanCopie {
                panel: p.clone(),
                titre: crate::storyboard::panel_title_n(&self.doc, &numeros, i),
                version: self.doc.numero_de_version(p.id),
                nom: noms[i].clone(),
                sequence: numeros.sequence_label(&self.doc, sequences[i]),
            })
            .collect()
    }

    /// Une image d'écran du suivi (voir le haut du fichier). Sans envoi, il ne fait rien.
    pub(crate) fn envois_tick(&mut self, ctx: &egui::Context) {
        let _t = crate::perf::section("envois (suivi)");
        if self.envois_ui.epoch != self.doc_epoch {
            self.envois_ui = EnvoisUi { epoch: self.doc_epoch, ..Default::default() };
        }
        self.envois_poll(ctx);
        // Un marquage dont les dessins se copient : un morceau par image d'écran, rien d'autre ne se fait pendant.
        if self.envois_ui.marquage_prep.is_some() {
            self.envois_marquer_copier(ctx, budget_marquage());
            ctx.request_repaint();
            return;
        }
        // La liste a changé (envoi ajouté ou retiré) : les états se recomparent.
        let signature = (self.doc.envois.list.len(), self.doc.envois.dernier().map_or(0, |e| e.id));
        if self.envois_ui.signature != signature {
            self.envois_ui.signature = signature;
            self.envois_etats();
        }
        // Le pont Kitsu (fenêtre ouverte) a aussi besoin des empreintes des plans : il n'envoie que ce qui a changé.
        let veut = !self.doc.envois.is_empty() || self.kitsu.ouvert;
        if !veut && self.envois_ui.marquage.is_none() {
            return;
        }
        let now = ctx.input(|i| i.time);
        let releve = self.envois_releve();
        if self.envois_ui.releve != Some(releve) {
            self.envois_ui.releve = Some(releve);
            self.envois_ui.calme_depuis = now;
            if self.envois_ui.prep.as_ref().is_some_and(|p| p.releve != releve) {
                self.envois_ui.prep = None;
            }
        }
        let a_faire = self.envois_ui.fait_pour != Some(releve);
        if a_faire && self.envois_ui.travail.is_none() && self.envois_ui.marquage.is_none() && veut {
            if self.envois_ui.prep.is_some() {
                self.envois_copier(ctx, BUDGET);
            } else if now - self.envois_ui.calme_depuis >= CALME && !self.playing && !ctx.input(|i| i.pointer.any_down()) {
                self.envois_commencer(releve);
                self.envois_copier(ctx, BUDGET);
            }
        }
        if a_faire || self.envois_ui.travail.is_some() || self.envois_ui.marquage.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Commence un relevé : les plans sont copiés d'un coup (leurs dessins, eux, par petits morceaux).
    fn envois_commencer(&mut self, releve: Releve) {
        let mut restants = Vec::new();
        let mut vus = std::collections::HashSet::new();
        for p in &self.doc.panels {
            for id in crate::empreinte::dessins_du_plan(p) {
                if !vus.insert(id) {
                    continue;
                }
                let Some(c) = self.doc.cels.get(&id) else { continue };
                if self.envois_ui.connus.get(&id).is_none_or(|(v, _)| *v != c.version) {
                    restants.push(id);
                }
            }
        }
        self.envois_ui.prep = Some(Preparation { releve, plans: self.envois_plans(), cels: HashMap::new(), restants });
    }

    /// Copie des dessins pendant au plus `budget` ; la dernière copie lance le fil.
    fn envois_copier(&mut self, ctx: &egui::Context, budget: Duration) {
        let start = Instant::now();
        let Some(prep) = &mut self.envois_ui.prep else { return };
        while let Some(id) = prep.restants.pop() {
            if let Some(c) = self.doc.cels.get(&id) {
                prep.cels.insert(id, Copie { version: c.version, donnees: Donnees::de(c) });
            }
            if start.elapsed() >= budget {
                break;
            }
        }
        if !prep.restants.is_empty() {
            return;
        }
        let Some(prep) = self.envois_ui.prep.take() else { return };
        let demande = Demande { plans: prep.plans, cels: prep.cels, connus: self.envois_ui.connus.clone(), tete: crate::project::Tete::de(&self.doc), envoi: None };
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = ctx.clone();
        let started = std::thread::Builder::new().name("envois".into()).spawn(move || {
            // (Le hachage des plans passe après l'interface.)
            crate::parallele::fil_et_ses_fils_sous_la_normale();
            let r = envois::construire(demande);
            let _ = tx.send(r);
            wake.request_repaint();
        });
        if started.is_ok() {
            self.envois_ui.travail = Some(Travail { rx, releve: prep.releve, generation: self.envois_ui.generation });
        }
    }

    /// Les résultats arrivés des fils : un relevé analysé, ou un envoi prêt.
    fn envois_poll(&mut self, ctx: &egui::Context) {
        if let Some(t) = self.envois_ui.travail.take() {
            match t.rx.try_recv() {
                Ok(Ok(r)) => {
                    if t.generation == self.envois_ui.generation {
                        self.envois_ui.connus.extend(r.empreintes);
                        self.envois_ui.digests = r.plans.into_iter().collect();
                        self.envois_ui.fait_pour = Some(t.releve);
                        self.envois_etats();
                    }
                }
                Ok(Err(e)) => self.status = format!("Envois : le suivi des plans a échoué ({e})."),
                Err(TryRecvError::Empty) => self.envois_ui.travail = Some(t),
                Err(TryRecvError::Disconnected) => {}
            }
        }
        if let Some(m) = self.envois_ui.marquage.take() {
            match m.rx.try_recv() {
                Ok(Ok(r)) => self.envois_fini(ctx, r, m.releve, m.variantes),
                Ok(Err(e)) => self.status = format!("Envoi « {} » : échec ({e}).", m.nom),
                Err(TryRecvError::Empty) => self.envois_ui.marquage = Some(m),
                Err(TryRecvError::Disconnected) => self.status = format!("Envoi « {} » : le calcul s'est arrêté.", m.nom),
            }
        }
    }

    /// Compare chaque plan au dernier envoi.
    fn envois_etats(&mut self) {
        let ui = &mut self.envois_ui;
        ui.etats.clear();
        ui.reference = None;
        let Some(dernier) = self.doc.envois.dernier() else { return };
        ui.reference = Some((dernier.id, dernier.libelle()));
        for p in &self.doc.panels {
            if let Some(&e) = ui.digests.get(&p.id) {
                match envois::etat_du_plan_dans(&self.doc, dernier, p.id, e) {
                    Etat::Inchange => {}
                    s => {
                        ui.etats.insert(p.id, s);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ marquer un envoi

    /// Marque un envoi : l'état de chaque plan est copié par petits morceaux, image d'écran après image d'écran (`envois_marquer_copier` :
    /// copier d'un coup les 4 400 dessins d'un épisode de 200 plans coûtait 90 à 110 ms au fil de l'interface, surtout les traits des
    /// dessins vectoriels), puis l'instantané se bâtit sur un fil.
    pub(crate) fn envois_marquer(&mut self, ctx: &egui::Context, nom: &str, date: u64) {
        // Film partagé : un envoi est une décision sur tout le film, il demande le jeton du montage.
        if self.partage_refuse_montage() {
            return;
        }
        if self.envois_ui.marquage.is_some() || self.envois_ui.marquage_prep.is_some() {
            self.status = "Un envoi est déjà en préparation.".into();
            return;
        }
        self.end_stroke();
        self.envois_marquer_commencer(nom, date, 0);
        self.status = format!("Envoi « {} » : état des plans en cours de relevé…", nom.trim());
        self.envois_marquer_copier(ctx, budget_marquage());
    }

    /// Commence (ou recommence, `reprises` fois déjà) la copie d'un marquage : la liste des dessins à copier et ce qu'il faut de tout le film
    /// pour nommer les plans ; les plans et les dessins se copient ensuite par morceaux.
    fn envois_marquer_commencer(&mut self, nom: &str, date: u64, reprises: u32) {
        let start = Instant::now();
        let numeros = crate::numerotation::Numeros::of(&self.doc);
        let noms = self.doc.noms_de_tous_les_plans();
        let sequences = self.doc.sequence_ids();
        let mut restants = Vec::new();
        let mut vus = std::collections::HashSet::new();
        for p in &self.doc.panels {
            for id in crate::empreinte::dessins_du_plan(p) {
                if vus.insert(id) && self.doc.cels.contains_key(&id) {
                    restants.push(id);
                }
            }
        }
        // Un relevé en cours ne vaut plus rien : le marquage donne un résultat plus récent.
        self.envois_ui.generation += 1;
        self.envois_ui.prep = None;
        self.envois_ui.travail = None;
        self.envois_ui.marquage_prep = Some(PrepMarquage {
            releve: self.envois_releve(),
            plans: Vec::with_capacity(self.doc.panels.len()),
            numeros,
            noms,
            sequences,
            cels: HashMap::new(),
            restants,
            nom: nom.trim().to_string(),
            date,
            variantes: self.doc.variantes_pour_envoi(),
            reprises,
            pire: start.elapsed(),
        });
    }

    /// Copie des dessins d'un marquage pendant au plus `budget` ; la dernière copie lance le fil. Si le projet change pendant la copie,
    /// elle recommence (l'instantané montre le film d'un seul instant).
    fn envois_marquer_copier(&mut self, ctx: &egui::Context, budget: Duration) {
        let start = Instant::now();
        if self.envois_ui.marquage_prep.is_none() {
            return;
        }
        // Rien ne se copie (ni ne se valide) pendant un geste : un trait en cours ne bouge ni la version du dessin ni l'historique avant son
        // dernier point, la copie en garderait la moitié. Même attente pour un rendu exact en route (il change la version du dessin à son
        // arrivée, et la copie recommencerait à chaque dessin).
        if ctx.input(|i| i.pointer.any_down())
            || self.cel_here().is_some_and(|c| self.cel_busy(c))
            || self.rejeu.has_pending()
            || !self.rejeu_deferred.is_empty()
            || !self.rejeu_again.is_empty()
        {
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        let releve = self.envois_releve();
        let Some(prep) = &mut self.envois_ui.marquage_prep else { return };
        if prep.releve != releve {
            let (nom, date, reprises) = (prep.nom.clone(), prep.date, prep.reprises + 1);
            if reprises > MARQUAGE_REPRISES_MAX {
                // Le projet n'arrête pas de changer : pas d'envoi fait d'un film à moitié copié.
                self.envois_ui.marquage_prep = None;
                self.status = format!("Envoi « {nom} » abandonné : le projet changeait pendant le relevé. Recommence quand plus rien ne bouge.");
                return;
            }
            self.envois_marquer_commencer(&nom, date, reprises);
            ctx.request_repaint();
            return;
        }
        // Les plans d'abord (au moins un par morceau), puis les dessins.
        while prep.plans.len() < self.doc.panels.len() {
            let i = prep.plans.len();
            let p = &self.doc.panels[i];
            prep.plans.push(PlanCopie {
                panel: p.clone(),
                titre: crate::storyboard::panel_title_n(&self.doc, &prep.numeros, i),
                version: self.doc.numero_de_version(p.id),
                nom: prep.noms.get(i).cloned().unwrap_or_default(),
                sequence: prep.sequences.get(i).map(|&s| prep.numeros.sequence_label(&self.doc, s)).unwrap_or_default(),
            });
            if start.elapsed() >= budget {
                break;
            }
        }
        while prep.plans.len() >= self.doc.panels.len() {
            let Some(id) = prep.restants.pop() else { break };
            if let Some(c) = self.doc.cels.get(&id) {
                prep.cels.insert(id, Copie { version: c.version, donnees: Donnees::de(c) });
            }
            if start.elapsed() >= budget {
                break;
            }
        }
        prep.pire = prep.pire.max(start.elapsed());
        if prep.plans.len() < self.doc.panels.len() || !prep.restants.is_empty() {
            ctx.request_repaint();
            return;
        }
        let Some(prep) = self.envois_ui.marquage_prep.take() else { return };
        let (nom, date, variantes, plans, cels, releve) = (prep.nom, prep.date, prep.variantes, prep.plans, prep.cels, prep.releve);
        let note_le = crate::recovery::now_secs();
        let demande = Demande {
            plans,
            cels,
            connus: self.envois_ui.connus.clone(),
            tete: crate::project::Tete::de(&self.doc),
            envoi: Some(NouvelEnvoi { id: self.doc.envois.prochain_numero(), nom: nom.trim().to_string(), date, note_le, unique: unique() }),
        };
        // (Le plus long morceau de copie sur le fil de l'interface : c'est lui que l'utilisateur sent.)
        self.envois_ui.derniere_copie = Some(prep.pire);
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = ctx.clone();
        let started = std::thread::Builder::new().name("envoi".into()).spawn(move || {
            // (Le hachage des plans passe après l'interface.)
            crate::parallele::fil_et_ses_fils_sous_la_normale();
            let r = envois::construire(demande);
            let _ = tx.send(r);
            wake.request_repaint();
        });
        match started {
            Ok(_) => {
                self.envois_ui.marquage = Some(Marquage { rx, releve, nom: nom.trim().to_string(), variantes });
                self.status = format!("Envoi « {} » : état des plans en cours de relevé…", nom.trim());
            }
            Err(e) => self.status = format!("Envoi impossible : {e}"),
        }
    }

    /// L'instantané est prêt : l'envoi entre dans le projet, et le projet est enregistré.
    fn envois_fini(&mut self, ctx: &egui::Context, r: Resultat, releve: Releve, variantes: Vec<envois::VarianteEnvoyee>) {
        let Some((mut envoi, instantane)) = r.envoi else { return };
        envoi.variantes = variantes;
        let (n, titre) = (envoi.plans.len(), envoi.libelle());
        self.envois_ui.connus.extend(r.empreintes);
        self.envois_ui.digests = r.plans.into_iter().collect();
        self.envois_ui.fait_pour = Some(releve);
        self.doc.envois.ajouter(envoi, instantane);
        self.envois_ui.signature = (self.doc.envois.list.len(), self.doc.envois.dernier().map_or(0, |e| e.id));
        self.envois_etats();
        self.modified = true;
        // Un projet encore à l'ancien nom (`.pelure`) : l'enregistrer le renommerait en `.shot` ; ce n'est pas à un simple marquage de
        // le faire en silence. L'envoi reste dans le projet (modifié : Quitter le demandera), et le message dit de quoi il s'agit.
        if self.path.as_deref().is_some_and(|p| self.partage.is_none() && crate::project::est_ancien_manifeste(p)) {
            self.status = format!("Envoi « {titre} » marqué : {n} plan{} notés. Enregistre le projet (Ctrl+S) pour le garder : il passera alors au nouveau nom .shot.", if n > 1 { "s" } else { "" });
        } else if self.path.is_some() {
            self.save();
            self.status = format!("Envoi « {titre} » marqué : {n} plan{} notés. Le projet s'enregistre.", if n > 1 { "s" } else { "" });
        } else {
            self.status = format!("Envoi « {titre} » marqué : {n} plan{} notés. Enregistre le projet pour le garder.", if n > 1 { "s" } else { "" });
        }
        ctx.request_repaint();
    }

    /// Ouvre la fenêtre « Marquer un envoi », avec un nom et la date du jour.
    pub(crate) fn envois_ouvrir_marquage(&mut self) {
        if self.partage_refuse_montage() {
            return;
        }
        let n = self.doc.envois.prochain_numero();
        self.envois_ui.nouveau = Some(NouveauEnvoi { nom: format!("v{n} au réal"), date: envois::date_courte(crate::recovery::now_secs()), focus: true });
    }

    // ------------------------------------------------------------------ repères et navigation

    /// Le repère d'un plan : son état depuis le dernier envoi et la phrase qui le dit.
    pub(crate) fn envoi_repere(&self, plan: u64) -> Option<(Etat, String)> {
        let etat = self.envois_ui.etat(plan)?;
        let (_, quand) = self.envois_ui.reference.as_ref()?;
        let phrase = match etat {
            Etat::Modifie => format!("Modifié depuis l'envoi « {quand} »."),
            Etat::Nouveau => format!("Ajouté depuis l'envoi « {quand} » : il n'y était pas."),
            Etat::Inchange => return None,
        };
        Some((etat, phrase))
    }

    /// Les plans (rangs) qui ne sont plus comme au dernier envoi, dans l'ordre du film.
    pub(crate) fn plans_modifies(&self) -> Vec<usize> {
        self.doc.panels.iter().enumerate().filter(|(_, p)| self.envois_ui.etats.contains_key(&p.id)).map(|(i, _)| i).collect()
    }

    /// Va au plan modifié suivant (`sens` 1) ou précédent (-1), en tournant.
    fn plan_modifie(&mut self, sens: i32) {
        if self.doc.envois.is_empty() {
            self.status = "Aucun envoi : « Marquer un envoi… » d'abord.".into();
            return;
        }
        let liste = self.plans_modifies();
        if liste.is_empty() {
            let quand = self.envois_ui.reference.as_ref().map(|r| r.1.clone()).unwrap_or_default();
            self.status = if self.envois_ui.fait_pour.is_none() {
                "Les plans sont en cours de relevé : réessaie dans un instant.".into()
            } else {
                format!("Aucun plan modifié depuis l'envoi « {quand} ».")
            };
            return;
        }
        let cur = self.cur_panel;
        let cible = if sens >= 0 { liste.iter().copied().find(|&i| i > cur).unwrap_or(liste[0]) } else { liste.iter().copied().rev().find(|&i| i < cur).unwrap_or(liste[liste.len() - 1]) };
        self.select_panel(cible);
        self.board_sel = vec![self.doc.panels[cible].id];
        let rang = liste.iter().position(|&i| i == cible).map_or(0, |k| k + 1);
        self.status = format!("Plan modifié {rang} sur {} depuis l'envoi « {} ».", liste.len(), self.envois_ui.reference.as_ref().map(|r| r.1.as_str()).unwrap_or(""));
    }

    /// Choisit tous les plans modifiés (pour les copier, les colorer, les exporter ensemble…).
    fn choisir_plans_modifies(&mut self) {
        let liste = self.plans_modifies();
        if liste.is_empty() {
            self.status = if self.doc.envois.is_empty() { "Aucun envoi : « Marquer un envoi… » d'abord.".into() } else { "Aucun plan modifié depuis le dernier envoi.".into() };
            return;
        }
        self.board_sel = liste.iter().map(|&i| self.doc.panels[i].id).collect();
        self.select_panel(liste[0]);
        self.status = format!("{} plan{} modifié{} depuis le dernier envoi, choisi{}.", liste.len(), if liste.len() > 1 { "s" } else { "" }, if liste.len() > 1 { "s" } else { "" }, if liste.len() > 1 { "s" } else { "" });
    }

    /// Les commandes `envoi.*` ; vrai si `action` en était une.
    pub(crate) fn envois_action(&mut self, action: &str) -> bool {
        match action {
            "envoi.marquer" => self.envois_ouvrir_marquage(),
            "envoi.liste" => self.envois_ui.liste = !self.envois_ui.liste,
            "envoi.modifie_suivant" => self.plan_modifie(1),
            "envoi.modifie_precedent" => self.plan_modifie(-1),
            "envoi.modifies_choisir" => self.choisir_plans_modifies(),
            "envoi.journal" => self.journal_ouvrir(None),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------------ les fenêtres

    /// Les fenêtres des envois : le marquage et la liste.
    pub(crate) fn envois_windows(&mut self, ctx: &egui::Context) {
        self.envois_fenetre_marquage(ctx);
        self.envois_fenetre_liste(ctx);
        self.journal_fenetre(ctx);
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
                "FEANOR note l'état des {} plans du film tel qu'il est maintenant. Les plans qui changeront ensuite seront repérés dans le montage et sur la planche.",
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
        let mut aller: Option<usize> = None;
        let mut retirer: Option<u64> = None;
        let mut comparer: Option<u64> = None;
        let mut relire: Option<u64> = None;
        let mut journal: Option<u64> = None;
        egui::Window::new("Envois").open(&mut ouverte).default_width(420.0).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if bulles::icone(ui, ph::PAPER_PLANE_TILT, "Marquer un envoi…", "Noter l'état de tous les plans : ce qui change ensuite sera repéré.", &self.raccourci("envoi.marquer")).clicked() {
                    self.envois_ouvrir_marquage();
                }
                ui.separator();
                if bulles::icone(ui, ph::ARROW_FAT_LEFT, "Plan modifié précédent", "Aller au plan modifié avant le plan courant.", &self.raccourci("envoi.modifie_precedent")).clicked() {
                    self.plan_modifie(-1);
                }
                if bulles::icone(ui, ph::ARROW_FAT_RIGHT, "Plan modifié suivant", "Aller au plan modifié après le plan courant.", &self.raccourci("envoi.modifie_suivant")).clicked() {
                    self.plan_modifie(1);
                }
                if bulles::icone(ui, ph::LIST_CHECKS, "Choisir les plans modifiés", "Choisir d'un coup tous les plans modifiés depuis le dernier envoi (pour les copier, les colorer, les exporter).", &self.raccourci("envoi.modifies_choisir")).clicked() {
                    self.choisir_plans_modifies();
                }
            });
            ui.separator();
            if self.doc.envois.is_empty() {
                ui.label("Aucun envoi pour l'instant. « Marquer un envoi » note l'état de tous les plans (par exemple « v2 au réal »), et repère ensuite ceux qui changent.");
                return;
            }
            match self.envois_ui.reference.clone() {
                Some((_, quand)) => {
                    let (m, n) = self.envois_ui.etats.values().fold((0, 0), |(m, n), e| if *e == Etat::Nouveau { (m, n + 1) } else { (m + 1, n) });
                    if self.envois_ui.fait_pour.is_none() {
                        ui.label(RichText::new("Relevé des plans en cours…").color(theme::texte_doux()));
                    } else if m + n == 0 {
                        ui.label(format!("Aucun plan modifié depuis « {quand} »."));
                    } else {
                        let nouveaux = if n > 0 { format!(", {n} ajouté{}", if n > 1 { "s" } else { "" }) } else { String::new() };
                        ui.label(format!("Depuis « {quand} » : {m} plan{} modifié{}{nouveaux}.", if m > 1 { "s" } else { "" }, if m > 1 { "s" } else { "" }));
                    }
                }
                None => {}
            }
            let modifies = self.plans_modifies();
            if !modifies.is_empty() {
                egui::ScrollArea::vertical().id_salt("envois_modifies").max_height(150.0).show(ui, |ui| {
                    for &i in &modifies {
                        let etat = self.envois_ui.etat(self.doc.panels[i].id);
                        let sorte = if etat == Some(Etat::Nouveau) { "ajouté" } else { "modifié" };
                        let texte = format!("{}  ({sorte})", crate::storyboard::panel_title(&self.doc, i));
                        if ui.selectable_label(i == self.cur_panel, texte).clicked() {
                            aller = Some(i);
                        }
                    }
                });
            }
            ui.separator();
            ui.label(RichText::new("Les envois").strong());
            let mut lignes: Vec<&envois::Envoi> = self.doc.envois.list.iter().collect();
            lignes.sort_by_key(|e| std::cmp::Reverse(e.id));
            egui::ScrollArea::vertical().id_salt("envois_liste").max_height(220.0).show(ui, |ui| {
                for e in lignes {
                    ui.horizontal(|ui| {
                        // Combien de plans diffèrent de cet envoi (d'après les empreintes du dernier relevé).
                        let diff = self.doc.panels.iter().filter(|p| self.envois_ui.digests.get(&p.id).is_some_and(|&d| envois::etat_du_plan_dans(&self.doc, e, p.id, d) != Etat::Inchange)).count();
                        ui.label(RichText::new(e.libelle()).strong());
                        let detail = if self.envois_ui.fait_pour.is_none() { format!("{} plans", e.plans.len()) } else { format!("{} plans, {diff} différent{} d'aujourd'hui", e.plans.len(), if diff > 1 { "s" } else { "" }) };
                        ui.label(RichText::new(detail).color(theme::texte_doux()));
                        // Les variantes de séquences choisies à ce moment-là (`variantes.rs`).
                        if !e.variantes.is_empty() {
                            let v: Vec<String> = e.variantes.iter().map(|v| format!("séquence {} : {}", self.doc.sequence_label(v.sequence), crate::variantes::etiquette(v.numero, &v.nom))).collect();
                            ui.label(RichText::new(format!("· {}", v.join(" ; "))).color(theme::texte_doux()))
                                .on_hover_text("La variante de chaque séquence qui en a plusieurs quand cet envoi est parti.");
                        }
                        // Le plan courant, comparé à son état à cet envoi (s'il y était).
                        let present = self.doc.panels.get(self.cur_panel).is_some_and(|p| self.doc.plan_dans_envoi(e, p.id).is_some()) && !e.fichier.is_empty();
                        let r = bulles::icone_si(ui, present, ph::COLUMNS, "Comparer le plan courant avec cet envoi", "Le plan d'aujourd'hui et son état à cet envoi, côte à côte ou superposés (lecture seule).", "");
                        if r.clicked() {
                            comparer = Some(e.id);
                        }
                        // Le journal des différences depuis cet envoi jusqu'à aujourd'hui (`journalui.rs`).
                        if bulles::icone(ui, ph::ARROWS_LEFT_RIGHT, "Journal des différences depuis cet envoi", "Ce qui a changé depuis cet envoi : les plans modifiés avant / après, ajoutés, retirés, déplacés, et les notes traitées. PDF et page HTML.", &self.raccourci("envoi.journal")).clicked() {
                            journal = Some(e.id);
                        }
                        // La page de relecture de cet envoi : les plans qui ont changé depuis, à envoyer au réalisateur.
                        if bulles::icone(ui, ph::EYEGLASSES, "Exporter pour relecture", "Une page à envoyer au réalisateur (qui n'a pas FEANOR) pour qu'il écrive ses notes dessus ; proposée avec les plans modifiés depuis cet envoi.", &self.raccourci("fichier.exporter_relecture")).clicked() {
                            relire = Some(e.id);
                        }
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
        if let Some(id) = relire {
            self.open_relecture_export(Some(id));
        }
        if let Some(id) = journal {
            self.journal_ouvrir(Some(id));
        }
        if let Some(id) = comparer {
            self.comparer_ouvrir(self.cur_panel, Some(crate::comparer::Source::Envoi(id)), Some(crate::comparer::Source::Actuelle));
        }
        if let Some(i) = aller {
            self.select_panel(i);
            self.board_sel = vec![self.doc.panels[i].id];
        }
        self.envois_ui.liste = ouverte;
    }
}

/// Dessine le repère « modifié depuis l'envoi » centré en `centre` : une pastille d'avertissement du thème avec
/// un crayon (modifié) ou un plus (ajouté).
pub(crate) fn peindre_repere(painter: &egui::Painter, centre: egui::Pos2, etat: Etat, rayon: f32) {
    painter.circle_filled(centre, rayon, theme::braise());
    painter.circle_stroke(centre, rayon, Stroke::new(1.0_f32, theme::encre()));
    let glyphe = if etat == Etat::Nouveau { ph::PLUS } else { ph::PENCIL_SIMPLE };
    painter.text(centre, Align2::CENTER_CENTER, glyphe, FontId::proportional(rayon * 1.25), theme::encre());
}

/// Largeur en dessous de laquelle un bloc de la bande n'a plus la place d'une pastille.
const BLOC_ETROIT: f32 = 44.0;
/// Épaisseur de la bande qui tient lieu de pastille sur un bloc étroit.
const BANDE: f32 = 5.0;

/// Le repère d'un bloc de cette taille est une pastille (sinon une bande le long du bord haut). L'étiquette de
/// version (`pilevers.rs`) s'en sert pour lui laisser sa place.
pub(crate) fn repere_en_pastille(bloc: Rect) -> bool {
    bloc.width() >= BLOC_ETROIT && bloc.height() >= 40.0
}

/// Peint le repère d'un plan sur son bloc de la bande et rend la zone où la souris le survole : une pastille en
/// haut à droite si le bloc est assez large pour ne pas cacher sa poignée de bord, sinon (la bande zoomée loin) une
/// bande ambre le long de son bord haut, qui se voit à n'importe quel zoom.
pub(crate) fn peindre_repere_du_bloc(painter: &egui::Painter, bloc: Rect, etat: Etat) -> Rect {
    if repere_en_pastille(bloc) {
        let zone = Rect::from_center_size(pos2(bloc.right() - 18.0, bloc.top() + 18.0), vec2(REPERE * 2.0 + 2.0, REPERE * 2.0 + 2.0));
        peindre_repere(painter, zone.center(), etat, REPERE);
        zone
    } else {
        let bande = Rect::from_min_max(bloc.min, pos2(bloc.right(), bloc.top() + BANDE));
        painter.rect_filled(bande, 1.0, theme::braise());
        bande.expand2(vec2(0.0, 2.0))
    }
}