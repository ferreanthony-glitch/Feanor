//! La vue d'édition de l'Espace 3D : navigation (tourner, glisser, avancer), choix des cartes, des objets du décor 3D
//! (partie 184, `poignees3d.rs`) et de la caméra à la souris, manipulateurs (déplacer, tourner, échelle ; l'origine seule avec
//! « Modifier l'origine »), caméra du plan, tracé. Tout est dessiné par egui : les cartes (les
//! calques du plan, texturés par leur dessin) dans l'ordre de la pile des calques — ce que l'image compose
//! (`docs/ESPACE3D.md`, « Ordre ») —, le quadrillage du sol, la caméra et les poignées.

use eframe::egui::{self, epaint::Vertex, pos2, vec2, Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::espace3d::{self, SceneCam, MAX_TILT};
use crate::espace3dgizmo::{self as gz, Gizmo, Handle, Mode as GizmoMode, Originals, CAMERA_ID};
use crate::espace3dmath::{self as m, Orbite, Ray, V3, View};
use crate::espace3dui::Drag;

const ACCENT: Color32 = crate::theme::POIGNEE_SELECTION;
/// (Aussi les flèches x, y, z d'une poignée de trajectoire, `trajectoirepoignees.rs`, partie 154.)
pub(crate) const AXIS_COLORS: [Color32; 3] = [Color32::from_rgb(232, 80, 80), Color32::from_rgb(100, 205, 100), Color32::from_rgb(86, 138, 245)];
pub(crate) const HOT: Color32 = Color32::from_rgb(255, 214, 70);
/// Distance (pixels d'écran) à laquelle on attrape une poignée.
const GRAB: f32 = 9.0;
/// Un point plus près de l'œil que cela n'est pas dessiné.
pub(crate) const NEAR: f32 = 0.05;
/// Geste « déplacer la caméra du plan » dans la vue par la caméra (une seule étape d'annulation).
const GESTURE_CAMERA: u64 = 0x5c3d_0001;

/// Le plus grand rectangle de proportions `frame` dans `rect`, centré (avec une petite marge).
pub(crate) fn letterbox(rect: Rect, frame: [usize; 2]) -> Rect {
    let aspect = frame[0].max(1) as f32 / frame[1].max(1) as f32;
    let avail = rect.shrink(8.0);
    let (w, h) = if avail.width() / avail.height().max(1.0) > aspect { (avail.height() * aspect, avail.height()) } else { (avail.width(), avail.width() / aspect) };
    Rect::from_center_size(rect.center(), vec2(w.max(1.0), h.max(1.0)))
}

/// Un pas rond (1, 2 ou 5 × 10ⁿ) proche de `span` / 12.
pub(crate) fn nice_step(span: f32) -> f32 {
    let raw = (span / 12.0).max(1e-3);
    let p = 10f32.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|k| k * p).find(|s| *s >= raw).unwrap_or(10.0 * p)
}

/// Un trait du quadrillage du sol : deux points du monde et une couleur (rouge, vert, bleu, opacité de 0 à 1).
#[derive(Clone, Copy, Debug)]
pub(crate) struct GridLine {
    pub a: V3,
    pub b: V3,
    pub color: [f32; 4],
}

/// Les traits du quadrillage du sol autour de `target`, à l'échelle de ce qu'on regarde.
#[cfg(test)]
pub(crate) fn grid_lines(target: V3, distance: f32) -> Vec<GridLine> {
    grid_lines_pas(target, distance, None)
}

/// `grid_lines`, au pas `pas` (mètres) s'il est donné (l'environnement de l'Espace 3D, `espace3ddecor.rs`) : les traits
/// couvrent au moins la même étendue qu'au pas automatique, et au moins 60 pas de chaque côté (vue tout près de la scène,
/// le sol reste quadrillé loin vers l'horizon, partie 158) ; 300 au plus de chaque côté.
pub(crate) fn grid_lines_pas(target: V3, distance: f32, pas: Option<f32>) -> Vec<GridLine> {
    let auto = nice_step((distance * 2.4).clamp(1.0, 400.0));
    let (step, n) = match pas.filter(|p| p.is_finite() && *p > 0.0) {
        Some(p) => (p, ((15.0 * auto / p).ceil() as i32).clamp(60, 300)),
        None => (auto, 15i32),
    };
    let (cx, cz) = ((target[0] / step).round() * step, (target[2] / step).round() * step);
    let half = n as f32 * step;
    let mut out = Vec::with_capacity(((2 * n + 1) * 2) as usize);
    for i in -n..=n {
        let (x, z) = (cx + i as f32 * step, cz + i as f32 * step);
        let major = ((x / step).round() as i64) % 5 == 0;
        let minor = [0.7, 0.7, 0.72, if major { 0.32 } else { 0.14 }];
        let line_x = if z.abs() < step * 0.01 { [0.9, 0.32, 0.32, 0.75] } else { minor };
        let line_z = if x.abs() < step * 0.01 { [0.35, 0.5, 0.95, 0.75] } else { minor };
        out.push(GridLine { a: [cx - half, 0.0, z], b: [cx + half, 0.0, z], color: line_x });
        out.push(GridLine { a: [x, 0.0, cz - half], b: [x, 0.0, cz + half], color: line_z });
    }
    out
}

/// Le morceau du trait du monde `a`–`b` qui est devant l'œil de `view` (plus loin que `NEAR` en perspective) ; rien s'il
/// est tout entier derrière.
///
/// (Partie 158. Avant, un trait dont un bout passait derrière l'œil n'était pas dessiné du tout. Les traits du maillage
/// traversent tout le sol autour de ce qu'on regarde : vue tout près de la scène, presque tous ont un bout derrière
/// l'œil, et il ne restait que ceux du lointain, en bande vers l'horizon.)
pub(crate) fn devant_l_oeil(view: &View, a: V3, b: V3) -> Option<(V3, V3)> {
    let proche = if view.is_ortho() { 1e-5 } else { NEAR };
    let (da, db) = (view.to_view(a)[2], view.to_view(b)[2]);
    if !(da.is_finite() && db.is_finite()) || (da < proche && db < proche) {
        return None;
    }
    // (Le point du trait à la profondeur `proche`, depuis le bout `p` (derrière) vers le bout `q` (devant).)
    let coupe = |p: V3, q: V3, dp: f32, dq: f32| {
        let t = (proche - dp) / (dq - dp);
        [0, 1, 2].map(|k| p[k] + (q[k] - p[k]) * t)
    };
    let a2 = if da < proche { coupe(a, b, da, db) } else { a };
    let b2 = if db < proche { coupe(b, a, db, da) } else { b };
    Some((a2, b2))
}

/// Les traits du quadrillage `lignes` tels que `view` les montre : à l'écran (`center` : le point de l'image où elle regarde,
/// `origine` : le coin de l'image à l'écran), avec leur couleur. Un trait qui passe derrière l'œil est coupé là où il
/// le passe (`devant_l_oeil`), puis au bord de `zone` (le morceau coupé tout près de l'œil part très loin hors de l'écran).
pub(crate) fn traits_a_l_ecran(view: &View, center: [f32; 2], origine: Pos2, zone: Rect, lignes: &[GridLine]) -> Vec<(Pos2, Pos2, [f32; 4])> {
    let ecran = |p: V3| view.project(p, center).map(|q| pos2(origine.x + q[0], origine.y + q[1]));
    let zone = zone.expand(2.0);
    let mut out = Vec::with_capacity(lignes.len());
    for l in lignes {
        let Some((a, b)) = devant_l_oeil(view, l.a, l.b) else { continue };
        let (Some(a), Some(b)) = (ecran(a), ecran(b)) else { continue };
        if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
            continue;
        }
        if let Some((a, b)) = crate::guides::clip_line(a, b - a, zone, true) {
            out.push((a, b, l.color));
        }
    }
    out
}

/// Un appui sur 1 ou 2 dans l'Espace 3D vaut ce nombre de points de molette : le même pas que la feuille (× 1,25), puisque la
/// molette zoome de `exp(0,002 × points)`.
pub(crate) const CRAN_CLAVIER: f32 = 111.571_77; // ln(1,25) / 0,002

impl App {
    /// La hauteur (mètres) où se pose le quadrillage du sol. Le seul endroit qui la donne, pour la vue seule comme pour
    /// chaque vue (`espace3dvues.rs`).
    pub(crate) fn e3_hauteur_sol(&self) -> f32 {
        // (Le sol des surfaces, `Monde::sol_y` : tout plan est dans l'espace, partie 159.)
        self.e3_world().sol_y()
    }

    /// Le quadrillage d'une vue : le sol autour de `target`, à l'échelle de `distance` (perspective, caméra du plan) ; ou,
    /// pour une vue orthographique (`ortho` : son point de vue, ce qu'elle regarde, sa largeur sur sa hauteur), le plan de
    /// l'espace qui lui fait face. La même fonction pour la vue seule et pour chaque vue.
    pub(crate) fn e3_quadrillage(&self, target: V3, distance: f32, ortho: Option<(crate::espace3dvues::PointDeVue, crate::espace3dvues::Ortho, f32)>) -> Vec<GridLine> {
        let sol = self.e3_hauteur_sol();
        // (L'environnement du projet : le pas, la couleur et l'opacité du maillage, `espace3ddecor.rs`. D'origine : rien ne
        // change.)
        let env = self.e3_environnement();
        let lignes = match ortho {
            Some((p, o, aspect)) => crate::espace3dvues::grille_ortho_pas(p, &o, aspect, sol, env.pas_du_maillage()),
            None => grid_lines_pas(target, distance, env.pas_du_maillage()).into_iter().map(|l| GridLine { a: [l.a[0], l.a[1] + sol, l.a[2]], b: [l.b[0], l.b[1] + sol, l.b[2]], color: l.color }).collect(),
        };
        env.teinter_maillage(lignes)
    }

    /// Peint ces traits de quadrillage, vus par `view` (`center` : le point de l'image où elle regarde, `origine` : le coin
    /// de l'image à l'écran ; `traits_a_l_ecran`).
    pub(crate) fn e3_peindre_quadrillage(painter: &egui::Painter, view: &View, center: [f32; 2], origine: Pos2, lignes: Vec<GridLine>) {
        for (a, b, [r, g, bl, alpha]) in traits_a_l_ecran(view, center, origine, painter.clip_rect(), &lignes) {
            let c = Color32::from_rgba_unmultiplied((r * 255.0) as u8, (g * 255.0) as u8, (bl * 255.0) as u8, (alpha * 255.0) as u8);
            painter.line_segment([a, b], Stroke::new(1.0_f32, c));
        }
    }

    /// La vue d'édition, ou la vue de la caméra du plan.
    pub(crate) fn e3_view(&self, draw: Rect) -> View {
        if self.espace3d.through_camera {
            self.e3_world().scene_cam(&self.current_camera()).view(draw.width())
        } else {
            self.espace3d.cam.unwrap_or(Orbite { target: [0.0; 3], azimuth: 30.0, elevation: 22.0, distance: 8.0, focal: 35.0 }).view(draw.width())
        }
    }

    /// Le manipulateur de la sélection.
    pub(crate) fn e3_gizmo(&self, view: &View) -> Option<Gizmo> {
        // « Modifier l'origine » (partie 184) : des flèches et des carrés posés sur l'origine de ce qui est choisi seul.
        if self.espace3d.origine {
            return self.e3_gizmo_origine(view);
        }
        let mode = self.espace3d.mode;
        if mode == GizmoMode::Select || self.espace3d.sel.is_empty() {
            return None;
        }
        let xfs = self.e3_xfs(&self.espace3d.sel);
        let first = xfs.first()?;
        // (L'origine d'une carte seule, la moyenne de celles de plusieurs : partie 182.)
        let origin = self.e3_pivot(&xfs);
        // (L'échelle se fait sur les axes de la carte : ses cubes suivent donc toujours la carte.)
        let local = (self.espace3d.local || mode == GizmoMode::Scale) && xfs.len() == 1;
        Some(Gizmo { origin, axes: gz::axes_for(local, &first.1), len: Gizmo::size_for(view, origin), mode })
    }

    /// Les cartes visibles du plan courant dans l'ordre où la vue les trace.
    ///
    /// Plan ordinaire : du fond vers le dessus de la pile, l'ordre où l'image les compose (`e3_visible_cards`).
    ///
    /// **Plan en Espace 3D** (`docs/ESPACE-LIBRE.md`, étape A7) : du plus loin au plus près de l'œil de la vue, comme
    /// l'image les compose vues de la caméra (`espace3d::ordre_vu` : même plan = ordre de la pile, dossier en bloc,
    /// annotations en dernier). « Regarder par la caméra » : l'œil est la caméra du plan, l'ordre est exactement celui de
    /// l'image.
    pub(crate) fn e3_cartes_a_tracer(&self, view: &View) -> Vec<usize> {
        let visibles = self.e3_visible_cards();
        if !self.doc.ordre_par_la_distance(self.cur_panel) {
            return visibles;
        }
        let layers = self.layers();
        let pile = crate::calqueannot::ordre(layers, true);
        let world = self.e3_world();
        let repere = self.doc.repere_du_plan(self.cur_panel, self.cur_frame);
        let regard = if self.espace3d.through_camera { espace3d::Regard::Camera(repere.as_ref()) } else { espace3d::Regard::Vue { eye: view.eye, forward: view.forward } };
        espace3d::ordre_vu(&world, layers, &pile, self.cur_frame, &regard).into_iter().filter(|li| visibles.contains(li)).collect()
    }

    /// La carte touchée par le rayon : la plus proche dont le dessin est touché, sinon la plus proche dont la feuille
    /// l'est (en dernier recours, pour attraper un calque vide).
    pub(crate) fn e3_pick(&mut self, ray: &Ray) -> Option<u64> {
        let (drawn_card, sheet_card) = self.e3_pick_cartes(ray);
        drawn_card.map(|(t, id, _)| (t, id)).or(sheet_card).map(|b| b.1)
    }

    /// Les cartes touchées par le rayon : la plus proche dont le dessin est touché (sa distance, son numéro de sélection, et
    /// si elle partage la profondeur du décor 3D, `profondeur::partage`), et la plus proche dont la feuille l'est.
    pub(crate) fn e3_pick_cartes(&self, ray: &Ray) -> (Option<(f32, u64, bool)>, Option<(f32, u64)>) {
        let world = self.e3_world();
        let frame = self.cur_frame;
        let mut drawn_card: Option<(f32, u64, bool)> = None;
        let mut sheet_card: Option<(f32, u64)> = None;
        if self.espace3d.show_cards {
            for li in self.e3_visible_cards() {
                let layer = &self.layers()[li];
                let xf = world.carte(layer, frame);
                let Some((t, p)) = world.pick_card(&xf, ray) else { continue };
                let key = espace3d::card_selection(layer.id);
                if self.e3_card_drawn_at(li, p) && drawn_card.is_none_or(|(b, _, _)| t < b) {
                    drawn_card = Some((t, key, crate::profondeur::partage(layer)));
                }
                if sheet_card.is_none_or(|(b, _)| t < b) {
                    sheet_card = Some((t, key));
                }
            }
        }
        (drawn_card, sheet_card)
    }

    /// Le calque `li` a quelque chose de dessiné près du point `p` de son dessin (à l'image courante).
    pub(crate) fn e3_card_drawn_at(&self, li: usize, p: Pos2) -> bool {
        let Some(layer) = self.layers().get(li) else { return false };
        let Some(cel) = layer.shown_at(self.cur_frame).and_then(|c| self.doc.cels.get(&c)) else { return false };
        const R: f32 = 8.0;
        (-2..=2).any(|j| {
            (-2..=2).any(|i| {
                let (x, y) = (p.x + i as f32 * R / 2.0, p.y + j as f32 * R / 2.0);
                x >= 0.0 && y >= 0.0 && (x as usize) < cel.raster.width && (y as usize) < cel.raster.height && cel.raster.get(x as usize, y as usize).a() > 24
            })
        })
    }

    /// La caméra du plan est touchée par le pixel **sur son corps** (son œil, le sommet de la pyramide), ce qui passe avant une
    /// carte au clic (partie 152 : la caméra devant une carte se prend). Les traits fins de la pyramide et de son cadre, eux,
    /// ne passent qu'après les cartes : vue de face, ils traversent la carte que la caméra regarde, et la rendaient impossible
    /// à choisir.
    pub(crate) fn e3_pick_camera_devant(&self, view: &View, px: [f32; 2], center: [f32; 2]) -> bool {
        let cam = self.e3_world().scene_cam(&self.current_camera());
        view.project(cam.pos, center).is_some_and(|p| ((p[0] - px[0]).powi(2) + (p[1] - px[1]).powi(2)).sqrt() < 12.0)
    }

    /// La caméra du plan est touchée par le pixel (près de son corps ou de sa pyramide).
    pub(crate) fn e3_pick_camera(&self, view: &View, px: [f32; 2], center: [f32; 2]) -> bool {
        let cam = self.e3_world().scene_cam(&self.current_camera());
        let corners = self.e3_camera_corners(&cam, view);
        let near = |a: V3, b: V3| match (view.project(a, center), view.project(b, center)) {
            (Some(p), Some(q)) => seg_dist(px, p, q) < 7.0,
            _ => false,
        };
        let pos = cam.pos;
        (0..4).any(|k| near(pos, corners[k]) || near(corners[k], corners[(k + 1) % 4]))
    }

    /// Les coins de la pyramide de la caméra du plan, à une distance qui se lit dans la vue.
    fn e3_camera_corners(&self, cam: &SceneCam, view: &View) -> [V3; 4] {
        // (Une vue orthographique regarde de très loin : la pyramide prend une taille à l'échelle de ce qu'elle montre.)
        let d = match crate::espace3dvues::largeur_ortho(view) {
            Some(span) => (span * 0.08).clamp(0.3, 6.0),
            None => (m::length(m::sub(cam.pos, view.eye)) * 0.22).clamp(0.5, 6.0),
        };
        cam.corners_at(d, self.doc.frame)
    }

    /// La souris dans la zone de dessin 3D (une seule vue ; plusieurs : `espace3dvues.rs`).
    fn e3_input(&mut self, ctx: &egui::Context, draw: Rect, response: &egui::Response) {
        let scroll = ctx.input(|i| i.smooth_scroll_delta);
        let Some(mut cam) = self.espace3d.cam else { return };
        let through = self.espace3d.through_camera;
        let delta = response.drag_delta();

        // Navigation. Par la caméra du plan, elle déplace la caméra elle-même.
        if through {
            self.e3_nav_camera(draw, response, scroll.y, delta);
        } else {
            // (Le zoom au clavier vaut des crans de molette, la souris sur la vue ou non.)
            let clavier = std::mem::take(&mut self.espace3d.molette_clavier);
            if response.hovered() && scroll.y != 0.0 {
                cam.dolly((-scroll.y * 0.002).exp());
            }
            if clavier != 0.0 {
                cam.dolly((-clavier * 0.002).exp());
            }
            if response.dragged_by(PointerButton::Middle) {
                cam.pan(delta.x, delta.y, draw.width());
            }
            if response.dragged_by(PointerButton::Secondary) {
                cam.orbit(-delta.x * 0.35, delta.y * 0.35);
            }
        }

        let view = if through { self.e3_view(draw) } else { cam.view(draw.width()) };
        if self.e3_input_objets(ctx, &view, draw, response) && !through {
            cam.orbit(-delta.x * 0.35, delta.y * 0.35);
        }
        if !through {
            self.espace3d.cam = Some(cam);
        }
    }

    /// Par la caméra du plan : la molette l'avance, le bouton du milieu la déplace, le clic droit la tourne (sa clé de
    /// l'image courante si elle est animée), en une étape d'annulation par geste.
    pub(crate) fn e3_nav_camera(&mut self, draw: Rect, response: &egui::Response, scroll_y: f32, delta: egui::Vec2) {
        // Le zoom au clavier (`molette_clavier`) avance la caméra comme la molette, la souris sur la vue ou non.
        let clavier = std::mem::take(&mut self.espace3d.molette_clavier);
        let scroll_y = if response.hovered() { scroll_y } else { 0.0 } + clavier;
        let moving_camera = scroll_y != 0.0 || (response.hovered() && (response.dragged_by(PointerButton::Middle) || response.dragged_by(PointerButton::Secondary)));
        if moving_camera {
            self.gesture_change(GESTURE_CAMERA, |s| {
                let snap = s.snapshot();
                let before = s.history.context.replace("Déplacer la caméra".to_string());
                s.history.push(snap);
                s.history.context = before;
            });
            self.keep_gesture(GESTURE_CAMERA);
            // La caméra du plan : réglée à l'image courante, ou sa clé de cette image si elle est animée.
            let world = self.e3_world();
            let base = self.current_camera();
            let mut sc = world.scene_cam(&base);
            let view = sc.view(draw.width());
            if scroll_y != 0.0 {
                sc.pos = m::add(sc.pos, m::scale(view.forward, scroll_y * 0.01));
            }
            if response.dragged_by(PointerButton::Middle) {
                let k = 0.004 * m::length(m::sub(sc.pos, [0.0, 1.0, 0.0])).clamp(1.0, 20.0);
                sc.pos = m::add(sc.pos, m::add(m::scale(view.right, -delta.x * k), m::scale(view.up, delta.y * k)));
            }
            if response.dragged_by(PointerButton::Secondary) {
                sc.pan += delta.x * 0.15;
                sc.tilt = (sc.tilt - delta.y * 0.15).clamp(-MAX_TILT, MAX_TILT);
            }
            let mut new = world.camera_from_scene(&sc.sanitized());
            new.zoom = base.zoom;
            self.set_current_camera_libre(new);
        }
    }

    /// Ce que la souris fait dans une vue `view` dont la zone est `draw` : un clic choisit, un glisser tire une poignée ou
    /// une carte, le survol éclaire une poignée. Vrai si le glisser du bouton principal tourne la vue (à l'appelant de la
    /// tourner : la vue seule, une des vues, `espace3dvues.rs`).
    pub(crate) fn e3_input_objets(&mut self, ctx: &egui::Context, view: &View, draw: Rect, response: &egui::Response) -> bool {
        let (mods, press, primary_down) = ctx.input(|i| (i.modifiers, i.pointer.press_origin(), i.pointer.primary_down()));
        // (Maj tenue : l'origine qu'on tire s'accroche aux points de sa boîte, partie 184.)
        self.espace3d.maj = mods.shift;
        let view = *view;
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let rel = |p: Pos2| [p.x - draw.left(), p.y - draw.top()];
        let mut orbiting = false;
        // Un outil de dessin a été pris dans la vue : le pointeur dessine sur la carte du calque courant
        // (`espace3ddessin.rs`, partie 146). Alt+glisser tourne encore la vue ; les gestes de manipulation attendent
        // qu'un outil de la vue soit repris.
        // L'outil Trajectoire a été pris dans la vue : le pointeur trace, dans l'espace, le chemin de ce qui est choisi
        // (`trajectoire3d.rs`, partie 149). La navigation reste la même.
        // L'outil Sélection a été pris dans la vue : son lasso prend sur la carte du calque courant (`espace3dlasso.rs`,
        // partie 152). La navigation reste la même. (Dans la vue seule comme dans chacune des vues, partie 151.)
        let trace = self.e3_trace();
        let selectionne = self.e3_selectionne();
        let dessine = self.e3_dessine() || trace || selectionne;
        if dessine {
            let regard = crate::espace3ddessin::Regard3d { rect: draw, view };
            if trace {
                self.e3_trajectoire_pointeur(ctx, response, regard);
            } else if selectionne {
                self.e3_selection_pointeur(ctx, response, regard);
            } else {
                self.e3_dessin_pointeur(ctx, response, regard);
            }
            orbiting = matches!(self.espace3d.drag, Some(Drag::Orbit)) && response.dragged_by(PointerButton::Primary);
        }
        if !dessine && response.drag_started_by(PointerButton::Primary) {
            let at = press.or(response.interact_pointer_pos()).unwrap_or(draw.center());
            self.e3_press(&view, center, rel(at), mods);
        }
        if !dessine && response.dragged_by(PointerButton::Primary) {
            orbiting = matches!(self.espace3d.drag, Some(Drag::Orbit));
            let handling = matches!(self.espace3d.drag, Some(Drag::Handle { .. }));
            if handling {
                if let Some(p) = response.interact_pointer_pos() {
                    self.e3_drag(&view, center, rel(p));
                }
            }
        }
        if !dessine && response.clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                self.e3_click(&view, center, rel(p), mods);
            }
        }
        if !primary_down {
            self.e3_finish_drag();
        }
        // Survol d'une poignée (pour la mettre en évidence ; la barre d'état dit ce qu'elle fait, ou pourquoi elle ne fait rien).
        let avant = self.espace3d.hover;
        self.espace3d.hover = response.hover_pos().filter(|_| self.espace3d.drag.is_none() && !dessine).and_then(|p| self.e3_gizmo(&view)?.hit(&view, center, rel(p), GRAB));
        if self.espace3d.hover != avant {
            self.e3_survol_poignee();
        }
        orbiting
    }

    /// Un clic sans glisser : choisir la carte (ou la caméra) sous la souris, ou ne plus rien choisir.
    ///
    /// **La caméra l'emporte quand le pointeur est sur son dessin** (son corps, sa pyramide), même avec une carte derrière :
    /// il est tracé par-dessus les cartes, et une carte passe presque toujours derrière lui (le sol, sous une caméra vue de
    /// haut). C'est la règle du Ctrl+clic de l'outil Trajectoire (`e3_traj_choisir`, partie 152) ; avant, un clic simple
    /// prenait la carte de derrière.
    pub(crate) fn e3_click(&mut self, view: &View, center: [f32; 2], px: [f32; 2], mods: egui::Modifiers) {
        let ray = view.ray(px, center);
        let additive = mods.shift || mods.command;
        if self.e3_gizmo(view).is_some_and(|g| g.hit(view, center, px, GRAB).is_some()) {
            return;
        }
        // (Une vue orthographique montre par la tranche les cartes qui lui font face de côté : leur trait s'attrape aussi ; la caméra d'abord.)
        // (Les objets du décor 3D aussi, partie 184 : le plus proche de l'œil, carte ou objet, l'emporte.)
        let devant = !self.espace3d.through_camera && self.e3_pick_camera_devant(view, px, center);
        let hit = if devant { None } else { self.e3_pick_tout(&ray).or_else(|| self.e3_pick_tranche(view, center, px)) };
        let cam_hit = devant || (hit.is_none() && !self.espace3d.through_camera && self.e3_pick_camera(view, px, center));
        match (hit, cam_hit) {
            // Un objet du décor : choisi (Maj : ajouté, ou retiré), et choisi aussi dans la liste du panneau.
            (Some(id), _) if crate::poignees3d::objet_of(id).is_some() => {
                if let Some(oid) = crate::poignees3d::objet_of(id) {
                    self.e3_choisir_objet(oid, additive);
                }
            }
            (Some(id), _) => {
                if additive {
                    match self.espace3d.sel.iter().position(|s| *s == id) {
                        Some(p) => {
                            self.espace3d.sel.remove(p);
                        }
                        None => self.espace3d.sel.push(id),
                    }
                } else {
                    self.espace3d.sel = vec![id];
                    // Une carte choisie est aussi le calque courant : le panneau Calques la suit.
                    self.e3_follow_card(id);
                }
            }
            // (La caméra : seule, les cartes sont lâchées ; Maj ou Ctrl : elle rejoint ce qui est choisi, ou le quitte.)
            (None, true) => self.e3_choisir_la_camera(additive),
            (None, false) => {
                if !additive {
                    self.espace3d.sel.clear();
                }
            }
        }
    }

    /// Ce qui est choisi (la caméra mise à part) est composé de cartes.
    fn e3_only_cards(&self) -> bool {
        !self.espace3d.sel.is_empty() && self.espace3d.sel.iter().all(|&s| espace3d::card_of(s).is_some())
    }

    /// Ce qui est choisi ne peut pas bouger : des cartes dont les calques sont verrouillés, ou dont la position l'est
    /// (le message dit pourquoi).
    fn e3_frozen(&mut self) -> bool {
        let cards: Vec<usize> = self.espace3d.sel.iter().filter_map(|&s| self.e3_card_layer(s)).collect();
        let others = self.espace3d.sel.len() - cards.len();
        if cards.is_empty() || others > 0 {
            return false;
        }
        let stuck = |l: &crate::document::Layer| l.locked || l.position_locked;
        if cards.iter().all(|&li| stuck(&self.layers()[li])) {
            let l = &self.layers()[cards[0]];
            self.status = if l.locked { format!("Le calque « {} » est verrouillé.", l.name) } else { format!("La position du calque « {} » est verrouillée.", l.name) };
            return true;
        }
        false
    }

    /// Si `selection` est une carte : son calque devient le calque courant (le choix de l'espace est déjà fait).
    pub(crate) fn e3_follow_card(&mut self, selection: u64) {
        if let Some(li) = self.e3_card_layer(selection) {
            let id = self.layers()[li].id;
            if li != self.cur_layer {
                self.select_layer(li);
            }
            self.espace3d.last_layer = id;
        }
    }

    /// Début d'un glisser : une poignée, une carte qu'on tire directement, ou la vue qu'on tourne.
    pub(crate) fn e3_press(&mut self, view: &View, center: [f32; 2], px: [f32; 2], mods: egui::Modifiers) {
        self.end_stroke();
        let ray = view.ray(px, center);
        if mods.alt {
            self.espace3d.drag = Some(Drag::Orbit);
            return;
        }
        // Une poignée du manipulateur.
        if let Some(gizmo) = self.e3_gizmo(view) {
            if let Some(mut handle) = gizmo.hit(view, center, px, GRAB) {
                // (Une poignée qui ne peut rien ici, partie 184 : la hauteur d'un objet aimanté, un objet du décor qu'on
                // tournerait hors de la verticale, l'origine d'un calque hors de son dessin. La barre d'état dit pourquoi.)
                if let Some(raison) = self.e3_poignee_refusee(gizmo.mode, handle) {
                    self.status = raison;
                    return;
                }
                // « Modifier l'origine » : le geste déplace l'origine seule.
                if self.espace3d.origine {
                    self.e3_begin(handle, GizmoMode::Move, gizmo, ray, px, "Déplacer l'origine");
                    return;
                }
                // Plusieurs cartes, ou un objet du décor (son échelle est la même sur les trois axes) : échelle uniforme seulement.
                if gizmo.mode == GizmoMode::Scale && (self.espace3d.sel.len() > 1 || self.espace3d.sel.contains(&CAMERA_ID) || !self.e3_objets_choisis().is_empty()) {
                    handle = Handle::Center;
                }
                if gizmo.mode == GizmoMode::Scale && self.espace3d.sel.contains(&CAMERA_ID) {
                    self.espace3d.drag = Some(Drag::Orbit);
                    return;
                }
                // Une carte est plate : sa profondeur n'a pas de taille (la poignée Z de l'échelle ne fait rien).
                if gizmo.mode == GizmoMode::Scale && handle == Handle::Axis(2) && self.e3_only_cards() {
                    return;
                }
                if self.e3_frozen() {
                    return;
                }
                let objets = !self.e3_objets_choisis().is_empty() && self.espace3d.sel.iter().all(|&s| crate::poignees3d::objet_of(s).is_some());
                let name = match (gizmo.mode, objets) {
                    (GizmoMode::Rotate, false) => "Tourner le calque",
                    (GizmoMode::Scale, false) => "Mettre le calque à l'échelle",
                    (_, false) => "Déplacer le calque",
                    (GizmoMode::Rotate, true) => "Tourner un objet 3D",
                    (GizmoMode::Scale, true) => "Mettre un objet 3D à l'échelle",
                    (_, true) => "Déplacer un objet 3D",
                };
                self.e3_begin(handle, gizmo.mode, gizmo, ray, px, name);
                return;
            }
        }
        // Une carte (ou la caméra) tirée directement : elle glisse dans son plan (Maj : en profondeur). La caméra glisse
        // à l'horizontale (Maj : en hauteur). (La caméra d'abord, comme au clic : tirer son dessin la déplace, elle, même
        // avec une carte derrière. Une vue orthographique attrape aussi une carte par sa tranche.)
        let devant = !self.espace3d.through_camera && self.e3_pick_camera_devant(view, px, center);
        let hit = if devant { None } else { self.e3_pick_tout(&ray).or_else(|| self.e3_pick_tranche(view, center, px)) };
        let cam_hit = devant || (hit.is_none() && !self.espace3d.through_camera && self.e3_pick_camera(view, px, center));
        // « Modifier l'origine » (partie 184) : un appui hors des poignées choisit ce qu'il touche, comme un clic, sans rien
        // déplacer ; dans le vide, il tourne la vue comme d'habitude.
        if self.espace3d.origine && (hit.is_some() || cam_hit) {
            self.e3_click(view, center, px, mods);
            return;
        }
        if hit.is_some() || cam_hit {
            let id = hit.unwrap_or(CAMERA_ID);
            if !self.espace3d.sel.contains(&id) {
                if cam_hit {
                    // (Seule, comme au clic : `e3_choisir_la_camera`.)
                    self.e3_choisir_la_camera(false);
                } else if let Some(oid) = crate::poignees3d::objet_of(id) {
                    self.e3_choisir_objet(oid, false);
                } else {
                    self.espace3d.sel = vec![id];
                    self.e3_follow_card(id);
                }
            }
            // Un objet du décor tiré directement (partie 184) : il glisse sur le sol, l'aimant le repose ; Maj : en hauteur,
            // s'il n'est pas aimanté (l'aimant tient sa hauteur, comme le champ grisé du panneau).
            if let Some(oid) = crate::poignees3d::objet_of(id) {
                let xfs = self.e3_xfs(&self.espace3d.sel);
                let origin = self.e3_pivot(&xfs);
                let gizmo = Gizmo { origin, axes: gz::axes_for(false, &Default::default()), len: 1.0, mode: GizmoMode::Move };
                let aimante = self.e3_objet(oid).is_some_and(|o| o.aimante);
                let handle = if mods.shift && !aimante { Handle::Axis(1) } else { Handle::Plane(1) };
                if mods.shift && aimante {
                    self.status = crate::poignees3d::HAUTEUR_TENUE.into();
                }
                self.e3_begin(handle, GizmoMode::Move, gizmo, ray, px, "Déplacer un objet 3D");
                return;
            }
            if self.e3_frozen() {
                return;
            }
            let xfs = self.e3_xfs(&self.espace3d.sel);
            let origin = self.e3_pivot(&xfs);
            let gizmo = Gizmo { origin, axes: gz::axes_for(false, &xfs.first().map_or_else(Default::default, |x| x.1)), len: 1.0, mode: GizmoMode::Move };
            let is_card = espace3d::card_of(id).is_some();
            let handle = match (is_card, mods.shift) {
                (false, true) => Handle::Axis(1),
                (false, false) => Handle::Plane(1),
                (true, true) => Handle::Axis(2),
                (true, false) => Handle::Plane(2),
            };
            let name = if id == CAMERA_ID { "Déplacer la caméra" } else { "Déplacer le calque" };
            self.e3_begin(handle, GizmoMode::Move, gizmo, ray, px, name);
            return;
        }
        if !self.espace3d.through_camera {
            self.espace3d.drag = Some(Drag::Orbit);
        }
    }

    pub(crate) fn e3_begin(&mut self, handle: Handle, mode: GizmoMode, gizmo: Gizmo, start: Ray, px: [f32; 2], name: &'static str) {
        // (Une retouche du décor ouverte par le panneau se termine d'abord : son étape, puis celle du geste, partie 184.)
        if !self.e3_objets_choisis().is_empty() {
            self.decor3d_retouche_finir();
        }
        let originals = self.e3_xfs(&self.espace3d.sel);
        let snap = self.snapshot();
        let origine = if self.espace3d.origine { self.e3_geste_origine().map(Box::new) } else { None };
        // (« Modifier l'origine » sans origine à déplacer : aucun geste, surtout pas celui qui déplacerait l'objet.)
        if self.espace3d.origine && origine.is_none() {
            self.espace3d.drag = None;
            return;
        }
        self.espace3d.drag = Some(Drag::Handle { handle, mode, gizmo, start, start_px: px, originals, snap, changed: false, name, origine });
    }

    /// Le geste suit la souris : nouveaux placements, aimantés si l'aimantation est allumée.
    pub(crate) fn e3_drag(&mut self, view: &View, center: [f32; 2], px: [f32; 2]) {
        let ray = view.ray(px, center);
        // « Modifier l'origine » (partie 184) : l'origine seule bouge.
        if matches!(&self.espace3d.drag, Some(Drag::Handle { origine: Some(_), .. })) {
            self.e3_drag_origine(view, center, px);
            return;
        }
        let Some(Drag::Handle { handle, mode, gizmo, start, start_px, originals, .. }) = &self.espace3d.drag else { return };
        let (handle, mode, gizmo, start, start_px, originals) = (*handle, *mode, *gizmo, *start, *start_px, originals.clone());
        let snap_on = self.espace3d.snap;
        let (step_move, step_rot, step_scale) = (self.espace3d.step_move, self.espace3d.step_rot, self.espace3d.step_scale);
        let world_axes = gizmo.axes == [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let mut message = String::new();
        // Vu de la vue, au départ du geste : un plan de profil (le regard le longe), un axe de bout (le regard le suit). Une
        // vue orthographique les montre souvent ainsi (la vue Dessus et une carte dressée, la vue Face et le sol).
        let visee = start.d;
        let de_profil = |n: V3| m::dot(visee, n).abs() < 0.15;
        let de_bout = |a: V3| m::dot(visee, a).abs() > 0.97;
        if let (GizmoMode::Move | GizmoMode::Scale, Handle::Axis(k)) = (mode, handle) {
            if de_bout(gizmo.axes[k]) {
                self.status = format!("L'axe {} est vu de bout dans cette vue : ce geste ne le change pas. Prends une autre vue, ou glisse sans Maj.", ["X", "Y", "Z"][k]);
                return;
            }
        }
        // (Les axes qu'un glisser dans le plan de l'écran a bougés, pour l'aimantation.)
        let mut moved_ecran: Option<[bool; 3]> = None;
        let mut new: Originals = match (mode, handle) {
            (GizmoMode::Move, Handle::Axis(k)) => gz::move_xfs(&originals, m::drag_along_axis(&start, &ray, gizmo.origin, gizmo.axes[k])),
            (GizmoMode::Move, Handle::Plane(k)) => {
                let n = gizmo.axes[k];
                let d = if de_profil(n) {
                    // Le plan de glisse est vu de profil : on glisse dans le plan de l'écran, à la profondeur de l'objet.
                    moved_ecran = Some([0, 1, 2].map(|a| view.forward[a].abs() < 0.5));
                    match (view.screen_to_plane(start_px, center, gizmo.origin), view.screen_to_plane(px, center, gizmo.origin)) {
                        (Some(a), Some(b)) => m::sub(b, a),
                        _ => [0.0; 3],
                    }
                } else {
                    // (Le déplacement reste dans son plan : rien ne dérive le long de sa normale.)
                    let d = m::drag_in_plane(&start, &ray, gizmo.origin, n);
                    m::sub(d, m::scale(n, m::dot(d, n)))
                };
                gz::move_xfs(&originals, if d.iter().all(|v| v.is_finite()) { d } else { [0.0; 3] })
            }
            (GizmoMode::Rotate, Handle::Ring(k)) => {
                let axis = gizmo.axes[k];
                let mut angle = if m::dot(visee, axis).abs() < 0.2 {
                    // L'anneau est vu de profil (un trait) : l'angle que la souris fait à l'écran autour du centre projeté.
                    match view.project(gizmo.origin, center) {
                        Some(c) => {
                            let (a, b) = ([start_px[0] - c[0], start_px[1] - c[1]], [px[0] - c[0], px[1] - c[1]]);
                            if a[0].hypot(a[1]) < 2.0 || b[0].hypot(b[1]) < 2.0 {
                                0.0
                            } else {
                                // (L'écran a son y vers le bas : un tour dans le sens des aiguilles d'une montre est positif.)
                                let ecran = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).to_degrees();
                                if m::dot(axis, view.forward) > 0.0 { ecran } else { -ecran }
                            }
                        }
                        None => 0.0,
                    }
                } else {
                    m::drag_angle(&start, &ray, gizmo.origin, axis)
                };
                if !angle.is_finite() {
                    angle = 0.0;
                }
                if snap_on {
                    angle = m::snap(angle, step_rot);
                }
                message = format!("Rotation {angle:+.0}°");
                let mut out = gz::rotate_xfs(&originals, gizmo.origin, gizmo.axes[k], angle);
                // (Un objet du décor ne tourne qu'autour de la verticale : son angle suit le geste tel quel, sans repasser par
                // des angles d'Euler qui le ramèneraient entre −180° et 180°.)
                crate::poignees3d::tourner_les_objets(&originals, &mut out, gizmo.axes[k], angle);
                out
            }
            (GizmoMode::Scale, Handle::Axis(k)) => {
                let factor = m::drag_scale_axis(&start, &ray, gizmo.origin, gizmo.axes[k]);
                let mut out = gz::scale_xfs(&originals, gizmo.origin, Some(k), factor);
                if snap_on {
                    for (_, xf) in &mut out {
                        let s = m::snap(xf.size[k].abs(), step_scale).max(0.01);
                        xf.size[k] = s * xf.size[k].signum();
                    }
                }
                // (Une carte qui a une origine s'étire autour d'elle, partie 182 : son origine reste où elle était.)
                self.e3_garder_les_origines(&originals, &mut out);
                message = format!("Taille ×{factor:.2}");
                out
            }
            (GizmoMode::Scale, _) => {
                let factor = m::scale_from_screen(px[1] - start_px[1]);
                message = format!("Taille ×{factor:.2}");
                gz::scale_xfs(&originals, gizmo.origin, None, factor)
            }
            _ => return,
        };
        if mode == GizmoMode::Move {
            // Aimantation : la première carte tombe sur la grille (sur les axes que le geste a bougés), les autres suivent
            // le même décalage.
            if snap_on && world_axes {
                let moved = match handle {
                    _ if moved_ecran.is_some() => moved_ecran.unwrap_or_default(),
                    Handle::Axis(k) => [k == 0, k == 1, k == 2],
                    Handle::Plane(k) => [k != 0, k != 1, k != 2],
                    _ => [false; 3],
                };
                if let Some(first) = new.first().map(|n| n.1.pos) {
                    let snapped = m::snap_position(first, step_move, moved);
                    let fix = m::sub(snapped, first);
                    for (_, xf) in &mut new {
                        xf.pos = m::add(xf.pos, fix);
                    }
                }
            }
            if let Some((_, xf)) = new.first() {
                message = format!("Position {:.2} · {:.2} · {:.2} m", xf.pos[0], xf.pos[1], xf.pos[2]);
            }
        }
        self.e3_set_xfs(&new);
        if !message.is_empty() {
            self.status = message;
        }
        if let Some(Drag::Handle { changed, .. }) = &mut self.espace3d.drag {
            *changed = true;
        }
    }

    /// Fin du geste : l'étape d'annulation, si quelque chose a bougé.
    pub(crate) fn e3_finish_drag(&mut self) {
        if let Some(Drag::Handle { snap, changed: true, name, origine, originals, .. }) = self.espace3d.drag.take() {
            // L'origine déplacée (partie 184) : l'aimant la repose ; une étape d'annulation.
            if let Some(g) = origine {
                self.e3_origine_lachee(*g, snap, name);
                return;
            }
            // Un objet du décor a bougé : l'étape de la retouche du décor (le projet entier : l'annuler ramène l'objet et les
            // cartes aimantées qui reposaient sur lui).
            if originals.iter().any(|(id, _)| crate::poignees3d::objet_of(*id).is_some()) {
                self.commit_named(snap, name);
                return;
            }
            let before = self.history.context.replace(name.to_string());
            self.history.push(snap);
            self.history.context = before;
            self.modified = true;
        } else {
            self.espace3d.drag = None;
        }
    }

    /// La zone du dessin, en mode Espace 3D.
    pub(crate) fn espace3d_panel(&mut self, ui: &mut egui::Ui, rect: Rect, response: &egui::Response) {
        // (Plein écran ou à côté de la feuille : `espace3d_fit` cadre dans cette zone.)
        self.espace3d.rect = rect;
        if self.espace3d.cam.is_none() {
            self.espace3d_fit();
        }
        // Le calque courant a changé (panneau Calques, timeline) : sa carte est choisie. (À l'entrée dans l'espace aussi.)
        let current = self.layer().id;
        if self.espace3d.last_layer != current {
            self.espace3d.last_layer = current;
            if espace3d::is_card(self.layer()) && self.espace3d.drag.is_none() && self.espace3d.show_cards {
                self.espace3d.sel = vec![espace3d::card_selection(current)];
            }
        }
        // Les ombres des cartes sous le soleil du décor 3D (partie 189, `ombres3d.rs`) : les mêmes pour toutes les vues de cette image.
        // (Lâchées après les vues : elles tiennent les textures des cartes.)
        self.espace3d.ombres = self.e3_ombres_de_la_vue(ui.ctx());
        // Plusieurs vues (`espace3dvues.rs`) : chacune son point de vue, les mêmes cartes et la même sélection.
        if self.e3_vues_multiples() {
            self.e3_vues_panel(ui, rect);
            self.espace3d.ombres = None;
            return;
        }
        let through = self.espace3d.through_camera;
        let draw = if through { letterbox(rect, self.doc.frame) } else { rect };
        self.e3_input(ui.ctx(), draw, response);
        let cam_now = self.e3_world().scene_cam(&self.current_camera());
        let view = self.e3_view(draw);
        let center = [draw.width() / 2.0, draw.height() / 2.0];
        let painter = ui.painter_at(rect);
        let to_screen = |q: [f32; 2]| pos2(draw.left() + q[0], draw.top() + q[1]);
        let proj = |p: V3| view.project(p, center).map(to_screen);

        // Le fond : la couleur de l'environnement (`espace3ddecor.rs` ; d'origine, le gris de toujours).
        painter.rect_filled(rect, 0.0, self.e3_environnement().couleur_du_fond());
        // Le quadrillage du sol.
        if self.espace3d.show_grid {
            let distance = self.espace3d.cam.map_or(8.0, |c| c.distance);
            let target = self.espace3d.cam.map_or([0.0; 3], |c| c.target);
            // (Le même quadrillage que chacune des vues multiples, `e3_quadrillage`, à la hauteur du sol,
            // `e3_hauteur_sol`.)
            let lignes = self.e3_quadrillage(target, if through { 8.0 } else { distance }, None);
            Self::e3_peindre_quadrillage(&painter, &view, center, draw.left_top(), lignes);
        }

        // Le décor 3D de la séquence (partie 180), dessous ; les cartes s'y cachent selon leur distance (partie 183).
        let decor = self.e3_peindre_decor(&painter, draw, &view, 1).then_some((1, draw));
        // Les cartes (les calques du plan), du fond vers le dessus de la pile.
        self.e3_paint_cards(ui.ctx(), &painter, &view, center, &to_screen, decor);
        self.espace3d.ombres = None;
        if !through {
            self.e3_paint_camera(&painter, &proj, &view, &cam_now);
        }
        // (On dessine, on sélectionne ou on trace une trajectoire : pas de poignées sur la carte, elles ne répondraient pas.)
        let trace = self.e3_trace();
        let selectionne = self.e3_selectionne();
        let dessine = self.e3_dessine() || trace || selectionne;
        if let Some(g) = self.e3_gizmo(&view).filter(|_| !dessine) {
            self.e3_paint_gizmo(&painter, &proj, &g);
        }
        // Légendes.
        if through {
            painter.rect_stroke(draw, 0.0, Stroke::new(1.0_f32, crate::theme::camera()), egui::StrokeKind::Outside);
            painter.text(
                draw.left_top() + vec2(8.0, 6.0),
                Align2::LEFT_TOP,
                format!("Caméra du plan · {:.0} mm · {:.0}° · glisser (milieu) : déplacer · clic droit : tourner · molette : avancer", cam_now.focal, cam_now.angle_of_view()),
                FontId::proportional(11.0),
                crate::theme::camera(),
            );
        } else if !dessine {
            let legende = if self.espace3d.origine { crate::poignees3d::LEGENDE_ORIGINE } else { "Clic : choisir une carte, un objet du décor ou la caméra · glisser : déplacer (Maj : en profondeur) · Alt+glisser ou clic droit : tourner la vue · milieu : glisser la vue · molette : avancer" };
            painter.text(rect.left_bottom() + vec2(8.0, -6.0), Align2::LEFT_BOTTOM, legende, FontId::proportional(11.0), Color32::from_gray(170));
        }
        let label = format!("{} carte(s)", self.e3_visible_cards().len());
        painter.text(rect.right_top() + vec2(-8.0, 6.0), Align2::RIGHT_TOP, label, FontId::proportional(11.0), Color32::from_gray(170));
        // On dessine dans la vue : la pointe du trait, le cercle du pinceau posé sur la carte, la légende
        // (`espace3ddessin.rs`).
        if trace {
            // L'outil Trajectoire : le chemin de ce qui est choisi, ses marques, sa légende (`trajectoire3d.rs`).
            self.e3_trajectoire_peindre(ui.ctx(), &painter, response, rect, crate::espace3ddessin::Regard3d { rect: draw, view });
        } else if selectionne {
            // L'outil Sélection : le lasso, le contour et la boîte de ce qui est choisi, posés sur la carte (`espace3dlasso.rs`).
            self.e3_selection_peindre(ui.ctx(), &painter, response, rect, crate::espace3ddessin::Regard3d { rect: draw, view });
        } else if dessine {
            self.e3_dessin_peindre(ui.ctx(), &painter, response, rect, crate::espace3ddessin::Regard3d { rect: draw, view });
        }
        // Curseur.
        if response.hover_pos().is_some() && !dessine {
            let icon = match (&self.espace3d.drag, self.espace3d.hover) {
                (Some(Drag::Handle { .. }), _) if self.espace3d.origine => CursorIcon::Crosshair,
                (Some(_), _) => CursorIcon::Grabbing,
                (None, Some(h)) if self.e3_poignee_refusee(self.e3_gizmo(&view).map_or(GizmoMode::Move, |g| g.mode), h).is_some() => CursorIcon::NotAllowed,
                (None, Some(_)) if self.espace3d.origine => CursorIcon::Crosshair,
                (None, Some(_)) => CursorIcon::Grab,
                _ => CursorIcon::Default,
            };
            ui.ctx().set_cursor_icon(icon);
        }
    }

    /// Les cartes : pour chaque calque visible du plan, du fond vers le dessus de la pile (l'ordre de l'image), le voile
    /// de sa feuille, son dessin, son contour (en couleur de sélection si elle est choisie) et son nom.
    ///
    /// `decor` : la prise du décor 3D de cette vue et sa zone, si la carte graphique l'a dessiné. Chaque carte qui partage sa
    /// profondeur (`profondeur::partage`) reçoit alors, juste après son dessin, le voile du décor qui est devant elle
    /// (`decor3dgpu::VoileCarte`, partie 183).
    pub(crate) fn e3_paint_cards(&mut self, ctx: &egui::Context, painter: &egui::Painter, view: &View, center: [f32; 2], to_screen: &dyn Fn([f32; 2]) -> Pos2, decor: Option<(u64, Rect)>) {
        if !self.espace3d.show_cards {
            return;
        }
        let world = self.e3_world();
        let frame = self.cur_frame;
        let cible_egui = self.gpu.as_ref().map(|g| g.render_state().target_format);
        let mut rang = 0u32;
        let mut rang_recue = 0u32;
        for li in self.e3_cartes_a_tracer(view) {
            let layer = &self.layers()[li];
            let xf = world.carte(layer, frame);
            let (id, name, cel) = (layer.id, layer.name.clone(), layer.shown_at(frame));
            let opacity = (layer.opacity_at(frame) * 255.0).round() as u8;
            let selected = self.espace3d.sel.contains(&espace3d::card_selection(id));
            let mx = xf.matrix();
            let at = |u: f32, v: f32| m::point(&mx, [u - 0.5, 0.5 - v, 0.0]);
            // (Rien de la carte devant l'œil : pas de maillage du tout, `card_mesh`.)
            if let Some(voile) = card_mesh(view, center, to_screen, &at, None, crate::viewbar::veil(if selected { 46 } else { 30 })) {
                painter.add(Shape::mesh(voile));
                // Une sélection qu'on déplace sur cette carte (`espace3dlasso.rs`) : comme sur la feuille, le dessin sans
                // elle, puis elle seule, posée où le geste la met (`flottante.rs`). Sinon le dessin, tel quel.
                let flottante = cel.and_then(|c| self.e3_feuille_flottante(c));
                let image = match flottante {
                    Some(f) => Some(f.base),
                    None => cel,
                };
                // (Un nouveau dessin qui attend son image, `espace3dcote.rs` : la carte montre le dessin d'avant.)
                let tex = image.and_then(|c| self.e3_card_texture(ctx, c)).or_else(|| {
                    if flottante.is_some() {
                        return None;
                    }
                    let avant = self.espace3d.vu_par_calque.get(&id)?;
                    Some(self.espace3d.card_textures.get(avant)?.tex.id())
                });
                if let Some(c) = cel.filter(|c| self.espace3d.card_textures.get(c).is_some_and(|t| t.version == self.doc.cels.get(c).map_or(0, |d| d.version))) {
                    self.espace3d.vu_par_calque.insert(id, c);
                }
                if let Some(dessin) = tex.and_then(|tex| card_mesh(view, center, to_screen, &at, Some(tex), crate::viewbar::veil(opacity))) {
                    // Les ombres que la carte reçoit (partie 189) : retirées juste après son dessin, sur son maillage.
                    let recue = match (decor, tex) {
                        (Some((cle, zone)), Some(tex)) => self.e3_ombre_recue(li, &dessin, tex, &at, view, zone, cle, rang_recue),
                        _ => None,
                    };
                    painter.add(Shape::mesh(dessin));
                    if let Some((zone, r)) = recue {
                        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(zone, r));
                        rang_recue += 1;
                    }
                }
                if let Some(f) = flottante {
                    // (Le pixel (u, v) de la feuille, mené au point du dessin où le geste le met, puis sur la carte.)
                    let [w, h] = world.stage;
                    let sur_la_carte = |u: f32, v: f32| {
                        let p = f.aff.apply(egui::pos2(u * f.taille[0], v * f.taille[1]));
                        m::point(&mx, [p.x / w - 0.5, 0.5 - p.y / h, 0.0])
                    };
                    if let Some(feuille) = self.e3_card_texture(ctx, f.feuille).and_then(|tex| card_mesh(view, center, to_screen, &sur_la_carte, Some(tex), crate::viewbar::veil(opacity))) {
                        painter.add(Shape::mesh(feuille));
                    }
                }
                // La profondeur partagée avec le décor 3D (partie 183) : là où le décor est plus près que la carte, il la recouvre.
                if let (Some((cle, zone)), Some(cible_egui)) = (decor, cible_egui) {
                    if crate::profondeur::partage(&self.layers()[li]) {
                        let coins = world.card_corners(&xf);
                        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(zone, crate::decor3dgpu::VoileCarte { cle, rang, coins, cible_egui }));
                        rang += 1;
                    }
                }
            }
            // La grille de repère d'un calque 3D (`espace3ddecor.rs`, partie 157) : peinte sur la carte, par-dessus son dessin,
            // jamais dans le dessin.
            if let Some(g) = self.layers()[li].calque_3d.filter(|g| g.grille) {
                let lignes = crate::espace3ddecor::lignes_de_grille(world.stage, [xf.size[0].abs(), xf.size[1].abs()], g.pas);
                let [w, h] = world.stage;
                crate::espace3ddecor::peindre_grille(painter, &lignes, &|p: Pos2| view.project(at(p.x / w, p.y / h), center).map(to_screen));
            }
            let corners = world.card_corners(&xf);
            let stroke = if selected { Stroke::new(2.0_f32, ACCENT) } else { Stroke::new(1.0_f32, crate::viewbar::veil(90)) };
            for k in 0..4 {
                if let (Some(a), Some(b)) = (view.project(corners[k], center), view.project(corners[(k + 1) % 4], center)) {
                    painter.line_segment([to_screen(a), to_screen(b)], stroke);
                }
            }
            // L'origine de la carte choisie (partie 182) : là où est sa place, et autour d'elle qu'elle tourne.
            if selected {
                let o = world.origine_monde(&xf, world.ecart(&self.layers()[li]));
                if let Some(p) = view.project(o, center).map(to_screen) {
                    crate::origine::peindre_repere(painter, p, ACCENT, false);
                }
            }
            if let Some(p) = view.project(corners[0], center).map(to_screen) {
                let depth = world.profondeur(&self.layers()[li], frame);
                let label = if depth == 0.0 { name } else { format!("{name}  ({depth:+.2})") };
                let galley = painter.layout_no_wrap(label, FontId::proportional(11.0), if selected { ACCENT } else { Color32::from_gray(225) });
                let pos = p + vec2(4.0, -galley.size().y - 3.0);
                painter.rect_filled(Rect::from_min_size(pos - vec2(3.0, 1.0), galley.size() + vec2(6.0, 2.0)), 3.0, Color32::from_black_alpha(140));
                painter.galley(pos, galley, Color32::WHITE);
            }
        }
        self.e3_lacher_les_cliches(ctx.cumulative_pass_nr());
    }

    /// La caméra du plan : pyramide de vision, cadre, haut de l'image, et sa course pendant le plan si elle est animée.
    pub(crate) fn e3_paint_camera(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, view: &View, cam: &SceneCam) {
        let selected = self.espace3d.sel.contains(&CAMERA_ID);
        let color = if selected { ACCENT } else { crate::theme::camera() };
        // (Sa course : ses clés, et sa trajectoire dans l'espace si elle en suit une, partie 149.)
        if self.doc.panels[self.cur_panel].camera.len() >= 2 || self.doc.panels[self.cur_panel].trajectoire_camera.is_some() {
            let world = self.e3_world();
            let n = self.frame_count().max(1);
            let pts: Vec<Pos2> = (0..n).filter_map(|f| proj(world.eye(&self.doc.panel_camera_reelle(self.cur_panel, f)))).collect();
            painter.add(Shape::line(pts, Stroke::new(1.0_f32, color.gamma_multiply(0.7))));
        }
        let corners = self.e3_camera_corners(cam, view);
        let strong = Stroke::new(if selected { 2.0_f32 } else { 1.5 }, color);
        let line = |a: V3, b: V3, stroke: Stroke| {
            if let (Some(p), Some(q)) = (proj(a), proj(b)) {
                painter.line_segment([p, q], stroke);
            }
        };
        for k in 0..4 {
            line(cam.pos, corners[k], Stroke::new(1.0_f32, color.gamma_multiply(0.7)));
            line(corners[k], corners[(k + 1) % 4], strong);
        }
        // Un triangle au-dessus du cadre : le haut de l'image.
        let up = m::scale(m::add(corners[0], corners[1]), 0.5);
        let v = cam.view(self.doc.frame[0] as f32);
        let tip = m::add(up, m::scale(v.up, m::length(m::sub(corners[0], corners[1])) * 0.12));
        line(corners[0], tip, strong);
        line(corners[1], tip, strong);
        if let Some(p) = proj(cam.pos) {
            painter.circle(p, if selected { 6.0 } else { 4.5 }, color, Stroke::new(1.5_f32, Color32::from_black_alpha(160)));
            painter.text(p + vec2(9.0, -4.0), Align2::LEFT_BOTTOM, format!("Caméra · {:.0} mm", cam.focal), FontId::proportional(11.0), color);
        }
    }

    /// Les poignées du manipulateur.
    pub(crate) fn e3_paint_gizmo(&self, painter: &egui::Painter, proj: &dyn Fn(V3) -> Option<Pos2>, g: &Gizmo) {
        let hot = |h: Handle| self.espace3d.hover == Some(h) || matches!(&self.espace3d.drag, Some(Drag::Handle { handle, .. }) if *handle == h);
        // (Une poignée qui ne peut rien ici est grise, partie 184 : `e3_poignee_refusee`.)
        let color = |h: Handle, k: usize| if self.e3_poignee_refusee(g.mode, h).is_some() { Color32::from_gray(105) } else if hot(h) { HOT } else { AXIS_COLORS[k] };
        let Some(o) = proj(g.origin) else { return };
        match g.mode {
            GizmoMode::Select => {}
            GizmoMode::Move | GizmoMode::Scale => {
                for k in 0..3 {
                    let h = Handle::Axis(k);
                    let Some(t) = proj(g.axis_tip(k)) else { continue };
                    let c = color(h, k);
                    painter.line_segment([o, t], Stroke::new(if hot(h) { 3.0_f32 } else { 2.0 }, c));
                    if g.mode == GizmoMode::Move {
                        // Pointe de flèche.
                        let d = (t - o).normalized();
                        let n = vec2(-d.y, d.x);
                        painter.add(Shape::convex_polygon(vec![t + d * 9.0, t - d * 3.0 + n * 5.0, t - d * 3.0 - n * 5.0], c, Stroke::NONE));
                    } else {
                        painter.rect_filled(Rect::from_center_size(t, vec2(9.0, 9.0)), 1.0, c);
                    }
                }
                if g.mode == GizmoMode::Move {
                    for k in 0..3 {
                        let q = g.plane_quad(k).map(proj);
                        if let [Some(a), Some(b), Some(c), Some(d)] = q {
                            let col = color(Handle::Plane(k), k);
                            painter.add(Shape::convex_polygon(vec![a, b, c, d], col.gamma_multiply(0.35), Stroke::new(1.0_f32, col)));
                        }
                    }
                } else {
                    painter.rect_filled(Rect::from_center_size(o, vec2(11.0, 11.0)), 1.0, if hot(Handle::Center) { HOT } else { Color32::from_gray(225) });
                }
            }
            GizmoMode::Rotate => {
                for k in 0..3 {
                    let pts: Vec<Pos2> = g.ring_points(k, 64).into_iter().filter_map(|p| proj(p)).collect();
                    let h = Handle::Ring(k);
                    painter.add(Shape::line(pts, Stroke::new(if hot(h) { 3.0_f32 } else { 2.0 }, color(h, k))));
                }
            }
        }
    }
}

/// Distance du point `p` au segment [a, b] (tout en pixels).
fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (a, b) = (pos2(a[0], a[1]), pos2(b[0], b[1]));
    let (d, w) = (b - a, pos2(p[0], p[1]) - a);
    let l2 = d.length_sq();
    let t = if l2 > 0.0 { (w.dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (w - d * t).length()
}

/// Les carreaux d'une carte de la vue sur chaque côté (`card_mesh`) : le sommet k du maillage est le point (u, v) =
/// (k mod (n + 1), k div (n + 1)) / n de la carte (les ombres reçues le retrouvent, partie 189).
pub(crate) const SUBDIV_CARTE: usize = 16;

/// Une carte subdivisée : les points `at(u, v)` (u, v de 0 à 1, v vers le bas), texturée ou d'une couleur. La texture est
/// posée par triangles : sans subdivision, la perspective la déformerait. Un morceau qui passe derrière l'œil n'est pas
/// dessiné.
///
/// Rien (`None`) quand il ne reste devant l'œil **aucun carreau entier** : un maillage qui a des sommets et pas un
/// triangle ne doit jamais partir à la carte graphique. egui ne jette que les maillages sans sommet ni triangle ; celui-là
/// arrivait à wgpu, qui refuse une tranche vide (« Buffer slices can not be empty ») et arrêtait FEANOR — le plantage
/// « quand on se balade dans l'Espace 3D » (`docs/INTERFACE.md`, partie 147).
pub(crate) fn card_mesh(view: &View, center: [f32; 2], to_screen: &dyn Fn([f32; 2]) -> Pos2, at: &dyn Fn(f32, f32) -> V3, texture: Option<egui::TextureId>, color: Color32) -> Option<egui::Mesh> {
    const SUBDIV: usize = SUBDIV_CARTE;
    let mut mesh = egui::Mesh::with_texture(texture.unwrap_or_default());
    let n = SUBDIV;
    let mut front = Vec::with_capacity((n + 1) * (n + 1));
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let p = at(u, v);
            let ahead = view.to_view(p)[2] >= NEAR;
            front.push(ahead);
            let pos = if ahead { view.project(p, center).map_or(Pos2::ZERO, to_screen) } else { Pos2::ZERO };
            let uv = if texture.is_some() { pos2(u, v) } else { egui::epaint::WHITE_UV };
            mesh.vertices.push(Vertex { pos, uv, color });
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
    (!mesh.indices.is_empty()).then_some(mesh)
}
