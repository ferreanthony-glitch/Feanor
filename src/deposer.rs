//! Déposer des images sur la planche des Vignettes (Anthony, 2026-10-21) : depuis l'Explorateur de Windows ou depuis
//! le panneau Références, sur une case, entre deux cases, ou dans les cases vides après la fin.
//!
//! - **Sur une case** : l'image va dans cette case, sur un calque bitmap « Image » **à part, sous le dessin**, ajusté au cadre de la
//!   caméra (tout le cadre, proportions gardées, centré). Ce calque est un calque ordinaire (on peut l'estomper, le cacher, le
//!   verrouiller). Dans une case qui n'est qu'une marque d'image d'un plan, l'image n'occupe que les images de cette case.
//!   Déposer une autre image sur la même case remplace celle du calque « Image ».
//! - **Plusieurs images** : elles remplissent les cases à la suite de celle qu'on vise (l'une après l'autre, dans l'ordre du dépôt) ;
//!   quand les cases du storyboard manquent, de nouveaux plans se font à la fin.
//! - **Dans les cases vides** après la dernière image : un nouveau plan par image, dans l'ordre (les cases vides sautées avant celle
//!   qu'on vise deviennent des plans vides, comme quand on y dessine). Si la barre devant la première case vide est grise, la première
//!   image fait une image de plus du dernier plan.
//! - **Entre deux cases** (sur la barre) : un nouveau plan par image, insérés là ; entre deux images d'un même plan, le plan est
//!   d'abord coupé à cet endroit.
//! - **Une seule étape d'annulation** pour tout le dépôt (« Déposer des images »).
//! - **Sans latence** : lire, réduire et poser l'image sur la feuille se fait sur un fil de fond (`Travail`) ; l'interface n'ajoute que le
//!   résultat au document, quand il est prêt.
//! - Depuis le panneau Références, on glisse la référence (ou les références choisies) hors de son panneau ; l'Explorateur de Windows
//!   ne donne pas l'endroit du lâcher à la fenêtre : on lit le curseur de l'écran (`curseur_fenetre`).

use std::path::PathBuf;
use std::thread::JoinHandle;

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Stroke, StrokeKind};

use crate::app::App;
use crate::board::{self, BoardLayout, Hit};
use crate::boardui::Xf;
use crate::document::{Cel, CelId, Document, Layer, LayerKind};
use crate::raster::{premultiply, Raster, TILE};

/// Le nom du calque qui reçoit les images déposées.
pub const NOM_DU_CALQUE: &str = "Image";

/// Ce que le panneau Références donne à la planche quand on glisse des références hors de son panneau.
#[derive(Clone, Debug)]
pub(crate) struct RefsDrag {
    pub ids: Vec<u64>,
}

/// Ce que vise un dépôt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cible {
    /// Une case (par le numéro de sa vignette) ; les images suivantes vont aux cases suivantes.
    Case(u64),
    /// La case vide `e` (0 : la première, juste après la dernière vignette).
    Vide(usize),
    /// Entre cette vignette et la suivante.
    Entre(u64),
}

/// Ce que la planche vise au point `p` (planche) ; rien hors des cases, barres et cases vides.
pub(crate) fn cible_en(layout: &BoardLayout, p: Pos2, bar_min_w: f32) -> Option<Cible> {
    match board::hit(layout, p, bar_min_w) {
        Hit::Thumb(k) | Hit::Note(k) => Some(Cible::Case(layout.cards[k].id)),
        // (La barre devant la première case vide d'une séquence : cette case-là.)
        Hit::Bar(after) if layout.vers_pale(after) => Some(Cible::Vide(layout.groupe_de(after).map_or(0, |g| g.vides.start))),
        Hit::Bar(after) => Some(Cible::Entre(layout.cards[after].id)),
        Hit::Empty(e) => Some(Cible::Vide(e)),
        Hit::Titre(_) => None,
        Hit::Nothing => layout.cards.iter().find(|c| c.card.contains(p)).map(|c| Cible::Case(c.id)),
    }
}

/// Les extensions d'images que SHOT sait lire.
pub(crate) fn est_une_image(p: &std::path::Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
}

/// Un dépôt en cours de préparation sur un fil de fond.
pub(crate) struct Travail {
    handle: JoinHandle<Vec<Result<Raster, String>>>,
    noms: Vec<String>,
    cible: Cible,
    /// Le projet où il a été lâché (un autre projet : abandonné).
    epoch: u64,
}

/// La feuille et le cadre, ce qu'il faut pour poser une image sans toucher au document.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Feuille {
    pub sheet: [usize; 2],
    pub frame: [usize; 2],
    /// Coin haut gauche du cadre sur la feuille.
    pub origin: (i64, i64),
}

impl Feuille {
    pub fn de(doc: &Document) -> Feuille {
        let r = doc.frame_rect();
        Feuille { sheet: [doc.width, doc.height], frame: doc.frame, origin: (r.min.x as i64, r.min.y as i64) }
    }
}

/// L'image ajustée au cadre (en entier, proportions gardées, centrée), posée sur une feuille.
pub(crate) fn raster_ajuste(img: image::RgbaImage, f: Feuille) -> Raster {
    let [fw, fh] = f.frame;
    let (iw, ih) = (img.width().max(1) as f32, img.height().max(1) as f32);
    let k = (fw as f32 / iw).min(fh as f32 / ih);
    let (nw, nh) = (((iw * k).round() as u32).clamp(1, fw.max(1) as u32), ((ih * k).round() as u32).clamp(1, fh.max(1) as u32));
    let img = if (nw, nh) != (img.width(), img.height()) { image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle) } else { img };
    let x0 = f.origin.0 + (fw as i64 - nw as i64) / 2;
    let y0 = f.origin.1 + (fh as i64 - nh as i64) / 2;
    let mut out = Raster::new(f.sheet[0], f.sheet[1]);
    for (y, row) in img.rows().enumerate() {
        let sy = y0 + y as i64;
        if sy < 0 || sy >= f.sheet[1] as i64 {
            continue;
        }
        for (x, p) in row.enumerate() {
            let sx = x0 + x as i64;
            if sx < 0 || sx >= f.sheet[0] as i64 || p[3] == 0 {
                continue;
            }
            let (sx, sy) = (sx as usize, sy as usize);
            let idx = out.tile_index_of(sx, sy);
            out.tile_mut(idx)[(sy % TILE) * TILE + sx % TILE] = premultiply(p[0], p[1], p[2], p[3]);
        }
    }
    out
}

/// Le calque « Image » du plan `pi` (le crée tout en bas s'il manque), l'image `cel` sur les images `[a, b)`.
pub(crate) fn poser_image(doc: &mut Document, pi: usize, a: usize, b: usize, cel: CelId) {
    let n = doc.panels[pi].frame_count();
    let b = b.min(n);
    let existant = doc.panels[pi].layers.iter().position(|l| l.name == NOM_DU_CALQUE && l.kind == LayerKind::Bitmap && !l.folder && l.effet.is_none() && l.couleur.is_none() && l.video.is_none());
    let li = existant.unwrap_or_else(|| {
        let mut layer = Layer::new(NOM_DU_CALQUE, LayerKind::Bitmap);
        layer.frames = vec![None; n];
        doc.panels[pi].layers.insert(0, layer);
        0
    });
    let layer = &mut doc.panels[pi].layers[li];
    if layer.frames.len() < n {
        layer.frames.resize(n, None);
    }
    for f in a..b {
        layer.frames[f] = Some(cel);
    }
}

impl App {
    /// Lance le travail de fond pour `sources` (fichiers) visant `cible`.
    fn depot_lancer(&mut self, sources: Vec<PathBuf>, cible: Cible) {
        let feuille = Feuille::de(&self.doc);
        let noms: Vec<String> = sources.iter().map(|p| p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned())).collect();
        let handle = std::thread::Builder::new().name("depot-images".into()).spawn(move || {
            crate::parallele::baisser_la_priorite_ici();
            sources.iter().map(|p| crate::io::load_image(p).map(|i| raster_ajuste(i, feuille))).collect()
        });
        match handle {
            Ok(handle) => {
                self.status = format!("{} image{} en préparation…", noms.len(), if noms.len() > 1 { "s" } else { "" });
                self.vignettes.depots.push(Travail { handle, noms, cible, epoch: self.doc_epoch });
            }
            Err(e) => self.status = format!("Dépôt impossible : {e}"),
        }
    }

    /// Des fichiers lâchés sur la fenêtre, à `at` (point d'écran) : si la planche est là, ce sont ses images. Vrai si pris.
    pub(crate) fn board_take_drop(&mut self, paths: &[PathBuf], at: Option<Pos2>) -> bool {
        if !self.board_shown() {
            return false;
        }
        let Some(at) = at.filter(|p| self.vignettes.view.contains(*p)) else { return false };
        let images: Vec<PathBuf> = paths.iter().filter(|p| est_une_image(p)).cloned().collect();
        if images.is_empty() {
            return false;
        }
        self.depot_sur(images, at);
        true
    }

    /// Des références lâchées (par leur numéro) à `at` : leurs images (la première image d'une vidéo).
    pub(crate) fn board_take_refs(&mut self, ids: &[u64], at: Pos2) -> bool {
        if !self.board_shown() || !self.vignettes.view.contains(at) {
            return false;
        }
        let images: Vec<PathBuf> = ids.iter().filter_map(|id| self.doc.reference(*id)).filter_map(|r| r.shown_path(0)).collect();
        if images.is_empty() {
            self.status = "Ces références n'ont pas d'image à déposer.".into();
            return true;
        }
        self.depot_sur(images, at);
        true
    }

    fn depot_sur(&mut self, images: Vec<PathBuf>, at: Pos2) {
        let layout = self.board_layout();
        let xf = self.vignettes.xf;
        let tol = 14.0 / xf.zoom.max(1e-3);
        match cible_en(&layout, xf.to_board(at), tol) {
            Some(c) => self.depot_lancer(images, c),
            None => self.status = "Dépose les images sur une case, entre deux cases ou sur une case vide.".into(),
        }
    }

    /// Chaque image d'écran : les dépôts prêts sont ajoutés au document (une étape d'annulation chacun).
    pub(crate) fn depot_tick(&mut self, ctx: &egui::Context) {
        if self.vignettes.depots.is_empty() {
            return;
        }
        if self.vignettes.depots.iter().any(|t| !t.handle.is_finished()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        let Some(k) = self.vignettes.depots.iter().position(|t| t.handle.is_finished()) else { return };
        if self.modal_open() {
            return;
        }
        let t = self.vignettes.depots.remove(k);
        if t.epoch != self.doc_epoch {
            return;
        }
        let rasters = t.handle.join().unwrap_or_else(|_| vec![Err("le travail s'est arrêté".into())]);
        let mut ok = Vec::new();
        let mut echecs = Vec::new();
        for (nom, r) in t.noms.iter().zip(rasters) {
            match r {
                Ok(r) => ok.push(r),
                Err(e) => echecs.push(format!("{nom} : {e}")),
            }
        }
        if !ok.is_empty() {
            self.depot_appliquer(t.cible, ok);
        }
        if let Some(e) = echecs.first() {
            self.status = format!("Image non déposée ({e})");
        }
    }

    /// Ajoute les images au document : une étape d'annulation, « Déposer des images ».
    pub(crate) fn depot_appliquer(&mut self, cible: Cible, rasters: Vec<Raster>) {
        let layout = self.board_layout();
        let n = rasters.len();
        // Les cases visées, dans l'ordre : (indice de la vignette, ou rien pour un plan à faire).
        let k0 = match cible {
            Cible::Case(id) => layout.cards.iter().position(|c| c.id == id),
            _ => None,
        };
        let existantes = k0.map_or(0, |k| (layout.cards.len() - k).min(n));
        let touches: Vec<u64> = k0.map(|k| layout.cards[k..k + existantes].iter().map(|c| c.plan).collect()).unwrap_or_default();
        let nouveaux = n - existantes;
        if nouveaux == 0 {
            if !self.partage_peut_toucher(&touches) {
                return;
            }
            if let Some(c) = k0.map(|k| layout.cards[k]) {
                self.select_board(c.panel, c.frame);
            }
        } else if self.storyboard_refuses() {
            return;
        }
        // Entre deux images d'un plan qui a des versions rangées : couper le plan pose d'abord la question « les versions vont avec quelle
        // partie ? » (`coupeversions.rs`) ; les images attendent la réponse, rien n'est fait en attendant.
        if let Cible::Entre(id) = cible {
            if let Some(c) = layout.cards.iter().find(|c| c.id == id).copied() {
                if self.coupe_versions_necessaire(c.plan, c.end) {
                    self.coupe_versions_ouvrir(c.plan, c.end, crate::coupeversions::Refaire::Depot { cible, rasters });
                    return;
                }
            }
        }
        let joins = self.vignettes.empty_joins;
        let mut rasters = rasters.into_iter();
        let mut first: Option<u64> = None;
        let porte = nouveaux > 0;
        let ok = self.avec_etape("Déposer des images", |s| {
            s.structure_op_porte(porte, |d, _| {
                fn cels(r: Raster, d: &mut Document) -> CelId {
                    d.add_cel(Cel::bitmap(r))
                }
                // 1. Les cases qui existent.
                if let Some(k) = k0 {
                    for c in layout.cards[k..k + existantes].iter() {
                        let Some(r) = rasters.next() else { break };
                        let cel = cels(r, d);
                        let Some(pi) = d.panels.iter().position(|p| p.id == c.plan) else { continue };
                        poser_image(d, pi, c.frame, c.end, cel);
                        first.get_or_insert(c.id);
                    }
                }
                // 2. Des plans neufs : à la fin, entre deux cases, ou dans les cases vides.
                let reste: Vec<Raster> = rasters.by_ref().collect();
                let mut reste = reste.into_iter();
                let mut last = match cible {
                    Cible::Entre(id) => {
                        let c = layout.cards.iter().find(|c| c.id == id).copied();
                        c.and_then(|c| {
                            let pi = d.panels.iter().position(|p| p.id == c.plan)?;
                            // Entre deux images d'un même plan : on coupe d'abord (la vignette de droite commence un plan neuf).
                            if c.end < d.panels[pi].frame_count() {
                                // (`split_plan` rend le plan de droite ; on garde le plan de gauche, après lequel on insère.)
                                d.split_plan(pi, c.end)?;
                            }
                            Some(pi)
                        })
                    }
                    _ => None,
                }
                .or_else(|| match cible {
                    // Une case vide : à la fin de sa séquence, qui est la dernière du film quand la planche n'en a qu'une.
                    Cible::Vide(e) => layout.groupes.iter().find(|g| g.vides.contains(&e)).map(|g| g.dernier_plan.min(d.panels.len() - 1)),
                    _ => None,
                })
                .unwrap_or(d.panels.len() - 1);
                let mut vides = match cible {
                    // (Les cases vides qui précèdent celle-ci dans sa séquence deviennent des plans vides.)
                    Cible::Vide(e) => layout.groupes.iter().find(|g| g.vides.contains(&e)).map_or(e, |g| e - g.vides.start),
                    _ => 0,
                };
                if let (Cible::Vide(0), true) = (cible, joins) {
                    // La barre grise devant la première case vide : une image de plus du dernier plan.
                    if let Some(r) = reste.next() {
                        let old = d.panels[last].frame_count();
                        let image = board::plan_boards(d, last).last().map_or(1, |b| b.2 - b.1).max(1);
                        d.set_duration(last, old + image);
                        d.toggle_mark(last, old)?;
                        let cel = cels(r, d);
                        poser_image(d, last, old, old + image, cel);
                        if let Some(m) = d.panels[last].marks.iter().find(|m| m.frame == old) {
                            first.get_or_insert(m.id);
                        }
                    }
                }
                while vides > 0 && matches!(cible, Cible::Vide(_)) && reste.len() > 0 {
                    last = d.add_scene_after(last);
                    vides -= 1;
                }
                for r in reste {
                    last = d.add_scene_after(last);
                    let n = d.panels[last].frame_count();
                    let cel = cels(r, d);
                    poser_image(d, last, 0, n, cel);
                    first.get_or_insert(d.panels[last].id);
                }
                let at = first.and_then(|f| d.panels.iter().position(|p| p.id == f || p.marks.iter().any(|m| m.id == f)));
                Some(at.unwrap_or(last))
            })
        });
        if !ok {
            return;
        }
        if let Some(id) = first {
            let l = self.board_layout();
            if let Some(c) = l.card_of(id).copied() {
                self.select_board(c.panel, c.frame);
                self.board_sel = vec![c.plan];
            }
        }
        // Le calque « Image » est tout en bas, sous le dessin : le calque courant est le premier qui n'est pas lui.
        let dessin = self.layers().iter().position(|l| !(l.name == NOM_DU_CALQUE && l.kind == LayerKind::Bitmap)).unwrap_or(0);
        self.cur_layer = dessin;
        self.status = format!("{n} image{} déposée{} sur la planche (Ctrl+Z défait tout le dépôt).", if n > 1 { "s" } else { "" }, if n > 1 { "s" } else { "" });
    }

    // ------------------------------------------------------------------ repère

    /// Le repère du dépôt à `pos` (point d'écran) : la case visée (et les suivantes), la barre, ou les cases vides ; `n` images.
    pub(crate) fn depot_repere(&self, painter: &egui::Painter, xf: Xf, layout: &BoardLayout, pos: Pos2, n: usize) {
        let accent = crate::theme::accent();
        let cases = crate::annotations::cases(layout);
        let tol = 14.0 / xf.zoom.max(1e-3);
        let Some(cible) = cible_en(layout, xf.to_board(pos), tol) else {
            painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, "Dépose sur une case ou une case vide", FontId::proportional(12.0), crate::theme::texte_doux());
            return;
        };
        let cadre = |r: Rect| {
            painter.rect_filled(xf.rect(r), 0.0, accent.gamma_multiply(0.18));
            painter.rect_stroke(xf.rect(r), 0.0, Stroke::new(2.0_f32, accent), StrokeKind::Outside);
        };
        let texte: String;
        match cible {
            Cible::Case(id) => {
                let k = layout.cards.iter().position(|c| c.id == id).unwrap_or(0);
                for r in cases.iter().skip(k).take(n.max(1)) {
                    cadre(*r);
                }
                texte = if n > 1 { format!("{n} images à la suite") } else { "Dans cette case, sous le dessin".into() };
            }
            Cible::Vide(e) => {
                let k = layout.cards.len() + e;
                for r in cases.iter().skip(k).take(n.max(1)) {
                    cadre(*r);
                }
                texte = if n > 1 { format!("{n} nouveaux plans") } else { "Nouveau plan".into() };
            }
            Cible::Entre(id) => {
                if let Some(b) = layout.cards.iter().position(|c| c.id == id).and_then(|k| layout.bars.get(k)) {
                    let r = xf.rect(b.zone);
                    painter.rect_filled(Rect::from_center_size(r.center(), vec2(4.0, r.height())), 2.0, accent);
                }
                texte = format!("Insérer {n} plan{} ici", if n > 1 { "s" } else { "" });
            }
        }
        painter.text(pos + vec2(14.0, 14.0), Align2::LEFT_TOP, texte, FontId::proportional(12.0), Color32::WHITE);
    }

    /// Chaque image d'écran, planche à l'écran : le repère d'un dépôt qui vient (fichiers au-dessus de la fenêtre, ou références glissées),
    /// et le lâcher des références. `view`: la zone de la planche.
    pub(crate) fn depot_survol(&mut self, ctx: &egui::Context, painter: &egui::Painter, xf: Xf, layout: &BoardLayout, over: Option<Pos2>) {
        let fichiers = ctx.input(|i| i.raw.hovered_files.iter().filter(|f| f.path.as_deref().is_none_or(est_une_image)).count());
        if fichiers > 0 {
            if let Some(p) = curseur_fenetre(ctx).filter(|p| self.vignettes.view.contains(*p)) {
                self.depot_repere(painter, xf, layout, p, fichiers);
            }
        }
        let Some(payload) = egui::DragAndDrop::payload::<RefsDrag>(ctx) else { return };
        let Some(p) = over else { return };
        self.depot_repere(painter, xf, layout, p, payload.ids.len());
        if ctx.input(|i| i.pointer.any_released()) {
            if let Some(payload) = egui::DragAndDrop::take_payload::<RefsDrag>(ctx) {
                self.board_take_refs(&payload.ids, p);
            }
        }
    }
}

/// Le curseur de l'écran, en points de la fenêtre : un fichier qu'on lâche depuis l'Explorateur n'indique pas où (winit ne donne
/// pas la position d'un dépôt sous Windows), mais le curseur est alors à l'endroit du lâcher.
pub(crate) fn curseur_fenetre(ctx: &egui::Context) -> Option<Pos2> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Point {
            x: i32,
            y: i32,
        }
        #[link(name = "user32")]
        extern "system" {
            fn GetCursorPos(p: *mut Point) -> i32;
        }
        let mut p = Point { x: 0, y: 0 };
        // SAFETY: `GetCursorPos` remplit le `POINT` qu'on lui donne ; rien d'autre n'est touché.
        let ok = unsafe { GetCursorPos(&mut p) } != 0;
        // (Les pixels par point **de l'interface**, taille choisie comprise : `inner_rect` est en points de cette taille, c'est
        // `egui-winit::pixels_per_point`. Avec ceux de l'écran seuls, un dépôt tombait à côté dès que la taille n'était plus 100 %.)
        let (inner, ppp) = ctx.input(|i| (i.viewport().inner_rect, i.pixels_per_point));
        if ok {
            if let Some(inner) = inner {
                return Some(curseur_en_points((p.x, p.y), inner.min.to_vec2(), ppp));
            }
        }
    }
    ctx.input(|i| i.pointer.latest_pos())
}

/// Un point de l'écran (pixels) en points de la fenêtre : le coin du dessin de la fenêtre (`inner_min`, en points) est retiré,
/// `ppp` sont les pixels par point de l'interface.
pub(crate) fn curseur_en_points(ecran_px: (i32, i32), inner_min: egui::Vec2, ppp: f32) -> Pos2 {
    pos2(ecran_px.0 as f32 / ppp, ecran_px.1 as f32 / ppp) - inner_min
}

#[cfg(test)]
#[path = "deposer_tests.rs"]
mod tests;
