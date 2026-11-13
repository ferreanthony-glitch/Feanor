//! Grab et Pull dans l'application (le cœur, sans interface : `sculpt.rs`).
//!
//! Un geste : appui (on prend ce qui est sous le pinceau), glisser (à chaque image d'écran, les pas du
//! pointeur sont posés), lâcher (une seule étape d'annulation pour tous les dessins touchés).
//!
//! * **Sans latence** : pendant le geste, rien ne rejoue le moteur de brosses. Un calque vectoriel déplace
//!   les points de sa copie de travail (le document ne change qu'au lâcher) et montre les pixels déformés du
//!   cercle du pinceau (la copie est à nous : les tuiles ne se recopient pas, `Raster` les partage). Au lâcher,
//!   la nouvelle géométrie est posée, et le rendu exact des traits déformés est demandé au fil de travail
//!   (`rejeu_cel`) ; l'image montrée reste celle du geste jusqu'à son arrivée. Un calque bitmap travaille les
//!   pixels du cercle, rien de plus. Un geste n'est calculé qu'une fois par image d'écran (les mouvements du
//!   pointeur de l'image sont réunis), en pas qui restent petits devant le rayon.
//! * **Plusieurs clés** (Anthony, 2026-10-24) : quand plusieurs clés du calque courant sont choisies dans le
//!   montage ou la timeline, la même déformation est posée sur le dessin de chacune (et sur le dessin qu'on
//!   voit), dans la même étape d'annulation.
//! * **L'aplat** (la strate peinte et la coupe, `fond.rs`) suit : mêmes pas, mêmes fonctions. Sa forme
//!   déduite du trait suit le trait toute seule. Quand la sous-ligne de l'aplat est la cible du dessin, seul
//!   l'aplat est déformé.

use eframe::egui::{self, vec2, Color32, Pos2, Rect, RichText, Stroke};

use crate::app::{App, Tool};
use crate::document::CelId;
use crate::history::Edit;
use crate::raster::Raster;
use crate::sculpt::{self, Prise, Reglages};
use crate::vector::VectorDrawing;

const ACCENT: Color32 = crate::theme::SELECTION_BOITE;

/// Le temps qu'un geste de Pull peut prendre dans une image d'écran avant de poser le reste de son mouvement d'un seul pas.
const BUDGET_IMAGE: std::time::Duration = std::time::Duration::from_millis(10);

/// Le dessin vectoriel d'un des dessins du geste : avant, en travail, et (Grab) les points pris.
struct VecteurGeste {
    avant: VectorDrawing,
    travail: VectorDrawing,
    /// Les boîtes des éléments du dessin de travail (Pull : un pas ne regarde que ceux qui touchent le pinceau).
    boites: sculpt::Boites,
    prises: Vec<Prise>,
    /// Des points ont bougé (sinon le dessin reste tel quel au lâcher).
    change: bool,
}

/// Les pixels de l'aplat d'un dessin au début du geste.
struct AplatGeste {
    peinture: Raster,
    coupe: Raster,
}

/// Un dessin touché par le geste.
struct CelGeste {
    cel: CelId,
    vecteur: Option<VecteurGeste>,
    /// L'image du dessin à l'appui : l'annulation d'un calque bitmap, et la source du Grab.
    principal: Raster,
    aplat: Option<AplatGeste>,
}

/// Un geste de Grab ou de Pull en cours.
pub(crate) struct Geste {
    outil: Tool,
    reglages: Reglages,
    /// Le geste vient d'un stylet (la pression compte).
    pen: bool,
    presse: Pos2,
    /// Où est le pointeur (point du dessin), et jusqu'où les pas ont été posés.
    cible: Pos2,
    applique: Pos2,
    /// Grab : rayon et force pris à l'appui.
    rayon: f32,
    force: f32,
    /// La cible du dessin est l'aplat : seul lui bouge.
    seul_aplat: bool,
    cels: Vec<CelGeste>,
    /// Ce que le dernier pas de Grab a touché (à remettre avant le pas suivant), et tout ce que le geste a touché.
    zone_prec: Rect,
    zone_tot: Rect,
    bouge: bool,
}

impl Geste {
    /// Le geste travaille sur le dessin `cel`.
    pub(crate) fn touche(&self, cel: CelId) -> bool {
        self.cels.iter().any(|c| c.cel == cel)
    }
}

impl App {
    /// Les dessins que le geste déforme : le dessin qu'on voit, et — quand au moins deux clés du calque courant sont
    /// choisies — le dessin de chacune.
    pub(crate) fn sculpt_cibles(&self, ici: CelId) -> Vec<CelId> {
        let mut choisies: Vec<CelId> = Vec::new();
        if let Some(panel) = self.doc.panels.get(self.cur_panel) {
            let layer = self.layer();
            for k in self.key_sel.iter().filter(|k| k.panel == panel.id && k.layer == layer.id) {
                if let Some(cel) = layer.cel_at(k.start).filter(|c| !choisies.contains(c)) {
                    choisies.push(cel);
                }
            }
        }
        if choisies.len() < 2 {
            return vec![ici];
        }
        if !choisies.contains(&ici) {
            choisies.insert(0, ici);
        }
        choisies
    }

    /// Appui avec Grab ou Pull : on prend les dessins, et pour Grab les points sous le pinceau.
    pub(crate) fn sculpt_press(&mut self, p: Pos2, pen: bool) {
        self.end_stroke();
        self.playing = false;
        if !self.layer_editable() {
            return;
        }
        let Some(ici) = self.cel_here() else {
            self.status = "Rien à déformer : il n'y a pas de dessin à cette image.".into();
            return;
        };
        let outil = self.tool;
        let reglages = *self.sculpte.de(outil);
        let appui = if pen { self.pen_pressure } else { 1.0 };
        let (rayon, force) = reglages.effectifs(appui);
        let seul_aplat = self.aplat_en_cible();
        let mut cels = Vec::new();
        for cel in self.sculpt_cibles(ici) {
            let Some(c) = self.doc.cels.get(&cel) else { continue };
            let aplat = c.fond.as_ref().map(|s| AplatGeste { peinture: s.peinture.raster.clone(), coupe: s.coupe.clone() });
            if seul_aplat && aplat.is_none() {
                continue;
            }
            let vecteur = match (&c.vector, seul_aplat) {
                (Some(d), false) => {
                    let avant = d.clone();
                    let mut travail = d.clone();
                    let prises = if outil == Tool::Grab {
                        sculpt::densifier(&mut travail, p, rayon, sculpt::pas_de(rayon));
                        sculpt::prendre(&travail, p, rayon, reglages.attenuation)
                    } else {
                        Vec::new()
                    };
                    let boites = sculpt::Boites::de(&travail);
                    Some(VecteurGeste { avant, travail, boites, prises, change: false })
                }
                _ => None,
            };
            cels.push(CelGeste { cel, vecteur, principal: c.raster.clone(), aplat });
        }
        if cels.is_empty() {
            self.status = "L'aplat de ce dessin n'a rien de peint : rien à déformer (sa forme suit le trait).".into();
            return;
        }
        self.sculpt_geste = Some(Geste {
            outil,
            reglages,
            pen,
            presse: p,
            cible: p,
            applique: p,
            rayon,
            force,
            seul_aplat,
            cels,
            zone_prec: Rect::NOTHING,
            zone_tot: Rect::NOTHING,
            bouge: false,
        });
    }

    /// Le pointeur bouge : sa place est notée, les pas sont posés une fois par image (`sculpt_tick`). Faux : pas de
    /// geste en cours.
    pub(crate) fn sculpt_drag(&mut self, p: Pos2) -> bool {
        let Some(g) = self.sculpt_geste.as_mut() else { return false };
        g.cible = p;
        true
    }

    /// À chaque image d'écran, les pas du pointeur depuis la dernière : Grab déplace les points pris du décalage depuis
    /// l'appui ; Pull pose des pas qui restent petits devant le rayon (le quart au plus, 16 pas par image).
    pub(crate) fn sculpt_tick(&mut self) {
        let Some(mut g) = self.sculpt_geste.take() else { return };
        match g.outil {
            Tool::Grab => {
                if g.cible != g.applique {
                    self.sculpt_grab_pas(&mut g);
                    g.applique = g.cible;
                }
            }
            Tool::Pull => {
                let (from, to) = (g.applique, g.cible);
                let dist = from.distance(to);
                if dist >= 0.25 {
                    let appui = if g.pen { self.pen_pressure } else { 1.0 };
                    let (rayon, force) = g.reglages.effectifs(appui);
                    let n = ((dist / (rayon * 0.25).max(1.0)).ceil() as usize).clamp(1, 16);
                    let d = (to - from) / n as f32;
                    let debut = std::time::Instant::now();
                    for k in 0..n {
                        // Un rayon énorme sur un geste rapide : si l'image a déjà pris son temps, le reste du mouvement est
                        // posé d'un seul pas (jamais plus d'une image d'écran de retard).
                        if k > 0 && debut.elapsed() > BUDGET_IMAGE {
                            let reste = to - (from + d * k as f32);
                            self.sculpt_pull_pas(&mut g, from + d * k as f32, reste, rayon, force);
                            break;
                        }
                        self.sculpt_pull_pas(&mut g, from + d * k as f32, d, rayon, force);
                    }
                    g.applique = to;
                }
            }
            _ => {}
        }
        self.sculpt_geste = Some(g);
    }

    /// Les tuiles de `zone` à renvoyer à l'affichage.
    fn sculpt_invalider(&mut self, g: &Geste, zone: Rect) {
        if let Some(cg) = g.cels.first() {
            let ids = sculpt::tuiles_de(&cg.principal, zone);
            self.renderer.invalidate_tiles(&ids);
        }
    }

    /// Un pas de Grab : le décalage depuis l'appui, sur les points pris et sur les pixels.
    fn sculpt_grab_pas(&mut self, g: &mut Geste) {
        let delta = g.cible - g.presse;
        let (rayon, force, att) = (g.rayon, g.force, g.reglages.attenuation);
        let mut nouvelle = Rect::NOTHING;
        let mut vecteur_bouge = false;
        for cg in g.cels.iter_mut() {
            if let Some(v) = cg.vecteur.as_mut() {
                sculpt::grab_appliquer(&mut v.travail, &v.prises, delta, force);
                // (Une force nulle, ou un retour au point d'appui, ne change rien : pas d'étape au lâcher.)
                v.change = !v.prises.is_empty() && (delta * force).length() > 1e-4;
                vecteur_bouge |= v.change;
            }
            let Some(c) = self.doc.cels.get_mut(&cg.cel) else { continue };
            if !g.seul_aplat {
                sculpt::restaurer(&mut c.raster, &cg.principal, g.zone_prec);
                nouvelle = nouvelle.union(sculpt::saisir_raster(&cg.principal, &mut c.raster, g.presse, rayon, att, force, delta));
            }
            if let (Some(a), Some(s)) = (cg.aplat.as_ref(), c.fond.as_mut()) {
                sculpt::restaurer(&mut s.peinture.raster, &a.peinture, g.zone_prec);
                nouvelle = nouvelle.union(sculpt::saisir_raster(&a.peinture, &mut s.peinture.raster, g.presse, rayon, att, force, delta));
                sculpt::restaurer(&mut s.coupe, &a.coupe, g.zone_prec);
                sculpt::saisir_raster(&a.coupe, &mut s.coupe, g.presse, rayon, att, force, delta);
                s.peinture.version += 1;
            }
            c.version += 1;
        }
        let zone = g.zone_prec.union(nouvelle);
        self.sculpt_invalider(g, zone);
        g.zone_prec = nouvelle;
        g.zone_tot = g.zone_tot.union(nouvelle);
        g.bouge |= nouvelle != Rect::NOTHING || vecteur_bouge;
    }

    /// Un pas de Pull au point `c` du dessin, du décalage `d`.
    fn sculpt_pull_pas(&mut self, g: &mut Geste, c: Pos2, d: egui::Vec2, rayon: f32, force: f32) {
        let att = g.reglages.attenuation;
        let mut zone = Rect::NOTHING;
        let mut vecteur_bouge = false;
        for cg in g.cels.iter_mut() {
            if let Some(v) = cg.vecteur.as_mut() {
                if sculpt::pull_pas(&mut v.travail, &mut v.boites, c, d, rayon, att, force) {
                    v.change = true;
                    vecteur_bouge = true;
                }
            }
            let Some(cel) = self.doc.cels.get_mut(&cg.cel) else { continue };
            if !g.seul_aplat {
                zone = zone.union(sculpt::pousser(&mut cel.raster, c, d, rayon, att, force));
            }
            if let (Some(_), Some(s)) = (cg.aplat.as_ref(), cel.fond.as_mut()) {
                zone = zone.union(sculpt::pousser(&mut s.peinture.raster, c, d, rayon, att, force));
                sculpt::pousser(&mut s.coupe, c, d, rayon, att, force);
                s.peinture.version += 1;
            }
            cel.version += 1;
        }
        if zone == Rect::NOTHING && !vecteur_bouge {
            return;
        }
        self.sculpt_invalider(g, zone);
        g.zone_tot = g.zone_tot.union(zone);
        g.bouge = true;
    }

    /// Lâcher : la dernière place du pointeur est posée, puis tout est gardé en une étape d'annulation. Un dessin
    /// vectoriel reçoit sa nouvelle géométrie et son rendu exact est demandé au fil de travail.
    pub(crate) fn sculpt_finish(&mut self) {
        if self.sculpt_geste.is_none() {
            return;
        }
        self.sculpt_tick();
        let Some(g) = self.sculpt_geste.take() else { return };
        if !g.bouge {
            return;
        }
        let mut edits: Vec<Edit> = Vec::new();
        let mut rejeux: Vec<CelId> = Vec::new();
        let zone = g.zone_tot;
        for cg in g.cels {
            let Some(c) = self.doc.cels.get_mut(&cg.cel) else { continue };
            if !g.seul_aplat {
                match cg.vecteur {
                    Some(v) if v.change => {
                        edits.push(Edit::Vector { cel: cg.cel, drawing: v.avant });
                        c.vector = Some(v.travail);
                        rejeux.push(cg.cel);
                    }
                    Some(_) => {
                        // Les points n'ont pas bougé : l'image reprend ce qu'elle était.
                        sculpt::restaurer(&mut c.raster, &cg.principal, zone);
                    }
                    None => {
                        let tiles: Vec<_> = sculpt::tuiles_de(&cg.principal, zone).into_iter().map(|id| (id, cg.principal.slot(id).cloned())).collect();
                        if !tiles.is_empty() {
                            edits.push(Edit::Tiles { cel: cg.cel, tiles });
                        }
                    }
                }
            }
            if let (Some(a), Some(s)) = (cg.aplat, c.fond.as_mut()) {
                for (avant, coupe) in [(&a.peinture, false), (&a.coupe, true)] {
                    let tiles: Vec<_> = sculpt::tuiles_de(avant, zone).into_iter().map(|id| (id, avant.slot(id).cloned())).collect();
                    if !tiles.is_empty() {
                        edits.push(Edit::FondTiles { cel: cg.cel, tiles, coupe });
                    }
                }
                s.peinture.version += 1;
            }
            c.version += 1;
        }
        if edits.is_empty() {
            self.renderer.invalidate_all();
            return;
        }
        let nom = if g.outil == Tool::Pull { "Pull" } else { "Grab" };
        let avant = self.history.context.replace(nom.to_string());
        self.history.push(if edits.len() == 1 { edits.pop().unwrap_or(Edit::Group(Vec::new())) } else { Edit::Group(edits) });
        self.history.context = avant;
        self.modified = true;
        self.renderer.invalidate_all();
        for cel in rejeux {
            self.rejeu_cel(cel);
        }
    }

    /// Un des dessins du geste est en cours de déformation : son image est celle du geste (le rendu exact d'un rejeu
    /// attend la fin, `cel_busy`).
    pub(crate) fn sculpt_occupe(&self, cel: CelId) -> bool {
        self.sculpt_geste.as_ref().is_some_and(|g| g.touche(cel))
    }

    // ------------------------------------------------------------------ curseur et dessin par-dessus

    /// Le rayon que le cercle montre (en pixels du dessin) : celui du geste en cours, sinon le réglé.
    fn sculpt_rayon_montre(&self) -> f32 {
        match &self.sculpt_geste {
            Some(g) if g.outil == Tool::Pull => g.reglages.effectifs(if g.pen { self.pen_pressure } else { 1.0 }).0,
            Some(g) => g.rayon,
            None => self.sculpte.de(self.tool).rayon,
        }
    }

    /// Curseur de Grab et de Pull : le cercle du pinceau (« Afficher le curseur »), sinon la croix du système.
    pub(crate) fn sculpt_cursor(&self, ctx: &egui::Context, painter: &egui::Painter, p: Pos2) {
        let reg = self.sculpte.de(self.tool);
        let system = self.settings.system_cursor;
        ctx.set_cursor_icon(if system || !reg.curseur { egui::CursorIcon::Crosshair } else { egui::CursorIcon::None });
        if !reg.curseur {
            return;
        }
        let r = (self.sculpt_rayon_montre() * self.layer_zoom()).max(1.5);
        painter.circle_stroke(p, r, Stroke::new(1.0_f32, Color32::from_black_alpha(170)));
        painter.circle_stroke(p, r + 1.0, Stroke::new(1.0_f32, Color32::from_white_alpha(150)));
        if !system {
            let cross = Stroke::new(1.0_f32, Color32::from_gray(230));
            painter.line_segment([p - vec2(4.0, 0.0), p + vec2(4.0, 0.0)], cross);
            painter.line_segment([p - vec2(0.0, 4.0), p + vec2(0.0, 4.0)], cross);
        }
    }

    /// Pendant un geste de Grab : le cercle de l'appui, et un trait jusqu'au pointeur.
    pub(crate) fn paint_sculpt_overlay(&self, painter: &egui::Painter) {
        let Some(g) = self.sculpt_geste.as_ref().filter(|g| g.outil == Tool::Grab && g.bouge) else { return };
        if !self.sculpte.grab.curseur {
            return;
        }
        let (a, b) = (self.layer_to_screen(g.presse), self.layer_to_screen(g.cible));
        let r = (g.rayon * self.layer_zoom()).max(1.5);
        painter.circle_stroke(a, r, Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.8)));
        painter.line_segment([a, b], Stroke::new(1.5_f32, ACCENT));
        painter.circle_filled(a, 2.5, ACCENT);
    }

    // ------------------------------------------------------------------ réglages

    /// Réglages de Grab et de Pull (panneau Réglages) : rayon et force, chacun avec « selon l'appui » ;
    /// atténuation (quatre courbes) ; « Afficher le curseur ».
    pub(crate) fn sculpt_options_ui(&mut self, ui: &mut egui::Ui) {
        use crate::bulles::{choix, interrupteur};
        use egui_phosphor::regular as ph;
        let tool = self.tool;
        let mut r = *self.sculpte.de(tool);
        let avant = r;
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut r.rayon, Reglages::RAYON_MIN..=Reglages::RAYON_MAX).logarithmic(true).text("Rayon (px)").fixed_decimals(0));
            interrupteur(ui, &mut r.rayon_appui, ph::PEN_NIB, "Rayon selon l'appui", "Le rayon suit la pression du stylet (à la souris, la pression vaut 100 %).", "");
        });
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut r.force, 0.0..=1.0).text("Force").fixed_decimals(2));
            interrupteur(ui, &mut r.force_appui, ph::PEN_NIB, "Force selon l'appui", "La force suit la pression du stylet (à la souris, la pression vaut 100 %).", "");
        });
        ui.horizontal(|ui| {
            ui.label("Atténuation");
            for a in sculpt::Attenuation::ALL {
                choix(ui, &mut r.attenuation, a, a.icon(), a.label(), a.description(), "");
            }
        });
        ui.horizontal(|ui| {
            courbe_attenuation(ui, r.attenuation);
            ui.checkbox(&mut r.curseur, "Afficher le curseur");
        });
        // (Le geste de Grab et de Pull est dans l'animation « Grab et Pull » et le registre des explications, partie 153.)
        let cles = self.sculpt_cibles_choisies();
        if cles >= 2 {
            ui.label(RichText::new(format!("{cles} clés choisies : la même déformation s'applique à chacune.")).small().strong());
        }
        if r != avant {
            *self.sculpte.de_mut(tool) = r;
        }
    }

    /// Combien de clés du calque courant sont choisies (de quoi dire à qui la déformation s'applique).
    fn sculpt_cibles_choisies(&self) -> usize {
        let Some(ici) = self.cel_here() else { return 0 };
        let n = self.sculpt_cibles(ici).len();
        if n >= 2 {
            n
        } else {
            0
        }
    }
}

/// La courbe d'atténuation, de bord à bord du cercle (poids de 0 à 1 en hauteur).
fn courbe_attenuation(ui: &mut egui::Ui, att: sculpt::Attenuation) {
    let (rect, _) = ui.allocate_exact_size(vec2(140.0, 36.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let inner = rect.shrink2(vec2(4.0, 4.0));
    let base = Stroke::new(1.0_f32, crate::theme::bord());
    painter.line_segment([inner.left_bottom(), inner.right_bottom()], base);
    painter.line_segment([egui::pos2(inner.center().x, inner.top()), egui::pos2(inner.center().x, inner.bottom())], base);
    let pts: Vec<egui::Pos2> = (0..=48)
        .map(|k| {
            let x = k as f32 / 24.0 - 1.0;
            let w = att.weight(x.abs());
            egui::pos2(inner.left() + inner.width() * (x + 1.0) / 2.0, inner.bottom() - inner.height() * w)
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, ACCENT)));
}
