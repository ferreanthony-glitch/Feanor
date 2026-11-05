//! « Enregistrer sous » vers un projet qui existe déjà : remplacer sans rien perdre.
//!
//! Avant, SHOT refusait d'écrire sur un projet existant, mais le refus n'était dit que dans la barre d'état : une
//! boîte « Modifications non enregistrées » (fermer, quitter) disparaissait comme si tout s'était bien passé
//! (Anthony, 2026-10-28 : une soirée de dessins perdue). Maintenant :
//! - le choix du fichier se **décide** à part de la fenêtre de fichiers de Windows (`decider_enregistrer_sous`, qui reçoit
//!   le chemin choisi : c'est ce qu'on teste, `rfd` ne tourne pas dans les essais) ;
//! - un projet qui existe déjà ouvre une vraie fenêtre : **Remplacer**, **Choisir un autre nom**, **Annuler** ;
//! - « Remplacer » ne supprime RIEN : le dossier de l'ancien projet est renommé à côté (« Mon film (remplacé le
//!   2026-10-28 22h45) »), puis le projet en cours s'écrit à sa place, puis l'action en attente (fermer, quitter…) suit ;
//! - un renommage impossible (dossier tenu par un autre programme, droits) dit pourquoi dans la fenêtre d'erreur, et
//!   rien n'est écrit ni fermé ; un projet ouvert dans un autre onglet n'est jamais remplacé.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::app::{App, Pending};
use crate::project;

/// Ce que « Enregistrer sous » a trouvé à l'endroit choisi.
pub(crate) enum Decision {
    /// Rien n'y est : on écrit.
    Ecrire,
    /// Un projet y est déjà : la fenêtre demande.
    Remplacer(Remplacement),
    /// On n'écrit pas là, et la fenêtre d'erreur dit pourquoi.
    Refuser(String),
}

/// La question « Un projet existe déjà ici » en attente de réponse.
#[derive(Clone)]
pub(crate) struct Remplacement {
    /// Où le projet en cours s'écrira.
    pub manifest: PathBuf,
    /// Le dossier de l'ancien projet (celui qui sera renommé).
    pub dossier: PathBuf,
    /// Le nom de l'ancien projet.
    pub nom: String,
    /// Son dernier enregistrement (secondes depuis 1970), s'il est connu.
    pub date: Option<u64>,
    /// Le nom que prendra son dossier : « Mon film (remplacé le 2026-10-28 22h45) ».
    pub nouveau_nom: String,
    /// L'action qui attendait l'enregistrement (fermer, quitter…).
    pub then: Option<Pending>,
}

/// Le choix fait dans la fenêtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChoixRemplacement {
    Remplacer,
    AutreNom,
    Annuler,
}

// ------------------------------------------------------------------ l'heure d'ici

/// Jours depuis le 1er janvier 1970 d'une date civile (calendrier grégorien).
fn jours_depuis_1970(y: i64, m: u32, d: u32) -> i64 {
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// L'écart de l'heure d'ici avec l'heure universelle, en secondes (arrondi au quart d'heure). Zéro hors Windows.
fn decalage_local() -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct HeureSysteme {
            annee: u16,
            mois: u16,
            jour_semaine: u16,
            jour: u16,
            heure: u16,
            minute: u16,
            seconde: u16,
            milliseconde: u16,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(t: *mut HeureSysteme);
        }
        let mut t = HeureSysteme { annee: 0, mois: 0, jour_semaine: 0, jour: 0, heure: 0, minute: 0, seconde: 0, milliseconde: 0 };
        // SAFETY: `GetLocalTime` remplit la structure qu'on lui donne ; rien d'autre n'est touché.
        unsafe { GetLocalTime(&mut t) };
        let local = jours_depuis_1970(i64::from(t.annee), u32::from(t.mois), u32::from(t.jour)) * 86_400 + i64::from(t.heure) * 3600 + i64::from(t.minute) * 60 + i64::from(t.seconde);
        let diff = local - crate::recovery::now_secs() as i64;
        ((diff as f64 / 900.0).round() as i64) * 900
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// (année, mois, jour, heure, minute) à l'heure d'ici pour des secondes depuis 1970.
pub(crate) fn heure_locale(secs: u64) -> (i64, u32, u32, u32, u32) {
    let t = secs as i64 + decalage_local();
    let (y, m, d) = crate::projetui::civil_from_days(t.div_euclid(86_400));
    let s = t.rem_euclid(86_400);
    (y, m, d, (s / 3600) as u32, (s % 3600 / 60) as u32)
}

/// « 26/10/2026 à 21:03 », à l'heure d'ici ; « date inconnue » pour une date qu'on n'a pas.
pub(crate) fn date_locale(secs: Option<u64>) -> String {
    match secs {
        Some(s) if s > 0 => {
            let (y, m, d, h, mi) = heure_locale(s);
            format!("{d:02}/{m:02}/{y} à {h:02}:{mi:02}")
        }
        _ => "date inconnue".into(),
    }
}

/// Le nom que prend le dossier d'un projet remplacé : « Mon film (remplacé le 2026-10-28 22h45) ».
pub(crate) fn nom_de_remplacement(nom: &str, maintenant: u64) -> String {
    let (y, m, d, h, mi) = heure_locale(maintenant);
    format!("{nom} (remplacé le {y}-{m:02}-{d:02} {h:02}h{mi:02})")
}

/// `chemin` est-il `dir` ou dedans ? (Windows ne distingue pas les majuscules.)
fn est_dans(chemin: &Path, dir: &Path) -> bool {
    let mots = |p: &Path| p.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect::<Vec<_>>();
    let (a, b) = (mots(chemin), mots(dir));
    a.len() >= b.len() && a[..b.len()] == b[..]
}

/// La date de dernière modification d'un fichier ou d'un dossier, en secondes depuis 1970.
fn modifie_le(p: &Path) -> Option<u64> {
    std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Renomme `dossier` à côté de lui : « Nom (remplacé le … ) », et « … ) 2 », « … ) 3 » si ce nom est pris. Rien n'est
/// effacé. Rend le nouveau chemin ; une erreur dit pourquoi (dossier tenu par un autre programme, droits…).
pub(crate) fn mettre_de_cote(dossier: &Path, nouveau_nom: &str) -> Result<PathBuf, String> {
    let parent = dossier.parent().ok_or_else(|| format!("{} n'a pas de dossier parent", dossier.display()))?;
    let mut cible = parent.join(nouveau_nom);
    let mut k = 1;
    while cible.exists() {
        k += 1;
        cible = parent.join(format!("{nouveau_nom} {k}"));
        if k > 999 {
            return Err(format!("trop de dossiers « {nouveau_nom} » dans {}", parent.display()));
        }
    }
    std::fs::rename(dossier, &cible).map_err(|e| format!("le dossier {} n'a pas pu être renommé en « {} » : {e}", dossier.display(), cible.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()))?;
    Ok(cible)
}

impl App {
    /// Que faire d'un fichier choisi dans « Enregistrer sous » ? (Sans fenêtre de fichiers : `chosen` est le chemin choisi.)
    /// Lit le disque ; n'écrit rien.
    pub(crate) fn decider_enregistrer_sous(&self, chosen: &Path) -> Decision {
        let (dir, manifest) = project::target_for(chosen);
        // (Le projet ouvert sous son ancien nom, `Bus/Bus.pelure`, enregistré sous `Bus/Bus.shot` : c'est lui, pas un autre.)
        let courant = self.path.as_deref().map(|p| if self.partage.is_none() { project::manifeste_shot(p) } else { p.to_path_buf() });
        if project::check_target(&manifest, courant.as_deref()).is_ok() {
            return Decision::Ecrire;
        }
        let nom = manifest.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Le projet est ouvert ici même (ou dans le même dossier) : le renommer casserait ce qu'on tient.
        if let Some(p) = self.path.as_deref() {
            if p.parent().is_some_and(|d| est_dans(d, &dir)) {
                return Decision::Refuser(format!("« {nom} » est le projet ouvert dans cet onglet (ou il le contient) : on ne peut pas le remplacer par lui-même. Choisis un autre nom."));
            }
        }
        // Ouvert dans un autre onglet de SHOT : jamais remplacé.
        let meme_dossier = |chemin: &Path| chemin.parent().is_some_and(|d| est_dans(d, &dir));
        for (i, info) in self.infos_onglets().into_iter().enumerate() {
            if i != self.actif && info.chemin.as_deref().is_some_and(meme_dossier) {
                return Decision::Refuser(format!("« {nom} » est ouvert dans un autre onglet de FEANOR : ferme cet onglet avant de le remplacer, ou choisis un autre nom."));
            }
        }
        let date = [Some(manifest.clone()), project::manifeste_jumeau(&manifest)]
            .into_iter()
            .flatten()
            .filter_map(|m| modifie_le(&m))
            .max()
            .or_else(|| modifie_le(&dir));
        let dossier_nom = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| nom.clone());
        Decision::Remplacer(Remplacement {
            manifest,
            dossier: dir,
            nom,
            date,
            nouveau_nom: nom_de_remplacement(&dossier_nom, crate::recovery::now_secs()),
            then: None,
        })
    }

    /// Le chemin choisi dans « Enregistrer sous » : écrit, ou ouvre la fenêtre qui demande, ou dit pourquoi non. Rend vrai si
    /// l'écriture est lancée.
    pub(crate) fn enregistrer_sous_vers(&mut self, chosen: &Path, then: Option<Pending>) -> bool {
        match self.decider_enregistrer_sous(chosen) {
            Decision::Ecrire => {
                let (_, manifest) = project::target_for(chosen);
                self.start_save(manifest, then)
            }
            Decision::Remplacer(mut r) => {
                r.then = then;
                self.status = format!("Un projet existe déjà à cet endroit : « {} ». Remplacer, choisir un autre nom ou annuler ?", r.nom);
                self.remplacement = Some(r);
                false
            }
            Decision::Refuser(raison) => {
                self.status = format!("Enregistrement annulé : {raison}");
                self.save_error = Some(raison);
                false
            }
        }
    }

    /// La suite d'un enregistrement qui n'a rien lancé sans rien dire (la fenêtre de fichiers a été fermée, ou le
    /// remplacement annulé) : la question « Modifications non enregistrées » revient, au lieu de disparaître.
    pub(crate) fn rendre_la_question(&mut self, then: Option<Pending>) {
        if let Some(action) = then {
            if self.pending.is_none() && self.save_error.is_none() && self.remplacement.is_none() {
                self.pending = Some(action);
            }
        }
    }

    /// Le choix fait dans la fenêtre « Un projet existe déjà ici ».
    pub(crate) fn remplacement_choisir(&mut self, choix: ChoixRemplacement) {
        let Some(r) = self.remplacement.take() else { return };
        match choix {
            ChoixRemplacement::Annuler => {
                self.status = "Remplacement annulé : rien n'a été écrit, le projet reste ouvert et modifié.".into();
                self.rendre_la_question(r.then);
            }
            ChoixRemplacement::AutreNom => {
                // (La fenêtre d'enregistrement se rouvre ; si on la ferme sans choisir, la question revient.)
                if !self.save_as_then(r.then) {
                    self.rendre_la_question(r.then);
                }
            }
            ChoixRemplacement::Remplacer => self.remplacer_et_enregistrer(r),
        }
    }

    /// « Remplacer » : le dossier de l'ancien projet est renommé à côté (rien n'est effacé), puis le projet en cours s'écrit à
    /// sa place, puis l'action en attente suit. Si le renommage échoue, la fenêtre d'erreur dit pourquoi et rien n'est écrit.
    fn remplacer_et_enregistrer(&mut self, r: Remplacement) {
        // (Un dernier regard : depuis la question, l'état du disque ou des onglets a pu changer.)
        if let Decision::Refuser(raison) = self.decider_enregistrer_sous(&r.manifest) {
            self.status = format!("Remplacement annulé : {raison}");
            self.save_error = Some(raison);
            return;
        }
        // (Le dossier a disparu depuis la question : rien à mettre de côté.)
        let deplace = r.dossier.exists();
        let de_cote = match if deplace { mettre_de_cote(&r.dossier, &r.nouveau_nom) } else { Ok(r.dossier.clone()) } {
            Ok(p) => p,
            Err(e) => {
                let texte = format!("Impossible de remplacer « {} » : {e}. Il est peut-être ouvert dans un autre programme. Rien n'a été écrit ni effacé.", r.nom);
                self.status = format!("Remplacement impossible : {e}");
                self.save_error = Some(texte);
                return;
            }
        };
        let nom_de_cote = de_cote.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !self.start_save(r.manifest.clone(), r.then) {
            // L'écriture n'a pas pu partir (la fenêtre d'erreur le dit) : l'ancien projet reprend sa place.
            let rendu = !deplace || (!r.dossier.exists() && std::fs::rename(&de_cote, &r.dossier).is_ok());
            if !rendu {
                let texte = format!("{} L'ancien projet « {} » est gardé tel quel dans le dossier « {nom_de_cote} ».", self.save_error.clone().unwrap_or_default(), r.nom);
                self.save_error = Some(texte);
            }
            return;
        }
        self.status = format!("« {} » est remplacé. Son ancien dossier est gardé tel quel à côté : « {nom_de_cote} ».", r.nom);
    }

    /// La fenêtre « Un projet existe déjà ici » : Remplacer, Choisir un autre nom, Annuler.
    pub(crate) fn remplacement_fenetre(&mut self, ctx: &egui::Context) {
        let Some(r) = self.remplacement.clone() else { return };
        let mut choix = None;
        egui::Modal::new(egui::Id::new("remplacement")).show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.heading("Un projet existe déjà ici");
            ui.add_space(4.0);
            ui.label(format!("Un projet existe déjà ici : « {} » (dernier enregistrement : {}).", r.nom, date_locale(r.date)));
            ui.add_space(4.0);
            ui.label(format!(
                "« Remplacer » ne supprime rien : l'ancien dossier est gardé à côté sous le nom « {} », et ton projet en cours est enregistré à sa place.",
                r.nouveau_nom
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Remplacer").clicked() {
                    choix = Some(ChoixRemplacement::Remplacer);
                }
                if ui.button("Choisir un autre nom").clicked() {
                    choix = Some(ChoixRemplacement::AutreNom);
                }
                if ui.button("Annuler").clicked() {
                    choix = Some(ChoixRemplacement::Annuler);
                }
            });
        });
        if let Some(c) = choix {
            self.remplacement_choisir(c);
        }
    }
}
