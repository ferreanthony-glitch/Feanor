//! Les mouvements de caméra dans la vue Vignettes, à l'écran (Anthony, 2026-10-21). La logique est dans `vignettecam.rs`.
//!
//! **Ctrl + glisser** (ou **T tenue**, `vignettes.camera`, comme Espace tient la main) dans une case, de son image, de son empreinte
//! ou de sa marge, quel que soit l'outil : c'est **toujours** un geste de caméra, jamais un plan qu'on déplace ni qu'on choisit
//! (un Ctrl + clic qui ne bouge pas choisit des plans comme avant ; Maj + glisser le cadre d'une case déplace son plan) :
//! - case sans mouvement : le cadre d'origine reste **A** ; un nouveau cadre **B** suit le pointeur (le dedans le déplace, un coin
//!   le resserre ou l'élargit) ; au lâcher, deux clés de caméra sont posées (A à la première image du plan, B à la dernière) ;
//! - case avec des cadres : le dedans d'un cadre **le déplace** (le plus petit qui contient le point : B dans A, dedans B on déplace B),
//!   un de ses coins change sa taille (le zoom de la clé). Rien ne naît d'un glissé simple ;
//! - **Ctrl + Alt + glisser** (ou T + Alt) : un cadre de plus (C, D…), qui part du dernier ; **« Ajouter un cadre »** au clic droit
//!   de la case en pose un à côté du dernier ; **« Retirer ce cadre »** au clic droit sur un cadre.
//!
//! L'empreinte de la case suit en direct (les cadres à l'échelle des cases, les autres cases se rangent : la largeur réservée ne
//! rétrécit pas tant que le geste dure, `board::Reserve`) : le geste modifie les clés du plan à chaque image et ne prend son étape
//! d'annulation, nommée (« Mouvement de caméra », « Cadre C », « Zoom du cadre B »…), qu'au lâcher ; Échap remet les clés d'avant.
//! Les cadres se dessinent fins, en bleu, avec leur lettre, et se relient (flèche d'un travelling, coins d'un zoom ou d'une diagonale).

use eframe::egui::{self, vec2, Align2, Color32, CursorIcon, FontId, Modifiers, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::board::{self, BoardLayout, CardPlace, MoveBoard, Reserve};
use crate::boardui::{Press, Xf, MOVE_THRESHOLD};
use crate::camera::{Camera, CameraKey};
use crate::history::Edit;
use crate::vignettecam::{self, Choix, Grille, Lien, Prise};

/// Distance (points d'écran) à laquelle un coin de cadre se saisit.
const PRISE: f32 = 9.0;

/// Le bleu des cadres de caméra : fin, le même sur le papier et sur le dessin.
pub const BLEU: Color32 = Color32::from_rgb(0x37, 0x8A, 0xDD);

/// Un geste Ctrl + glisser sur un cadre, de l'appui au lâcher.
pub(crate) struct CamGeste {
    pub plan: u64,
    /// L'image du plan où commence la case prise (0 : la première case ; sinon une marque d'image).
    pub image: usize,
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
    /// Points d'écran par point de la planche à l'appui, et points de la planche par unité de la feuille.
    pub zoom: f32,
    pub k: f32,
    /// L'aimant (`vignettecam::aimante`) : la boîte du cadre qu'on déplace, relative au coin haut-gauche de A (planche), et les
    /// alignements qui l'attirent, très près seulement. Rien pour un zoom, ni quand on déplace A.
    pub rect0: Rect,
    pub grille: Grille,
    pub aimante: bool,
    /// Le nombre de cadres avant le geste (zéro quand le plan n'a pas encore de mouvement).
    pub avant: usize,
    /// L'étape d'annulation, prise au premier mouvement.
    pub snap: Option<Edit>,
    /// Le Ctrl + clic d'avant : la vignette (rang sur la planche) et les touches, si le geste ne bouge pas.
    pub clic: (usize, Modifiers),
    /// La place que l'empreinte a prise pendant le geste : les autres cases ne la regagnent qu'au lâcher.
    pub hold: Option<Reserve>,
}

impl App {
    fn largeur_case(&self) -> f32 {
        self.layout.card_size.clamp(crate::board::CARD_MIN, crate::board::CARD_MAX)
    }

    /// La marge autour d'une case où un appui de caméra la prend encore (3 % de sa largeur, au moins 6 points : moins que la marge de
    /// trait de 12 %, pour que deux cases voisines ne se disputent pas l'appui), en points de la planche.
    pub(crate) fn marge_camera(&self) -> f32 {
        (self.largeur_case() * 0.03).max(6.0)
    }

    /// Les quatre coins de chaque cadre de la case `k`, sur la planche, ou ceux de la vignette (la case sans mouvement). Vrai
    /// pour le second cas.
    pub(crate) fn cadres_sur_la_planche(&self, layout: &BoardLayout, k: usize) -> (Vec<[Pos2; 4]>, bool) {
        let place = &layout.cards[k];
        match layout.move_of(k) {
            Some(m) if !m.cadres.is_empty() => (m.quads(), false),
            _ => (vec![vignettecam::coins_de(place.thumb)], true),
        }
    }

    /// Un mouvement de caméra peut-il se dessiner sur cette case ? Sa première : toujours ; une autre case du plan (une
    /// marque d'image) : seulement si le plan ne bouge pas encore.
    pub(crate) fn cam_possible(&self, place: &CardPlace) -> bool {
        place.frame == 0 || self.doc.panels.get(place.panel).is_some_and(|p| p.camera.len() < 2)
    }

    /// La tolérance, sur la planche, à laquelle un coin se saisit.
    fn tol_coin(&self, xf: Xf, place: &CardPlace) -> f32 {
        (PRISE / xf.zoom.max(1e-3)).min(place.thumb.width().min(place.thumb.height()) * 0.3)
    }

    /// Appui de caméra (Ctrl, ou T tenue) sur la case `k` : prend le cadre que l'appui saisit. Faux seulement si la case ne peut pas
    /// porter de mouvement (une marque d'image d'un plan qui bouge déjà) : l'appui garde alors son sens (choisir des plans). Le
    /// Ctrl + clic reste un choix de plans tant que le pointeur ne bouge pas (`finish_cam_geste`). `ajoute` : Alt est tenue, un
    /// cadre de plus part du dernier.
    pub(crate) fn cam_press(&mut self, layout: &BoardLayout, xf: Xf, k: usize, pos: Pos2, m: Modifiers, ajoute: bool) -> bool {
        let place = layout.cards[k];
        // Le mouvement d'un plan se dessine sur sa première case ; sur une autre case du plan (une marque d'image), le
        // geste ne vaut que tant que le plan n'a pas de mouvement : la case entière est alors le cadre A (la caméra du
        // plan), comme une case ordinaire. Un plan qui bouge déjà se règle sur sa première case, où ses cadres sont.
        if !self.cam_possible(&place) {
            self.status = "Le mouvement de caméra de ce plan se règle sur sa première case, où sont ses cadres.".into();
            return false;
        }
        let (cadres, simple) = self.cadres_sur_la_planche(layout, k);
        let tol = self.tol_coin(xf, &place);
        let Some(Choix { prise, rang, coin }) = vignettecam::choisit(&cadres, simple, xf.to_board(pos), tol, ajoute) else { return false };
        let keys0 = self.doc.panels[place.panel].camera.clone();
        let a = keys0.first().map_or_else(|| self.doc.panel_camera(place.panel, 0), |k| k.cam);
        let cam = if simple { a } else { keys0.get(rang).map_or(a, |k| k.cam) };
        let frame = self.doc.frame;
        let k_feuille = self.largeur_case() / vignettecam::taille_cadre(&self.doc, &a).x.max(1e-3);
        let echelle = xf.zoom * k_feuille;
        // L'aimant : ce qui attire le cadre qu'on déplace (A : jamais ; un zoom : non). Tout est relatif au coin haut-gauche de A.
        let au = place.thumb.min.to_vec2();
        let mobile = match prise {
            Prise::Deplace(i) | Prise::Zoom(i) => i.min(cadres.len() - 1),
            Prise::Nouveau | Prise::NouveauZoom => cadres.len() - 1,
        };
        let rect0 = vignettecam::boite(&cadres[mobile]).translate(-au);
        let aimante = matches!(prise, Prise::Nouveau | Prise::Deplace(1..));
        let mut grille = Grille::default();
        for q in &cadres {
            let r = vignettecam::boite(q).translate(-au);
            grille.xs.extend([r.left(), r.center().x, r.right()]);
            grille.ys.extend([r.top(), r.center().y, r.bottom()]);
        }
        // Les colonnes de la planche (un pas de case et d'écart de coupe, depuis A) et ses rangées (le haut et le bas des vignettes).
        let pas = place.thumb.width() + board::gap_cut(place.thumb.width());
        for c in -6..=6 {
            grille.xs.extend([c as f32 * pas, c as f32 * pas + place.thumb.width()]);
        }
        for c in &layout.cards {
            grille.ys.extend([c.thumb.top() - au.y, c.thumb.bottom() - au.y]);
        }
        grille.ys.sort_by(f32::total_cmp);
        grille.ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        self.vignettes.cam = Some(CamGeste {
            plan: place.plan,
            image: place.frame,
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
            zoom: xf.zoom,
            k: k_feuille,
            rect0,
            grille,
            aimante,
            avant,
            snap: None,
            clic: (k, m),
            hold: None,
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
            g.hold = None;
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
                    self.select_board(pi, g.image);
                    g.snap = Some(self.snapshot());
                    g.actif = true;
                    self.vignettes.press = Press::Dead;
                }
            }
        }
        if g.actif {
            let delta = if g.aimante {
                // L'aimant : en points de la planche, à quelques points d'écran près d'un alignement, le cadre y colle (doucement :
                // aucun saut en y entrant ni en en sortant) ; partout ailleurs il suit le pointeur exactement.
                let libre = (g.maintenant - g.depart) / g.zoom.max(1e-6);
                vignettecam::aimante(g.rect0, libre, &g.grille, vignettecam::AIMANT / g.zoom.max(1e-6)) / g.k.max(1e-6)
            } else {
                (g.maintenant - g.depart) / g.echelle.max(1e-6)
            };
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

    /// Pendant le geste, l'empreinte ne rend pas sa place aux autres cases : on retient l'étendue qu'elle a prise (`layout` : la
    /// planche de cette image d'écran, déjà rangée avec la réserve d'avant).
    pub(crate) fn cam_garder_la_place(&mut self, layout: &BoardLayout) {
        let Some(g) = self.vignettes.cam.as_mut().filter(|g| g.actif && !g.annule) else { return };
        let Some(k) = layout.cards.iter().position(|c| c.plan == g.plan && c.frame == 0) else { return };
        if let Some(r) = Reserve::depuis(layout, k) {
            g.hold = Some(g.hold.map_or(r, |h| h.avec(r)));
        }
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
            if k < layout.cards.len() && (m.ctrl || m.command) {
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
        self.etape_camera(snap, &nom);
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

    /// Prend l'étape d'annulation `snap`, sous le nom `nom`.
    fn etape_camera(&mut self, snap: Edit, nom: &str) {
        let avant = self.history.context.replace(nom.to_string());
        self.history.push(snap);
        self.history.context = avant;
        self.modified = true;
    }

    /// « Ajouter un cadre » (clic droit sur la case du plan `pi`) : un cadre de plus à côté du dernier, qu'on prend ensuite pour le
    /// glisser où l'on veut. Une étape d'annulation.
    pub(crate) fn cam_ajouter_cadre(&mut self, pi: usize) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let keys0 = self.doc.panels[pi].camera.clone();
        let len = self.doc.panels[pi].frame_count();
        let a = keys0.first().map_or_else(|| self.doc.panel_camera(pi, 0), |k| k.cam);
        let base = if keys0.len() >= 2 { keys0[keys0.len() - 1].cam } else { a };
        let nouveau = vignettecam::cadre_suivant(self.doc.frame, base, vignettecam::taille_cadre(&self.doc, &base));
        let Some(keys) = vignettecam::avec_nouveau_cadre(&keys0, a, nouveau, len) else {
            self.status = "Ce plan est trop court pour un cadre de plus : allonge-le d'abord.".into();
            return;
        };
        let snap = self.snapshot();
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        self.doc.panels[pi].camera = keys;
        let nom = vignettecam::nom_etape(Prise::Nouveau, avant);
        self.etape_camera(snap, &nom);
        let n = self.doc.panels[pi].camera.len();
        self.status = format!("Cadre {} ajouté : tire-le avec Ctrl.", vignettecam::lettre(n - 1));
    }

    /// « Retirer ce cadre » : la clé `rang` du plan `pi` s'en va. Retirer B d'un mouvement à deux cadres enlève le mouvement.
    pub(crate) fn cam_retirer_cadre(&mut self, pi: usize, rang: usize) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let Some(keys) = vignettecam::sans_le_cadre(&self.doc.panels[pi].camera, rang, self.doc.panels[pi].frame_count()) else { return };
        let snap = self.snapshot();
        let reste = keys.len();
        self.doc.panels[pi].camera = keys;
        let nom = format!("Retirer le cadre {}", vignettecam::lettre(rang));
        self.etape_camera(snap, &nom);
        self.status = if reste < 2 { "Mouvement de caméra retiré.".into() } else { format!("Cadre {} retiré.", vignettecam::lettre(rang)) };
    }

    /// Le curseur d'une case où Ctrl (ou T) est tenue : un cadre à prendre (déplacer, ou changer sa taille à un coin), et une petite
    /// caméra à côté du pointeur. `ajoute` : Alt est aussi tenue (un cadre de plus).
    pub(crate) fn cam_curseur(&self, ctx: &egui::Context, layout: &BoardLayout, xf: Xf, pointer: Pos2, ajoute: bool) {
        let p = xf.to_board(pointer);
        let Some(k) = layout.card_near(p, self.marge_camera()).filter(|&k| self.cam_possible(&layout.cards[k])) else { return };
        let (cadres, simple) = self.cadres_sur_la_planche(layout, k);
        let tol = self.tol_coin(xf, &layout.cards[k]);
        let Some(choix) = vignettecam::choisit(&cadres, simple, p, tol, ajoute) else { return };
        let icon = match choix {
            Choix { prise: Prise::Zoom(_) | Prise::NouveauZoom, coin, .. } => {
                if coin % 2 == 0 {
                    CursorIcon::ResizeNwSe
                } else {
                    CursorIcon::ResizeNeSw
                }
            }
            _ => CursorIcon::Move,
        };
        ctx.set_cursor_icon(icon);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("curseur_camera")));
        let at = pointer + vec2(13.0, 11.0);
        let texte = if ajoute { format!("{}+", egui_phosphor::regular::VIDEO_CAMERA) } else { egui_phosphor::regular::VIDEO_CAMERA.to_string() };
        painter.text(at + vec2(1.0, 1.0), Align2::LEFT_TOP, &texte, FontId::proportional(15.0), Color32::from_white_alpha(200));
        painter.text(at, Align2::LEFT_TOP, texte, FontId::proportional(15.0), BLEU);
    }

    /// Ce qui se dessine par-dessus la tuile d'une empreinte : les cadres fins en bleu avec leur lettre (tournés si la caméra l'est),
    /// et ce qui les relie : les côtés de l'enveloppe entre deux cadres (la silhouette), jamais une ligne qui traverse l'intérieur, et la
    /// flèche d'un travelling. Le dessin reste net partout dans l'enveloppe (Anthony, 2026-10-22 : plus de voile sur les traits).
    pub(crate) fn paint_empreinte(&self, painter: &Painter, view: Rect, xf: Xf, place: &CardPlace, mv: &MoveBoard) {
        let courant = self.cur_panel == place.panel;
        let clip = painter.with_clip_rect(xf.rect(place.foot).expand(40.0).intersect(view));
        let alpha = |a: u8| Color32::from_rgba_unmultiplied(BLEU.r(), BLEU.g(), BLEU.b(), a);
        let (trait_, halo) = (if courant { 1.4_f32 } else { 1.1_f32 }, Stroke::new(3.0_f32, Color32::from_white_alpha(110)));
        let distincts = vignettecam::cadres_distincts(&mv.quads());
        for (c, _) in mv.cadres.iter().zip(&distincts).filter(|(_, d)| **d) {
            let q: Vec<Pos2> = c.coins.iter().map(|p| xf.to_screen(*p)).collect();
            if !vignettecam::boite(&q).intersects(view) {
                continue;
            }
            clip.add(Shape::closed_line(q.clone(), halo));
            clip.add(Shape::closed_line(q.clone(), Stroke::new(trait_, alpha(if courant { 245 } else { 215 }))));
            let lettre = vignettecam::lettre(c.rang).to_string();
            clip.text(q[0] + vec2(3.0, 1.0), Align2::LEFT_TOP, lettre, FontId::proportional(11.0), alpha(250));
        }
        let s = |p: Pos2| xf.to_screen(p);
        let fin = Stroke::new(1.0_f32, alpha(225));
        for lien in vignettecam::liens(&mv.quads(), place.thumb) {
            match lien {
                Lien::Bord(de, vers) => {
                    clip.line_segment([s(de), s(vers)], fin);
                }
                Lien::FlecheDessus { de, vers } | Lien::FlecheVerticale { de, vers } => {
                    let (a, b) = (s(de), s(vers));
                    clip.line_segment([a, b], Stroke::new(1.0_f32, alpha(235)));
                    let d = b - a;
                    if d.length() > 2.0 {
                        let u = d.normalized();
                        let perp = vec2(-u.y, u.x);
                        clip.add(Shape::convex_polygon(vec![b, b - u * 7.0 + perp * 3.5, b - u * 7.0 - perp * 3.5], alpha(235), Stroke::NONE));
                    }
                }
            }
        }
    }
}