//! « Espace 3D à côté » (`docs/ESPACE3D.md`, partie 13 ; `docs/INTERFACE.md`, partie 144) : la vue 3D partage la zone du
//! dessin avec la feuille. On dessine, on anime, on lit le plan à gauche ; à droite, les cartes (les calques) suivent.
//!
//! - **Partage** : la feuille à gauche, une barre à tirer, la vue 3D à droite avec sa rangée d'outils. La part de la vue
//!   est retenue (`Layout::part_espace3d`). Le plein écran d'avant (`espace3d.basculer`) reste : il l'emporte tant qu'il
//!   est ouvert, et le refermer rend le côte à côte.
//! - **Gestes** : un geste appartient à la vue où il commence, jusqu'au relâché (chaque vue a sa propre zone d'egui). Le
//!   stylet ne dessine que sur la feuille (`canvas_input` ne prend que les appuis sur elle).
//! - **Sans latence** (règle de la maison) : pendant un geste sur la feuille, la vue 3D se refait au plus
//!   `RYTHME_GESTE` fois par seconde ; entre-temps, elle repeint ce qu'elle a tracé (`VueGardee`). En lecture, elle se
//!   refait quand l'image du film change. Une carte dont le dessin change n'est refaite que là où ses tuiles ont changé
//!   (`changement`), pixel pour pixel comme une image refaite en entier (`render::cel_image_region`), et une image
//!   d'écran ne prépare pas plus de `BUDGET_TEXELS` texels de cartes (au moins une carte) : le reste attend l'image
//!   suivante, la carte montrant son dessin d'avant.

use eframe::egui::{self, pos2, vec2, CursorIcon, Rect, Sense, Shape, Stroke, TextureHandle};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::document::CelId;
use crate::raster::{Raster, Tile, TILE};

/// Pendant un geste sur la feuille : la vue 3D se refait au plus tant de fois par seconde.
pub(crate) const RYTHME_GESTE: f64 = 30.0;
/// Largeur maximale de l'image d'une carte dans la vue 3D.
pub(crate) const LARGEUR_CARTE: usize = 1024;
/// Texels de cartes préparés au plus par image d'écran (une carte entière de 1024 × 576 à peu près ; au moins une carte).
pub(crate) const BUDGET_TEXELS: usize = 600_000;
/// Part de la largeur donnée à la vue 3D : par défaut, au moins, au plus.
pub(crate) const PART_DEFAUT: f32 = 0.5;
pub(crate) const PART_MIN: f32 = 0.2;
pub(crate) const PART_MAX: f32 = 0.8;
/// Largeur minimale de chacune des deux vues (points) ; plus étroit, la feuille garde toute la zone.
pub(crate) const LARGEUR_MIN: f32 = 160.0;
/// Largeur de la barre à tirer entre les deux vues.
pub(crate) const BARRE: f32 = 6.0;

/// L'image d'une carte dans la vue 3D, et de quoi savoir ce qui a changé depuis.
pub(crate) struct CarteTexture {
    pub version: u64,
    pub tex: TextureHandle,
    pub size: [usize; 2],
    /// Les tuiles du dessin quand l'image a été faite. Gardé seulement pour les cartes tracées à la dernière image : il
    /// retient de vieilles tuiles (copie à l'écriture) et empêcherait les dessins qu'on ne regarde plus de se ranger.
    pub cliche: Option<Cliche>,
    /// Numéro de l'image d'écran où la carte a été tracée pour la dernière fois.
    pub vue: u64,
}

/// Les tuiles d'un dessin à un moment donné (copies partagées : rien n'est recopié).
pub(crate) struct Cliche {
    w: usize,
    h: usize,
    tiles: Vec<Option<Tile>>,
    outer: Vec<((i32, i32), Tile)>,
}

impl Cliche {
    pub(crate) fn of(r: &Raster) -> Cliche {
        Cliche { w: r.width, h: r.height, tiles: r.tiles.clone(), outer: r.outer.iter().map(|(k, t)| (*k, t.clone())).collect() }
    }
}

/// Ce qu'il faut refaire de l'image d'une carte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Changement {
    Rien,
    Tout,
    /// Ce morceau de l'image réduite : `[x0, y0, x1, y1]`, x1 et y1 exclus.
    Zone([usize; 4]),
}

impl Changement {
    /// Texels à calculer.
    pub(crate) fn cout(&self, size: [usize; 2]) -> usize {
        match self {
            Changement::Rien => 0,
            Changement::Tout => size[0] * size[1],
            Changement::Zone(z) => (z[2] - z[0]) * (z[3] - z[1]),
        }
    }
}

/// Ce qui a changé de l'image réduite (`render::cel_image(r, max_w)`) depuis le cliché `avant` : les tuiles qui ne sont
/// plus les mêmes (une écriture copie la tuile partagée), étendues de ce que l'échantillonnage lit autour. Un changement
/// hors de la feuille, ou de taille, refait tout ; un morceau de plus de la moitié de l'image aussi.
pub(crate) fn changement(avant: &Cliche, r: &Raster, max_w: usize) -> Changement {
    if avant.w != r.width || avant.h != r.height || avant.tiles.len() != r.tiles.len() {
        return Changement::Tout;
    }
    if avant.outer.len() != r.outer.len() || avant.outer.iter().zip(r.outer.iter()).any(|((ka, ta), (kb, tb))| ka != kb || !ta.same_as(tb)) {
        return Changement::Tout;
    }
    let [tw, th] = crate::render::cel_image_size(r, max_w);
    let (sx, sy) = (r.width.max(1) as f32 / tw as f32, r.height.max(1) as f32 / th as f32);
    let tiles_x = r.tiles_x.max(1);
    let mut zone: Option<[usize; 4]> = None;
    for (i, (a, b)) in avant.tiles.iter().zip(&r.tiles).enumerate() {
        let meme = match (a, b) {
            (None, None) => true,
            (Some(a), Some(b)) => a.same_as(b),
            _ => false,
        };
        if meme {
            continue;
        }
        let (tx, ty) = (i % tiles_x, i / tiles_x);
        let z = [
            premier_texel((tx * TILE) as f32, sx),
            premier_texel((ty * TILE) as f32, sy),
            dernier_texel(((tx + 1) * TILE) as f32, sx, tw),
            dernier_texel(((ty + 1) * TILE) as f32, sy, th),
        ];
        zone = Some(match zone {
            None => z,
            Some(u) => [u[0].min(z[0]), u[1].min(z[1]), u[2].max(z[2]), u[3].max(z[3])],
        });
    }
    match zone {
        None => Changement::Rien,
        Some(z) if z[2] <= z[0] || z[3] <= z[1] => Changement::Rien,
        Some(z) if 2 * Changement::Zone(z).cout([tw, th]) > tw * th => Changement::Tout,
        Some(z) => Changement::Zone(z),
    }
}

/// Le premier texel de l'image réduite (pas `s` pixels de feuille par texel) qui lit un pixel de feuille ≥ `a`. Un texel
/// `x` lit les pixels de `⌊(x + 0,25)·s − 0,5⌋` à `⌊(x + 0,75)·s − 0,5⌋ + 1` (4 échantillons bilinéaires) : on prend un
/// texel de marge de chaque côté.
fn premier_texel(a: f32, s: f32) -> usize {
    ((a - 1.0) / s - 1.0).floor().max(0.0) as usize
}

/// Le texel (exclu) après le dernier qui lit un pixel de feuille < `b`.
fn dernier_texel(b: f32, s: f32, n: usize) -> usize {
    (((b + 2.0) / s + 1.0).ceil().max(0.0) as usize).min(n)
}

/// Ce que la vue d'à côté a tracé à sa dernière image refaite.
pub(crate) struct VueGardee {
    pub rect: Rect,
    pub temps: f64,
    pub panel: usize,
    pub frame: usize,
    pub formes: Vec<Shape>,
}

/// Le partage de la zone du dessin : la feuille, la barre à tirer, la vue 3D. `None` : trop étroit pour deux vues.
pub(crate) fn partage(rect: Rect, part: f32) -> Option<(Rect, Rect, Rect)> {
    let w = rect.width();
    if w < 2.0 * LARGEUR_MIN + BARRE {
        return None;
    }
    let part = if part.is_finite() { part.clamp(PART_MIN, PART_MAX) } else { PART_DEFAUT };
    let vue_w = (w * part).clamp(LARGEUR_MIN, w - LARGEUR_MIN - BARRE);
    let vue = Rect::from_min_max(pos2(rect.right() - vue_w, rect.top()), rect.max);
    let barre = Rect::from_min_max(pos2(vue.left() - BARRE, rect.top()), pos2(vue.left(), rect.bottom()));
    let feuille = Rect::from_min_max(rect.min, pos2(barre.left(), rect.bottom()));
    Some((feuille, barre, vue))
}

impl App {
    /// L'Espace 3D est à la place de la feuille (le mode d'avant).
    pub(crate) fn espace3d_plein(&self) -> bool {
        self.espace3d.on && !self.camera_view
    }

    /// La vue 3D est à côté de la feuille (demandée, et le plein écran n'est pas ouvert).
    pub(crate) fn espace3d_a_cote_visible(&self) -> bool {
        self.espace3d.a_cote && !self.espace3d_plein() && !self.in_card && !self.doc.film_vide()
    }

    /// Une commande de l'Espace 3D (un outil, Recadrer, Regarder par la caméra) : à côté, elle règle la vue d'à côté ;
    /// sinon elle ouvre le plein écran, comme avant.
    pub(crate) fn montrer_espace3d(&mut self) {
        if self.espace3d_a_cote_visible() {
            return;
        }
        if !self.espace3d_plein() {
            self.set_espace3d(true);
        }
    }

    /// « Espace 3D à côté (afficher / masquer) ». Depuis le plein écran : le referme et met la vue à côté.
    pub(crate) fn basculer_espace3d_a_cote(&mut self) {
        if self.espace3d_plein() {
            self.set_espace3d(false);
            self.espace3d.a_cote = true;
        } else {
            self.espace3d.a_cote = !self.espace3d.a_cote;
        }
        self.espace3d.drag = None;
        self.espace3d.gardee = None;
        // La feuille change de largeur : elle est recadrée dans sa nouvelle zone (comme à l'ouverture d'un projet).
        self.fit_pending = true;
        if !self.espace3d.a_cote {
            self.espace3d.clic_3d = false;
        }
        self.status = if self.espace3d.a_cote {
            "Espace 3D à côté du dessin : dessine à gauche, les cartes suivent à droite. Dans la vue 3D : clic sur une carte pour la choisir (son calque devient le calque courant), glisser pour la déplacer, clic droit pour tourner la vue.".into()
        } else {
            "Espace 3D à côté : masqué.".into()
        };
    }

    /// La vue 3D d'à côté : la barre à tirer `barre`, la rangée d'outils et la vue dans `vue` ; `total` est toute la zone
    /// du dessin (pour la part retenue).
    pub(crate) fn espace3d_a_cote_panel(&mut self, ui: &mut egui::Ui, barre: Rect, vue: Rect, total: Rect) {
        // La barre à tirer (double-clic : moitié-moitié).
        let r = ui.interact(barre, ui.id().with("barre_espace3d_a_cote"), Sense::click_and_drag());
        if r.hovered() || r.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        if r.dragged() {
            if let Some(p) = r.interact_pointer_pos() {
                self.layout.part_espace3d = ((total.right() - p.x - BARRE / 2.0) / total.width().max(1.0)).clamp(PART_MIN, PART_MAX);
            }
        }
        if r.double_clicked() {
            self.layout.part_espace3d = PART_DEFAUT;
        }
        let painter = ui.painter_at(barre);
        painter.rect_filled(barre, 0.0, crate::theme::entete());
        let trait_couleur = if r.hovered() || r.dragged() { crate::theme::SELECTION_BOITE } else { crate::viewbar::veil(70) };
        painter.line_segment([pos2(barre.center().x, barre.center().y - 14.0), pos2(barre.center().x, barre.center().y + 14.0)], Stroke::new(2.0_f32, trait_couleur));

        // La rangée d'outils de la vue.
        let outils = Rect::from_min_size(vue.min, vec2(vue.width(), crate::viewbar::TOOLS_BAR_H));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(outils).id_salt("outils_espace3d_a_cote"));
        child.painter().rect_filled(outils, 0.0, crate::theme::entete());
        child.set_clip_rect(outils);
        child.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::barres::ECART, 0.0);
            ui.add_space(4.0);
            self.espace3d_tools(ui);
            ui.add_space(crate::barres::MARGE_TRAIT);
            ui.separator();
            ui.add_space(crate::barres::MARGE_TRAIT);
            if self.case_icone(ui, ph::ARROWS_OUT, None, "Agrandir", "La vue 3D à la place de la feuille (le plein écran). La refermer remet la vue à côté du dessin.", "espace3d.basculer").clicked() {
                self.set_espace3d(true);
            }
            if self.case_icone(ui, ph::X, None, "Fermer la vue 3D d'à côté", "La feuille reprend toute la zone du dessin.", "espace3d.a_cote").clicked() {
                self.basculer_espace3d_a_cote();
            }
        });

        // La vue.
        let zone = Rect::from_min_max(pos2(vue.left(), outils.bottom()), vue.max);
        let response = ui.interact(zone, ui.id().with("vue_espace3d_a_cote"), Sense::click_and_drag());
        if response.is_pointer_button_down_on() {
            self.espace3d.clic_3d = true;
        }
        if !self.espace3d.a_cote || self.espace3d_plein() {
            // (Fermée ou agrandie par un bouton de la rangée, à cette image même.)
            return;
        }
        let now = ui.input(|i| i.time);
        // Repeindre ce qui a été tracé, sans rien refaire : pendant un geste sur la feuille (au plus `RYTHME_GESTE` fois
        // par seconde), et en lecture tant que l'image du film ne change pas.
        let reprise = self.espace3d.drag.is_none()
            && !response.hovered()
            && self.espace3d.gardee.as_ref().is_some_and(|g| {
                let meme = g.rect == zone && g.panel == self.cur_panel && g.frame == self.cur_frame;
                meme && ((self.espace3d.geste_feuille && now - g.temps < 1.0 / RYTHME_GESTE) || self.playing)
            });
        if reprise {
            let g = self.espace3d.gardee.as_ref().expect("vue gardée");
            ui.painter_at(zone).extend(g.formes.iter().cloned());
            if self.espace3d.geste_feuille {
                let reste = (1.0 / RYTHME_GESTE - (now - g.temps)).max(0.0);
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(reste));
            }
            return;
        }
        let layer = ui.layer_id();
        let debut = ui.ctx().graphics(|g| g.get(layer).map_or(0, |l| l.next_idx().0));
        self.espace3d.reportee = false;
        self.espace3d_panel(ui, zone, &response);
        // Une carte n'a pas pu être préparée à cette image : la vue sera refaite à la suivante (rien n'est gardé).
        self.espace3d.gardee = if self.espace3d.reportee {
            None
        } else {
            let formes = ui.ctx().graphics(|g| g.get(layer).map(|l| l.all_entries().skip(debut).map(|c| c.shape.clone()).collect()).unwrap_or_default());
            Some(VueGardee { rect: zone, temps: now, panel: self.cur_panel, frame: self.cur_frame, formes })
        };
    }

    /// L'image du dessin `cel` pour la vue 3D, refaite là où il a changé (au plus `BUDGET_TEXELS` par image d'écran,
    /// toutes cartes comprises, mais toujours au moins une carte). Une carte qui doit attendre rend son image d'avant (ou
    /// rien si elle n'en a pas encore) et demande une image de plus.
    pub(crate) fn e3_card_texture(&mut self, ctx: &egui::Context, cel: CelId) -> Option<egui::TextureId> {
        let pass = ctx.cumulative_pass_nr();
        let c = self.doc.cels.get(&cel)?;
        let size = crate::render::cel_image_size(&c.raster, LARGEUR_CARTE);
        let travail = match self.espace3d.card_textures.get(&cel) {
            Some(t) if t.version == c.version => None,
            Some(t) if t.size == size => Some(t.cliche.as_ref().map_or(Changement::Tout, |k| changement(k, &c.raster, LARGEUR_CARTE))),
            _ => Some(Changement::Tout),
        };
        let cout = travail.map_or(0, |w| w.cout(size));
        if cout > 0 {
            let (p, utilise) = self.espace3d.budget;
            let utilise = if p == pass { utilise } else { 0 };
            if utilise > 0 && utilise + cout > BUDGET_TEXELS {
                self.espace3d.budget = (pass, utilise);
                self.espace3d.reportee = true;
                ctx.request_repaint();
                let t = self.espace3d.card_textures.get_mut(&cel)?;
                t.vue = pass;
                return Some(t.tex.id());
            }
            self.espace3d.budget = (pass, utilise + cout);
        }
        let version = c.version;
        let cliche = Some(Cliche::of(&c.raster));
        match travail {
            None => {}
            Some(Changement::Rien) => {
                let t = self.espace3d.card_textures.get_mut(&cel).expect("la carte a son image");
                t.version = version;
                t.cliche = cliche;
            }
            Some(Changement::Zone(z)) => {
                let image = crate::render::cel_image_region(&c.raster, LARGEUR_CARTE, z);
                let t = self.espace3d.card_textures.get_mut(&cel).expect("la carte a son image");
                t.tex.set_partial([z[0], z[1]], image, egui::TextureOptions::LINEAR);
                t.version = version;
                t.cliche = cliche;
            }
            Some(Changement::Tout) => {
                let image = crate::render::cel_image(&c.raster, LARGEUR_CARTE);
                if let Some(t) = self.espace3d.card_textures.get_mut(&cel).filter(|t| t.size == size) {
                    t.tex.set(image, egui::TextureOptions::LINEAR);
                    t.version = version;
                    t.cliche = cliche;
                } else {
                    if self.espace3d.card_textures.len() > 256 {
                        self.espace3d.card_textures.clear();
                    }
                    let tex = ctx.load_texture(format!("espace 3d {cel}"), image, egui::TextureOptions::LINEAR);
                    self.espace3d.card_textures.insert(cel, CarteTexture { version, tex, size, cliche, vue: pass });
                }
            }
        }
        let t = self.espace3d.card_textures.get_mut(&cel)?;
        t.vue = pass;
        Some(t.tex.id())
    }

    /// Après le tracé des cartes : les clichés des cartes qui n'ont pas été tracées à cette image sont lâchés (ils
    /// retiendraient de vieilles tuiles).
    pub(crate) fn e3_lacher_les_cliches(&mut self, pass: u64) {
        for t in self.espace3d.card_textures.values_mut() {
            if t.vue != pass {
                t.cliche = None;
            }
        }
    }
}
