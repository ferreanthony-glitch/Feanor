//! L'**aplat intelligent**, côté application (Anthony, 2026-10-28) : une case « Aplat intelligent » sur l'aplat d'un calque
//! (`FondReglages::intelligent`) et une commande « Colorier comme la clé précédente ». Le calcul des régions et leur
//! correspondance sont dans `aplatregions.rs` ; ici : quand ça agit, et comment le résultat arrive dans le dessin.
//!
//! ## Quand ça agit (case cochée seulement ; décochée, rien ne change)
//! 1. **Retouche du trait d'une clé déjà coloriée.** Au début d'un trait de pinceau, de crayon ou de gomme sur le trait du calque,
//!    on garde le dessin et sa peinture d'aplat (`Avant`, quelques tuiles partagées : rien n'est recopié). À la fin du trait, si le
//!    trait a changé, l'aplat de la clé est refait : chaque zone fermée **garde sa couleur** par correspondance avec les zones
//!    d'avant la retouche (le trait a pu déplacer le bord d'une zone, en fermer une nouvelle, en fondre deux…).
//! 2. **Une clé qui suit une clé coloriée.** Si l'aplat de la clé n'a encore aucune peinture (clé neuve, ou dont on vient de
//!    redessiner le trait), ses zones prennent les couleurs de la clé coloriée la plus proche **avant** elle sur le calque. C'est
//!    automatique à la fin du trait ; et la commande « Colorier comme la clé précédente » le fait pour les clés choisies dans la
//!    timeline, **en chaîne** (chacune suit la précédente, déjà coloriée par la commande) pour qu'un mouvement progressif suive.
//!    Une clé faite par duplication emporte déjà l'aplat de l'original (peinture comprise) : ses retouches suivent le cas 1.
//! 3. **Réglable en redessinant.** Peindre ou remplir une zone à la main (la sous-ligne de l'aplat choisie) ne déclenche rien : la
//!    peinture est dans la strate, et c'est elle que lisent les cas 1 et 2 pour la suite. Une zone qui n'a pas de correspondance
//!    reste vide (couleur du calque), ou prend la couleur de la zone qui l'entoure quand celle-ci est appariée (sûr : un œil neuf
//!    dans un visage colorié) — voir `aplatregions.rs`.
//!
//! ## Calcul
//! Jamais sur le fil de l'interface : un fil de travail reçoit les tuiles (partagées) du trait, de la peinture et de la coupe, et rend
//! la peinture et la coupe refaites. Le résultat est posé à l'arrivée (`poll_aplat_multi`, à chaque image d'écran, rien au repos), si
//! le dessin n'a pas changé depuis le départ ; **dans l'étape d'annulation du trait** (retouche) ou dans une étape à lui
//! (commande, ou si une autre étape s'est glissée). Pas de pose pendant un geste de dessin : le résultat attend. Un trait fait
//! pendant que le calcul tourne le remplace : le nouveau calcul part de l'état d'avant le premier trait. Une clé vectorielle
//! attend son rendu exact avant que le calcul parte. L'enregistrement attend les calculs en route (`attendre_aplat_multi`).

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::app::{App, Tool};
use crate::aplatregions::{self as reg, Bilan, Entree};
use crate::document::{Cel, CelId, Strate};
use crate::fond::FondReglages;
use crate::history::Edit;
use crate::raster::Raster;

/// Un dessin et son aplat : le trait, la peinture de la strate, sa coupe (les tuiles sont partagées, rien n'est recopié).
#[derive(Clone)]
pub(crate) struct Src {
    trait_: Raster,
    peinture: Raster,
    coupe: Raster,
}

impl Src {
    fn de(c: &Cel) -> Src {
        let (w, h) = (c.raster.width, c.raster.height);
        let (peinture, coupe) = match c.fond.as_deref() {
            Some(s) => (s.peinture.raster.clone(), s.coupe.clone()),
            None => (Raster::new(w, h), Raster::new(w, h)),
        };
        Src { trait_: c.raster.clone(), peinture, coupe }
    }

    fn entree(&self) -> Entree<'_> {
        Entree { trait_: &self.trait_, peinture: &self.peinture, coupe: &self.coupe }
    }
}

/// Une source possible d'une cible : le rang dans la liste des sources (`docs` d'abord, puis une par cible, celle que la cible
/// a une fois coloriée : la chaîne), et si c'est la même clé avant sa retouche.
#[derive(Clone, Copy)]
struct Ref {
    idx: usize,
    retouche: bool,
}

/// Un dessin à colorier.
struct Cible {
    cel: CelId,
    trait_: Raster,
    peinture: Raster,
    coupe: Raster,
    /// Les versions du dessin au départ : (dessin, peinture de la strate) ; le résultat n'est posé que si rien n'a changé.
    versions: (u64, u64),
    gen: u64,
    /// Les sources à essayer, la plus proche d'abord ; la première utilisable sert.
    refs: Vec<Ref>,
}

struct Resultat {
    cel: CelId,
    peinture: Raster,
    coupe: Raster,
    versions: (u64, u64),
    gen: u64,
    bilan: Bilan,
    /// Retouche : celui des états d'avant qui avait le plus de zones fermées, (le « souvenir », voir `Souvenir`).
    souvenir: Option<Src>,
}

/// Le dernier état d'un dessin où ses zones étaient fermées (le plus de zones), avec ses couleurs d'alors. Un trait qui ouvre le
/// contour d'une zone (une gomme qui y fait une brèche) la fait disparaître : la retouche qui le referme retrouve la zone et sa
/// couleur dans ce souvenir, pas dans l'état d'avant ce trait-là, où elle n'existe plus. Valable tant que la peinture de l'aplat
/// n'a pas été touchée à la main (`pv` : sa version, après notre dernière pose).
struct Souvenir {
    src: Src,
    pv: u64,
}

/// Ce qui revient du fil de travail.
struct Fini {
    /// Le numéro de l'historique juste après l'étape du trait : un résultat qui arrive tant qu'il est le dernier la rejoint.
    serial: Option<u64>,
    nom: &'static str,
    resultats: Vec<Resultat>,
    /// Cibles sans aucune source utilisable.
    sans_source: usize,
}

/// Le trait qui vient de commencer : le dessin d'avant.
struct Avant {
    panel: u64,
    layer: u64,
    /// Rien : il n'y avait pas de dessin à cette image.
    src: Option<Src>,
}

/// Un trait fini dont le calcul n'est pas encore parti (le rendu exact d'un trait vectoriel).
struct Attente {
    cel: CelId,
    panel: u64,
    layer: u64,
    docs: Vec<Src>,
    refs: Vec<Ref>,
    serial: u64,
}

/// Les calculs de l'aplat intelligent d'une application (un par onglet de projet).
pub(crate) struct Etat {
    avant: Option<Avant>,
    attentes: Vec<Attente>,
    /// Le dernier calcul demandé pour chaque dessin : un résultat d'un autre numéro est périmé.
    gens: HashMap<CelId, u64>,
    /// Pour chaque dessin dont un calcul de retouche est en route : l'état d'avant le premier trait.
    sources: HashMap<CelId, Src>,
    prochain: u64,
    tx: Sender<Fini>,
    rx: Receiver<Fini>,
    en_route: usize,
    /// Résultats arrivés pendant un geste de dessin.
    retenus: Vec<Fini>,
    souvenirs: HashMap<CelId, Souvenir>,
}

impl Default for Etat {
    fn default() -> Self {
        let (tx, rx) = channel();
        Etat { avant: None, attentes: Vec::new(), gens: HashMap::new(), sources: HashMap::new(), prochain: 1, tx, rx, en_route: 0, retenus: Vec::new(), souvenirs: HashMap::new() }
    }
}

impl Etat {
    /// Rien n'est en route, en attente ni retenu.
    fn vide(&self) -> bool {
        self.en_route == 0 && self.attentes.is_empty() && self.retenus.is_empty()
    }
}

/// Le calcul d'un lot : pour chaque cible, la première source utilisable, puis la mise en couleur.
fn calculer(reglages: FondReglages, docs: Vec<Src>, cibles: Vec<Cible>, serial: Option<u64>, nom: &'static str) -> Fini {
    let n_docs = docs.len();
    let mut srcs: Vec<Option<Src>> = docs.into_iter().map(Some).collect();
    srcs.extend(cibles.iter().map(|_| None));
    let mut preps: Vec<Option<Option<reg::Preparee>>> = srcs.iter().map(|_| None).collect();
    let mut resultats = Vec::new();
    let mut sans_source = 0;
    for (i, c) in cibles.iter().enumerate() {
        // Retouche : parmi les états d'avant, celui qui retrouve le plus de zones ; sinon, la première clé coloriée qui suit.
        let mut retouche: Option<(Raster, Raster, Bilan)> = None;
        let mut suivi: Option<(Raster, Raster, Bilan)> = None;
        let mut souvenir: Option<(usize, usize)> = None;
        let dst = Entree { trait_: &c.trait_, peinture: &c.peinture, coupe: &c.coupe };
        for rf in &c.refs {
            if preps[rf.idx].is_none() {
                preps[rf.idx] = Some(srcs[rf.idx].as_ref().and_then(|s| reg::preparer(&s.entree(), &reglages)));
            }
            let (Some(s), Some(Some(p))) = (srcs[rf.idx].as_ref(), preps[rf.idx].as_ref()) else { continue };
            if rf.retouche {
                let n = p.regions.regs.len();
                if souvenir.is_none_or(|(_, m)| n > m) {
                    souvenir = Some((rf.idx, n));
                }
                if let Some(r) = reg::colorier(&s.entree(), p, &dst, &reglages, true) {
                    if retouche.as_ref().is_none_or(|m| r.2.appariees > m.2.appariees) {
                        retouche = Some(r);
                    }
                }
            } else if retouche.is_none() && suivi.is_none() && p.coloriee() {
                // Suivre une autre clé n'a de sens que si elle est coloriée.
                suivi = reg::colorier(&s.entree(), p, &dst, &reglages, false);
            }
        }
        let souvenir = souvenir.and_then(|(idx, _)| srcs[idx].clone());
        match retouche.or(suivi) {
            Some((peinture, coupe, bilan)) => {
                // La chaîne : la clé suivante a pour source celle-ci, une fois coloriée.
                srcs[n_docs + i] = Some(Src { trait_: c.trait_.clone(), peinture: peinture.clone(), coupe: coupe.clone() });
                preps[n_docs + i] = None;
                resultats.push(Resultat { cel: c.cel, peinture, coupe, versions: c.versions, gen: c.gen, bilan, souvenir });
            }
            None => sans_source += 1,
        }
    }
    Fini { serial, nom, resultats, sans_source }
}

/// Le dessin `cel` comme cible d'un calcul. Le numéro du calcul (`gen`) est pris dans `prochain` ; l'appelant le range dans `gens`.
fn cible_de(prochain: &mut u64, cel: CelId, c: &Cel, refs: Vec<Ref>) -> Cible {
    let (peinture, coupe, pv) = match c.fond.as_deref() {
        Some(s) => (s.peinture.raster.clone(), s.coupe.clone(), s.peinture.version),
        None => (Raster::new(c.raster.width, c.raster.height), Raster::new(c.raster.width, c.raster.height), u64::MAX),
    };
    let gen = *prochain;
    *prochain += 1;
    Cible { cel, trait_: c.raster.clone(), peinture, coupe, versions: (c.version, pv), gen, refs }
}

/// Les mêmes tuiles, au même endroit (aucun pixel n'a changé).
fn memes_tuiles(a: &Raster, b: &Raster) -> bool {
    a.tiles == b.tiles && a.outer == b.outer
}

impl App {
    /// L'aplat du calque de rang `li` est intelligent.
    pub(crate) fn aplat_intelligent(&self, li: usize) -> bool {
        self.layers().get(li).and_then(|l| l.fond).is_some_and(|f| f.intelligent)
    }

    /// Un trait de pinceau, de crayon ou de gomme sur le trait d'un calque à aplat intelligent va commencer : on garde le dessin
    /// d'avant (`begin_paint`, avant que le trait ne crée ou ne copie le dessin).
    pub(crate) fn intel_avant_trait(&mut self) {
        self.aplat_intel.avant = None;
        if !matches!(self.tool, Tool::Brush | Tool::Pencil | Tool::Eraser) {
            return;
        }
        let li = self.cur_layer;
        if !self.aplat_intelligent(li) || self.aplat_en_cible() || self.sur_aplat_cible().is_some() {
            return;
        }
        let Some(l) = self.layers().get(li) else { return };
        if l.couleur.is_some() || l.folder || l.video.is_some() {
            return;
        }
        let (layer, panel) = (l.id, self.doc.panels[self.cur_panel].id);
        let src = l.cel_at(self.cur_frame).and_then(|c| self.doc.cels.get(&c)).map(Src::de);
        self.aplat_intel.avant = Some(Avant { panel, layer, src });
    }

    /// Un trait vient de finir : si le trait du dessin a changé, l'aplat de la clé suit (cas 1 ou 2 du début du fichier).
    pub(crate) fn intel_apres_trait(&mut self) {
        let Some(avant) = self.aplat_intel.avant.take() else { return };
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == avant.panel) else { return };
        let Some(li) = self.doc.panels[pi].layers.iter().position(|l| l.id == avant.layer) else { return };
        let l = &self.doc.panels[pi].layers[li];
        if !l.fond.is_some_and(|f| f.intelligent) {
            return;
        }
        let Some(cel) = l.cel_at(self.cur_frame) else { return };
        let Some(c) = self.doc.cels.get(&cel) else { return };
        let change = match &avant.src {
            Some(s) => !memes_tuiles(&s.trait_, &c.raster),
            None => !c.raster.is_blank(),
        };
        if !change {
            return;
        }
        let peinte = avant.src.as_ref().is_some_and(|s| !s.peinture.is_blank());
        // Un calcul de retouche déjà en route sur ce dessin : on repart de ce qu'il avait pour source (l'état d'avant le premier
        // trait), pas de l'état d'avant celui-ci, qui n'a pas encore ses couleurs.
        let en_route = self.aplat_intel.sources.get(&cel).cloned();
        let (mut docs, mut refs) = (Vec::new(), Vec::new());
        if let Some(s) = en_route.or(avant.src.filter(|_| peinte)) {
            docs.push(s);
            refs.push(Ref { idx: 0, retouche: true });
            // Le souvenir (un état d'avant avec plus de zones fermées), s'il est encore à jour.
            let pv = c.fond.as_ref().map_or(u64::MAX, |f| f.peinture.version);
            if let Some(sv) = self.aplat_intel.souvenirs.get(&cel).filter(|sv| sv.pv == pv) {
                refs.push(Ref { idx: docs.len(), retouche: true });
                docs.push(sv.src.clone());
            }
        }
        // La clé coloriée la plus proche avant celle-ci : la source d'une clé sans peinture, et l'ultime recours d'une retouche.
        if let Some(p) = self.cle_precedente_coloriee(pi, li, self.cur_frame, &[cel]) {
            refs.push(Ref { idx: docs.len(), retouche: false });
            docs.push(p);
        }
        if refs.is_empty() {
            return;
        }
        let serial = self.history.serial();
        self.aplat_intel.attentes.retain(|a| a.cel != cel);
        self.aplat_intel.attentes.push(Attente { cel, panel: avant.panel, layer: avant.layer, docs, refs, serial });
        self.intel_lancer_pretes(false);
    }

    /// La clé coloriée (aplat peint) la plus proche avant l'image `frame` du calque `li` du plan `pi`, hors les dessins `exclure`.
    fn cle_precedente_coloriee(&self, pi: usize, li: usize, frame: usize, exclure: &[CelId]) -> Option<Src> {
        let l = self.doc.panels.get(pi)?.layers.get(li)?;
        let (debut, _) = l.run_bounds(frame);
        let mut vus: Vec<CelId> = Vec::new();
        for f in (0..debut).rev() {
            let Some(cel) = l.cel_at(f) else { continue };
            if exclure.contains(&cel) || vus.contains(&cel) {
                continue;
            }
            vus.push(cel);
            let Some(c) = self.doc.cels.get(&cel) else { continue };
            if c.fond.as_deref().is_some_and(|s| !s.peinture.raster.is_blank()) {
                return Some(Src::de(c));
            }
        }
        None
    }

    /// Lance les calculs en attente dont le dessin est prêt. `force` : sans attendre le rendu exact d'un trait vectoriel
    /// (l'enregistrement : le trait du rendu approché a les mêmes zones).
    fn intel_lancer_pretes(&mut self, force: bool) {
        if self.aplat_intel.attentes.is_empty() {
            return;
        }
        let attentes = std::mem::take(&mut self.aplat_intel.attentes);
        let mut reste = Vec::new();
        for a in attentes {
            if !force && (self.rejeu.is_pending(a.cel) || self.rejeu_again.contains(&a.cel) || self.cel_busy(a.cel)) {
                reste.push(a);
            } else {
                self.intel_lancer_une(a);
            }
        }
        self.aplat_intel.attentes = reste;
    }

    fn intel_lancer_une(&mut self, a: Attente) {
        let Some(reglages) = self
            .doc
            .panels
            .iter()
            .find(|p| p.id == a.panel)
            .and_then(|p| p.layers.iter().find(|l| l.id == a.layer))
            .and_then(|l| l.fond)
            .filter(|f| f.intelligent)
        else {
            return;
        };
        let Some(c) = self.doc.cels.get(&a.cel) else { return };
        let cible = cible_de(&mut self.aplat_intel.prochain, a.cel, c, a.refs.clone());
        let gen = cible.gen;
        self.aplat_intel.gens.insert(a.cel, gen);
        if a.refs.first().is_some_and(|r| r.retouche) {
            self.aplat_intel.sources.insert(a.cel, a.docs[0].clone());
        }
        self.lancer_fil(reglages, a.docs, vec![cible], Some(a.serial), "Aplat intelligent");
    }


    fn lancer_fil(&mut self, reglages: FondReglages, docs: Vec<Src>, cibles: Vec<Cible>, serial: Option<u64>, nom: &'static str) {
        let tx = self.aplat_intel.tx.clone();
        let ctx = self.ctx.clone();
        let spawned = std::thread::Builder::new().name("aplat-intelligent".into()).spawn(move || {
            let _ = tx.send(calculer(reglages, docs, cibles, serial, nom));
            ctx.request_repaint();
        });
        if spawned.is_ok() {
            self.aplat_intel.en_route += 1;
        }
    }

    /// Essais : un calcul est parti et son résultat n'est pas encore arrivé.
    #[cfg(test)]
    pub(crate) fn aplat_intel_en_route(&self) -> bool {
        self.aplat_intel.en_route > 0
    }

    /// Quelque chose est en route, en attente ou retenu (l'enregistrement attend).
    pub(crate) fn intel_occupe(&self) -> bool {
        !self.aplat_intel.vide()
    }

    /// Une fois par image d'écran (`poll_aplat_multi`) : lance ce qui est prêt, pose ce qui est arrivé. Rien au repos. `force` :
    /// même pendant un geste, et sans attendre un rendu vectoriel (avant d'enregistrer).
    pub(crate) fn poll_intel(&mut self, force: bool) {
        if self.aplat_intel.vide() {
            return;
        }
        self.intel_lancer_pretes(force);
        let arrives: Vec<Fini> = self.aplat_intel.rx.try_iter().collect();
        for f in arrives {
            self.aplat_intel.en_route = self.aplat_intel.en_route.saturating_sub(1);
            self.aplat_intel.retenus.push(f);
        }
        // Pendant un geste de dessin, les résultats attendent la fin du geste.
        let geste = self.stroke.is_some() || self.vstroke.is_some() || self.verase.is_some();
        if force || !geste {
            for f in std::mem::take(&mut self.aplat_intel.retenus) {
                self.intel_poser(f);
            }
        }
        if !self.aplat_intel.vide() {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    /// Pose un lot arrivé : la peinture et la coupe de chaque dessin dont rien n'a changé depuis le départ, en une étape d'annulation.
    fn intel_poser(&mut self, f: Fini) {
        let mut edits: Vec<Edit> = Vec::new();
        let (mut poses, mut abandonnes) = (0usize, 0usize);
        let mut bilan = Bilan::default();
        for r in f.resultats {
            // Un calcul plus récent est parti pour ce dessin : celui-ci est périmé.
            if self.aplat_intel.gens.get(&r.cel) != Some(&r.gen) {
                continue;
            }
            self.aplat_intel.gens.remove(&r.cel);
            self.aplat_intel.sources.remove(&r.cel);
            let Some(c) = self.doc.cels.get_mut(&r.cel) else {
                abandonnes += 1;
                continue;
            };
            if (c.version, c.fond.as_ref().map_or(u64::MAX, |s| s.peinture.version)) != r.versions {
                abandonnes += 1;
                continue;
            }
            let vide = Raster::new(c.raster.width, c.raster.height);
            let (vp, vc) = match c.fond.as_deref() {
                Some(s) => (&s.peinture.raster, &s.coupe),
                None => (&vide, &vide),
            };
            let tp = crate::multisel::changed_tiles(vp, &r.peinture);
            let tc = crate::multisel::changed_tiles(vc, &r.coupe);
            if !(tp.is_empty() && tc.is_empty()) {
                let (w, h) = (c.raster.width, c.raster.height);
                let s = c.fond.get_or_insert_with(|| Box::new(Strate::new(w, h)));
                if !tp.is_empty() {
                    edits.push(Edit::FondTiles { cel: r.cel, tiles: tp, coupe: false });
                }
                if !tc.is_empty() {
                    edits.push(Edit::FondTiles { cel: r.cel, tiles: tc, coupe: true });
                }
                s.peinture.raster = r.peinture;
                s.coupe = r.coupe;
                s.peinture.version += 1;
                c.version += 1;
                poses += 1;
            }
            if let Some(src) = r.souvenir {
                let pv = c.fond.as_ref().map_or(u64::MAX, |s| s.peinture.version);
                self.aplat_intel.souvenirs.insert(r.cel, Souvenir { src, pv });
            }
            bilan.regions_cible += r.bilan.regions_cible;
            bilan.appariees += r.bilan.appariees;
            bilan.englobees += r.bilan.englobees;
            bilan.laissees += r.bilan.laissees;
        }
        if !edits.is_empty() {
            self.modified = true;
            self.renderer.invalidate_all();
            let groupe = Edit::Group(edits);
            if f.serial == Some(self.history.serial()) {
                self.history.extend_last(groupe);
            } else {
                let avant = self.history.context.replace(f.nom.to_string());
                self.history.push(groupe);
                self.history.context = avant;
            }
        }
        let zones = if bilan.regions_cible > 0 {
            format!("{} zone{} suivie{}", bilan.appariees + bilan.englobees, if bilan.appariees + bilan.englobees > 1 { "s" } else { "" }, if bilan.appariees + bilan.englobees > 1 { "s" } else { "" })
                + &if bilan.laissees > 0 { format!(", {} sans correspondance (laissée{} comme elle est)", bilan.laissees, if bilan.laissees > 1 { "s" } else { "" }) } else { String::new() }
        } else {
            String::new()
        };
        if f.serial.is_none() {
            self.status = match (poses, abandonnes, f.sans_source) {
                (0, 0, 0) => "Aplat intelligent : rien à changer, les clés ont déjà ces couleurs.".into(),
                (n, 0, 0) => format!("Aplat intelligent : {n} clé{} coloriée{} comme la précédente ({zones}).", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" }),
                (n, a, s) => format!("Aplat intelligent : {n} clé(s) coloriée(s) ; {a} retouchée(s) pendant le calcul, {s} sans clé coloriée avant elle."),
            };
        } else if abandonnes > 0 {
            self.status = "Aplat intelligent : le dessin a changé pendant le calcul, ses couleurs n'ont pas été suivies (un nouveau trait les suivra).".into();
        } else if poses > 0 {
            self.status = format!("Aplat intelligent : {zones}.");
        }
    }

    /// « Colorier comme la clé précédente » : les clés choisies du calque (sinon la clé qu'on voit), chacune comme la clé coloriée la
    /// plus proche avant elle, **en chaîne** : une clé choisie qui en précède une autre lui sert de source, une fois coloriée.
    pub(crate) fn colorier_comme_precedente(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let pi = self.cur_panel;
        let panel = self.doc.panels[pi].id;
        // Le calque des clés choisies (le menu d'une clé peut viser un autre calque que le courant), sinon le calque courant.
        let layer = self.key_sel.iter().find(|k| k.panel == panel).map(|k| k.layer).or_else(|| self.layers().get(self.cur_layer).map(|l| l.id));
        let Some(li) = layer.and_then(|id| self.layers().iter().position(|l| l.id == id)) else { return };
        let Some(reglages) = self.layers()[li].fond else {
            self.status = "Aplat intelligent : ce calque n'a pas d'aplat (bouton « Aplat sous le trait » du calque).".into();
            return;
        };
        if !reglages.intelligent {
            self.status = "Aplat intelligent : coche « Aplat intelligent » dans les réglages de l'aplat de ce calque d'abord.".into();
            return;
        }
        let l = &self.layers()[li];
        let layer_id = l.id;
        let mut debuts: Vec<usize> = self.key_sel.iter().filter(|k| k.panel == panel && k.layer == layer_id).map(|k| k.start).collect();
        if debuts.is_empty() {
            debuts.push(self.cur_frame);
        }
        // Les clés du calque (un dessin une fois, à sa première image), dans l'ordre.
        let mut cles: Vec<(usize, CelId)> = Vec::new();
        for (f, c) in l.frames.iter().enumerate() {
            if let Some(c) = c {
                if !cles.iter().any(|(_, k)| k == c) {
                    cles.push((f, *c));
                }
            }
        }
        let mut cibles_cels: Vec<(usize, CelId)> = Vec::new();
        for d in debuts {
            if let Some(cel) = l.cel_at(d) {
                if let Some(&(f, _)) = cles.iter().find(|(_, k)| *k == cel) {
                    if !cibles_cels.iter().any(|(_, k)| *k == cel) {
                        cibles_cels.push((f, cel));
                    }
                }
            }
        }
        cibles_cels.sort();
        cibles_cels.retain(|(_, c)| self.doc.cels.contains_key(c));
        // Pour chaque cible, ses sources, la plus proche d'abord : une cible d'avant (la chaîne), ou une clé coloriée du calque.
        // Pour chaque cible, ses sources, la plus proche d'abord : une cible d'avant (la chaîne), ou une clé coloriée du calque.
        enum Brute {
            Doc(usize),
            Cible(usize),
        }
        let mut docs: Vec<Src> = Vec::new();
        let mut doc_de: HashMap<CelId, usize> = HashMap::new();
        let mut brutes: Vec<Vec<Brute>> = Vec::new();
        for &(debut, _) in &cibles_cels {
            let mut liste = Vec::new();
            for &(_, k) in cles.iter().rev().filter(|(f, _)| *f < debut) {
                if let Some(j) = cibles_cels.iter().position(|(_, c)| *c == k) {
                    liste.push(Brute::Cible(j));
                } else if let Some(&d) = doc_de.get(&k) {
                    liste.push(Brute::Doc(d));
                } else if let Some(c) = self.doc.cels.get(&k).filter(|c| c.fond.as_deref().is_some_and(|s| !s.peinture.raster.is_blank())) {
                    doc_de.insert(k, docs.len());
                    liste.push(Brute::Doc(docs.len()));
                    docs.push(Src::de(c));
                }
            }
            brutes.push(liste);
        }
        // Rien à suivre : aucune clé choisie n'a de clé avant elle.
        if brutes.iter().all(Vec::is_empty) {
            self.status = "Aplat intelligent : aucune clé avant les clés choisies, rien à suivre.".into();
            return;
        }
        let n_docs = docs.len();
        let mut cibles: Vec<Cible> = Vec::new();
        for (j, &(_, cel)) in cibles_cels.iter().enumerate() {
            let refs: Vec<Ref> = brutes[j]
                .iter()
                .map(|b| match b {
                    Brute::Doc(d) => Ref { idx: *d, retouche: false },
                    Brute::Cible(t) => Ref { idx: n_docs + *t, retouche: false },
                })
                .collect();
            // (Une cible sans source reste dans le lot : ses rangs doivent rester ceux de la chaîne.)
            let Some(c) = self.doc.cels.get(&cel) else { continue };
            let cible = cible_de(&mut self.aplat_intel.prochain, cel, c, refs);
            self.aplat_intel.gens.insert(cel, cible.gen);
            cibles.push(cible);
        }
        let n = cibles.len();
        self.lancer_fil(reglages, docs, cibles, None, "Aplat : colorier comme la clé précédente");
        self.status = format!("Aplat intelligent : {n} clé{} à colorier comme la précédente (calcul en arrière-plan).", if n > 1 { "s" } else { "" });
    }
}
