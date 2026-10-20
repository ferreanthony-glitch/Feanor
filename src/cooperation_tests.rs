//! Travailler à plusieurs (`docs/COOPERATION.md`) : deux postes, un dossier, une horloge qu'on avance.
//!
//! Les essais font tourner le moteur « à la main » (`Session::pomper`), sans fil, avec une horloge réglée : tout est
//! déterministe. Les dossiers synchronisés (Dropbox, Drive) sont imités par `Synchro`, qui recopie les fichiers d'un poste
//! à l'autre quand l'essai le décide, dans l'ordre qu'il choisit, et fabrique les « copies en conflit ».

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui::Color32;

use crate::cooperation::{Delais, Effets, Horloge, Refus, Session};
use crate::depot::{Depot, DossierPartage};
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::equipe::Identite;
use crate::fiabilite_fixture as fx;
use crate::project::partage_io::{self, ResultatPartage, Signature};
use crate::project::{self, Loaded};
use crate::verrous::{Cible, EtatPrise};

// ------------------------------------------------------------------------------------------ outils

/// Un petit film de `n` plans (dont le deuxième commence une deuxième séquence), chacun avec trois dessins.
pub(crate) fn film(n: usize) -> Document {
    let mut doc = Document::with_frame(48, 27, 24);
    for i in 1..n {
        if i == 2 {
            doc.add_sequence_after(i - 1);
        } else {
            doc.add_scene_after(i - 1);
        }
    }
    for i in 0..doc.panels.len() {
        let mut layer = Layer::new("Calque", LayerKind::Bitmap);
        for k in 0..3 {
            let mut cel = Cel::new(doc.width, doc.height, LayerKind::Bitmap);
            cel.raster.tile_mut(0)[k] = Color32::from_rgba_premultiplied(10 + (i * 20) as u8, 10 + k as u8, 10, 255);
            layer.frames.push(Some(doc.add_cel(cel)));
        }
        doc.panels[i].layers = vec![layer];
        doc.panels[i].length = 3;
    }
    doc
}

fn dossier(nom: &str) -> PathBuf {
    fx::dossier(&format!("coop-{nom}"))
}

pub(crate) fn signature(nom: &str) -> Signature {
    Signature { id: nom.to_lowercase(), nom: nom.into(), le: 1_000 }
}

/// Partage `doc` dans un dossier neuf de `racine` : le fichier de structure.
pub(crate) fn partager(doc: &Document, racine: &Path) -> PathBuf {
    let manifeste = racine.join("Film").join("Film.pelure");
    partage_io::partager(doc, &manifeste, &signature("Anthony"), "p-essai", None, 2).expect("partage");
    manifeste
}

/// Les durées du protocole, celles de tous les jours.
fn delais() -> Delais {
    Delais::default()
}

/// Un poste : un document ouvert depuis un dossier partagé, et sa session.
pub(crate) struct Poste {
    pub doc: Document,
    pub s: Session,
    ecrits: HashMap<CelId, u64>,
    pub manifeste: PathBuf,
    pub horloge: Horloge,
    /// Tout ce que `sonder` a dit depuis le début.
    pub messages: Vec<String>,
    pub perdues: Vec<Cible>,
}

impl Poste {
    pub(crate) fn ouvrir(manifeste: &Path, nom: &str, horloge: &Horloge) -> Poste {
        Poste::ouvrir_seance(manifeste, nom, horloge, "s1")
    }

    /// Une fenêtre de plus du même équipier : une autre séance.
    pub(crate) fn ouvrir_seance(manifeste: &Path, nom: &str, horloge: &Horloge, seance: &str) -> Poste {
        let Loaded { mut doc, partage, written, .. } = project::load(manifeste).expect("projet partagé lisible");
        let o = partage.expect("un projet partagé");
        let moi = Identite { id: nom.to_lowercase(), nom: nom.into(), couleur: [200, 40, 40], machine: format!("poste-{nom}"), seance: seance.into() };
        let depot = Arc::new(DossierPartage::nouveau(manifeste));
        let s = Session::rejoindre(depot, moi, o, &mut doc, delais(), horloge.clone(), false, None).expect("session");
        Poste { doc, s, ecrits: written, manifeste: manifeste.to_path_buf(), horloge: horloge.clone(), messages: Vec::new(), perdues: Vec::new() }
    }

    /// Un tour du moteur, puis ce qu'il a préparé est posé dans le document.
    pub(crate) fn tour(&mut self) -> Effets {
        self.s.pomper();
        let fx = self.s.sonder(&mut self.doc);
        self.messages.extend(fx.messages.iter().cloned());
        self.perdues.extend(fx.prises_perdues.iter().copied());
        fx
    }

    pub(crate) fn enregistrer(&mut self) -> ResultatPartage {
        let ctx = self.s.contexte_ecriture(&self.doc);
        let job = partage_io::preparer(&self.doc, &self.manifeste, &mut self.ecrits, ctx).expect("préparation");
        let rapport = job.run_with(None, 2).expect("écriture");
        let r = rapport.partage.expect("rapport de partage");
        self.s.enregistrement_fini(&r, &self.doc);
        r
    }

    pub(crate) fn prendre_plan(&mut self, i: usize) -> Result<(), Refus> {
        let id = self.doc.panels[i].id;
        self.s.prendre(&[Cible::Plan(id)], false).remove(0).1
    }

    pub(crate) fn plan_id(&self, i: usize) -> u64 {
        self.doc.panels[i].id
    }

    /// « Dessiner » dans le plan `i` : change un pixel du premier dessin et sa version.
    pub(crate) fn dessiner(&mut self, i: usize, rouge: u8) {
        let id = self.doc.panels[i].layers[0].frames[0].expect("dessin");
        let cel = self.doc.cels.get_mut(&id).expect("dessin");
        cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(rouge, 1, 2, 255);
        cel.version += 1;
    }

    pub(crate) fn rouge(&self, i: usize) -> u8 {
        let id = self.doc.panels[i].layers[0].frames[0].expect("dessin");
        self.doc.cels[&id].raster.get(0, 0).r()
    }

    pub(crate) fn etat(&self, i: usize) -> EtatPrise {
        self.s.vue().etat(Cible::Plan(self.plan_id(i))).clone()
    }
}

/// Deux postes devant un même dossier (immédiat, comme un partage réseau), une horloge commune.
pub(crate) fn deux_postes(nom: &str, n: usize) -> (Poste, Poste, Horloge, PathBuf) {
    let racine = dossier(nom);
    let manifeste = partager(&film(n), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let a = Poste::ouvrir(&manifeste, "Anthony", &h);
    let b = Poste::ouvrir(&manifeste, "Lea", &h);
    (a, b, h, racine)
}

/// Quelques tours pour que chacun lise ce que l'autre a écrit.
pub(crate) fn tours(a: &mut Poste, b: &mut Poste) {
    for _ in 0..2 {
        a.tour();
        b.tour();
    }
}

/// Le temps qu'une prise soit confirmée (§5.1).
pub(crate) fn confirmer(h: &Horloge) {
    h.avancer(9_000);
}

/// Les brouillons du dossier partagé.
fn brouillons(manifeste: &Path) -> Vec<String> {
    DossierPartage::nouveau(manifeste).lister_brouillons().expect("brouillons")
}

fn chemin_plan(racine: &Path, plan: u64) -> PathBuf {
    racine.join("Film").join("plans").join(format!("{plan}.json"))
}

// ------------------------------------------------------------------------------------------ le format

#[test]
fn a_complete_project_shared_and_reopened_is_the_same_project() {
    let (f, classique) = fx::projet_enregistre("coop-complet");
    let relu = project::load(&classique).expect("projet classique");
    let attendu = fx::instantane(&relu.doc, &f.sans_pixels);
    let racine = dossier("complet-partage");
    let manifeste = racine.join("Film").join("Film.pelure");
    partage_io::partager(&relu.doc, &manifeste, &signature("Anthony"), "p-essai", None, 2).expect("partage");
    let ouvert = project::load(&manifeste).expect("projet partagé lisible");
    assert!(ouvert.warnings.is_empty(), "aucun avertissement : {:?}", ouvert.warnings);
    assert!(ouvert.partage.is_some());
    let obtenu = fx::instantane(&ouvert.doc, &f.sans_pixels);
    fx::meme_projet(&attendu, &obtenu).expect("le projet partagé est le même projet");
}

#[test]
fn sharing_leaves_the_original_untouched_and_a_classic_project_keeps_its_format() {
    let (_, classique) = fx::projet_enregistre("coop-original");
    let avant = fx::arbre(fx::racine(&classique));
    let relu = project::load(&classique).expect("projet");
    let racine = dossier("original-partage");
    partage_io::partager(&relu.doc, &racine.join("Film").join("Film.pelure"), &signature("Anthony"), "p-essai", None, 2).expect("partage");
    assert_eq!(avant, fx::arbre(fx::racine(&classique)), "l'original n'a pas bougé d'un octet");
    // Un projet classique s'écrit au format 33, sans bloc de partage.
    let manifeste = fx::racine(&classique).join("Film.pelure");
    let v = fx::lire_json(&manifeste);
    assert_eq!(v["version"], 33);
    assert!(v.get("partage").is_none());
    assert!(project::load(&manifeste).expect("classique").partage.is_none());
}

#[test]
fn the_shared_manifest_is_format_34_with_no_plan_inside_and_one_file_per_plan() {
    let racine = dossier("disposition");
    let manifeste = partager(&film(4), &racine);
    let v = fx::lire_json(&manifeste);
    assert_eq!(v["version"], 34);
    assert_eq!(v["partage"]["plans"].as_array().map(Vec::len), Some(4));
    for seq in v["sequences"].as_array().expect("séquences") {
        for sc in seq["scenes"].as_array().expect("scènes") {
            assert_eq!(sc["panneaux"].as_array().map(Vec::len), Some(0), "la structure ne porte aucun plan");
        }
    }
    assert_eq!(crate::depot::nombre_de_plans(fx::racine(&manifeste)), 4);
    // Un SHOT sans partage refuse la structure plutôt que de montrer un film sans plans : le format est plus récent.
    assert!(v["version"].as_u64().unwrap_or(0) > u64::from(project::VERSION));
}

#[test]
fn numbers_of_two_members_never_meet() {
    let (mut a, mut b, _, _) = deux_postes("numeros", 3);
    assert_ne!(a.s.prefixe, b.s.prefixe, "chaque équipier a son préfixe");
    let mut vus = std::collections::HashSet::new();
    for p in [&mut a, &mut b] {
        for _ in 0..50 {
            assert!(vus.insert(p.doc.new_cel(LayerKind::Bitmap)), "deux équipiers n'ont jamais donné le même numéro de dessin");
        }
    }
    // Ces numéros restent dans ce qu'un JSON sait écrire sans perte.
    assert!(vus.iter().all(|id| *id < (1 << 53)));
    // Les numéros d'avant le partage sont gardés tels quels.
    assert!(a.doc.panels.iter().all(|p| p.id < (1 << 32)));
    // Le préfixe se retrouve : le même équipier rouvre avec le même.
    let p = a.s.prefixe;
    let autre = Poste::ouvrir(&a.manifeste, "Anthony", &a.horloge);
    assert_eq!(autre.s.prefixe, p);
}

#[test]
fn a_plan_file_that_is_missing_or_unreadable_is_a_shell_that_cannot_be_taken_or_saved() {
    let racine = dossier("coquille");
    let manifeste = partager(&film(3), &racine);
    let doc = project::load(&manifeste).expect("projet").doc;
    let (casse, absent) = (doc.panels[0].id, doc.panels[1].id);
    std::fs::write(chemin_plan(&racine, casse), b"{ pas du json").expect("abîmer");
    std::fs::remove_file(chemin_plan(&racine, absent)).expect("retirer");
    let avant = std::fs::read(chemin_plan(&racine, casse)).expect("lecture");
    let h = Horloge::reglable(1_760_000_000_000);
    let mut a = Poste::ouvrir(&manifeste, "Anthony", &h);
    assert_eq!(a.doc.panels.len(), 3, "le film a ses trois plans, deux sont des coquilles");
    for id in [casse, absent] {
        assert!(matches!(a.s.peut_modifier_plan(id), Err(Refus::EnAttente(_))), "plan en attente");
        assert!(matches!(a.s.prendre(&[Cible::Plan(id)], false).remove(0).1, Err(Refus::EnAttente(_))));
    }
    // Il prend le troisième, enregistre : les fichiers des coquilles ne sont pas touchés (ni créé ni écrasé).
    a.prendre_plan(2).expect("le troisième est sain");
    confirmer(&h);
    a.tour();
    a.dessiner(2, 99);
    a.enregistrer();
    assert_eq!(std::fs::read(chemin_plan(&racine, casse)).expect("lecture"), avant);
    assert!(!chemin_plan(&racine, absent).exists(), "un plan absent n'est pas fabriqué à partir d'une coquille");
}

#[test]
fn a_plan_file_from_a_newer_version_is_refused_with_a_clear_message_and_left_alone() {
    let racine = dossier("plan-futur");
    let manifeste = partager(&film(2), &racine);
    let doc = project::load(&manifeste).expect("projet").doc;
    let id = doc.panels[1].id;
    let mut v = fx::lire_json(&chemin_plan(&racine, id));
    v["version"] = serde_json::json!(99);
    fx::ecrire_json(&chemin_plan(&racine, id), &v);
    let avant = std::fs::read(chemin_plan(&racine, id)).expect("lecture");
    let h = Horloge::reglable(1_760_000_000_000);
    let a = Poste::ouvrir(&manifeste, "Anthony", &h);
    let Err(Refus::EnAttente(raison)) = a.s.peut_modifier_plan(id) else { panic!("le plan d'une version plus récente doit être en attente") };
    assert!(raison.contains("plus récente"), "{raison}");
    assert_eq!(std::fs::read(chemin_plan(&racine, id)).expect("lecture"), avant);
}

// ------------------------------------------------------------------------------------------ prendre

#[test]
fn a_plan_is_taken_by_one_person_at_a_time() {
    let (mut a, mut b, _, _) = deux_postes("prise", 3);
    assert_eq!(a.etat(0), EtatPrise::Libre);
    assert!(a.prendre_plan(0).is_ok());
    tours(&mut a, &mut b);
    assert!(matches!(a.etat(0), EtatPrise::Moi { .. }));
    match b.etat(0) {
        EtatPrise::Autre(r) => assert_eq!(r.equipier.nom, "Anthony"),
        e => panic!("Léa devrait voir le plan à Anthony : {e:?}"),
    }
    assert_eq!(b.prendre_plan(0), Err(Refus::PrisPar("Anthony".into())));
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(0)), Err(Refus::PrisPar("Anthony".into())));
    // Un autre plan est libre pour Léa.
    assert!(b.prendre_plan(1).is_ok());
    tours(&mut a, &mut b);
    assert_eq!(a.s.peut_modifier_plan(a.plan_id(1)), Err(Refus::PrisPar("Lea".into())));
    // Anthony rend : Léa peut prendre.
    a.s.rendre(&[Cible::Plan(a.plan_id(0))]);
    tours(&mut a, &mut b);
    assert_eq!(b.etat(0), EtatPrise::Libre);
    assert!(b.prendre_plan(0).is_ok());
}

#[test]
fn a_plan_one_has_taken_can_be_changed_and_one_has_not_cannot() {
    let (mut a, _, _, _) = deux_postes("modifier", 2);
    let id = a.plan_id(0);
    assert_eq!(a.s.peut_modifier_plan(id), Err(Refus::Libre));
    a.prendre_plan(0).expect("libre");
    assert_eq!(a.s.peut_modifier_plan(id), Ok(()), "ma prise est vérifiée tout de suite : rien à attendre");
    a.s.rendre(&[Cible::Plan(id)]);
    assert_eq!(a.s.peut_modifier_plan(id), Err(Refus::Libre));
}

#[test]
fn taking_a_whole_sequence_takes_every_free_plan_and_leaves_the_ones_others_hold() {
    let (mut a, mut b, _, _) = deux_postes("sequence", 4);
    // Les plans 2 et 3 sont la deuxième séquence (voir `film`) ; Léa en tient un.
    let seq = a.doc.sequence_ids();
    assert_eq!(seq[2], seq[3]);
    b.prendre_plan(3).expect("libre");
    tours(&mut a, &mut b);
    let plans: Vec<Cible> = (0..a.doc.panels.len()).filter(|&i| a.doc.sequence_ids()[i] == seq[2]).map(|i| Cible::Plan(a.plan_id(i))).collect();
    let r = a.s.prendre(&plans, false);
    assert!(r[0].1.is_ok(), "le plan libre est pris");
    assert_eq!(r[1].1, Err(Refus::PrisPar("Lea".into())), "celui de Léa est laissé, avec son nom");
}

// ------------------------------------------------------------------------------------------ la course et la perte

#[test]
fn two_people_taking_the_same_plan_before_seeing_each_other_get_the_same_winner() {
    let (mut a, mut b, h, _) = deux_postes("course", 3);
    // Anthony prend le plan 1, puis (une seconde plus tard) Léa : aucun des deux n'a encore lu la prise de l'autre.
    a.prendre_plan(1).expect("libre");
    h.avancer(1_000);
    b.prendre_plan(1).expect("libre pour elle aussi : elle ne voit pas encore la prise d'Anthony");
    assert!(b.s.tiens(Cible::Plan(b.plan_id(1))));
    // Léa dessine pendant que sa prise est provisoire.
    b.dessiner(1, 99);
    a.tour();
    b.tour();
    a.tour();
    // Le gagnant est le même partout : la plus ancienne prise.
    assert!(matches!(a.etat(1), EtatPrise::Moi { .. }), "Anthony a pris le premier : {:?}", a.etat(1));
    b.tour();
    assert!(matches!(b.etat(1), EtatPrise::Autre(ref r) if r.equipier.nom == "Anthony"), "{:?}", b.etat(1));
    assert_eq!(b.perdues, vec![Cible::Plan(b.plan_id(1))], "Léa est prévenue");
    assert!(b.messages.iter().any(|m| m.contains("Anthony")), "le message dit qui était là avant : {:?}", b.messages);
    assert!(!b.s.tiens(Cible::Plan(b.plan_id(1))));
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(1)), Err(Refus::PrisPar("Anthony".into())));
    // Son travail n'est pas perdu : un brouillon l'attend dans le dossier (écrit à son prochain enregistrement).
    b.enregistrer();
    let liste = brouillons(&b.manifeste);
    assert_eq!(liste.len(), 1, "un brouillon : {liste:?}");
    assert!(liste[0].starts_with(&format!("plan-{}.lea.", b.plan_id(1))));
    // Le plan de Léa reprend celui d'Anthony (le dessin 99 n'y est plus), le brouillon, lui, le garde.
    b.tour();
    b.tour();
    assert_ne!(b.rouge(1), 99, "le plan reprend le contenu du porteur");
    let octets = b.s.lire_brouillon(&liste[0]).expect("brouillon");
    let tete = project::Tete::de(&b.doc);
    let pc = partage_io::charger_plan(fx::racine(&b.manifeste), &tete, b.plan_id(1), &octets, false).expect("le brouillon est un plan");
    let id = pc.panel.layers[0].frames[0].expect("dessin");
    assert_eq!(pc.cels[&id].raster.get(0, 0).r(), 99, "le dessin de Léa est dans le brouillon");
    // Le dessin du porteur, lui, n'a pas été écrasé : son fichier est resté tel qu'il était.
    let dessin_commun = a.doc.panels[1].layers[0].frames[0].expect("dessin");
    assert_ne!(id, dessin_commun, "le brouillon cite un dessin neuf, pas celui du plan");
}

#[test]
fn a_draft_becomes_a_version_of_the_plan_for_its_holder() {
    let (mut a, mut b, h, _) = deux_postes("brouillon-version", 3);
    a.prendre_plan(1).expect("libre");
    h.avancer(1_000);
    b.prendre_plan(1).expect("libre aussi");
    b.dessiner(1, 99);
    a.tour();
    b.tour();
    b.enregistrer();
    let nom = brouillons(&b.manifeste).remove(0);
    // Anthony, le porteur, importe le brouillon de Léa comme version du plan.
    confirmer(&h);
    a.tour();
    let octets = a.s.lire_brouillon(&nom).expect("brouillon");
    let plan = a.plan_id(1);
    let numero = a.s.importer_comme_version(&mut a.doc, plan, &octets, "conflit de Lea").expect("importé");
    assert_eq!(numero, 2, "V2 : la version active est la V1");
    let v = &a.doc.versions[&plan];
    assert_eq!(v.autres.len(), 1);
    assert!(v.autres[0].panel.notes.starts_with("[conflit de Lea]"), "la version dit d'où elle vient");
    let id = v.autres[0].panel.layers[0].frames[0].expect("dessin");
    assert_eq!(a.doc.cels[&id].raster.get(0, 0).r(), 99);
    assert_ne!(Some(id), a.doc.panels[1].layers[0].frames[0], "les dessins d'une version sont à elle");
    // Il l'enregistre : la version est dans le fichier du plan, et Léa la voit après son rafraîchissement.
    a.dessiner(1, 55);
    a.enregistrer();
    tours(&mut a, &mut b);
    assert_eq!(b.doc.versions[&plan].autres.len(), 1);
    assert_eq!(b.rouge(1), 55);
    // Un plan sans version n'a pas d'entrée de versions.
    assert!(!b.doc.versions.contains_key(&b.plan_id(0)));
}

#[test]
fn a_lease_that_runs_out_can_be_taken_over_and_the_old_holder_loses_for_good() {
    let (mut a, mut b, h, _) = deux_postes("bail", 3);
    a.prendre_plan(0).expect("libre");
    tours(&mut a, &mut b);
    a.dessiner(0, 77);
    // Anthony s'éloigne (plus de battement) : onze minutes passent, seule Léa travaille.
    h.avancer(11 * 60_000);
    b.tour();
    b.tour();
    let id = b.plan_id(0);
    assert!(matches!(b.etat(0), EtatPrise::Perimee(_)), "{:?}", b.etat(0));
    assert!(matches!(b.s.peut_modifier_plan(id), Err(Refus::Perimee { .. })));
    // Sans confirmation, rien.
    assert!(matches!(b.prendre_plan(0), Err(Refus::Perimee { .. })));
    assert_eq!(b.s.prendre(&[Cible::Plan(id)], true).remove(0).1, Ok(()), "avec confirmation, Léa reprend");
    tours(&mut b, &mut a);
    // Anthony revient : son bail se renouvelle, mais la prise est reprise pour toujours.
    h.avancer(31_000);
    a.tour();
    a.tour();
    assert_eq!(a.perdues, vec![Cible::Plan(id)], "Anthony est prévenu une fois");
    assert!(!a.s.tiens(Cible::Plan(id)));
    assert!(matches!(a.etat(0), EtatPrise::Autre(_)), "{:?}", a.etat(0));
    // Son dessin non enregistré est dans un brouillon.
    a.enregistrer();
    assert_eq!(brouillons(&a.manifeste).len(), 1);
    // Et il ne reprend pas la place de Léa en renouvelant son bail.
    for _ in 0..3 {
        h.avancer(31_000);
        a.tour();
        b.tour();
    }
    assert!(matches!(b.etat(0), EtatPrise::Moi { .. }));
    assert_eq!(a.perdues.len(), 1, "une seule perte signalée");
}

#[test]
fn a_holder_that_keeps_its_lease_alive_is_never_taken_over() {
    let (mut a, mut b, h, _) = deux_postes("bail-vivant", 3);
    a.prendre_plan(0).expect("libre");
    for _ in 0..40 {
        h.avancer(31_000);
        a.tour();
        b.tour();
    }
    // Plus de 20 minutes ont passé, mais Anthony bat toutes les 30 s.
    assert!(matches!(b.etat(0), EtatPrise::Autre(_)), "{:?}", b.etat(0));
}

#[test]
fn a_fiche_seen_for_the_first_time_after_a_long_silence_is_expired_but_one_seen_changing_is_alive() {
    let (mut a, mut b, h, _) = deux_postes("horloge-fausse", 3);
    a.prendre_plan(0).expect("libre");
    a.tour();
    b.tour();
    assert!(matches!(b.etat(0), EtatPrise::Autre(_)));
    // Une heure passe sans que la fiche d'Anthony change : périmée.
    h.avancer(3_600_000);
    b.tour();
    assert!(matches!(b.etat(0), EtatPrise::Perimee(_)), "{:?}", b.etat(0));
    // Mais dès qu'on la voit changer, elle est vivante : Anthony reprend le battement.
    h.avancer(31_000);
    a.tour();
    b.tour();
    assert!(matches!(b.etat(0), EtatPrise::Autre(_)), "{:?}", b.etat(0));
}

#[test]
fn a_session_that_died_leaves_prises_its_own_equipier_takes_back_without_a_question() {
    let (mut a, _, h, _) = deux_postes("reprise-plantage", 3);
    a.prendre_plan(0).expect("libre");
    a.tour();
    let manifeste = a.manifeste.clone();
    // SHOT plante (pas de `fermer`) : la session suivante d'Anthony, dix minutes plus tard.
    h.avancer(11 * 60_000);
    std::mem::forget(a.s);
    let mut a2 = Poste::ouvrir_seance(&manifeste, "Anthony", &h, "s2");
    a2.s.pomper();
    assert!(a2.s.tiens(Cible::Plan(a2.plan_id(0))), "il retrouve ses prises");
    // Une autre fenêtre du même équipier, pendant que la deuxième séance vit, ne lui prend pas ses plans.
    a2.tour();
    let mut a3 = Poste::ouvrir_seance(&manifeste, "Anthony", &h, "s3");
    a3.tour();
    assert!(!a3.s.tiens(Cible::Plan(a3.plan_id(0))));
    assert!(matches!(a3.etat(0), EtatPrise::Autre(_)), "{:?}", a3.etat(0));
}

// ------------------------------------------------------------------------------------------ enregistrer

#[test]
fn saving_writes_only_the_plans_one_holds_and_only_when_they_changed() {
    let (mut a, mut b, h, racine) = deux_postes("enregistrer", 3);
    a.prendre_plan(0).expect("libre");
    b.prendre_plan(1).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    // Rien n'a changé : rien n'est écrit.
    let r = a.enregistrer();
    assert!(r.plans_ecrits.is_empty(), "un plan inchangé n'est pas réécrit : {r:?}");
    assert_eq!(r.plans_inchanges.len(), 1);
    let (plan0, plan1) = (chemin_plan(&racine, a.plan_id(0)), chemin_plan(&racine, a.plan_id(1)));
    let (avant0, avant1) = (std::fs::read(&plan0).expect("plan 0"), std::fs::read(&plan1).expect("plan 1"));
    // Anthony dessine dans son plan ET (par erreur) dans celui de Léa : seul le sien s'écrit.
    a.dessiner(0, 100);
    a.dessiner(1, 101);
    let r = a.enregistrer();
    assert_eq!(r.plans_ecrits.len(), 1);
    assert_eq!(r.plans_ecrits[0].plan, a.plan_id(0));
    assert_ne!(std::fs::read(&plan0).expect("plan 0"), avant0, "un dessin modifié change le fichier du plan (son empreinte)");
    assert_eq!(std::fs::read(&plan1).expect("plan 1"), avant1, "le plan de Léa n'a pas été touché");
    // La révision avance d'un à chaque écriture.
    let f = fx::lire_json(&plan0);
    assert_eq!(f["revision"], 2, "1 à la création du partage, 2 à cet enregistrement");
    assert_eq!(f["enregistre_par"]["nom"], "Anthony");
    // Un deuxième enregistrement sans changement ne touche plus rien.
    let r = a.enregistrer();
    assert!(r.plans_ecrits.is_empty() && r.structure.is_none());
    // Les dessins de Léa ne sont pas écrits par Anthony.
    let dessin1 = a.doc.panels[1].layers[0].frames[0].expect("dessin");
    let fichier_dessin1 = racine.join("Film").join("dessins").join(format!("{dessin1}.png"));
    let octets = std::fs::read(&fichier_dessin1).expect("dessin de Léa");
    a.dessiner(1, 102);
    a.enregistrer();
    assert_eq!(std::fs::read(&fichier_dessin1).expect("dessin de Léa"), octets);
    // Aucun fichier temporaire ne reste.
    let restes: Vec<String> = fx::arbre(&racine).keys().filter(|k| k.ends_with(".tmp") || k.ends_with(".part")).cloned().collect();
    assert!(restes.is_empty(), "{restes:?}");
}

#[test]
fn a_prise_still_being_confirmed_is_not_written() {
    let (mut a, _, h, racine) = deux_postes("provisoire", 3);
    a.prendre_plan(0).expect("libre");
    a.dessiner(0, 100);
    let plan0 = chemin_plan(&racine, a.plan_id(0));
    let avant = std::fs::read(&plan0).expect("plan");
    // Trois secondes : la prise est provisoire, une prise plus ancienne chez l'autre peut encore arriver.
    h.avancer(3_000);
    let r = a.enregistrer();
    assert!(r.plans_ecrits.is_empty() && r.plans_inchanges.is_empty());
    assert_eq!(std::fs::read(&plan0).expect("plan"), avant);
    // Une fois confirmée, le dessin part.
    confirmer(&h);
    let r = a.enregistrer();
    assert_eq!(r.plans_ecrits.len(), 1);
}

#[test]
fn what_another_saved_arrives_at_the_next_refresh_and_never_overwrites_my_plan() {
    let (mut a, mut b, h, _) = deux_postes("rafraichir", 3);
    a.prendre_plan(0).expect("libre");
    b.prendre_plan(1).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    a.dessiner(0, 120);
    b.dessiner(1, 130);
    a.enregistrer();
    b.enregistrer();
    let fx_a = a.tour();
    assert!(fx_a.documents_changes, "le plan de Léa est arrivé");
    b.tour();
    assert_eq!(a.rouge(1), 130, "Anthony voit le dessin de Léa");
    assert_eq!(b.rouge(0), 120, "Léa voit le dessin d'Anthony");
    assert_eq!(a.rouge(0), 120, "Anthony garde son travail");
    assert_eq!(b.rouge(1), 130);
    // Les notifications le disent.
    assert!(a.s.notifications.iter().any(|n| n.texte.contains("Lea a enregistré")), "{:?}", a.s.notifications);
    // Un plan que je tiens et qu'un autre poste réécrirait n'est jamais remplacé sous mes mains.
    a.dessiner(0, 121);
    a.tour();
    assert_eq!(a.rouge(0), 121);
}

#[test]
fn a_change_in_a_plan_one_does_not_hold_is_put_aside_not_lost_when_the_real_plan_arrives() {
    let (mut a, mut b, h, _) = deux_postes("garde-fou", 3);
    b.prendre_plan(1).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    // Anthony retouche le plan de Léa sans le tenir (une barrière oubliée).
    a.dessiner(1, 200);
    // Léa enregistre : son plan arrive chez Anthony, qui avait retouché le sien.
    b.dessiner(1, 130);
    b.enregistrer();
    a.tour();
    assert_eq!(a.rouge(1), 130, "le plan de Léa remplace la retouche");
    a.enregistrer();
    assert_eq!(brouillons(&a.manifeste).len(), 1, "la retouche d'Anthony est mise de côté");
}

#[test]
fn a_released_plan_can_be_taken_over_by_the_other_with_its_last_content() {
    let (mut a, mut b, h, _) = deux_postes("passer-la-main", 3);
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    a.tour();
    a.dessiner(0, 150);
    a.enregistrer();
    a.s.rendre(&[Cible::Plan(a.plan_id(0))]);
    tours(&mut a, &mut b);
    assert_eq!(b.rouge(0), 150);
    assert!(b.prendre_plan(0).is_ok());
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(0)), Ok(()));
    // Léa continue à partir de ce qu'Anthony a laissé.
    confirmer(&h);
    b.tour();
    b.dessiner(0, 151);
    b.enregistrer();
    a.tour();
    a.tour();
    assert_eq!(a.rouge(0), 151);
}
