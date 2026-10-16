//! Poignées de la timeline du plan (passe interface, demandée par Anthony le 2026-10-16) : on y
//! prend à la souris et on fait glisser dans le temps les marques d'image, les signets, les
//! points d'entrée et de sortie et les clés d'effets ; le bord droit d'un dessin étire ou
//! raccourcit son exposition (ce qui suit glisse d'autant, comme dans TVPaint). Une étape
//! d'annulation par geste, seulement si quelque chose a bougé ; Échap remet tout comme avant.
//! (Les clés de caméra se prenaient déjà : `camtools`.)

use eframe::egui;

use crate::app::App;
use crate::document::{ImageMark, Layer, Panel};
use crate::fx::FxKey;
use crate::history::Edit;

/// Ce que l'on tient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handle {
    /// Marque d'image du plan (son rang dans `Panel::marks`).
    Mark(usize),
    /// Signet (son rang dans `Cues::bookmarks`).
    Bookmark(usize),
    In,
    Out,
    /// Clé d'effet : calque, effet, rang de la clé.
    FxKey { layer: usize, effect: usize, key: usize },
    /// Bord droit du dessin exposé de `start` à `end` (exclu) sur le calque `layer`.
    Exposure { layer: usize, start: usize, end: usize },
}

/// Où l'on a cliqué droit dans la timeline du plan (en dehors des cases des calques, qui ont
/// leur menu des rôles et des images).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GridMenu {
    /// La règle (l'image courante vient sous le clic).
    Ruler,
    /// La ligne de la caméra, ou son nom.
    Camera,
    /// Une clé d'effet (le calque et l'effet deviennent courants).
    FxKey,
    /// La ligne du son : la piste et l'extrait sous le clic, l'image du montage.
    Sound { track: usize, clip: Option<u64>, frame: usize },
    /// Le nom de la ligne du son.
    SoundHeader,
    /// Le nom de la ligne des notes.
    NotesHeader,
}

/// Une poignée tenue : le plan (id), le plan d'avant (pour Échap et pour savoir si quelque chose
/// a bougé) et l'étape d'annulation de tout le geste.
pub(crate) struct HandleDrag {
    pub handle: Handle,
    panel: u64,
    before: Panel,
    snap: Edit,
}

/// `f` gardé entre les voisins `prev` et `next` (exclus) : l'ordre ne change pas.
pub fn between(f: usize, prev: Option<usize>, next: Option<usize>) -> usize {
    let lo = prev.map_or(0, |p| p + 1);
    let hi = next.map_or(usize::MAX, |n| n.saturating_sub(1));
    f.clamp(lo, hi.max(lo))
}

/// La marque `i` amenée vers l'image `frame` (entre ses voisines, jamais sur la première image
/// du plan, avant la fin `len`). Renvoie où elle est.
pub fn move_mark(marks: &mut [ImageMark], i: usize, frame: usize, len: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| marks[p].frame).or(Some(0));
    let next = marks.get(i + 1).map(|m| m.frame).or(Some(len.max(2)));
    let f = between(frame, prev, next);
    marks[i].frame = f;
    f
}

/// Le signet `i` (signets triés) amené vers l'image `frame`, entre ses voisins.
pub fn move_bookmark(bookmarks: &mut [usize], i: usize, frame: usize, len: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| bookmarks[p]);
    let next = bookmarks.get(i + 1).copied().or(Some(len.max(1)));
    let f = between(frame, prev, next);
    bookmarks[i] = f;
    f
}

/// La clé `i` d'un effet (clés triées) amenée vers l'image `frame`, entre ses voisines.
pub fn move_fx_key(keys: &mut [FxKey], i: usize, frame: usize) -> usize {
    let prev = i.checked_sub(1).map(|p| keys[p].frame);
    let next = keys.get(i + 1).map(|k| k.frame);
    let f = between(frame, prev, next);
    keys[i].frame = f;
    f
}

/// Le dessin exposé de `start` à `end` (exclu) dure maintenant `len` images (au moins une) ;
/// les images d'après, leurs rôles et les clés d'effets glissent d'autant.
pub fn set_run_length(layer: &mut Layer, start: usize, end: usize, len: usize) {
    let len = len.max(1);
    let old = end.saturating_sub(start);
    if start >= layer.frames.len() || old == 0 {
        return;
    }
    if len > old {
        let cel = layer.frames[start];
        layer.insert(end.min(layer.frames.len()), cel, len - old);
    } else if len < old {
        let a = start + len;
        let b = end.min(layer.frames.len());
        layer.frames.drain(a..b);
        layer.shift_frames(a, b - a, true);
    }
    layer.trim();
}

impl App {
    /// Prend une poignée du plan courant (refusé sous le verrou du storyboard, sauf les signets
    /// et les points d'entrée et de sortie).
    pub(crate) fn begin_handle_drag(&mut self, handle: Handle) -> bool {
        let refused = match handle {
            Handle::Mark(_) => self.storyboard_refuses(),
            Handle::FxKey { .. } | Handle::Exposure { .. } => self.content_refuses(),
            _ => false,
        };
        if refused {
            return false;
        }
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let before = self.panel().clone();
        self.handle_drag = Some(HandleDrag { handle, panel: before.id, before, snap });
        true
    }

    /// La poignée tenue suit la souris : `frame`, l'image sous elle ; `edge`, le bord d'image le
    /// plus proche (pour une exposition).
    pub(crate) fn drag_handle(&mut self, frame: usize, edge: usize) {
        let Some(d) = &self.handle_drag else { return };
        let (handle, id) = (d.handle, d.panel);
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == id) else {
            self.handle_drag = None;
            return;
        };
        let len = self.doc.panels[pi].frame_count();
        let label = |app: &App, f: usize| app.frame_label(f);
        let status = match handle {
            Handle::Mark(i) => {
                let f = move_mark(&mut self.doc.panels[pi].marks, i, frame, len);
                format!("Marque d'image : image {}.", label(self, f))
            }
            Handle::Bookmark(i) => {
                let f = move_bookmark(&mut self.doc.panels[pi].cues.bookmarks, i, frame, len);
                format!("Signet : image {}.", label(self, f))
            }
            // L'entrée reste avant (ou sur) la sortie, et inversement ; la sortie est la dernière
            // image jouée.
            Handle::In => {
                let cues = &mut self.doc.panels[pi].cues;
                let f = frame.min(cues.out_point.unwrap_or(len.saturating_sub(1)));
                cues.in_point = Some(f);
                format!("Point d'entrée : image {}.", label(self, f))
            }
            Handle::Out => {
                let cues = &mut self.doc.panels[pi].cues;
                let f = frame.min(len.saturating_sub(1)).max(cues.in_point.unwrap_or(0));
                cues.out_point = Some(f);
                format!("Point de sortie : image {}.", label(self, f))
            }
            Handle::FxKey { layer, effect, key } => {
                let Some(e) = self.doc.panels[pi].layers.get_mut(layer).and_then(|l| l.fx.get_mut(effect)) else { return };
                let f = move_fx_key(&mut e.keys, key, frame);
                self.renderer.invalidate_all();
                format!("Clé d'effet : image {}.", label(self, f))
            }
            Handle::Exposure { layer, start, end } => {
                let Some(original) = self.handle_drag.as_ref().and_then(|d| d.before.layers.get(layer)).cloned() else { return };
                let mut l = original;
                set_run_length(&mut l, start, end, edge.saturating_sub(start));
                self.doc.panels[pi].layers[layer] = l;
                self.renderer.invalidate_all();
                let n = edge.saturating_sub(start).max(1);
                format!("Exposition : {n} image{}.", if n > 1 { "s" } else { "" })
            }
        };
        self.status = status;
    }

    /// Lâche la poignée : une étape d'annulation si quelque chose a bougé.
    pub(crate) fn end_handle_drag(&mut self) {
        let Some(d) = self.handle_drag.take() else { return };
        let Some(p) = self.doc.panels.iter().find(|p| p.id == d.panel) else { return };
        let changed = match d.handle {
            Handle::Mark(_) => p.marks != d.before.marks,
            Handle::Bookmark(_) | Handle::In | Handle::Out => p.cues != d.before.cues,
            Handle::FxKey { layer, .. } | Handle::Exposure { layer, .. } => {
                p.layers.get(layer).map(|l| (&l.frames, &l.fx, &l.roles)) != d.before.layers.get(layer).map(|l| (&l.frames, &l.fx, &l.roles))
            }
        };
        if changed {
            self.history.push(d.snap);
            self.modified = true;
        }
    }

    /// Le menu d'un clic droit dans la timeline du plan (hors cases des calques). Chaque entrée
    /// est une commande : un clic droit dessus lui donne un raccourci.
    pub(crate) fn grid_menu_ui(&mut self, ui: &mut egui::Ui, m: GridMenu) {
        match m {
            GridMenu::Ruler => {
                self.action_item(ui, "Signet ici (poser / retirer)", "signet.basculer");
                self.action_item(ui, "Marque d'image ici (poser / retirer)", "marque.basculer");
                ui.separator();
                self.action_item(ui, "Point d'entrée ici", "entree.poser");
                self.action_item(ui, "Point de sortie ici", "sortie.poser");
                self.action_item(ui, "Enlever l'entrée et la sortie", "entree_sortie.enlever");
                ui.separator();
                self.action_item(ui, "Adapter la timeline au plan", "timeline.adapter_plan");
                self.action_item(ui, "Timeline : tout voir", "timeline.tout_voir");
            }
            GridMenu::Camera => {
                self.action_item(ui, "Image clé de caméra ici", "camera.cle_ici");
                self.action_item(ui, "Retirer la clé de caméra d'ici", "camera.cle_supprimer");
                self.action_item(ui, "Adoucir le mouvement à cette clé", "camera.cle_adoucir");
                ui.separator();
                self.action_item(ui, "Clé au début du plan", "camera.cle_debut");
                self.action_item(ui, "Clé à la fin du plan", "camera.cle_fin");
                self.action_item(ui, "Adapter la timeline aux clés", "timeline.adapter_cles");
                ui.separator();
                self.action_item(ui, "Vue à travers la caméra", "camera.vue");
                self.action_item(ui, "Réinitialiser la caméra", "camera.reinitialiser");
            }
            GridMenu::FxKey => {
                self.action_item(ui, "Supprimer cette clé d'effet", "fx.cle_supprimer");
                self.action_item(ui, "Pile d'effets du calque…", "fx.panneau");
                self.action_item(ui, "Clés d'effets dans la timeline (montrer / cacher)", "fx.cles_timeline");
            }
            GridMenu::Sound { track, clip, frame } => self.lane_menu(ui, track, clip, frame),
            GridMenu::SoundHeader => {
                self.action_item(ui, "Importer un son…", "son.importer");
                self.action_item(ui, "Nouvelle piste son", "son.nouvelle_piste");
                self.action_item(ui, "Couper le son à l'image courante", "son.couper");
                ui.separator();
                self.action_item(ui, "Son au défilement (activer / couper)", "son.defilement");
                self.action_item(ui, "Son pendant la lecture (activer / couper)", "son.lecture");
                self.action_item(ui, "Onglet Son (pistes, volumes)", "onglet.son");
            }
            GridMenu::NotesHeader => {
                self.action_item(ui, "Onglet Notes (crayon, gomme, couleurs)", "onglet.notes");
            }
        }
    }

    /// Abandonne la poignée : le plan revient comme avant.
    pub(crate) fn cancel_handle_drag(&mut self) {
        let Some(d) = self.handle_drag.take() else { return };
        if let Some(p) = self.doc.panels.iter_mut().find(|p| p.id == d.panel) {
            *p = d.before;
            self.renderer.invalidate_all();
        }
    }
}
