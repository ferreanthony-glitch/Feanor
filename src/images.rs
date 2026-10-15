//! Images de la timeline (13c-2) : images choisies (Maj + clic / glisser dans la grille, sur
//! les calques choisis), supprimer avec un mode de trou, insérer, couper / copier / coller en
//! insérant ou en remplaçant, dupliquer, inverser, tout choisir. Les commandes « du plan »
//! (I, Maj+I, Maj+F5 : tes touches TVPaint) touchent tous les calques, la durée du plan, ses
//! clés de caméra et ses marques.
//!
//! Choix d'Anthony (2026-10-15) pour le trou laissé par des images supprimées : laisser vide,
//! combler depuis la gauche (le dessin d'avant est tenu), combler depuis la droite (la suite
//! recule).

use std::collections::HashMap;

use eframe::egui;

use crate::app::{App, Mode};
use crate::document::{Cel, CelId, Panel};

/// Ce que devient la place d'images supprimées.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Hole {
    /// La suite du calque recule (comme « Supprimer le dessin » jusqu'ici).
    #[default]
    FromRight,
    /// Les images restent, vides.
    Empty,
    /// Le dessin d'avant le trou y est tenu.
    FromLeft,
}

impl Hole {
    pub(crate) const ALL: [Hole; 3] = [Hole::Empty, Hole::FromLeft, Hole::FromRight];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Hole::Empty => "Laisser vide",
            Hole::FromLeft => "Combler depuis la gauche (le dessin d'avant est tenu)",
            Hole::FromRight => "Combler depuis la droite (la suite recule)",
        }
    }
}

type Frames = Vec<Option<CelId>>;

fn trim(frames: &mut Frames) {
    while frames.last() == Some(&None) {
        frames.pop();
    }
}

/// Les images [a, b) d'un calque (vides au-delà de sa fin).
pub(crate) fn frames_in(frames: &Frames, a: usize, b: usize) -> Frames {
    (a..b).map(|f| frames.get(f).copied().flatten()).collect()
}

/// Supprime les images [a, b) ; le trou selon `hole`.
pub(crate) fn delete_frames(frames: &mut Frames, a: usize, b: usize, hole: Hole) {
    let b = b.min(frames.len());
    if a >= b {
        return;
    }
    match hole {
        Hole::FromRight => {
            frames.drain(a..b);
        }
        Hole::Empty => frames[a..b].fill(None),
        Hole::FromLeft => {
            let before = a.checked_sub(1).and_then(|i| frames[i]);
            frames[a..b].fill(before);
        }
    }
    trim(frames);
}

/// Insère `row` à l'image `at` ; la suite du calque avance.
pub(crate) fn insert_frames(frames: &mut Frames, at: usize, row: &[Option<CelId>]) {
    if at > frames.len() {
        if row.iter().all(Option::is_none) {
            return;
        }
        frames.resize(at, None);
    }
    frames.splice(at..at, row.iter().copied());
    trim(frames);
}

/// Inverse l'ordre des images [a, b).
pub(crate) fn reverse_frames(frames: &mut Frames, a: usize, b: usize) {
    if b > frames.len() {
        frames.resize(b, None);
    }
    frames[a..b].reverse();
    trim(frames);
}

/// Décale les clés de caméra et les marques d'image du plan quand `n` images sont insérées à
/// `at` (ou, `removed`, quand les images [at, at + n) sont supprimées : celles qui y étaient
/// partent).
pub(crate) fn shift_panel(panel: &mut Panel, at: usize, n: usize, removed: bool) {
    if removed {
        panel.camera.retain(|k| k.frame < at || k.frame >= at + n);
        panel.marks.retain(|m| m.frame < at || m.frame >= at + n);
        for k in panel.camera.iter_mut().filter(|k| k.frame >= at + n) {
            k.frame -= n;
        }
        for m in panel.marks.iter_mut().filter(|m| m.frame >= at + n) {
            m.frame -= n;
        }
    } else {
        for k in panel.camera.iter_mut().filter(|k| k.frame >= at) {
            k.frame += n;
        }
        for m in panel.marks.iter_mut().filter(|m| m.frame >= at) {
            m.frame += n;
        }
    }
    // Une marque ne peut pas être sur la première image (c'est le début du plan).
    panel.marks.retain(|m| m.frame > 0);
}

/// Images copiées : une rangée par calque (du plus bas au plus haut) et leurs dessins.
#[derive(Clone)]
pub(crate) struct ImageClip {
    pub rows: Vec<Frames>,
    pub cels: HashMap<CelId, Cel>,
}

impl App {
    /// Images visées : les images choisies, sinon l'image courante.
    pub(crate) fn image_range(&self) -> (usize, usize) {
        self.frame_sel.filter(|(a, b)| a < b).unwrap_or((self.cur_frame, self.cur_frame + 1))
    }

    /// Calques visés : les calques choisis, sans les dossiers.
    fn image_layers(&self) -> Vec<usize> {
        self.chosen_layers().into_iter().filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Tous les calques du plan, sans les dossiers.
    fn all_image_layers(&self) -> Vec<usize> {
        (0..self.layers().len()).filter(|&li| !self.layers()[li].folder).collect()
    }

    /// Les images choisies visent la timeline : Ctrl+C / X / V et Suppr agissent sur elles
    /// après un clic dans la grille (comme pour les plans dans le montage, 12e).
    pub(crate) fn images_have_focus(&self) -> bool {
        self.images_focus && self.mode == Mode::Animation && self.frame_sel.is_some()
    }

    fn done(&mut self, snap: crate::history::Edit, status: String) {
        self.vsel = None;
        self.commit_structure(snap);
        self.status = status;
    }

    /// Supprime les images visées des calques choisis (`all` : de tous les calques du plan),
    /// selon le mode de trou. Sur tout le plan et en reculant la suite, le plan raccourcit et
    /// ses clés de caméra et marques suivent.
    pub(crate) fn delete_images(&mut self, all: bool) {
        if !all && self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let (a, b) = self.image_range();
        let lis = if all { self.all_image_layers() } else { self.image_layers() };
        let hole = self.hole;
        let snap = self.snapshot();
        for li in lis {
            delete_frames(&mut self.layers_mut()[li].frames, a, b, hole);
        }
        let n = b - a;
        if all && hole == Hole::FromRight {
            let len = self.panel().length;
            let panel = self.panel_mut();
            shift_panel(panel, a, n, true);
            panel.length = len.saturating_sub(n).max(1);
            self.cur_frame = a.min(self.panel().length - 1);
        }
        self.frame_sel = None;
        self.done(snap, format!("{n} image{} supprimée{}.", s(n), s(n)));
    }

    /// Insère `n` images vides avant (ou après) les images visées : sur les calques choisis, ou
    /// (`all`) sur tout le plan, qui s'allonge d'autant.
    pub(crate) fn insert_images(&mut self, before: bool, n: usize, all: bool) {
        self.end_stroke();
        let (a, b) = self.image_range();
        let at = if before { a } else { b };
        let lis = if all { self.all_image_layers() } else { self.image_layers() };
        let snap = self.snapshot();
        let row = vec![None; n];
        for li in lis {
            insert_frames(&mut self.layers_mut()[li].frames, at, &row);
        }
        if all {
            let panel = self.panel_mut();
            shift_panel(panel, at, n, false);
            panel.length += n;
        }
        self.cur_frame = at;
        self.follow_frame = true;
        self.frame_sel = None;
        self.done(snap, format!("{n} image{} insérée{}.", s(n), s(n)));
    }

    /// Copie les images visées des calques choisis.
    pub(crate) fn copy_images(&mut self) {
        let (a, b) = self.image_range();
        let rows: Vec<Frames> = self.image_layers().into_iter().map(|li| frames_in(&self.layers()[li].frames, a, b)).collect();
        let cels = rows
            .iter()
            .flatten()
            .flatten()
            .filter_map(|id| self.doc.cels.get(id).map(|c| (*id, c.clone())))
            .collect();
        let (n, k) = (b - a, rows.len());
        self.image_clipboard = Some(ImageClip { rows, cels });
        self.status = format!("{n} image{} de {k} calque{} copiée{}.", s(n), s(k), s(n));
    }

    /// Coupe les images visées : copiées, puis supprimées (selon le mode de trou).
    pub(crate) fn cut_images(&mut self) {
        self.copy_images();
        self.delete_images(false);
    }

    /// Colle les images copiées : avant l'image courante (en insérant), ou (`replace`) à la
    /// place des images choisies. La première rangée va sur le calque courant, les suivantes
    /// sur les calques au-dessus ; chaque collage a ses propres copies des dessins.
    pub(crate) fn paste_images(&mut self, replace: bool) {
        let Some(clip) = self.image_clipboard.clone() else {
            self.status = "Aucune image copiée.".into();
            return;
        };
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let targets: Vec<usize> = (self.cur_layer..self.layers().len()).filter(|&li| !self.layers()[li].folder).take(clip.rows.len()).collect();
        let (a, b) = self.image_range();
        let at = if replace { a } else { self.cur_frame };
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for (row, &li) in clip.rows.iter().zip(&targets) {
            let pasted: Frames = row
                .iter()
                .map(|f| {
                    f.and_then(|old| {
                        if let Some(&c) = copies.get(&old) {
                            return Some(c);
                        }
                        let c = self.doc.add_cel(clip.cels.get(&old)?.clone());
                        copies.insert(old, c);
                        Some(c)
                    })
                })
                .collect();
            let frames = &mut self.layers_mut()[li].frames;
            if replace {
                delete_frames(frames, a, b, Hole::FromRight);
            }
            insert_frames(frames, at, &pasted);
        }
        let n = clip.rows.first().map_or(0, Vec::len);
        self.frame_sel = Some((at, at + n));
        let lost = clip.rows.len().saturating_sub(targets.len());
        let mut status = format!("{n} image{} collée{}.", s(n), s(n));
        if lost > 0 {
            status += &format!(" {lost} rangée{} sans calque au-dessus, laissée{} de côté.", s(lost), s(lost));
        }
        self.done(snap, status);
    }

    /// Duplique les images visées juste après elles.
    pub(crate) fn duplicate_images(&mut self) {
        if self.refuse_on_folder() {
            return;
        }
        self.end_stroke();
        let (a, b) = self.image_range();
        let snap = self.snapshot();
        let mut copies: HashMap<CelId, CelId> = HashMap::new();
        for li in self.image_layers() {
            let row = frames_in(&self.layers()[li].frames, a, b);
            let dup: Frames = row
                .iter()
                .map(|f| {
                    f.map(|old| match copies.get(&old) {
                        Some(&c) => c,
                        None => {
                            let c = self.doc.duplicate_cel(old);
                            copies.insert(old, c);
                            c
                        }
                    })
                })
                .collect();
            insert_frames(&mut self.layers_mut()[li].frames, b, &dup);
        }
        self.frame_sel = Some((b, b + (b - a)));
        self.done(snap, "Images dupliquées après elles-mêmes.".into());
    }

    /// Inverse l'ordre des images visées.
    pub(crate) fn reverse_images(&mut self) {
        let (a, b) = self.image_range();
        if b - a < 2 {
            self.status = "Choisis plusieurs images à inverser (Maj + clic dans la timeline).".into();
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        for li in self.image_layers() {
            reverse_frames(&mut self.layers_mut()[li].frames, a, b);
        }
        self.done(snap, "Ordre des images inversé.".into());
    }

    /// Choisit toutes les images du plan (sur les calques choisis).
    pub(crate) fn select_all_images(&mut self) {
        self.frame_sel = Some((0, self.frame_count()));
        self.images_focus = true;
        self.status = "Toutes les images du plan sont choisies.".into();
    }

    /// Menu « Images » (barre de la timeline, menu Animation).
    pub(crate) fn images_menu(&mut self, ui: &mut egui::Ui) {
        let (a, b) = self.image_range();
        let what = if b - a > 1 { format!("les {} images choisies", b - a) } else { "l'image courante".into() };
        ui.label(egui::RichText::new(format!("Sur {what}, calques choisis :")).weak());
        self.action_item(ui, "Supprimer", "images.supprimer");
        self.action_item(ui, "Couper", "images.couper");
        self.action_item(ui, "Copier", "images.copier");
        self.action_item(ui, "Coller (insérer avant l'image courante)", "images.coller");
        self.action_item(ui, "Coller (remplacer les images choisies)", "images.coller_remplacer");
        self.action_item(ui, "Dupliquer", "images.dupliquer");
        self.action_item(ui, "Inverser l'ordre", "images.inverser");
        self.action_item(ui, "Insérer une image avant", "images.inserer_avant_calques");
        self.action_item(ui, "Insérer une image après", "images.inserer_apres_calques");
        ui.separator();
        ui.label(egui::RichText::new("Sur tout le plan (sa durée change) :").weak());
        self.action_item(ui, "Ajouter une image après l'image courante", "images.ajouter");
        self.action_item(ui, "Insérer une image avant l'image courante", "images.inserer");
        self.action_item(ui, "Supprimer l'image (ou les images choisies)", "images.supprimer_plan");
        ui.separator();
        self.action_item(ui, "Choisir toutes les images", "images.tout_choisir");
        ui.menu_button("Trou laissé par une suppression", |ui| {
            for h in Hole::ALL {
                if ui.radio(self.hole == h, h.label()).clicked() {
                    self.hole = h;
                    ui.close_menu();
                }
            }
        });
    }
}

fn s(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}
