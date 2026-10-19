//! Panneau Calques (6c) : chaque calque avec la vignette de son dessin à l'image courante,
//! visibilité et verrou, opacité du calque courant en haut, commandes en icônes en bas, réglages
//! du calque courant dessous.

use std::collections::HashMap;

use eframe::egui::{self, vec2, Color32, RichText, Sense, Stroke, StrokeKind, TextureHandle};

use crate::app::App;
use crate::blend::BlendMode;
use crate::document::{CelId, LayerKind};
use crate::render::over;

/// Taille des vignettes de calque.
const THUMB: [usize; 2] = [64, 36];
/// Marque des calques de toute la scène (6f).
pub(crate) const SCENE_WIDE: Color32 = Color32::from_rgb(214, 170, 92);

/// Vignettes des dessins (par dessin et version), refaites quand le dessin change.
#[derive(Default)]
pub(crate) struct LayerThumbs {
    /// Dessin → (version du dessin, vignette, dernière image d'écran où elle a servi).
    cache: HashMap<CelId, (u64, TextureHandle, u64)>,
}

impl LayerThumbs {
    /// Nombre de vignettes gardées (une texture chacune) avant de lâcher celles qui ne servent plus.
    const LIMIT: usize = 1500;

    /// Oublie toutes les vignettes (leur résolution a changé : elles se refont à leur tour).
    pub(crate) fn clear(&mut self) {
        self.cache.clear();
    }

    /// Vignettes gardées (essais).
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.cache.len()
    }
}

/// Renommage d'un calque en cours : le calque, le nom tapé, le champ a déjà pris le clavier, et
/// où est le champ (en-tête de la timeline, 13b ; colonne de gauche du montage ; sinon panneau
/// Calques).
pub(crate) struct LayerRename {
    pub id: u64,
    pub name: String,
    pub focused: bool,
    pub in_timeline: bool,
    pub in_montage: bool,
}

impl LayerRename {
    pub(crate) fn new(id: u64, name: &str, in_timeline: bool) -> Self {
        Self { id, name: name.to_string(), focused: false, in_timeline, in_montage: false }
    }
}

impl App {
    /// Vignette du dessin `cel` : la partie filmée (le cadre), sur fond blanc.
    pub(crate) fn layer_thumb(&mut self, ctx: &egui::Context, cel: CelId) -> Option<TextureHandle> {
        let c = self.doc.cels.get(&cel)?;
        let pass = ctx.cumulative_pass_nr();
        if let Some((v, t, used)) = self.layer_thumbs.cache.get_mut(&cel) {
            if *v == c.version {
                *used = pass;
                return Some(t.clone());
            }
        }
        let fr = self.doc.frame_rect();
        let [w, h] = THUMB;
        // (Résolution des vignettes du diagnostic des performances : définition plus basse, même
        // taille à l'écran.)
        let w = crate::thumbs::scaled(w, crate::thumbs::scale_pct(), 16);
        let h = ((w as f32 * fr.height() / fr.width().max(1.0)).round() as usize).clamp(8, h * 2);
        let mut pixels = Vec::with_capacity(w * h);
        // Quatre échantillons par point de la vignette.
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for (dx, dy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let sx = fr.min.x + (x as f32 + dx) / w as f32 * fr.width();
                    let sy = fr.min.y + (y as f32 + dy) / h as f32 * fr.height();
                    let (sx, sy) = (sx as usize, sy as usize);
                    let p = if sx < c.raster.width && sy < c.raster.height { c.raster.get(sx, sy) } else { Color32::TRANSPARENT };
                    let q = over(Color32::WHITE, p, 255, None);
                    r += q.r() as u32;
                    g += q.g() as u32;
                    b += q.b() as u32;
                }
                pixels.push(Color32::from_rgb((r / 4) as u8, (g / 4) as u8, (b / 4) as u8));
            }
        }
        let image = egui::ColorImage { size: [w, h], pixels };
        let tex = ctx.load_texture(format!("calque {cel}"), image, egui::TextureOptions::LINEAR);
        // Trop de vignettes : on garde celles des deux dernières images d'écran (ce qu'on voit) ;
        // tout vider refaisait, à chaque image, toutes celles qui restaient à l'écran.
        if self.layer_thumbs.cache.len() >= LayerThumbs::LIMIT {
            self.layer_thumbs.cache.retain(|_, e| pass.saturating_sub(e.2) <= 2);
            if self.layer_thumbs.cache.len() >= LayerThumbs::LIMIT {
                self.layer_thumbs.cache.clear();
            }
        }
        self.layer_thumbs.cache.insert(cel, (c.version, tex.clone(), pass));
        Some(tex)
    }

    /// Calques de toute la scène accordés d'un panneau à l'autre (6f), à chaque image ; le
    /// calque courant reste le même.
    pub(crate) fn sync_scene_layers(&mut self) {
        let id = self.layers().get(self.cur_layer).map(|l| l.id);
        if self.doc.sync_scene_layers(self.cur_panel, self.cur_frame) {
            if let Some(i) = id.and_then(|id| self.layers().iter().position(|l| l.id == id)) {
                self.cur_layer = i;
            }
            self.clamp_nav();
            self.renderer.invalidate_all();
        }
    }

    /// Nouveau calque bitmap « Fond » tout en bas (13a) : un seul dessin, tenu sur tout le plan.
    pub(crate) fn add_scene_background(&mut self) {
        if self.content_refuses() {
            return;
        }
        self.end_stroke();
        self.drop_bitmap_selection();
        let snap = self.snapshot();
        let n = self.frame_count();
        let cel = self.doc.new_cel(LayerKind::Bitmap);
        let mut fond = crate::document::Layer::new("Fond", LayerKind::Bitmap);
        fond.frames = vec![Some(cel); n];
        self.layers_mut().insert(0, fond);
        self.cur_layer = 0;
        self.vsel = None;
        self.commit_structure(snap);
        self.status = "Calque « Fond » : un seul dessin, tenu sur tout le plan.".into();
    }
    pub(crate) fn layers_panel(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Opacité du calque courant (comme en haut d'une liste de calques).
        let li = self.cur_layer;
        let mut opacity = self.layers()[li].opacity * 100.0;
        let mut mode = self.layers()[li].blend;
        // (À la ligne si le panneau est étroit ; la liste se resserre.)
        let combo_w = (ui.available_width() - 90.0).clamp(60.0, 120.0);
        ui.horizontal_wrapped(|ui| {
            // Mode de fusion (8), comme dans les logiciels de retouche.
            let resp = egui::ComboBox::from_id_salt("fusion_calque")
                .selected_text(mode.label())
                .width(combo_w)
                .show_ui(ui, |ui| {
                    for m in BlendMode::ALL {
                        // Familles : normal, assombrir, éclaircir, contraste, écarts, couleur.
                        if BlendMode::FAMILY_STARTS.contains(&m) {
                            ui.separator();
                        }
                        ui.selectable_value(&mut mode, m, m.label()).on_hover_text(m.hint());
                    }
                })
                .response;
            crate::bulles::bulle(resp, "Mode de fusion", mode.hint(), "");
            crate::bulles::etiquette(ui, ph::CIRCLE_HALF, "Opacité du calque", "");
            let resp = ui.add(egui::DragValue::new(&mut opacity).range(0.0..=100.0).max_decimals(0).suffix(" %").speed(0.5));
            let resp = crate::bulles::bulle(resp, "Opacité du calque", "Glisser ou taper une valeur.", "");
            if self.edited(&resp) {
                self.layers_mut()[li].opacity = opacity / 100.0;
            }
        });
        if mode != self.layers()[li].blend {
            self.record_step();
            self.layers_mut()[li].blend = mode;
            self.renderer.invalidate_all();
        }
        ui.add_space(2.0);
        // Les commandes, en haut et toujours visibles (Anthony, 2026-10-17 : « la zone [des
        // boutons] doit rester en haut ») ; seule la liste défile.
        self.layer_commands(ui);
        ui.add_space(2.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("liste_calques")
            .auto_shrink([false, false])
            .show(ui, |ui| self.layer_list(ui));
    }

    /// Les commandes des calques en icônes (nouveau, dupliquer, supprimer, monter, copier…).
    fn layer_commands(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
            if self.bouton(ui, ph::BEZIER_CURVE, "Nouveau calque vectoriel", "Des traits qui restent modifiables.", "calque.nouveau_vectoriel").clicked() {
                self.add_layer(LayerKind::Vector);
            }
            if self.bouton(ui, ph::PAINT_BRUSH_BROAD, "Nouveau calque bitmap", "De la peinture en pixels, pinceaux texturés.", "calque.nouveau_bitmap").clicked() {
                self.add_layer(LayerKind::Bitmap);
            }
            if self
                .bouton(ui, ph::MOUNTAINS, "Nouveau calque de fond", "Tout en bas, un seul dessin tenu sur tout le plan (décor).", "calque.nouveau_fond")
                .clicked()
            {
                self.add_scene_background();
            }
            if self
                .bouton(ui, ph::COPY_SIMPLE, "Dupliquer les calques choisis", "Une copie de chaque calque choisi et de ses dessins, juste au-dessus de lui.", "calque.dupliquer")
                .clicked()
            {
                self.duplicate_chosen_layers();
            }
            if self.bouton(ui, ph::TRASH, "Supprimer les calques choisis", "Le calque courant et ceux choisis par Ctrl + clic dans la timeline.", "calque.supprimer").clicked() {
                self.delete_chosen_layers();
            }
            if self.bouton(ui, ph::ARROW_UP, "Monter le calque", "D'un rang vers le haut.", "calque.monter").clicked() {
                self.move_layer(true);
            }
            if self.bouton(ui, ph::ARROW_DOWN, "Descendre le calque", "D'un rang vers le bas.", "calque.descendre").clicked() {
                self.move_layer(false);
            }
            if self.bouton(ui, ph::COPY, "Copier le calque", "", "calque.copier").clicked() {
                self.copy_layer();
            }
            let can_paste = self.layer_clipboard.is_some();
            if self.bouton_si(ui, can_paste, ph::CLIPBOARD, "Coller le calque", "Le calque copié, au-dessus du calque courant.", "calque.coller").clicked() {
                self.paste_layer();
            }
            // Plusieurs calques choisis (Ctrl ou Maj + clic) : ils fusionnent ; sinon, avec celui
            // du dessous.
            let n = self.chosen_layers().len();
            if n > 1 {
                let what = format!("Fusionner les {n} calques choisis");
                if self.bouton(ui, ph::ARROWS_IN_SIMPLE, &what, "Ils n'en font plus qu'un (Ctrl ou Maj + clic pour en choisir plusieurs).", "calque.fusionner_choisis").clicked() {
                    self.merge_chosen_layers();
                }
            } else {
                let can_merge = self.cur_layer > 0;
                if self
                    .bouton_si(
                        ui,
                        can_merge,
                        ph::ARROWS_IN_SIMPLE,
                        "Fusionner avec le calque du dessous",
                        "Les deux calques n'en font plus qu'un. Ctrl ou Maj + clic sur des calques pour en fusionner plusieurs.",
                        "calque.fusionner_dessous",
                    )
                    .clicked()
                {
                    self.merge_layer_down();
                }
            }
        });
    }

    /// La liste : un rang par calque, le plus haut en premier. Clic sur le rang : il devient
    /// courant ; glisser : il monte ou descend ; clic droit : tout ce qui touche au calque
    /// (`layer_full_menu`) ; double-clic sur le nom : renommer.
    fn layer_list(&mut self, ui: &mut egui::Ui) {
        use egui_phosphor::regular as ph;
        // Le calque cliqué, et comment : Ctrl l'ajoute ou le retire du choix, Maj choisit ceux
        // d'entre (pour fusionner les calques choisis, Anthony, 2026-10-17) ; clic droit.
        let mut select: Option<(usize, bool)> = None;
        let mods = ui.input(|i| i.modifiers);
        let mut toggle = None;
        let mut lock = None;
        let mut rename_done: Option<bool> = None;
        // Bascule Trait / Fond cliquée sur la ligne du calque courant.
        let mut strate = false;
        let lock_key = self.raccourci("calque.verrouiller");
        let frame = self.cur_frame;
        let accent = ui.visuals().selection.bg_fill;
        // Vignette plus petite dans un panneau étroit : le nom reste lisible.
        let thumb_w = (ui.available_width() * 0.3).clamp(32.0, THUMB[0] as f32);
        let thumb_size = vec2(thumb_w, thumb_w * THUMB[1] as f32 / THUMB[0] as f32);
        // Glisser une ligne la monte ou la descend (passe interface) : les lignes de cette image,
        // pour savoir où elle se pose.
        let list_top = ui.cursor().top();
        let mut drag_rows: Vec<crate::layerheads::Row> = Vec::new();
        for li in (0..self.layers().len()).rev() {
            let (id, visible, locked, kind, name, cel, folder, ctg, has_fond) = {
                let l = &self.layers()[li];
                (l.id, l.visible, l.locked, l.kind, l.name.clone(), l.cel_at(frame), l.folder, l.ctg.is_some(), l.fond.is_some())
            };
            // Code couleur de sa sorte (bitmap vert, vectoriel bleu, CTG rose ; Anthony, 2026-10-17).
            let hue = if folder { accent } else { crate::theme::couleur_calque(kind, ctg) };
            let current = li == self.cur_layer;
            let chosen = !current && self.is_chosen_layer(li);
            let thumb = cel.and_then(|c| self.layer_thumb(ui.ctx(), c));
            // Le rang entier, posé avant ses boutons (à sa place de l'image d'écran d'avant) : les
            // boutons restent au-dessus et gardent leurs clics (posé après eux, ce fond qui se
            // glisse les rendait tous muets : œil, verrou, clic droit, 2026-10-17).
            let grip_id = ui.id().with(("glisser_calque", id));
            let grip = ui.ctx().data(|d| d.get_temp::<egui::Rect>(grip_id)).map(|r| ui.interact(r, grip_id, Sense::click_and_drag()));
            let shown = egui::Frame::new()
                .fill(match (current, chosen) {
                    (true, _) => hue.gamma_multiply(0.55),
                    (false, true) => hue.gamma_multiply(0.25),
                    (false, false) => Color32::TRANSPARENT,
                })
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(3, 2))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
                        let eye = if visible { ph::EYE } else { ph::EYE_SLASH };
                        let resp = ui.add(egui::Button::new(RichText::new(eye).size(15.0)).frame(false).min_size(vec2(20.0, 36.0)));
                        let resp = crate::bulles::bulle(resp, "Afficher / masquer", "Montrer ou cacher le calque.", "");
                        if resp.clicked() {
                            toggle = Some(li);
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        let pad = if locked { ph::LOCK_SIMPLE } else { ph::LOCK_SIMPLE_OPEN };
                        let pad = RichText::new(pad).size(14.0);
                        let pad = if locked { pad } else { pad.weak() };
                        let resp = ui.add(egui::Button::new(pad).frame(false).min_size(vec2(18.0, 36.0)));
                        let resp = crate::bulles::bulle(resp, "Verrouiller / déverrouiller", "Un calque verrouillé ne peut plus être dessiné.", &lock_key);
                        if resp.clicked() {
                            lock = Some(li);
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        // Vignette du dessin à l'image courante (vide : cadre gris).
                        let (rect, resp) = ui.allocate_exact_size(thumb_size, Sense::click());
                        match &thumb {
                            Some(t) => {
                                ui.painter().image(
                                    t.id(),
                                    rect,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    if visible { Color32::WHITE } else { Color32::from_gray(120) },
                                );
                            }
                            None => {
                                ui.painter().rect_filled(rect, 0.0, crate::theme::controle());
                            }
                        }
                        let border = if current { Stroke::new(2.0_f32, crate::theme::contour()) } else { Stroke::new(1.0_f32, crate::theme::bord()) };
                        ui.painter().rect_stroke(rect, 0.0, border, StrokeKind::Inside);
                        if resp.clicked() || resp.secondary_clicked() {
                            select = Some((li, resp.secondary_clicked()));
                        }
                        resp.context_menu(|ui| self.layer_full_menu(ui, li, false));
                        ui.vertical(|ui| {
                            let renaming = self.layer_rename.as_ref().is_some_and(|r| r.id == id && !r.in_timeline && !r.in_montage);
                            if renaming {
                                rename_done = self.layer_rename_field(ui, 120.0);
                            } else {
                                let text = if visible { RichText::new(&name) } else { RichText::new(&name).weak() };
                                let resp = ui.add(egui::Label::new(text).sense(Sense::click()).truncate());
                                if resp.clicked() || resp.secondary_clicked() {
                                    select = Some((li, resp.secondary_clicked()));
                                }
                                if resp.double_clicked() {
                                    self.layer_rename = Some(LayerRename::new(id, &name, false));
                                }
                                resp.on_hover_text("Double-clic : renommer ; clic droit : tout ce qui touche au calque").context_menu(|ui| self.layer_full_menu(ui, li, false));
                            }
                            let kind = match kind {
                                _ if folder => "dossier",
                                _ if ctg => "CTG",
                                LayerKind::Vector => "vectoriel",
                                LayerKind::Bitmap => "bitmap",
                            };
                            let label = RichText::new(kind).small();
                            ui.label(if folder || current { label.weak() } else { label.color(hue) });
                            // Fond sous le trait (`fondui.rs`) : sur le calque courant, où peint le
                            // pinceau (le trait ou le fond) ; sur les autres, qu'il en a un.
                            if has_fond && current {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 2.0;
                                    let on = self.paint_fond;
                                    let t = ui.add(egui::SelectableLabel::new(!on, RichText::new("Trait").small()));
                                    let f = ui.add(egui::SelectableLabel::new(on, RichText::new("Fond").small()));
                                    let f = f.on_hover_text("Le pinceau, le crayon et la gomme peignent dans le fond, sous le trait.");
                                    if (t.clicked() && on) || (f.clicked() && !on) {
                                        strate = true;
                                    }
                                });
                            } else if has_fond {
                                ui.label(RichText::new("+ fond").small().weak()).on_hover_text("Ce calque a un fond sous son trait.");
                            }
                        });
                    });
                });
            let rect = shown.response.rect;
            if !folder {
                // La bande de sa sorte, au bord gauche du rang.
                let band = egui::Rect::from_min_size(rect.left_top() + vec2(0.0, 2.0), vec2(3.0, rect.height() - 4.0));
                ui.painter().rect_filled(band, 1.0, hue);
            }
            ui.ctx().data_mut(|d| d.insert_temp(grip_id, rect));
            drag_rows.push(crate::layerheads::Row { li, top: rect.top() - list_top, h: rect.height() });
            if let Some(grip) = grip {
                if grip.drag_started() && self.layer_rename.is_none() {
                    self.panel_layer_drag = Some(li);
                }
                if grip.clicked() || grip.secondary_clicked() {
                    select = Some((li, grip.secondary_clicked()));
                }
                grip.context_menu(|ui| self.layer_full_menu(ui, li, false));
            }
        }
        if let Some(from) = self.panel_layer_drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let pointer = ui.input(|i| i.pointer.interact_pos());
            if let Some(p) = pointer {
                let slot = drag_rows.iter().filter(|r| r.top + r.h * 0.5 < p.y - list_top).count();
                let y = list_top + drag_rows.get(slot).map_or_else(|| drag_rows.last().map_or(0.0, |r| r.top + r.h), |r| r.top);
                let x = ui.max_rect().x_range();
                ui.painter().line_segment([egui::pos2(x.min, y), egui::pos2(x.max, y)], Stroke::new(2.0_f32, crate::theme::contour()));
            }
            if ui.input(|i| !i.pointer.any_down()) {
                self.panel_layer_drag = None;
                if let Some(p) = pointer {
                    let to = crate::layerheads::drop_index(&drag_rows, from, p.y - list_top);
                    if to != from && !self.content_refuses() {
                        self.move_layer_to(from, to);
                    }
                }
            }
        }
        if let Some(li) = toggle {
            self.record_step();
            let layer = &mut self.layers_mut()[li];
            layer.visible = !layer.visible;
        }
        if let Some(li) = lock {
            self.toggle_layer_lock(li);
        }
        if let Some((li, secondary)) = select {
            // Clic droit sur un calque déjà choisi : le choix reste (le menu vise les choisis).
            if !(secondary && self.is_chosen_layer(li)) {
                self.click_layer_head(li, mods.command && !secondary, mods.shift && !secondary);
            }
        }
        self.finish_layer_rename(rename_done);
        if strate {
            self.run_action("pinceau.strate_fond");
        }

        // Clic droit dans le vide sous la liste : de nouveaux calques.
        let rest = ui.available_rect_before_wrap();
        if rest.height() > 4.0 {
            let resp = ui.interact(rest, ui.id().with("vide_calques"), Sense::click());
            resp.context_menu(|ui| {
                self.action_item(ui, "Nouveau calque vectoriel", "calque.nouveau_vectoriel");
                self.action_item(ui, "Nouveau calque bitmap", "calque.nouveau_bitmap");
                self.action_item(ui, "Nouveau calque de fond", "calque.nouveau_fond");
                ui.separator();
                self.action_item(ui, "Coller le calque", "calque.coller");
            });
        }
    }
    /// Champ du nouveau nom du calque en cours de renommage ; renvoie `Some(true)` quand on le
    /// valide (Entrée, clic ailleurs), `Some(false)` quand on abandonne (Échap).
    pub(crate) fn layer_rename_field(&mut self, ui: &mut egui::Ui, width: f32) -> Option<bool> {
        let r = self.layer_rename.as_mut()?;
        let mut out = egui::TextEdit::singleline(&mut r.name).desired_width(width).show(ui);
        let resp = out.response.clone();
        if !r.focused {
            // Tout le nom choisi : on tape le nouveau directement.
            resp.request_focus();
            let n = r.name.chars().count();
            out.state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n),
            )));
            out.state.store(ui.ctx(), resp.id);
            r.focused = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            Some(false)
        } else if resp.lost_focus() {
            Some(true)
        } else {
            None
        }
    }

    /// Fin du renommage : `Some(true)` pose le nom, `Some(false)` l'abandonne.
    pub(crate) fn finish_layer_rename(&mut self, done: Option<bool>) {
        match done {
            Some(true) => self.commit_layer_rename(),
            Some(false) => self.layer_rename = None,
            None => {}
        }
    }

    /// Pose le nouveau nom du calque en cours de renommage (une étape d'annulation).
    fn commit_layer_rename(&mut self) {
        let Some(LayerRename { id, name, .. }) = self.layer_rename.take() else { return };
        let name = name.trim().to_string();
        let Some(li) = self.layers().iter().position(|l| l.id == id) else { return };
        if !name.is_empty() && self.layers()[li].name != name {
            self.record_step();
            self.layers_mut()[li].name = name;
        }
    }

    /// Menu d'un calque (clic droit) : renommer, profondeur, dupliquer,
    /// verrouiller, supprimer. `in_timeline` : ouvert depuis l'en-tête de la timeline (13b),
    /// où se fait alors le renommage.
    pub(crate) fn layer_menu(&mut self, ui: &mut egui::Ui, li: usize, in_timeline: bool) {
        self.layer_menu_where(ui, li, in_timeline, false);
    }

    /// `layer_menu`, ouvert depuis la colonne de gauche du montage si `in_montage` (le nom s'y
    /// renomme). Ni timeline ni montage : le panneau Calques, où ses boutons et son verrou font
    /// dupliquer, verrouiller et supprimer (seulement renommer et la profondeur).
    pub(crate) fn layer_menu_where(&mut self, ui: &mut egui::Ui, li: usize, in_timeline: bool, in_montage: bool) {
        if self.layers().get(li).is_none() {
            ui.close_menu();
            return;
        }
        self.rename_item(ui, li, in_timeline, in_montage);
        self.depth_item(ui, li);
        // Dans le panneau Calques, ses boutons et son verrou font le reste (pas de doublon).
        if !in_timeline && !in_montage {
            return;
        }
        ui.separator();
        self.duplicate_lock_delete_items(ui, li);
    }

    /// « Renommer… » (le champ vient dans la timeline, la colonne du montage ou le panneau).
    pub(crate) fn rename_item(&mut self, ui: &mut egui::Ui, li: usize, in_timeline: bool, in_montage: bool) {
        let Some((id, name)) = self.layers().get(li).map(|l| (l.id, l.name.clone())) else { return };
        if ui.button("Renommer…").clicked() {
            ui.close_menu();
            self.layer_rename = Some(LayerRename { in_montage, ..LayerRename::new(id, &name, in_timeline) });
        }
    }

    /// Profondeur du calque (multiplan).
    pub(crate) fn depth_item(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some((mut depth, fixed)) = self.layers().get(li).map(|l| (l.depth, l.position_locked)) else { return };
        let resp = ui
            .horizontal(|ui| {
                ui.label("Profondeur");
                // Position verrouillée (13b) : la profondeur ne bouge plus non plus.
                ui.add_enabled(
                    !fixed,
                    egui::DragValue::new(&mut depth).speed(0.01).range(crate::document::Layer::MIN_DEPTH..=1000.0).max_decimals(2),
                )
            })
            .inner
            .on_hover_text(
                "Distance du calque quand la caméra se déplace (multiplan) :\n\
                 0 = plan de référence, défile avec la caméra ;\n\
                 1 = deux fois plus loin, défile deux fois moins vite ;\n\
                 négatif = premier plan, défile plus vite.",
            );
        if self.edited(&resp) {
            self.layers_mut()[li].depth = depth;
        }
    }

    /// Dupliquer, verrouiller (ou déverrouiller), supprimer le calque `li`.
    pub(crate) fn duplicate_lock_delete_items(&mut self, ui: &mut egui::Ui, li: usize) {
        let Some(locked) = self.layers().get(li).map(|l| l.locked) else { return };
        if ui.button("Dupliquer").clicked() {
            ui.close_menu();
            self.select_layer(li);
            self.duplicate_layer();
        }
        if ui.button(if locked { "Déverrouiller" } else { "Verrouiller" }).clicked() {
            ui.close_menu();
            self.toggle_layer_lock(li);
        }
        if ui.button("Supprimer").clicked() {
            ui.close_menu();
            // Ce calque seul, même si d'autres sont choisis.
            self.layer_sel.clear();
            self.select_layer(li);
            self.delete_chosen_layers();
        }
    }
}
