//! Repères du plan (13d), comme dans la timeline de TVPaint : signets (basculer, précédent /
//! suivant, enlever tous ou ceux des images choisies), points d'entrée et de sortie (la lecture
//! du plan reste entre eux, en boucle ou non ; « boucle entre les images choisies »), image de
//! départ du projet (numéro de la première image), affichage des numéros : image, time code ou
//! trame (deux trames par image).

use eframe::egui;

use crate::app::App;
use crate::document::Cues;

/// Comment la timeline numérote les images.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TimeDisplay {
    #[default]
    Image,
    TimeCode,
    Field,
}

impl TimeDisplay {
    pub(crate) const ALL: [TimeDisplay; 3] = [TimeDisplay::Image, TimeDisplay::TimeCode, TimeDisplay::Field];

    pub(crate) fn label(self) -> &'static str {
        match self {
            TimeDisplay::Image => "Image",
            TimeDisplay::TimeCode => "Time code",
            TimeDisplay::Field => "Trame",
        }
    }
}

/// Ce que « Adapter » fait tenir dans la timeline (13d).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fit {
    Plan,
    Layer,
    Selection,
    Image,
    Keys,
    All,
}

/// Images [début, fin) que lit la lecture du plan : entre les points d'entrée et de sortie
/// (tout le plan s'ils manquent ou se croisent).
pub(crate) fn play_range(cues: &Cues, count: usize) -> (usize, usize) {
    let count = count.max(1);
    let a = cues.in_point.unwrap_or(0).min(count - 1);
    let b = cues.out_point.map_or(count, |o| (o + 1).min(count));
    if b <= a {
        (0, count)
    } else {
        (a, b)
    }
}

/// Le numéro affiché de l'image `f` (comptée depuis 0) : image (depuis l'image de départ),
/// time code HH:MM:SS:II, ou trame.
pub(crate) fn frame_label(f: usize, start: usize, fps: u32, how: TimeDisplay) -> String {
    match how {
        TimeDisplay::Image => (f + start).to_string(),
        TimeDisplay::TimeCode => {
            let fps = fps.max(1) as usize;
            let n = (f + start).saturating_sub(1);
            let (s, i) = (n / fps, n % fps);
            format!("{:02}:{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60, i)
        }
        TimeDisplay::Field => ((f + start).saturating_sub(1) * 2 + 1).to_string(),
    }
}

/// Lit un numéro tapé (image, time code « HH:MM:SS:II » ou trame) ; rend l'image comptée
/// depuis 0.
pub(crate) fn parse_frame(text: &str, start: usize, fps: u32, how: TimeDisplay) -> Option<usize> {
    let text = text.trim();
    if text.contains(':') {
        let parts: Vec<usize> = text.split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
        let fps = fps.max(1) as usize;
        let n = parts.iter().rev().enumerate().fold(0usize, |acc, (k, v)| {
            acc + match k {
                0 => *v,
                1 => v * fps,
                2 => v * fps * 60,
                _ => v * fps * 3600,
            }
        });
        return (n + 1).checked_sub(start);
    }
    let v: usize = text.parse().ok()?;
    match how {
        TimeDisplay::Field => (v.saturating_sub(1) / 2 + 1).checked_sub(start),
        _ => v.checked_sub(start),
    }
}

/// Signet à l'image `f` : posé, ou retiré s'il y en avait un ; renvoie vrai s'il est posé.
pub(crate) fn toggle_bookmark(cues: &mut Cues, f: usize) -> bool {
    match cues.bookmarks.binary_search(&f) {
        Ok(k) => {
            cues.bookmarks.remove(k);
            false
        }
        Err(k) => {
            cues.bookmarks.insert(k, f);
            true
        }
    }
}

impl App {
    /// Images que lit la lecture du plan courant.
    pub(crate) fn plan_play_range(&self) -> (usize, usize) {
        play_range(&self.panel().cues, self.frame_count())
    }

    /// Le numéro affiché de l'image `f` du plan.
    pub(crate) fn frame_label(&self, f: usize) -> String {
        frame_label(f, self.doc.start_frame, self.doc.fps, self.time_display)
    }

    /// Change les repères du plan courant (une étape d'annulation).
    fn set_cues(&mut self, change: impl FnOnce(&mut Cues), status: String) {
        self.record_step();
        change(&mut self.panel_mut().cues);
        self.status = status;
    }

    /// Signet à l'image courante : posé ou retiré.
    pub(crate) fn toggle_bookmark_here(&mut self) {
        let f = self.cur_frame;
        let label = self.frame_label(f);
        self.record_step();
        let on = toggle_bookmark(&mut self.panel_mut().cues, f);
        self.status = format!("Signet {} à l'image {label}.", if on { "posé" } else { "retiré" });
    }

    /// Va au signet suivant (`next`) ou précédent.
    pub(crate) fn goto_bookmark(&mut self, next: bool) {
        let f = self.cur_frame;
        let marks = &self.panel().cues.bookmarks;
        let to = if next { marks.iter().copied().find(|&b| b > f) } else { marks.iter().rev().copied().find(|&b| b < f) };
        match to {
            Some(b) => self.goto_frame(b),
            None => self.status = if next { "Pas de signet après." } else { "Pas de signet avant." }.into(),
        }
    }

    /// Enlève tous les signets du plan.
    pub(crate) fn clear_bookmarks(&mut self) {
        let n = self.panel().cues.bookmarks.len();
        self.set_cues(|c| c.bookmarks.clear(), format!("{n} signet{} enlevé{}.", s(n), s(n)));
    }

    /// Enlève les signets des images choisies.
    pub(crate) fn clear_chosen_bookmarks(&mut self) {
        let (a, b) = self.image_range();
        let n = self.panel().cues.bookmarks.iter().filter(|&&f| f >= a && f < b).count();
        self.set_cues(move |c| c.bookmarks.retain(|&f| f < a || f >= b), format!("{n} signet{} enlevé{}.", s(n), s(n)));
    }

    /// Point d'entrée (`out` : de sortie) à l'image courante ; `toggle` : retiré s'il y était
    /// déjà. Refusé s'il tomberait sur l'autre point.
    pub(crate) fn set_cue_point(&mut self, out: bool, toggle: bool) {
        let f = self.cur_frame;
        let cues = &self.panel().cues;
        let (mine, other) = if out { (cues.out_point, cues.in_point) } else { (cues.in_point, cues.out_point) };
        let what = if out { "sortie" } else { "entrée" };
        let label = self.frame_label(f);
        if toggle && mine == Some(f) {
            self.set_cues(move |c| if out { c.out_point = None } else { c.in_point = None }, format!("Point de {what} retiré."));
            return;
        }
        if other == Some(f) {
            self.status = "Point d'entrée = point de sortie : les deux seraient à la même image.".into();
            return;
        }
        self.set_cues(
            move |c| if out { c.out_point = Some(f) } else { c.in_point = Some(f) },
            format!("Point de {what} à l'image {label}."),
        );
    }

    /// Enlève les points d'entrée et de sortie.
    pub(crate) fn clear_cue_points(&mut self) {
        self.set_cues(
            |c| {
                c.in_point = None;
                c.out_point = None;
            },
            "Points d'entrée et de sortie enlevés : la lecture prend tout le plan.".into(),
        );
    }

    /// Les images choisies deviennent les limites de la lecture (points d'entrée et de sortie).
    pub(crate) fn chosen_to_loop_limits(&mut self) {
        let Some((a, b)) = self.frame_sel.filter(|(a, b)| b - a > 1) else {
            self.status = "Choisis d'abord des images (Maj + clic dans la timeline).".into();
            return;
        };
        self.set_cues(
            move |c| {
                c.in_point = Some(a);
                c.out_point = Some(b - 1);
            },
            "La lecture est limitée aux images choisies.".into(),
        );
    }

    /// Boucle entre les images choisies : allumée (limites posées, lecture en boucle) ou
    /// éteinte (limites enlevées).
    pub(crate) fn toggle_loop_on_chosen(&mut self) {
        let cues = &self.panel().cues;
        let on = self.looping && self.frame_sel.is_some_and(|(a, b)| cues.in_point == Some(a) && cues.out_point == Some(b - 1));
        if on {
            self.clear_cue_points();
            self.status = "Boucle entre les images choisies éteinte.".into();
        } else {
            self.chosen_to_loop_limits();
            if self.frame_sel.is_some() {
                self.looping = true;
                self.status = "Lecture en boucle entre les images choisies.".into();
            }
        }
    }

    /// « Adapter » (13d) : le zoom de la timeline fait tenir ces images dans la largeur visible.
    pub(crate) fn fit_timeline(&mut self, what: Fit) {
        let n = self.frame_count();
        let range = match what {
            Fit::Plan => Some((0, n)),
            Fit::Layer => {
                let f = &self.layer().frames;
                f.iter().position(Option::is_some).zip(f.iter().rposition(Option::is_some)).map(|(a, b)| (a, b + 1))
            }
            Fit::Selection => Some(self.image_range()),
            Fit::Image => Some((self.cur_frame, self.cur_frame + 1)),
            Fit::Keys => {
                let keys = &self.panel().camera;
                keys.first().zip(keys.last()).map(|(a, b)| (a.frame, b.frame + 1)).filter(|(a, b)| b > a)
            }
            Fit::All => {
                let longest = self.layers().iter().map(|l| l.frames.len()).max().unwrap_or(0);
                let cues = &self.panel().cues;
                let last_cue = cues.bookmarks.iter().chain(cues.out_point.iter()).max().map_or(0, |f| f + 1);
                Some((0, n.max(longest).max(last_cue)))
            }
        };
        match range {
            Some(r) => self.timeline_fit = Some(r),
            None => self.status = "Rien à adapter.".into(),
        }
    }

    /// Menu d'affichage de la timeline : zoom, « Adapter », croix des instances vides,
    /// miniatures.
    pub(crate) fn timeline_view_menu(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Zoom");
            ui.add(
                egui::Slider::new(&mut self.timeline_cell, crate::timeline::CELL_MIN..=crate::timeline::CELL_MAX)
                    .logarithmic(true)
                    .suffix(" px par image"),
            );
        });
        ui.label(egui::RichText::new("Ctrl + molette sur la grille : zoomer.").weak().small());
        ui.separator();
        ui.label(egui::RichText::new("Adapter").strong());
        self.action_item(ui, "Le plan", "timeline.adapter_plan");
        self.action_item(ui, "Le calque courant", "timeline.adapter_calque");
        self.action_item(ui, "Les images choisies", "timeline.adapter_choix");
        self.action_item(ui, "L'image courante", "timeline.adapter_image");
        self.action_item(ui, "Les clés de caméra", "timeline.adapter_cles");
        self.action_item(ui, "Tout voir", "timeline.tout_voir");
        ui.separator();
        ui.checkbox(&mut self.empty_x, "Croix sur les instances vides");
        ui.checkbox(&mut self.timeline_thumbs, "Miniatures des dessins (lignes complètes)");
    }

    /// Menu « Repères » (barre de la timeline).
    pub(crate) fn cues_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Signets").strong());
        self.action_item(ui, "Poser / retirer un signet", "signet.basculer");
        self.action_item(ui, "Signet précédent", "signet.precedent");
        self.action_item(ui, "Signet suivant", "signet.suivant");
        self.action_item(ui, "Enlever les signets des images choisies", "signets.enlever_choisis");
        self.action_item(ui, "Enlever tous les signets", "signets.tout_enlever");
        ui.separator();
        ui.label(egui::RichText::new("Entrée / sortie (lecture du plan)").strong());
        self.action_item(ui, "Point d'entrée à l'image courante", "entree.poser");
        self.action_item(ui, "Poser / retirer le point d'entrée", "entree.basculer");
        self.action_item(ui, "Point de sortie à l'image courante", "sortie.poser");
        self.action_item(ui, "Poser / retirer le point de sortie", "sortie.basculer");
        self.action_item(ui, "Les images choisies deviennent les limites", "boucle.limites_choisies");
        self.action_item(ui, "Boucle entre les images choisies (allumer / éteindre)", "boucle.images_choisies");
        self.action_item(ui, "Enlever les points d'entrée et de sortie", "entree_sortie.enlever");
        ui.separator();
        ui.label(egui::RichText::new("Numéros").strong());
        for t in TimeDisplay::ALL {
            if ui.radio(self.time_display == t, t.label()).clicked() {
                self.time_display = t;
                ui.close_menu();
            }
        }
        ui.horizontal(|ui| {
            ui.label("Image de départ du projet");
            let mut start = self.doc.start_frame;
            let resp = ui.add(egui::DragValue::new(&mut start).range(0..=100_000));
            if self.edited(&resp) {
                self.doc.start_frame = start;
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
