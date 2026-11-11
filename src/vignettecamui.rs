//! Les mouvements de caméra dans la vue Vignettes, à l'écran (Anthony, 2026-10-21). La logique est dans `vignettecam.rs`.
//!
//! **Ctrl + Maj + glisser** (ou **T tenue**, `vignettes.camera`, comme Espace tient la main) dans une case, de son image, de son empreinte
//! ou de sa marge, quel que soit l'outil : c'est **toujours** un geste de caméra, jamais un plan qu'on déplace ni qu'on choisit, ni un
//! lasso (Ctrl seul est le lasso ; un Ctrl + Maj + clic qui ne bouge pas ne fait rien ; Maj + glisser le cadre d'une case déplace son plan) :
//! - case sans mouvement : le cadre d'origine reste **A** ; un nouveau cadre **B** suit le pointeur (le dedans le déplace, un coin
//!   le resserre ou l'élargit) ; au lâcher, deux clés de caméra sont posées (A à la première image du plan, B à la dernière) ;
//! - case avec des cadres : le dedans d'un cadre **le déplace** (le plus petit qui contient le point : B dans A, dedans B on déplace B),
//!   un de ses coins change sa taille (le zoom de la clé). Rien ne naît d'un glissé simple ;
//! - **« Ajouter un cadre »** au clic droit : un cadre de plus (C, D…), qui part du dernier ; le geste avec Alt est retiré depuis la partie 113 ;
//!   de la case en pose un à côté du dernier ; **« Retirer ce cadre »** au clic droit sur un cadre.
//!
//! L'empreinte de la case suit en direct (les cadres à l'échelle des cases, les autres cases se rangent : la largeur réservée ne
//! rétrécit pas tant que le geste dure, `board::Reserve`) : le geste modifie les clés du plan à chaque image et ne prend son étape
//! d'annulation, nommée (« Mouvement de caméra », « Cadre C », « Zoom du cadre B »…), qu'au lâcher ; Échap remet les clés d'avant.
//! Les cadres se dessinent fins, en bleu, avec leur lettre, et se relient (flèche d'un travelling, coins d'un zoom ou d'une diagonale).

use eframe::egui::{self, vec2, Align2, Color32, CursorIcon, FontId, Painter, Pos2, Rect, Shape, Stroke};

use crate::app::App;
use crate::board::{self, BoardLayout, CardPlace, MoveBoard, Reserve};
use crate::boardui::{Press, Xf, MOVE_THRESHOLD};
use crate::camera::{Camera, CameraKey};
use crate::history::Edit;
use crate::vignettecam::{self, Choix, Grille, Lien, Prise};

/// Distance (points d'écran) à laquelle un coin de cadre se saisit.
const PRISE: f32 = 9.0;

/// (Le bleu des cadres de caméra, fin, le même sur le papier et sur le dessin, est un réglage : `App::cadre_couleur`, Préférences > Vignettes.)

/// Un geste Ctrl + Maj + glisser (ou T tenue) sur un cadre, de l'appui au lâcher.
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
    /// Les clés du plan telles qu'elles étaient à l'appui, avant `recale` (Échap les remet, le lâcher les compare).
    pub orig: Vec<CameraKey>,
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
    /// La place que l'empreinte a prise pendant le geste : les autres cases ne la regagnent qu'au lâcher.
    pub hold: Option<Reserve>,
    /// Où était la case A à l'appui (sa rangée, son x) : elle y reste tant que le geste dure (`Reserve::pose`).
    pub pose: Option<(usize, f32)>,
    /// Lue à la japonaise : le haut de l'image de la case A à l'appui, depuis le haut des colonnes (`Reserve::pose_y`).
    pub pose_y: Option<f32>,
    /// Le retrait de A à l'appui, sous la ligne des images de sa rangée (à la japonaise : à droite du bord de sa colonne) : tenu pendant le
    /// geste (`Reserve::pose_dec`).
    pub pose_dec: Option<f32>,
    /// (Partie 142) Le mouvement que le geste retouche : l'image du plan où commence la case qui le porte (0 : la première case), ses clés
    /// parmi celles du plan à l'appui (`rangs` ; vide : la case n'en porte pas encore), le rang de la lettre de son premier cadre, et sur
    /// combien d'images ses cadres se répartissent. `keys0` sont **ses** clés, comptées depuis `debut`.
    pub debut: usize,
    pub rangs: std::ops::Range<usize>,
    pub lettre0: usize,
    pub len: usize,
}

/// Le mouvement d'une case (partie 142, `vignettecam::groupes`) : celui qu'elle porte, ou celui qu'elle peut commencer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MouvementCase {
    /// L'image du plan où commence la case.
    pub debut: usize,
    /// Ses clés parmi celles du plan (vide : la case ne porte pas encore de mouvement ; c'est alors là qu'elles iront).
    pub rangs: std::ops::Range<usize>,
    /// Le rang de la lettre de son premier cadre (A : 0).
    pub lettre0: usize,
    /// Sur combien d'images ses cadres se répartissent (la durée de la case quand elle peut porter deux clés), et jusqu'où il peut aller
    /// (le mouvement suivant, ou la fin du plan), comptées depuis `debut`.
    pub len: usize,
    pub total: usize,
}

impl App {
    fn largeur_case(&self) -> f32 {
        self.layout.card_size.clamp(crate::board::CARD_MIN, crate::board::CARD_MAX)
    }

    /// Le mouvement que porte, ou peut commencer, la case qui commence à l'image `debut` du plan `pi` (partie 142). La première case : toujours
    /// (le mouvement du plan, comme avant). Une case de continuité : le sien si elle en porte un ; sinon celui qu'elle peut commencer, quand
    /// la caméra y est à l'arrêt après un mouvement (aucune clé du plan de son début jusqu'au mouvement suivant, et une clé avant elle).
    /// Rien : la caméra est en route sur cette case, ou le plan ne bouge pas encore (son premier mouvement naît sur sa première case).
    pub(crate) fn mouvement_de_case(&self, pi: usize, debut: usize) -> Option<MouvementCase> {
        let p = self.doc.panels.get(pi)?;
        let plan = p.frame_count();
        let cases = vignettecam::cases_du_plan(&self.doc, pi);
        let gs = vignettecam::groupes(&p.camera, &cases);
        let fin_case = cases.iter().copied().find(|&f| f > debut).unwrap_or(plan);
        if let Some(g) = gs.iter().find(|g| g.debut == debut) {
            let limite = gs.get(g.numero + 1).map_or(plan, |s| s.debut);
            let (span, total) = (fin_case.min(limite).saturating_sub(debut), limite.saturating_sub(debut));
            return Some(MouvementCase { debut, rangs: g.rangs.clone(), lettre0: g.lettre0(), len: vignettecam::etendue_du_mouvement(span, total), total });
        }
        if !cases.contains(&debut) {
            return None;
        }
        let limite = gs.iter().map(|g| g.debut).find(|&d| d > debut).unwrap_or(plan);
        let at = p.camera.iter().position(|k| k.frame >= debut).unwrap_or(p.camera.len());
        if at == 0 || p.camera.len() < 2 || p.camera.iter().any(|k| k.frame >= debut && k.frame < limite) {
            return None;
        }
        let (span, total) = (fin_case.min(limite) - debut, limite - debut);
        Some(MouvementCase { debut, rangs: at..at, lettre0: vignettecam::rang_de_lettre(&gs, at - 1), len: vignettecam::etendue_du_mouvement(span, total), total })
    }

    /// L'image où commence la case qui porte (ou portera) le mouvement qu'un geste fait sur la case `frame` du plan `pi` retouche : 0 pour
    /// la première case et pour un plan qui ne bouge pas encore (son premier mouvement naît sur sa première case, comme avant) ; sinon
    /// la case elle-même (partie 142 : une case de continuité porte le sien).
    pub(crate) fn case_du_mouvement(&self, pi: usize, frame: usize) -> usize {
        if frame == 0 || self.doc.panels.get(pi).is_none_or(|p| p.camera.len() < 2) {
            0
        } else {
            frame
        }
    }

    /// Les clés du mouvement `m` du plan `pi`, comptées depuis le début de sa case et rangées sur sa durée (`vignettecam::recale`), et la
    /// caméra dont il part : le premier cadre (la case), qui pour une case de continuité est le dernier cadre d'avant.
    fn cles_du_mouvement(&self, pi: usize, m: &MouvementCase) -> (Vec<CameraKey>, Camera) {
        let orig = &self.doc.panels[pi].camera;
        let locales: Vec<CameraKey> = orig[m.rangs.clone()].iter().map(|k| CameraKey { frame: k.frame.saturating_sub(m.debut), ..*k }).collect();
        let keys0 = vignettecam::recale(&locales, m.len, m.total);
        let a = match m.rangs.start.checked_sub(1).filter(|_| m.debut > 0) {
            Some(avant) => orig[avant].cam,
            None => keys0.first().map_or_else(|| self.doc.panel_camera(pi, 0), |k| k.cam),
        };
        (keys0, a)
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
    /// marque d'image) : si le plan ne bouge pas encore, ou (partie 142) si elle porte son propre mouvement ou peut en commencer un (la
    /// caméra y est à l'arrêt après un mouvement : elle part du cadre qu'elle montre).
    pub(crate) fn cam_possible(&self, place: &CardPlace) -> bool {
        self.cam_possible_ici(place.panel, place.frame)
    }

    /// Comme `cam_possible`, pour la case qui commence à l'image `frame` du plan `pi`.
    pub(crate) fn cam_possible_ici(&self, pi: usize, frame: usize) -> bool {
        frame == 0 || self.doc.panels.get(pi).is_some_and(|p| p.camera.len() < 2) || self.mouvement_de_case(pi, frame).is_some()
    }

    /// Maj seule tenue (partie 113, proposition A) : l'appui en `p` (planche) sur la case `k` prend-il la caméra plutôt que le plan ? Oui à
    /// un coin de la case ou d'un cadre (le zoom), et sur le bord d'un cadre B, C… (le déplacer) ; non partout ailleurs (le plan).
    pub(crate) fn maj_prend_la_camera(&self, layout: &BoardLayout, xf: Xf, k: usize, p: Pos2) -> bool {
        let place = layout.cards[k];
        if !self.cam_possible(&place) {
            return false;
        }
        let (cadres, simple) = self.cadres_sur_la_planche(layout, k);
        let tol = self.tol_coin(xf, &place);
        if cadres.iter().any(|q| q.iter().any(|c| c.distance(p) <= tol)) {
            return true;
        }
        if simple {
            return false;
        }
        let bord = (PRISE / xf.zoom.max(1e-3)).min(tol);
        cadres.iter().skip(1).any(|q| (0..4).any(|i| vignettecam::distance_segment(q[i], q[(i + 1) % 4], p) <= bord))
    }

    /// La tolérance, sur la planche, à laquelle un coin se saisit.
    fn tol_coin(&self, xf: Xf, place: &CardPlace) -> f32 {
        (PRISE / xf.zoom.max(1e-3)).min(place.thumb.width().min(place.thumb.height()) * 0.3)
    }

    /// Appui de caméra (Ctrl, ou T tenue) sur la case `k` : prend le cadre que l'appui saisit. Faux seulement si la case ne peut pas
    /// porter de mouvement (une marque d'image d'un plan qui bouge déjà) : l'appui garde alors son sens (choisir des plans). Le
    /// Ctrl + clic reste un choix de plans tant que le pointeur ne bouge pas (`finish_cam_geste`). `ajoute` : Alt est tenue, un
    /// cadre de plus part du dernier.
    pub(crate) fn cam_press(&mut self, layout: &BoardLayout, xf: Xf, k: usize, pos: Pos2, ajoute: bool) -> bool {
        let place = layout.cards[k];
        // Le mouvement d'un plan se dessine sur sa première case ; sur une autre case du plan (une marque d'image), le
        // geste ne vaut que tant que le plan n'a pas de mouvement : la case entière est alors le cadre A (la caméra du
        // plan), comme une case ordinaire. Un plan qui bouge déjà se règle sur sa première case, où ses cadres sont.
        // (Partie 142 : une case de continuité porte son propre mouvement, ou en commence un, quand la caméra y est à l'arrêt après un
        // mouvement ; sinon, la caméra y est en route, et le mouvement se règle là où sont ses cadres.)
        let debut = self.case_du_mouvement(place.panel, place.frame);
        let Some(mvt) = self.mouvement_de_case(place.panel, debut).filter(|_| self.cam_possible(&place)) else {
            self.status = "La caméra est déjà en mouvement sur cette case : il se règle sur la case où sont ses cadres.".into();
            return false;
        };
        let (cadres, simple) = self.cadres_sur_la_planche(layout, k);
        let tol = self.tol_coin(xf, &place);
        let Some(Choix { prise, rang, coin }) = vignettecam::choisit(&cadres, simple, xf.to_board(pos), tol, ajoute) else { return false };
        // Le premier cadre d'un mouvement de continuité est la case : il tient la caméra de la case d'avant, il ne se déplace pas ici.
        if debut > 0 && matches!(prise, Prise::Deplace(0) | Prise::Zoom(0)) {
            self.status = "Ce cadre tient la caméra de la case d'avant (la continuité) : il ne se déplace pas ici. Glisse les cadres suivants ; pour recadrer cette case, coupe le plan devant elle.".into();
            return false;
        }
        let orig = self.doc.panels[place.panel].camera.clone();
        let (keys0, a) = self.cles_du_mouvement(place.panel, &mvt);
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
        let aimante = matches!(prise, Prise::Nouveau | Prise::Deplace(1..)) && self.settings.vignettes.aimant_points().is_some();
        let mut grille = Grille::default();
        for q in &cadres {
            let r = vignettecam::boite(q).translate(-au);
            grille.xs.extend([r.left(), r.center().x, r.right()]);
            grille.ys.extend([r.top(), r.center().y, r.bottom()]);
        }
        if layout.sens.japonais() {
            // Lue à la japonaise, c'est l'inverse : les rangées de la planche sont les cases d'une colonne (un pas de case, de sa ligne de
            // numéro et d'écart de coupe, depuis A) et ses colonnes (le bord gauche et le bord droit des vignettes).
            let pas = board::LABEL_H + place.thumb.height() + board::gap_cut(place.thumb.width());
            for c in -6..=6 {
                grille.ys.extend([c as f32 * pas, c as f32 * pas + place.thumb.height()]);
            }
            for c in &layout.cards {
                grille.xs.extend([c.ligne.left() - au.x, c.ligne.right() - au.x]);
            }
            grille.xs.sort_by(f32::total_cmp);
            grille.xs.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        } else {
            // Les colonnes de la planche (un pas de case et d'écart de coupe, depuis A) et ses rangées (le haut et le bas des vignettes).
            let pas = place.thumb.width() + board::gap_cut(place.thumb.width());
            for c in -6..=6 {
                grille.xs.extend([c as f32 * pas, c as f32 * pas + place.thumb.width()]);
            }
            for c in &layout.cards {
                grille.ys.extend([c.ligne.top() - au.y, c.ligne.bottom() - au.y]);
            }
        }
        grille.ys.sort_by(f32::total_cmp);
        grille.ys.dedup_by(|a, b| (*a - *b).abs() < 0.5);
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        let reserve0 = layout.cards.iter().position(|c| c.plan == place.plan && c.frame == debut).and_then(|j| Reserve::depuis(layout, j));
        self.vignettes.cam = Some(CamGeste {
            plan: place.plan,
            image: place.frame,
            prise,
            depart: pos,
            maintenant: pos,
            actif: false,
            annule: false,
            keys0,
            orig,
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
            hold: None,
            pose: reserve0.as_ref().and_then(|r| r.pose),
            pose_y: reserve0.as_ref().and_then(|r| r.pose_y),
            pose_dec: reserve0.as_ref().and_then(|r| r.pose_dec),
            debut,
            rangs: mvt.rangs,
            lettre0: mvt.lettre0,
            len: mvt.len,
        });
        true
    }

    /// À chaque image : le pointeur suit ; au-delà du seuil le geste devient un mouvement de caméra, et les clés du plan
    /// suivent le pointeur (calculées depuis celles d'avant le geste) ; Échap remet celles d'avant.
    pub(crate) fn update_cam_geste(&mut self, pointer: Option<Pos2>, escape: bool) {
        let Some(mut g) = self.vignettes.cam.take() else { return };
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) else { return };
        if escape && g.actif && !g.annule {
            self.doc.panels[pi].camera = g.orig.clone();
            (g.annule, g.actif, g.snap) = (true, false, None);
            g.hold = None;
            self.status = "Mouvement de caméra abandonné.".into();
        }
        if let Some(p) = pointer {
            g.maintenant = p;
            if !g.actif && !g.annule && g.depart.distance(p) > MOVE_THRESHOLD {
                // (Créer un mouvement sur une case qui en a d'autres derrière elle coupe le plan : il faut pouvoir changer le montage.)
                let coupe = g.avant == 0 && self.doc.panels[pi].marks.iter().any(|m| m.frame > g.debut);
                if self.content_refuses() || !self.partage_peut_toucher(&[g.plan]) || (coupe && self.storyboard_refuses()) {
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
                let seuil = self.settings.vignettes.aimant_points().unwrap_or(0.0);
                vignettecam::aimante(g.rect0, libre, &g.grille, seuil / g.zoom.max(1e-6)) / g.k.max(1e-6)
            } else {
                (g.maintenant - g.depart) / g.echelle.max(1e-6)
            };
            match vignettecam::applique(self.doc.frame, &g.keys0, g.a, g.prise, delta, g.coin, g.centre, g.len) {
                Some(keys) => {
                    // (Les clés du mouvement de cette case reprennent leur place parmi celles du plan ; le mouvement suivant, s'il partait
                    // de son dernier cadre, part du nouveau : la continuité tient.)
                    let tenait = if g.rangs.is_empty() { g.a } else { g.orig[g.rangs.end - 1].cam };
                    self.doc.panels[pi].camera = vignettecam::recolle(&g.orig, &g.rangs, g.debut, &keys, tenait);
                    self.modified = true;
                }
                None => {
                    self.doc.panels[pi].camera = g.orig.clone();
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
        let debut = g.debut;
        let Some(k) = layout.cards.iter().position(|c| c.plan == g.plan && c.frame == debut) else { return };
        if let Some(r) = Reserve::depuis(layout, k) {
            let r = Reserve { pose: g.pose.or(r.pose), pose_y: g.pose_y.or(r.pose_y), pose_dec: g.pose_dec.or(r.pose_dec), ..r };
            g.hold = Some(g.hold.map_or(r, |h| h.avec(r)));
        }
    }

    /// Au lâcher : le mouvement prend son étape d'annulation, nommée ; un Ctrl + clic qui n'a pas bougé choisit des plans.
    pub(crate) fn finish_cam_geste(&mut self, _layout: &BoardLayout, any_down: bool) {
        if any_down {
            return;
        }
        let Some(mut g) = self.vignettes.cam.take() else { return };
        if g.annule {
            return;
        }
        if !g.actif {
            // (Un Ctrl + Maj + clic qui ne bouge pas ne fait rien : ni plan choisi, ni plan déplacé.)
            return;
        }
        let Some(pi) = self.doc.panels.iter().position(|p| p.id == g.plan) else { return };
        let Some(snap) = g.snap.take() else { return };
        let changed = self.doc.panels[pi].camera != g.orig;
        if !changed {
            return;
        }
        let nom = vignettecam::nom_etape_depuis(g.prise, g.avant, g.lettre0);
        // (Le nombre de cadres de ce mouvement, maintenant : ceux d'avant, plus ce que le geste a ajouté au plan.)
        let n = (g.rangs.len() + self.doc.panels[pi].camera.len()).saturating_sub(g.orig.len());
        // Un mouvement créé sur une case qui a des cases de continuité derrière elle : elles deviennent un plan à part, dans la même étape.
        let scinde = g.avant == 0 && self.scinder_derriere_le_mouvement(pi, &g.orig, g.debut);
        self.etape_camera(snap, &nom);
        let (premier, dernier) = (vignettecam::lettre(g.lettre0), vignettecam::lettre(g.lettre0 + n.saturating_sub(1)));
        self.status = match g.prise {
            Prise::Nouveau | Prise::NouveauZoom if g.debut == 0 => {
                format!("Cadre {dernier} posé : la caméra va de A à {dernier} sur la durée du plan ({n} cadres, clés réparties sur le plan).")
            }
            // (Partie 142 : le mouvement d'une case de continuité, qui part du cadre qu'elle montrait.)
            Prise::Nouveau | Prise::NouveauZoom => {
                format!("Cadre {dernier} posé : dans cette case, la caméra va de {premier} à {dernier} ({n} cadres, sur la durée de la case).")
            }
            Prise::Deplace(i) => format!("Cadre {} déplacé.", vignettecam::lettre(g.lettre0 + i)),
            Prise::Zoom(i) => format!("Cadre {} : zoom changé.", vignettecam::lettre(g.lettre0 + i)),
        };
        if scinde {
            self.status += " Les cases qui suivaient forment un plan à part (une coupe après celle-ci).";
        }
    }

    /// Une commande pendant le glissé (un raccourci tapé, le bouton tenu : réunir, supprimer le plan, annuler…), un changement
    /// d'espace de travail ou d'onglet : le mouvement de caméra en cours se pose d'abord, avec son étape d'annulation, et le
    /// reste du glissé ne fait plus rien. Sans cela la commande prenait les clés du geste dans sa propre étape (annuler la
    /// commande les laissait, sans étape pour les défaire), ou le geste restait ouvert, sa réserve de place gardée sur la
    /// planche (passe de débogage au hasard, 2026-10-22).
    pub(crate) fn poser_le_geste_de_camera(&mut self) {
        if self.vignettes.cam.is_none() {
            return;
        }
        let layout = self.board_layout();
        self.finish_cam_geste(&layout, false);
        self.vignettes.cam = None;
        if self.vignettes.press != Press::None {
            self.vignettes.press = Press::Dead;
        }
    }

    /// Le mouvement du plan `pi` vient d'être créé (ses clés sont posées ; `avant` : celles d'avant) : si le plan a d'autres cases derrière la
    /// première, elles passent dans un plan neuf, juste après (`Document::scinder_apres_mouvement`). Vrai si le plan s'est coupé.
    /// (Partie 142) `debut` : l'image où commence la case qui porte le mouvement ; ce sont les cases **derrière elle** qui partent.
    fn scinder_derriere_le_mouvement(&mut self, pi: usize, avant: &[CameraKey], debut: usize) -> bool {
        let p = &self.doc.panels[pi];
        if p.camera.len() < 2 || !p.marks.iter().any(|m| m.frame > debut) {
            return false;
        }
        let finale = p.camera.clone();
        let n = self.doc.panels.len();
        let fait = self.doc.scinder_apres_mouvement_de(pi, avant, finale, debut).is_some() && self.doc.panels.len() == n + 1;
        if fait {
            self.board_sel = vec![self.doc.panels[pi].id];
            // (Le geste a pu commencer sur une case qui est maintenant dans l'autre plan.)
            self.cur_frame = self.cur_frame.min(self.frame_count().saturating_sub(1));
            self.follow_frame = true;
        }
        fait
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
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cam_ajouter_cadre(&mut self, pi: usize) {
        self.cam_ajouter_cadre_case(pi, 0);
    }

    /// Comme `cam_ajouter_cadre`, pour le mouvement de la case qui commence à l'image `debut` du plan `pi` (partie 142) : sur une case de
    /// continuité à l'arrêt après un mouvement, c'est un mouvement de plus, qui part du cadre qu'elle montre (B vers C, puis C vers D…).
    pub(crate) fn cam_ajouter_cadre_case(&mut self, pi: usize, debut: usize) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let Some(mvt) = self.mouvement_de_case(pi, debut) else {
            self.status = "La caméra est déjà en mouvement sur cette case : il se règle sur la case où sont ses cadres.".into();
            return;
        };
        let (keys0, a) = self.cles_du_mouvement(pi, &mvt);
        let base = if keys0.len() >= 2 { keys0[keys0.len() - 1].cam } else { a };
        let nouveau = vignettecam::cadre_suivant(self.doc.frame, base, vignettecam::taille_cadre(&self.doc, &base));
        let Some(keys) = vignettecam::avec_nouveau_cadre(&keys0, a, nouveau, mvt.len) else {
            self.status = if debut == 0 { "Ce plan est trop court pour un cadre de plus : allonge-le d'abord." } else { "Cette case est trop courte pour un cadre de plus : allonge-la d'abord." }.into();
            return;
        };
        let avant = if keys0.len() >= 2 { keys0.len() } else { 0 };
        if avant == 0 && self.doc.panels[pi].marks.iter().any(|m| m.frame > debut) && self.storyboard_refuses() {
            return;
        }
        let snap = self.snapshot();
        let orig = self.doc.panels[pi].camera.clone();
        let tenait = if mvt.rangs.is_empty() { a } else { orig[mvt.rangs.end - 1].cam };
        let n = keys.len();
        self.doc.panels[pi].camera = vignettecam::recolle(&orig, &mvt.rangs, debut, &keys, tenait);
        let nom = vignettecam::nom_etape_depuis(Prise::Nouveau, avant, mvt.lettre0);
        let scinde = avant == 0 && self.scinder_derriere_le_mouvement(pi, &orig, debut);
        self.etape_camera(snap, &nom);
        self.status = format!("Cadre {} ajouté : Maj tenue, glisse-le par son bord.", vignettecam::lettre(mvt.lettre0 + n - 1));
        if scinde {
            self.status += " Les cases qui suivaient forment un plan à part (une coupe après celle-ci).";
        }
    }

    /// « Retirer ce cadre » : la clé `rang` du plan `pi` s'en va. Retirer B d'un mouvement à deux cadres enlève le mouvement.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn cam_retirer_cadre(&mut self, pi: usize, rang: usize) {
        self.cam_retirer_cadre_case(pi, 0, rang);
    }

    /// Comme `cam_retirer_cadre`, pour le cadre `local` (0 : le premier) du mouvement de la case qui commence à l'image `debut` (partie
    /// 142). Le premier cadre d'un mouvement de continuité est la case (il tient le cadre d'avant) : il ne se retire pas ; quand il ne
    /// reste que lui, le mouvement de la case n'existe plus, et la case montre de nouveau le cadre d'avant, à l'arrêt.
    pub(crate) fn cam_retirer_cadre_case(&mut self, pi: usize, debut: usize, local: usize) {
        let Some(id) = self.doc.panels.get(pi).map(|p| p.id) else { return };
        if self.content_refuses() || !self.partage_peut_toucher(&[id]) {
            return;
        }
        let Some(mvt) = self.mouvement_de_case(pi, debut).filter(|m| local < m.rangs.len()) else { return };
        if debut > 0 && local == 0 {
            self.status = "Ce cadre est la case : il tient la caméra de la case d'avant. Retire plutôt les cadres suivants.".into();
            return;
        }
        let (keys0, _) = self.cles_du_mouvement(pi, &mvt);
        let Some(mut keys) = vignettecam::sans_le_cadre(&keys0, local, mvt.len) else { return };
        if debut > 0 && keys.len() < 2 {
            keys.clear();
        }
        let snap = self.snapshot();
        let reste = keys.len();
        let orig = self.doc.panels[pi].camera.clone();
        let tenait = orig[mvt.rangs.end - 1].cam;
        self.doc.panels[pi].camera = vignettecam::recolle(&orig, &mvt.rangs, debut, &keys, tenait);
        let lettre = vignettecam::lettre(mvt.lettre0 + local);
        let nom = format!("Retirer le cadre {lettre}");
        self.etape_camera(snap, &nom);
        self.status = if reste < 2 { "Mouvement de caméra retiré.".into() } else { format!("Cadre {lettre} retiré.") };
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
        painter.text(at, Align2::LEFT_TOP, texte, FontId::proportional(15.0), self.cadre_couleur());
    }

    /// L'outil Caméra des Vignettes (Anthony, 2026-11-05 : « signaler que l'on peut interagir avec la caméra, la sélectionner, la déplacer ») :
    /// les cadres de la case sous le pointeur se tracent plus francs, avec une poignée à chaque coin ; ceux de la caméra choisie restent
    /// soulignés (plus épais, avec leur lettre). Rien n'est lu dans le document ailleurs que pour ces deux cases.
    pub(crate) fn paint_cadres_camera(&self, painter: &Painter, view: Rect, xf: Xf, layout: &BoardLayout, over: Option<Pos2>) {
        let survol = over.and_then(|p| layout.card_near(xf.to_board(p), self.marge_camera())).filter(|&k| self.cam_possible(&layout.cards[k]));
        let bleu = self.cadre_couleur();
        let halo = Stroke::new(4.0_f32, Color32::from_white_alpha(150));
        for (k, place) in layout.cards.iter().enumerate() {
            let choisie = self.vignettes.camera_choisie == Some(place.plan) && place.frame == 0;
            if !choisie && survol != Some(k) {
                continue;
            }
            let (cadres, _) = self.cadres_sur_la_planche(layout, k);
            // (La lettre du premier cadre : A, ou celle du cadre que tient une case de continuité qui a son mouvement, partie 142.)
            let lettre0 = self.mouvement_de_case(place.panel, self.case_du_mouvement(place.panel, place.frame)).map_or(0, |m| m.lettre0);
            let clip = painter.with_clip_rect(xf.rect(place.foot).expand(40.0).intersect(view));
            for (i, q) in cadres.iter().enumerate() {
                let pts: Vec<Pos2> = q.iter().map(|p| xf.to_screen(*p)).collect();
                clip.add(Shape::closed_line(pts.clone(), halo));
                clip.add(Shape::closed_line(pts.clone(), Stroke::new(if choisie { 2.4_f32 } else { 1.8_f32 }, bleu)));
                if survol == Some(k) || choisie {
                    for c in &pts {
                        clip.add(Shape::circle_filled(*c, 3.5, Color32::WHITE));
                        clip.add(Shape::circle_stroke(*c, 3.5, Stroke::new(1.2_f32, bleu)));
                    }
                }
                clip.text(pts[0] + vec2(3.0, 1.0), Align2::LEFT_TOP, vignettecam::lettre(lettre0 + i).to_string(), FontId::proportional(11.0), bleu);
            }
        }
    }

    /// Ce qui se dessine par-dessus la tuile d'une empreinte : les cadres fins en bleu avec leur lettre (tournés si la caméra l'est),
    /// et ce qui les relie : les côtés de l'enveloppe entre deux cadres (la silhouette), jamais une ligne qui traverse l'intérieur, et la
    /// flèche d'un travelling. Le dessin reste net partout dans l'enveloppe (Anthony, 2026-10-22 : plus de voile sur les traits).
    pub(crate) fn paint_empreinte(&self, painter: &Painter, view: Rect, xf: Xf, place: &CardPlace, mv: &MoveBoard) {
        let courant = self.cur_panel == place.panel;
        let clip = painter.with_clip_rect(xf.rect(place.foot).expand(40.0).intersect(view));
        let bleu = self.cadre_couleur();
        let alpha = |a: u8| Color32::from_rgba_unmultiplied(bleu.r(), bleu.g(), bleu.b(), a);
        let (trait_, halo) = (if courant { 1.4_f32 } else { 1.1_f32 }, Stroke::new(3.0_f32, Color32::from_white_alpha(110)));
        let distincts = vignettecam::cadres_distincts(&mv.quads());
        // (Les lettres courent d'un bout à l'autre du plan : la case de continuité qui a son mouvement repart de la lettre qu'elle tient.)
        let groupes = vignettecam::groupes_du_plan(&self.doc, place.panel);
        for (c, _) in mv.cadres.iter().zip(&distincts).filter(|(_, d)| **d) {
            let q: Vec<Pos2> = c.coins.iter().map(|p| xf.to_screen(*p)).collect();
            if !vignettecam::boite(&q).intersects(view) {
                continue;
            }
            clip.add(Shape::closed_line(q.clone(), halo));
            clip.add(Shape::closed_line(q.clone(), Stroke::new(trait_, alpha(if courant { 245 } else { 215 }))));
            let lettre = vignettecam::lettre_du_cadre(&groupes, c.rang).to_string();
            clip.text(q[0] + vec2(3.0, 1.0), Align2::LEFT_TOP, lettre, FontId::proportional(11.0), alpha(250));
        }
        let s = |p: Pos2| xf.to_screen(p);
        let fin = Stroke::new(1.0_f32, alpha(225));
        for lien in vignettecam::liens(&mv.quads(), place.thumb) {
            match lien {
                Lien::Bord(de, vers) => {
                    clip.line_segment([s(de), s(vers)], fin);
                }
                Lien::Fleche { de, vers } => {
                    // Une fine flèche, de la teinte des cadres, sans plein ni grosse pointe : un petit chevron au bout. Un liseré blanc très
                    // léger dessous la garde lisible sur le gris de la planche comme sur le blanc des cases.
                    let (a, b) = (s(de), s(vers));
                    let d = b - a;
                    let long = d.length();
                    if long > 3.0 {
                        let u = d / long;
                        let chevron = (long * 0.25).clamp(3.0, 5.5);
                        let [h1, h2] = vignettecam::chevron(b, u, chevron);
                        let (halo_f, trait_f) = (Stroke::new(2.4_f32, Color32::from_white_alpha(70)), Stroke::new(1.0_f32, alpha(vignettecam::FLECHE_ALPHA)));
                        for st in [halo_f, trait_f] {
                            clip.line_segment([a, b], st);
                            clip.line_segment([b, h1], st);
                            clip.line_segment([b, h2], st);
                        }
                    }
                }
            }
        }
    }
}