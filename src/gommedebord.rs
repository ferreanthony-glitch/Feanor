//! La gomme de la planche efface aussi ce qui dépasse des cases (partie 131).
//!
//! # Pourquoi
//!
//! Depuis la partie 131 un trait appartient tout entier à la case où il commence, et la planche le montre où qu'il aille : sur le papier, entre
//! les cases, par-dessus les cases voisines. Avant, ce qui sortait du champ d'une case passait au papier, et la gomme posée sur le papier
//! l'effaçait. Sans ce fichier, poser la gomme sur le bout d'un trait qui dépasse ne ferait plus rien : il faudrait deviner de quelle case il
//! vient et partir de dedans. **Ce qu'on voit sur le papier se gomme sur le papier.**
//!
//! # Ce que ça fait
//!
//! La gomme partie **hors de toute image** (le geste de gomme de la planche, `BoardDrag::Erase`, qui efface déjà les traits du papier et les
//! notes) efface aussi, sous son cercle, ce que les cases ont dessiné **hors de leur cadre** :
//! - un **trait vectoriel** : entier, comme un trait du papier (la gomme de la planche prend des traits, elle ne les entame pas), **si elle le
//!   touche hors du cadre de sa case** ; un trait qu'elle ne toucherait que dans le cadre, ou qui y est tout entier, reste ;
//! - des **pixels** (calque bitmap) : ceux qui sont sous le cercle, hors de l'image de leur case.
//!
//! **Dans l'image d'une case, son dessin est à elle** : tant que la gomme de la planche passe dans le cadre d'une case, elle ne touche pas au
//! dessin de cette case (elle y efface encore ce qui vient d'ailleurs). Pour gommer dans une case, on y pose la gomme, comme avant.
//!
//! # Ce qu'elle ne touche jamais
//!
//! - **Rien quand le storyboard est verrouillé** (les dessins des plans ne changent plus), ni quand tout le film est écarté.
//! - **Rien avec le passe-partout** : il cache ce qui dépasse des cases, on ne gomme pas ce qu'on ne voit pas.
//! - **Seulement là où la planche montre le dessin** : jusqu'où la vignette de la case s'étend (l'encre du plan, quatre cadres au plus,
//!   `Thumbs::debord_de_case`).
//! - Les calques qu'on ne dessine pas depuis la planche : verrouillés, masqués, transformés, les calques d'effet, de couleur, de vidéo,
//!   d'annotation ; et, dans un film partagé, les plans tenus par un autre poste.
//! - **Les dessins que plusieurs cases montrent** (`render::dessins_communs` : le Décor, une planche-mère, un calque de toute la scène, un
//!   dessin tenu d'une marque d'image à l'autre) : ce qui dépasse d'une case est dans le cadre d'une autre. On les gomme dans leur case.
//!
//! Une seule étape d'annulation pour tout le geste, avec ce qu'il efface du papier (`History::marque`, `regrouper_depuis`). Aucun rejeu du
//! moteur sur le fil de l'interface : un dessin vectoriel au pinceau se refait en arrière-plan (`rejeu.rs`).
//!
//! # Limite
//!
//! Le trait d'une case qui passe **dans l'image d'une autre case** s'y voit (la feuille) mais ne s'y gomme pas en posant la gomme dans cette
//! autre case : la gomme y est celle de son dessin à elle. On le gomme depuis le papier (le geste continue par-dessus les cases), depuis sa
//! case, ou on le prend au lasso (Ctrl + clic) et Suppr.

use eframe::egui::{pos2, vec2, Color32, Pos2, Rect};

use crate::app::App;
use crate::board::BoardLayout;
use crate::document::{CelId, LayerKind};
use crate::history::Edit;
use crate::horschamp::Pose;
use crate::raster::{Tile, TILE};
use crate::vector::{Element, VPoint};

/// Au-delà de ce rayon, en pixels du dessin, la gomme ne passe pas par cette case : le cercle n'a plus de sens (tout près de l'horizon d'une
/// caméra basculée, une unité de la planche couvre des milliers de pixels de la feuille) et le lire gèlerait l'écran.
const RAYON_MAX: f32 = 2048.0;

/// Le geste de gomme de la planche en cours : ce qu'il a déjà touché dans les dessins des cases (leur état d'avant n'entre qu'une fois dans
/// l'étape d'annulation).
#[derive(Debug)]
pub(crate) struct GommeDebord {
    /// La marque de l'historique au début du geste : tout ce qu'il pose ne fait qu'une étape.
    marque: u64,
    /// Le nombre d'étapes d'annulation à la fin de la dernière image du geste : s'il a baissé depuis, on a annulé pendant le geste, et ce
    /// qu'il croyait avoir déjà rangé ne l'est plus.
    etapes: usize,
    /// Le point d'avant (planche) : la gomme efface tout le long du chemin, pas seulement sous le pointeur à chaque image d'écran.
    dernier: Option<Pos2>,
    cels: Vec<CelId>,
    tuiles: Vec<(CelId, usize)>,
}

impl App {
    /// Un geste de gomme de la planche commence : rien ne reste d'un geste d'avant qui ne se serait pas fini (la vue quittée pendant l'appui).
    pub(crate) fn gomme_debords_oublier(&mut self) {
        self.vignettes.gomme_debord = None;
    }

    /// Le geste de gomme de la planche commence (ou continue) : sa marque dans l'historique, prise avant qu'il ne pose quoi que ce soit.
    pub(crate) fn gomme_debords_commencer(&mut self) {
        if self.vignettes.gomme_debord.is_none() {
            self.vignettes.gomme_debord = Some(GommeDebord { marque: self.history.marque(), etapes: self.history.undo_len(), dernier: None, cels: Vec::new(), tuiles: Vec::new() });
        }
    }

    /// La fin d'une image d'écran du geste, après tout ce qu'il a posé dans l'historique (le papier aussi) : le compte d'étapes à retrouver.
    pub(crate) fn gomme_debords_noter(&mut self) {
        let etapes = self.history.undo_len();
        if let Some(g) = self.vignettes.gomme_debord.as_mut() {
            g.etapes = etapes;
        }
    }

    /// Le geste de gomme de la planche est fini : tout ce qu'il a posé dans l'historique (le papier, les notes, les dessins des cases) ne fait
    /// qu'une étape d'annulation.
    pub(crate) fn gomme_debords_finir(&mut self) {
        if let Some(g) = self.vignettes.gomme_debord.take() {
            self.history.regrouper_depuis(g.marque);
        }
    }

    /// La gomme de la planche arrive en `p` (planche), de rayon `radius` : ce que les cases ont dessiné hors de leur cadre et que son cercle
    /// touche, le long du chemin depuis son point d'avant (voir l'en-tête). Vrai si quelque chose a été effacé.
    pub(crate) fn gomme_debords(&mut self, layout: &BoardLayout, p: Pos2, radius: f32) -> bool {
        // (Annulé pendant le geste : les états d'avant déjà rangés sont partis de l'historique ; on repart d'une marque neuve.)
        let (etapes, marque) = (self.history.undo_len(), self.history.marque());
        let Some(g) = self.vignettes.gomme_debord.as_mut() else { return false };
        if etapes < g.etapes {
            g.cels.clear();
            g.tuiles.clear();
            g.marque = marque;
        }
        let de = g.dernier.replace(p).unwrap_or(p);
        // Le storyboard verrouillé : les dessins des plans ne changent plus. Le passe-partout : ce qui dépasse des cases est caché.
        if self.doc.storyboard_locked || self.doc.film_vide() || self.vignettes.annot.marie || !radius.is_finite() || radius <= 0.0 {
            return false;
        }
        let n = ((p - de).length() / (radius * 0.5).max(1e-3)).ceil().clamp(1.0, 64.0) as usize;
        let mut fait = false;
        for i in 1..=n {
            fait |= self.gomme_debords_en(layout, de + (p - de) * (i as f32 / n as f32), radius);
        }
        if fait {
            self.modified = true;
            self.ctx.request_repaint();
        }
        fait
    }

    /// La planche montre-t-elle du dessin de la case `k` sous le cercle de la gomme en `p` : hors de son image, et pas plus loin que là où sa
    /// vignette s'étend (l'encre que la case est seule à montrer, `render::MARGE_MAX_CADRES` cadres au plus). Sans rien composer.
    fn gomme_atteint(&mut self, layout: &BoardLayout, k: usize, p: Pos2, radius: f32) -> bool {
        let c = layout.cards[k];
        let mv = layout.move_of(k);
        let zone = mv.map_or(c.thumb, |m| m.tuile.rect);
        let loin = crate::render::MARGE_MAX_CADRES as f32;
        // (D'abord sans rien calculer : hors de portée de toute vignette.)
        if !zone.expand2(zone.size() * loin + vec2(radius, radius)).contains(p) {
            return false;
        }
        let d = self.thumbs.debord_de_case(&self.doc, c.panel, c.frame, c.id, mv.map(|m| &m.tuile.zone)).map(|v| v.min(loin));
        if d.iter().all(|v| *v <= 0.0) {
            return false;
        }
        Rect::from_min_max(zone.min - vec2(d[0] * zone.width(), d[1] * zone.height()), zone.max + vec2(d[2] * zone.width(), d[3] * zone.height())).expand(radius).contains(p)
    }

    fn gomme_debords_en(&mut self, layout: &BoardLayout, p: Pos2, radius: f32) -> bool {
        let mut fait = false;
        for k in 0..layout.cards.len() {
            // Dans l'image de cette case, son dessin est à elle.
            if crate::horschamp::dans_image(layout, k, p) || !self.gomme_atteint(layout, k, p, radius) {
                continue;
            }
            let c = layout.cards[k];
            // (Dans un film partagé, on n'écrit jamais dans un plan qu'on ne tient pas.)
            if self.partage.as_ref().is_some_and(|s| s.peut_modifier_plan(c.plan).is_err()) {
                continue;
            }
            let Some(pose) = crate::horschamp::pose_de_la_case(&self.doc, layout, k) else { continue };
            let Some(pd) = pose.dessin_sur(p) else { continue };
            let rd = radius / pose.echelle_en(pd).max(1e-6);
            if !rd.is_finite() || rd > RAYON_MAX {
                continue;
            }
            let Some(panel) = self.doc.panels.get(c.panel) else { continue };
            // (Ce que la case n'est pas seule à montrer n'est pas à gommer d'ici : son débord est dans le cadre d'une autre case.)
            let communs = crate::render::dessins_communs(&self.doc, c.panel, c.frame);
            let calques = &panel.layers;
            let mut cibles: Vec<(CelId, LayerKind)> = Vec::new();
            for (rang, l) in calques.iter().enumerate() {
                if l.folder || l.effet.is_some() || l.video.is_some() || l.decor || l.annotation || l.couleur.is_some() {
                    continue;
                }
                if !l.drawable() || crate::folders::locked_by_folder(calques, rang) || crate::folders::effect(calques, rang).is_none() {
                    continue;
                }
                let vue = l.transform_seen_at(&self.doc.camera_seen(c.panel, c.frame), c.frame);
                if !l.pose(c.frame).is_identity() || vue.is_none_or(|t| !t.is_identity()) {
                    continue;
                }
                let Some(cel) = l.cel_at(c.frame) else { continue };
                if !communs.contains(&cel) && !cibles.iter().any(|(autre, _)| *autre == cel) {
                    cibles.push((cel, l.kind));
                }
            }
            for (cel, sorte) in cibles {
                fait |= match sorte {
                    LayerKind::Vector => self.gomme_debord_vecteur(layout, k, cel, &pose, pd, rd),
                    LayerKind::Bitmap => self.gomme_debord_bitmap(layout, k, cel, &pose, pd, rd),
                };
            }
        }
        fait
    }

    /// Les traits du dessin vectoriel `cel` de la case `k` que le cercle (`pd`, `rd` : dans le dessin) touche **hors de l'image de la case** :
    /// retirés, entiers.
    fn gomme_debord_vecteur(&mut self, layout: &BoardLayout, k: usize, cel: CelId, pose: &Pose, pd: Pos2, rd: f32) -> bool {
        let Some(d) = self.drawing(cel) else { return false };
        let dehors = |q: Pos2| !crate::horschamp::dans_image(layout, k, pose.planche(q));
        let touches: Vec<usize> = d
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Element::Stroke { points, .. } if touche_dehors(points, pd, rd, &dehors)))
            .map(|(i, _)| i)
            .collect();
        if touches.is_empty() {
            return false;
        }
        let au_pinceau = touches.iter().any(|i| matches!(&d.elements[*i], Element::Stroke { brush: Some(_), .. }));
        let region = touches.iter().fold(Rect::NOTHING, |r, i| r.union(d.elements[*i].bounds()));
        // L'état d'avant du dessin : une fois par geste.
        let premier = self.vignettes.gomme_debord.as_ref().is_some_and(|g| !g.cels.contains(&cel));
        if premier {
            let avant = d.clone();
            if let Some(g) = self.vignettes.gomme_debord.as_mut() {
                g.cels.push(cel);
            }
            self.history.push(Edit::Vector { cel, drawing: avant });
        }
        if let Some(d) = self.drawing_mut(cel) {
            for i in touches.into_iter().rev() {
                d.elements.remove(i);
            }
        }
        // Aucun rejeu du moteur sur le fil de l'interface : un dessin au pinceau se refait en arrière-plan.
        if au_pinceau {
            if let Some(c) = self.doc.cels.get_mut(&cel) {
                c.version += 1;
            }
            self.rejeu_cel(cel);
        } else {
            self.refresh_cel_region(cel, region.expand(2.0));
        }
        true
    }

    /// Les pixels du dessin bitmap `cel` de la case `k` sous le cercle (`pd`, `rd` : dans le dessin), hors de l'image de la case : effacés.
    fn gomme_debord_bitmap(&mut self, layout: &BoardLayout, k: usize, cel: CelId, pose: &Pose, pd: Pos2, rd: f32) -> bool {
        let t = TILE as i64;
        let (x0, y0, x1, y1) = ((pd.x - rd).floor() as i64, (pd.y - rd).floor() as i64, (pd.x + rd).ceil() as i64, (pd.y + rd).ceil() as i64);
        let Some(c) = self.doc.cels.get(&cel) else { return false };
        // Tuile par tuile : seules celles qui ont du dessin coûtent une lecture (un grand cercle sur une feuille presque vide ne lit presque rien).
        let mut a_effacer: Vec<(i64, i64)> = Vec::new();
        for ty in y0.div_euclid(t)..=y1.div_euclid(t) {
            for tx in x0.div_euclid(t)..=x1.div_euclid(t) {
                if c.raster.tile_at(tx as i32, ty as i32).is_none() {
                    continue;
                }
                for y in y0.max(ty * t)..=y1.min(ty * t + t - 1) {
                    for x in x0.max(tx * t)..=x1.min(tx * t + t - 1) {
                        let q = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        if (q - pd).length_sq() <= rd * rd && c.raster.get_i(x, y).a() != 0 && !crate::horschamp::dans_image(layout, k, pose.planche(q)) {
                            a_effacer.push((x, y));
                        }
                    }
                }
            }
        }
        if a_effacer.is_empty() {
            return false;
        }
        let Some(c) = self.doc.cels.get_mut(&cel) else { return false };
        let mut changees: Vec<usize> = Vec::new();
        let mut vieux: Vec<(usize, Option<Tile>)> = Vec::new();
        for (x, y) in a_effacer {
            let id = c.raster.id_of(x.div_euclid(t) as i32, y.div_euclid(t) as i32);
            if !changees.contains(&id) {
                changees.push(id);
                // L'état d'avant de la tuile : une fois par geste.
                if let Some(g) = self.vignettes.gomme_debord.as_mut().filter(|g| !g.tuiles.contains(&(cel, id))) {
                    g.tuiles.push((cel, id));
                    vieux.push((id, c.raster.slot(id).cloned()));
                }
            }
            c.raster.tile_mut(id)[(y.rem_euclid(t) * t + x.rem_euclid(t)) as usize] = Color32::TRANSPARENT;
        }
        c.version += 1;
        self.renderer.invalidate_tiles(&changees);
        if !vieux.is_empty() {
            self.history.push(Edit::Tiles { cel, tiles: vieux });
        }
        true
    }
}

/// Le cercle (`p`, `rayon`) touche-t-il le trait `points` **en un endroit `dehors`** (le point du trait le plus proche du centre, sur un de
/// ses segments à portée) ? Un trait tout entier dans le cadre de sa case, ou que la gomme n'atteint que dans ce cadre, n'est pas touché.
fn touche_dehors(points: &[VPoint], p: Pos2, rayon: f32, dehors: &dyn Fn(Pos2) -> bool) -> bool {
    match points {
        [] => false,
        [a] => a.pos().distance(p) <= rayon + a.r && dehors(a.pos()),
        pts => pts.windows(2).any(|w| {
            let (a, b) = (w[0].pos(), w[1].pos());
            let ab = b - a;
            let t = if ab.length_sq() > 0.0 { ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + ab * t;
            q.distance(p) <= rayon + w[0].r.max(w[1].r) && dehors(q)
        }),
    }
}
