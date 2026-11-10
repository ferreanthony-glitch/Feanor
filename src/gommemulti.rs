//! La gomme sur plusieurs calques (Anthony, 2026-11-10 : « Les outils sélection, gomme, déformer et transformer s'appliquent à
//! plusieurs calques quand plusieurs calques sont sélectionnés. »).
//!
//! Avec plusieurs calques choisis (Ctrl ou Maj + clic dans le panneau Calques, la timeline ou le montage), un passage de la gomme
//! efface sur chacun ce qu'elle touche **là où on le voit** : le calque courant comme toujours (`begin_paint`), et chaque autre
//! calque choisi reçoit les mêmes points, ramenés dans le repère de son dessin (sa transformation, ses clés de position et le
//! défilement multiplan à l'image courante, `multisel::layer_to_stage`).
//!
//! - **Chacun selon son genre** : un calque bitmap efface ses pixels avec la brosse de la gomme (agrandie ou réduite comme le
//!   calque), un calque vectoriel coupe ses traits (`vector::erase_circle`, comme la gomme vectorielle du calque courant, avec le
//!   réglage « traits entiers »).
//! - **Une seule étape d'annulation** pour tout le geste : la marque de l'historique est prise avant le trait du calque courant,
//!   et tout ce qui a été posé depuis est regroupé au lâcher (`History::regrouper_depuis`, juste même avec un historique plein).
//! - **Calques laissés de côté** : cachés, verrouillés (eux ou leur dossier, ou leur position), vidéo, sans dessin à cette
//!   image : la gomme les laisse tels quels et la barre d'état les nomme (`Targets::skipped_text`). Le calque courant refusé (par
//!   exemple verrouillé) ne bloque pas les autres.
//! - **Sans latence** : rien n'est rejoué ; un calque vectoriel refait seulement la zone des traits coupés, comme la gomme
//!   vectorielle d'un seul calque ; un calque bitmap pose les empreintes de la brosse, comme un trait.
//! - Ni dans une case des Vignettes (la planche a ses propres règles, `gommedebord.rs`), ni quand l'aplat est la cible
//!   (`aplatmulti.rs` s'en charge, sur les clés).

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui::{Pos2, Rect};

use crate::app::{App, Tool};
use crate::brush::{StrokeMode, StrokeState};
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::Raster;
use crate::vector::{self, Affine, VectorDrawing};

enum Sorte {
    /// Un dessin bitmap : la brosse de la gomme, comme un trait.
    Pixels(Box<StrokeState>),
    /// Un dessin vectoriel : son état d'avant le geste, le dernier point, la zone des traits coupés.
    Traits { avant: Box<VectorDrawing>, dernier: Option<Pos2>, region: Rect, change: bool },
}

/// Un autre calque choisi que la gomme traverse.
struct Autre {
    cel: CelId,
    /// Du dessin du calque courant vers le dessin de ce calque (à l'image courante).
    vers: Affine,
    /// L'échelle de ce passage : la gomme garde sa taille à l'écran.
    k: f32,
    /// Ce dessin est celui qu'on voit (sinon : celui d'une autre clé choisie, qu'on ne voit pas).
    live: bool,
    /// Un dessin qu'on ne voit pas reçoit ses points au lâcher (rien ne coûte pendant le geste) : point (dans son repère), appui,
    /// instant.
    attente: Vec<(Pos2, f32, f64)>,
    sorte: Sorte,
}

/// Les réglages de la gomme pour un point.
#[derive(Clone, Copy)]
struct Reglage {
    size: f32,
    pressure_size: bool,
    whole: bool,
}

/// La gomme passe en `q` (repère du dessin) sur ce dessin (ses pixels, ou ses traits) : rend les tuiles changées (bitmap) et la
/// zone des traits coupés (vectoriel), dont l'image est à refaire. Sans `App` ni document : sert aussi sur le fil de travail.
fn gommer(raster: &mut Raster, vecteur: Option<&mut VectorDrawing>, a: &mut Autre, q: Pos2, pressure: f32, time: f64, r: Reglage) -> (Vec<usize>, Option<Rect>) {
    let mut dirty = Vec::new();
    match &mut a.sorte {
        Sorte::Pixels(stroke) => {
            stroke.add_point_at(raster, q, pressure, time, &mut dirty);
            (dirty, None)
        }
        Sorte::Traits { dernier, region, change, .. } => {
            let factor = if r.pressure_size { pressure.clamp(0.05, 1.0) } else { 1.0 };
            let radius = (r.size * 0.5 * factor * a.k).max(0.5);
            let Some(d) = vecteur else { return (dirty, None) };
            // Le long du segment parcouru (comme `App::erase_to`), et seule la zone des traits touchés est refaite.
            let from = dernier.unwrap_or(q);
            let steps = ((from.distance(q) / (radius * 0.5).max(0.5)).ceil() as usize).max(1);
            let swept = Rect::from_two_pos(from, q).expand(radius + 1.0);
            let zone = d.elements.iter().map(|e| e.bounds().expand(1.5)).filter(|b| b.intersects(swept)).fold(Rect::NOTHING, |r, b| r.union(b));
            let mut coupe = false;
            for k in 1..=steps {
                let p = from + (q - from) * (k as f32 / steps as f32);
                coupe |= vector::erase_circle(d, p, radius, r.whole);
            }
            *dernier = Some(q);
            if coupe {
                *change = true;
                *region = region.union(zone);
                return (dirty, Some(zone));
            }
            (dirty, None)
        }
    }
}

/// Ce qu'un dessin qu'on ne voit pas reçoit au lâcher, calculé sur un fil de travail : le dessin gommé, son étape d'annulation.
enum Fini {
    Pixels { raster: Raster, edit: Option<Edit> },
    Traits { dessin: Box<VectorDrawing>, avant: Box<VectorDrawing> },
    Rien,
}

/// Ce que contenait un dessin au départ de son calcul, pour savoir à l'arrivée s'il a été retouché entre-temps. Pas sa version :
/// elle monte aussi quand le rendu exact d'un dessin vectoriel arrive (`App::poll_rejeu`), sans que ses traits changent.
enum Sceau {
    /// Les tuiles de l'image (partagées : rien n'est recopié).
    Pixels(Raster),
    /// La géométrie des traits (`sceau_des_traits`).
    Traits(u64),
}

/// L'empreinte de la géométrie d'un dessin vectoriel : le nombre d'éléments, et pour chacun sa boîte, son nombre de points, sa
/// couleur et son opacité. Les pixels en cache des traits n'y sont pas.
pub(crate) fn sceau_des_traits(d: &VectorDrawing) -> u64 {
    use crate::vector::Element;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ d.elements.len() as u64;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    for e in &d.elements {
        let b = e.bounds();
        for v in [b.min.x, b.min.y, b.max.x, b.max.y] {
            mix(v.to_bits() as u64);
        }
        match e {
            Element::Stroke { points, color, opacity, .. } => {
                mix(points.len() as u64);
                mix(u64::from(color[0]) | u64::from(color[1]) << 8 | u64::from(color[2]) << 16);
                mix(opacity.to_bits() as u64);
                for p in points {
                    mix(p.x.to_bits() as u64 ^ (p.y.to_bits() as u64) << 32);
                }
            }
            Element::Fill { contours, color, opacity, .. } => {
                mix(contours.iter().map(|c| c.len()).sum::<usize>() as u64);
                mix(u64::from(color[0]) | u64::from(color[1]) << 8 | u64::from(color[2]) << 16);
                mix(opacity.to_bits() as u64);
            }
            _ => mix(7),
        }
    }
    h
}

fn sceau_de(c: &crate::document::Cel) -> Sceau {
    match &c.vector {
        Some(d) => Sceau::Traits(sceau_des_traits(d)),
        None => Sceau::Pixels(c.raster.clone()),
    }
}

/// Le dessin a-t-il gardé le contenu de son sceau ?
fn intact(c: &crate::document::Cel, s: &Sceau) -> bool {
    match (s, &c.vector) {
        (Sceau::Traits(h), Some(d)) => sceau_des_traits(d) == *h,
        (Sceau::Pixels(r), None) => c.raster.tiles == r.tiles && c.raster.outer == r.outer,
        _ => false,
    }
}

/// Un résultat qui revient du fil, pour le dessin `cel`. `fini` : rien si le calcul a échoué (le fil a paniqué).
struct Retour {
    cel: CelId,
    fini: Option<Fini>,
}

/// Un dessin à gommer sur le fil : le geste entier (ses points), et une copie de son image ou de ses traits (les tuiles se
/// partagent : rien n'est recopié).
struct Travail {
    autre: Autre,
    raster: Raster,
    dessin: Option<VectorDrawing>,
    r: Reglage,
    /// (Essais : le projet, pour qu'un essai fasse échouer le calcul d'un dessin précis, `PANIQUE_ESSAI`.)
    #[cfg(test)]
    epoque: u64,
}

/// Essais seulement : le calcul du dessin (projet, dessin) panique, comme un défaut dans le moteur.
#[cfg(test)]
pub(crate) static PANIQUE_ESSAI: std::sync::Mutex<Option<(u64, CelId)>> = std::sync::Mutex::new(None);

/// Essais seulement : les calculs de ce projet attendent (pour qu'un essai place un geste pendant le calcul, sans course).
#[cfg(test)]
pub(crate) static PAUSE_ESSAI: std::sync::Mutex<Option<u64>> = std::sync::Mutex::new(None);

fn calculer(t: Travail) -> Fini {
    #[cfg(test)]
    {
        if PANIQUE_ESSAI.lock().is_ok_and(|p| *p == Some((t.epoque, t.autre.cel))) {
            panic!("essai : le calcul de ce dessin échoue");
        }
        let debut = std::time::Instant::now();
        while PAUSE_ESSAI.lock().is_ok_and(|p| *p == Some(t.epoque)) && debut.elapsed() < std::time::Duration::from_secs(30) {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }
    let Travail { mut autre, mut raster, mut dessin, r, .. } = t;
    for (q, pressure, time) in std::mem::take(&mut autre.attente) {
        gommer(&mut raster, dessin.as_mut(), &mut autre, q, pressure, time, r);
    }
    match autre.sorte {
        Sorte::Pixels(mut stroke) => {
            let mut dirty = Vec::new();
            let edit = stroke.finish(&mut raster, &mut dirty);
            Fini::Pixels { raster, edit }
        }
        Sorte::Traits { avant, change: true, .. } => match dessin {
            Some(d) => Fini::Traits { dessin: Box::new(d), avant },
            None => Fini::Rien,
        },
        Sorte::Traits { .. } => Fini::Rien,
    }
}

/// La part cachée d'un geste lâché, qui attend son tour : les dessins des autres clés, avec les points du geste ; l'étape
/// d'annulation du geste ; son projet.
struct Lot {
    autres: Vec<Autre>,
    etape: u64,
    quoi: String,
    r: Reglage,
    epoque: u64,
}

/// Le lot qui se calcule : ses comptes, et les sceaux des dessins au départ.
struct EnCours {
    etape: u64,
    quoi: String,
    epoque: u64,
    total: usize,
    poses: usize,
    retouches: usize,
    annules: usize,
    perdus: usize,
    sceaux: Vec<(CelId, Sceau)>,
}

/// Les dessins des clés qu'on ne voit pas, gommés sur des fils de travail après le lâcher (comme les remplissages de l'aplat sur
/// plusieurs clés, `aplatmulti.rs`), un geste après l'autre, dans l'ordre. Chaque résultat rejoint l'étape d'annulation du geste,
/// qui existe dès le lâcher ; si elle a été annulée entre-temps, ou si le dessin a été retouché, le résultat est abandonné. Un
/// résultat qui arrive pendant un geste (sur ce dessin ou un autre) attend sa fin. Enregistrer, Ctrl+Z et Ctrl+Maj+Z attendent la
/// fin du calcul.
pub(crate) struct GommeTravaux {
    tx: Sender<Retour>,
    rx: Receiver<Retour>,
    /// Les résultats du lot en cours qui ne sont pas encore arrivés.
    pub(crate) en_route: usize,
    /// Ceux qui sont arrivés pendant un geste : posés à sa fin.
    differes: Vec<Retour>,
    courant: Option<EnCours>,
    file: std::collections::VecDeque<Lot>,
    /// Les commandes (Ctrl+Z, Ctrl+Maj+Z…) qui attendent la fin du calcul, dans l'ordre (`gomme_retient`).
    pub(crate) commandes: Vec<&'static str>,
    /// Un enregistrement qui attend la fin du calcul (`gomme_retient_sauvegarde`).
    sauvegarde: Option<(std::path::PathBuf, Option<crate::app::Pending>)>,
}

impl Default for GommeTravaux {
    fn default() -> Self {
        let (tx, rx) = channel();
        GommeTravaux { tx, rx, en_route: 0, differes: Vec::new(), courant: None, file: Default::default(), commandes: Vec::new(), sauvegarde: None }
    }
}

impl GommeTravaux {
    /// Un calcul est en route, ou en attente.
    pub(crate) fn occupe(&self) -> bool {
        self.en_route > 0 || !self.differes.is_empty() || self.courant.is_some() || !self.file.is_empty()
    }

    /// « 12/20 » : les dessins faits du lot en cours, sur son total (et les lots qui attendent).
    fn progression(&self) -> String {
        let Some(c) = &self.courant else { return String::new() };
        let faits = c.poses + c.retouches + c.annules + c.perdus;
        let suite = if self.file.is_empty() { String::new() } else { format!(" (puis {} geste{})", self.file.len(), if self.file.len() > 1 { "s" } else { "" }) };
        format!("Gomme : {faits}/{} dessins des autres clés…{suite}", c.total)
    }
}

/// Le geste de gomme en cours sur les autres calques choisis.
#[derive(Default)]
pub(crate) struct GommeMulti {
    autres: Vec<Autre>,
    /// La marque de l'historique avant le geste (`History::marque`).
    marque: Option<u64>,
    /// Le geste est en cours (même si le calque courant n'a rien à gommer).
    actif: bool,
    pen: bool,
    /// Le nombre de dessins que touche le geste (celui qu'on voit compris) : l'alerte au-delà de `SEUIL_ALERTE`.
    dessins: usize,
    /// Ce qu'il touche, en clair (« 6 dessins (6 clés sur 2 calques) ») : la barre d'état le redit à la fin du calcul.
    quoi: String,
}

/// Au-delà de ce nombre de dessins touchés par un geste de gomme, une alerte bien visible s'affiche sur le dessin pendant tout
/// le geste (Anthony, 2026-11-10 : un avertissement, sans confirmation ni blocage).
pub(crate) const SEUIL_ALERTE: usize = 24;

impl GommeMulti {
    /// Un geste de gomme est en cours sur plusieurs calques.
    pub(crate) fn actif(&self) -> bool {
        self.actif
    }

    pub(crate) fn pen(&self) -> bool {
        self.pen
    }

    /// La gomme passe en ce moment sur le dessin `cel` (son image n'est pas encore celle de sa géométrie : un rendu exact qui
    /// arriverait maintenant attend la fin du geste).
    pub(crate) fn busy_on(&self, cel: CelId) -> bool {
        self.actif && self.autres.iter().any(|a| a.cel == cel)
    }
}

impl App {
    /// La gomme agit-elle sur plusieurs calques ? L'outil Gomme, deux calques choisis au moins, hors des Vignettes et hors de
    /// l'aplat.
    pub(crate) fn gomme_multi_voulue(&self) -> bool {
        // (« Peint sur l'aplat » détourne la gomme vers un aplat : ce chemin-là, pas celui-ci.)
        self.tool == Tool::Eraser && !self.in_card && !self.aplat_en_cible() && self.sur_aplat_cible().is_none() && self.plusieurs_dessins_choisis()
    }

    /// Juste avant le trait du calque courant (`begin_paint`) : le geste d'avant est fini, et la marque de l'historique est prise
    /// (toutes les étapes du geste, sur tous les calques, n'en feront qu'une). Rien sans plusieurs calques choisis.
    pub(crate) fn gomme_multi_avant(&mut self) -> Option<u64> {
        if !self.gomme_multi_voulue() {
            return None;
        }
        // (Le geste démarre tout de suite, même si le calcul du précédent est en route : sa part cachée partira après, dans l'ordre.)
        self.end_stroke();
        Some(self.history.marque())
    }

    /// Juste après le trait du calque courant : les autres calques choisis reçoivent le même départ `p` (point du dessin du
    /// calque courant).
    pub(crate) fn gomme_multi_debut(&mut self, marque: u64, p: Pos2, pressure: f32, pen: bool) {
        self.gomme_multi = GommeMulti::default();
        if self.content_refuses() {
            return;
        }
        let Some(courant) = crate::multisel::layer_to_stage(&self.doc, self.cur_panel, self.cur_frame, self.cur_layer) else { return };
        // Le calque courant gomme déjà (son trait a commencé) : il n'est pas repris ici.
        let deja = (self.stroke.is_some() || self.verase.is_some()).then(|| self.cel_here()).flatten();
        let (targets, par_cles) = self.dessins_choisis();
        let Some(settings) = self.current_brush() else { return };
        let tol = self.close_tolerance();
        let mut autres = Vec::new();
        for t in &targets.list {
            // (Un calque de couleur et plusieurs de ses clés : ses autres clés suivent déjà le trait par les miroirs de l'aplat,
            // `aplatmulti.rs` ; elles ne sont pas gommées deux fois.)
            if Some(t.cel) == deja || autres.iter().any(|a: &Autre| a.cel == t.cel) || self.aplat_miroirs.contient(t.cel) {
                continue;
            }
            let Some(inv) = t.to_stage.inverse() else { continue };
            let vers = courant.then(&inv);
            let k = (vers.a * vers.d - vers.b * vers.c).abs().sqrt().max(1e-3);
            let sorte = if t.vector {
                let Some(d) = self.doc.cels.get(&t.cel).and_then(|c| c.vector.as_ref()) else { continue };
                Sorte::Traits { avant: Box::new(d.clone()), dernier: None, region: Rect::NOTHING, change: false }
            } else {
                let layer = self.layers().iter().find(|l| l.id == t.id);
                let keep_alpha = layer.is_some_and(|l| l.keep_alpha);
                // (Comme `begin_stroke` : la toile infinie, sauf pour les griffonnages d'un calque de couleur.)
                let zone = layer.and_then(|l| l.couleur.as_ref()).is_some_and(|c| c.griffonner);
                let mut s = settings;
                s.size *= k;
                // (Le zoom du dessin de ce calque à l'écran : celui de la vue fois son échelle sur la feuille.)
                let echelle = (t.to_stage.a * t.to_stage.d - t.to_stage.b * t.to_stage.c).abs().sqrt();
                let mut stroke = StrokeState::new(t.cel, StrokeMode::Erase, s, None, pen)
                    .with_textures(self.brushes.textures(&s))
                    .with_feel(self.toucher)
                    .with_zoom(self.view.zoom * echelle)
                    .with_secondary(self.couleurs.prefs.back)
                    .on_layer(keep_alpha, None)
                    .with_options(false, false, tol);
                if !zone {
                    stroke = stroke.on_infinite_canvas();
                }
                Sorte::Pixels(Box::new(stroke))
            };
            autres.push(Autre { cel: t.cel, vers, k, live: t.live, attente: Vec::new(), sorte });
        }
        // (Le dessin qu'on voit gomme toujours, choisi ou non.)
        let n = targets.list.len() + usize::from(deja.is_some_and(|c| !targets.list.iter().any(|t| t.cel == c)));
        let quoi = if par_cles { Self::dessins_texte(&targets, true) } else { format!("{n} calque{}", if n > 1 { "s" } else { "" }) };
        self.gomme_multi = GommeMulti { autres, marque: Some(marque), actif: true, pen, dessins: n, quoi: quoi.clone() };
        self.gomme_multi_point(p, pressure);
        self.status = match targets.skipped_text() {
            Some(l) => format!("Gomme sur {quoi}. {l}"),
            None => format!("Gomme sur {quoi}."),
        };
    }

    /// La gomme passe en `p` (point du dessin du calque courant) : chaque autre calque choisi efface là, dans son repère.
    pub(crate) fn gomme_multi_point(&mut self, p: Pos2, pressure: f32) {
        if !self.gomme_multi.actif || self.gomme_multi.autres.is_empty() {
            return;
        }
        let time = self.input_time;
        let r = Reglage { size: self.eraser.size, pressure_size: self.eraser.pressure_size, whole: self.vector_erase_whole };
        let mut autres = std::mem::take(&mut self.gomme_multi.autres);
        for a in &mut autres {
            let q = a.vers.apply(p);
            if a.live {
                let Some(c) = self.doc.cels.get_mut(&a.cel) else { continue };
                let (mut tiles, zone) = gommer(&mut c.raster, c.vector.as_mut(), a, q, pressure, time, r);
                if let Some(z) = zone {
                    tiles.extend(c.rerender_region(z.expand(2.0)));
                }
                self.renderer.invalidate_tiles(&tiles);
            } else {
                a.attente.push((q, pressure, time));
            }
        }
        self.gomme_multi.autres = autres;
    }

    /// Le geste est fini (`end_stroke`, après la gomme du calque courant) : chaque autre calque pose son étape, et tout ce qui a
    /// été posé depuis la marque devient une seule étape d'annulation.
    pub(crate) fn gomme_multi_fin(&mut self) {
        if !self.gomme_multi.actif {
            return;
        }
        let GommeMulti { autres, marque, quoi, .. } = std::mem::take(&mut self.gomme_multi);
        let mut edits: Vec<Edit> = Vec::new();
        let mut dirty = Vec::new();
        let r = Reglage { size: self.eraser.size, pressure_size: self.eraser.pressure_size, whole: self.vector_erase_whole };
        let mut caches: Vec<Autre> = Vec::new();
        for a in autres {
            // Les dessins des autres clés (qu'on ne voit pas) reçoivent le geste entier sur un fil de travail, à leur tour.
            if !a.live {
                if !a.attente.is_empty() {
                    caches.push(a);
                }
                continue;
            }
            match a.sorte {
                Sorte::Pixels(mut stroke) => {
                    let Some(c) = self.doc.cels.get_mut(&a.cel) else { continue };
                    let edit = stroke.finish(&mut c.raster, &mut dirty);
                    c.version += 1;
                    edits.extend(edit);
                }
                Sorte::Traits { avant, region, change, .. } => {
                    if !change {
                        continue;
                    }
                    if let Some(c) = self.doc.cels.get_mut(&a.cel) {
                        c.version += 1;
                    }
                    self.refresh_cel_region(a.cel, region);
                    edits.push(Edit::Vector { cel: a.cel, drawing: *avant });
                }
            }
        }
        self.renderer.invalidate_tiles(&dirty);
        if !edits.is_empty() {
            let avant = self.history.context.replace("Gomme".to_string());
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
            self.modified = true;
        }
        if let Some(m) = marque {
            self.history.regrouper_depuis(m);
        }
        if caches.is_empty() {
            return;
        }
        // L'étape du geste existe dès le lâcher, même si rien de ce qu'on voit n'a changé : les résultats la rejoindront, et aucun
        // n'atterrit dans l'étape d'un autre geste.
        if marque.is_none_or(|m| self.history.last_id() <= m) {
            let avant = self.history.context.replace("Gomme".to_string());
            self.history.push(Edit::Group(Vec::new()));
            self.history.context = avant;
        }
        let etape = self.history.last_id();
        self.gomme_travaux.file.push_back(Lot { autres: caches, etape, quoi, r, epoque: self.doc_epoch });
        // (Un calcul d'avant est encore en route : ce lot part à sa fin, dans l'ordre.)
        self.gomme_lancer_suivant();
        if self.gomme_travaux.occupe() {
            self.status = self.gomme_travaux.progression();
        }
    }

    /// Le lot en cours est fini (tout est arrivé et posé) : son message ; puis le lot suivant de la file part, ses dessins pris tels
    /// qu'ils sont maintenant (le geste d'avant y est posé).
    fn gomme_lancer_suivant(&mut self) {
        if self.gomme_travaux.en_route > 0 || !self.gomme_travaux.differes.is_empty() {
            return;
        }
        if let Some(c) = self.gomme_travaux.courant.take() {
            self.gomme_finir_lot(c);
        }
        while let Some(lot) = self.gomme_travaux.file.pop_front() {
            if lot.epoque != self.doc_epoch {
                continue;
            }
            let Lot { autres, etape, quoi, r, epoque } = lot;
            let mut travaux = Vec::new();
            let mut sceaux = Vec::new();
            for mut a in autres {
                let Some(c) = self.doc.cels.get(&a.cel) else { continue };
                // (Ce que le geste annulera : le dessin tel qu'il est maintenant, le geste d'avant posé.)
                if let (Sorte::Traits { avant, .. }, Some(d)) = (&mut a.sorte, &c.vector) {
                    *avant = Box::new(d.clone());
                }
                sceaux.push((a.cel, sceau_de(c)));
                travaux.push(Travail {
                    raster: c.raster.clone(),
                    dessin: c.vector.clone(),
                    r,
                    autre: a,
                    #[cfg(test)]
                    epoque,
                });
            }
            let n = travaux.len();
            if n == 0 {
                continue;
            }
            let fils = std::thread::available_parallelism().map_or(2, |p| p.get()).saturating_sub(1).clamp(1, 4).min(n);
            let mut paquets: Vec<Vec<Travail>> = (0..fils).map(|_| Vec::new()).collect();
            for (i, t) in travaux.into_iter().enumerate() {
                paquets[i % fils].push(t);
            }
            let mut lances = 0;
            let mut perdus = 0;
            for paquet in paquets {
                let k = paquet.len();
                let (tx, ctx) = (self.gomme_travaux.tx.clone(), self.ctx.clone());
                let lance = std::thread::Builder::new().name("gomme-cles".into()).spawn(move || {
                    for t in paquet {
                        let cel = t.autre.cel;
                        // Un calcul qui panique est compté comme perdu : la file ne reste jamais bloquée (comme `rejeu.rs`).
                        let fini = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| calculer(t))).ok();
                        let _ = tx.send(Retour { cel, fini });
                        ctx.request_repaint();
                    }
                });
                match lance {
                    Ok(_) => lances += k,
                    Err(_) => perdus += k,
                }
            }
            self.gomme_travaux.en_route += lances;
            self.gomme_travaux.courant = Some(EnCours { etape, quoi, epoque, total: n, poses: 0, retouches: 0, annules: 0, perdus, sceaux });
            if lances == 0 {
                // (Aucun fil n'a pu partir : le lot est fini, et perdu.)
                if let Some(c) = self.gomme_travaux.courant.take() {
                    self.gomme_finir_lot(c);
                }
                continue;
            }
            return;
        }
    }

    /// Un lot est fini : son message. Si des dessins ont été perdus (un calcul a échoué), la règle du tout ou rien : le geste entier
    /// est annulé s'il est encore la dernière étape (rien ne reste à moitié fait) ; sinon il reste, et la barre d'état dit combien de
    /// dessins n'ont pas été gommés.
    fn gomme_finir_lot(&mut self, c: EnCours) {
        if c.epoque != self.doc_epoch {
            return;
        }
        if c.perdus > 0 {
            if self.history.etat_de_l_etape(c.etape) == Some(true) && self.history.last_id() == c.etape {
                let mut nav = crate::history::Nav { panel: self.cur_panel, layer: self.cur_layer };
                if self.history.undo_and_forget(&mut self.doc, &mut nav) {
                    self.renderer.invalidate_all();
                    self.rejeu_after_history();
                    self.status = format!("Gomme annulée en entier : le calcul de {} dessin{} a échoué, rien n'a changé.", c.perdus, if c.perdus > 1 { "s" } else { "" });
                    return;
                }
            }
            self.status = format!("Gomme incomplète : le calcul de {} dessin{} a échoué (Ctrl+Z annule le geste en entier).", c.perdus, if c.perdus > 1 { "s" } else { "" });
        } else if c.annules > 0 {
            self.status = "Gomme : le geste a été annulé, le calcul des autres clés est abandonné.".into();
        } else if c.retouches > 0 {
            self.status = format!("Gomme : {} dessin(s) retouché(s) pendant le calcul laissé(s) tel(s) quel(s) ; {} gommé(s).", c.retouches, c.poses);
        } else {
            self.status = format!("Gomme sur {} : posée partout.", c.quoi);
        }
    }

    /// Un geste est en cours (un trait, un glissé, un champ tenu…) : un résultat qui arrive attend sa fin, pour ne pas se mêler à
    /// son étape d'annulation.
    fn geste_en_cours(&self) -> bool {
        self.gomme_multi.actif
            || self.stroke.is_some()
            || self.verase.is_some()
            || self.vdrag.is_some()
            || self.bdrag.is_some()
            || self.multi.drag.is_some()
            || self.layer_drag.is_some()
            || self.gesture.en_cours()
    }

    /// Une fois par image d'écran (`poll_aplat_multi`) : pose les résultats arrivés (sauf pendant un geste), et lance le lot suivant.
    /// Puis, le calcul fini, les commandes qui l'attendaient (Ctrl+Z…). Rien au repos.
    pub(crate) fn poll_gomme_multi(&mut self) {
        if !self.gomme_travaux.occupe() && self.gomme_travaux.commandes.is_empty() && self.gomme_travaux.sauvegarde.is_none() {
            return;
        }
        let arrives: Vec<Retour> = self.gomme_travaux.rx.try_iter().collect();
        for r in arrives {
            self.gomme_travaux.en_route = self.gomme_travaux.en_route.saturating_sub(1);
            self.gomme_travaux.differes.push(r);
        }
        if !self.geste_en_cours() {
            let differes = std::mem::take(&mut self.gomme_travaux.differes);
            for r in differes {
                if self.cel_busy(r.cel) {
                    self.gomme_travaux.differes.push(r);
                } else {
                    self.poser_gomme(r);
                }
            }
        }
        self.gomme_lancer_suivant();
        if self.gomme_travaux.occupe() {
            // (Le calcul d'un projet qu'on a quitté se termine sans rien dire.)
            if self.gomme_travaux.courant.as_ref().is_some_and(|c| c.epoque == self.doc_epoch) {
                self.status = self.gomme_travaux.progression();
            }
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
            return;
        }
        // Le calcul est fini : les commandes retenues partent, dans l'ordre, puis l'enregistrement retenu.
        for action in std::mem::take(&mut self.gomme_travaux.commandes) {
            self.run_action(action);
        }
        if let Some((manifeste, ensuite)) = self.gomme_travaux.sauvegarde.take() {
            self.start_save(manifeste, ensuite);
        }
    }

    /// Enregistrer pendant le calcul : on attend un peu (150 ms) ; si ce n'est pas fini, l'enregistrement est retenu et partira à la
    /// fin du calcul. Rend ce qu'il faut enregistrer maintenant, ou rien s'il est retenu.
    pub(crate) fn gomme_retient_sauvegarde(&mut self, manifeste: std::path::PathBuf, ensuite: Option<crate::app::Pending>) -> Option<(std::path::PathBuf, Option<crate::app::Pending>)> {
        if !self.gomme_travaux.occupe() {
            return Some((manifeste, ensuite));
        }
        self.attendre_gomme_multi_pendant(std::time::Duration::from_millis(150));
        if !self.gomme_travaux.occupe() {
            return Some((manifeste, ensuite));
        }
        self.gomme_travaux.sauvegarde = Some((manifeste, ensuite));
        self.status = format!("{} — l'enregistrement attend la fin du calcul.", self.gomme_travaux.progression());
        None
    }

    /// Ctrl+Z, Ctrl+Maj+Z (`action`) pendant le calcul : on attend un peu (150 ms) ; si ce n'est pas fini, la commande est retenue et
    /// partira à la fin du calcul (la fenêtre continue de se peindre, la barre d'état et le bandeau montrent l'avancée). Vrai si la
    /// commande est retenue (l'appelant ne fait rien de plus).
    pub(crate) fn gomme_retient(&mut self, action: &'static str) -> bool {
        if !self.gomme_travaux.commandes.is_empty() {
            self.gomme_travaux.commandes.push(action);
            return true;
        }
        if !self.gomme_travaux.occupe() {
            return false;
        }
        self.attendre_gomme_multi_pendant(std::time::Duration::from_millis(150));
        if !self.gomme_travaux.occupe() {
            return false;
        }
        self.gomme_travaux.commandes.push(action);
        self.status = format!("{} — {} attend la fin du calcul.", self.gomme_travaux.progression(), if action == "edition.annuler" { "Annuler" } else { "Rétablir" });
        true
    }

    /// Pose un résultat, s'il vaut encore : même projet, étape du geste toujours à annuler, dessin pas retouché depuis le départ du
    /// calcul (son contenu, pas sa version : `Sceau`).
    fn poser_gomme(&mut self, r: Retour) {
        let Some(courant) = self.gomme_travaux.courant.as_mut() else { return };
        if courant.epoque != self.doc_epoch {
            return;
        }
        let etape = courant.etape;
        let Some(fini) = r.fini else {
            courant.perdus += 1;
            return;
        };
        if self.history.etat_de_l_etape(etape) != Some(true) {
            courant.annules += 1;
            return;
        }
        let sceau_ok = match (self.doc.cels.get(&r.cel), courant.sceaux.iter().find(|(c, _)| *c == r.cel)) {
            (Some(c), Some((_, s))) => intact(c, s),
            _ => false,
        };
        if !sceau_ok {
            courant.retouches += 1;
            return;
        }
        let Some(c) = self.doc.cels.get_mut(&r.cel) else { return };
        let edit = match fini {
            Fini::Rien => {
                if let Some(c) = self.gomme_travaux.courant.as_mut() {
                    c.poses += 1;
                }
                return;
            }
            Fini::Pixels { raster, edit } => {
                let changed = c.raster.ids_either(&raster);
                c.raster = raster;
                c.version += 1;
                self.renderer.invalidate_tiles(&changed);
                edit
            }
            Fini::Traits { dessin, avant } => {
                c.vector = Some(*dessin);
                c.version += 1;
                // (Son image se refait en arrière-plan ; on ne voit pas ce dessin.)
                self.rejeu_cel(r.cel);
                Some(Edit::Vector { cel: r.cel, drawing: *avant })
            }
        };
        if let Some(c) = self.gomme_travaux.courant.as_mut() {
            c.poses += 1;
        }
        self.modified = true;
        let Some(edit) = edit else { return };
        // L'étape du geste reçoit ce retour en arrière (elle est encore à annuler : vérifié plus haut). Le numéro de l'historique
        // avance ; un champ tenu ne se croit pas pour autant devancé (`Gesture::suivre`).
        let avant = self.history.serial();
        self.history.extend_step(etape, edit);
        self.gesture.suivre(avant, self.history.serial());
    }

    /// L'alerte du geste en cours : plus de `SEUIL_ALERTE` dessins touchés. Rien sinon, ni hors d'un geste.
    pub(crate) fn gomme_alerte(&self) -> Option<String> {
        let g = &self.gomme_multi;
        if g.actif && g.dessins > SEUIL_ALERTE {
            return Some(format!("Gomme sur {} dessins (Ctrl+Z annule tout)", g.dessins));
        }
        // (Une commande attend la fin du calcul : l'avancée, bien visible, jusqu'à ce qu'elle parte.)
        let t = &self.gomme_travaux;
        if !t.commandes.is_empty() || t.sauvegarde.is_some() {
            let quoi = if t.sauvegarde.is_some() { "Enregistrer" } else { "Annuler" };
            return Some(format!("{} — {quoi} attend", t.progression()));
        }
        None
    }

    /// Le bandeau de l'alerte, en haut du dessin (`rect` : la zone du dessin à l'écran), de la couleur d'alerte, pendant le geste.
    pub(crate) fn paint_gomme_alerte(&self, painter: &eframe::egui::Painter, rect: Rect) {
        use eframe::egui::{Color32, FontId, Stroke, StrokeKind, Vec2};
        let Some(texte) = self.gomme_alerte() else { return };
        let [r, v, b] = crate::theme::PAPIER_ALERTE;
        let galley = painter.layout_no_wrap(texte, FontId::proportional(17.0), Color32::WHITE);
        let taille = galley.size() + Vec2::new(28.0, 14.0);
        let boite = Rect::from_center_size(eframe::egui::pos2(rect.center().x, rect.top() + 14.0 + taille.y * 0.5), taille);
        painter.rect(boite, 6.0, Color32::from_rgb(r, v, b), Stroke::new(2.0_f32, Color32::WHITE), StrokeKind::Outside);
        painter.galley(boite.min + Vec2::new(14.0, 7.0), galley, Color32::WHITE);
    }

    /// Attend les résultats en route et les pose : avant d'enregistrer (le fichier a le geste entier), avant un nouveau geste de
    /// gomme, et dans les essais.
    pub(crate) fn attendre_gomme_multi(&mut self) {
        self.attendre_gomme_multi_pendant(std::time::Duration::from_secs(120));
    }

    /// Comme `attendre_gomme_multi`, au plus `duree`. (Un résultat retenu par un geste en cours ne peut pas être posé : on n'attend
    /// pas pour lui.)
    pub(crate) fn attendre_gomme_multi_pendant(&mut self, duree: std::time::Duration) {
        let debut = std::time::Instant::now();
        while self.gomme_travaux.occupe() && debut.elapsed() < duree {
            // (Les commandes et l'enregistrement retenus ne partent pas d'ici : de la prochaine image d'écran.)
            let commandes = std::mem::take(&mut self.gomme_travaux.commandes);
            let sauvegarde = self.gomme_travaux.sauvegarde.take();
            self.poll_gomme_multi();
            self.gomme_travaux.commandes = commandes;
            self.gomme_travaux.sauvegarde = sauvegarde;
            if self.gomme_travaux.en_route == 0 && !self.gomme_travaux.differes.is_empty() {
                break;
            }
            if self.gomme_travaux.occupe() {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
}
