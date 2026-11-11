use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Event, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    TouchPhase,
};

use crate::app::{App, Tool};
use crate::pixtools::BDrag;
use crate::vtools::VDrag;

const ACCENT: Color32 = crate::theme::SELECTION_BOITE;

impl App {
    pub(crate) fn canvas_panel(&mut self, ui: &mut egui::Ui) {
        let mut rect = ui.available_rect_before_wrap();
        // Barre de la vue (plateau / caméra, cadres de composition), sauf dans une vignette.
        if !self.in_card {
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), crate::viewbar::VIEW_BAR_H));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).id_salt("barre_vue"));
            self.view_bar(&mut child);
            rect.min.y = bar.max.y;
            // (Les pastilles des personnages sont à l'extrémité droite de cette barre, `view_bar`.)
            // Dessous, les commandes de la vue (en 3D : seulement les vues, pour en sortir).
            let tools = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - crate::viewbar::TOOLS_BAR_H), rect.max);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tools).id_salt("outils_vue"));
            self.view_tools_bar(&mut child);
            rect.max.y = tools.min.y;
            // Espace 3D : à la place du dessin à plat, avec ses commandes dans les Réglages.
            if self.espace3d.on && !self.camera_view {
                let response = ui.allocate_rect(rect, Sense::click_and_drag());
                self.canvas_rect = rect;
                self.espace3d_panel(ui, rect, &response);
                return;
            }
        }
        // Tout le film est annulé : un état vide clair, rien à dessiner (`planannule.rs`).
        if !self.in_card && self.doc.film_vide() {
            self.etat_film_vide(ui, rect);
            return;
        }
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        // Un clic dans le dessin reprend la main au montage : l'édition vise de nouveau le dessin.
        if response.is_pointer_button_down_on() {
            self.panel_focus = false;
            self.images_focus = false;
            self.key_focus = false;
            // L'outil pris par une touche tenue sert : au relâché, l'outil d'avant revient.
            self.spring_used();
        }
        self.canvas_rect = rect;
        let size = self.canvas_size();
        // En vue caméra, la vue suit la caméra du plan (avant de lire la souris).
        self.apply_camera_view(rect);
        // Une grande case de mouvement de caméra (Vignettes) : la vue montre la zone des cadres, la feuille et ce qui la
        // déborde, à l'échelle du cadre A (`vignettecam::Zone`).
        let zone = if self.in_card { self.vignettes.zone } else { None };
        if self.fit_pending && !self.camera_view && zone.is_none() {
            // On cadre la caméra (centrée sur la feuille), avec un peu de hors-champ autour.
            let [fw, fh] = self.doc.frame;
            let margin = if [fw, fh] == [self.doc.width, self.doc.height] { 1.0 } else { 1.12 };
            self.view.fit(rect, vec2(fw as f32, fh as f32) * margin);
            self.fit_pending = false;
        }
        if let Some(z) = zone {
            self.view = z.vue(rect, size);
        }

        // La boîte de la sélection est à jour avant d'agir (`seltools.rs`).
        self.sync_sel_box();
        self.canvas_input(ui.ctx(), &response, rect);
        // Clic droit sur le dessin (passe interface) : les réglages de l'outil sous le curseur, avec tous les outils sauf la
        // Sélection et la Pipette, qui gardent le menu des commandes courantes (`reglagesdroit.rs`). Pas dans une
        // vignette de la planche, qui a son propre menu.
        if !self.in_card && !self.modal_open() {
            // L'anneau sous le stylet (`anneauui.rs`) : il lit lui-même le bouton du stylet sur ce dessin. Son clic bref fait ce
            // que le clic droit faisait (les réglages) ; glisser lance une commande, maintenir le montre.
            let anneau = self.anneau_en_service();
            if anneau {
                self.anneau_noter_le_dessin(&response, rect, true);
            }
            if self.reglages_droit_permis() {
                if !anneau && response.secondary_clicked() {
                    self.clic_droit_dans_le_dessin(response.interact_pointer_pos());
                }
            } else {
                response.context_menu(|ui| self.canvas_menu(ui));
            }
        } else if self.in_card && !self.modal_open() {
            // Dans la vue Vignettes, le clic droit sur l'image courante ouvre le menu de la vignette (`boardui.rs`).
            response.context_menu(|ui| self.card_context_menu(ui));
        }
        self.update_text_preview(ui.ctx());
        self.check_persp();
        self.check_envelope();
        self.check_deform_multi();

        // Garde : une feuille plus grande que la texture maximale de la carte graphique (ou de l'affichage de
        // secours) ne se compose pas ; au lieu d'une vue blanche en silence, on le dit sur le dessin et dans la barre d'état.
        let limit = self.gpu.as_ref().map_or(crate::document::MAX_SHEET_SIDE as u32, |g| g.max_side()) as usize;
        if self.doc.width > limit || self.doc.height > limit {
            let msg = format!("La feuille ({}×{} px) dépasse la taille maximale de l'affichage ({limit} px de côté) : elle ne peut pas être montrée. Réduire la feuille ou le projet.", self.doc.width, self.doc.height);
            self.status = msg.clone();
            ui.painter_at(rect).rect_filled(rect, 0.0, Color32::from_gray(30));
            ui.painter_at(rect).text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(15.0), crate::theme::texte_fort());
            return;
        }
        let plan = self.build_plan(self.cur_frame, true, true);
        // Tremblement du trait animé à l'arrêt (`tremblement.rs`) : seul le prochain changement de
        // variante réveille l'écran (12 fois par seconde par défaut), jamais à la cadence de l'écran.
        if !self.playing {
            if let Some(wait) = crate::tremblement::rest_wake(&plan.items, self.doc.fps, crate::fx::now_ms()) {
                ui.ctx().request_repaint_after(wait);
            }
        }
        // Effets (13i) : jamais calculés ici. En lecture, au défilement et pendant un geste sur un
        // effet, l'aperçu à la taille de l'écran ; au repos, la pleine résolution, calculée par le
        // fil de fond (`fxcache.rs`) ; l'écran montre le dernier résultat en attendant.
        let level = match &self.gpu {
            Some(_) => crate::fx::draft_level(ui.ctx(), self.cur_panel, self.cur_frame, self.playing, self.view.zoom * ui.ctx().pixels_per_point()),
            None => 0,
        };
        // Calques d'effets (`calqueffet.rs`) : le repère de chacun et ce qu'il couvre deviennent le
        // dessin dérivé, demandé au fil de fond et montré quand il est prêt (sans l'effet avant).
        // Sur la carte graphique (`gpufx.rs`), les calques d'ajustement hors dossier restent des repères :
        // la carte passe leurs effets sur l'image composée, à pleine résolution, à chaque image.
        let gpu_fx = self.gpu.as_ref().is_some_and(|g| g.effects_on());
        let plan = if gpu_fx && crate::gpufx::adjustments_supported(&plan) {
            plan
        } else {
            match crate::calqueffet::resolve(&self.doc, &plan, crate::calqueffet::Mode::Display { level, prio: 0 }) {
                std::borrow::Cow::Borrowed(_) => plan,
                std::borrow::Cow::Owned(resolved) => resolved,
            }
        };
        let painter = ui.painter_at(rect);
        let stage_corners = [pos2(0.0, 0.0), pos2(size.x, 0.0), pos2(size.x, size.y), pos2(0.0, size.y)];
        let corners_h = stage_corners.map(|p| self.view.to_screen_h(rect, size, p));
        let corners = corners_h.map(|c| c.0);
        // Vue en perspective (caméra tournée, 9f) : w des coins pour la carte graphique.
        let ws = self.view.persp.is_some().then(|| corners_h.map(|c| c.1));
        // Toile infinie : la feuille n'est plus une page cernée d'une ombre, le papier et l'arrière-plan
        // couvrent tout ce que la vue regarde, et ce qui est dessiné hors de la feuille s'y voit
        // (`monde.rs`). Une vignette ou une vue en perspective restent à la feuille.
        // (Une case vivante des Vignettes voit à travers la caméra de son image : après un travelling, le cadre B peut sortir de la feuille, et
        // ce qui est dessiné au-delà — ou le papier — doit s'y voir, comme dans la vignette. Sans cela, la case coupait net au bord de la feuille,
        // sa moitié au-delà restait blanche. Seule la case vivante compose le monde : une seule à la fois, rien de plus par vignette.)
        let world = ((!self.in_card || zone.is_some() || self.camera_view) && ws.is_none()).then(|| self.visible_world(rect, size));
        // Arrière-plan (13e) : aucun → noir ; damier → cases des couleurs A et B sous les calques.
        self.paint_background(ui.ctx(), &painter, rect, size, world, plan.paper);
        if let Some(view) = world {
            self.paint_monde(ui.ctx(), &painter, rect, size, &plan, view);
        }
        // Dessins des images qui viennent, à envoyer d'avance à la carte graphique (leurs effets
        // sont demandés au fil de fond en même temps).
        let ahead: Vec<crate::gpu::Ahead> = match &self.gpu {
            Some(gpu) => {
                let (at, cap) = (self.play_position(), gpu.ahead_capacity(&self.doc, plan.items.len()));
                let mut ahead = crate::playback::upcoming_derived_with(&self.doc, at, level, cap, gpu_fx);
                ahead.extend(
                    crate::playback::upcoming_cels_with(&self.doc, at, &plan, cap, gpu_fx).into_iter().map(|id| crate::gpu::Ahead { id, cel: None }),
                );
                // Les autres variantes d'un tremblement animé à l'arrêt : demandées au fil de fond
                // et envoyées d'avance, pour que l'écran passe de l'une à l'autre sans attendre.
                if !self.playing {
                    // (Sur la carte graphique, le tremblement se redessine à chaque image : rien à demander.)
                    let others: Vec<_> = crate::tremblement::other_variants(&plan.items).into_iter().filter(|i| !(gpu_fx && crate::gpu::gpu_fx_item(&self.doc, i))).collect();
                    crate::fx::prefetch(&self.doc, &others, 1, level);
                    for item in &others {
                        if let Some((id, cel)) = crate::fx::ready(&self.doc, item, level) {
                            if !ahead.iter().any(|a| a.id == id) {
                                ahead.push(crate::gpu::Ahead { id, cel: Some(cel) });
                            }
                        }
                    }
                }
                ahead
            }
            None => Vec::new(),
        };
        // Une transition du montage sous la tête de lecture (fondu, volet, noir) : les plans à
        // superposer, composés par la carte graphique comme le plan courant (`transitionsui.rs`).
        let mixed = if self.gpu.is_some() { self.transition_preview() } else { None };
        let setup = mixed.map(|(_, mix)| self.mix_setup(&mix, &plan, rect, size));
        if let Some(gpu) = &mut self.gpu {
            // Les calques sont superposés par la carte graphique, directement à leur place.
            let (all, tiles) = self.renderer.take_gpu_dirty();
            let stage = [self.doc.width, self.doc.height];
            match &setup {
                None => {
                    // (Le fond sous le trait se calcule en arrière-plan : `fx::differe`.)
                    let resolved = {
                        let _t = crate::perf::section("composition : envoi à la carte graphique");
                        crate::fx::differe(|| gpu.sync(&self.doc, &plan, all, &tiles, &ahead, level))
                    };
                    painter.add(gpu.paint(&plan, &resolved, corners, ws, rect, stage));
                }
                Some(s) => {
                    // Les dessins des deux plans partent ensemble à la carte (un seul `sync` : les
                    // textures de l'un ne chassent pas celles de l'autre).
                    let mut both = s.first.clone();
                    if let Some((second, _, _)) = &s.second {
                        both.items.extend(second.items.iter().cloned());
                    }
                    let resolved = {
                        let _t = crate::perf::section("composition : envoi à la carte graphique (transition)");
                        crate::fx::differe(|| gpu.sync(&self.doc, &both, all, &tiles, &ahead, level))
                    };
                    let n = s.first.items.len().min(resolved.len());
                    let first = crate::gpu::MixLayer { plan: &s.first, resolved: &resolved[..n], corners: s.first_corners, ws: s.first_ws };
                    let second = s.second.as_ref().map(|(p, c, w)| crate::gpu::MixLayer { plan: p, resolved: &resolved[n..], corners: *c, ws: *w });
                    painter.add(gpu.paint_mix(first, second, Some(&s.display), rect, stage));
                }
            }
        } else {
            let _t = crate::perf::section("composition : processeur");
            let texture = crate::fx::differe(|| self.renderer.update(ui.ctx(), &self.doc, plan));
            let mut mesh = egui::Mesh::with_texture(texture);
            // En perspective, egui plaque la texture par triangle (sans perspective) : on
            // subdivise, et on laisse ce qui passe derrière la caméra.
            let n = if ws.is_some() { 24 } else { 1 };
            let mut front = Vec::with_capacity((n + 1) * (n + 1));
            for j in 0..=n {
                for i in 0..=n {
                    let uv = pos2(i as f32 / n as f32, j as f32 / n as f32);
                    let (pos, w) = self.view.to_screen_h(rect, size, pos2(uv.x * size.x, uv.y * size.y));
                    front.push(w > 0.0);
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: Color32::WHITE });
                }
            }
            for j in 0..n {
                for i in 0..n {
                    let a = j * (n + 1) + i;
                    let (b, c, d) = (a + 1, a + n + 1, a + n + 2);
                    if front[a] && front[b] && front[c] && front[d] {
                        mesh.add_triangle(a as u32, b as u32, d as u32);
                        mesh.add_triangle(a as u32, d as u32, c as u32);
                    }
                }
            }
            painter.add(Shape::mesh(mesh));
        }

        // (La grande case d'un mouvement de caméra a ses propres cadres, A, B, C… : `vignettecamui.rs`.)
        if zone.is_none() {
            self.paint_camera(&painter, rect);
            // Incrustations (timecode…) : sur l'image finale, dans le cadre (`prereglagesui.rs`).
            self.paint_incrustations(ui.ctx(), &painter);
        }
        // Le dessin d'un relecteur, quand son commentaire est choisi (`relectureui.rs`).
        self.paint_review_drawing(ui.ctx(), &painter);
        // Notes dessinées par-dessus l'image pendant la lecture (13e).
        self.paint_notes_overlay(&painter, rect);
        if !self.in_card {
            self.paint_frame_overlays(&painter, rect);
        }
        self.paint_layer_transform(&painter);
        self.paint_trajectoire(&painter);
        let shift = ui.input(|i| i.modifiers.shift);
        self.paint_guides(&painter, rect, self.hover_canvas);
        self.paint_vector_overlay(&painter, rect, shift);
        self.paint_text_overlay(&painter);
        self.paint_persp_overlay(&painter);
        self.paint_envelope_overlay(&painter);
        self.paint_deform_multi_overlay(&painter);
        // La gomme sur beaucoup de dessins : l'alerte, pendant le geste (`gommemulti.rs`).
        self.paint_gomme_alerte(&painter, rect);
        self.paint_stroke_tip(&painter);
        self.paint_droit(&painter);
        if !self.in_card {
            self.paint_canvas_title(&painter, rect);
        }
        self.paint_cursor(ui.ctx(), &painter, &response);
        self.text_edit_window(ui.ctx());
        // (Les pastilles des personnages ne sont plus sur le dessin : elles sont à droite de la barre du haut, `view_bar`.)
    }

    /// La zone du monde (pixels de la toile infinie) que la vue regarde : le rectangle d'écran `rect`
    /// ramené dans le monde, rotation et miroir compris, à un pixel près.
    pub(crate) fn visible_world(&self, rect: Rect, size: egui::Vec2) -> crate::render::Area {
        let pts = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()].map(|p| self.view.to_canvas(rect, size, p));
        let lo = pts.iter().fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
        let hi = pts.iter().fold((f32::MIN, f32::MIN), |(x, y), p| (x.max(p.x), y.max(p.y)));
        if ![lo.0, lo.1, hi.0, hi.1].iter().all(|v| v.is_finite()) {
            return crate::render::Area::sheet(&self.doc);
        }
        crate::render::Area::around((lo.0 - 1.0, lo.1 - 1.0), (hi.0 + 1.0, hi.1 + 1.0))
    }

    /// Pose sous la feuille ce que le plan montre hors d'elle (`monde.rs`).
    fn paint_monde(&mut self, ctx: &egui::Context, painter: &egui::Painter, rect: Rect, size: egui::Vec2, plan: &crate::render::Plan, view: crate::render::Area) {
        let (all, ids) = self.renderer.take_world_dirty();
        let _t = crate::perf::section("composition : hors de la feuille");
        let Some(shown) = self.monde.update(ctx, &self.doc, plan, view, all, &ids) else { return };
        let a = shown.area;
        let (x0, y0, x1, y1) = (a.x0 as f32, a.y0 as f32, a.x1 as f32, a.y1 as f32);
        let mut mesh = egui::Mesh::with_texture(shown.id);
        for (p, uv) in [pos2(x0, y0), pos2(x1, y0), pos2(x1, y1), pos2(x0, y1)].into_iter().zip([pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)]) {
            mesh.vertices.push(egui::epaint::Vertex { pos: self.view.to_screen(rect, size, p), uv, color: Color32::WHITE });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(mesh));
    }

    /// Scène et panneau en cours, en haut à droite du dessin.
    fn paint_canvas_title(&self, painter: &egui::Painter, rect: Rect) {
        let text = crate::planversui::titre_long_avec_version(&self.doc, self.cur_panel);
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(17.0), Color32::from_gray(235));
        let at = egui::pos2(rect.right() - galley.size().x - 14.0, rect.top() + 8.0);
        let bg = Rect::from_min_size(at - vec2(8.0, 4.0), galley.size() + vec2(16.0, 8.0));
        painter.rect_filled(bg, 4.0, Color32::from_black_alpha(150));
        painter.galley(at, galley, Color32::from_gray(235));
    }

    /// Pendant un trait : relie le dernier point posé à la pointe du stylet, pour que le trait
    /// reste attaché à la croix (le lissage le fait sinon traîner un peu derrière). Rien n'est
    /// écrit dans le dessin : le trait définitif arrive dans les images suivantes.
    fn paint_stroke_tip(&self, painter: &egui::Painter) {
        let Some(s) = &self.stroke else { return };
        let (Some((last, raw, r)), Some((rgb, alpha))) = (s.tip(), s.tip_paint()) else { return };
        let (a, b) = (self.layer_to_screen(last), self.layer_to_screen(raw));
        let w = (r * self.layer_zoom()).max(0.5);
        let color = Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], (alpha * 255.0).round() as u8);
        if a.distance(b) > 0.5 {
            painter.line_segment([a, b], Stroke::new(2.0 * w, color));
        }
        painter.circle_filled(b, w, color);
    }

    /// Menu du clic droit sur le dessin. Chaque entrée est une commande : un clic droit dessus
    /// lui donne un raccourci.
    /// Menu court (2026-10-18) : la vue (zoom, rotation, miroir, pelure, table lumineuse,
    /// caméra) a ses boutons sous le dessin.
    pub(crate) fn canvas_menu(&mut self, ui: &mut egui::Ui) {
        self.action_item(ui, "Annuler", "edition.annuler");
        self.action_item(ui, "Rétablir", "edition.retablir");
        ui.separator();
        self.action_item(ui, "Couper", "edition.couper");
        self.action_item(ui, "Copier", "edition.copier");
        self.action_item(ui, "Coller", "edition.coller");
        self.action_item(ui, "Supprimer la sélection", "edition.supprimer");
        ui.separator();
        self.action_item(ui, "Ajuster à la fenêtre", "vue.ajuster");
        ui.separator();
        let mode = if self.mode == crate::app::Mode::Storyboard { "Animer ce plan" } else { "Revenir au storyboard" };
        self.action_item(ui, mode, "mode.basculer");
    }

    fn canvas_input(&mut self, ctx: &egui::Context, response: &egui::Response, rect: Rect) {
        let size = self.canvas_size();
        let (events, hover, scroll, zoom, mods, space, primary_down) = ctx.input(|i| {
            (
                i.events.clone(),
                i.pointer.hover_pos(),
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.modifiers,
                self.keymap.held(i, "vue.pan_libre"),
                i.pointer.primary_down(),
            )
        });
        let modal = self.modal_open();

        // Molette : zoom (ou rotation avec Maj), Ctrl+molette / pincement : zoom.
        if response.hovered() && !modal {
            if let Some(p) = hover {
                if mods.shift {
                    let d = scroll.x + scroll.y;
                    if d != 0.0 {
                        self.view.rotate_around(rect, size, p, d * 0.004);
                    }
                } else {
                    let factor = zoom * (scroll.y * 0.003).exp();
                    if (factor - 1.0).abs() > 1e-4 {
                        self.view.zoom_around(rect, size, p, factor);
                    }
                }
            }
        }

        // Déplacement de la vue : bouton du milieu, Espace + glisser, ou outil Main.
        let panning = self.tool == Tool::Pan || space;
        if response.dragged_by(PointerButton::Middle)
            || (panning && response.dragged_by(PointerButton::Primary))
        {
            self.view.pan += response.drag_delta();
        }
        // « Décaler la pelure » (13f) : glisser déplace les images de la pelure, pas le dessin.
        let shifting = self.onion_shift && !panning;
        if shifting && response.dragged_by(PointerButton::Primary) {
            if let Some(p) = response.interact_pointer_pos() {
                let now = self.view.to_canvas(rect, size, p);
                let before = self.view.to_canvas(rect, size, p - response.drag_delta());
                self.onion_offset += now - before;
            }
        }

        // Partie visible seulement (une vignette de la planche peut dépasser de la vue).
        // (Un appui sur la bande de note d'une case, que la planche a pris pour un trait de cette case, est aussi sur le dessin : le trait
        // part de la marge sous la case, `Vignettes::appui_bande`.)
        let appui_bande = if self.in_card { self.vignettes.appui_bande } else { None };
        let on_canvas = |pos: Pos2| {
            (rect.contains(pos) && response.interact_rect.contains(pos) && ctx.layer_id_at(pos) == Some(response.layer_id)) || appui_bande == Some(pos)
        };

        // Horodatage par événement (sensation des brosses, §2.5) : egui date tous les événements
        // d'une image d'écran de la même heure, alors que les échantillons du stylet (l'historique
        // WM_POINTER lu par winit) sont réguliers. On étale ceux du pointeur entre l'image d'avant
        // et celle-ci : la vitesse et le lissage selon le temps y gagnent.
        let now = ctx.input(|i| i.time);
        let pointer_events = events.iter().filter(|e| matches!(e, Event::PointerMoved(_) | Event::PointerButton { .. })).count();
        let mut k = 0;
        for event in &events {
            if matches!(event, Event::PointerMoved(_) | Event::PointerButton { .. }) {
                k += 1;
                self.input_time = crate::brush::spread_time(self.last_frame_time, now, k, pointer_events);
            }
            match event {
                // Le stylet arrive sous forme d'événements « tactiles » qui portent la pression ;
                // egui les double d'événements souris, que l'on suit pour la position.
                Event::Touch { phase, force, .. } => {
                    if let Some(f) = force {
                        self.pen_pressure = f.clamp(0.0, 1.0);
                    }
                    if *phase == TouchPhase::Start {
                        self.pen_touch = true;
                    }
                }
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let pen = std::mem::take(&mut self.pen_touch);
                    if modal || panning || shifting || !on_canvas(*pos) {
                        continue;
                    }
                    // Un geste de l'anneau sous le stylet est en cours (`anneauui.rs`) : la pointe posée pendant que le bouton du
                    // stylet est tenu ne trace rien.
                    if self.anneau.geste.is_some() {
                        continue;
                    }
                    // Dans une vignette de la planche (`boardui.rs`), seul l'appui que la planche a donné au dessin dessine :
                    // Maj (ou Ctrl + Maj : la caméra) + appui ne dessine pas, un appui sur une barre, une note ou hors d'une image ne trace rien (la mise en page
                    // a pu changer sous la souris pendant cette même image).
                    if self.in_card && (modifiers.shift || self.vignettes.camera_tenue || !matches!(self.vignettes.press, crate::boardui::Press::Card | crate::boardui::Press::None)) {
                        continue;
                    }
                    // `stage` : point de la feuille (guides, caméra, pipette) ; `p` : point du dessin
                    // du calque courant, qui peut être transformé (pinceaux, formes, sélection…).
                    let stage = self.view.to_canvas(rect, size, *pos);
                    let p = self.to_layer(stage);
                    // Un point de fuite du guide se déplace avec n'importe quel outil.
                    if let Some(k) = self.guide_point_at(*pos) {
                        self.guides.drag = Some(k);
                        continue;
                    }
                    // (Avec le Dégradé, Alt dit « tout le calque » : la pipette reste à l'outil Pipette.)
                    if (modifiers.alt && self.tool != Tool::Gradient) || self.tool == Tool::Picker {
                        self.picking = true;
                        self.pick_color(stage);
                        continue;
                    }
                    // Une brosse du décor choisie, sur le dessin du Storyboard : le trait part dans le calque Décor du plan
                    // (créé au premier trait, `decorui.rs` ; dans une case de la planche, c'est la planche qui l'a fait).
                    // (Un personnage choisi : son calque, `calquespastilles.rs`.)
                    let p = if !self.in_card && self.pastille_a_un_calque() {
                        self.pastille_avant_trait();
                        self.to_layer(stage)
                    } else {
                        p
                    };
                    // Plan en Espace 3D, carte inclinée : le regard ne touche pas la carte du calque courant ici (elle est
                    // derrière la caméra, vue par la tranche, ou trop loin de ses bords) : ce qui peint ne commence pas.
                    if matches!(self.tool, Tool::Fill | Tool::Gradient | Tool::Brush | Tool::Mixer | Tool::Pencil | Tool::Eraser) && self.to_layer_opt(stage).is_none() {
                        self.status = "Rien à peindre ici : la carte du calque courant n'est pas sous le pointeur (elle est derrière la caméra, ou vue par la tranche).".into();
                        continue;
                    }
                    match self.tool {
                        Tool::Fill => self.fill_at(p),
                        Tool::Gradient => self.degrade_press(p, modifiers.alt),
                        Tool::Brush | Tool::Mixer | Tool::Pencil | Tool::Eraser => {
                            let pressure = if pen { self.pen_pressure } else { 1.0 };
                            // Gomme et plusieurs calques (ou clés) choisis : la marque de l'historique avant le trait (`gommemulti.rs`).
                            let gomme_multi = self.gomme_multi_avant();
                            // Le trait d'abord (`begin_paint` clôt le précédent, ce qui oublie le
                            // guide), puis le guide qu'il suit : sinon l'aimant ne servait jamais.
                            self.begin_paint(p, pressure, pen);
                            if self.stroke.is_some() || self.erase_pen().is_some() {
                                self.begin_guided_stroke(stage);
                                // Maj tenue pendant le trait : un segment droit (`tracedroit.rs`).
                                self.droit_start(p);
                            }
                            if let Some(marque) = gomme_multi {
                                self.gomme_multi_debut(marque, p, pressure, pen);
                            }
                        }
                        Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Polyline => self.shape_press(p, modifiers.shift),
                        Tool::Text => self.text_press(p),
                        // Déformer : l'enveloppe de Bézier ou la perspective (`envelopetools.rs`,
                        // `perstools.rs`).
                        // Plusieurs calques (ou clés) choisis : une seule déformation pour tous (`deformmulti.rs`).
                        Tool::Perspective if self.deform_multi_voulu() => self.deform_multi_press(stage, *pos),
                        Tool::Perspective => self.deform_press(p, *pos),
                        // La boîte de la sélection : déplacer, échelle, rotation, inclinaison, pivot
                        // (`seltools.rs`). Avec plusieurs calques choisis, le lasso prend sur tous
                        // et la boîte les entoure (`multitools.rs`).
                        // Ctrl+Maj : tous les calques du plan.
                        Tool::Select if self.multi_wanted() || crate::multitools::is_all_layers_gesture(modifiers) => {
                            self.multi_press(stage, *pos, modifiers.shift, crate::multitools::is_all_layers_gesture(modifiers))
                        }
                        Tool::Select => self.select_press(p, *pos, modifiers.shift),
                        Tool::SelectColor => self.color_select_press(p, *pos, modifiers.shift),
                        Tool::Cutter => self.cutter_press(p),
                        // Grab et Pull (`sculptapp.rs`).
                        Tool::Grab | Tool::Pull => self.sculpt_press(p, pen),
                        // Trajectoire (`trajectoireui.rs`) : sur la feuille.
                        Tool::Trajectoire => self.trajectoire_presser(stage, modifiers.shift),
                        Tool::Camera => self.camera_press(stage, *pos),
                        Tool::Transform => {
                            // Une zone bitmap choisie : sa boîte la transforme ; ailleurs, c'est le calque.
                            if self.bsel.is_some() && self.box_hit(*pos).is_some() {
                                self.bitmap_select_press(p, *pos, false);
                            } else {
                                self.transform_press(stage, *pos);
                            }
                        }
                        Tool::Picker | Tool::Pan => {}
                    }
                }
                Event::PointerMoved(pos) => {
                    let stage = self.view.to_canvas(rect, size, *pos);
                    let p = self.to_layer(stage);
                    if let Some(k) = self.guides.drag {
                        self.move_guide_point(k, stage);
                    } else if let Some(pen) = self.stroke.as_ref().map(|s| s.pen) {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        // Trait aligné sur le guide (sur la feuille) : rien tant que la direction
                        // n'est pas choisie.
                        if let Some(q) = self.guided_point(stage) {
                            // (Le trait ne suit pas le pointeur là où il quitte la carte : ces points sont laissés.)
                            if let Some(l) = self.to_layer_opt(q) {
                                self.stroke_point(l, pressure, &mods, false);
                            }
                        }
                    } else if let Some(pen) = self.erase_pen() {
                        let pressure = if pen { self.pen_pressure } else { 1.0 };
                        if let Some(q) = self.guided_point(stage) {
                            if let Some(l) = self.to_layer_opt(q) {
                                self.stroke_point(l, pressure, &mods, true);
                            }
                        }
                    } else if self.gomme_multi.actif() {
                        // (La gomme sur les autres calques choisis, quand le calque courant n'a rien à gommer.)
                        let pressure = if self.gomme_multi.pen() { self.pen_pressure } else { 1.0 };
                        if let Some(q) = self.guided_point(stage) {
                            if let Some(l) = self.to_layer_opt(q) {
                                self.gomme_multi_point(l, pressure);
                            }
                        }
                    } else if self.cutter_drag(p) {
                    } else if self.sculpt_drag(p) {
                    } else if self.deform_multi_drag(stage) {
                    } else if self.envelope_drag(p) {
                    } else if self.persp_drag(p) {
                    } else if self.shape_bend_move(p) {
                    } else if self.degrade_drag(p, mods.shift) {
                    } else if let Some((_, end)) = &mut self.shape_drag {
                        *end = p;
                    } else if self.multi.drag.is_some() {
                        self.multi_drag(stage, mods.shift, mods.command || mods.ctrl);
                    } else if self.vdrag.is_some() {
                        self.select_drag(p, mods.shift, mods.command || mods.ctrl);
                    } else if self.bdrag.is_some() {
                        self.bitmap_drag(p, mods.shift, mods.command || mods.ctrl);
                    } else if self.cam_drag.is_some() {
                        self.camera_drag(stage, mods.shift);
                    } else if self.layer_drag.is_some() {
                        self.transform_drag(stage, mods.shift, mods.command || mods.ctrl);
                    } else if self.trajectoire_glisser(stage) {
                    } else if self.picking {
                        self.pick_color(stage);
                    }
                }
                Event::PointerButton {
                    button: PointerButton::Primary,
                    pressed: false,
                    ..
                }
                | Event::PointerGone => {
                    self.shape_release(mods.shift);
                    self.degrade_release();
                    self.persp_release();
                    self.envelope_release();
                    self.deform_multi_release();
                    if self.multi.drag.is_some() {
                        self.multi_finish_drag();
                    }
                    if self.vdrag.is_some() {
                        self.finish_vdrag(mods.shift);
                    }
                    if self.bdrag.is_some() {
                        self.finish_bdrag();
                    }
                    self.guides.drag = None;
                    self.end_stroke();
                    self.picking = false;
                }
                _ => {}
            }
        }
        self.input_time = now;
        self.last_frame_time = now;
        // Grab et Pull : les pas du pointeur de cette image, posés une seule fois.
        self.sculpt_tick();
        // Le dégradé posé : retouche, fige, ou remontre l'axe d'une zone vectorielle choisie.
        self.degrade_tick();
        // Relâchement manqué (perte de focus…) : on clôt le geste quand même.
        if !primary_down {
            self.degrade_release();
            self.end_stroke();
            self.picking = false;
            self.guides.drag = None;
            // Le calque Décor né de ce trait et le trait ne font qu'une étape ; un clic qui n'a rien tracé ne le laisse pas
            // (`decor_avant_trait`). Dans une case de la planche, c'est la planche qui finit.
            if !self.in_card {
                self.finish_creation(false);
            }
        }

        self.hover_canvas = hover
            .filter(|p| rect.contains(*p))
            .map(|p| self.view.to_canvas(rect, size, p));
        // Maj tenue (ou relâchée) sans que la souris bouge : le segment droit suit quand même.
        let pen = self.stroke.as_ref().map(|s| s.pen).or(self.erase_pen()).unwrap_or(false);
        self.droit_follow(&mods, primary_down, self.hover_canvas, if pen { self.pen_pressure } else { 1.0 });
    }

    /// Sélection vectorielle, lasso et aperçu des formes, par-dessus le dessin.
    fn paint_vector_overlay(&self, painter: &egui::Painter, _rect: Rect, shift: bool) {
        // Tout ce qui suit est en coordonnées du dessin du calque courant (transformé ou non).
        let to_s = |p: Pos2| self.layer_to_screen(p);

        self.paint_shape_overlay(painter, shift);
        self.paint_degrade_overlay(painter);

        // Lasso ou rectangle en cours de tracé (vectoriel ou bitmap).
        // Une ligne fine ouverte et des points ambrés (`contours::lasso_shape`) ; un rectangle se
        // referme sur son premier coin.
        let drawing_lasso: Option<Vec<Pos2>> = match (&self.vdrag, &self.bdrag) {
            (Some(VDrag::Lasso(points)), _) | (_, Some(BDrag::Lasso { points, .. })) => Some(points.iter().map(|p| to_s(*p)).collect()),
            (_, Some(BDrag::Rect { from, to, .. })) => {
                let mut pts: Vec<Pos2> = crate::pixsel::rect_polygon(Rect::from_two_pos(*from, *to)).into_iter().map(to_s).collect();
                pts.extend(pts.first().copied());
                Some(pts)
            }
            _ => None,
        };
        if let Some(shape) = drawing_lasso.and_then(|pts| crate::contours::lasso_shape(&pts)) {
            painter.add(shape);
        }

        // Geste du cutter en cours.
        if let Some(g) = self.cut_gesture.as_ref().filter(|g| g.len() >= 2) {
            let pts: Vec<Pos2> = g.iter().map(|p| to_s(*p)).collect();
            painter.add(Shape::line(pts.clone(), Stroke::new(3.0_f32, Color32::from_black_alpha(120))));
            painter.add(Shape::line(pts, Stroke::new(1.5_f32, Color32::from_rgb(240, 70, 60))));
        }

        // Geste de Grab : le cercle de l'appui et le trait jusqu'au pointeur.
        self.paint_sculpt_overlay(painter);

        // Contour de la zone bitmap sélectionnée : un trait orange d'1 px (gardé, `contours.rs`).
        if let Some(shape) = self.bitmap_selection_shape() {
            painter.add(shape);
        }

        // Sélection sur plusieurs calques : lasso, contours de chaque calque et boîte commune.
        if self.paint_multi_overlay(painter) {
            return;
        }
        if self.sel_frame().is_none() {
            return;
        }
        if let Some(sel) = &self.vsel {
            // Geste en cours : les éléments n'ont pas bougé, leurs contours suivent la feuille.
            // (Contours gardés d'une image à l'autre, simplifiés à l'écran : `contours.rs`.)
            let gesture = self.flottante.as_ref().is_some_and(|f| f.pending.is_none());
            let moved = self.flottante.as_ref().filter(|f| f.pending.is_none()).map(|f| f.aff).unwrap_or_default();
            if let Some(shape) = self.selection_contours(sel.cel, &sel.items, moved, gesture, ACCENT) {
                painter.add(shape);
            }
        }
        // Le cadre, les huit poignées et le pivot (`seltools.rs`).
        self.paint_selection_box(painter);
    }

    fn paint_cursor(&self, ctx: &egui::Context, painter: &egui::Painter, response: &egui::Response) {
        let Some(p) = response.hover_pos() else { return };
        if self.modal_open() {
            return;
        }
        // Vignettes : Maj tenue sur le cadre de la case, c'est la planche qui donne le curseur (la main), pas le pinceau.
        if self.in_card && (self.vignettes.frame_hot || self.annot_actif()) {
            return;
        }
        let (space, alt) = ctx.input(|i| (self.keymap.held(i, "vue.pan_libre"), i.modifiers.alt));
        if self.guides.drag.is_some() || self.guide_point_at(p).is_some() {
            ctx.set_cursor_icon(if self.guides.drag.is_some() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            });
            return;
        }
        if space || self.tool == Tool::Pan {
            ctx.set_cursor_icon(if response.dragged() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            });
            return;
        }
        // « Décaler la pelure » (13f) : pas de curseur d'outil, on ne dessine pas.
        if self.onion_shift {
            ctx.set_cursor_icon(CursorIcon::Move);
            return;
        }
        if alt || matches!(self.tool, Tool::Picker | Tool::Fill | Tool::Gradient | Tool::Cutter) || self.tool.is_shape() {
            ctx.set_cursor_icon(CursorIcon::Crosshair);
            return;
        }
        if self.tool == Tool::Text {
            ctx.set_cursor_icon(CursorIcon::Text);
            return;
        }
        if self.tool == Tool::Perspective {
            ctx.set_cursor_icon(self.deform_cursor(p));
            return;
        }
        // Grab et Pull : le cercle du pinceau.
        if matches!(self.tool, Tool::Grab | Tool::Pull) {
            self.sculpt_cursor(ctx, painter, p);
            return;
        }
        if self.tool == Tool::Trajectoire {
            ctx.set_cursor_icon(self.trajectoire_curseur(p));
            return;
        }
        // La zone bitmap choisie reste transformable avec l'outil Transformer.
        if self.tool == Tool::Transform && self.bsel.is_some() && self.box_cursor(ctx, painter, p) {
            return;
        }
        if self.tool == Tool::Transform {
            use crate::layertools::BoxHandle;
            let poignee = self.transform_handle_at(p);
            // Le mode d'emploi de la poignée sous le pointeur, en bulle (il n'est plus dans le panneau Réglages).
            // (Pas pour l'intérieur de la boîte : une bulle y gênerait le dessin.)
            if let Some(h) = poignee.filter(|h| *h != BoxHandle::Move) {
                Self::bulle_poignee(ctx, h);
            }
            ctx.set_cursor_icon(match poignee {
                Some(BoxHandle::Move) => CursorIcon::Move,
                Some(BoxHandle::Corner(0 | 2)) => CursorIcon::ResizeNwSe,
                Some(BoxHandle::Corner(_)) => CursorIcon::ResizeNeSw,
                Some(BoxHandle::Edge(0 | 2)) => CursorIcon::ResizeVertical,
                Some(BoxHandle::Edge(_)) => CursorIcon::ResizeHorizontal,
                Some(BoxHandle::Rotate | BoxHandle::RotateCorner(_)) => CursorIcon::Alias,
                None => CursorIcon::Default,
            });
            return;
        }
        let handle = match self.tool {
            Tool::Camera => Some(self.camera_handle_at(p)),
            _ => None,
        };
        if let Some(h) = handle {
            ctx.set_cursor_icon(match h {
                Some(crate::camtools::CamHandle::Move) => CursorIcon::Move,
                Some(crate::camtools::CamHandle::Zoom) => CursorIcon::ResizeNwSe,
                Some(crate::camtools::CamHandle::Rotate) => CursorIcon::Alias,
                None => CursorIcon::Default,
            });
            return;
        }
        if self.tool.is_select() {
            let pick = self.tool == Tool::SelectColor;
            // La boîte de la sélection : un curseur par partie (`seltools.rs`) ; avec la sélection
            // par couleur, l'intérieur reste à la pipette.
            let handle = self.active_or_hit_handle(p);
            if handle.is_some_and(|h| !(pick && h == crate::selbox::Handle::Move)) && self.box_cursor(ctx, painter, p) {
                return;
            }
            ctx.set_cursor_icon(if pick { CursorIcon::Crosshair } else { CursorIcon::Default });
            return;
        }
        let Some(brush) = self.current_brush() else { return };
        // La croix du système suit le stylet sans retard ; le cercle (taille du pinceau), dessiné
        // dans l'image, arrive une image plus tard.
        let system = self.settings.system_cursor;
        ctx.set_cursor_icon(if system { CursorIcon::Crosshair } else { CursorIcon::None });
        let r = (brush.size * 0.5 * self.layer_zoom()).max(1.5);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        if !system {
            let cross = Stroke::new(1.0_f32, Color32::from_gray(230));
            painter.line_segment([p - vec2(4.0, 0.0), p + vec2(4.0, 0.0)], cross);
            painter.line_segment([p - vec2(0.0, 4.0), p + vec2(0.0, 4.0)], cross);
        }
        // (Le petit « V » qui suivait le curseur sur un calque vectoriel est retiré : Anthony,
        // 2026-10-18 ; la sorte du calque se lit dans le panneau Calques et le panneau Réglages.)
    }
}
