//! Préréglages d'effets et calques d'effets dans l'interface : la fenêtre « Préréglage d'effet »
//! (réglages, aperçu en direct sur le dessin), les menus « Calque d'effet », les actions de la
//! palette (Ctrl+K), l'éditeur d'une incrustation et son tracé à l'écran. La logique est dans
//! `prereglages.rs` (clés), `calqueffet.rs` (rendu) et `incrustation.rs` (texte).
//!
//! L'aperçu n'est pas un calcul de plus : tant que la fenêtre est ouverte, le calque qu'elle
//! créerait est ajouté à la pile des calques POUR L'AFFICHAGE (`build_plan`), et passe par le même
//! chemin que les autres effets — demandé au fil de fond, montré quand il est prêt, jamais calculé sur
//! le fil de l'interface. « Appliquer » écrit ce même calque dans le plan (une étape d'annulation).

use eframe::egui::{self, vec2, RichText};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::document::{Layer, LayerKind};
use crate::incrustation::{self, Ancre, Incrustation, Portee, Quand, Source};
use crate::prereglages::{self, Construit, Contexte, Genre, Reglages};

/// Ce que l'interface garde des préréglages d'effets.
#[derive(Default)]
pub(crate) struct EffetsUi {
    /// La fenêtre de réglage, si elle est ouverte.
    pub dialog: Option<EffetDialog>,
    /// Derniers réglages appliqués : la fenêtre repart de là.
    pub last: Option<Reglages>,
    /// Textures des incrustations (contenu → texture), les plus récentes d'abord.
    textures: Vec<(u64, egui::TextureHandle)>,
}

pub(crate) struct EffetDialog {
    pub r: Reglages,
}

/// Un nom qui n'est pas déjà celui d'un calque : « Impact frame », « Impact frame 2 »…
fn unique_name(layers: &[Layer], base: &str) -> String {
    if !layers.iter().any(|l| l.name == base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base} {n}")).find(|n| !layers.iter().any(|l| &l.name == n)).unwrap_or_else(|| base.to_string())
}

/// Le calque d'effet que `built` décrit, dont les images montrent le dessin `cel`.
pub(crate) fn layer_of(built: &Construit, cel: crate::document::CelId) -> Layer {
    let mut l = Layer::new_effect(built.nom.clone(), built.effet.clone());
    l.fx = built.fx.clone();
    l.blend = built.fusion;
    l.frames = vec![None; built.debut];
    l.frames.extend(std::iter::repeat_n(Some(cel), built.longueur));
    l
}

impl App {
    /// Le projet à la tête de lecture, pour le plan `panel` : de quoi construire un préréglage.
    pub(crate) fn effect_context(&self, panel: usize, debut: usize) -> Contexte {
        let doc = &self.doc;
        let images = doc.panels[panel].frame_count();
        let cam = doc.panel_camera(panel, debut.min(images.saturating_sub(1)));
        let corners = cam.corners(doc.frame);
        let (mut lo, mut hi) = (corners[0], corners[0]);
        for c in corners {
            lo = lo.min(c);
            hi = hi.max(c);
        }
        let (w, h) = (doc.width.max(1) as f32, doc.height.max(1) as f32);
        Contexte {
            fps: doc.fps,
            cadre: doc.frame,
            debut,
            images,
            centre: [(lo.x + hi.x) / 2.0 / w * 100.0, (lo.y + hi.y) / 2.0 / h * 100.0],
            taille: [((hi.x - lo.x) / w * 100.0).clamp(1.0, 200.0), ((hi.y - lo.y) / h * 100.0).clamp(1.0, 200.0)],
        }
    }

    /// Rang du nouveau calque d'effet dans le plan courant : tout au-dessus, pour couvrir tout ce qui
    /// est dessous ; juste au-dessus du calque courant s'il est rangé dans un dossier (l'effet reste
    /// dans le dossier et ne couvre que lui).
    fn new_effect_index(&self) -> usize {
        match self.layers().get(self.cur_layer) {
            Some(l) if l.parent.is_some() => self.cur_layer + 1,
            _ => self.layers().len(),
        }
    }

    /// La pile de calques du plan courant avec, en plus, ce que la fenêtre des préréglages
    /// ajouterait (aperçu en direct) ; et le rang du calque courant dedans. Rien si la fenêtre est fermée.
    pub(crate) fn effect_preview_layers(&self) -> Option<(Vec<Layer>, usize)> {
        let d = self.effets_ui.dialog.as_ref()?;
        let r = d.r.sanitized(self.doc.fps);
        let built = prereglages::build(&r, &self.effect_context(self.cur_panel, self.cur_frame))?;
        let mut layers = self.layers().to_vec();
        if r.sur_calque && r.genre.is_effect() {
            let l = layers.get_mut(self.cur_layer).filter(|l| !l.folder)?;
            l.fx.extend(built.fx);
            return Some((layers, self.cur_layer));
        }
        let mut candidate = layer_of(&built, 0);
        let at = self.new_effect_index();
        // (Dans un dossier : il en fait partie, et ne couvre que lui.)
        candidate.parent = if at == self.cur_layer + 1 { self.layers().get(self.cur_layer).and_then(|l| l.parent) } else { None };
        layers.insert(at, candidate);
        let cur = if at <= self.cur_layer { self.cur_layer + 1 } else { self.cur_layer };
        Some((layers, cur))
    }

    /// Ouvre la fenêtre du préréglage `genre` (les derniers réglages, pour l'intensité et la graine).
    pub(crate) fn open_effect_dialog(&mut self, genre: Genre) {
        let fps = self.doc.fps;
        let r = match &self.effets_ui.last {
            Some(last) => last.with_genre(genre, fps),
            None => Reglages::new(genre, fps),
        };
        self.effets_ui.dialog = Some(EffetDialog { r });
        self.renderer.invalidate_all();
    }

    /// Pose le préréglage `r` : un calque d'effet neuf (ou des effets sur le calque courant), avec ses
    /// clés à partir de la tête de lecture. Une étape d'annulation.
    pub(crate) fn apply_effect_preset(&mut self, r: &Reglages) {
        if self.content_refuses() {
            return;
        }
        let r = r.sanitized(self.doc.fps);
        let on_layer = r.sur_calque && r.genre.is_effect();
        if on_layer && self.layers().get(self.cur_layer).is_none_or(|l| l.folder) {
            self.status = "Un dossier n'a pas d'effets : choisis un de ses calques, ou crée un calque d'effet.".into();
            return;
        }
        let all_plans = r.tous_les_plans && r.duree.is_none() && !on_layer;
        self.end_stroke();
        self.playing = false;
        let snap = self.snapshot();
        let panels: Vec<usize> = if all_plans { (0..self.doc.panels.len()).collect() } else { vec![self.cur_panel] };
        let (mut first_frame, mut count, mut cut) = (0usize, 0usize, false);
        let mut made = 0usize;
        for pi in panels {
            let debut = if all_plans { 0 } else { self.cur_frame };
            let Some(built) = prereglages::build(&r, &self.effect_context(pi, debut)) else { continue };
            (first_frame, count, cut) = (built.debut, built.longueur, cut || built.coupe);
            made += 1;
            if on_layer {
                let li = self.cur_layer;
                let first = self.layers()[li].fx.len();
                self.layers_mut()[li].fx.extend(built.fx);
                self.select_fx(first);
                continue;
            }
            let cel = self.doc.new_cel(LayerKind::Bitmap);
            let mut layer = layer_of(&built, cel);
            layer.name = unique_name(&self.doc.panels[pi].layers, &layer.name);
            let layers = &mut self.doc.panels[pi].layers;
            let at = if pi == self.cur_panel {
                match layers.get(self.cur_layer) {
                    Some(l) if l.parent.is_some() => {
                        layer.parent = l.parent;
                        self.cur_layer + 1
                    }
                    _ => layers.len(),
                }
            } else {
                layers.len()
            };
            layers.insert(at, layer);
            if pi == self.cur_panel {
                self.cur_layer = at;
                self.layer_sel.clear();
                self.vsel = None;
                if !self.layers()[at].fx.is_empty() {
                    self.select_fx(0);
                }
            }
        }
        if made == 0 {
            self.status = "Préréglage d'effet : l'image de départ est après la fin du plan, rien n'est posé.".into();
            return;
        }
        self.effets_ui.last = Some(r.clone());
        self.renderer.invalidate_all();
        let name = r.genre.layer_name();
        self.status = format!(
            "{name} : {}{} à partir de l'image {}{}. Les clés se règlent dans le panneau Effets ; Ctrl+Z annule tout.",
            if on_layer { "effets posés sur le calque" } else { "calque d'effet posé" },
            if all_plans { format!(" dans {made} plans") } else { format!(" ({count} image{})", if count > 1 { "s" } else { "" }) },
            self.frame_label(first_frame),
            if cut { " (coupé par la fin du plan)" } else { "" }
        );
        self.commit_as(snap, &format!("Préréglage d'effet : {name}"));
        if r.genre.is_effect() {
            self.show_pane(crate::dock::Pane::Effects);
        }
    }

    /// Un calque d'effet vide : il couvre tout le plan, on y ajoute des effets dans le panneau Effets.
    pub(crate) fn new_empty_effect_layer(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        let snap = self.snapshot();
        let len = self.frame_count();
        let cel = self.doc.new_cel(LayerKind::Bitmap);
        let mut layer = Layer::new_effect(unique_name(self.layers(), "Calque d'effet"), crate::calqueffet::CalqueEffet::ajustement(""));
        layer.frames = vec![Some(cel); len];
        let at = self.new_effect_index();
        layer.parent = if at == self.cur_layer + 1 { self.layers().get(self.cur_layer).and_then(|l| l.parent) } else { None };
        self.layers_mut().insert(at, layer);
        self.cur_layer = at;
        self.layer_sel.clear();
        self.vsel = None;
        self.renderer.invalidate_all();
        self.status = "Calque d'effet créé : il agit sur tout ce qui est dessous. Ajoute-lui des effets dans le panneau Effets.".into();
        self.commit_as(snap, "Calque d'effet");
        self.show_pane(crate::dock::Pane::Effects);
    }

    /// Les actions « effet.* » : ouvrir la fenêtre d'un préréglage, l'appliquer, créer un calque vide.
    pub(crate) fn effet_action(&mut self, action: &str) -> bool {
        if let Some(genre) = Genre::from_action(action) {
            self.open_effect_dialog(genre);
            return true;
        }
        match action {
            "effet.appliquer" => {
                if let Some(d) = self.effets_ui.dialog.take() {
                    self.apply_effect_preset(&d.r);
                    self.renderer.invalidate_all();
                }
                true
            }
            "effet.vide" => {
                self.new_empty_effect_layer();
                true
            }
            "effet.fermer" => {
                if self.effets_ui.dialog.take().is_some() {
                    self.renderer.invalidate_all();
                }
                true
            }
            _ => false,
        }
    }

    /// Les préréglages en entrées de menu (menu Calques, clic droit, bouton du panneau Calques, panneau
    /// Effets).
    pub(crate) fn effect_preset_menu(&mut self, ui: &mut egui::Ui) {
        for genre in Genre::ALL {
            let resp = ui.add(egui::Button::new(format!("{}…", genre.label())).shortcut_text(self.keymap.label(genre.action())));
            let resp = resp.on_hover_text(genre.note());
            if resp.clicked() {
                ui.close_menu();
                self.run_action(genre.action());
            } else if resp.secondary_clicked() {
                ui.close_menu();
                self.start_assign(genre.action());
            }
        }
        ui.separator();
        self.action_item(ui, "Calque d'effet vide (au-dessus de tout)", "effet.vide");
    }

    /// La fenêtre « Préréglage d'effet ».
    pub(crate) fn effect_dialog_window(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.effets_ui.dialog.take() else { return };
        let fps = self.doc.fps;
        let before = d.r.clone();
        let (mut apply, mut cancel) = (false, false);
        let sample = {
            let inc = &d.r.incrustation;
            incrustation::text_for(inc, &self.doc, self.cur_panel, self.cur_frame, incrustation::now_secs())
        };
        let start = self.cur_frame;
        let images = self.frame_count();
        let start_label = self.frame_label(start);
        let end_label = self.frame_label(images.saturating_sub(1));
        let has_folder_layer = self.layers().get(self.cur_layer).is_some_and(|l| l.folder);
        // (À droite de l'écran au départ, et déplaçable : le dessin et son aperçu restent en vue.)
        let screen = ctx.screen_rect();
        egui::Window::new("Préréglage d'effet")
            .collapsible(false)
            .resizable(false)
            .default_pos(egui::pos2((screen.right() - 360.0).max(10.0), screen.top() + 96.0))
            .show(ctx, |ui| {
                ui.set_max_width(330.0);
                let r = &mut d.r;
                ui.horizontal(|ui| {
                    ui.label("Préréglage");
                    let mut picked = None;
                    egui::ComboBox::from_id_salt("prereglage_effet").selected_text(r.genre.label()).width(230.0).show_ui(ui, |ui| {
                        for g in Genre::ALL {
                            if ui.selectable_label(r.genre == g, g.label()).on_hover_text(g.note()).clicked() {
                                picked = Some(g);
                            }
                        }
                    });
                    if let Some(g) = picked {
                        *r = r.with_genre(g, fps);
                    }
                });
                ui.label(RichText::new(r.genre.note()).small().weak());
                ui.add_space(4.0);
                egui::Grid::new("grille_prereglage").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                    if r.genre.is_effect() {
                        ui.label("Où");
                        ui.vertical(|ui| {
                            ui.radio_value(&mut r.sur_calque, false, "Nouveau calque d'effet").on_hover_text("Agit sur tout ce qui est dessous ; durée et clés visibles dans la timeline.");
                            ui.add_enabled_ui(!has_folder_layer, |ui| {
                                ui.radio_value(&mut r.sur_calque, true, "Sur le calque courant").on_hover_text("Ajoute les effets à la pile du calque courant, avec leurs clés.");
                            });
                        });
                        ui.end_row();
                    }
                    if r.genre.has_intensity() {
                        ui.label("Intensité");
                        let mut pct = r.intensite * 100.0;
                        if ui.add(egui::DragValue::new(&mut pct).range(0.0..=300.0).speed(1.0).max_decimals(0).suffix(" %")).on_hover_text("100 % : la force proposée.").changed() {
                            r.intensite = pct / 100.0;
                        }
                        ui.end_row();
                    }
                    ui.label("Durée");
                    ui.vertical(|ui| {
                        let mut until_end = r.duree.is_none();
                        if ui.checkbox(&mut until_end, "Jusqu'à la fin du plan").changed() {
                            r.duree = if until_end { None } else { r.genre.default_frames(fps).or(Some(fps as usize)) };
                        }
                        if let Some(n) = &mut r.duree {
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(n).range(1..=5000).speed(0.3).suffix(" i")).on_hover_text("Images du calque d'effet (et des clés).");
                                ui.label(RichText::new(crate::storyboard::timecode(*n, fps)).small().weak());
                            });
                        }
                    });
                    ui.end_row();
                    if r.genre.has_color() {
                        ui.label("Couleur");
                        ui.color_edit_button_srgb(&mut r.couleur);
                        ui.end_row();
                    }
                    if r.genre.has_seed() {
                        ui.label("Graine");
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut r.graine).range(1..=999).speed(0.2)).on_hover_text("Même graine, mêmes clés : l'effet se refait à l'identique.");
                            if crate::bulles::icone(ui, ph::DICE_FIVE, "Autre graine", "Tirer un autre hasard.", "").clicked() {
                                r.graine = (r.graine.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)) % 999 + 1;
                            }
                        });
                        ui.end_row();
                    }
                });
                // Les réglages propres à chaque préréglage.
                match r.genre {
                    Genre::Impact => {
                        egui::ComboBox::from_id_salt("mode_impact")
                            .selected_text(["Négatif", "Noir et blanc", "Noir et blanc inversé", "Postérisé"][r.mode_impact.min(3) as usize])
                            .show_ui(ui, |ui| {
                                for (k, name) in ["Négatif", "Noir et blanc", "Noir et blanc inversé", "Postérisé"].into_iter().enumerate() {
                                    ui.selectable_value(&mut r.mode_impact, k as u8, name);
                                }
                            });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut r.teinte, "Teinte");
                            if r.teinte {
                                ui.color_edit_button_srgb(&mut r.couleur);
                            }
                        });
                        ui.checkbox(&mut r.eclat, "Éclat de rayons (burst)");
                        ui.checkbox(&mut r.lignes, "Lignes de vitesse");
                    }
                    Genre::Lueur => {
                        ui.horizontal(|ui| {
                            ui.label("Seuil de brillance");
                            ui.add(egui::DragValue::new(&mut r.seuil).range(0.0..=255.0).speed(1.0).max_decimals(0)).on_hover_text("Seules les parties plus claires que ça brillent.");
                        });
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut r.pulsation, "Pulsation");
                            if r.pulsation {
                                ui.add(egui::DragValue::new(&mut r.periode).range(0.1..=30.0).speed(0.05).max_decimals(2).suffix(" s")).on_hover_text("Durée d'une pulsation.");
                            }
                        });
                    }
                    Genre::Tremblement => {
                        egui::ComboBox::from_id_salt("style_tremblement").selected_text(r.style.label()).width(240.0).show_ui(ui, |ui| {
                            for p in [crate::camfx::Preset::LightShake, crate::camfx::Preset::Shoulder, crate::camfx::Preset::Explosion, crate::camfx::Preset::Jolt] {
                                if ui.selectable_label(r.style == p, p.label()).on_hover_text(p.note()).clicked() {
                                    r.style = p;
                                    r.frequence = p.default_freq();
                                    r.duree = r.duree.map(|_| ((p.default_seconds() * fps as f32).round() as usize).max(1));
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label("Fréquence");
                            ui.add(egui::DragValue::new(&mut r.frequence).range(0.1..=(fps as f32 / 2.0).max(1.0)).speed(0.05).max_decimals(1).suffix(" Hz"));
                        });
                        ui.checkbox(&mut r.flou_bouge, "Flou de bougé").on_hover_text("Floute l'image dans le sens du décalage.");
                    }
                    Genre::FlouVitesse => {
                        ui.horizontal(|ui| {
                            ui.label("Direction");
                            ui.add(egui::DragValue::new(&mut r.angle).range(-180.0..=180.0).speed(1.0).max_decimals(0).suffix("°")).on_hover_text("0° : horizontal.");
                        });
                    }
                    Genre::Timecode => {
                        incrustation_editor(ui, &mut r.incrustation, &sample, "dialogue");
                    }
                    _ => {}
                }
                if matches!(r.genre, Genre::Timecode | Genre::Vignettage | Genre::Grain | Genre::Aberration | Genre::Lueur) && r.duree.is_none() && !r.sur_calque {
                    ui.checkbox(&mut r.tous_les_plans, "Dans tous les plans du film").on_hover_text("Un calque d'effet par plan, sur toute sa durée.");
                }
                ui.add_space(4.0);
                let shown = r.duree.map_or(images.saturating_sub(start), |n| n.min(images.saturating_sub(start)));
                ui.label(
                    RichText::new(if start >= images {
                        "L'image courante est après la fin du plan : rien ne sera posé.".to_string()
                    } else if r.tous_les_plans && r.duree.is_none() {
                        "Tous les plans, sur toute leur durée.".to_string()
                    } else {
                        format!("De l'image {start_label} à l'image {end_label} du plan : {shown} image{}.", if shown > 1 { "s" } else { "" })
                    })
                    .small()
                    .weak(),
                );
                ui.label(RichText::new("L'aperçu est sur le dessin : change les réglages, il suit.").small().weak());
                ui.horizontal(|ui| {
                    let key = self.keymap.label("effet.appliquer");
                    let resp = ui.add_enabled(start < images, egui::Button::new(format!("{}  Appliquer", ph::MAGIC_WAND)));
                    if crate::bulles::bulle(resp, "Appliquer le préréglage", "Pose le calque d'effet (ou les effets) avec ses clés ; une seule étape d'annulation.", &key).clicked() {
                        apply = true;
                    }
                    if ui.button("Annuler").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel = true;
                    }
                });
            });
        if apply {
            self.apply_effect_preset(&d.r);
            self.renderer.invalidate_all();
        } else if cancel {
            self.renderer.invalidate_all();
        } else {
            if d.r != before {
                // (L'aperçu change : l'image se recompose, les effets passent par le fil de fond.)
                self.renderer.invalidate_all();
                crate::fx::note_interaction(ctx);
            }
            self.effets_ui.dialog = Some(d);
        }
    }

    /// Les incrustations de l'image courante, posées sur le dessin dans le cadre de la caméra.
    pub(crate) fn paint_incrustations(&mut self, ctx: &egui::Context, painter: &egui::Painter) {
        if self.in_card {
            return;
        }
        let preview = self.effect_preview_layers();
        let layers: &[Layer] = preview.as_ref().map_or(self.layers(), |p| p.0.as_slice());
        if !layers.iter().any(|l| l.overlay().is_some()) {
            return;
        }
        let boxes = incrustation::boxes_in(&self.doc, layers, self.cur_panel, self.cur_frame, false, incrustation::now_secs());
        if boxes.is_empty() {
            return;
        }
        let map = self.doc.panel_camera(self.cur_panel, self.cur_frame).frame_map(self.doc.frame);
        let (rect, size) = (self.canvas_rect, self.canvas_size());
        for b in boxes {
            let (w, h) = (b.raster.width, b.raster.height);
            let pixels = b.raster.to_flat();
            let key = {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (w, h).hash(&mut hasher);
                for p in &pixels {
                    p.to_array().hash(&mut hasher);
                }
                hasher.finish()
            };
            let texture = match self.effets_ui.textures.iter().position(|(k, _)| *k == key) {
                Some(i) => self.effets_ui.textures[i].1.clone(),
                None => {
                    let t = ctx.load_texture("incrustation", egui::ColorImage { size: [w, h], pixels }, egui::TextureOptions::LINEAR);
                    self.effets_ui.textures.insert(0, (key, t.clone()));
                    self.effets_ui.textures.truncate(6);
                    t
                }
            };
            let (x0, y0, x1, y1) = (b.x as f32, b.y as f32, (b.x + w as i32) as f32, (b.y + h as i32) as f32);
            let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(u, v)| map.at(u, v).map(|p| self.view.to_screen(rect, size, p)));
            if corners.iter().any(Option::is_none) {
                continue;
            }
            let uvs = [egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(0.0, 1.0)];
            let mut mesh = egui::Mesh::with_texture(texture.id());
            for (c, uv) in corners.iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex { pos: c.unwrap_or_default(), uv, color: egui::Color32::WHITE });
            }
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            painter.add(egui::Shape::mesh(mesh));
        }
    }

    /// Dans le panneau Effets : ce qu'est le calque d'effet courant, et l'éditeur de son incrustation.
    pub(crate) fn effect_layer_section(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let Some(l) = self.layers().get(li) else { return };
        let Some(effet) = l.effet.clone() else { return };
        match effet.contenu {
            crate::calqueffet::Contenu::Ajustement => {
                ui.label(RichText::new(format!("{}  Calque d'effet : sa pile agit sur tout ce qui est dessous{}.", ph::SPARKLE, if l.parent.is_some() { " dans son dossier" } else { "" })).small())
                    .on_hover_text("Il agit aux images où il a une exposition (comme un dessin tenu dans la timeline) ; les clés des effets font varier les réglages. Son opacité et son mode de fusion sont ceux du panneau Calques.");
                ui.separator();
            }
            crate::calqueffet::Contenu::Incrustation(mut inc) => {
                ui.label(RichText::new(format!("{}  Incrustation : du texte sur l'image finale.", ph::TEXT_T)).small())
                    .on_hover_text("Posé dans le cadre de la caméra, gravé dans l'export quand le calque est visible. Il agit aux images où le calque a une exposition.");
                let sample = incrustation::text_for(&inc, &self.doc, self.cur_panel, self.cur_frame, incrustation::now_secs());
                let before = inc.clone();
                incrustation_editor(ui, &mut inc, &sample, "panneau");
                if inc != before {
                    self.note_change(egui::Id::new(("incrustation", li)));
                    if let Some(l) = self.layers_mut().get_mut(li) {
                        l.effet = Some(crate::calqueffet::CalqueEffet { contenu: crate::calqueffet::Contenu::Incrustation(inc), prereglage: effet.prereglage.clone() });
                    }
                    self.renderer.invalidate_all();
                }
                ui.separator();
            }
        }
    }
}

/// Les réglages d'une incrustation (dans la fenêtre des préréglages et dans le panneau Effets).
/// `sample` : le texte tel qu'il s'écrirait à l'image courante.
pub(crate) fn incrustation_editor(ui: &mut egui::Ui, inc: &mut Incrustation, sample: &str, salt: &str) {
    egui::Grid::new(("grille_incrustation", salt)).num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label("Contenu");
        egui::ComboBox::from_id_salt(("incrustation_source", salt)).selected_text(inc.source.label()).width(190.0).show_ui(ui, |ui| {
            for s in Source::ALL {
                ui.selectable_value(&mut inc.source, s, s.label());
            }
        });
        ui.end_row();
        if inc.source == Source::Libre {
            ui.label("Texte");
            ui.add(egui::TextEdit::singleline(&mut inc.texte).desired_width(190.0).hint_text("{timecode} {image} {plan} {sequence} {titre} {date}"))
                .on_hover_text("Les mots entre accolades sont remplacés : {timecode} {image} {plan} {sequence} {titre} {date}.");
            ui.end_row();
        }
        if matches!(inc.source, Source::Timecode | Source::NumeroImage | Source::Libre) {
            ui.label("Compté depuis");
            ui.horizontal(|ui| {
                ui.radio_value(&mut inc.portee, Portee::Montage, "le film");
                ui.radio_value(&mut inc.portee, Portee::Plan, "le plan");
            });
            ui.end_row();
        }
        ui.label("Place");
        egui::Grid::new(("incrustation_ancres", salt)).spacing([2.0, 2.0]).show(ui, |ui| {
            let icons = [
                ph::ARROW_UP_LEFT,
                ph::ARROW_UP,
                ph::ARROW_UP_RIGHT,
                ph::ARROW_LEFT,
                ph::CIRCLE,
                ph::ARROW_RIGHT,
                ph::ARROW_DOWN_LEFT,
                ph::ARROW_DOWN,
                ph::ARROW_DOWN_RIGHT,
            ];
            for (k, a) in Ancre::ALL.into_iter().enumerate() {
                let icon = crate::bulles::symbole_etat(icons[k], inc.ancre == a);
                if ui.add(egui::Button::new(icon).selected(inc.ancre == a).min_size(vec2(24.0, 20.0))).on_hover_text(a.label()).clicked() {
                    inc.ancre = a;
                }
                if k % 3 == 2 {
                    ui.end_row();
                }
            }
        });
        ui.end_row();
        ui.label("Taille");
        ui.add(egui::DragValue::new(&mut inc.taille).range(1.0..=30.0).speed(0.1).max_decimals(1).suffix(" %")).on_hover_text("Hauteur des lettres, en % de la hauteur du cadre.");
        ui.end_row();
        ui.label("Couleur");
        ui.color_edit_button_srgba_unmultiplied(&mut inc.couleur);
        ui.end_row();
        ui.label("Boîte");
        ui.horizontal(|ui| {
            let mut on = inc.fond.is_some();
            if ui.checkbox(&mut on, "").on_hover_text("Une boîte derrière le texte.").changed() {
                inc.fond = on.then_some([0, 0, 0, 160]);
            }
            if let Some(f) = &mut inc.fond {
                ui.color_edit_button_srgba_unmultiplied(f);
            }
        });
        ui.end_row();
        ui.label("Marge");
        ui.add(egui::DragValue::new(&mut inc.marge).range(0.0..=30.0).speed(0.1).max_decimals(1).suffix(" %")).on_hover_text("Distance au bord du cadre, en % de sa hauteur.");
        ui.end_row();
        ui.label("Visible");
        egui::ComboBox::from_id_salt(("incrustation_quand", salt)).selected_text(inc.quand.label()).width(190.0).show_ui(ui, |ui| {
            for q in Quand::ALL {
                ui.selectable_value(&mut inc.quand, q, q.label());
            }
        });
        ui.end_row();
    });
    ui.label(RichText::new(format!("Ici : « {sample} »")).small().weak());
}
