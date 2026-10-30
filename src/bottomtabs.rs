//! Onglets du bas de la timeline (13e), comme ceux de TVPaint : Notes, Son, Groupes,
//! Fond de la feuille, Image de départ. Un clic sur un onglet ouvre sa bande de réglages au-dessus de
//! la barre ; un second clic la referme. Les notes dessinées ont aussi leur rangée dans la
//! grille (alignée sur les images).
//!
//! Le fond de la feuille se dessine ici aussi : aucun (noir à l'écran, transparent à l'export), la
//! couleur du papier, ou un damier des couleurs A et B (transparent à l'export).

use eframe::egui::{self, pos2, Color32, Rect, RichText, Shape, Stroke, TextureHandle, Vec2};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles::bulle;
use crate::document::BackgroundKind;

/// Les onglets du bas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BottomTab {
    Notes,
    Sound,
    Groups,
    Background,
    StartFrame,
}

impl BottomTab {
    pub(crate) const ALL: [BottomTab; 5] = [BottomTab::Notes, BottomTab::Sound, BottomTab::Groups, BottomTab::Background, BottomTab::StartFrame];

    fn label(self) -> (&'static str, &'static str, &'static str) {
        match self {
            BottomTab::Notes => (ph::PENCIL_LINE, "Notes", "Notes dessinées sur la timeline, vues pendant la lecture, jamais exportées."),
            BottomTab::Sound => (ph::WAVEFORM, "Son", "Pistes son : volume, muette, verrou ; Ctrl + glisser l'onde pour la décaler."),
            BottomTab::Groups => (ph::SWATCHES, "Groupes", "Groupes de couleur des calques."),
            BottomTab::Background => (ph::CHECKERBOARD, "Papier", "Ce qu'on voit sous les calques : aucun (transparent), la couleur du papier ou un damier."),
            BottomTab::StartFrame => (ph::HASH, "Image de départ", "Numéro de la première image du projet."),
        }
    }
}

/// Damier en cache : couleurs A et B, texture 2 × 2 répétée.
#[derive(Default)]
pub(crate) struct Checker {
    texture: Option<(([u8; 3], [u8; 3]), TextureHandle)>,
}

impl App {
    /// La barre d'onglets du bas (en bas du panneau) et la bande de l'onglet ouvert.
    pub(crate) fn bottom_tabs(&mut self, ui: &mut egui::Ui) {
        if let Some(tab) = self.bottom_tab {
            ui.separator();
            ui.horizontal_wrapped(|ui| match tab {
                BottomTab::Notes => self.notes_strip_ui(ui),
                BottomTab::Sound => self.sound_tab_ui(ui),
                BottomTab::Groups => self.groups_inline(ui),
                BottomTab::Background => self.background_ui(ui),
                BottomTab::StartFrame => self.start_frame_ui(ui),
            });
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for tab in BottomTab::ALL {
                let (icon, name, description) = tab.label();
                let on = self.bottom_tab == Some(tab);
                let resp = ui.selectable_label(on, RichText::new(format!("{icon} {name}")).size(12.0));
                if bulle(resp, name, description, "").clicked() {
                    self.bottom_tab = if on { None } else { Some(tab) };
                }
            }
        });
    }

    /// Ouvre l'onglet `tab` (ou le referme s'il l'était).
    pub(crate) fn toggle_bottom_tab(&mut self, tab: BottomTab) {
        self.bottom_tab = if self.bottom_tab == Some(tab) { None } else { Some(tab) };
    }

    /// Choisit l'arrière-plan (une étape d'annulation).
    pub(crate) fn set_background(&mut self, kind: BackgroundKind) {
        if self.doc.background != kind {
            self.record_step();
            self.doc.background = kind;
            self.renderer.invalidate_all();
        }
        self.status = format!("Papier : {}.", kind.label().to_lowercase());
    }

    /// Onglet Son : volume général, son à la lecture et au défilement ; chaque piste (muette,
    /// verrouillée, volume, dupliquer, supprimer). Ctrl + glisser l'onde dans la timeline décale
    /// un extrait.
    fn sound_tab_ui(&mut self, ui: &mut egui::Ui) {
        let mut master = self.doc.master_volume * 100.0;
        let resp = ui.add(egui::DragValue::new(&mut master).range(0.0..=400.0).suffix(" %").prefix("volume général "));
        if self.edited(&resp) {
            self.doc.master_volume = master / 100.0;
        }
        let mut on = self.sound_on;
        if ui.checkbox(&mut on, "à la lecture").changed() {
            self.run_action("son.lecture");
        }
        let mut scrub = self.sound_scrub;
        if ui.checkbox(&mut scrub, "au défilement").changed() {
            self.run_action("son.defilement");
        }
        ui.separator();
        if self.doc.audio_tracks.is_empty() {
            ui.label("Aucune piste son.");
        }
        let mut delete = None;
        let mut duplicate = None;
        for ti in 0..self.doc.audio_tracks.len() {
            let (name, muted, locked, volume) = {
                let t = &self.doc.audio_tracks[ti];
                (t.name.clone(), t.muted, t.locked, t.volume)
            };
            ui.label(RichText::new(if name.is_empty() { format!("Piste {}", ti + 1) } else { name }).strong());
            let mut m = muted;
            if ui.checkbox(&mut m, "muette").changed() {
                self.record_step();
                self.doc.audio_tracks[ti].muted = m;
            }
            let mut solo = self.doc.audio_tracks[ti].solo;
            if ui.checkbox(&mut solo, "solo").on_hover_text("Seules les pistes en solo s'entendent.").changed() {
                self.record_step();
                self.doc.audio_tracks[ti].solo = solo;
            }
            let mut l = locked;
            let resp = crate::bulles::bulle(ui.checkbox(&mut l, ph::LOCK_SIMPLE), "Verrouiller la piste", "Verrouillée : ses extraits ne bougent plus.", "");
            if resp.changed() {
                self.record_step();
                self.doc.audio_tracks[ti].locked = l;
            }
            let mut v = volume * 100.0;
            let resp = ui.add(egui::DragValue::new(&mut v).range(0.0..=400.0).suffix(" %"));
            if self.edited(&resp) {
                self.doc.audio_tracks[ti].volume = v / 100.0;
            }
            if crate::bulles::bulle(ui.small_button(ph::COPY), "Dupliquer la piste", "Une copie de la piste son, avec ses extraits.", "").clicked() {
                duplicate = Some(ti);
            }
            if crate::bulles::bulle(ui.small_button(ph::TRASH), "Supprimer la piste", "La piste son et ses extraits.", "").clicked() {
                delete = Some(ti);
            }
            ui.separator();
        }
        if let Some(t) = duplicate {
            self.duplicate_track(t);
        }
        if let Some(t) = delete {
            self.delete_audio_track(t);
        }
        self.action_item(ui, "Importer un son…", "son.importer");
        ui.label(RichText::new("Ctrl + glisser l'onde dans la timeline : décaler l'extrait.").weak().small());
    }

    fn background_ui(&mut self, ui: &mut egui::Ui) {
        let mut kind = self.doc.background;
        for k in BackgroundKind::ALL {
            ui.radio_value(&mut kind, k, k.label());
        }
        if kind != self.doc.background {
            self.set_background(kind);
        }
        ui.separator();
        ui.label("Couleur A");
        let mut a = self.doc.paper;
        let resp = crate::champcouleur::srgb(ui, &mut a);
        if self.edited(&resp) {
            self.doc.paper = a;
            self.renderer.invalidate_all();
        }
        ui.label("Couleur B (damier)");
        let mut b = self.doc.paper_b;
        let resp = crate::champcouleur::srgb(ui, &mut b);
        if self.edited(&resp) {
            self.doc.paper_b = b;
        }
        ui.label(RichText::new("La couleur sort à l'export ; « aucun » et le damier donnent une image transparente.").weak().small());
    }

    fn start_frame_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Numéro de la première image");
        let mut start = self.doc.start_frame;
        let resp = ui.add(egui::DragValue::new(&mut start).range(0..=100_000));
        if self.edited(&resp) {
            self.doc.start_frame = start;
        }
        ui.label(RichText::new("Pour qu'un plan coïncide avec une séquence existante (numéros de la timeline et du time code).").weak().small());
    }

    fn groups_inline(&mut self, ui: &mut egui::Ui) {
        if ui.button(format!("{} Ouvrir les groupes de calques", ph::SWATCHES)).clicked() {
            self.groups_window = true;
        }
        ui.menu_button("Commandes de groupes", |ui| self.groups_menu(ui));
    }

    /// Dessine l'arrière-plan sous les calques. Toile infinie : sur tout le monde visible (`world`, la
    /// zone du monde que la vue regarde) ; sinon (vignette, vue en perspective), sur la feuille seule.
    /// `paper` : la couleur du papier du plan, que la feuille posera aussi ; hors de la feuille, il
    /// faut la peindre ici.
    pub(crate) fn paint_background(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, size: Vec2, world: Option<crate::render::Area>, paper: Option<[u8; 3]>) {
        let sheet = crate::render::Area::sheet(&self.doc);
        let area = world.map_or(sheet, |w| w.union(&sheet));
        let (x0, y0, x1, y1) = (area.x0 as f32, area.y0 as f32, area.x1 as f32, area.y1 as f32);
        let corners = [pos2(x0, y0), pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)].map(|p| self.view.to_screen(rect, size, p));
        match self.doc.background {
            BackgroundKind::Color => {
                if let (Some(_), Some(p)) = (world, paper) {
                    painter.add(Shape::convex_polygon(corners.to_vec(), Color32::from_rgb(p[0], p[1], p[2]), Stroke::NONE));
                }
            }
            BackgroundKind::None => {
                painter.add(Shape::convex_polygon(corners.to_vec(), Color32::BLACK, Stroke::NONE));
            }
            BackgroundKind::Checker => {
                let key = (self.doc.paper, self.doc.paper_b);
                let texture = match &self.checker.texture {
                    Some((k, t)) if *k == key => t.clone(),
                    _ => {
                        let [a, b] = [key.0, key.1].map(|c| Color32::from_rgb(c[0], c[1], c[2]));
                        let image = egui::ColorImage { size: [2, 2], pixels: vec![a, b, b, a] };
                        let options = egui::TextureOptions {
                            magnification: egui::TextureFilter::Nearest,
                            minification: egui::TextureFilter::Nearest,
                            wrap_mode: egui::TextureWrapMode::Repeat,
                            mipmap_mode: None,
                        };
                        let t = ctx.load_texture("damier", image, options);
                        self.checker.texture = Some((key, t.clone()));
                        t
                    }
                };
                // Une case = 32 pixels de la feuille ; le damier part du coin de la feuille et continue dans
                // tout le monde.
                let (u0, v0, u1, v1) = (x0 / 64.0, y0 / 64.0, x1 / 64.0, y1 / 64.0);
                let mut mesh = egui::Mesh::with_texture(texture.id());
                for (p, uv) in corners.iter().zip([pos2(u0, v0), pos2(u1, v0), pos2(u1, v1), pos2(u0, v1)]) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: *p, uv, color: Color32::WHITE });
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                painter.add(Shape::mesh(mesh));
            }
        }
    }
}
