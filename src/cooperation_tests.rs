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
        let ctx = self.s.contexte_ecriture(&self.doc, false);
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
    // Un projet classique s'écrit au format classique (35 depuis l'Espace 3D), sans bloc de partage.
    let manifeste = fx::racine(&classique).join("Film.pelure");
    let v = fx::lire_json(&manifeste);
    assert_eq!(v["version"], project::VERSION);
    assert!(v.get("partage").is_none());
    assert!(project::load(&manifeste).expect("classique").partage.is_none());
}

#[test]
fn the_shared_manifest_is_the_newer_of_the_shared_and_classic_format_with_no_plan_inside_and_one_file_per_plan() {
    let racine = dossier("disposition");
    let manifeste = partager(&film(4), &racine);
    let v = fx::lire_json(&manifeste);
    // (Espace 3D : le format classique est passé à 35, un projet partagé écrit le plus grand des deux numéros.)
    assert_eq!(v["version"], project::version_partage_ecrite());
    assert!(project::version_partage_ecrite() >= project::VERSION_PARTAGE && project::version_partage_ecrite() >= project::VERSION);
    assert_eq!(v["partage"]["plans"].as_array().map(Vec::len), Some(4));
    for seq in v["sequences"].as_array().expect("séquences") {
        for sc in seq["scenes"].as_array().expect("scènes") {
            assert_eq!(sc["panneaux"].as_array().map(Vec::len), Some(0), "la structure ne porte aucun plan");
        }
    }
    assert_eq!(crate::depot::nombre_de_plans(fx::racine(&manifeste)), 4);
    // Un SHOT sans partage (format classique 33 au plus) refuse la structure plutôt que de montrer un film sans plans :
    // le format est plus récent.
    assert!(v["version"].as_u64().unwrap_or(0) > 33);
}

#[test]
fn numbers_of_two_members_never_meet() {
    let (mut a, mut b, _, _) = deux_postes("numeros", 3);
    assert_ne!(a.doc.id_prefix(), b.doc.id_prefix(), "chaque équipier a son préfixe");
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
    let p = a.doc.id_prefix();
    let autre = Poste::ouvrir_seance(&a.manifeste, "Anthony", &a.horloge, "s9");
    assert_eq!(autre.doc.id_prefix(), p);
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

// ------------------------------------------------------------------------------------------ le montage

#[test]
fn the_structure_token_is_held_by_one_person_and_the_montage_follows_to_the_other() {
    let (mut a, mut b, h, _) = deux_postes("jeton", 3);
    assert!(a.s.prendre(&[Cible::Montage], false).remove(0).1.is_ok());
    confirmer(&h);
    tours(&mut a, &mut b);
    assert_eq!(b.s.peut_modifier_montage(), Err(Refus::PrisPar("Anthony".into())));
    assert_eq!(a.s.peut_modifier_montage(), Ok(()));
    // Anthony nomme la séquence et crée un plan : ces plans neufs sont à lui dès leur naissance.
    let seq = a.doc.sequences[0].id;
    a.doc.sequence_mut(seq).expect("séquence").name = "Acte I".into();
    let neuf = a.doc.add_scene_after(1);
    let neuf_id = a.doc.panels[neuf].id;
    assert!(neuf_id >> 32 == a.doc.id_prefix() >> 32, "son numéro porte le préfixe d'Anthony");
    assert_eq!(a.s.apres_structure(&a.doc), vec![neuf_id]);
    assert!(a.s.tiens(Cible::Plan(neuf_id)));
    let r = a.enregistrer();
    assert!(r.structure.is_some(), "le montage a changé : la structure est écrite");
    assert!(r.plans_ecrits.iter().any(|p| p.plan == neuf_id), "et le fichier du plan neuf avec");
    tours(&mut a, &mut b);
    tours(&mut a, &mut b);
    assert_eq!(b.doc.sequences[0].name, "Acte I");
    assert_eq!(b.doc.panels.len(), 4, "le plan neuf est arrivé");
    assert_eq!(b.doc.panels[neuf].id, neuf_id);
    assert!(b.s.notifications.iter().any(|n| n.texte.contains("Anthony a modifié le montage")), "{:?}", b.s.notifications);
    // Léa ne peut ni prendre le jeton, ni modifier le plan neuf.
    assert_eq!(b.s.prendre(&[Cible::Montage], false).remove(0).1, Err(Refus::PrisPar("Anthony".into())));
    assert_eq!(b.s.peut_modifier_plan(neuf_id), Err(Refus::PrisPar("Anthony".into())));
    // Anthony rend le jeton, Léa le prend et déplace un plan : Anthony le voit.
    a.s.rendre(&[Cible::Montage]);
    tours(&mut a, &mut b);
    assert!(b.s.prendre(&[Cible::Montage], false).remove(0).1.is_ok());
    confirmer(&h);
    b.tour();
    let premier = b.doc.panels[0].id;
    b.doc.move_plans(&[premier], None).expect("déplacer");
    b.s.apres_structure(&b.doc);
    b.enregistrer();
    tours(&mut a, &mut b);
    tours(&mut a, &mut b);
    assert_eq!(a.doc.panels.last().map(|p| p.id), Some(premier), "le plan déplacé est à la fin chez Anthony aussi");
}

#[test]
fn a_plan_deleted_from_the_montage_goes_away_for_the_other_and_its_file_stays() {
    let (mut a, mut b, h, racine) = deux_postes("suppression", 3);
    a.s.prendre(&[Cible::Montage], false);
    a.prendre_plan(2).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    let id = a.plan_id(2);
    a.doc.remove_panels(&[id]).expect("supprimer");
    a.s.apres_structure(&a.doc);
    a.enregistrer();
    tours(&mut a, &mut b);
    tours(&mut a, &mut b);
    assert_eq!(b.doc.panels.len(), 2);
    assert!(b.doc.panels.iter().all(|p| p.id != id));
    assert!(chemin_plan(&racine, id).exists(), "rien n'est effacé du dossier partagé");
    assert!(!a.s.tiens(Cible::Plan(id)), "un plan supprimé n'est plus tenu");
}

// ------------------------------------------------------------------------------------------ les commentaires

#[test]
fn comments_of_two_people_merge_and_anyone_can_resolve_or_delete_any_comment() {
    let (mut a, mut b, h, _) = deux_postes("commentaires", 3);
    let (p0, p1) = (a.plan_id(0), a.plan_id(1));
    // On peut commenter n'importe quel plan, sans rien tenir.
    let c1 = a.doc.add_comment(p0, 1, "Anthony", 1);
    a.doc.comment_mut(c1).expect("commentaire").text = "plus de mouvement ici".into();
    let c2 = b.doc.add_comment(p1, 2, "Lea", 2);
    b.doc.comment_mut(c2).expect("commentaire").text = "trop sombre".into();
    a.enregistrer();
    h.avancer(10);
    b.enregistrer();
    tours(&mut a, &mut b);
    assert_eq!(a.doc.comments.len(), 2);
    assert_eq!(b.doc.comments.len(), 2);
    // Léa résout le commentaire d'Anthony, Anthony supprime celui de Léa.
    h.avancer(1_000);
    b.doc.comment_mut(c1).expect("commentaire d'Anthony").done = true;
    a.doc.remove_comment(c2);
    a.enregistrer();
    b.enregistrer();
    tours(&mut a, &mut b);
    assert!(a.doc.comment(c1).expect("encore là").done, "résolu par Léa");
    assert!(b.doc.comment(c1).expect("encore là").done);
    assert!(a.doc.comment(c2).is_none() && b.doc.comment(c2).is_none(), "supprimé par Anthony, chez Léa aussi");
    // Rien d'autre n'a été touché : les commentaires de chacun sont dans son fichier.
    let dossier_com = fx::racine(&a.manifeste).join("commentaires");
    assert!(dossier_com.join("anthony.json").exists() && dossier_com.join("lea.json").exists());
    // Un enregistrement sans changement ne réécrit pas le fichier.
    let avant = std::fs::read(dossier_com.join("anthony.json")).expect("fichier");
    let r = a.enregistrer();
    assert!(!r.commentaires_ecrits);
    assert_eq!(std::fs::read(dossier_com.join("anthony.json")).expect("fichier"), avant);
}

#[test]
fn comments_merge_the_same_way_whatever_the_order_files_arrive_in() {
    use crate::commentaires::Comment;
    use crate::partagecom::{fusionner, Enregistrement};
    let base = |id, texte: &str, done| Comment { id, panel: 5, frame: 0, text: texte.into(), author: "x".into(), color: [1, 2, 3], created: 0, done, note: None, annotation: None };
    let a = vec![
        Enregistrement { at: 10, supprime: false, commentaire: base(1, "v1", false) },
        Enregistrement { at: 30, supprime: false, commentaire: base(2, "de a", false) },
    ];
    let b = vec![
        Enregistrement { at: 20, supprime: false, commentaire: base(1, "v2 (b)", true) },
        Enregistrement { at: 30, supprime: true, commentaire: base(2, "de a", false) },
    ];
    let ab = fusionner(&[("a".into(), a.clone()), ("b".into(), b.clone())]);
    let ba = fusionner(&[("b".into(), b), ("a".into(), a)]);
    assert_eq!(ab, ba, "le résultat ne dépend pas de l'ordre");
    assert_eq!(ab[&1].text, "v2 (b)", "le plus récent gagne");
    assert!(ab.contains_key(&2), "à égalité de date, le plus petit équipier (« a ») l'emporte : sa version vivante");
}

// ------------------------------------------------------------------------------------------ les dossiers synchronisés

/// Deux dossiers « Dropbox » : chacun a sa copie, la synchronisation est faite à la main (et dans l'ordre qu'on veut).
struct Synchro {
    a: PathBuf,
    b: PathBuf,
    connu: HashMap<String, u64>,
}

#[derive(Clone, Copy)]
enum Sens {
    AversB,
    BversA,
}

fn lister(racine: &Path) -> std::collections::BTreeMap<String, u64> {
    fx::arbre(racine).into_iter().filter(|(k, _)| !k.ends_with(".tmp") && !k.ends_with(".part")).collect()
}

fn copier_dossier(de: &Path, vers: &Path) {
    for rel in lister(de).keys() {
        let (s, d) = (de.join(rel), vers.join(rel));
        std::fs::create_dir_all(d.parent().expect("parent")).expect("dossier");
        std::fs::copy(s, d).expect("copie");
    }
}

impl Synchro {
    /// `a` est complet ; `b` en est une copie (la première synchronisation est faite).
    fn nouveau(a: &Path, b: &Path) -> Synchro {
        copier_dossier(a, b);
        Synchro { a: a.to_path_buf(), b: b.to_path_buf(), connu: lister(a).into_iter().collect() }
    }

    /// Recopie ce qui a changé dans `de` vers `vers`, parmi les fichiers que `filtre` accepte. Si l'autre côté a aussi
    /// changé le même fichier, il le garde et la version qui arrive devient une « copie en conflit ».
    fn pousser(&mut self, sens: Sens, filtre: &dyn Fn(&str) -> bool) -> Vec<String> {
        let (de, vers) = match sens {
            Sens::AversB => (self.a.clone(), self.b.clone()),
            Sens::BversA => (self.b.clone(), self.a.clone()),
        };
        let (s, d) = (lister(&de), lister(&vers));
        let mut copies = Vec::new();
        for (rel, hs) in &s {
            if !filtre(rel) {
                continue;
            }
            let connu = self.connu.get(rel).copied();
            if connu == Some(*hs) {
                continue;
            }
            let hd = d.get(rel).copied();
            if hd == Some(*hs) {
                self.connu.insert(rel.clone(), *hs);
                continue;
            }
            let cible = if hd == connu {
                self.connu.insert(rel.clone(), *hs);
                vers.join(rel)
            } else {
                // Les deux côtés ont changé ce fichier : comme Dropbox, la copie qui arrive prend un autre nom.
                let (dossier, nom) = rel.rsplit_once('/').map_or(("", rel.as_str()), |(d, n)| (d, n));
                let (souche, ext) = nom.rsplit_once('.').unwrap_or((nom, ""));
                let nouveau = format!("{souche} (Copie en conflit de poste 2026-10-20).{ext}");
                vers.join(dossier).join(nouveau)
            };
            std::fs::create_dir_all(cible.parent().expect("parent")).expect("dossier");
            std::fs::copy(de.join(rel), &cible).expect("copie");
            copies.push(rel.clone());
        }
        copies
    }

    fn tout(&mut self, sens: Sens) -> Vec<String> {
        self.pousser(sens, &|_| true)
    }
}

/// Deux postes devant deux copies d'un dossier, synchronisées à la main.
fn deux_postes_synchro(nom: &str, n: usize) -> (Poste, Poste, Horloge, Synchro) {
    let racine = dossier(nom);
    let manifeste_a = partager(&film(n), &racine.join("A"));
    let manifeste_b = racine.join("B").join("Film").join("Film.pelure");
    let sync = Synchro::nouveau(manifeste_a.parent().expect("dossier"), manifeste_b.parent().expect("dossier"));
    let h = Horloge::reglable(1_760_000_000_000);
    let a = Poste::ouvrir(&manifeste_a, "Anthony", &h);
    let b = Poste::ouvrir(&manifeste_b, "Lea", &h);
    (a, b, h, sync)
}

#[test]
fn a_plan_file_that_arrives_before_its_drawings_is_waited_for_not_shown_half_done() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-dessins", 3);
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    a.tour();
    a.dessiner(0, 150);
    a.enregistrer();
    // Le fichier du plan arrive chez Léa, pas encore le dessin qu'il cite.
    sync.pousser(Sens::AversB, &|f| !f.starts_with("dessins/"));
    b.tour();
    h.avancer(4_000);
    b.tour();
    assert_ne!(b.rouge(0), 150, "le plan à moitié arrivé n'est pas montré");
    let info = b.s.vue().plans.get(&b.plan_id(0)).cloned().expect("info du plan");
    assert!(info.attente.is_some(), "le plan est en attente de ses dessins : {info:?}");
    assert!(matches!(b.s.peut_modifier_plan(b.plan_id(0)), Err(Refus::EnAttente(_))));
    // Le dessin arrive : au tour suivant, le plan complet est posé.
    sync.tout(Sens::AversB);
    h.avancer(10_000);
    b.tour();
    h.avancer(10_000);
    b.tour();
    assert_eq!(b.rouge(0), 150);
    assert!(b.s.vue().plans.get(&b.plan_id(0)).is_some_and(|i| i.attente.is_none()));
}

#[test]
fn the_structure_that_arrives_before_a_new_plan_shows_a_shell_then_the_plan() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-structure", 3);
    a.s.prendre(&[Cible::Montage], false);
    confirmer(&h);
    a.tour();
    let neuf = a.doc.add_scene_after(1);
    let neuf_id = a.doc.panels[neuf].id;
    a.s.apres_structure(&a.doc);
    a.enregistrer();
    // Seule la structure (et les fiches) arrive chez Léa : le fichier du plan neuf, pas encore.
    sync.pousser(Sens::AversB, &|f| !f.contains("plans/") && !f.contains("dessins/"));
    b.tour();
    assert_eq!(b.doc.panels.len(), 4, "le plan neuf est dans le film de Léa");
    assert_eq!(b.doc.panels[neuf].id, neuf_id);
    assert!(matches!(b.s.peut_modifier_plan(neuf_id), Err(Refus::EnAttente(_))), "mais seulement une coquille");
    assert!(matches!(b.s.prendre(&[Cible::Plan(neuf_id)], false).remove(0).1, Err(Refus::EnAttente(_))));
    // Le plan arrive.
    sync.tout(Sens::AversB);
    h.avancer(10_000);
    b.tour();
    h.avancer(10_000);
    b.tour();
    assert!(b.s.peut_modifier_plan(neuf_id).is_err(), "il est à Anthony");
    assert!(b.s.vue().plans.get(&neuf_id).is_some_and(|i| i.attente.is_none()), "{:?}", b.s.vue().plans.get(&neuf_id));
}

#[test]
fn a_plan_file_that_arrives_before_the_structure_waits_for_it() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-plan-avant", 3);
    a.s.prendre(&[Cible::Montage], false);
    confirmer(&h);
    a.tour();
    let neuf = a.doc.add_scene_after(1);
    let neuf_id = a.doc.panels[neuf].id;
    a.s.apres_structure(&a.doc);
    let mut cel = Cel::new(a.doc.width, a.doc.height, LayerKind::Bitmap);
    cel.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(33, 1, 2, 255);
    let id_cel = a.doc.add_cel(cel);
    a.doc.panels[neuf].layers[0].frames.push(Some(id_cel));
    a.enregistrer();
    // Les plans et les dessins arrivent d'abord, la structure ensuite.
    sync.pousser(Sens::AversB, &|f| !f.ends_with("Film.pelure"));
    b.tour();
    h.avancer(5_000);
    b.tour();
    assert_eq!(b.doc.panels.len(), 3, "le plan n'est pas dans le film tant que la structure n'est pas là");
    sync.tout(Sens::AversB);
    b.tour();
    b.tour();
    assert_eq!(b.doc.panels.len(), 4);
    assert_eq!(b.doc.panels[neuf].id, neuf_id);
}

#[test]
fn taking_a_plan_waits_for_the_file_its_last_holder_wrote_even_when_the_release_arrives_first() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-prise", 3);
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    a.tour();
    a.dessiner(0, 150);
    a.enregistrer();
    a.s.rendre(&[Cible::Plan(a.plan_id(0))]);
    a.tour();
    // La fiche (prise rendue, révision 2) arrive chez Léa, pas le plan.
    sync.pousser(Sens::AversB, &|f| f.contains("partage/"));
    b.tour();
    assert_eq!(b.etat(0), EtatPrise::Libre);
    assert!(b.prendre_plan(0).is_ok(), "la prise est enregistrée…");
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(0)), Err(Refus::Synchronisation), "…mais le plan reste fermé tant qu'il n'est pas à jour");
    b.tour();
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(0)), Err(Refus::Synchronisation));
    // Le plan et ses dessins arrivent : le plan s'ouvre.
    sync.tout(Sens::AversB);
    h.avancer(10_000);
    b.tour();
    h.avancer(10_000);
    b.tour();
    assert_eq!(b.rouge(0), 150);
    assert_eq!(b.s.peut_modifier_plan(b.plan_id(0)), Ok(()));
    // Et une synchronisation trop lente le dit.
    let (mut a2, mut b2, h2, mut sync2) = deux_postes_synchro("synchro-lente", 3);
    a2.prendre_plan(0).expect("libre");
    confirmer(&h2);
    a2.tour();
    a2.dessiner(0, 9);
    a2.enregistrer();
    a2.s.rendre(&[Cible::Plan(a2.plan_id(0))]);
    a2.tour();
    sync2.pousser(Sens::AversB, &|f| f.contains("partage/"));
    b2.tour();
    b2.prendre_plan(0).expect("prise");
    h2.avancer(61_000);
    b2.tour();
    assert!(b2.s.vue().synchro_lente.contains(&Cible::Plan(b2.plan_id(0))));
}

#[test]
fn two_copies_of_the_same_plan_made_in_two_folders_give_a_conflict_copy_that_becomes_a_version() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-conflit", 3);
    // Anthony et Léa prennent le même plan chacun dans leur copie, avant que rien ne soit synchronisé, et l'enregistrent.
    a.prendre_plan(1).expect("libre");
    b.prendre_plan(1).expect("libre chez elle aussi");
    confirmer(&h);
    a.tour();
    b.tour();
    a.dessiner(1, 111);
    b.dessiner(1, 222);
    a.enregistrer();
    b.enregistrer();
    // La synchronisation tombe sur deux modifications du même fichier de plan.
    let copiees = sync.tout(Sens::AversB);
    assert!(copiees.iter().any(|f| f.contains("plans/")));
    let copies: Vec<String> = lister(&b.manifeste.parent().expect("dossier").join("plans")).keys().cloned().collect();
    assert!(copies.iter().any(|f| f.contains("Copie en conflit")), "{copies:?}");
    // Le balayage de Léa repère la copie en conflit.
    b.tour();
    let vue = b.s.vue();
    assert_eq!(vue.copies.len(), 1, "{:?}", vue.copies);
    // Les fiches sont arrivées aussi : Léa a perdu la course (Anthony était là avant), son travail est en brouillon…
    h.avancer(1_000);
    b.tour();
    b.tour();
    assert!(b.perdues.contains(&Cible::Plan(b.plan_id(1))), "{:?}", b.perdues);
}

#[test]
fn a_conflict_copy_of_a_plan_the_holder_has_is_imported_as_a_version_and_filed_away() {
    // Un seul poste suffit : le dossier contient une copie en conflit fabriquée par un client de synchronisation.
    let (mut a, _, h, racine) = deux_postes("copie-conflit", 3);
    a.prendre_plan(1).expect("libre");
    confirmer(&h);
    a.tour();
    let id = a.plan_id(1);
    // La « copie en conflit » est un plan où Léa avait dessiné 77.
    let (mut l, _, _, _) = deux_postes("copie-conflit-source", 3);
    l.prendre_plan(1).expect("libre");
    confirmer(&l.horloge);
    l.tour();
    let mut neuf = Cel::new(l.doc.width, l.doc.height, LayerKind::Bitmap);
    neuf.raster.tile_mut(0)[0] = Color32::from_rgba_premultiplied(77, 1, 2, 255);
    let id_neuf = l.doc.add_cel(neuf);
    l.doc.panels[1].layers[0].frames[0] = Some(id_neuf);
    l.enregistrer();
    let source = chemin_plan(&racine.parent().expect("parent").join("shot-fiabilite-coop-copie-conflit-source").join(""), id);
    let _ = source;
    let copie_src = fx::racine(&l.manifeste).join("plans").join(format!("{id}.json"));
    // Les dessins de Léa sont à côté de ceux du projet d'Anthony (un client de synchronisation les aurait amenés aussi).
    for e in std::fs::read_dir(fx::racine(&l.manifeste).join("dessins")).expect("dessins").flatten() {
        let vers = fx::racine(&a.manifeste).join("dessins").join(e.file_name());
        std::fs::copy(e.path(), vers).expect("copie");
    }
    let copie = fx::racine(&a.manifeste).join("plans").join(format!("{id} (Copie en conflit de Lea 2026-10-20).json"));
    std::fs::copy(copie_src, &copie).expect("copie en conflit");
    a.tour();
    let vue = a.s.vue();
    assert_eq!(vue.copies.len(), 1);
    let c = vue.copies[0].clone();
    assert_eq!(c.origine, crate::depot::OrigineCopie::Plan(id));
    let octets = a.s.depot().lire_copie(&c).expect("lecture");
    let numero = a.s.importer_comme_version(&mut a.doc, id, &octets, "copie en conflit de Lea").expect("importée");
    assert_eq!(numero, 2);
    let v = &a.doc.versions[&id].autres[0];
    let cel = v.panel.layers[0].frames[0].expect("dessin");
    assert_eq!(a.doc.cels[&cel].raster.get(0, 0).r(), 77);
    a.s.depot().ranger_copie(&c).expect("rangée");
    a.s.copie_traitee(&c);
    a.tour();
    assert!(a.s.vue().copies.is_empty());
    assert!(copie.exists() == false && fx::racine(&a.manifeste).join("partage").join("importes").exists(), "déplacée, jamais effacée");
}

#[test]
fn work_flows_both_ways_between_two_synchronised_folders_and_a_late_fiche_does_not_matter() {
    let (mut a, mut b, h, mut sync) = deux_postes_synchro("synchro-deux-sens", 4);
    a.prendre_plan(0).expect("libre");
    b.prendre_plan(2).expect("libre");
    confirmer(&h);
    a.tour();
    b.tour();
    a.dessiner(0, 31);
    b.dessiner(2, 32);
    a.enregistrer();
    b.enregistrer();
    // Rien n'a encore voyagé : chacun ne voit que son plan.
    a.tour();
    b.tour();
    assert_ne!(a.rouge(2), 32);
    assert_ne!(b.rouge(0), 31);
    // Léa envoie d'abord (ses fichiers arrivent chez Anthony), Anthony ensuite.
    sync.tout(Sens::BversA);
    a.tour();
    h.avancer(5_000);
    a.tour();
    assert_eq!(a.rouge(2), 32, "le plan de Léa est arrivé chez Anthony");
    assert!(matches!(a.etat(2), EtatPrise::Autre(_)));
    assert!(matches!(a.etat(0), EtatPrise::Moi { .. }), "Anthony tient toujours le sien");
    sync.tout(Sens::AversB);
    b.tour();
    h.avancer(5_000);
    b.tour();
    assert_eq!(b.rouge(0), 31);
    assert!(matches!(b.etat(0), EtatPrise::Autre(_)));
    // Les deux ont enregistré chacun son plan : aucune copie en conflit.
    assert!(a.s.vue().copies.is_empty() && b.s.vue().copies.is_empty());
}
// ------------------------------------------------------------------------------------------ hors connexion

/// Un dépôt qu'on peut couper : tout échoue tant qu'il est coupé (un dossier réseau éteint).
struct DepotCoupe {
    dedans: DossierPartage,
    coupe: Arc<std::sync::atomic::AtomicBool>,
}

impl DepotCoupe {
    fn ok(&self) -> Result<(), String> {
        if self.coupe.load(std::sync::atomic::Ordering::Relaxed) {
            Err("le dossier réseau ne répond pas".into())
        } else {
            Ok(())
        }
    }
}

impl crate::depot::Depot for DepotCoupe {
    fn nom(&self) -> String {
        "dépôt coupable".into()
    }
    fn racine(&self) -> &Path {
        self.dedans.racine()
    }
    fn surveiller(&self, c: &mut crate::depot::Curseur) -> Result<crate::depot::Releve, String> {
        self.ok()?;
        self.dedans.surveiller(c)
    }
    fn lire_structure(&self) -> Result<Option<crate::depot::Fichier>, String> {
        self.ok()?;
        self.dedans.lire_structure()
    }
    fn ecrire_structure(&self, o: &[u8]) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_structure(o)
    }
    fn lire_plan(&self, p: u64) -> Result<Option<crate::depot::Fichier>, String> {
        self.ok()?;
        self.dedans.lire_plan(p)
    }
    fn ecrire_plan(&self, p: u64, o: &[u8]) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_plan(p, o)
    }
    fn lire_fiches(&self) -> Result<Vec<crate::verrous::Fiche>, String> {
        self.ok()?;
        self.dedans.lire_fiches()
    }
    fn ecrire_fiche(&self, f: &crate::verrous::Fiche) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_fiche(f)
    }
    fn lire_membres(&self) -> Result<Vec<crate::depot::Membre>, String> {
        self.ok()?;
        self.dedans.lire_membres()
    }
    fn ecrire_membre(&self, m: &crate::depot::Membre) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_membre(m)
    }
    fn lire_commentaires(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        self.ok()?;
        self.dedans.lire_commentaires()
    }
    fn ecrire_commentaires(&self, e: &str, o: &[u8]) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_commentaires(e, o)
    }
    fn ecrire_brouillon(&self, n: &str, o: &[u8]) -> Result<(), String> {
        self.ok()?;
        self.dedans.ecrire_brouillon(n, o)
    }
    fn lister_brouillons(&self) -> Result<Vec<String>, String> {
        self.ok()?;
        self.dedans.lister_brouillons()
    }
    fn lire_brouillon(&self, n: &str) -> Result<Vec<u8>, String> {
        self.ok()?;
        self.dedans.lire_brouillon(n)
    }
    fn ranger_brouillon(&self, n: &str) -> Result<(), String> {
        self.ok()?;
        self.dedans.ranger_brouillon(n)
    }
    fn lire_copie(&self, c: &crate::depot::CopieEnConflit) -> Result<Vec<u8>, String> {
        self.ok()?;
        self.dedans.lire_copie(c)
    }
    fn ranger_copie(&self, c: &crate::depot::CopieEnConflit) -> Result<(), String> {
        self.ok()?;
        self.dedans.ranger_copie(c)
    }
}

/// Anthony travaille par un dépôt qu'on peut couper ; Léa, par un dépôt ordinaire du même dossier.
fn anthony_coupable(nom: &str) -> (Poste, Poste, Horloge, Arc<std::sync::atomic::AtomicBool>) {
    let racine = dossier(nom);
    let manifeste = partager(&film(3), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let Loaded { mut doc, partage, written, .. } = project::load(&manifeste).expect("projet");
    let coupe = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let depot = Arc::new(DepotCoupe { dedans: DossierPartage::nouveau(&manifeste), coupe: coupe.clone() });
    let moi = Identite { id: "anthony".into(), nom: "Anthony".into(), couleur: [1, 2, 3], machine: "poste".into(), seance: "s1".into() };
    let s = Session::rejoindre(depot, moi, partage.expect("partagé"), &mut doc, delais(), h.clone(), false, None).expect("session");
    let a = Poste { doc, s, ecrits: written, manifeste: manifeste.clone(), horloge: h.clone(), messages: Vec::new(), perdues: Vec::new() };
    let b = Poste::ouvrir(&manifeste, "Lea", &h);
    (a, b, h, coupe)
}

#[test]
fn working_offline_on_a_plan_one_still_holds_is_saved_after_reconnecting() {
    let (mut a, mut b, h, coupe) = anthony_coupable("hors-ligne-garde");
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    coupe.store(true, std::sync::atomic::Ordering::Relaxed);
    a.dessiner(0, 66);
    a.tour();
    assert!(a.s.vue().hors_ligne.is_some(), "le moteur dit que le dépôt ne répond pas");
    // L'enregistrement échoue sans rien perdre : le travail reste en mémoire.
    let ctx = a.s.contexte_ecriture(&a.doc, false);
    let job = partage_io::preparer(&a.doc, &a.manifeste, &mut a.ecrits, ctx).expect("préparation");
    assert!(job.run_with(None, 2).is_err());
    assert_eq!(a.rouge(0), 66);
    // Quelques minutes passent (moins que le bail), le réseau revient.
    h.avancer(4 * 60_000);
    coupe.store(false, std::sync::atomic::Ordering::Relaxed);
    a.ecrits.clear();
    a.tour();
    a.tour();
    assert!(a.s.vue().hors_ligne.is_none());
    assert!(a.s.tiens(Cible::Plan(a.plan_id(0))), "personne ne lui a pris le plan");
    let r = a.enregistrer();
    assert_eq!(r.plans_ecrits.len(), 1);
    b.tour();
    b.tour();
    assert_eq!(b.rouge(0), 66);
}

#[test]
fn work_done_offline_on_a_plan_that_was_taken_over_meanwhile_becomes_a_draft() {
    let (mut a, mut b, h, coupe) = anthony_coupable("hors-ligne-repris");
    a.prendre_plan(0).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    coupe.store(true, std::sync::atomic::Ordering::Relaxed);
    a.dessiner(0, 66);
    a.tour();
    // Une heure passe : Léa reprend le plan d'Anthony, qui a disparu.
    h.avancer(60 * 60_000);
    b.tour();
    let id = b.plan_id(0);
    assert_eq!(b.s.prendre(&[Cible::Plan(id)], true).remove(0).1, Ok(()));
    b.tour();
    // Le réseau revient chez Anthony : il apprend qu'il a perdu le plan, son dessin n'est pas perdu.
    coupe.store(false, std::sync::atomic::Ordering::Relaxed);
    a.tour();
    a.tour();
    assert_eq!(a.perdues, vec![Cible::Plan(id)]);
    a.ecrits.clear();
    a.enregistrer();
    assert_eq!(brouillons(&a.manifeste).len(), 1, "le travail d'Anthony est dans un brouillon");
    // Le plan de Léa n'a pas bougé.
    b.tour();
    assert_ne!(b.rouge(0), 66);
}

// ------------------------------------------------------------------------------------------ présence, annuler

#[test]
fn presence_shows_who_is_there_and_what_they_look_at() {
    let (mut a, mut b, h, _) = deux_postes("presence", 3);
    a.s.regarde(Some(a.plan_id(1)), "Animation");
    a.prendre_plan(0).expect("libre");
    tours(&mut a, &mut b);
    let vue = b.s.vue();
    let anthony = vue.equipe.iter().find(|e| e.nom == "Anthony").expect("Anthony est en ligne");
    assert!(anthony.en_ligne && !anthony.moi);
    assert_eq!(anthony.tient, vec![Cible::Plan(a.plan_id(0))]);
    // La présence (le plan regardé) est écrite au plus toutes les cinq secondes.
    h.avancer(6_000);
    a.tour();
    b.tour();
    let anthony = b.s.vue().equipe.iter().find(|e| e.nom == "Anthony").cloned().expect("Anthony");
    assert_eq!(anthony.plan, Some(a.plan_id(1)));
    assert_eq!(anthony.mode, "Animation");
    assert!(b.s.vue().equipe.iter().any(|e| e.moi && e.nom == "Lea"));
    // Plus de nouvelles depuis longtemps : hors ligne (mais s'il tient un plan, il reste dans la liste).
    h.avancer(200_000);
    b.tour();
    let anthony = b.s.vue().equipe.iter().find(|e| e.nom == "Anthony").cloned().expect("encore listé : il tient un plan");
    assert!(!anthony.en_ligne);
    // Il rend son plan : on ne le voit plus.
    h.avancer(31_000);
    a.s.rendre(&[Cible::Plan(a.plan_id(0))]);
    a.tour();
    b.tour();
    h.avancer(200_000);
    b.tour();
    assert!(!b.s.vue().equipe.iter().any(|e| e.nom == "Anthony"));
}

#[test]
fn undo_never_brings_back_an_old_copy_of_a_plan_somebody_else_saved() {
    let (mut a, mut b, h, _) = deux_postes("annuler", 3);
    b.prendre_plan(1).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    // Anthony garde une photo de son film d'avant (comme une étape d'annulation), puis Léa enregistre.
    let avant = a.doc.structure();
    b.doc.panels[1].action = "Léa écrit l'action".into();
    b.dessiner(1, 130);
    b.enregistrer();
    a.tour();
    assert_eq!(a.doc.panels[1].action, "Léa écrit l'action");
    // Anthony annule : l'étape remet toute la structure d'avant, y compris le vieux plan de Léa.
    let _ = a.doc.replace_structure(avant);
    assert_eq!(a.doc.panels[1].action, "", "le vieux plan est revenu avec l'annulation");
    a.s.rafraichir_etranger(&mut a.doc);
    assert_eq!(a.doc.panels[1].action, "Léa écrit l'action", "…puis il est remis dans son dernier état reçu");
}

#[test]
fn a_plan_deleted_by_the_montage_holder_while_another_works_in_it_is_put_aside() {
    let (mut a, mut b, h, _) = deux_postes("supprime-tenu", 3);
    b.prendre_plan(2).expect("libre");
    confirmer(&h);
    tours(&mut a, &mut b);
    b.dessiner(2, 88);
    // Anthony prend le montage et supprime le plan 2 que Léa tient (l'interface le lui refuserait ; ici la course est faite
    // avant que sa prise lui soit arrivée).
    let id = a.plan_id(2);
    a.s.prendre(&[Cible::Montage], false);
    confirmer(&h);
    a.tour();
    a.doc.remove_panels(&[id]).expect("supprimer");
    a.s.apres_structure(&a.doc);
    a.enregistrer();
    b.tour();
    b.tour();
    assert_eq!(b.doc.panels.len(), 2, "le plan a disparu du film de Léa");
    assert!(b.messages.iter().any(|m| m.contains("supprimé") && m.contains("brouillon")), "{:?}", b.messages);
    assert!(!b.s.tiens(Cible::Plan(id)));
    b.enregistrer();
    let liste = brouillons(&b.manifeste);
    assert_eq!(liste.len(), 1);
    assert!(liste[0].ends_with(".supprime.json"), "{liste:?}");
}

#[test]
fn the_loser_of_the_token_race_keeps_her_montage_aside_and_gets_the_winners() {
    let (mut a, mut b, h, _) = deux_postes("jeton-course", 3);
    a.s.prendre(&[Cible::Montage], false);
    h.avancer(1_000);
    b.s.prendre(&[Cible::Montage], false);
    // Léa renomme la séquence 0 pendant que sa prise est provisoire ; Anthony, lui, la nomme autrement.
    let seq = b.doc.sequences[0].id;
    b.doc.sequence_mut(seq).expect("séquence").name = "Nom de Léa".into();
    a.doc.sequence_mut(seq).expect("séquence").name = "Nom d'Anthony".into();
    confirmer(&h);
    a.tour();
    b.tour();
    a.tour();
    a.enregistrer();
    for _ in 0..3 {
        b.tour();
    }
    assert!(b.perdues.contains(&Cible::Montage));
    assert_eq!(b.doc.sequences[0].name, "Nom d'Anthony", "le montage du gagnant remplace le sien");
    b.enregistrer();
    let liste = brouillons(&b.manifeste);
    assert!(liste.iter().any(|n| n.starts_with("montage.lea.")), "son montage est gardé : {liste:?}");
    let brouillon = b.s.lire_brouillon(liste.iter().find(|n| n.starts_with("montage.lea.")).expect("brouillon")).expect("lecture");
    assert!(String::from_utf8_lossy(&brouillon).contains("Nom de Léa"));
}

#[test]
fn opening_a_shared_project_does_not_reload_the_plans_it_just_read() {
    let (mut a, _, _, _) = deux_postes("ouverture-calme", 4);
    for _ in 0..3 {
        let fx = a.tour();
        assert!(!fx.documents_changes, "rien n'a changé depuis la lecture");
    }
    assert!(a.s.notifications.is_empty(), "{:?}", a.s.notifications);
}

#[test]
fn a_shared_film_of_150_plans_costs_little_to_follow() {
    let racine = dossier("cout");
    let manifeste = partager(&film(150), &racine);
    let h = Horloge::reglable(1_760_000_000_000);
    let mut a = Poste::ouvrir(&manifeste, "Anthony", &h);
    a.tour();
    let t = std::time::Instant::now();
    for _ in 0..10 {
        a.tour();
    }
    let tour = t.elapsed().as_secs_f64() * 100.0;
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        a.s.sonder(&mut a.doc);
    }
    let sonder = t.elapsed().as_secs_f64() * 1e3;
    assert!(tour < 300.0, "un tour du moteur coûte {tour:.1} ms (fil de fond)");
    assert!(sonder < 100.0, "1000 images d'écran au repos coûtent {sonder:.1} ms de la session");
}