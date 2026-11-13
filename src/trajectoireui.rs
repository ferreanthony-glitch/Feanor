//! L'outil Trajectoire à l'écran (Anthony, 2026-11-07 ; le modèle : `trajectoire.rs`).
//!
//! Avec l'outil, sur le calque courant :
//! * un trait sur la feuille **trace le chemin** du calque, depuis la tête de lecture (un second trait long le retrace, les
//!   marques restent) ;
//! * un petit trait **en travers du chemin pose une marque** ;
//! * un appui sur une marque la **glisse** le long du chemin ; tirée loin du chemin, elle est **arrachée** (retirée au lâcher) ;
//! * un appui sur le chemin le **tire** : il se tord là où on le prend, comme une corde (Anthony : « que la trajectoire soit éditable
//!   facilement ») ; Maj tenue, il se **déplace** en bloc ; Échap abandonne le geste en cours ;
//! * Maj + un appui sur **un bout** du chemin (son début ou sa fin), puis un trait : il **prolonge** le chemin, devant ou
//!   derrière (partie 151, `trajectoireprolonge.rs`) ;
//! * le curseur « Lissage » des réglages adoucit le chemin, sans perdre le trait tracé (partie 151, `trajectoirelisse.rs`).
//!
//! Deux sortes de clés (réglages) :
//! * **clés d'interpolation** : une clé de position par image (`Layer::pos_keys`), le calque glisse le long de la courbe ; pendant
//!   un glisser, les clés sont reposées à chaque image d'écran, sans rejeu d'aucun dessin ;
//! * **clés d'images** : à chaque marque et à l'arrivée, **un dessin** (copie du dessin du départ, déplacé à sa place sur le
//!   chemin), exposé jusqu'au jalon suivant : la timeline se scinde comme pour toute clé de dessin, et chaque pose se redessine.
//!   Les dessins sont refaits au lâcher d'un geste (pas pendant) ; un dessin déjà posé est déplacé, pas refait : ce qu'on y a
//!   dessiné suit.
//!
//! **Le dessin tourne avec le chemin** (réglage, Anthony 2026-11-08) : il pivote autour du point d'ancrage pour regarder où va le
//! chemin, en gardant son orientation du départ ; en clés d'images, chaque copie est tournée (`Raster::transformee`, éléments
//! vectoriels transformés) et refaite depuis le dessin d'origine quand son angle change. **Insérer ou retirer des images** décale la
//! trajectoire avec les clés et les dessins (`Layer::shift_frames` → `Trajectoire::decaler_images`) : la fin est posée dès le tracé.
//!
//! **Recaler dans le temps** : glisser sur la timeline (ou au montage) une clé qui est un jalon de la trajectoire — la première,
//! la dernière, ou une marque — recale ce jalon (`Trajectoire::recaler`) et repose les clés entre le départ et la fin. **La
//! trajectoire ne s'en va jamais d'elle-même** (Anthony : « garder la trajectoire visible et toujours éditable ») : Suppr sur une
//! marque la retire, le départ et la fin restent ; poser ou lisser une clé à la main est refusé avec la raison ; Transformer déplace
//! le calque et son chemin ensemble. Seuls « Détacher », « Effacer », « Supprimer toutes les clés », couper ou réunir un plan l'enlèvent.

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Key, Pos2, RichText, Shape, Stroke, Vec2};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::history::Edit;
use crate::poskeys::PosKey;
use crate::document::LayerTransform;
use crate::trajectoire::{distance, Ancre, Cles, DessinJalon, Trajectoire};
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
/// Ce que disent les commandes des clés de position sur un calque à trajectoire : elles n'y touchent pas.
pub(crate) const CLES_PAR_LA_TRAJECTOIRE: &str = "Ce calque a une trajectoire : c'est elle qui pose ses clés. Tire le chemin, glisse une marque, ou « Détacher » dans les réglages de l'outil Trajectoire pour les reprendre à la main.";

/// Un geste de l'outil en cours.
pub(crate) enum Geste {
    /// Un trait : le chemin, ou une marque, on le saura au lâcher.
    Trait { points: Vec<[f32; 2]> },
    /// La marque `i` prise : glissée le long du chemin, ou arrachée.
    Marque { i: usize, avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool, arrachee: bool },
    /// Le chemin pris en `fraction` : tiré là (tordu, comme une corde), ou, Maj tenue, déplacé en bloc.
    Chemin { depart: [f32; 2], fraction: f32, bloc: bool, avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool },
    /// Maj + appui sur un bout du chemin, puis un trait : il prolonge le chemin (`devant` : depuis son début), au lâcher
    /// (`trajectoireprolonge.rs`, partie 151).
    Prolonger { points: Vec<[f32; 2]>, devant: bool },
}

/// L'état de l'outil pour un projet.
#[derive(Default)]
pub(crate) struct Etat {
    pub(crate) geste: Option<Geste>,
    /// La sorte de clés proposée à la prochaine trajectoire tracée (d'origine : clés d'images).
    pub(crate) cles_propose: Cles,
    /// « Le dessin tourne avec le chemin », proposé à la prochaine trajectoire tracée (d'origine : non).
    pub(crate) tourne_propose: bool,
    /// Le prochain appui sur le dessin choisit le point d'ancrage (bouton « Un point du dessin : cliquer »).
    pub(crate) choisir_ancre: bool,
    /// Le geste en cours dans la vue Espace 3D (`trajectoire3d.rs`, partie 149).
    pub(crate) espace: Option<crate::trajectoire3d::GesteEspace>,
    /// Le lissage du curseur « Lissage » (de 0 à 1, `trajectoirelisse.rs`, partie 151) : celui des prochains tracés.
    pub(crate) lissage: f32,
}

/// Le texte du réglage « le dessin tourne avec le chemin ».
pub(crate) const TOURNE: &str = "Le dessin tourne avec le chemin";
const TOURNE_AIDE: &str = "Le dessin pivote, autour du point qui suit le chemin, pour regarder où va le chemin, comme une voiture dans un virage ; au départ il garde son orientation. Clés d'images : chaque copie est tournée.";

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
        // (Une trajectoire tracée dans l'espace ne se prend pas sur la feuille : ses points ne sont pas des pixels de la feuille.)
        let Some(t) = self.layers().get(self.cur_layer).and_then(|l| l.trajectoire.as_ref()).filter(|t| t.sur_la_feuille()) else { return (None, false) };
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
        // (Le chemin se trace sur la feuille ; la pose d'un calque accroché se lit dans le cadre de la caméra, `accroche.rs`.)
        if self.layer().accroche {
            self.status = "Ce calque est accroché à la caméra : décroche-le pour lui tracer une trajectoire (elle se trace sur la feuille).".into();
            return;
        }
        // Une trajectoire tracée dans l'espace (partie 149) se retouche dans la vue Espace 3D : ici elle est seulement montrée.
        if self.layer().trajectoire.as_ref().is_some_and(|t| !t.sur_la_feuille()) {
            self.status = crate::trajectoire3d::SUR_LA_FEUILLE.into();
            return;
        }
        let p = [stage.x, stage.y];
        // Le point d'ancrage se choisit d'un clic sur le dessin (bouton des réglages).
        if std::mem::take(&mut self.traj.choisir_ancre) {
            let li = self.cur_layer;
            if let (true, Some(q)) = (self.layer().trajectoire.is_some(), self.to_layer_opt(stage)) {
                let snap = self.snapshot();
                if let Some(t) = self.layers_mut()[li].trajectoire.as_mut() {
                    t.ancre = Ancre::Point { x: q.x, y: q.y };
                }
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, "Point d'ancrage de la trajectoire");
                self.status = "Point d'ancrage posé : c'est lui qui suit le chemin.".into();
            }
            return;
        }
        // Maj + appui sur un bout du chemin : le trait qui suit le prolonge (partie 151).
        if let Some(devant) = self.trajectoire_bout_sous(stage).filter(|_| maj) {
            self.traj.geste = Some(Geste::Prolonger { points: vec![p], devant });
            self.status = crate::trajectoireprolonge::PROLONGER.into();
            return;
        }
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
            Geste::Trait { points } | Geste::Prolonger { points, .. } => {
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
                self.status = if *bloc { "Chemin déplacé en bloc (Maj).".into() } else { "Chemin tiré : il se tord là où on le prend. Maj à l'appui : le déplacer en bloc ; Maj sur un bout : le prolonger.".into() };
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
            Geste::Prolonger { points, devant } => self.trajectoire_prolonger(li, &points, devant),
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
                        t.ancre = e.ancre;
                        t.decalage_depart = e.decalage_depart;
                        t.source = e.source;
                        t.dessins = e.dessins;
                        t.tourne = e.tourne;
                        // (Retracé : le chemin garde son lissage, partie 151.)
                        t.lissage = e.lissage;
                    }
                    None => {
                        // La clé de départ démarre sur la tête de lecture ; la fin est posée sur la dernière image du plan (une fin
                        // posée suit les images insérées ou retirées, comme les clés ; sans fin posée, le chemin s'étirerait jusqu'à
                        // la nouvelle dernière image).
                        t.depart = self.cur_frame;
                        let duree = self.frame_count();
                        t.fin = (duree >= t.depart + 2).then(|| duree - 1);
                        t.cles = self.traj.cles_propose;
                        t.tourne = self.traj.tourne_propose;
                        // (Le lissage du curseur s'applique au nouveau tracé, partie 151.)
                        t.lissage = self.traj.lissage;
                    }
                }
                let lissage = t.lissage;
                t.lisser_a(lissage);
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
    /// image (la pose propre du calque, celle qu'il a à l'image du départ moins ce que le dernier calcul y avait ajouté, reste la
    /// base) ; en clés d'images, aucune clé de position, un dessin à chaque jalon. Le point d'ancrage choisi (`Ancre`) est ce qui
    /// suit le chemin. Un chemin qui ne couvre aucune image (plan trop court) laisse le calque fixe.
    pub(crate) fn trajectoire_recalculer_dans(&mut self, pi: usize, li: usize) {
        let Some(p) = self.doc.panels.get(pi) else { return };
        let duree = p.frame_count();
        let Some(t) = p.layers.get(li).and_then(|l| l.trajectoire.clone()) else { return };
        // Une trajectoire tracée dans l'espace (partie 149) : la carte se déplace dans l'espace, `trajectoire3d.rs` pose ses clés.
        if !t.sur_la_feuille() {
            return self.trajectoire_recalculer_dans_l_espace(pi, li);
        }
        let l = &self.doc.panels[pi].layers[li];
        let posee = crate::poskeys::pose_at(&l.pos_keys, t.depart, l.transform);
        let base = LayerTransform { x: posee.x - t.decalage_depart[0], y: posee.y - t.decalage_depart[1], ..posee };
        let ancre = self.trajectoire_point_d_ancrage(pi, li, &t, base);
        let pivot = self.doc.stage_center();
        // (Plan en Espace 3D : un calque redevenu fixe garde sa profondeur, même derrière la caméra d'origine du plan.)
        let libre = self.doc.plan_en_espace(pi);
        let l = &mut self.doc.panels[pi].layers[li];
        match t.cles {
            Cles::Interpolation => {
                l.pos_keys = t.cles(duree, base, ancre, [pivot.x, pivot.y]);
                let premiere = l.pos_keys.first().map(|k| [k.t.x - base.x, k.t.y - base.y]).unwrap_or([0.0, 0.0]);
                if l.pos_keys.is_empty() {
                    l.transform = base;
                    l.fold_dz_selon(libre);
                }
                if let Some(tr) = l.trajectoire.as_mut() {
                    tr.decalage_depart = premiere;
                }
                if !t.dessins.is_empty() {
                    self.trajectoire_defaire_les_dessins(pi, li);
                }
            }
            Cles::Images => {
                // Les clés de position s'en vont : le calque garde sa pose propre, les dessins font le mouvement.
                l.pos_keys.clear();
                l.transform = base;
                l.fold_dz_selon(libre);
                if let Some(tr) = l.trajectoire.as_mut() {
                    tr.decalage_depart = [0.0, 0.0];
                }
                self.trajectoire_poser_les_dessins(pi, li, ancre, base);
            }
        }
    }

    /// La boîte exacte du dessin `cel` (coordonnées du dessin) : les traits d'un dessin vectoriel, les pixels dessinés d'un bitmap
    /// (feuille et hors-feuille). Rien pour un dessin vide.
    fn boite_du_dessin(&self, cel: CelId) -> Option<([f32; 2], [f32; 2])> {
        let c = self.doc.cels.get(&cel)?;
        if let Some(v) = &c.vector {
            let r = v.elements.iter().fold(egui::Rect::NOTHING, |r, e| r.union(e.bounds()));
            return (r.is_finite() && r.width() >= 0.0).then_some(([r.min.x, r.min.y], [r.max.x, r.max.y]));
        }
        let (mut min, mut max) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
        let r = &c.raster;
        let mut voir = |ox: i64, oy: i64, t: &crate::raster::Tile| {
            for (i, px) in t.iter().enumerate() {
                if px.a() != 0 {
                    let (x, y) = ((ox + (i % crate::raster::TILE) as i64) as f32, (oy + (i / crate::raster::TILE) as i64) as f32);
                    min = [min[0].min(x), min[1].min(y)];
                    max = [max[0].max(x + 1.0), max[1].max(y + 1.0)];
                }
            }
        };
        for (idx, t) in r.tiles.iter().enumerate() {
            if let Some(t) = t {
                let (ox, oy) = r.tile_origin(idx);
                voir(ox, oy, t);
            }
        }
        for (&(tx, ty), t) in &r.outer {
            voir(tx as i64 * crate::raster::TILE as i64, ty as i64 * crate::raster::TILE as i64, t);
        }
        (min[0].is_finite() && max[0].is_finite()).then_some((min, max))
    }

    /// Le point de la feuille qui suit le chemin, pour la trajectoire `t` du calque `li` du plan `pi` sous la pose propre `base` : le
    /// départ du trait, ou un point du dessin du départ (son bas, son centre…, ou le point cliqué) mis sur la feuille par la pose.
    fn trajectoire_point_d_ancrage(&self, pi: usize, li: usize, t: &Trajectoire, base: LayerTransform) -> [f32; 2] {
        // (Le départ du trait, ou l'ancien départ d'un chemin prolongé par son début : `Trajectoire::point_de_reference`.)
        let depart = t.point_de_reference();
        let affine = base.affine(self.doc.stage_center());
        let sur_la_feuille = |p: [f32; 2]| {
            let q = affine.apply(pos2(p[0], p[1]));
            [q.x, q.y]
        };
        match t.ancre {
            Ancre::Depart => depart,
            Ancre::Point { x, y } => sur_la_feuille([x, y]),
            a => {
                let Some(l) = self.doc.panels.get(pi).and_then(|p| p.layers.get(li)) else { return depart };
                let Some(cel) = t.source.filter(|c| self.doc.cels.contains_key(c)).or_else(|| l.cel_at(t.depart)) else { return depart };
                match self.boite_du_dessin(cel).and_then(|(min, max)| a.dans_la_boite(min, max)) {
                    Some(p) => sur_la_feuille(p),
                    None => depart,
                }
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

    /// Le dessin `cel` copié et passé par l'affinité `a` (coordonnées du dessin : tourné autour du point d'ancrage, puis déplacé) : un
    /// nouveau dessin. Une simple translation : `trajectoire_copie_decalee`. Vectoriel : chaque élément est transformé et le rendu exact
    /// est demandé au fil de travail ; bitmap : les pixels rééchantillonnés (`Raster::transformee`) ; l'aplat (strate Fond) suit.
    fn trajectoire_copie_posee(&mut self, cel: CelId, a: &Affine) -> CelId {
        if a.is_translation() {
            return self.trajectoire_copie_decalee(cel, [a.tx, a.ty]);
        }
        let nouveau = self.doc.duplicate_cel(cel);
        let mut vectoriel = false;
        if let Some(c) = self.doc.cels.get_mut(&nouveau) {
            if let Some(v) = c.vector.as_mut() {
                v.elements = v.elements.iter().map(|e| e.transformed(a)).collect();
                vectoriel = true;
            }
            c.raster = c.raster.transformee(a);
            if let Some(f) = c.fond.as_mut() {
                f.peinture.raster = f.peinture.raster.transformee(a);
                f.coupe = f.coupe.transformee(a);
                f.peinture.version += 1;
            }
            c.version += 1;
        }
        if vectoriel {
            self.rejeu_cel(nouveau);
        }
        nouveau
    }

    /// Clés d'images : un dessin à chaque jalon (le départ, chaque marque, l'arrivée), exposé jusqu'au jalon suivant (le dernier,
    /// jusqu'à la fin du plan). Le dessin d'origine (celui du départ, noté dans `source`) sert tel quel quand il n'a pas à bouger
    /// (ancrage « là où part le trait »), sinon chaque jalon reçoit une copie déplacée pour que le point d'ancrage `ancre` soit sur
    /// le chemin — et tournée autour de lui si le dessin tourne avec le chemin (`base` : la pose propre du calque, qui encadre le
    /// calcul dans les coordonnées du dessin). Un dessin déjà posé par la trajectoire est déplacé de ce qu'il faut (ce qu'on y a
    /// dessiné suit) ; s'il doit tourner autrement, il est refait depuis le dessin d'origine (tourner une copie déjà tournée
    /// l'abîmerait). Faux, avec la raison dans la ligne d'état, s'il n'y a pas de dessin au départ.
    fn trajectoire_poser_les_dessins(&mut self, pi: usize, li: usize, ancre: [f32; 2], base: LayerTransform) -> bool {
        let duree = self.doc.panels[pi].frame_count();
        let Some(t) = self.doc.panels[pi].layers[li].trajectoire.clone() else { return false };
        let images = t.images_des_jalons(duree);
        if images.len() < 2 {
            return false;
        }
        let Some(source) = t.source.filter(|c| self.doc.cels.contains_key(c)).or_else(|| self.doc.panels[pi].layers[li].cel_at(t.depart)) else {
            self.status = format!("Clés d'images : dessine d'abord à l'image {} du départ, la trajectoire en fera une copie à chaque marque.", t.depart + 1);
            return false;
        };
        let poses = t.poses(ancre);
        // Une pose tournée, dans les coordonnées du dessin : sur la feuille, tourner autour du point d'ancrage puis déplacer, le tout
        // encadré par la pose propre du calque (sous une pose sans échelle ni rotation, ce sont les mêmes coordonnées).
        let affine_base = base.affine(self.doc.stage_center());
        let inverse = affine_base.inverse().unwrap_or(Affine::IDENTITY);
        let dans_le_dessin = |d: [f32; 2], angle: f32| -> Affine {
            let feuille = Affine::rotate_about(pos2(ancre[0], ancre[1]), angle.to_radians()).then(&Affine::translate(vec2(d[0], d[1])));
            affine_base.then(&feuille).then(&inverse)
        };
        let proche = |a: f32, b: f32| (a - b).abs() < 0.5;
        let meme_angle = |a: f32, b: f32| (a - b).abs() < 0.01;
        let mut dessins = Vec::with_capacity(images.len());
        for (k, &(d, angle)) in poses.iter().enumerate().take(images.len()) {
            let ancien = t.dessins.get(k).filter(|a| self.doc.cels.contains_key(&a.dessin)).copied();
            let dessin = match ancien {
                Some(a) if proche(a.dx, d[0]) && proche(a.dy, d[1]) && meme_angle(a.angle, angle) => a.dessin,
                Some(a) if a.dessin != source && meme_angle(a.angle, angle) => self.trajectoire_copie_decalee(a.dessin, [d[0] - a.dx, d[1] - a.dy]),
                _ if angle.abs() < 0.01 && d[0].abs() < 0.5 && d[1].abs() < 0.5 => source,
                _ if angle.abs() < 0.01 => self.trajectoire_copie_decalee(source, d),
                _ => self.trajectoire_copie_posee(source, &dans_le_dessin(d, angle)),
            };
            dessins.push(DessinJalon { dessin, dx: d[0], dy: d[1], angle });
        }
        let l = &mut self.doc.panels[pi].layers[li];
        if l.frames.len() < duree {
            l.frames.resize(duree, None);
        }
        for k in 0..images.len() {
            let fin = if k + 1 < images.len() { images[k + 1] } else { duree };
            for f in images[k]..fin.min(duree) {
                l.frames[f] = Some(dessins[k].dessin);
            }
        }
        if let Some(t) = l.trajectoire.as_mut() {
            t.source = Some(source);
            t.dessins = dessins;
        }
        true
    }

    /// Les dessins posés par la trajectoire en clés d'images quittent le calque : leurs images reprennent le dessin d'origine.
    /// (Sert aussi à une trajectoire tracée dans l'espace, `trajectoire3d.rs`, partie 152.)
    pub(crate) fn trajectoire_defaire_les_dessins(&mut self, pi: usize, li: usize) {
        let l = &mut self.doc.panels[pi].layers[li];
        let (depart, source_notee, poses) = match l.trajectoire.as_mut() {
            Some(t) => (t.depart, t.source.take(), t.dessins.drain(..).map(|d| d.dessin).collect::<Vec<CelId>>()),
            None => return,
        };
        let source = source_notee.filter(|c| self.doc.cels.contains_key(c)).or_else(|| l.cel_at(depart));
        let poses: Vec<CelId> = poses.into_iter().filter(|c| Some(*c) != source).collect();
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
    pub(crate) fn trajectoire_valider(&mut self, snap: Edit, nom: &str) {
        self.modified = true;
        self.commit_named(snap, nom);
    }

    /// « Marque posée : 3 marques, le chemin dure de l'image 1 à 36. » En clés d'images sans dessin au départ, le dit.
    pub(crate) fn trajectoire_resume(&self, li: usize, debut: &str) -> String {
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.as_ref()) else { return debut.to_string() };
        let images = t.images_des_jalons(self.frame_count());
        let n = t.marques.len();
        let mut texte = match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("{debut} : {n} marque{}, le chemin dure de l'image {} à l'image {}.", pluriel(n), a + 1, b + 1),
            _ => format!("{debut} : {n} marque{} ; le plan s'arrête avant l'image {} du départ, le calque ne bouge pas encore.", pluriel(n), t.depart + 1),
        };
        if t.cles == Cles::Images && t.dessins.is_empty() && images.len() >= 2 {
            texte.push_str(&format!(" Il manque un dessin au départ : dessine d'abord à l'image {}, la trajectoire en fera une copie à chaque marque.", t.depart + 1));
        }
        texte
    }

    /// La clé de l'image `frame` du calque `li`, un jalon de sa trajectoire, qu'on supprime (Suppr, le menu) : une marque s'en va
    /// (les clés sont reposées) ; le départ et la fin ne se suppriment pas, la ligne d'état dit comment enlever la trajectoire. Vrai
    /// si c'était un jalon (la clé n'est alors pas supprimée à la main).
    pub(crate) fn trajectoire_supprimer_jalon(&mut self, li: usize, frame: usize) -> bool {
        let duree = self.frame_count();
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.as_ref()) else { return false };
        let images = t.images_des_jalons(duree);
        let Some(k) = images.iter().position(|&f| f == frame) else { return false };
        if k == 0 || k + 1 == images.len() {
            self.status = "Le départ et la fin de la trajectoire ne se suppriment pas : « Effacer » dans les réglages de l'outil Trajectoire enlève tout.".into();
            return true;
        }
        if let Some(t) = self.layers_mut()[li].trajectoire.as_mut() {
            t.retirer_marque(k - 1);
        }
        self.trajectoire_recalculer(li);
        self.status = self.trajectoire_resume(li, "Marque retirée");
        true
    }

    /// Le calque `li`, à trajectoire, vient d'être déplacé (agrandi, tourné) avec l'outil Transformer, de la pose `avant` à sa pose
    /// courante à l'image courante : le calque et sa trajectoire bougent ensemble (le chemin suit le déplacement, la pose propre du
    /// calque prend le reste), et les clés sont reposées. Vrai si le calque a une trajectoire.
    pub(crate) fn trajectoire_apres_transformer(&mut self, li: usize, avant: LayerTransform) -> bool {
        let f = self.cur_frame;
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.clone()) else { return false };
        // Une trajectoire tracée dans l'espace (partie 149) : la pose propre de la carte prend le geste, son chemin la suit.
        if !t.sur_la_feuille() {
            self.status = "Le calque et sa trajectoire ont bougé ensemble.".into();
            return self.trajectoire_espace_apres_pose(li);
        }
        let apres = self.layers()[li].pose(f);
        let delta = [apres.x - avant.x, apres.y - avant.y];
        let libre = self.doc.plan_en_espace(self.cur_panel);
        let l = &mut self.layers_mut()[li];
        // La pose propre : celle du départ, moins ce que la trajectoire y ajoutait. Le geste s'y applique, sauf s'il a changé la pose du
        // départ lui-même (la clé du départ, ou la pose fixe) : elle le porte déjà.
        let posee = crate::poskeys::pose_at(&l.pos_keys, t.depart, l.transform);
        let mut base = LayerTransform { x: posee.x - t.decalage_depart[0], y: posee.y - t.decalage_depart[1], ..posee };
        if !l.pos_keys.is_empty() && f != t.depart {
            base = composer(base, avant, apres);
        }
        l.pos_keys.clear();
        l.transform = base;
        l.fold_dz_selon(libre);
        if let Some(tr) = l.trajectoire.as_mut() {
            tr.decalage_depart = [0.0, 0.0];
            tr.deplacer(delta[0], delta[1]);
        }
        self.trajectoire_recalculer(li);
        self.status = "Le calque et sa trajectoire ont bougé ensemble.".into();
        true
    }

    /// Échap pendant un geste de l'outil : tout revient comme avant le geste ; vrai si la touche a servi.
    pub(crate) fn trajectoire_touche(&mut self, key: Key) -> bool {
        if key != Key::Escape || self.tool != Tool::Trajectoire {
            return false;
        }
        // (Un geste en cours dans la vue Espace 3D : `trajectoire3d.rs`.)
        if self.e3_traj_abandonner() {
            return true;
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
            Some(Geste::Trait { .. } | Geste::Prolonger { .. }) => return CursorIcon::Crosshair,
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
        // Une trajectoire tracée dans l'espace (partie 149) : sur la feuille d'un plan en Espace 3D, son chemin est montré comme
        // la caméra du plan le voit, sans se laisser prendre (`trajectoire3d.rs`).
        if self.paint_trajectoire_dans_l_espace(painter) {
            return;
        }
        let ecran = |p: [f32; 2]| self.stage_to_screen(pos2(p[0], p[1]));
        if let Some(Geste::Trait { points } | Geste::Prolonger { points, .. }) = &self.traj.geste {
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
        // Où en est le point d'ancrage à l'image courante : sur le chemin quand le chemin couvre l'image ; en clés d'images, avant
        // le départ, le dessin n'a pas bougé, le point d'ancrage est sur lui.
        let point = match t.fraction_a_l_image(self.cur_frame, duree) {
            Some(f) => Some(t.point_a(f)),
            None if t.cles == Cles::Images && self.cur_frame < t.depart => {
                let posee = l.pose(t.depart);
                let base = LayerTransform { x: posee.x - t.decalage_depart[0], y: posee.y - t.decalage_depart[1], ..posee };
                Some(self.trajectoire_point_d_ancrage(self.cur_panel, self.cur_layer, t, base))
            }
            None => None,
        };
        if let Some(p) = point {
            let c = ecran(p);
            painter.circle(c, 4.0, COULEUR, Stroke::new(1.5_f32, Color32::from_black_alpha(140)));
            if t.ancre != Ancre::Depart {
                painter.circle_stroke(c, 8.0, Stroke::new(1.0_f32, COULEUR.gamma_multiply(0.8)));
            }
        }
    }

    // ------------------------------------------------------------------ réglages

    /// Réglages de l'outil (panneau Réglages, fenêtre du clic droit) : la sorte des clés, le départ et la fin, les commandes de la
    /// trajectoire du calque courant.
    pub(crate) fn trajectoire_options_ui(&mut self, ui: &mut egui::Ui) {
        // Le calque courant suit une trajectoire tracée dans l'espace (partie 149) : ses réglages sont ceux de l'espace (ni point
        // d'ancrage ni « le dessin tourne » : ils n'y servent pas).
        if self.trajectoire_espace_options_ui(ui) {
            return;
        }
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
            ui.checkbox(&mut self.traj.tourne_propose, TOURNE).on_hover_text(TOURNE_AIDE);
            self.trajectoire_lissage_ui(ui, false);
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
        // Le lissage du chemin, pour tous les calques choisis (partie 151).
        self.trajectoire_lissage_ui(ui, false);
        // Le dessin tourne avec le chemin (Anthony, 2026-11-08) : il pivote autour du point d'ancrage pour suivre la direction du chemin.
        let mut tourne = t.tourne;
        if ui.checkbox(&mut tourne, TOURNE).on_hover_text(TOURNE_AIDE).changed() {
            let snap = self.snapshot();
            if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                tr.tourne = tourne;
            }
            self.traj.tourne_propose = tourne;
            self.trajectoire_recalculer(li);
            self.trajectoire_valider(snap, if tourne { "Le dessin tourne avec le chemin" } else { "Le dessin ne tourne plus avec le chemin" });
        }
        // Ce qui suit le chemin : là où part le trait, le bas du dessin, son centre…, ou un point cliqué.
        ui.horizontal_wrapped(|ui| {
            ui.label("Ce qui suit le chemin :");
            let mut ancre = t.ancre;
            let actuel = ancre.label();
            egui::ComboBox::from_id_salt("trajectoire.ancre").selected_text(actuel).show_ui(ui, |ui| {
                for a in Ancre::CHOIX {
                    ui.selectable_value(&mut ancre, a, a.label());
                }
            });
            if ancre != t.ancre {
                let snap = self.snapshot();
                if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                    tr.ancre = ancre;
                }
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, "Ancrage de la trajectoire");
            }
            let on = self.traj.choisir_ancre;
            let bouton = egui::Button::new(crate::bulles::encre_etat(RichText::new("Un point du dessin : cliquer"), on)).selected(on);
            if ui.add(bouton).on_hover_text("Puis clique le point du dessin qui doit suivre le chemin (le bout d'un pied, une main…).").clicked() {
                self.traj.choisir_ancre = !self.traj.choisir_ancre;
                if self.traj.choisir_ancre {
                    self.status = "Clique le point du dessin qui doit suivre le chemin.".into();
                }
            }
        });
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
                let libre = self.doc.plan_en_espace(pi);
                let l = &mut self.layers_mut()[li];
                l.transform = crate::poskeys::pose_at(&l.pos_keys, t.depart, l.transform);
                l.pos_keys.clear();
                l.fold_dz_selon(libre);
                l.trajectoire = None;
                self.trajectoire_valider(snap, "Effacer la trajectoire");
                self.status = "Trajectoire effacée : le calque reste où il était au départ.".into();
            }
        });
        ui.label(
            RichText::new(
                "Tirer sur le chemin le tord là où on le prend, comme une corde (Maj : le déplacer en bloc). Maj + un trait depuis un \
                 bout du chemin le prolonge, devant ou derrière. Glisser une marque le long du \
                 chemin la déplace ; la tirer loin du chemin la retire. Un trait long retrace le chemin, les marques restent. Sur la \
                 timeline, glisser la première clé, la dernière ou une marque les recale dans le temps ; Suppr sur une marque la retire. \
                 Transformer déplace le calque et son chemin ensemble.",
            )
            .small()
            .weak(),
        );
    }
}

/// La pose `base` à laquelle on applique ce qu'un geste a changé entre `avant` et `apres` : déplacement, rotation, inclinaison et
/// écart de profondeur s'ajoutent ; échelle et étirements se multiplient.
fn composer(base: LayerTransform, avant: LayerTransform, apres: LayerTransform) -> LayerTransform {
    let ratio = |a: f32, b: f32| if a.abs() > 1e-6 { b / a } else { 1.0 };
    LayerTransform {
        x: base.x + (apres.x - avant.x),
        y: base.y + (apres.y - avant.y),
        scale: (base.scale * ratio(avant.scale, apres.scale)).clamp(0.01, 100.0),
        angle: base.angle + (apres.angle - avant.angle),
        stretch_x: LayerTransform::clamp_stretch(base.stretch_x * ratio(avant.stretch_x, apres.stretch_x)),
        stretch_y: LayerTransform::clamp_stretch(base.stretch_y * ratio(avant.stretch_y, apres.stretch_y)),
        tilt_x: LayerTransform::clamp_tilt(base.tilt_x + (apres.tilt_x - avant.tilt_x)),
        tilt_y: LayerTransform::clamp_tilt(base.tilt_y + (apres.tilt_y - avant.tilt_y)),
        dz: base.dz + (apres.dz - avant.dz),
    }
}

/// Les deux sortes de clés : interpolées (le calque glisse le long du chemin) ou par images (un dessin à chaque marque) ; vrai si
/// le choix a changé.
fn cles_ui(ui: &mut egui::Ui, cles: &mut Cles) -> bool {
    let avant = *cles;
    ui.horizontal(|ui| {
        ui.radio_value(cles, Cles::Images, Cles::Images.label()).on_hover_text("Un dessin à chaque marque, copie du dessin du départ déplacé sur le chemin, exposé jusqu'à la marque suivante : la timeline se scinde, chaque pose se redessine. D'origine.");
        ui.radio_value(cles, Cles::Interpolation, Cles::Interpolation.label()).on_hover_text("Une clé de position par image : le calque glisse le long du chemin, à la vitesse que donnent les marques.");
    });
    *cles != avant
}
