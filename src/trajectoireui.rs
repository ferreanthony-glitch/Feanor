//! L'outil Trajectoire à l'écran (Anthony, 2026-11-07 ; le modèle : `trajectoire.rs`).
//!
//! Avec l'outil, sur le calque courant :
//! * un trait sur la feuille **trace le chemin** du calque, depuis la tête de lecture (un second trait long le retrace, les
//!   marques restent) ;
//! * un petit trait **en travers du chemin pose une marque** ;
//! * un appui sur une marque la **glisse** le long du chemin ; tirée loin du chemin, elle est **arrachée** (retirée au lâcher) ;
//! * un appui sur le chemin le **tire** : il se tord là où on le prend, comme une corde (Anthony : « que la trajectoire soit éditable
//!   facilement ») ; Maj tenue, il se **déplace** en bloc ; Échap abandonne le geste en cours.
//!
//! Deux sortes de clés (réglages) :
//! * **clés d'interpolation** : une clé de position par image (`Layer::pos_keys`), le calque glisse le long de la courbe ; pendant
//!   un glisser, les clés sont reposées à chaque image d'écran, sans rejeu d'aucun dessin ;
//! * **clés d'images** : à chaque marque et à l'arrivée, **un dessin** (copie du dessin du départ, déplacé à sa place sur le
//!   chemin), exposé jusqu'au jalon suivant : la timeline se scinde comme pour toute clé de dessin, et chaque pose se redessine.
//!   Les dessins sont refaits au lâcher d'un geste (pas pendant) ; un dessin déjà posé est déplacé, pas refait : ce qu'on y a
//!   dessiné suit.
//!
//! **Recaler dans le temps** : glisser sur la timeline (ou au montage) une clé qui est un jalon de la trajectoire — la première,
//! la dernière, ou une marque — recale ce jalon (`Trajectoire::recaler`) et repose les clés entre le départ et la fin ; la
//! trajectoire reste. Poser, supprimer ou lisser une clé à la main, ou déplacer un calque animé avec Transformer, la **détache**
//! (`App::trajectoire_detacher`) : les clés restent, le chemin s'en va, la ligne d'état le dit.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Key, Pos2, RichText, Shape, Stroke, Vec2};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::history::Edit;
use crate::poskeys::PosKey;
use crate::trajectoire::{distance, Cles, DessinJalon, Trajectoire};
use crate::vector::Affine;

/// La teinte du chemin, de ses marques et des silhouettes : celle des poignées du calque.
pub(crate) const COULEUR: Color32 = crate::theme::POIGNEE_CALQUE;
/// La teinte des silhouettes du calque aux jalons (dans le plan à composer, clés d'interpolation).
pub(crate) const FANTOME: [u8; 3] = [120, 220, 140];
/// L'opacité des silhouettes.
pub(crate) const FANTOME_OPACITE: u8 = 70;
/// À moins de tant de pixels d'écran, l'appui prend une marque, ou le chemin.
pub(crate) const TOL_ECRAN: f32 = 10.0;
/// Une marque tirée à plus de tant de pixels d'écran du chemin est arrachée (retirée au lâcher).
pub(crate) const ARRACHE_ECRAN: f32 = 40.0;
/// La demi-longueur d'une marque à l'écran.
const DEMI_MARQUE: f32 = 7.0;
/// Ce qu'une commande des clés ajoute à sa ligne d'état quand elle a détaché une trajectoire.
pub(crate) const DETACHEE: &str = " La trajectoire du calque est détachée : le chemin s'en va, les clés restent.";

/// Un geste de l'outil en cours.
pub(crate) enum Geste {
    /// Un trait : le chemin, ou une marque, on le saura au lâcher.
    Trait { points: Vec<[f32; 2]> },
    /// La marque `i` prise : glissée le long du chemin, ou arrachée.
    Marque { i: usize, avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool, arrachee: bool },
    /// Le chemin pris en `fraction` : tiré là (tordu, comme une corde), ou, Maj tenue, déplacé en bloc.
    Chemin { depart: [f32; 2], fraction: f32, bloc: bool, avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool },
}

/// L'état de l'outil pour un projet.
#[derive(Default)]
pub(crate) struct Etat {
    pub(crate) geste: Option<Geste>,
    /// La sorte de clés proposée à la prochaine trajectoire tracée.
    pub(crate) cles_propose: Cles,
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

impl App {
    /// Combien de pixels d'écran vaut un pixel de la feuille, autour du point `p` de la feuille.
    fn trajectoire_echelle(&self, p: Pos2) -> f32 {
        let (a, b) = (self.stage_to_screen(p), self.stage_to_screen(p + vec2(1.0, 0.0)));
        (b - a).length().max(1e-3)
    }

    /// La marque sous le point `p` de la feuille (à `TOL_ECRAN` près), s'il y en a une ; sinon vrai si le chemin est sous `p`.
    fn trajectoire_sous(&self, p: Pos2) -> (Option<usize>, bool) {
        let Some(t) = self.layers().get(self.cur_layer).and_then(|l| l.trajectoire.as_ref()) else { return (None, false) };
        let tol = TOL_ECRAN / self.trajectoire_echelle(p);
        let q = [p.x, p.y];
        match t.marque_pres_de(q, tol) {
            Some(i) => (Some(i), true),
            None => (None, t.plus_proche(q).1 <= tol),
        }
    }

    /// Appui avec l'outil : une marque, le chemin (`maj` : pour le déplacer en bloc ; sinon pour le tirer), ou le début d'un trait.
    pub(crate) fn trajectoire_presser(&mut self, stage: Pos2, maj: bool) {
        self.end_stroke();
        self.playing = false;
        if self.content_refuses() {
            return;
        }
        if !self.layer().drawable() {
            self.layer_editable();
            return;
        }
        if self.layer().keys_follow_line() {
            self.status = "Ce calque de couleur suit son calque de trait : trace la trajectoire sur le calque de trait.".into();
            return;
        }
        let p = [stage.x, stage.y];
        let (marque, chemin) = self.trajectoire_sous(stage);
        let l = self.layer();
        if let (Some(avant), true) = (l.trajectoire.clone(), chemin) {
            let cles_avant = l.pos_keys.clone();
            let fraction = avant.plus_proche(p).0;
            let snap = self.snapshot();
            self.traj.geste = Some(match marque {
                Some(i) => Geste::Marque { i, avant, cles_avant, snap, bouge: false, arrachee: false },
                None => Geste::Chemin { depart: p, fraction, bloc: maj, avant, cles_avant, snap, bouge: false },
            });
            return;
        }
        self.traj.geste = Some(Geste::Trait { points: vec![p] });
    }

    /// Le pointeur bouge pendant un geste de l'outil ; vrai si un geste est en cours. En clés d'interpolation, les clés suivent à
    /// chaque image d'écran ; en clés d'images, les dessins attendent le lâcher (le chemin, lui, suit).
    pub(crate) fn trajectoire_glisser(&mut self, stage: Pos2) -> bool {
        let Some(mut g) = self.traj.geste.take() else { return false };
        let p = [stage.x, stage.y];
        let li = self.cur_layer;
        match &mut g {
            Geste::Trait { points } => {
                if points.last().is_none_or(|l| distance(*l, p) > 0.5) {
                    points.push(p);
                }
            }
            Geste::Marque { i, avant, bouge, arrachee, .. } => {
                let (f, d) = avant.plus_proche(p);
                let loin = d > ARRACHE_ECRAN / self.trajectoire_echelle(stage);
                let mut t = avant.clone();
                if loin {
                    t.retirer_marque(*i);
                } else {
                    t.deplacer_marque(*i, f);
                }
                *arrachee = loin;
                *bouge = true;
                self.trajectoire_poser_en_direct(li, t);
                self.status = if loin { "Marque arrachée : lâcher la retire.".into() } else { format!("Marque à {} % du chemin.", (f * 100.0).round()) };
            }
            Geste::Chemin { depart, fraction, bloc, avant, bouge, .. } => {
                let mut t = avant.clone();
                let (dx, dy) = (p[0] - depart[0], p[1] - depart[1]);
                if *bloc {
                    t.deplacer(dx, dy);
                } else {
                    let rayon = t.rayon_de_tirage();
                    t.tirer(*fraction, dx, dy, rayon);
                }
                *bouge = true;
                self.trajectoire_poser_en_direct(li, t);
                self.status = if *bloc { "Chemin déplacé en bloc (Maj).".into() } else { "Chemin tiré : relâche Maj pour le tordre, Maj pour le déplacer en bloc.".into() };
            }
        }
        self.traj.geste = Some(g);
        true
    }

    /// Pendant un geste : la trajectoire `t` posée sur le calque `li` ; en clés d'interpolation, ses clés aussi.
    fn trajectoire_poser_en_direct(&mut self, li: usize, t: Trajectoire) {
        let interpolation = t.cles == Cles::Interpolation;
        if let Some(l) = self.layers_mut().get_mut(li) {
            l.trajectoire = Some(t);
        }
        if interpolation {
            self.trajectoire_recalculer(li);
            self.renderer.invalidate_params();
        }
    }

    /// Lâcher : le trait devient marque ou chemin ; une marque glissée, arrachée, un chemin déplacé sont gardés, en une étape.
    pub(crate) fn trajectoire_lacher(&mut self) {
        let Some(g) = self.traj.geste.take() else { return };
        let li = self.cur_layer;
        if li >= self.layers().len() {
            return;
        }
        match g {
            Geste::Trait { points } => self.trajectoire_poser_le_trait(li, &points),
            Geste::Marque { snap, bouge, arrachee, .. } => {
                if arrachee {
                    self.trajectoire_recalculer(li);
                    self.trajectoire_valider(snap, "Retirer une marque");
                    self.status = self.trajectoire_resume(li, "Marque retirée");
                } else if bouge {
                    self.trajectoire_recalculer(li);
                    self.trajectoire_valider(snap, "Glisser une marque");
                }
            }
            Geste::Chemin { snap, bouge, bloc, .. } => {
                if bouge {
                    self.trajectoire_recalculer(li);
                    self.trajectoire_valider(snap, if bloc { "Déplacer la trajectoire" } else { "Tirer la trajectoire" });
                }
            }
        }
    }

    /// Un trait lâché : une marque s'il croise le chemin en étant court, sinon le chemin (neuf, ou retracé), sinon rien.
    fn trajectoire_poser_le_trait(&mut self, li: usize, points: &[[f32; 2]]) {
        let existante = self.layers()[li].trajectoire.clone();
        if let Some(t) = &existante {
            if let Some(f) = t.marque_de(points) {
                let snap = self.snapshot();
                let posee = self.layers_mut()[li].trajectoire.as_mut().and_then(|t| t.ajouter_marque(f)).is_some();
                if posee {
                    self.trajectoire_recalculer(li);
                    self.trajectoire_valider(snap, "Marque de trajectoire");
                    self.status = self.trajectoire_resume(li, "Marque posée");
                } else {
                    self.status = "Une marque est déjà là (ou c'est un bout du chemin) : glisse-la plutôt.".into();
                }
                return;
            }
        }
        match Trajectoire::nouvelle(points) {
            Some(mut t) => {
                let snap = self.snapshot();
                let nom = if existante.is_some() { "Retracer la trajectoire" } else { "Trajectoire" };
                match existante {
                    Some(e) => {
                        t.marques = e.marques;
                        t.depart = e.depart;
                        t.fin = e.fin;
                        t.temps = e.temps;
                        t.cles = e.cles;
                        t.dessins = e.dessins;
                    }
                    None => {
                        // La clé de départ démarre sur la tête de lecture.
                        t.depart = self.cur_frame;
                        t.cles = self.traj.cles_propose;
                    }
                }
                self.layers_mut()[li].trajectoire = Some(t);
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, nom);
                self.status = self.trajectoire_resume(li, if nom == "Trajectoire" { "Trajectoire tracée" } else { "Chemin retracé, marques gardées" });
            }
            None if existante.is_none() => {
                self.status = "Trajectoire : trace d'un trait le chemin du calque, puis de petits traits en travers pour les marques.".into();
            }
            None => {}
        }
    }

    // ------------------------------------------------------------------ les clés et les dessins

    /// Les clés du calque `li` du plan courant reposées d'après sa trajectoire (rien s'il n'en a pas).
    pub(crate) fn trajectoire_recalculer(&mut self, li: usize) {
        let pi = self.cur_panel;
        self.trajectoire_recalculer_dans(pi, li);
    }

    /// Les clés du calque `li` du plan `pi` reposées d'après sa trajectoire : en clés d'interpolation, une clé de position par
    /// image (la pose au départ reste celle du calque à l'image du départ) ; en clés d'images, aucune clé de position, un dessin
    /// à chaque jalon. Un chemin qui ne couvre aucune image (plan trop court) laisse le calque fixe.
    pub(crate) fn trajectoire_recalculer_dans(&mut self, pi: usize, li: usize) {
        let Some(p) = self.doc.panels.get(pi) else { return };
        let duree = p.frame_count();
        let Some(t) = p.layers.get(li).and_then(|l| l.trajectoire.clone()) else { return };
        let l = &mut self.doc.panels[pi].layers[li];
        let base = crate::poskeys::pose_at(&l.pos_keys, t.depart, l.transform);
        match t.cles {
            Cles::Interpolation => {
                l.pos_keys = t.cles(duree, base);
                if l.pos_keys.is_empty() {
                    l.transform = base;
                    l.fold_dz();
                }
                if !t.dessins.is_empty() {
                    self.trajectoire_defaire_les_dessins(pi, li);
                }
            }
            Cles::Images => {
                // Les clés de position s'en vont : le calque garde sa pose du départ, les dessins font le mouvement.
                l.pos_keys.clear();
                l.transform = base;
                l.fold_dz();
                self.trajectoire_poser_les_dessins(pi, li);
            }
        }
    }

    /// Le dessin `cel` copié et déplacé de `d` (pixels de la feuille) : un nouveau dessin. Vectoriel : chaque élément est
    /// translaté et le rendu exact est demandé au fil de travail (l'image en cache, décalée, le montre en attendant) ; bitmap : les
    /// pixels, décalés à l'entier ; l'aplat (strate Fond) suit.
    fn trajectoire_copie_decalee(&mut self, cel: CelId, d: [f32; 2]) -> CelId {
        let nouveau = self.doc.duplicate_cel(cel);
        let (dx, dy) = (d[0].round() as i64, d[1].round() as i64);
        let mut vectoriel = false;
        if let Some(c) = self.doc.cels.get_mut(&nouveau) {
            if let Some(v) = c.vector.as_mut() {
                let a = Affine::translate(vec2(d[0], d[1]));
                v.elements = v.elements.iter().map(|e| e.transformed(&a)).collect();
                vectoriel = true;
            }
            c.raster = c.raster.decalee(dx, dy);
            if let Some(f) = c.fond.as_mut() {
                f.peinture.raster = f.peinture.raster.decalee(dx, dy);
                f.coupe = f.coupe.decalee(dx, dy);
                f.peinture.version += 1;
            }
            c.version += 1;
        }
        if vectoriel {
            self.rejeu_cel(nouveau);
        }
        nouveau
    }

    /// Clés d'images : un dessin à chaque jalon (une marque, l'arrivée), exposé jusqu'au jalon suivant (le dernier, jusqu'à la fin
    /// du plan) ; le départ garde le dessin d'origine. Un dessin déjà posé par la trajectoire est déplacé de ce qu'il faut (ce
    /// qu'on y a dessiné suit) ; un jalon neuf reçoit une copie du dessin du départ. Faux, avec la raison dans la ligne d'état,
    /// s'il n'y a pas de dessin au départ.
    fn trajectoire_poser_les_dessins(&mut self, pi: usize, li: usize) -> bool {
        let duree = self.doc.panels[pi].frame_count();
        let Some(t) = self.doc.panels[pi].layers[li].trajectoire.clone() else { return false };
        let images = t.images_des_jalons(duree);
        if images.len() < 2 {
            return false;
        }
        let Some(source) = self.doc.panels[pi].layers[li].cel_at(t.depart) else {
            self.status = format!("Clés d'images : dessine d'abord à l'image {} du départ, la trajectoire en fera une copie à chaque marque.", t.depart + 1);
            return false;
        };
        let decalages = t.decalages();
        let mut dessins = Vec::with_capacity(images.len() - 1);
        for k in 1..images.len() {
            let d = decalages[k];
            let ancien = t.dessins.get(k - 1).filter(|a| self.doc.cels.contains_key(&a.dessin)).copied();
            let dessin = match ancien {
                Some(a) if (a.dx - d[0]).abs() < 0.5 && (a.dy - d[1]).abs() < 0.5 => a.dessin,
                Some(a) => self.trajectoire_copie_decalee(a.dessin, [d[0] - a.dx, d[1] - a.dy]),
                None => self.trajectoire_copie_decalee(source, d),
            };
            dessins.push(DessinJalon { dessin, dx: d[0], dy: d[1] });
        }
        let l = &mut self.doc.panels[pi].layers[li];
        if l.frames.len() < duree {
            l.frames.resize(duree, None);
        }
        for k in 0..images.len() {
            let cel = if k == 0 { source } else { dessins[k - 1].dessin };
            let fin = if k + 1 < images.len() { images[k + 1] } else { duree };
            for f in images[k]..fin.min(duree) {
                l.frames[f] = Some(cel);
            }
        }
        if let Some(t) = l.trajectoire.as_mut() {
            t.dessins = dessins;
        }
        true
    }

    /// Les dessins posés par la trajectoire en clés d'images quittent le calque : leurs images reprennent le dessin du départ.
    fn trajectoire_defaire_les_dessins(&mut self, pi: usize, li: usize) {
        let l = &mut self.doc.panels[pi].layers[li];
        let Some(t) = l.trajectoire.as_mut() else { return };
        let depart = t.depart;
        let poses: Vec<CelId> = t.dessins.drain(..).map(|d| d.dessin).collect();
        let source = l.cel_at(depart);
        for f in l.frames.iter_mut() {
            if f.is_some_and(|c| poses.contains(&c)) {
                *f = source;
            }
        }
        l.trim();
    }

    /// La clé de l'image `de` du calque `li` du plan `pi`, un jalon de sa trajectoire (le départ, une marque, la fin), glissée à
    /// l'image `vers` : le jalon est recalé (`Trajectoire::recaler`) et les clés reposées entre le départ et la fin. Faux si la
    /// clé n'est pas un jalon (ou pas de trajectoire).
    pub(crate) fn trajectoire_recaler_dans(&mut self, pi: usize, li: usize, de: usize, vers: usize) -> bool {
        let Some(p) = self.doc.panels.get(pi) else { return false };
        let duree = p.frame_count();
        let Some(t) = p.layers.get(li).and_then(|l| l.trajectoire.as_ref()) else { return false };
        let Some(k) = t.images_des_jalons(duree).iter().position(|&f| f == de) else { return false };
        let recale = self.doc.panels[pi].layers[li].trajectoire.as_mut().and_then(|t| t.recaler(k, vers, duree));
        if recale.is_none() {
            return false;
        }
        self.trajectoire_recalculer_dans(pi, li);
        true
    }

    /// `trajectoire_recaler_dans`, sur le plan courant.
    pub(crate) fn trajectoire_recaler(&mut self, li: usize, de: usize, vers: usize) -> bool {
        let pi = self.cur_panel;
        self.trajectoire_recaler_dans(pi, li, de, vers)
    }

    /// Une étape d'annulation au nom `nom`, depuis l'état `snap` d'avant le geste.
    fn trajectoire_valider(&mut self, snap: Edit, nom: &str) {
        self.modified = true;
        self.commit_named(snap, nom);
    }

    /// « Marque posée : 3 marques, le chemin dure de l'image 1 à 36. »
    fn trajectoire_resume(&self, li: usize, debut: &str) -> String {
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.as_ref()) else { return debut.to_string() };
        let images = t.images_des_jalons(self.frame_count());
        let n = t.marques.len();
        match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("{debut} : {n} marque{}, le chemin dure de l'image {} à l'image {}.", pluriel(n), a + 1, b + 1),
            _ => format!("{debut} : {n} marque{} ; le plan s'arrête avant l'image {} du départ, le calque ne bouge pas encore.", pluriel(n), t.depart + 1),
        }
    }

    /// Échap pendant un geste de l'outil : tout revient comme avant le geste ; vrai si la touche a servi.
    pub(crate) fn trajectoire_touche(&mut self, key: Key) -> bool {
        if key != Key::Escape || self.tool != Tool::Trajectoire {
            return false;
        }
        let Some(g) = self.traj.geste.take() else { return false };
        let li = self.cur_layer;
        if let Geste::Marque { avant, cles_avant, .. } | Geste::Chemin { avant, cles_avant, .. } = g {
            if let Some(l) = self.layers_mut().get_mut(li) {
                l.trajectoire = Some(avant);
                l.pos_keys = cles_avant;
            }
            self.renderer.invalidate_params();
        }
        self.status = "Geste de trajectoire abandonné.".into();
        true
    }

    /// Les clés du calque `li` viennent d'être retouchées à la main (`pourquoi`) : sa trajectoire ne les commande plus, elle s'en va.
    /// Vrai si une trajectoire a été détachée (la ligne d'état le dit ; un appelant qui écrit la sienne ensuite y ajoute `DETACHEE`).
    pub(crate) fn trajectoire_detacher(&mut self, li: usize, pourquoi: &str) -> bool {
        let Some(l) = self.layers_mut().get_mut(li) else { return false };
        if l.trajectoire.take().is_none() {
            return false;
        }
        self.status = format!("Trajectoire détachée ({pourquoi}) : les clés de position restent, à ta main.");
        true
    }

    /// Le départ de la trajectoire du calque courant passe à la tête de lecture (bouton des réglages) ; une étape.
    pub(crate) fn trajectoire_depart_ici(&mut self) {
        self.trajectoire_borne_ici(true);
    }

    /// La fin de la trajectoire du calque courant passe à la tête de lecture (bouton des réglages) ; une étape.
    pub(crate) fn trajectoire_fin_ici(&mut self) {
        self.trajectoire_borne_ici(false);
    }

    fn trajectoire_borne_ici(&mut self, depart: bool) {
        let (pi, li, f) = (self.cur_panel, self.cur_layer, self.cur_frame);
        let duree = self.frame_count();
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.as_ref()) else { return };
        let images = t.images_des_jalons(duree);
        let k = if depart { 0 } else { images.len().saturating_sub(1) };
        if images.get(k) == Some(&f) {
            return;
        }
        let snap = self.snapshot();
        if self.doc.panels[pi].layers[li].trajectoire.as_mut().and_then(|t| t.recaler(k, f, duree)).is_none() {
            self.status = "Le plan est trop court pour recaler la trajectoire ici.".into();
            return;
        }
        self.trajectoire_recalculer(li);
        self.trajectoire_valider(snap, if depart { "Départ de la trajectoire" } else { "Fin de la trajectoire" });
        self.status = self.trajectoire_resume(li, if depart { "Départ à la tête de lecture" } else { "Fin à la tête de lecture" });
    }

    /// Le curseur de l'outil au point `p` de l'écran : la main sur une marque ou le chemin, la croix ailleurs.
    pub(crate) fn trajectoire_curseur(&self, p: Pos2) -> CursorIcon {
        match &self.traj.geste {
            Some(Geste::Marque { .. } | Geste::Chemin { .. }) => return CursorIcon::Grabbing,
            Some(Geste::Trait { .. }) => return CursorIcon::Crosshair,
            None => {}
        }
        let stage = self.view.to_canvas(self.canvas_rect, self.canvas_size(), p);
        match self.trajectoire_sous(stage) {
            (Some(_), _) => CursorIcon::Grab,
            (None, true) => CursorIcon::PointingHand,
            _ => CursorIcon::Crosshair,
        }
    }

    // ------------------------------------------------------------------ dessin par-dessus

    /// Le chemin, sa barre de départ, sa flèche d'arrivée, ses marques avec l'image où elles tombent, le point de l'image courante,
    /// et le trait en cours.
    pub(crate) fn paint_trajectoire(&self, painter: &egui::Painter) {
        if self.tool != Tool::Trajectoire || self.in_card {
            return;
        }
        let ecran = |p: [f32; 2]| self.stage_to_screen(pos2(p[0], p[1]));
        if let Some(Geste::Trait { points }) = &self.traj.geste {
            if points.len() >= 2 {
                let pts: Vec<Pos2> = points.iter().map(|p| ecran(*p)).collect();
                painter.add(Shape::line(pts, Stroke::new(1.5_f32, COULEUR.gamma_multiply(0.7))));
            }
        }
        let Some(l) = self.layers().get(self.cur_layer) else { return };
        let Some(t) = &l.trajectoire else { return };
        let duree = self.frame_count();
        let pts: Vec<Pos2> = t.points.iter().map(|p| ecran(*p)).collect();
        painter.add(Shape::line(pts.clone(), Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
        painter.add(Shape::line(pts, Stroke::new(1.5_f32, COULEUR)));
        // La direction du chemin à l'écran, à la fraction `f`.
        let direction = |f: f32| -> Vec2 {
            let (a, b) = (ecran(t.point_a((f - 0.01).max(0.0))), ecran(t.point_a((f + 0.01).min(1.0))));
            let d = b - a;
            if d.length() > 1e-3 { d.normalized() } else { vec2(1.0, 0.0) }
        };
        let normale = |d: Vec2| vec2(-d.y, d.x);
        // La marque prise, ou survolée.
        let tenue = match &self.traj.geste {
            Some(Geste::Marque { i, arrachee: false, .. }) => Some(*i),
            Some(Geste::Marque { arrachee: true, .. }) => None,
            _ => self.hover_canvas.and_then(|h| self.trajectoire_sous(h).0),
        };
        let images = t.images_des_jalons(duree);
        let jalons = t.jalons();
        let dernier = jalons.len() - 1;
        for (k, &f) in jalons.iter().enumerate() {
            let c = ecran(t.point_a(f));
            let d = direction(f);
            let n = normale(d);
            if k == 0 {
                // Le départ : une barre.
                painter.line_segment([c - n * (DEMI_MARQUE + 2.0), c + n * (DEMI_MARQUE + 2.0)], Stroke::new(2.5_f32, COULEUR));
            } else if k == dernier {
                // L'arrivée : une pointe de flèche.
                let (g, h) = (c - d * 11.0 + n * 6.0, c - d * 11.0 - n * 6.0);
                painter.add(Shape::line(vec![g, c, h], Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
                painter.add(Shape::line(vec![g, c, h], Stroke::new(2.0_f32, COULEUR)));
            } else {
                let i = k - 1;
                let prise = tenue == Some(i);
                let (a, b) = (c - n * DEMI_MARQUE, c + n * DEMI_MARQUE);
                painter.line_segment([a, b], Stroke::new(if prise { 5.0_f32 } else { 3.5_f32 }, Color32::from_black_alpha(90)));
                painter.line_segment([a, b], Stroke::new(2.0_f32, if prise { Color32::WHITE } else { COULEUR }));
            }
            if let Some(img) = images.get(k) {
                let cote = if n.y <= 0.0 { n } else { -n };
                painter.text(c + cote * (DEMI_MARQUE + 4.0), Align2::CENTER_CENTER, format!("{}", img + 1), FontId::proportional(10.0), COULEUR);
            }
        }
        // Où en est le calque à l'image courante.
        if let Some(f) = t.fraction_a_l_image(self.cur_frame, duree) {
            let c = ecran(t.point_a(f));
            painter.circle(c, 4.0, COULEUR, Stroke::new(1.5_f32, Color32::from_black_alpha(140)));
        }
    }

    // ------------------------------------------------------------------ réglages

    /// Réglages de l'outil (panneau Réglages, fenêtre du clic droit) : la sorte des clés, le départ et la fin, les commandes de la
    /// trajectoire du calque courant.
    pub(crate) fn trajectoire_options_ui(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let duree = self.frame_count();
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.clone()) else {
            ui.label(
                RichText::new(
                    "Trace le chemin du calque d'un trait : le calque le suit depuis la tête de lecture jusqu'à la fin du plan, en partant \
                     de là où il est. Puis de petits traits en travers du chemin : les marques. Le temps est le même entre deux marques ; \
                     serrées, le calque va lentement ; espacées, il va vite. Sur la timeline, glisser la première clé, la dernière ou une \
                     marque les recale dans le temps.",
                )
                .small()
                .weak(),
            );
            let mut cles = self.traj.cles_propose;
            if cles_ui(ui, &mut cles) {
                self.traj.cles_propose = cles;
            }
            return;
        };
        let n = t.marques.len();
        let images = t.images_des_jalons(duree);
        ui.label(match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!(
                "{n} marque{} · le chemin dure de l'image {} à l'image {} · {}.",
                pluriel(n),
                a + 1,
                b + 1,
                match t.cles {
                    Cles::Interpolation => format!("{} clés de position", self.layers()[li].pos_keys.len()),
                    Cles::Images => format!("{} dessin{} posé{}", t.dessins.len(), pluriel(t.dessins.len()), pluriel(t.dessins.len())),
                }
            ),
            _ => format!("{n} marque{} · le plan s'arrête avant l'image {} du départ : le calque ne bouge pas encore.", pluriel(n), t.depart + 1),
        });
        ui.horizontal_wrapped(|ui| {
            let f = self.cur_frame;
            if images.first().is_some_and(|&a| a != f) && ui.button("Partir de la tête de lecture").on_hover_text("La clé de départ passe à l'image courante ; les clés d'entre sont recalées.").clicked() {
                self.trajectoire_depart_ici();
            }
            if images.last().is_some_and(|&b| b != f) && ui.button("Finir à la tête de lecture").on_hover_text("La clé de fin passe à l'image courante ; les clés d'entre sont recalées.").clicked() {
                self.trajectoire_fin_ici();
            }
            if t.temps.is_some() && ui.button("Temps égaux entre les marques").on_hover_text("Oublie les marques recalées à la main : le temps est de nouveau partagé en parts égales.").clicked() {
                let snap = self.snapshot();
                if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                    tr.temps_egaux();
                }
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, "Temps égaux entre les marques");
            }
        });
        let mut cles = t.cles;
        if cles_ui(ui, &mut cles) {
            let snap = self.snapshot();
            if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                tr.cles = cles;
            }
            self.traj.cles_propose = cles;
            self.trajectoire_recalculer(li);
            self.trajectoire_valider(snap, "Sorte des clés de la trajectoire");
        }
        ui.horizontal_wrapped(|ui| {
            if n > 0 && ui.button("Retirer les marques").on_hover_text("Le chemin reste, à vitesse constante.").clicked() {
                let snap = self.snapshot();
                if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                    tr.marques.clear();
                    tr.temps = None;
                }
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, "Retirer les marques");
            }
            if ui.button("Détacher").on_hover_text("Les clés (ou les dessins) restent tels quels, le chemin s'en va : tu les retouches à la main.").clicked() {
                let snap = self.snapshot();
                self.layers_mut()[li].trajectoire = None;
                self.trajectoire_valider(snap, "Détacher la trajectoire");
                self.status = "Trajectoire détachée : ce qu'elle a posé reste, à ta main.".into();
            }
            if ui.button("Effacer").on_hover_text("Le chemin et ce qu'il a posé (clés, dessins) s'en vont ; le calque garde la pose du départ.").clicked() {
                let snap = self.snapshot();
                let pi = self.cur_panel;
                self.trajectoire_defaire_les_dessins(pi, li);
                let l = &mut self.layers_mut()[li];
                l.transform = crate::poskeys::pose_at(&l.pos_keys, t.depart, l.transform);
                l.pos_keys.clear();
                l.fold_dz();
                l.trajectoire = None;
                self.trajectoire_valider(snap, "Effacer la trajectoire");
                self.status = "Trajectoire effacée : le calque reste où il était au départ.".into();
            }
        });
        ui.label(
            RichText::new(
                "Tirer sur le chemin le tord là où on le prend, comme une corde (Maj : le déplacer en bloc). Glisser une marque le long du \
                 chemin la déplace ; la tirer loin du chemin la retire. Un trait long retrace le chemin, les marques restent. Sur la \
                 timeline, glisser la première clé, la dernière ou une marque les recale dans le temps. Poser, supprimer ou lisser une clé \
                 à la main détache la trajectoire.",
            )
            .small()
            .weak(),
        );
    }
}

/// Les deux sortes de clés : interpolées (le calque glisse le long du chemin) ou par images (un dessin à chaque marque) ; vrai si
/// le choix a changé.
fn cles_ui(ui: &mut egui::Ui, cles: &mut Cles) -> bool {
    let avant = *cles;
    ui.horizontal(|ui| {
        ui.radio_value(cles, Cles::Interpolation, Cles::Interpolation.label()).on_hover_text("Une clé de position par image : le calque glisse le long du chemin, à la vitesse que donnent les marques.");
        ui.radio_value(cles, Cles::Images, Cles::Images.label()).on_hover_text("Un dessin à chaque marque, copie du dessin du départ déplacé sur le chemin, exposé jusqu'à la marque suivante : la timeline se scinde, chaque pose se redessine.");
    });
    *cles != avant
}
