//! Le décor 3D d'une séquence, côté interface (parties 180 et 181, `docs/INTERFACE.md`) : Fichier > Importer > « Objet 3D pour
//! le décor de la séquence (glTF, .glb)… » et les modèles de la banque 3D (`banque3dui.rs`), qui **ajoutent** un objet par le
//! même chemin (lecture, mesure, budget de la machine pour tout le décor, fenêtre « Ce décor est lourd ») ; « Enlever le décor
//! 3D de la séquence » ; la liste des objets et la retouche de l'objet choisi (place, origine, aimanté au sol) et de l'aspect
//! dans le panneau Espace 3D ; le décor dans la vue Espace 3D ; le budget de cette machine (Préférences > Performance).
//!
//! **Rien n'attend sur le fil de l'interface** : la lecture, la mesure, l'étalonnage de la machine, l'allègement et la
//! simplification tournent sur d'autres fils ; l'écran montre l'avancement et reste libre. Le fichier d'origine n'est jamais
//! modifié : seule la copie gardée dans le projet (`decors/<id>.glb`) est allégée ou simplifiée.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use eframe::egui::{self, pos2, Align2, Color32, FontId, Rect, RichText, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::decor3d::{Allege, Analyse, Decor3d, Modele3d, Objet3d, Options, Place3d, Poids, Verdict};
use crate::decor3dbudget::{grand_nombre, BudgetMachine};
use crate::decor3dsimplifier::Suivi;
use crate::espace3d::Monde;
use crate::espace3dmath::{self as m, View, V3};

/// Où va l'objet qu'on ajoute : la séquence (celle du plan courant au début de l'import : changer de plan pendant la lecture
/// ne l'envoie pas ailleurs), et, lâché sur la vue Espace 3D, le point du sol sous le pointeur (x, z) ; rien : devant la caméra
/// du plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cible3d {
    pub seq: u64,
    pub xz: Option<[f32; 2]>,
}

/// Ce que l'interface du décor retient (un par projet ouvert).
#[derive(Default)]
pub(crate) struct Etat3d {
    /// La lecture et la mesure d'un fichier (ou d'un modèle de la banque), sur un autre fil.
    lecture: Option<JoinHandle<Result<Analyse, String>>>,
    /// Où ira l'objet lu.
    cible: Option<Cible3d>,
    /// Un fichier lu, qui attend la fin de l'étalonnage de la machine pour être jugé.
    lu: Option<Analyse>,
    /// La fenêtre « Ce décor est lourd ».
    lourd: Option<Lourd>,
    /// Les modèles déjà comparés au budget de cette machine (`Modele3d::uid`).
    vus: BTreeSet<u64>,
    /// La retouche en cours d'un objet ou de l'aspect : le projet d'avant (les décors, et les cartes aimantées qui reposent
    /// dessus), pour l'étape d'annulation, au lâcher.
    retouche: Option<crate::history::Edit>,
    /// L'objet choisi dans la liste du panneau Espace 3D (`Objet3d::id`) : ses champs se retouchent.
    pub(crate) choisi: Option<u64>,
}

impl Etat3d {
    /// La fenêtre « Ce décor est lourd » est ouverte (essais).
    #[cfg(test)]
    pub(crate) fn fenetre_ouverte(&self) -> bool {
        self.lourd.is_some()
    }

    /// Un import attend (lecture, étalonnage) (essais).
    #[cfg(test)]
    pub(crate) fn en_attente(&self) -> bool {
        self.lecture.is_some() || self.lu.is_some()
    }

    /// Un import est en cours (lecture, étalonnage, fenêtre ouverte) : un autre attend qu'il finisse.
    pub(crate) fn occupe(&self) -> bool {
        self.lecture.is_some() || self.lu.is_some() || self.lourd.is_some()
    }
}

/// Le but de la simplification.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cible {
    Moitie,
    Quart,
    Budget,
}

struct Calcul {
    options: Options,
    suivi: Arc<Suivi>,
    fil: Option<JoinHandle<Result<Allege, String>>>,
    fini: Option<Result<Allege, String>>,
}

struct Lourd {
    analyse: Analyse,
    raisons: Vec<String>,
    tel_quel: bool,
    alleger: bool,
    cote: u32,
    simplifier: bool,
    cible: Cible,
    calcul: Option<Calcul>,
    /// Un modèle déjà dans le projet (ouvert sur cet ordinateur-ci) : son `Modele3d::uid` ; rien : un objet qu'on ajoute.
    existant: Option<u64>,
    /// Où va l'objet qu'on ajoute.
    ajout: Option<Cible3d>,
    /// Ce que pèse déjà le décor de la séquence, sans lui (le budget vaut pour la somme).
    deja: Poids,
    /// Il reprend un modèle déjà dans la séquence (ses textures ne comptent pas deux fois).
    partage: bool,
}

/// Un bouton du presse-papiers des objets du décor (panneau Espace 3D, partie 190).
#[derive(Clone, Copy)]
enum GestePresse {
    Copier,
    Couper,
    Coller,
    /// Copier (faux) ou envoyer (vrai) vers la séquence.
    Vers(u64, bool),
}

/// Le titre de la fenêtre.
pub(crate) const TITRE_LOURD: &str = "Ce décor est lourd";

/// Pourquoi on juge un modèle.
enum Quoi {
    /// Un objet qu'on ajoute au décor de la séquence `Cible3d::seq`.
    Ajout(Cible3d),
    /// Un modèle déjà dans le projet (`Modele3d::uid`), dans un décor qui pèse déjà `deja` sans lui.
    Existant { uid: u64, deja: Poids },
}

/// Ce qu'on pose : un modèle lu (tel quel ou allégé), et d'où il vient.
struct Pose {
    nom: String,
    source: String,
    glb: Arc<Vec<u8>>,
    scene: Arc<crate::decor3dgltf::Scene3d>,
    empreinte: String,
    allege: Vec<String>,
}

impl App {
    /// La clé de cette machine (`decor3dbudget::cle_machine`).
    fn decor3d_cle(&self) -> String {
        crate::decor3dbudget::cle_machine(self.gpu.as_ref().map(|g| g.render_state().adapter.get_info()).as_ref())
    }

    /// Le budget gardé, s'il vaut pour cette machine.
    pub(crate) fn decor3d_budget(&self) -> Option<&BudgetMachine> {
        let cle = self.decor3d_cle();
        self.settings.decor3d.as_ref().filter(|b| b.cle == cle)
    }

    /// Au démarrage : le budget de cette machine, s'il a été mesuré ici, sert à l'écran et à l'export.
    pub(crate) fn decor3d_budget_au_demarrage(&mut self) {
        crate::decor3d::poser_budget(self.decor3d_budget().cloned());
    }

    /// Lance l'étalonnage de cette machine (rien s'il tourne déjà).
    fn decor3d_etalonner(&mut self) {
        let carte = self.gpu.as_ref().map(|g| g.render_state().clone());
        let max = carte.as_ref().map_or(16384, |rs| rs.device.limits().max_texture_dimension_2d);
        let feuille = [(self.doc.width as u32).min(max), (self.doc.height as u32).min(max)];
        crate::decor3dbudget::lancer(carte, feuille, self.doc.frame);
        self.status = "Décor 3D : FEANOR mesure ce que cet ordinateur dessine sans ralentir (quelques secondes)…".into();
    }

    /// « Remesurer » (Préférences > Performance, diagnostic des performances, palette).
    pub(crate) fn decor3d_remesurer(&mut self) {
        self.decor3d_etalonner();
    }

    /// Fichier > Importer > « Objet 3D pour le décor de la séquence (glTF, .glb)… » : ajoute un objet (partie 181 ; il ne
    /// remplace plus le décor).
    pub(crate) fn decor3d_importer(&mut self) {
        if self.sans_projet() || self.content_refuses() {
            return;
        }
        if self.decor3d.occupe() {
            self.status = "Un objet 3D est déjà en cours d'import.".into();
            return;
        }
        let Some(chemin) = rfd::FileDialog::new().set_title("Ajouter un objet 3D au décor de la séquence").add_filter("glTF 2.0", &["glb", "gltf"]).pick_file() else { return };
        self.decor3d_importer_chemin(chemin);
    }

    /// La séquence du plan courant, comme cible d'un ajout (devant la caméra).
    pub(crate) fn decor3d_cible_ici(&self) -> Cible3d {
        Cible3d { seq: self.doc.sequence_of(self.cur_panel), xz: None }
    }

    /// Lit et mesure `chemin` sur un autre fil, pour l'ajouter au décor de la séquence du plan courant ; la suite à
    /// `decor3d_tick`.
    pub(crate) fn decor3d_importer_chemin(&mut self, chemin: PathBuf) {
        let cible = self.decor3d_cible_ici();
        self.decor3d_lancer(cible, move || crate::decor3d::analyser(&chemin));
    }

    /// Le chemin commun de tout ajout (un fichier, un modèle de la banque 3D) : `lire` tourne sur un autre fil, puis le temps
    /// d'une image est mesuré sur la carte, puis le modèle est jugé contre le budget (`decor3d_juger`). Faux : un autre import
    /// est en cours (la barre d'état le dit).
    pub(crate) fn decor3d_lancer(&mut self, cible: Cible3d, lire: impl FnOnce() -> Result<Analyse, String> + Send + 'static) -> bool {
        if self.decor3d.occupe() {
            self.status = "Un objet 3D est déjà en cours d'import : attends qu'il soit posé.".into();
            return false;
        }
        let carte = self.gpu.as_ref().map(|g| g.render_state().clone());
        let (feuille, cadre) = ([self.doc.width, self.doc.height], self.doc.frame);
        self.decor3d.cible = Some(cible);
        // (Partie 198.5 : l'étalonnage d'abord, s'il faut ; la lecture l'attend avant de chronométrer une image du modèle.)
        if self.decor3d_budget().is_none() {
            self.decor3d_etalonner();
        } else {
            self.status = "Décor 3D : lecture du modèle…".into();
        }
        let interface = std::thread::current().id();
        #[cfg(test)]
        let cout_force = COUT_ESSAI.with(|c| c.get());
        self.decor3d.lecture = Some(std::thread::spawn(move || {
            let mut a = lire()?;
            // Le temps réel d'une image de ce décor sur la carte de cette machine (s'il n'est pas d'emblée hors budget), une fois
            // l'étalonnage fini : les deux en même temps sur la carte faussent la mesure (un modèle de 196 triangles mesuré à 1,6 ms).
            if let Some(rs) = carte.filter(|_| a.mesures.triangles <= 4_000_000) {
                let debut = std::time::Instant::now();
                while crate::decor3dbudget::en_cours_pour(interface) && debut.elapsed() < std::time::Duration::from_secs(60) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                a.cout_ms = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mesurer_cout(&rs, &a.scene, feuille, cadre))).ok().flatten();
            }
            #[cfg(test)]
            if let Some(ms) = cout_force {
                while crate::decor3dbudget::en_cours_pour(interface) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                a.cout_ms = Some(ms);
            }
            Ok(a)
        }));
        true
    }

    /// À chaque image d'écran : l'étalonnage, la lecture, la fenêtre, la vérification des décors du projet.
    pub(crate) fn decor3d_tick(&mut self, ctx: &egui::Context) {
        if let Some(b) = crate::decor3dbudget::fini() {
            self.status = format!("Décor 3D sur cet ordinateur : {}.", b.texte());
            self.settings.decor3d = Some(b);
            let _ = self.settings.save();
            crate::decor3d::poser_budget(self.settings.decor3d.clone());
            // (Un nouveau budget : les décors du projet sont jugés de nouveau.)
            self.decor3d.vus.clear();
        }
        if crate::decor3dbudget::en_cours() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if self.decor3d.lecture.as_ref().is_some_and(|h| h.is_finished()) {
            let r = self.decor3d.lecture.take().and_then(|h| h.join().ok()).unwrap_or_else(|| Err("lecture interrompue".into()));
            match r {
                Ok(a) => self.decor3d.lu = Some(a),
                Err(e) => {
                    self.decor3d.cible = None;
                    self.status = format!("Décor 3D refusé : {e}");
                }
            }
        } else if self.decor3d.lecture.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        // Un fichier lu se juge quand le budget de la machine est là.
        if self.decor3d.lu.is_some() {
            if let Some(b) = self.decor3d_budget().cloned() {
                let a = self.decor3d.lu.take().expect("fichier lu");
                let cible = self.decor3d.cible.take().unwrap_or_else(|| self.decor3d_cible_ici());
                self.decor3d_juger(a, &b, Quoi::Ajout(cible));
            } else if !crate::decor3dbudget::en_cours() {
                self.decor3d_etalonner();
            }
        }
        // Les décors du projet (ouvert sur cet ordinateur, ou rétablis) : chaque modèle comparé au budget d'ici une fois, avec
        // ce qui le précède dans le décor de sa séquence (le budget vaut pour la somme).
        let a_voir = self.doc.decors3d.par_sequence.values().flat_map(|d| &d.objets).any(|o| !self.decor3d.vus.contains(&o.modele.uid));
        if a_voir && self.decor3d.lourd.is_none() {
            match self.decor3d_budget().cloned() {
                Some(b) => self.decor3d_verifier_projet(&b),
                None if !crate::decor3dbudget::en_cours() => self.decor3d_etalonner(),
                None => {}
            }
        }
        self.decor3d_fenetre(ctx);
    }

    /// Compare au budget `b` les modèles du projet pas encore vus : un modèle qui ferait dépasser le décor de sa séquence est
    /// masqué sur cet ordinateur et la fenêtre « Ce décor est lourd » s'ouvre pour lui (une à la fois).
    fn decor3d_verifier_projet(&mut self, b: &BudgetMachine) {
        let mut trouve: Option<(Analyse, u64, Poids)> = None;
        'seq: for d in self.doc.decors3d.par_sequence.values() {
            let mut deja = Poids::default();
            let mut modeles: BTreeSet<u64> = BTreeSet::new();
            for o in &d.objets {
                let uid = o.modele.uid;
                if crate::decor3d::masque(uid) {
                    continue;
                }
                let partage = modeles.contains(&uid);
                if !self.decor3d.vus.contains(&uid) {
                    let mesures = mesures_de(&o.modele);
                    let v = crate::decor3d::verdict_sequence(&mesures, deja, partage, b, None);
                    if matches!(v, Verdict::Lourd { tel_quel: false, .. }) {
                        let a = Analyse { nom: o.nom.clone(), origine: o.source.clone(), glb: o.modele.octets.clone(), scene: o.modele.scene.clone(), mesures, cout_ms: None, empreinte: o.modele.empreinte.clone() };
                        trouve = Some((a, uid, deja));
                        break 'seq;
                    }
                    self.decor3d.vus.insert(uid);
                }
                deja.triangles += o.modele.scene.triangles() as u64;
                if !partage {
                    deja.octets_textures += o.modele.scene.octets_textures();
                }
                modeles.insert(uid);
            }
        }
        if let Some((a, uid, deja)) = trouve {
            self.decor3d.vus.insert(uid);
            self.status = format!("L'objet 3D « {} » fait dépasser ce que cet ordinateur dessine sans ralentir : il est masqué en attendant ton choix.", a.nom);
            crate::decor3d::masquer(uid, true);
            self.decor3d_juger(a, b, Quoi::Existant { uid, deja });
        }
    }

    /// Le modèle lu, comparé au budget avec ce que pèse déjà le décor de sa séquence : posé tout de suite, la fenêtre « Ce décor
    /// est lourd », ou refusé si la séquence est déjà au bout du budget (jamais de ralentissement).
    fn decor3d_juger(&mut self, a: Analyse, b: &BudgetMachine, quoi: Quoi) {
        let (deja, partage, ajout, existant) = match quoi {
            Quoi::Ajout(c) => {
                let d = self.doc.decors3d.par_sequence.get(&c.seq);
                let deja = d.map(|d| d.poids()).unwrap_or_default();
                if let Some(raison) = crate::decor3d::sequence_pleine(deja, b) {
                    self.status = format!("Objet 3D « {} » refusé : {raison}", a.nom);
                    return;
                }
                (deja, d.is_some_and(|d| d.modele_d_empreinte(&a.empreinte).is_some()), Some(c), None)
            }
            Quoi::Existant { uid, deja } => (deja, false, None, Some(uid)),
        };
        match crate::decor3d::verdict_sequence(&a.mesures, deja, partage, b, a.cout_ms) {
            Verdict::Direct => {
                if let Some(c) = ajout {
                    let p = Pose { nom: a.nom.clone(), source: a.origine.clone(), glb: a.glb.clone(), scene: a.scene.clone(), empreinte: a.empreinte.clone(), allege: Vec::new() };
                    self.decor3d_poser(p, c);
                }
            }
            Verdict::Lourd { raisons, tel_quel, simplifier } => {
                let trop_textures = a.mesures.octets_textures + deja.octets_textures > b.textures_mo << 20 || a.mesures.plus_grand_cote > 4096;
                self.decor3d.lourd = Some(Lourd { analyse: a, raisons, tel_quel, alleger: true, cote: if trop_textures { 2048 } else { 4096 }, simplifier, cible: Cible::Budget, calcul: None, existant, ajout, deja, partage });
            }
        }
    }

    /// Pose un objet neuf dans le décor de la séquence `c.seq` (une étape d'annulation) : son origine au centre du dessous de sa
    /// boîte, posée sur le sol devant la caméra du plan (ou au point lâché), aimanté au sol. Un modèle de même contenu déjà dans le
    /// projet est repris (un fichier, une lecture, un envoi à la carte).
    fn decor3d_poser(&mut self, p: Pose, c: Cible3d) {
        let modele = self.doc.decors3d.modele_d_empreinte(&p.empreinte).unwrap_or_else(|| Arc::new(Modele3d::avec_empreinte(p.scene.clone(), p.glb.clone(), p.empreinte.clone())));
        self.decor3d.vus.insert(modele.uid);
        let monde = Monde::of(&self.doc);
        let snap = self.snapshot();
        let id = self.doc.alloc();
        let mut o = Objet3d::neuf(id, p.nom.clone(), p.source, p.allege.clone(), modele);
        let (lo, hi) = o.modele.bornes;
        let [x, z] = c.xz.unwrap_or_else(|| {
            // Devant la caméra du plan : en face de son œil, le devant de l'objet contre la feuille (tout entier derrière).
            let oeil = monde.eye_seen(&self.doc.camera_seen(self.cur_panel.min(self.doc.panels.len().saturating_sub(1)), self.cur_frame));
            [oeil[0], -(hi[2] - lo[2]) / 2.0]
        });
        o.poser_au_sol(x, z, monde.sol_y());
        let triangles = o.modele.scene.triangles();
        let d = self.doc.decors3d.par_sequence.entry(c.seq).or_insert_with(|| Decor3d::new(Default::default()));
        d.objets.push(o);
        let n = d.objets.len();
        let notes = crate::decor3daimant::aimanter(d, monde.sol_y());
        self.reposer_les_cartes_aimantees(c.seq);
        let before = self.history.context.replace("Ajouter un objet 3D".into());
        self.commit_structure(snap);
        self.history.context = before;
        self.decor3d.choisi = Some(id);
        let taille = [0, 1, 2].map(|k| hi[k] - lo[k]);
        let plus_grand = taille.iter().cloned().fold(0.0f32, f32::max);
        self.status = format!(
            "Objet 3D « {} » posé dans le décor de la séquence ({n} objet{}) : {:.2} × {:.2} × {:.2} m (largeur, hauteur, profondeur), {} triangles{}{}{}.",
            p.nom,
            if n > 1 { "s" } else { "" },
            taille[0],
            taille[1],
            taille[2],
            grand_nombre(triangles as u64),
            if plus_grand > 500.0 {
                " — très grand : vérifie son échelle"
            } else if plus_grand < 0.05 {
                " — très petit : vérifie son échelle"
            } else {
                ""
            },
            if p.allege.is_empty() { String::new() } else { format!(" ; {}", p.allege.join(", ")) },
            if notes.is_empty() { String::new() } else { format!(" ; {}", notes.join(" ")) }
        );
    }

    /// Une retouche structurelle du décor de la séquence `seq` (enlever, dupliquer…) : `f` change le décor, les objets aimantés
    /// se reposent, une étape d'annulation nommée `nom`. Le texte de `f` va dans la barre d'état.
    fn decor3d_changer(&mut self, seq: u64, nom: &str, f: impl FnOnce(&mut Decor3d, &mut crate::document::Document) -> Option<String>) {
        if self.content_refuses() {
            return;
        }
        self.decor3d_retouche_finir();
        let Some(mut d) = self.doc.decors3d.par_sequence.get(&seq).cloned() else { return };
        let snap = self.snapshot();
        let Some(texte) = f(&mut d, &mut self.doc) else { return };
        let notes = crate::decor3daimant::aimanter(&mut d, Monde::of(&self.doc).sol_y());
        if d.objets.is_empty() {
            self.doc.decors3d.par_sequence.remove(&seq);
        } else {
            self.doc.decors3d.par_sequence.insert(seq, d);
        }
        self.reposer_les_cartes_aimantees(seq);
        let before = self.history.context.replace(nom.into());
        self.commit_structure(snap);
        self.history.context = before;
        self.status = if notes.is_empty() { texte } else { format!("{texte} {}", notes.join(" ")) };
    }

    /// « Enlever le décor 3D de la séquence » : tous ses objets (une étape d'annulation).
    pub(crate) fn decor3d_enlever(&mut self) {
        let seq = self.doc.sequence_of(self.cur_panel);
        if self.doc.decor_du_plan(self.cur_panel).is_none() {
            self.status = "Cette séquence n'a pas de décor 3D.".into();
            return;
        }
        self.decor3d_changer(seq, "Enlever le décor 3D", |d, _| {
            let n = d.objets.len();
            d.objets.clear();
            Some(format!("Décor 3D de la séquence enlevé ({n} objet{}) (Ctrl+Z le rend).", if n > 1 { "s" } else { "" }))
        });
    }

    /// « Enlever » un objet du décor de la séquence du plan courant (une étape d'annulation) ; ceux qui reposaient sur lui
    /// redescendent.
    pub(crate) fn decor3d_enlever_objet(&mut self, id: u64) {
        let seq = self.doc.sequence_of(self.cur_panel);
        self.decor3d_changer(seq, "Enlever un objet 3D", |d, _| {
            let i = d.objets.iter().position(|o| o.id == id)?;
            let o = d.objets.remove(i);
            Some(format!("Objet 3D « {} » enlevé du décor (Ctrl+Z le rend).", o.nom))
        });
        if self.decor3d.choisi == Some(id) {
            self.decor3d.choisi = None;
        }
        // (Ni dans la vue : partie 184.)
        if self.doc.decors3d.objet(id).is_none() {
            self.espace3d.sel.retain(|&s| s != crate::poignees3d::objet_selection(id));
        }
    }

    /// « Dupliquer » un objet : la copie (même modèle, partagé : rien n'est relu ni renvoyé à la carte) posée juste à côté, à sa
    /// droite (une étape d'annulation).
    pub(crate) fn decor3d_dupliquer_objet(&mut self, id: u64) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let mut neuf = None;
        self.decor3d_changer(seq, "Dupliquer un objet 3D", |d, doc| {
            let o = d.objet(id)?.clone();
            let (lo, hi) = o.bornes_monde();
            let mut copie = o.clone();
            copie.id = doc.alloc();
            copie.nom = format!("{} (copie)", o.nom);
            copie.place.position[0] += ((hi[0] - lo[0]) * 1.1).max(0.3);
            neuf = Some(copie.id);
            d.objets.push(copie);
            Some(format!("Objet 3D « {} » dupliqué à côté.", o.nom))
        });
        if neuf.is_some() {
            self.decor3d.choisi = neuf;
        }
    }

    // ------------------------------------------------------------------------------------- la fenêtre « lourd »
    // ------------------------------------------------------------------------------------- la fenêtre « lourd »

    fn decor3d_fenetre(&mut self, ctx: &egui::Context) {
        let Some(mut l) = self.decor3d.lourd.take() else { return };
        let budget = self.decor3d_budget().cloned();
        let options = l.options(budget.as_ref());
        // Le calcul d'après suit les choix : un autre choix arrête le calcul en cours et en lance un neuf.
        if l.calcul.as_ref().is_none_or(|c| c.options != options) {
            if let Some(c) = l.calcul.take() {
                c.suivi.annule.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if options.cote_max.is_some() || options.triangles.is_some() {
                let suivi = Arc::new(Suivi::default());
                let (a, s) = (l.analyse.clone(), suivi.clone());
                let fil = std::thread::spawn(move || crate::decor3d::alleger(&a, options, &s));
                l.calcul = Some(Calcul { options, suivi, fil: Some(fil), fini: None });
            }
        }
        if let Some(c) = l.calcul.as_mut() {
            if c.fil.as_ref().is_some_and(|h| h.is_finished()) {
                c.fini = Some(c.fil.take().and_then(|h| h.join().ok()).unwrap_or_else(|| Err("calcul interrompu".into())));
            } else if c.fil.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        let mut choix: Option<&'static str> = None;
        let mut pret: Option<Allege> = None;
        let mm = l.analyse.mesures.clone();
        egui::Window::new(TITRE_LOURD).collapsible(false).resizable(false).anchor(Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.set_max_width(520.0);
            ui.label(RichText::new(format!("« {} » ({})", l.analyse.nom, l.analyse.origine)).strong());
            if l.deja.triangles > 0 {
                ui.label(RichText::new(format!("Le décor de la séquence a déjà {} triangles : le budget de cet ordinateur vaut pour tous ses objets ensemble.", grand_nombre(l.deja.triangles))).small().weak());
            }
            egui::Grid::new("decor3d_mesures").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                let mut ligne = |a: &str, b: String| {
                    ui.label(a);
                    ui.label(b);
                    ui.end_row();
                };
                ligne("Fichier", crate::decor3d::octets_lisibles(mm.octets_fichier));
                ligne("Triangles", grand_nombre(mm.triangles as u64));
                ligne("Sommets", grand_nombre(mm.sommets as u64));
                ligne("Matières", mm.matieres.to_string());
                ligne("Textures", format!("{} ({} en mémoire, plus grand côté {} px)", mm.textures, crate::decor3d::octets_lisibles(mm.octets_textures), mm.plus_grand_cote));
                if mm.cartes_inutiles + mm.uv_en_plus + mm.tangentes + mm.couleurs_sommets + mm.animations + mm.peaux + mm.morphoses > 0 {
                    ligne(
                        "Inutilisé par FEANOR",
                        format!(
                            "{} carte(s) de normales, rugosité, occlusion ou émission ({}) ; {} primitive(s) avec d'autres coordonnées, tangentes ou couleurs de sommets ; {} animation(s), {} squelette(s), {} morphose(s)",
                            mm.cartes_inutiles,
                            crate::decor3d::octets_lisibles(mm.octets_cartes_inutiles),
                            mm.uv_en_plus.max(mm.tangentes).max(mm.couleurs_sommets),
                            mm.animations,
                            mm.peaux,
                            mm.morphoses
                        ),
                    );
                }
                if let Some(ms) = l.analyse.cout_ms {
                    ligne("Une image sur la carte", format!("{ms:.1} ms"));
                }
            });
            if let Some(b) = &budget {
                ui.label(RichText::new(format!("Cet ordinateur : {} (pour rester fluide).", b.texte())).small().weak());
            }
            ui.add_space(4.0);
            for r in &l.raisons {
                ui.label(format!("• {r}"));
            }
            ui.separator();
            ui.checkbox(&mut l.alleger, "Alléger (la forme ne change pas)");
            ui.add_enabled_ui(l.alleger, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Textures au plus :");
                    ui.radio_value(&mut l.cote, 4096, "4096 px");
                    ui.radio_value(&mut l.cote, 2048, "2048 px");
                    ui.radio_value(&mut l.cote, 1024, "1024 px");
                });
                ui.label(RichText::new("Retire aussi ce que FEANOR ne montre pas : cartes de normales, de rugosité, d'occlusion et d'émission, coordonnées en plus, tangentes, couleurs de sommets, animations, squelettes et morphoses (ils compteront pour les personnages, à l'étape 2 : le fichier d'origine les garde). Sommets soudés, indices sur 16 bits quand ils tiennent.").small().weak());
            });
            ui.checkbox(&mut l.simplifier, "Simplifier la géométrie (modifie la forme)");
            ui.add_enabled_ui(l.simplifier, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut l.cible, Cible::Moitie, "50 %");
                    ui.radio_value(&mut l.cible, Cible::Quart, "25 %");
                    let b = budget.as_ref().map_or("le budget".to_string(), |b| grand_nombre(b.triangles.saturating_sub(l.deja.triangles)));
                    ui.radio_value(&mut l.cible, Cible::Budget, format!("jusqu'à {b} triangles"));
                });
                ui.label(RichText::new("Contraction d'arêtes guidée par l'erreur : les bords, les coutures de texture et les frontières de matière restent.").small().weak());
            });
            ui.separator();
            match l.calcul.as_ref() {
                None => {
                    ui.label(RichText::new("Coche « Alléger » ou « Simplifier » pour voir le résultat.").weak());
                }
                Some(c) => match &c.fini {
                    None => {
                        ui.add(egui::ProgressBar::new(c.suivi.fraction()).text("Calcul de l'allègement…").animate(true));
                    }
                    Some(Err(e)) => {
                        ui.colored_label(Color32::from_rgb(220, 120, 90), format!("Allègement impossible : {e}"));
                    }
                    Some(Ok(r)) => {
                        let apres = &r.mesures;
                        ui.label(format!(
                            "Avant → après : fichier {} → {} ; textures en mémoire {} → {} ; triangles {} → {}.",
                            crate::decor3d::octets_lisibles(mm.octets_fichier),
                            crate::decor3d::octets_lisibles(apres.octets_fichier),
                            crate::decor3d::octets_lisibles(mm.octets_textures),
                            crate::decor3d::octets_lisibles(apres.octets_textures),
                            grand_nombre(mm.triangles as u64),
                            grand_nombre(apres.triangles as u64),
                        ));
                        let tient = budget.as_ref().is_none_or(|b| matches!(crate::decor3d::verdict_sequence(apres, l.deja, l.partage, b, None), Verdict::Direct | Verdict::Lourd { tel_quel: true, .. }));
                        if tient {
                            pret = Some(r.clone());
                        } else {
                            ui.colored_label(Color32::from_rgb(220, 160, 90), "Encore au-delà de ce que cet ordinateur dessine sans ralentir : allège ou simplifie davantage.");
                        }
                    }
                },
            }
            ui.horizontal(|ui| {
                let verbe = match (l.simplifier, l.alleger) {
                    (true, true) => "Simplifier, alléger et importer",
                    (true, false) => "Simplifier et importer",
                    _ => "Alléger et importer",
                };
                let verbe = if l.existant.is_some() { verbe.replace("importer", "remplacer") } else { verbe.to_string() };
                if ui.add_enabled(pret.is_some(), egui::Button::new(verbe)).clicked() {
                    choix = Some("alleger");
                }
                if l.tel_quel && ui.button(if l.existant.is_some() { "Garder tel quel" } else { "Importer tel quel" }).clicked() {
                    choix = Some("tel_quel");
                }
                if ui.button(if l.existant.is_some() { "Le masquer sur cet ordinateur" } else { "Annuler" }).clicked() {
                    choix = Some("annuler");
                }
            });
        });
        match choix {
            Some("alleger") => {
                if let Some(r) = pret {
                    match (l.existant, l.ajout) {
                        (Some(uid), _) => self.decor3d_remplacer(uid, r),
                        (None, Some(c)) => {
                            let p = Pose { nom: l.analyse.nom.clone(), source: l.analyse.origine.clone(), glb: r.glb.clone(), scene: r.scene.clone(), empreinte: r.empreinte.clone(), allege: r.notes.clone() };
                            self.decor3d_poser(p, c);
                        }
                        (None, None) => {}
                    }
                }
            }
            Some("tel_quel") => match (l.existant, l.ajout) {
                (Some(uid), _) => crate::decor3d::masquer(uid, false),
                (None, Some(c)) => {
                    let a = &l.analyse;
                    let p = Pose { nom: a.nom.clone(), source: a.origine.clone(), glb: a.glb.clone(), scene: a.scene.clone(), empreinte: a.empreinte.clone(), allege: Vec::new() };
                    self.decor3d_poser(p, c);
                }
                (None, None) => {}
            },
            Some(_) => {
                if let Some(c) = l.calcul {
                    c.suivi.annule.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                if l.existant.is_none() {
                    self.status = "Import du décor 3D annulé.".into();
                }
            }
            None => self.decor3d.lourd = Some(l),
        }
    }

    /// Le modèle `uid`, remplacé par sa version allégée dans tous les objets qui le montrent (une étape d'annulation) ; leurs
    /// noms, places et origines restent, les aimantés se reposent.
    fn decor3d_remplacer(&mut self, uid: u64, r: Allege) {
        if !self.doc.decors3d.par_sequence.values().flat_map(|d| &d.objets).any(|o| o.modele.uid == uid) {
            return;
        }
        let snap = self.snapshot();
        let modele = Arc::new(Modele3d::avec_empreinte(r.scene.clone(), r.glb.clone(), r.empreinte.clone()));
        self.decor3d.vus.insert(modele.uid);
        let sol = Monde::of(&self.doc).sol_y();
        let mut nom = String::new();
        let mut sequences = Vec::new();
        for (&seq, d) in self.doc.decors3d.par_sequence.iter_mut() {
            if !d.objets.iter().any(|o| o.modele.uid == uid) {
                continue;
            }
            for o in d.objets.iter_mut().filter(|o| o.modele.uid == uid) {
                o.modele = modele.clone();
                o.allege.extend(r.notes.iter().cloned());
                nom = o.nom.clone();
            }
            crate::decor3daimant::aimanter(d, sol);
            sequences.push(seq);
        }
        for seq in sequences {
            self.reposer_les_cartes_aimantees(seq);
        }
        crate::decor3d::masquer(uid, false);
        let before = self.history.context.replace("Alléger un objet 3D".into());
        self.commit_structure(snap);
        self.history.context = before;
        self.status = format!("Objet 3D « {nom} » allégé : {}.", r.notes.join(", "));
    }

    // ---------------------------------------------------------------------------------- la vue Espace 3D

    /// Le décor de la séquence dans une vue Espace 3D (entre le quadrillage et les cartes) : tous ses objets, dessinés par la
    /// carte graphique dans le même tampon de profondeur ; sans elle, leurs boîtes en fil de fer et une légende. L'objet choisi
    /// dans le panneau a sa boîte en clair. `cle` : la prise (une par vue). Vrai : la carte graphique dessine le décor de cette vue
    /// (les cartes peuvent alors s'y cacher, partie 183 : `e3_paint_cards`).
    pub(crate) fn e3_peindre_decor(&self, painter: &egui::Painter, draw: Rect, view: &View, cle: u64) -> bool {
        let Some(d) = self.doc.decor_du_plan(self.cur_panel) else { return false };
        let mut sur_la_carte = false;
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let proj = |p: V3| view.project(p, center).map(|q| pos2(draw.left() + q[0], draw.top() + q[1]));
        let boite = |o: &crate::decor3d::Objet3d, trait_: Stroke| {
            let (lo, hi) = o.modele.bornes;
            let mx = o.matrice();
            let coin = |i: usize| m::point(&mx, [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }));
            for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                if let (Some(p), Some(q)) = (proj(coin(a)), proj(coin(b))) {
                    painter.line_segment([p, q], trait_);
                }
            }
        };
        // (Partie 200 : « Caméra du plan » ne montre pas les objets cachés dans ce plan ; les autres vues les montrent, en pointillés.)
        let plan = self.doc.panels[self.cur_panel].id;
        let par_la_camera = self.espace3d.through_camera;
        let objets: Vec<crate::decor3d::Pose3d> = crate::decor3d::poses_sauf(d, par_la_camera.then_some(plan));
        if !par_la_camera {
            for o in d.vus().filter(|o| o.cache_dans(plan)) {
                let (lo, hi) = o.modele.bornes;
                let mx = o.matrice();
                let coin = |i: usize| m::point(&mx, [0, 1, 2].map(|k| if i >> k & 1 == 0 { lo[k] } else { hi[k] }));
                for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                    if let (Some(p), Some(q)) = (proj(coin(a)), proj(coin(b))) {
                        painter.extend(egui::Shape::dashed_line(&[p, q], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(230, 230, 230, 170)), 5.0, 4.0));
                    }
                }
            }
        }
        #[cfg(test)]
        PEINTS.with(|v| v.borrow_mut().push((par_la_camera, objets.len(), if par_la_camera { 0 } else { d.vus().filter(|o| o.cache_dans(plan)).count() })));
        if !objets.is_empty() {
            match self.gpu.as_ref() {
                Some(g) => {
                    let qualite = if d.aspect.ombres { crate::decor3d::qualite(crate::decor3d::triangles_de(&objets)) } else { crate::decor3dbudget::Qualite::Sans };
                    let cb = crate::decor3dgpu::VueDecor {
                        cle,
                        // (Partie 199 : l'aspect de la vue, un réglage du poste, pas celui des plans.)
                        // (Partie 200 : et ses lampes.)
                        // (Partie 202 : et son cyclo.)
                        dessins: crate::decor3d::dessins_gpu(&objets, crate::decor3drendu::Aspect { rendu: self.settings.aspect_vue_3d, lampes: crate::miseenscene3d::lampes_vues(d, &m::IDENT), cyclo: crate::decor3d::cyclo_vu(d), ..d.aspect }),
                        vue: crate::decor3dgpu::Vue::de_view(view, center),
                        taille_pts: [draw.width(), draw.height()],
                        cible_egui: g.render_state().target_format,
                        qualite,
                        // (Les ombres des cartes, partie 189 : seulement quand ce décor a les siennes.)
                        cartes: self.espace3d.ombres.as_ref().map(|o| o.cartes.clone()),
                        // (Partie 202 : le ciel du cyclo : toujours par « Caméra du plan », dans une perspective si « Cyclo » est allumé,
                        // jamais dans une vue orthographique.)
                        ciel: d.cyclo.is_some() && d.aspect.ciel && view.ortho.is_none() && (par_la_camera || self.o3.montrer_cyclo),
                    };
                    painter.add(eframe::egui_wgpu::Callback::new_paint_callback(draw, cb));
                    sur_la_carte = true;
                }
                None => {
                    // Sans carte graphique : les boîtes des objets, en fil de fer.
                    for o in d.vus().filter(|o| !par_la_camera || !o.cache_dans(plan)) {
                        boite(o, Stroke::new(1.0_f32, Color32::from_rgb(150, 170, 200)));
                    }
                    painter.text(draw.left_bottom() + egui::vec2(8.0, -22.0), Align2::LEFT_BOTTOM, format!("Décor 3D ({} objets) : leurs boîtes (sans carte graphique, la vue ne les dessine pas)", objets.len()), FontId::proportional(11.0), Color32::from_gray(170));
                }
            }
        }
        // Les objets choisis dans la vue (partie 184) : leur boîte à la couleur de la sélection, et leur origine. L'objet choisi
        // dans la liste du panneau, s'il ne l'est pas dans la vue : sa boîte en clair, comme avant.
        let choisis = self.e3_objets_choisis();
        for o in d.objets.iter().filter(|o| choisis.contains(&o.id)) {
            boite(o, Stroke::new(2.0_f32, crate::theme::POIGNEE_SELECTION));
            if let Some(p) = proj(o.place.position) {
                crate::origine::peindre_repere(painter, p, crate::theme::POIGNEE_SELECTION, false);
            }
        }
        if let Some(o) = self.decor3d.choisi.filter(|id| !choisis.contains(id)).and_then(|id| d.objet(id)).filter(|_| d.objets.len() > 1) {
            boite(o, Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 220, 120, 170)));
        }
        sur_la_carte
    }

    /// Le point du sol (x, z) sous `pos` dans la vue Espace 3D (une seule vue, ouverte) : là où un modèle lâché se pose.
    pub(crate) fn decor3d_sol_sous(&self, pos: egui::Pos2) -> Option<[f32; 2]> {
        if !self.espace3d_vue_ouverte() || self.e3_vues_multiples() || !self.espace3d.rect.contains(pos) {
            return None;
        }
        let rect = self.espace3d.rect;
        let draw = if self.espace3d.through_camera { crate::espace3dview::letterbox(rect, self.doc.frame) } else { rect };
        let view = self.e3_view(draw);
        let ray = view.ray([pos.x - draw.left(), pos.y - draw.top()], [draw.width() / 2.0, draw.height() / 2.0]);
        let sol = Monde::of(&self.doc).sol_y();
        let t = m::ray_plane(&ray, [0.0, sol, 0.0], [0.0, 1.0, 0.0]).filter(|t| *t > 0.0 && t.is_finite())?;
        let p = ray.at(t);
        // (Au ras de l'horizon, le point fuirait à des kilomètres : pas plus loin que 200 m.)
        (p[0].abs() < 200.0 && p[2].abs() < 200.0).then_some([p[0], p[2]])
    }

    // ------------------------------------------------------------------------------- le panneau Espace 3D

    /// Termine la retouche en cours (une étape d'annulation pour toute la retouche).
    pub(crate) fn decor3d_retouche_finir(&mut self) {
        if let Some(snap) = self.decor3d.retouche.take() {
            let before = self.history.context.replace("Retoucher le décor 3D".into());
            self.commit_structure(snap);
            self.history.context = before;
        }
    }

    /// Applique une retouche du décor de la séquence `seq` (champs du panneau) : les objets aimantés se reposent, l'étape
    /// d'annulation viendra au lâcher (`decor3d_retouche_finir`).
    pub(crate) fn decor3d_retoucher(&mut self, seq: u64, d: Decor3d) {
        if self.decor3d.retouche.is_none() {
            self.decor3d.retouche = Some(self.snapshot());
        }
        self.decor3d_appliquer(seq, d);
    }

    /// Le cœur d'une retouche, sans instantané : le décor `d` de la séquence `seq` remplace l'ancien, ses objets aimantés se
    /// reposent, les cartes aimantées de la séquence aussi. L'étape d'annulation est à l'appelant : la retouche du panneau
    /// (`decor3d_retoucher`), ou le geste des poignées de la vue Espace 3D, qui a déjà le sien (partie 184, `poignees3d.rs`).
    pub(crate) fn decor3d_appliquer(&mut self, seq: u64, mut d: Decor3d) {
        // (Partie 200 : un objet animé de la séquence courante retouché suit la règle des caméras de plan, `miseenscene3d::retoucher` ;
        // l'aimant ne change pas ses clés.)
        let courante = !self.doc.panels.is_empty() && self.doc.sequence_of(self.cur_panel) == seq;
        let retouches = if courante { crate::miseenscene3dui::places_retouchees(self.doc.decors3d.par_sequence.get(&seq), &d) } else { Vec::new() };
        let notes = crate::decor3daimant::aimanter(&mut d, Monde::of(&self.doc).sol_y());
        if !notes.is_empty() {
            self.status = notes.join(" ");
        }
        if courante && d.anime() {
            crate::miseenscene3dui::reporter_les_cles(&mut d, &retouches, self.image_de_sequence_courante());
        }
        self.doc.decors3d.par_sequence.insert(seq, d);
        self.reposer_les_cartes_aimantees(seq);
        self.modified = true;
    }

    /// Le décor de la séquence dans le panneau Espace 3D : son aspect, la liste de ses objets (choisir, renommer, dupliquer,
    /// enlever, en ajouter), et les champs de l'objet choisi (position de son origine, rotation, échelle, origine, aimanté au
    /// sol).
    pub(crate) fn decor3d_reglages(&mut self, ui: &mut egui::Ui) {
        self.decor3d_reglages_de(ui, PartiesDecor::TOUT);
    }

    /// Partie 198 : `decor3d_reglages`, seulement les parties `p` (les onglets Lumières, Monde, Décor et Objet de Propriétés, dans
    /// l'onglet 3D) ; toutes : le décor tel que le panneau Espace 3D le montre, dans son en-tête repliable. La même retouche,
    /// la même étape d'annulation, quelle que soit la partie.
    pub(crate) fn decor3d_reglages_de(&mut self, ui: &mut egui::Ui, p: PartiesDecor) {
        let seq = self.doc.sequence_of(self.cur_panel);
        let Some(mut d) = self.doc.decor_du_plan(self.cur_panel).cloned() else {
            ui.add_space(6.0);
            if p.decor {
                if ui.button("Ajouter un objet 3D au décor de la séquence…").on_hover_text("Un modèle glTF (.glb, .gltf) que tous les plans de la séquence montrent, derrière les calques. Ou glisse un modèle de la Bibliothèque, onglet 3D.").clicked() {
                    self.run_action("fichier.importer_decor3d");
                }
            } else {
                ui.label(RichText::new(SANS_DECOR).small().weak());
            }
            return;
        };
        ui.add_space(6.0);
        let avant = d.clone();
        let budget = self.decor3d_budget().cloned();
        let choisi = self.decor3d.choisi.filter(|id| d.objet(*id).is_some()).or_else(|| d.objets.last().map(|o| o.id));
        let mut nouveau_choix = choisi;
        let (mut enlever, mut dupliquer, mut ajouter) = (None, None, false);
        // Copier, couper, coller, copier ou envoyer vers une autre séquence (partie 190, `copiedecor.rs`) : les objets choisis dans
        // la vue Espace 3D, sinon celui de la liste.
        let choisis_vue = self.e3_objets_choisis();
        let vises: Vec<u64> = if choisis_vue.is_empty() { choisi.into_iter().collect() } else { choisis_vue.clone() };
        let en_presse = self.copie3d.presse.as_ref().map(|p| p.objets.len());
        let autres: Vec<(u64, String)> = self
            .doc
            .sequence_order()
            .into_iter()
            .filter(|&q| q != seq)
            .map(|q| {
                let n = self.doc.decors3d.par_sequence.get(&q).map_or(0, |d| d.objets.len());
                let nom = self.doc.sequence_label(q);
                (q, if n == 0 { nom } else { format!("{nom} ({n} objet{} 3D)", if n > 1 { "s" } else { "" }) })
            })
            .collect();
        let mut geste: Option<GestePresse> = None;
        let (mut clic_simple, mut bascule, mut prise) = (None, None, false);
        let monde = Monde::of(&self.doc);
        let poids = d.poids();
        let titre = format!("Décor 3D de la séquence ({} objet{})", d.objets.len(), if d.objets.len() > 1 { "s" } else { "" });
        let tout = p == PartiesDecor::TOUT;
        let mut contenu = |ui: &mut egui::Ui| {
            // L'aspect : commun à tous les objets (une lumière, un ciel).
            if p.soleil {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Soleil");
                    // (Partie 202 : avec un cyclo, le soleil est le sien : son heure et sa rotation, dans Propriétés > Monde.)
                    if d.cyclo.is_some() {
                        ui.label(egui::RichText::new("suit le cyclo : son heure et sa rotation (Monde)").weak());
                    } else {
                        ui.add(egui::DragValue::new(&mut d.aspect.soleil_azimut).speed(1.0).suffix("° autour"));
                        ui.add(egui::DragValue::new(&mut d.aspect.soleil_hauteur).speed(0.5).range(5.0..=89.0).suffix("° haut"));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                if p.monde {
                    ui.checkbox(&mut d.aspect.facettes, "Facettes");
                }
                if p.ciel || p.monde {
                    ui.checkbox(&mut d.aspect.ciel, "Ciel");
                }
                if p.monde {
                    ui.checkbox(&mut d.aspect.brume, "Brume");
                }
                if p.decor {
                    ui.checkbox(&mut d.aspect.ombres, "Ombres");
                }
            });
            if !p.decor && !p.objet {
                return;
            }
            if p.decor && !tout {
                // Partie 199 : « Dans les plans », l'aspect du décor dans les images des plans (le dessin, la planche, le montage,
                // les vignettes, l'export) : ses matières, les couleurs d'identification, l'argile.
                ui.horizontal_wrapped(|ui| {
                    ui.label("Dans les plans");
                    for r in crate::couleurs3d::Rendu::TOUS {
                        let resp = ui.selectable_label(d.aspect.rendu == r, r.nom());
                        if crate::bulles::bulle(resp, &format!("Dans les plans : {}", r.nom()), r.aide(), "").clicked() {
                            d.aspect.rendu = r;
                        }
                    }
                });
            }
            if p.decor {
                if let Some(b) = &budget {
                    let q = if d.aspect.ombres { b.qualite_pour(poids.triangles) } else { crate::decor3dbudget::Qualite::Sans };
                    ui.label(RichText::new(format!("{} triangles en tout · sur cet ordinateur : {} (jusqu'à {})", grand_nombre(poids.triangles), q.nom(), grand_nombre(b.triangles))).small().weak());
                    if !tout {
                        // (Partie 198 : les bords doux de la partie 193, selon le budget de la machine.)
                        let doux = crate::decor3d::bords_doux(poids.triangles, q);
                        ui.label(RichText::new(if doux { "Bords doux entre les cartes et le décor : oui (le budget les permet)." } else { "Bords doux entre les cartes et le décor : non (le budget de cet ordinateur ne les permet pas, ou pas encore mesuré)." }).small().weak());
                    }
                } else if !tout {
                    ui.label(RichText::new(format!("{} triangles en tout · cet ordinateur n'est pas encore mesuré.", grand_nombre(poids.triangles))).small().weak());
                }
            }
            ui.separator();
            if p.decor {
            // La liste des objets.
            egui::ScrollArea::vertical().id_salt("decor3d_objets").max_height(140.0).auto_shrink([false, true]).show(ui, |ui| {
                for o in &d.objets {
                    let masque = crate::decor3d::masque(o.modele.uid);
                    let texte = if masque { format!("{} (masqué)", o.nom) } else { o.nom.clone() };
                    let r = ui.selectable_label(Some(o.id) == choisi || choisis_vue.contains(&o.id), texte).on_hover_text(format!("{} · {} triangles · Ctrl ou Maj + clic : en choisir plusieurs (pour les copier ensemble)", o.source, grand_nombre(o.modele.scene.triangles() as u64)));
                    if r.clicked() {
                        let m = ui.input(|i| i.modifiers);
                        if m.shift || m.command || m.ctrl {
                            bascule = Some(o.id);
                        } else {
                            nouveau_choix = Some(o.id);
                            clic_simple = Some(o.id);
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Ajouter…").on_hover_text("Un fichier glTF (.glb, .gltf) de plus dans le décor. Les modèles de la Bibliothèque (onglet 3D) se glissent sur le dessin ou sur cette vue.").clicked() {
                    ajouter = true;
                }
                if let Some(id) = choisi {
                    if ui.button("Dupliquer").on_hover_text("Une copie de l'objet choisi, posée à côté (le même modèle : rien de plus à lire ni à envoyer à la carte).").clicked() {
                        dupliquer = Some(id);
                    }
                    if ui.button("Enlever").on_hover_text("Enlève l'objet choisi du décor (Ctrl+Z le rend).").clicked() {
                        enlever = Some(id);
                    }
                }
            });
            // Copier, couper, coller (partie 190) : les mêmes que Ctrl+C, Ctrl+X, Ctrl+V après un clic dans la vue ou dans ce décor.
            ui.horizontal_wrapped(|ui| {
                let n = vises.len();
                let lesquels = if n > 1 { format!("les {n} objets choisis") } else { "l'objet choisi".to_string() };
                if ui.add_enabled(n > 0, egui::Button::new("Copier")).on_hover_text(format!("Copie {lesquels} (Ctrl+C) : Ctrl+V les colle dans le décor de la séquence du plan courant, ici, dans une autre séquence ou dans un autre projet ouvert.")).clicked() {
                    geste = Some(GestePresse::Copier);
                }
                if ui.add_enabled(n > 0, egui::Button::new("Couper")).on_hover_text(format!("Copie {lesquels} et l'enlève du décor (Ctrl+X) ; Ctrl+V le colle ailleurs, à la même place.")).clicked() {
                    geste = Some(GestePresse::Couper);
                }
                let coller = match en_presse {
                    Some(k) if k > 1 => format!("Coller ({k} objets)"),
                    _ => "Coller".to_string(),
                };
                if ui.add_enabled(en_presse.is_some(), egui::Button::new(coller)).on_hover_text("Colle les objets 3D copiés dans ce décor (Ctrl+V) : à côté s'ils y sont encore, sinon à la même place que là d'où ils viennent.").clicked() {
                    geste = Some(GestePresse::Coller);
                }
            });
            ui.horizontal_wrapped(|ui| {
                let possible = !vises.is_empty() && !autres.is_empty();
                let pourquoi = if autres.is_empty() { "Le projet n'a pas d'autre séquence." } else { "Choisis d'abord un objet du décor." };
                for (deplacer, titre, bulle) in [
                    (false, "Copier vers la séquence…", "Une copie des objets choisis dans le décor d'une autre séquence du projet, à la même place (ceux-ci restent ici)."),
                    (true, "Envoyer vers la séquence…", "Les objets choisis quittent ce décor pour celui d'une autre séquence du projet, à la même place."),
                ] {
                    ui.add_enabled_ui(possible, |ui| {
                        ui.menu_button(titre, |ui| {
                            for (q, nom) in &autres {
                                if ui.button(nom).clicked() {
                                    geste = Some(GestePresse::Vers(*q, deplacer));
                                    ui.close_menu();
                                }
                            }
                        })
                        .response
                        .on_hover_text(bulle)
                        .on_disabled_hover_text(pourquoi);
                    });
                }
            });
            }
            // (Un appui dans ce décor lui donne la main : Ctrl+C, Ctrl+X, Ctrl+V visent ses objets.)
            if ui.ui_contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
                prise = true;
            }
            if !p.objet {
                return;
            }
            let Some(o) = choisi.and_then(|id| d.objet_mut(id)) else { return };
            if p.decor {
                ui.separator();
            }
            ui.horizontal(|ui| {
                ui.label("Nom");
                ui.add(egui::TextEdit::singleline(&mut o.nom).desired_width(f32::INFINITY));
            });
            if !o.source.is_empty() {
                ui.label(RichText::new(format!("Fichier d'origine : {}", o.source)).small().weak());
            }
            if !o.allege.is_empty() {
                ui.label(RichText::new(format!("Allégé : {}", o.allege.join(", "))).small().weak());
            }
            let aimante = o.aimante;
            egui::Grid::new("decor3d_place").num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                ui.label("Position (m)").on_hover_text("Où est l'origine de l'objet dans l'espace : x, y (hauteur), z.");
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut o.place.position[0]).speed(0.02).max_decimals(3));
                    ui.add_enabled(!aimante, egui::DragValue::new(&mut o.place.position[1]).speed(0.02).max_decimals(3)).on_disabled_hover_text("Aimanté au sol : la hauteur est calculée (décoche « Aimanté au sol » pour la régler).");
                    if aimante {
                        ui.label(RichText::new("aimanté").small().weak());
                    }
                    ui.add(egui::DragValue::new(&mut o.place.position[2]).speed(0.02).max_decimals(3));
                });
                ui.end_row();
                ui.label("Rotation (°)");
                ui.add(egui::DragValue::new(&mut o.place.rotation_y).speed(0.5).suffix("°")).on_hover_text("Autour de la verticale, autour de l'origine.");
                ui.end_row();
                ui.label("Échelle");
                ui.add(egui::DragValue::new(&mut o.place.echelle).speed(0.005).range(0.001..=1000.0)).on_hover_text("Autour de l'origine.");
                ui.end_row();
                ui.label("Origine").on_hover_text("Le point de l'objet que sa position place, autour duquel il tourne et change d'échelle : x, y, z dans le repère du modèle. La changer ne bouge pas l'objet.");
                ui.horizontal(|ui| {
                    let mut p = o.origine;
                    for v in p.iter_mut() {
                        ui.add(egui::DragValue::new(v).speed(0.01).max_decimals(3));
                    }
                    if p != o.origine {
                        o.changer_origine(p);
                    }
                    let (lo, hi) = o.modele.bornes;
                    let centre = [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0);
                    ui.menu_button(ph::CROSSHAIR, |ui| {
                        let choix: [(&str, [f32; 3]); 6] = [
                            ("Au sol (centre du dessous)", crate::decor3d::origine_au_sol((lo, hi))),
                            ("Au centre de la boîte", centre),
                            ("Coin bas, gauche, arrière", [lo[0], lo[1], lo[2]]),
                            ("Coin bas, droite, arrière", [hi[0], lo[1], lo[2]]),
                            ("Coin bas, gauche, devant", [lo[0], lo[1], hi[2]]),
                            ("Coin bas, droite, devant", [hi[0], lo[1], hi[2]]),
                        ];
                        for (nom, q) in choix {
                            if ui.button(nom).clicked() {
                                o.changer_origine(q);
                                ui.close_menu();
                            }
                        }
                        if ui.button("Le point (0, 0, 0) du fichier").clicked() {
                            o.changer_origine([0.0; 3]);
                            ui.close_menu();
                        }
                    })
                    .response
                    .on_hover_text("Origine : au sol (centre), au centre, à un coin… (l'objet ne bouge pas).");
                });
                ui.end_row();
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut o.aimante, "Aimanté au sol").on_hover_text("Sa hauteur est calculée : il repose sur le sol, ou sur le dessus des objets sous lui, sans y entrer. Décoché, il garde sa hauteur.");
                if ui.button("Recentrer l'origine").on_hover_text("Ramène l'origine au milieu de l'objet en largeur et en profondeur, à sa hauteur actuelle (il tourne alors sur lui-même) ; l'objet ne bouge pas.").clicked() {
                    let (lo, hi) = o.modele.bornes;
                    o.changer_origine([(lo[0] + hi[0]) / 2.0, o.origine[1], (lo[2] + hi[2]) / 2.0]);
                }
                if ui.button("Remettre au sol").on_hover_text("Rotation et échelle d'origine, l'origine au centre du dessous, posé sur le sol au même endroit.").clicked() {
                    let x = o.place.position[0];
                    let z = o.place.position[2];
                    o.origine = crate::decor3d::origine_au_sol(o.modele.bornes);
                    o.place.rotation_y = 0.0;
                    o.place.echelle = 1.0;
                    o.poser_au_sol(x, z, monde.sol_y());
                }
            });
        };
        if tout {
            egui::CollapsingHeader::new(RichText::new(titre).strong()).id_salt("decor3d_reglages").default_open(true).show(ui, |ui| contenu(ui));
        } else {
            contenu(ui);
        }
        self.decor3d.choisi = nouveau_choix;
        // Un objet choisi dans la liste l'est aussi dans la vue Espace 3D (partie 184) : sa boîte s'y éclaire, les poignées y
        // vont. (Et un objet choisi dans la vue l'est dans la liste : `e3_choisir_objet`.) Un clic seul en fait le seul choisi ;
        // Ctrl ou Maj + clic l'ajoute à ceux choisis dans la vue, ou l'en retire (partie 190).
        if let Some(id) = clic_simple.or(nouveau_choix.filter(|_| nouveau_choix != choisi)) {
            self.e3_choisir_objet(id, false);
        }
        if let Some(id) = bascule {
            // (Le premier Ctrl + clic part de l'objet de la liste, s'il n'était pas encore choisi dans la vue.)
            if let Some(c) = choisi.filter(|c| *c != id && choisis_vue.is_empty()) {
                self.e3_choisir_objet(c, false);
            }
            self.e3_choisir_objet(id, true);
            if !self.e3_objets_choisis().contains(&id) {
                self.decor3d.choisi = self.e3_objets_choisis().last().copied().or(self.decor3d.choisi.filter(|c| *c != id));
            }
        }
        if prise || clic_simple.is_some() || bascule.is_some() || geste.is_some() {
            self.decor3d_prendre_la_main();
        }
        if let Some(g) = geste {
            self.decor3d_retouche_finir();
            match g {
                GestePresse::Copier => {
                    self.decor3d_copier(&vises);
                }
                GestePresse::Couper => self.decor3d_couper(&vises),
                GestePresse::Coller => self.decor3d_coller(),
                GestePresse::Vers(q, deplacer) => self.decor3d_vers_sequence(&vises, q, deplacer),
            }
            return;
        }
        if ajouter {
            self.decor3d_retouche_finir();
            self.run_action("fichier.importer_decor3d");
            return;
        }
        if let Some(id) = enlever {
            self.decor3d_enlever_objet(id);
            return;
        }
        if let Some(id) = dupliquer {
            self.decor3d_dupliquer_objet(id);
            return;
        }
        let change = d.aspect != avant.aspect || d.objets.len() != avant.objets.len() || d.objets.iter().zip(&avant.objets).any(|(a, b)| a.place != b.place || a.origine != b.origine || a.aimante != b.aimante || a.nom != b.nom);
        if change {
            self.decor3d_retoucher(seq, d);
        }
        // Le geste fini (rien n'est tenu, aucun champ n'est en saisie) : une étape d'annulation pour toute la retouche.
        let tenu = ui.input(|i| i.pointer.any_down()) || ui.ctx().memory(|mem| mem.focused().is_some());
        if !tenu {
            self.decor3d_retouche_finir();
        }
    }

    /// Préférences > Performance : le budget 3D de cette machine et « Remesurer ».
    pub(crate) fn decor3d_prefs_ui(&mut self, ui: &mut egui::Ui) {
        let mut remesurer = false;
        ui.horizontal_wrapped(|ui| {
            let texte = match self.decor3d_budget() {
                Some(b) => format!("Décor 3D sur cet ordinateur : {} (mesuré le {}).", b.texte(), date_lisible(b.date)),
                None if crate::decor3dbudget::en_cours() => "Décor 3D sur cet ordinateur : mesure en cours…".to_string(),
                None => "Décor 3D sur cet ordinateur : pas encore mesuré (au premier décor importé ou ouvert).".to_string(),
            };
            ui.label(texte);
            let r = ui.add_enabled(!crate::decor3dbudget::en_cours(), egui::Button::new("Remesurer"));
            remesurer = r.on_hover_text("Mesure de nouveau ce que la carte graphique et le processeur de cet ordinateur dessinent sans ralentir (quelques secondes, en arrière-plan).").clicked();
        });
        if remesurer {
            self.decor3d_remesurer();
        }
    }
}

impl Lourd {
    fn options(&self, budget: Option<&BudgetMachine>) -> Options {
        let t = self.analyse.mesures.triangles;
        let triangles = self.simplifier.then(|| match self.cible {
            Cible::Moitie => t / 2,
            Cible::Quart => t / 4,
            Cible::Budget => budget.map_or(t / 2, |b| (b.triangles.saturating_sub(self.deja.triangles) as usize).clamp(1, t.max(1))),
        });
        Options { cote_max: self.alleger.then_some(self.cote), triangles }
    }
}

/// Les mesures d'un décor déjà chargé (ouvert avec le projet).
pub(crate) fn mesures_de(m: &Modele3d) -> crate::decor3dgltf::Mesures {
    let s = &m.scene;
    crate::decor3dgltf::Mesures {
        octets_fichier: m.octets.len() as u64,
        triangles: s.triangles(),
        sommets: s.sommets(),
        matieres: s.primitives.len(),
        textures: s.textures.len(),
        octets_textures: s.octets_textures(),
        plus_grand_cote: s.textures.iter().map(|t| t.largeur.max(t.hauteur)).max().unwrap_or(0),
        ..Default::default()
    }
}

// Partie 200, pour les essais : ce que chaque vue Espace 3D a peint du décor à la dernière image : « Caméra du plan » ou non, les objets
// dessinés, ceux en pointillés (cachés dans le plan).
#[cfg(test)]
thread_local! {
    pub(crate) static PEINTS: std::cell::RefCell<Vec<(bool, usize, usize)>> = const { std::cell::RefCell::new(Vec::new()) };
}

// Partie 198.5, pour les essais : le temps d'une image que la lecture d'un modèle « mesure » (les essais n'ont pas de carte
// graphique) : une première image trop lente, par exemple.
#[cfg(test)]
thread_local! {
    pub(crate) static COUT_ESSAI: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Le temps d'une image du décor sur la carte, à la taille de la feuille, posé à sa place d'origine (millisecondes).
fn mesurer_cout(rs: &eframe::egui_wgpu::RenderState, scene: &Arc<crate::decor3dgltf::Scene3d>, feuille: [usize; 2], cadre: [usize; 2]) -> Option<f32> {
    let max = rs.device.limits().max_texture_dimension_2d as usize;
    let feuille = [feuille[0].min(max), feuille[1].min(max)];
    let monde = Monde::new(feuille, cadre);
    let prise = crate::decor3drendu::Prise::de_la_feuille(&monde, monde.eye_reference())?;
    let mut mt = crate::decor3dgpu::Moteur::new(&rs.device, &rs.queue);
    let d = [crate::decor3dgpu::Dessin { id: 1, scene: scene.clone(), matrice: Place3d::d_origine(scene, &monde).matrice(), aspect: Default::default(), teinte: Default::default() }];
    crate::decor3dgpu::rendre_hors_ecran(&rs.device, &rs.queue, &mut mt, &d, &prise, [feuille[0] as u32, feuille[1] as u32], true, crate::decor3dbudget::Qualite::Sans, 4).map(|r| r.1)
}

/// « 19/11/2026 ».
fn date_lisible(secondes: u64) -> String {
    let jours = (secondes / 86_400) as i64;
    // (Calendrier civil depuis le 1er janvier 1970 : jours depuis l'époque vers année, mois, jour.)
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + ere * 400 + i64::from(mo <= 2);
    format!("{d:02}/{mo:02}/{y}")
}

/// Partie 198 : les parties du réglage du décor (`App::decor3d_reglages_de`) : le soleil, le ciel, le monde (facettes, ciel, brume),
/// le décor lui-même (son poids, les ombres, la liste de ses objets et ce qu'on en fait), l'objet choisi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PartiesDecor {
    pub soleil: bool,
    pub ciel: bool,
    pub monde: bool,
    pub decor: bool,
    pub objet: bool,
}

impl PartiesDecor {
    pub(crate) const TOUT: PartiesDecor = PartiesDecor { soleil: true, ciel: true, monde: true, decor: true, objet: true };
    pub(crate) const RIEN: PartiesDecor = PartiesDecor { soleil: false, ciel: false, monde: false, decor: false, objet: false };
}

/// La phrase d'un onglet de l'onglet 3D quand la séquence n'a pas de décor 3D.
pub(crate) const SANS_DECOR: &str = "La séquence n'a pas encore de décor 3D : l'onglet Décor en ajoute un, ou un clic sur un modèle de la Banque 3D, en bas.";
