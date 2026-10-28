//! Peindre et remplir l'aplat de **plusieurs clés** d'un coup (Anthony, 2026-10-28) : quand l'aplat est la cible (la
//! sous-ligne de l'aplat du calque, ou un calque de couleur) et que plusieurs clés sont choisies dans la timeline, avec celle
//! qu'on voit parmi elles, le pinceau, le crayon, la gomme, le pot de peinture et le cutter font la même chose sur l'aplat de
//! chaque clé choisie, en **une seule étape d'annulation**.
//!
//! - **Traits** : le trait qu'on voit se fait comme toujours, sans latence ; les autres clés reçoivent chaque point du même geste
//!   en direct (`Miroir` : une brosse par clé, même réglage, mêmes points), et leurs étapes d'annulation se joignent à celle du
//!   trait au lâcher. Les autres clés ne calculent pas la zone fermée par leur trait (calcul de l'ordre de la centaine de
//!   millisecondes) : leur peinture n'y est pas retenue ; sur un aplat de strate elle est de toute façon cachée hors de la forme
//!   de l'aplat.
//! - **Remplir, retirer une zone** : la zone fermée par le trait de chaque clé se calcule sur un fil de travail (`Travail`) ;
//!   le résultat est posé quand il arrive (`poll_aplat_multi`, à chaque image d'écran), dans la même étape d'annulation si rien
//!   d'autre ne s'est passé depuis, sinon dans une étape à lui. Un dessin que l'utilisateur a retouché entre-temps n'est pas
//!   écrasé (sa version a changé : le résultat est abandonné et la barre d'état le dit).
//!
//! Rien ne change pour une seule clé choisie, ni quand l'aplat n'est pas la cible.

use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui::{Color32, Pos2};

use crate::app::{App, Tool};
use crate::brush::{StrokeMode, StrokeState};
use crate::calquecouleur::Murs;
use crate::document::{CelId, Strate};
use crate::history::Edit;
use crate::raster::{Raster, Tile, TILE};

/// Quel aplat vise le geste : la strate d'un calque de trait, ou le dessin d'un calque de couleur (numéros de calque).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Visee {
    Strate { layer: u64 },
    Couleur { layer: u64 },
}

/// Un trait en cours sur l'aplat d'une autre clé.
pub(crate) struct Miroir {
    /// Le dessin dont l'aplat reçoit le trait : le dessin du trait (strate) ou celui du calque de couleur.
    cel: CelId,
    strate: bool,
    stroke: StrokeState,
    /// Le point où le trait a commencé, le trait de cette clé (calque de couleur) et la fermeture de ses trous : le trait
    /// n'est retenu dans sa zone qu'après coup, sur un fil de travail (`retenir_dans_la_zone`).
    depart: Pos2,
    zone: Option<(CelId, u8)>,
    /// La coupe de la strate suit le même geste (la gomme coupe, le pinceau recoud), comme sur la clé qu'on voit.
    cut: Option<StrokeState>,
}

/// Les traits en cours sur les autres clés, et le numéro de l'historique au début du geste.
#[derive(Default)]
pub(crate) struct Miroirs {
    liste: Vec<Miroir>,
    serial: u64,
}

impl Miroirs {
    pub(crate) fn est_vide(&self) -> bool {
        self.liste.is_empty()
    }
}

/// Ce que le fil de travail doit faire sur l'aplat d'une clé.
struct Travail {
    cel: CelId,
    strate: bool,
    /// Le trait de cette clé (les murs de la zone).
    trait_: Raster,
    /// La peinture de l'aplat (de la strate, ou du dessin de couleur) et la coupe (strate seule).
    peinture: Raster,
    coupe: Raster,
    trous: u8,
    p: Pos2,
    color: Color32,
    /// Retirer la zone de l'aplat (cutter) au lieu de la remplir.
    retirer: bool,
    /// Les versions du dessin au départ : (dessin, strate) ; le résultat n'est posé que si rien n'a changé.
    versions: (u64, u64),
}

/// Ce qui revient du fil de travail : une zone remplie sur l'aplat d'une clé, ou les tuiles d'un trait de couleur retenues dans la
/// zone fermée par le trait de cette clé.
enum Retour {
    Rempli(Resultat),
    Retenu { cel: CelId, versions: (u64, u64), tiles: Vec<(usize, Option<Tile>)> },
}

struct Resultat {
    cel: CelId,
    strate: bool,
    peinture: Raster,
    coupe: Raster,
    versions: (u64, u64),
}

/// Le calcul d'un travail : la zone fermée par le trait où l'on a cliqué, remplie (ou retirée). Rien si le clic ne tombe dans
/// aucune zone (sur le trait lui-même, ou clé sans trait).
fn calculer(t: Travail) -> Option<Resultat> {
    let Travail { cel, strate, trait_, mut peinture, mut coupe, trous, p, color, retirer, versions } = t;
    if p.x < 0.0 || p.y < 0.0 {
        return None;
    }
    let (x, y) = (p.x as usize, p.y as usize);
    if strate {
        let mask = crate::fond::zone(&trait_, x, y, trous)?;
        if retirer {
            crate::fond::paint_mask(&mut coupe, &mask, Some(Color32::WHITE));
        } else {
            crate::fond::paint_mask(&mut peinture, &mask, Some(color));
            crate::fond::paint_mask(&mut coupe, &mask, None);
        }
    } else {
        let zone = Murs::new(&trait_, trous)?.zone(x, y)?;
        zone.peindre(&mut peinture, if retirer { None } else { Some(color) });
    }
    Some(Resultat { cel, strate, peinture, coupe, versions })
}

/// La forme de l'aplat d'un dessin, peinte de la couleur `new` dans sa strate (là où rien n'est peint) : l'aplat de ce dessin change
/// de couleur sans que le calque, et donc les autres dessins, en changent (`aplatcouleur.rs`). Le calcul de la forme est celui de
/// l'aplat affiché ; la peinture est opaque, l'aplat la montre dans sa forme seulement, au bord adouci de la forme.
fn teinter(cel: CelId, trait_: Raster, r: crate::fond::FondReglages, mut peinture: Raster, coupe: Raster, new: [u8; 3], versions: (u64, u64)) -> Option<Retour> {
    let cover = crate::fond::silhouette(&trait_, &r);
    let w = trait_.width;
    let encre = Color32::from_rgb(new[0], new[1], new[2]);
    for idx in 0..peinture.tiles.len() {
        let (ox, oy, tw, th) = peinture.tile_bounds(idx);
        for ty in 0..th {
            for tx in 0..tw {
                if cover[(oy + ty) * w + ox + tx] > 0 && peinture.get(ox + tx, oy + ty).a() == 0 {
                    peinture.tile_mut(idx)[ty * TILE + tx] = encre;
                }
            }
        }
    }
    Some(Retour::Rempli(Resultat { cel, strate: true, peinture, coupe, versions }))
}

/// Un trait de couleur fait sur l'aplat d'une autre clé sans zone (`avant` : les tuiles d'avant le trait, `apres` : celles de
/// maintenant, aux mêmes rangs) : les tuiles retenues dans la zone fermée par le trait de cette clé où il a commencé en `p`.
/// Rien s'il n'y a pas de zone (clé sans trait, départ sur le trait) : le trait reste tel quel.
fn retenir_dans_la_zone(trait_: &Raster, trous: u8, p: Pos2, avant: Vec<(usize, Option<Tile>)>, apres: Vec<(usize, Option<Tile>)>) -> Option<Vec<(usize, Option<Tile>)>> {
    if p.x < 0.0 || p.y < 0.0 {
        return None;
    }
    let zone = Murs::new(trait_, trous)?.zone(p.x as usize, p.y as usize)?;
    let (w, h) = (trait_.width, trait_.height);
    let mut out = Vec::new();
    for ((i, before), (_, after)) in avant.into_iter().zip(apres) {
        let (ox, oy, tw, th) = crate::raster::tile_bounds(w, h, i);
        let mut tile = after.clone().unwrap_or_else(crate::raster::empty_tile);
        let mut change = false;
        for ty in 0..th {
            for tx in 0..tw {
                if !zone.contient(ox + tx, oy + ty) {
                    let old = before.as_ref().map_or(Color32::TRANSPARENT, |b| b[ty * TILE + tx]);
                    if tile[ty * TILE + tx] != old {
                        tile[ty * TILE + tx] = old;
                        change = true;
                    }
                }
            }
        }
        if change {
            out.push((i, Some(tile)));
        }
    }
    Some(out)
}

/// Les travaux en route sur le fil, et ce qu'il faut pour joindre leur résultat à l'étape d'annulation du geste.
pub(crate) struct Travaux {
    tx: Sender<Option<Retour>>,
    rx: Receiver<Option<Retour>>,
    /// Combien de résultats on attend encore.
    pub(crate) en_route: usize,
    /// Le numéro de l'historique juste après l'étape du geste : un résultat qui arrive tant qu'il est le dernier la rejoint.
    serial: u64,
    nom: &'static str,
    /// Résultats abandonnés (un dessin retouché entre-temps).
    abandonnes: usize,
    /// Dessins remplis depuis le début de ce geste.
    poses: usize,
    /// Le travail en route est un changement de couleur (`aplatcouleur.rs`) : pas de phrase « zone posée » à son arrivée.
    silencieux: bool,
}

impl Default for Travaux {
    fn default() -> Self {
        let (tx, rx) = channel();
        Travaux { tx, rx, en_route: 0, serial: 0, nom: "Aplat : remplir", abandonnes: 0, poses: 0, silencieux: false }
    }
}

impl App {
    /// L'aplat que visent les outils en ce moment : la sous-ligne de l'aplat du calque courant, ou le calque de couleur courant
    /// (sauf en mode « Griffonner » : les griffonnages remplissent déjà les zones d'eux-mêmes).
    pub(crate) fn aplat_visee(&self) -> Option<Visee> {
        let l = self.layers().get(self.cur_layer)?;
        if self.aplat_en_cible() {
            return Some(Visee::Strate { layer: l.id });
        }
        l.couleur.as_ref().filter(|c| !c.griffonner).map(|_| Visee::Couleur { layer: l.id })
    }

    /// Les dessins dont l'aplat est visé par les clés choisies, avec l'image où commence chaque clé : `(dessin de l'aplat, image)`,
    /// chaque dessin une fois. Pour la strate, le dessin est celui du trait ; pour un calque de couleur, celui de la couleur
    /// (les clés de son calque de trait comptent aussi).
    fn aplats_des_cles(&self, v: Visee) -> Vec<(CelId, usize)> {
        let Some(panel) = self.doc.panels.get(self.cur_panel) else { return Vec::new() };
        let (layer, trait_id) = match v {
            Visee::Strate { layer } => (layer, 0),
            Visee::Couleur { layer } => (layer, panel.layers.iter().find(|l| l.id == layer).and_then(|l| l.couleur.as_ref()).map_or(0, |c| c.trait_id)),
        };
        let Some(l) = panel.layers.iter().find(|l| l.id == layer) else { return Vec::new() };
        let mut out: Vec<(CelId, usize)> = Vec::new();
        for k in self.key_sel.iter().filter(|k| k.panel == panel.id && (k.layer == layer || (trait_id != 0 && k.layer == trait_id))) {
            let Some(cel) = l.cel_at(k.start) else { continue };
            if !out.iter().any(|(c, _)| *c == cel) {
                out.push((cel, k.start));
            }
        }
        out
    }

    /// Les autres clés choisies que celle qu'on voit (`primary` : son dessin d'aplat) : rien si moins de deux clés sont
    /// choisies, ou si celle qu'on voit n'en est pas.
    fn autres_aplats(&self, v: Visee, primary: CelId) -> Vec<(CelId, usize)> {
        let all = self.aplats_des_cles(v);
        if all.len() < 2 || !all.iter().any(|(c, _)| *c == primary) {
            return Vec::new();
        }
        all.into_iter().filter(|(c, _)| *c != primary).collect()
    }

    // ------------------------------------------------------------------------------------ les traits

    /// Le trait de `primary` vient de commencer en `p` : chacune des autres clés choisies reçoit le même.
    pub(crate) fn miroirs_begin(&mut self, v: Visee, primary: CelId, p: Pos2, pressure: f32, pen: bool) {
        self.aplat_miroirs.liste.clear();
        let autres = self.autres_aplats(v, primary);
        if autres.is_empty() {
            return;
        }
        let Some(settings) = self.current_brush().copied() else { return };
        let erase = self.tool == Tool::Eraser;
        let strate = matches!(v, Visee::Strate { .. });
        let layer_id = match v {
            Visee::Strate { layer } | Visee::Couleur { layer } => layer,
        };
        let keep_alpha = !strate && self.layers().iter().find(|l| l.id == layer_id).is_some_and(|l| l.keep_alpha);
        let (w, h) = (self.doc.width, self.doc.height);
        let time = self.input_time;
        let tol = self.close_tolerance();
        // Calque de couleur dont les traits restent dans la zone fermée : le calque de trait et la fermeture des trous.
        let confiner = (!strate && !erase)
            .then(|| self.layers().iter().find(|l| l.id == layer_id).and_then(|l| l.couleur.as_ref()).filter(|c| c.zone && !c.griffonner).map(|c| (c.trait_id, c.trous)))
            .flatten();
        let mut liste = Vec::new();
        let mut dirty = Vec::new();
        for (cel, start) in autres {
            let zone = confiner.and_then(|(line, trous)| self.layers().iter().find(|l| l.id == line).and_then(|l| l.cel_at(start)).map(|c| (c, trous)));
            let mode = if erase { StrokeMode::Erase } else { StrokeMode::Paint(self.color) };
            let faire = |mode: StrokeMode, keep: bool| {
                StrokeState::new(cel, mode, settings, None, pen)
                    .with_textures(self.brushes.textures(&settings))
                    .with_feel(self.toucher)
                    .with_zoom(self.layer_zoom())
                    .with_secondary(self.couleurs.prefs.back)
                    .on_layer(keep, None)
                    .with_options(false, false, tol)
            };
            let mut stroke = faire(mode, keep_alpha);
            // (La coupe de la strate suit : la gomme la peint, le pinceau l'efface.)
            let mut cut = strate.then(|| faire(if erase { StrokeMode::Paint([255, 255, 255]) } else { StrokeMode::Erase }, false));
            let Some(c) = self.doc.cels.get_mut(&cel) else { continue };
            if strate {
                let s = c.fond.get_or_insert_with(|| Box::new(Strate::new(w, h)));
                stroke.add_point_at(&mut s.peinture.raster, p, pressure, time, &mut dirty);
                if let Some(cut) = cut.as_mut() {
                    cut.add_point_at(&mut s.coupe, p, pressure, time, &mut dirty);
                }
            } else {
                stroke.add_point_at(&mut c.raster, p, pressure, time, &mut dirty);
            }
            liste.push(Miroir { cel, strate, stroke, depart: p, zone, cut });
        }
        self.aplat_miroirs = Miroirs { liste, serial: self.history.serial() };
    }

    /// Le trait continue en `p` : les autres clés aussi.
    pub(crate) fn miroirs_point(&mut self, p: Pos2, pressure: f32) {
        if self.aplat_miroirs.est_vide() {
            return;
        }
        let time = self.input_time;
        let mut liste = std::mem::take(&mut self.aplat_miroirs.liste);
        let mut dirty = Vec::new();
        for m in &mut liste {
            let Some(c) = self.doc.cels.get_mut(&m.cel) else { continue };
            if m.strate {
                if let Some(s) = c.fond.as_mut() {
                    m.stroke.add_point_at(&mut s.peinture.raster, p, pressure, time, &mut dirty);
                    if let Some(cut) = m.cut.as_mut() {
                        cut.add_point_at(&mut s.coupe, p, pressure, time, &mut dirty);
                    }
                }
            } else {
                m.stroke.add_point_at(&mut c.raster, p, pressure, time, &mut dirty);
            }
        }
        self.aplat_miroirs.liste = liste;
    }

    /// Le trait est fini : les autres clés finissent le leur, et leurs étapes d'annulation rejoignent celle du trait (une seule
    /// étape). Si le trait n'en a pas laissé (rien de peint), elles ont une étape à elles.
    pub(crate) fn miroirs_finish(&mut self) {
        if self.aplat_miroirs.est_vide() {
            return;
        }
        let Miroirs { liste, serial } = std::mem::take(&mut self.aplat_miroirs);
        let mut edits: Vec<Edit> = Vec::new();
        let mut dirty = Vec::new();
        // Les traits de couleur à retenir dans la zone fermée de leur clé : (dessin, tuiles d'avant, rangs, départ, trait, trous).
        let mut retenir: Vec<(CelId, Vec<(usize, Option<Tile>)>, Pos2, CelId, u8)> = Vec::new();
        for mut m in liste {
            let Some(c) = self.doc.cels.get_mut(&m.cel) else { continue };
            if m.strate {
                if let Some(s) = c.fond.as_mut() {
                    edits.extend(m.stroke.finish(&mut s.peinture.raster, &mut dirty).map(|e| crate::fond::to_fond_edit(e, false)));
                    if let Some(cut) = m.cut.as_mut() {
                        edits.extend(cut.finish(&mut s.coupe, &mut dirty).map(|e| crate::fond::to_fond_edit(e, true)));
                    }
                    s.peinture.version += 1;
                }
            } else {
                let edit = m.stroke.finish(&mut c.raster, &mut dirty);
                if let (Some((line, trous)), Some(Edit::Tiles { tiles, .. })) = (m.zone, edit.as_ref()) {
                    retenir.push((m.cel, tiles.clone(), m.depart, line, trous));
                }
                edits.extend(edit);
            }
            c.version += 1;
        }
        // La zone fermée de chaque clé se calcule sur un fil de travail : le trait y est retenu à l'arrivée du résultat.
        for (cel, avant, depart, line, trous) in retenir {
            let (Some(c), Some(l)) = (self.doc.cels.get(&cel), self.doc.cels.get(&line)) else { continue };
            let apres: Vec<(usize, Option<Tile>)> = avant.iter().map(|(i, _)| (*i, c.raster.tiles.get(*i).cloned().flatten())).collect();
            let (versions, trait_) = ((c.version, 0), l.raster.clone());
            let tx = self.aplat_travaux.tx.clone();
            let ctx = self.ctx.clone();
            let spawned = std::thread::Builder::new().name("aplat-zone".into()).spawn(move || {
                let tiles = retenir_dans_la_zone(&trait_, trous, depart, avant, apres);
                let _ = tx.send(tiles.filter(|t| !t.is_empty()).map(|tiles| Retour::Retenu { cel, versions, tiles }));
                ctx.request_repaint();
            });
            if spawned.is_ok() {
                self.aplat_travaux.en_route += 1;
            }
        }
        self.renderer.invalidate_all();
        if edits.is_empty() {
            return;
        }
        self.modified = true;
        let nom = if self.tool == Tool::Eraser { "Aplat : gomme" } else { "Aplat : peinture" };
        if self.history.serial() != serial {
            for e in edits {
                self.history.extend_last(e);
            }
        } else {
            let avant = self.history.context.replace(nom.to_string());
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
        }
    }

    // ------------------------------------------------------------------------------------ remplir, retirer une zone

    /// Le pot de peinture (`retirer` faux) ou le cutter (`retirer`) vient d'agir en `p` sur l'aplat visé, `avant` étant le numéro de
    /// l'historique avant lui : si cela a laissé une étape et que plusieurs clés sont choisies, la même zone se remplit (se
    /// retire) sur l'aplat des autres clés, sur un fil de travail ; le résultat vient dans l'étape du geste (`poll_aplat_multi`).
    pub(crate) fn aplat_autres_cles(&mut self, p: Pos2, retirer: bool, avant: u64) {
        if self.history.serial() == avant {
            return;
        }
        let Some(v) = self.aplat_visee() else { return };
        let Some(primary) = (match v {
            Visee::Strate { .. } => self.cel_here(),
            Visee::Couleur { .. } => self.cel_here(),
        }) else {
            return;
        };
        let autres = self.autres_aplats(v, primary);
        if autres.is_empty() {
            return;
        }
        let color = match v {
            Visee::Strate { .. } => {
                let fill = self.perso_fond_ou(self.color);
                Color32::from_rgb(fill[0], fill[1], fill[2])
            }
            Visee::Couleur { .. } => Color32::from_rgb(self.color[0], self.color[1], self.color[2]),
        };
        let li = self.cur_layer;
        let (w, h) = (self.doc.width, self.doc.height);
        let (trous, line_id) = match v {
            Visee::Strate { .. } => (self.layers()[li].fond.map_or(crate::fond::TROUS, |f| f.trous), 0),
            Visee::Couleur { .. } => {
                let c = self.layers()[li].couleur.as_ref();
                (c.map_or(crate::calquecouleur::TROUS, |c| c.trous), c.map_or(0, |c| c.trait_id))
            }
        };
        let mut lances = 0;
        for (cel, start) in autres {
            let strate = matches!(v, Visee::Strate { .. });
            // Le trait de cette clé : le dessin lui-même (strate), ou le dessin du calque de trait à la même image.
            let trait_cel = if strate {
                Some(cel)
            } else {
                self.layers().iter().find(|l| l.id == line_id).and_then(|l| l.cel_at(start))
            };
            let Some(trait_) = trait_cel.and_then(|t| self.doc.cels.get(&t)).map(|c| c.raster.clone()) else { continue };
            let Some(c) = self.doc.cels.get(&cel) else { continue };
            let (peinture, coupe, versions) = if strate {
                match c.fond.as_deref() {
                    Some(s) => (s.peinture.raster.clone(), s.coupe.clone(), (c.version, s.peinture.version)),
                    None => (Raster::new(w, h), Raster::new(w, h), (c.version, u64::MAX)),
                }
            } else {
                (c.raster.clone(), Raster::new(w, h), (c.version, 0))
            };
            let job = Travail { cel, strate, trait_, peinture, coupe, trous, p, color, retirer, versions };
            let tx = self.aplat_travaux.tx.clone();
            let ctx = self.ctx.clone();
            let spawned = std::thread::Builder::new().name("aplat-cles".into()).spawn(move || {
                let _ = tx.send(calculer(job).map(Retour::Rempli));
                ctx.request_repaint();
            });
            if spawned.is_ok() {
                lances += 1;
            }
        }
        if lances > 0 {
            let t = &mut self.aplat_travaux;
            t.en_route += lances;
            t.serial = self.history.serial();
            t.nom = if retirer { "Aplat : retirer la zone" } else { "Aplat : remplir" };
            t.abandonnes = 0;
            t.poses = 0;
            t.silencieux = false;
            self.status = format!("Aplat : la zone se {} sur {} autre{} clé{} (calcul en arrière-plan).", if retirer { "retire" } else { "remplit" }, lances, if lances > 1 { "s" } else { "" }, if lances > 1 { "s" } else { "" });
        }
    }

    /// Une fois par image d'écran : pose les résultats arrivés. Rien au repos.
    pub(crate) fn poll_aplat_multi(&mut self) {
        if self.aplat_travaux.en_route == 0 {
            return;
        }
        let arrives: Vec<Option<Retour>> = self.aplat_travaux.rx.try_iter().collect();
        for r in arrives {
            self.aplat_travaux.en_route = self.aplat_travaux.en_route.saturating_sub(1);
            match r {
                None => {}
                Some(Retour::Rempli(r)) => {
                    if self.poser_resultat(r) {
                        self.aplat_travaux.poses += 1;
                    } else {
                        self.aplat_travaux.abandonnes += 1;
                    }
                }
                Some(Retour::Retenu { cel, versions, tiles }) => self.poser_retenu(cel, versions, tiles),
            }
        }
        if self.aplat_travaux.en_route > 0 {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            let t = &mut self.aplat_travaux;
            let (poses, abandonnes) = (std::mem::take(&mut t.poses), std::mem::take(&mut t.abandonnes));
            if abandonnes > 0 {
                self.status = format!("Aplat : {abandonnes} clé(s) retouchée(s) pendant le calcul n'ont pas été changées ; {poses} remplie(s).");
            } else if poses > 0 && !t.silencieux {
                self.status = format!("Aplat : zone posée sur {} autre(s) clé(s).", poses);
            }
        }
    }

    /// Pose le résultat d'un travail si le dessin n'a pas changé depuis son départ ; vrai si posé.
    fn poser_resultat(&mut self, r: Resultat) -> bool {
        let Some(c) = self.doc.cels.get_mut(&r.cel) else { return false };
        let actuelles = (c.version, if r.strate { c.fond.as_ref().map_or(u64::MAX, |s| s.peinture.version) } else { 0 });
        if actuelles != r.versions {
            return false;
        }
        let mut edits: Vec<Edit> = Vec::new();
        if r.strate {
            let (w, h) = (c.raster.width, c.raster.height);
            let s = c.fond.get_or_insert_with(|| Box::new(Strate::new(w, h)));
            let t = crate::multisel::changed_tiles(&s.peinture.raster, &r.peinture);
            if !t.is_empty() {
                edits.push(Edit::FondTiles { cel: r.cel, tiles: t, coupe: false });
            }
            let t = crate::multisel::changed_tiles(&s.coupe, &r.coupe);
            if !t.is_empty() {
                edits.push(Edit::FondTiles { cel: r.cel, tiles: t, coupe: true });
            }
            s.peinture.raster = r.peinture;
            s.coupe = r.coupe;
            s.peinture.version += 1;
        } else {
            let t = crate::multisel::changed_tiles(&c.raster, &r.peinture);
            if !t.is_empty() {
                edits.push(Edit::Tiles { cel: r.cel, tiles: t });
            }
            c.raster = r.peinture;
        }
        c.version += 1;
        self.renderer.invalidate_all();
        self.modified = true;
        if edits.is_empty() {
            return true;
        }
        if self.history.serial() == self.aplat_travaux.serial {
            for e in edits {
                self.history.extend_last(e);
            }
        } else {
            let avant = self.history.context.replace(format!("{} (autres clés)", self.aplat_travaux.nom));
            self.history.push(Edit::Group(edits));
            self.history.context = avant;
        }
        self.aplat_travaux.serial = self.history.serial();
        true
    }

    /// Le trait de couleur d'une autre clé est retenu dans la zone fermée par son trait : les tuiles retenues remplacent celles
    /// du trait sans zone, si le dessin n'a pas changé depuis (sinon un geste ou une annulation est passé par là : le dessin est
    /// laissé tel quel). L'étape d'annulation du trait ne change pas : elle rend les tuiles d'avant, qui couvrent celles-ci.
    fn poser_retenu(&mut self, cel: CelId, versions: (u64, u64), tiles: Vec<(usize, Option<Tile>)>) {
        let Some(c) = self.doc.cels.get_mut(&cel) else { return };
        if (c.version, 0) != versions {
            return;
        }
        for (i, t) in tiles {
            c.raster.replace_slot(i, t);
        }
        c.version += 1;
        self.renderer.invalidate_all();
    }

    /// Le dessin `cel` du calque de rang `li` : sa forme d'aplat est peinte de la couleur `new` sur un fil de travail ; le résultat
    /// rejoint l'étape d'annulation `serial` (`aplatcouleur.rs`). Faux si le travail n'a pas pu partir.
    pub(crate) fn lancer_teinte(&mut self, li: usize, cel: CelId, new: [u8; 3], serial: u64) -> bool {
        let Some(r) = self.layers().get(li).and_then(|l| l.fond) else { return false };
        let Some(c) = self.doc.cels.get(&cel) else { return false };
        let (w, h) = (self.doc.width, self.doc.height);
        let (peinture, coupe, versions) = match c.fond.as_deref() {
            Some(s) => (s.peinture.raster.clone(), s.coupe.clone(), (c.version, s.peinture.version)),
            None => (Raster::new(w, h), Raster::new(w, h), (c.version, u64::MAX)),
        };
        let trait_ = c.raster.clone();
        let tx = self.aplat_travaux.tx.clone();
        let ctx = self.ctx.clone();
        let spawned = std::thread::Builder::new().name("aplat-teinte".into()).spawn(move || {
            let _ = tx.send(teinter(cel, trait_, r, peinture, coupe, new, versions));
            ctx.request_repaint();
        });
        if spawned.is_err() {
            return false;
        }
        let t = &mut self.aplat_travaux;
        t.en_route += 1;
        t.serial = serial;
        t.nom = "Aplat : couleur";
        t.silencieux = true;
        true
    }

    /// Attend les résultats en route et les pose : avant d'enregistrer (le fichier écrit les a), et dans les essais.
    pub(crate) fn attendre_aplat_multi(&mut self) {
        let debut = std::time::Instant::now();
        while self.aplat_travaux.en_route > 0 && debut.elapsed() < std::time::Duration::from_secs(60) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll_aplat_multi();
        }
    }
}
