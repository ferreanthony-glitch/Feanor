//! Les mouvements de caméra dans la vue Vignettes, à l'écran (Anthony, 2026-10-21). La logique est dans `vignettecam.rs`.
//!
//! **Ctrl + glisser** dans une case :
//! - case sans mouvement : le cadre d'origine reste **A** ; un nouveau cadre **B** suit le pointeur (le dedans le déplace,
//!   un coin le resserre ou l'élargit) ; au lâcher, deux clés de caméra sont posées (A à la première image du plan, B à la
//!   dernière) ;
//! - case avec mouvement : le bord d'un cadre (A, B, C…) le déplace, un de ses coins change sa taille (le zoom de la clé),
//!   et le dedans du dernier cadre en fait naître un nouveau (C, D…), les clés se répartissant sur la durée du plan.
//!
//! La case grandit en direct (elle prend les cases entières qu'il faut pour montrer tous les cadres) : le geste
//! modifie les clés du plan à chaque image et ne prend son étape d'annulation, nommée (« Mouvement de caméra »,
//! « Cadre C », « Zoom du cadre B »…), qu'au lâcher ; Échap remet les clés d'avant. Un Ctrl + clic qui ne bouge pas garde son
//! sens ancien : choisir des plans. Les cadres se dessinent fins, avec leur lettre, et une flèche va de l'un à
//! l'autre (la convention du storyboard).

use eframe::egui::{self, vec2, Color32, CursorIcon, FontId, Modifiers, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::board::{BoardLayout, CardPlace};
use crate::boardui::{Press, Xf, MOVE_THRESHOLD};
use crate::camera::{Camera, CameraKey};
use crate::history::Edit;
use crate::vignettecam::{self, Choix, Prise, Zone};

/// Distance (points d'écran) à laquelle un coin ou un bord de cadre se saisit.
const PRISE: f32 = 9.0;

/// Un geste Ctrl + glisser sur un cadre, de l'appui au lâcher.
pub(crate) struct CamGeste {
    pub plan: u64,
    pub prise: Prise,
    /// Le pointeur à l'appui et maintenant (écran).
    pub depart: Pos2,
    pub maintenant: Pos2,
    /// Le pointeur a quitté le seuil : le geste est un mouvement de caméra (sinon, c'est un Ctrl + clic).
    pub actif: bool,
    /// Échap : les clés d'avant sont remises, le lâcher ne fait rien.
    pub annule: bool,
    /// Les clés du plan à l'appui, et la caméra du début (A) quand il n'y a pas encore de mouvement.
    pub keys0: Vec<CameraKey>,
    pub a: Camera,
    /// Le coin saisi et le centre du cadre, sur la feuille (le zoom).
    pub coin: Pos2,
    pub centre: Pos2,
    /// Points d'écran par unité de la feuille, figés à l'appui (la case change de taille pendant le geste).
    pub echelle: f32,
    /// Le nombre de cadres avant le geste (zéro quand le plan n'a pas encore de mouvement).
    pub avant: usize,
    /// L'étape d'annulation, prise au premier mouvement.
    pub snap: Option<Edit>,
    /// Le Ctrl + clic d'avant : la vignette (rang sur la planche) et les touches, si le geste ne bouge pas.
    pub clic: (usize, Modifiers),
}

impl App {
    fn largeur_case(&self) -> f32 {
        self.layout.card_size.clamp(crate::board::CARD_MIN, crate::board::CARD_MAX)
    }

    /// Les quatre coins de chaque cadre de la case `place`, sur la planche (la grande case), ou ceux de la vignette (la
    /// case sans mouvement). Vrai pour le second cas.
    pub(crate) fn cadres_sur_la_planche(&self, place: &CardPlace) -> (Vec<[Pos2; 4]>, bool) {
        let cads = vignettecam::cadres(&self.doc, place.panel);
        match vignettecam::zone(&self.doc, place.panel, place.thumb.size(), self.largeur_case()) {
            Some(z) if !cads.is_empty() => (cads.iter().map(|c| c.coins.map(|p| z.vers_planche(place.thumb, p))).collect(), false),
            _ => (vec![vignettecam::coins_de(place.thumb)], true),
        }
    }

    /// Ctrl + appui sur la vignette `k` : prend un cadre si l'appui en saisit un. Faux : l'appui garde son sens (choisir
    /// des plans). Le Ctrl + clic reste un choix de plans tant que le pointeur ne bouge pas (`finish_cam_geste`).
    pub(crate) fn cam_press(&mut self, layout: &BoardLayout, xf: Xf, k: usize, pos: Pos2, m: Modifiers) -> bool {
        let place = layout.cards[k];
        if place.frame != 0 {
            return false;
        }
        let (cadres, simple) = self.cadres_sur_la_planche(&place);
        let tol = (PRISE / xf.zoom.max(1e-3)).min(place.thumb.width().min(place.thumb.height()) * 0.3);
        let Some(Choix { prise, rang, coin }) = vignettecam::choisit(&cadres, simple, xf.to_board(pos), tol) else { return false };
        let keys0 = self.doc.panels[place.panel].camera.clone();
        let a = keys0.first().map_or_else(|| self.doc.panel_camera(place.panel, 0), |k| k.cam);
        let cam = if simple { a } else { keys0.get(rang).map_or(a, |k| k.cam) };
        let frame = self.doc.frame;
        let echelle = xf.zoom * self.largeur_case() / vignettecam::taille_cadre(&self.doc, &a).x.max(1e-3);
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        self.vignettes.cam = Some(CamGeste {
            plan: place.plan,
            prise,
            depart: pos,
            maintenant: pos,
            actif: false,
            annule: false,
            keys0,
            a,
            coin: vignettecam::coin_de(frame, &cam, coin),
            centre: vignettecam::centre_de(frame, &cam),
            echelle,
            avant,
            snap: None,
            clic: (k, m),
        });
        true
    }

    /// À chaque image : le pointeur suit ; au-delà du seuil le geste devient un mouvement de caméra, et les clés du plan
    /// suivent le pointeur (calculées depuis celles d'avant le geste) ; Échap remet celles d'avant.
    pub(crate) fn update_cam_geste(&mut self, pointer: Option<Pos2>, escape: bool) {
        let Some(mut g) = self.vignettes.cam.take() else { return };
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) else { return };
        if escape && g.actif && !g.annule {
            self.doc.panels[pi].camera = g.keys0.clone();
            (g.annule, g.actif, g.snap) = (true, false, None);
            self.status = "Mouvement de caméra abandonné.".into();
        }
        if let Some(p) = pointer {
            g.maintenant = p;
            if !g.actif && !g.annule && g.depart.distance(p) > MOVE_THRESHOLD {
                if self.content_refuses() || !self.partage_peut_toucher(&[g.plan]) {
                    g.annule = true;
                    self.vignettes.press = Press::Dead;
                } else {
                    // Le plan devient courant ; l'étape d'annulation part de l'état d'avant le premier mouvement.
                    self.board_sel = vec![g.plan];
                    self.select_board(pi, 0);
                    g.snap = Some(self.snapshot());
                    g.actif = true;
                    self.vignettes.press = Press::Dead;
                }
            }
        }
        if g.actif {
            let delta = (g.maintenant - g.depart) / g.echelle.max(1e-6);
            let len = self.doc.panels[pi].frame_count();
            match vignettecam::applique(self.doc.frame, &g.keys0, g.a, g.prise, delta, g.coin, g.centre, len) {
                Some(keys) => {
                    self.doc.panels[pi].camera = keys;
                    self.modified = true;
                }
                None => {
                    self.doc.panels[pi].camera = g.keys0.clone();
                    self.status = "Ce plan est trop court pour un cadre de plus : allonge-le d'abord.".into();
                }
            }
        }
        self.vignettes.cam = Some(g);
    }

    /// Au lâcher : le mouvement prend son étape d'annulation, nommée ; un Ctrl + clic qui n'a pas bougé choisit des plans.
    pub(crate) fn finish_cam_geste(&mut self, layout: &BoardLayout, any_down: bool) {
        if any_down {
            return;
        }
        let Some(mut g) = self.vignettes.cam.take() else { return };
        if g.annule {
            return;
        }
        if !g.actif {
            let (k, m) = g.clic;
            if k < layout.cards.len() {
                self.board_select_click(layout, k, m);
            }
            return;
        }
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) else { return };
        let Some(snap) = g.snap.take() else { return };
        let changed = self.doc.panels[pi].camera != g.keys0;
        if !changed {
            return;
        }
        let nom = vignettecam::nom_etape(g.prise, g.avant);
        let avant = self.history.context.replace(nom.clone());
        self.history.push(snap);
        self.history.context = avant;
        self.modified = true;
        let n = self.doc.panels[pi].camera.len();
        let dernier = vignettecam::lettre(n.saturating_sub(1));
        self.status = match g.prise {
            Prise::Nouveau | Prise::NouveauZoom => {
                format!("Cadre {dernier} posé : la caméra va de A à {dernier} sur la durée du plan ({n} cadres, clés réparties sur le plan).")
            }
            Prise::Deplace(i) => format!("Cadre {} déplacé.", vignettecam::lettre(i)),
            Prise::Zoom(i) => format!("Cadre {} : zoom changé.", vignettecam::lettre(i)),
        };
    }

    /// Le curseur d'une case où Ctrl est tenue : un cadre à prendre (déplacer ou, à un coin, changer sa taille).
    pub(crate) fn cam_curseur(&self, ctx: &egui::Context, layout: &BoardLayout, xf: Xf, pointer: Pos2) {
        let Some(place) = layout.cards.iter().find(|c| c.frame == 0 && c.thumb.contains(xf.to_board(pointer))) else { return };
        let (cadres, simple) = self.cadres_sur_la_planche(place);
        let tol = (PRISE / xf.zoom.max(1e-3)).min(place.thumb.width().min(place.thumb.height()) * 0.3);
        let icon = match vignettecam::choisit(&cadres, simple, xf.to_board(pointer), tol) {
            Some(Choix { prise: Prise::Zoom(_) | Prise::NouveauZoom, .. }) => CursorIcon::ResizeNwSe,
            Some(_) => CursorIcon::Move,
            None => return,
        };
        ctx.set_cursor_icon(icon);
    }

    /// Les cadres du mouvement par-dessus la case : fins, avec leur lettre, et une flèche de l'un à l'autre. Discrets
    /// (couleur de la caméra du thème) ; rien tant que le plan n'a pas deux clés.
    pub(crate) fn paint_cadres(&self, painter: &Painter, view: Rect, xf: Xf, place: &CardPlace, zone: &Zone) {
        let cads = vignettecam::cadres(&self.doc, place.panel);
        if cads.len() < 2 {
            return;
        }
        let clip = painter.with_clip_rect(xf.rect(place.thumb).intersect(view));
        let teinte = crate::theme::camera();
        let encre = |alpha: u8| Color32::from_rgba_unmultiplied(teinte.r(), teinte.g(), teinte.b(), alpha);
        let courant = self.cur_panel == place.panel;
        let sur_ecran = |p: Pos2| xf.to_screen(zone.vers_planche(place.thumb, p));
        let n = cads.len();
        for (i, c) in cads.iter().enumerate() {
            let pts: Vec<Pos2> = c.coins.iter().map(|p| sur_ecran(*p)).collect();
            // Un liseré clair sous le trait : il se lit sur le papier comme sur le dessin.
            clip.add(Shape::closed_line(pts.clone(), Stroke::new(2.5_f32, Color32::from_white_alpha(120))));
            clip.add(Shape::closed_line(pts.clone(), Stroke::new(1.0_f32, encre(if courant { 230 } else { 190 }))));
            let lettre = vignettecam::lettre(i).to_string();
            let font = FontId::proportional(11.0);
            let at = pts[0] + vec2(3.0, 2.0);
            let galley = clip.layout_no_wrap(lettre, font, Color32::WHITE);
            let r = Rect::from_min_size(at - vec2(2.0, 1.0), galley.size() + vec2(4.0, 2.0));
            clip.rect_filled(r, 2.0, encre(215));
            clip.galley(at, galley, Color32::WHITE);
            // La flèche vers le cadre suivant : de centre à centre, raccourcie pour ne pas cacher les lettres.
            if i + 1 < n {
                let (a, b) = (sur_ecran(c.centre), sur_ecran(cads[i + 1].centre));
                let d = b - a;
                if d.length() > 14.0 {
                    let u = d.normalized();
                    let (a, b) = (a + u * 6.0, b - u * 6.0);
                    clip.add(Shape::line_segment([a, b], Stroke::new(1.5_f32, encre(235))));
                    let perp = vec2(-u.y, u.x);
                    clip.add(Shape::convex_polygon(vec![b, b - u * 8.0 + perp * 4.0, b - u * 8.0 - perp * 4.0], encre(235), Stroke::NONE));
                }
            }
        }
    }
}
