//! Flips dans l'interface (13h) : le panneau « Flips » (choisir, créer, régler, supprimer un
//! flip ; le tenir ; flips par rôle), et le flip en cours. Un flip basique ou directionnel dure
//! tant qu'on tient sa touche (actions `flip.jouer`, `flip.cles`…) ou le bouton du panneau ; un
//! flip manuel avance d'un pas par `flip.precedent` / `flip.suivant`. Voir `flips.rs`.

use eframe::egui::{self, vec2, Align2, RichText, Sense};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::flips::{self, Checkpoints, Flip, FlipKind, FlipScope, Limits};
use crate::roles::Role;

/// Ce qui tient le flip en cours.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hold {
    /// La touche de cette action.
    Key(String),
    /// Le bouton du panneau (le bouton de la souris).
    Button,
}

/// Le flip en cours.
pub(crate) struct FlipRun {
    flip: Flip,
    points: Vec<usize>,
    origin: usize,
    i0: usize,
    index: usize,
    started: f64,
    hold: Hold,
    /// Flip directionnel : pixels de souris pas encore comptés.
    accum: f32,
}

/// Les flips par rôle (TVPaint : « Flip les keyframes »…) : action, rôle, nom court.
const ROLE_FLIPS: [(&str, Role, &str); 6] = [
    ("flip.cles", Role::Key, "Clés"),
    ("flip.breakdowns", Role::Breakdown, "Breakdowns"),
    ("flip.intervalles", Role::Inbetween, "Intervalles à faire"),
    ("flip.intervalles_completes", Role::InbetweenDone, "Intervalles complétés"),
    ("flip.speciaux", Role::Special, "Spéciaux à faire"),
    ("flip.speciaux_completes", Role::SpecialDone, "Spéciaux complétés"),
];

/// Choix d'une couleur de marque (aucune : toutes).
fn mark_color_ui(ui: &mut egui::Ui, id: &str, couleur: &mut Option<u8>) {
    let text = match couleur {
        None => "toutes les couleurs".to_string(),
        Some(k) => format!("couleur {}", *k + 1),
    };
    egui::ComboBox::from_id_salt(id).selected_text(text).show_ui(ui, |ui| {
        ui.selectable_value(couleur, None, "Toutes les couleurs");
        for k in 0..crate::marks::COLORS.len() as u8 {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, crate::marks::color(k));
                ui.selectable_value(couleur, Some(k), format!("Couleur {}", k + 1));
            });
        }
    });
}

fn role_ui(ui: &mut egui::Ui, id: &str, role: &mut Role) {
    egui::ComboBox::from_id_salt(id).selected_text(role.label()).show_ui(ui, |ui| {
        for r in Role::ALL {
            ui.selectable_value(role, r, r.label());
        }
    });
}

fn limits_label(l: &Limits) -> &'static str {
    match l {
        Limits::Plan => "Le plan",
        Limits::Layer => "Le calque courant",
        Limits::InOut => "Entrée et sortie",
        Limits::Bookmarks => "Les signets",
        Limits::Marks { .. } => "Les marques de couleur",
        Limits::Role { .. } => "Un rôle",
        Limits::Custom { .. } => "Personnalisées",
    }
}

fn checkpoints_label(c: &Checkpoints) -> &'static str {
    match c {
        Checkpoints::Images => "Toutes les images",
        Checkpoints::Instances => "Les dessins du calque",
        Checkpoints::Bookmarks => "Les signets",
        Checkpoints::Marks { .. } => "Les marques de couleur",
        Checkpoints::Role { .. } => "Un rôle",
    }
}

/// Réglages d'un flip (le panneau).
fn flip_settings_ui(ui: &mut egui::Ui, f: &mut Flip) {
    egui::Grid::new("reglages_flip").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
        ui.label("Nom");
        ui.add(egui::TextEdit::singleline(&mut f.name).desired_width(200.0));
        ui.end_row();

        ui.label("Sorte");
        ui.horizontal(|ui| {
            for k in FlipKind::ALL {
                ui.selectable_value(&mut f.kind, k, k.label()).on_hover_text(k.help());
            }
        });
        ui.end_row();

        ui.label("Limites").on_hover_text("Jusqu'où le flip va, autour de l'image où il part.");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("limites_flip").selected_text(limits_label(&f.limits)).show_ui(ui, |ui| {
                for l in [
                    Limits::Plan,
                    Limits::Layer,
                    Limits::InOut,
                    Limits::Bookmarks,
                    Limits::Marks { couleur: None },
                    Limits::Role { role: Role::Key },
                    Limits::Custom { gauche: 6, droite: 6 },
                ] {
                    let same = std::mem::discriminant(&l) == std::mem::discriminant(&f.limits);
                    if ui.selectable_label(same, limits_label(&l)).clicked() && !same {
                        f.limits = l;
                    }
                }
            });
            match &mut f.limits {
                Limits::Marks { couleur } => mark_color_ui(ui, "limites_couleur", couleur),
                Limits::Role { role } => role_ui(ui, "limites_role", role),
                Limits::Custom { gauche, droite } => {
                    ui.add(egui::DragValue::new(gauche).range(0..=10_000).prefix("‹ ")).on_hover_text("Images à gauche");
                    ui.add(egui::DragValue::new(droite).range(0..=10_000).suffix(" ›")).on_hover_text("Images à droite");
                }
                _ => {}
            }
        });
        ui.end_row();

        ui.label("Checkpoints").on_hover_text("Les images par lesquelles le flip passe.");
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("checkpoints_flip").selected_text(checkpoints_label(&f.checkpoints)).show_ui(ui, |ui| {
                for c in [Checkpoints::Images, Checkpoints::Instances, Checkpoints::Bookmarks, Checkpoints::Marks { couleur: None }, Checkpoints::Role { role: Role::Key }] {
                    let same = std::mem::discriminant(&c) == std::mem::discriminant(&f.checkpoints);
                    if ui.selectable_label(same, checkpoints_label(&c)).clicked() && !same {
                        f.checkpoints = c;
                    }
                }
            });
            match &mut f.checkpoints {
                Checkpoints::Marks { couleur } => mark_color_ui(ui, "checkpoints_couleur", couleur),
                Checkpoints::Role { role } => role_ui(ui, "checkpoints_role", role),
                _ => {}
            }
        });
        ui.end_row();

        match f.kind {
            FlipKind::Basic => {
                ui.label("Flips par seconde");
                ui.add(egui::DragValue::new(&mut f.rate).range(1.0..=60.0).speed(0.2));
                ui.end_row();
            }
            FlipKind::Directional => {
                ui.label("Direction");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut f.vertical, false, "Horizontale");
                    ui.selectable_value(&mut f.vertical, true, "Verticale");
                    ui.checkbox(&mut f.backward, "En arrière").on_hover_text("Vers la droite (ou le bas) : les images d'avant.");
                });
                ui.end_row();
                ui.label("Distance");
                ui.add(egui::DragValue::new(&mut f.distance).range(2.0..=400.0).suffix(" px")).on_hover_text("Pixels de souris pour passer d'une image à la suivante.");
                ui.end_row();
            }
            FlipKind::Manual => {}
        }

        ui.label("");
        ui.vertical(|ui| {
            ui.checkbox(&mut f.looping, "Boucler entre les limites");
            ui.checkbox(&mut f.back_to_start, "Revenir à l'image de départ après le flip").on_hover_text("Flips basique et directionnel, une fois relâchés.");
        });
        ui.end_row();
    });
}

impl App {
    /// Les checkpoints de `flip` parti de l'image `start`, dans le plan et le calque courants.
    pub(crate) fn flip_points(&self, flip: &Flip, start: usize) -> Vec<usize> {
        FlipScope { panel: self.panel(), layer: self.layer(), len: self.frame_count() }.points(flip, start)
    }

    /// Lance `flip` depuis l'image courante ; il dure tant que `hold` tient.
    pub(crate) fn start_flip(&mut self, flip: Flip, hold: Hold) {
        if self.flip_run.is_some() {
            return;
        }
        self.end_stroke();
        self.playing = false;
        let origin = self.cur_frame;
        let points = self.flip_points(&flip, origin);
        if points.is_empty() {
            self.status = format!("Flip « {} » : aucune image à feuilleter ici.", flip.name);
            return;
        }
        let i0 = flips::index_of(&points, origin);
        let started = self.ctx.input(|i| i.time);
        self.status = format!("Flip « {} » : {} images.", flip.name, points.len());
        self.flip_run = Some(FlipRun { flip, points, origin, i0, index: i0, started, hold, accum: 0.0 });
    }

    /// Le flip courant, tenu par la touche de `action` (flip manuel : rien, il avance par pas).
    pub(crate) fn hold_current_flip(&mut self, action: &str) {
        let flip = self.flips.current();
        if flip.kind == FlipKind::Manual {
            let (prev, next) = (self.keymap.label("flip.precedent"), self.keymap.label("flip.suivant"));
            self.status = format!("Flip manuel : « image précédente / suivante du flip » ({prev} / {next}), ou les flèches du panneau Flips.");
            return;
        }
        self.start_flip(flip, Hold::Key(action.into()));
    }

    /// Flip par rôle : les images de ce rôle du calque courant, sur tout le plan, à la vitesse
    /// et avec les options du flip courant.
    pub(crate) fn role_flip(&mut self, role: Role, hold: Hold) {
        let base = self.flips.current();
        let flip = Flip {
            name: format!("{} (rôle)", role.label()),
            kind: FlipKind::Basic,
            limits: Limits::Plan,
            checkpoints: Checkpoints::Role { role },
            rate: if base.kind == FlipKind::Basic { base.rate } else { 12.0 },
            ..base
        };
        self.start_flip(flip, hold);
    }

    /// Flip manuel : un pas vers l'image précédente (`dir` < 0) ou suivante du flip courant. Les
    /// limites se prennent autour de l'image où l'on a commencé à avancer.
    pub(crate) fn flip_step(&mut self, dir: i32) {
        let flip = self.flips.current();
        let origin = match self.flip_manual {
            Some((origin, last)) if last == self.cur_frame => origin,
            _ => self.cur_frame,
        };
        let points = self.flip_points(&flip, origin);
        match flips::next_point(&points, self.cur_frame, dir, flip.looping) {
            Some(f) => {
                self.goto_frame(f);
                self.flip_manual = Some((origin, f));
            }
            None => self.status = format!("Flip « {} » : aucune image à feuilleter ici.", flip.name),
        }
    }

    /// À chaque image d'affichage : le flip en cours avance, ou finit s'il n'est plus tenu.
    pub(crate) fn tick_flip(&mut self, ctx: &egui::Context) {
        let Some(run) = self.flip_run.as_mut() else { return };
        let (held, now, delta) = ctx.input(|i| {
            let held = match &run.hold {
                Hold::Key(action) => self.keymap.held(i, action),
                Hold::Button => i.pointer.primary_down(),
            };
            (held, i.time, i.pointer.delta())
        });        if !held {
            let run = self.flip_run.take().expect("flip en cours");
            if run.flip.back_to_start {
                self.cur_frame = run.origin;
                self.follow_frame = true;
            }
            return;
        }
        match run.flip.kind {
            FlipKind::Basic => {
                run.index = flips::played(&run.points, run.i0, now - run.started, run.flip.rate, run.flip.looping);
            }
            FlipKind::Directional => {
                let d = if run.flip.vertical { delta.y } else { delta.x };
                run.accum += if run.flip.backward { -d } else { d };
                let step = run.flip.distance.max(2.0);
                while run.accum.abs() >= step {
                    let dir = run.accum.signum() as i32;
                    run.index = flips::step(&run.points, run.index, dir, run.flip.looping);
                    run.accum -= dir as f32 * step;
                }
            }
            FlipKind::Manual => {}
        }
        let f = run.points[run.index.min(run.points.len() - 1)];
        if f != self.cur_frame {
            self.cur_frame = f;
            self.follow_frame = true;
        }
        ctx.request_repaint();
    }

    /// Le panneau « Flips », s'il est ouvert.
    pub(crate) fn flips_window_ui(&mut self, ctx: &egui::Context) {
        if !self.flips_window {
            return;
        }
        let mut open = true;
        egui::Window::new("Flips")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - vec2(240.0, 220.0))
            .pivot(Align2::LEFT_TOP)
            .show(ctx, |ui| self.flips_panel(ui));
        if !open {
            self.flips_window = false;
        }
    }

    fn flips_panel(&mut self, ui: &mut egui::Ui) {
        // Choisir, créer, supprimer.
        ui.horizontal(|ui| {
            let names: Vec<String> = self.flips.flips.iter().map(|f| f.name.clone()).collect();
            let cur = self.flips.current.min(names.len().saturating_sub(1));
            egui::ComboBox::from_id_salt("choisir_flip").selected_text(names.get(cur).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                for (k, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut self.flips.current, k, n);
                }
            });
            if ui.button("Nouveau flip").clicked() {
                let n = self.flips.flips.len() + 1;
                self.flips.flips.push(Flip { name: format!("Flip {n}"), kind: FlipKind::Basic, limits: Limits::Plan, ..Flip::default() });
                self.flips.current = self.flips.flips.len() - 1;
            }
            let can_delete = self.flips.flips.len() > 1;
            if ui.add_enabled(can_delete, egui::Button::new("Supprimer")).on_hover_text("Il reste toujours au moins un flip.").clicked() {
                self.flips.flips.remove(cur);
                self.flips.current = cur.saturating_sub(1);
            }
        });
        ui.separator();
        let cur = self.flips.current.min(self.flips.flips.len().saturating_sub(1));
        if let Some(f) = self.flips.flips.get_mut(cur) {
            flip_settings_ui(ui, f);
        }
        ui.separator();
        // Le flip courant, depuis l'image courante.
        let flip = self.flips.current();
        let n = self.flip_points(&flip, self.cur_frame).len();
        ui.label(RichText::new(format!("Depuis l'image courante : {n} image{}.", if n > 1 { "s" } else { "" })).weak());
        ui.horizontal(|ui| match flip.kind {
            FlipKind::Manual => {
                let key = self.raccourci("flip.precedent");
                if crate::bulles::bulle(ui.button(RichText::new(ph::CARET_LEFT).size(crate::bulles::TAILLE)), "Image précédente du flip", "", &key).clicked() {
                    self.flip_step(-1);
                }
                let key = self.raccourci("flip.suivant");
                if crate::bulles::bulle(ui.button(RichText::new(ph::CARET_RIGHT).size(crate::bulles::TAILLE)), "Image suivante du flip", "", &key).clicked() {
                    self.flip_step(1);
                }
            }
            _ => {
                let text = if flip.kind == FlipKind::Basic { "Tenir pour flipper" } else { "Tenir et glisser pour flipper" };
                let resp = ui.add(egui::Button::new(RichText::new(format!("{}  {text}", ph::BOOK_OPEN))).sense(Sense::click_and_drag()));
                let key = self.raccourci("flip.jouer");
                let resp = crate::bulles::bulle(resp, "Flipper", "Le flip dure tant qu'on tient le bouton (ou la touche).", &key);
                if resp.is_pointer_button_down_on() && self.flip_run.is_none() {
                    self.start_flip(flip.clone(), Hold::Button);
                }
            }
        });
        ui.separator();
        // Flips par rôle.
        ui.label(RichText::new("Flips par rôle (tenir) : les images de ce rôle, sur le calque courant").weak());
        ui.horizontal_wrapped(|ui| {
            for (action, role, label) in ROLE_FLIPS {
                let resp = ui.add(egui::Button::new(label).sense(Sense::click_and_drag()));
                let key = self.raccourci(action);
                let resp = crate::bulles::bulle(resp, &format!("Flipper : {}", role.label()), "Tant qu'on tient le bouton (ou la touche).", &key);
                if resp.is_pointer_button_down_on() && self.flip_run.is_none() {
                    self.role_flip(role, Hold::Button);
                }
            }
        });
    }

    /// L'action qui lance un flip par rôle.
    pub(crate) fn role_flip_action(&mut self, action: &str) -> bool {
        let Some((_, role, _)) = ROLE_FLIPS.iter().find(|(a, _, _)| *a == action) else { return false };
        self.role_flip(*role, Hold::Key(action.into()));
        true
    }

    /// Passe au flip suivant de la liste.
    pub(crate) fn next_flip(&mut self) {
        let n = self.flips.flips.len().max(1);
        self.flips.current = (self.flips.current + 1) % n;
        self.status = format!("Flip courant : « {} ».", self.flips.current().name);
    }
}
