//! L'écran du crayon de la planche de références (`planchdessin.rs`) : les icônes Crayon et Gomme, l'encre et la taille, et les
//! gestes sur le tableau.
//!
//! - **Les outils** : la sélection (le geste d'origine : rien ne trace jamais), le crayon, la gomme. Tant que le crayon ou la
//!   gomme est choisi, un appui sur le tableau trace ou gomme ; Espace tenu, le bouton du milieu et la molette déplacent et
//!   zooment la vue comme avant. Quitter l'outil (l'icône de nouveau, ou Échap si rien d'autre n'a besoin d'Échap : un trait en
//!   cours, un texte qu'on écrit, un recadrage) rend tous les gestes d'avant.
//! - **Un trait = une étape d'annulation** (« Trait de crayon »), prise quand le trait est posé. La gomme enlève le trait entier
//!   que touche son cercle ; un seul passage de gomme, même sur dix traits, est une étape (« Gommer des traits »).
//! - **Sans latence** : pendant le geste, rien n'est écrit dans le document (le trait en cours est peint à part, au zoom courant)
//!   et rien ne grossit avec le nombre de traits déjà posés (les traits hors de l'écran ne sont pas peints, les éléments sont copiés
//!   sans copier les points).
//! - **Stylet** : la pression vient des événements « tactiles » de la fenêtre qui reçoit le geste (celle de la planche, détachée
//!   ou non) ; une souris donne la pression pleine.
//! - **Retenue** : l'outil, l'encre et la taille ne sont pas dans le projet (une séance).

use super::*;

use crate::dock::Pane;
use crate::history::Edit;
use crate::planchdessin::{self, INKS, INK_NAMES, WIDTH_DEFAULT, WIDTH_MAX, WIDTH_MIN};

/// L'outil de la planche.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Outil {
    /// Le geste d'origine : choisir, déplacer, déplacer la vue.
    #[default]
    Selection,
    Crayon,
    Gomme,
}

/// Un geste de crayon ou de gomme en cours.
struct Geste {
    outil: Outil,
    /// Les points du trait (tableau, pression), pour le crayon.
    points: Vec<(Pos2, f32)>,
    epoch: u64,
    serial: u64,
    /// La gomme : l'étape d'annulation, prise avant le premier trait enlevé, et combien en sont partis.
    snap: Option<Edit>,
    gommes: usize,
}

pub(crate) struct Crayon {
    pub outil: Outil,
    /// L'encre et l'épaisseur (à pression pleine, unités du tableau) du crayon ; l'épaisseur donne aussi la taille de la gomme.
    pub encre: [u8; 3],
    pub epaisseur: f32,
    geste: Option<Geste>,
    /// La pression du stylet, et le dernier instant où un événement « tactile » est arrivé (une souris n'en donne jamais).
    pression: f32,
    tactile_vu: f64,
    /// La pression à prendre maintenant : celle du stylet s'il en est un, sinon la pleine.
    courante: f32,
    /// Les positions du pointeur arrivées pendant cette image d'écran, avec leur pression.
    mouvements: Vec<(Pos2, f32)>,
}

impl Default for Crayon {
    fn default() -> Self {
        Crayon { outil: Outil::Selection, encre: planchdessin::INK_DEFAULT, epaisseur: WIDTH_DEFAULT, geste: None, pression: 1.0, tactile_vu: f64::NEG_INFINITY, courante: 1.0, mouvements: Vec::new() }
    }
}

impl Crayon {
    /// Un trait ou un passage de gomme est en cours.
    pub(crate) fn busy(&self) -> bool {
        self.geste.is_some()
    }

    /// Un autre projet est ouvert : le geste en cours est abandonné (l'outil choisi reste).
    pub(crate) fn abandon(&mut self) {
        self.geste = None;
    }
}

impl App {
    /// Choisit l'outil de la planche. Entrer dans le crayon ou la gomme lâche la sélection et le recadrage (leurs poignées ne
    /// servent à rien, et Suppr ne doit rien enlever pendant qu'on dessine) ; en sortir ne change rien d'autre.
    pub(crate) fn refs_set_tool(&mut self, tool: Outil) {
        if self.refs.pen.outil == tool {
            return;
        }
        self.refs.pen.geste = None;
        self.refs.pen.outil = tool;
        if tool != Outil::Selection {
            self.refs.layout = Layout::Board;
            self.refs.sel.clear();
            self.refs.crop_mode = None;
            self.refs.picking = false;
            self.refs.drag = None;
            self.refs_edit_leave_now();
        }
        self.status = match tool {
            Outil::Selection => "Planche : outil de sélection (les gestes d'origine).",
            Outil::Crayon => "Planche : crayon (Échap pour revenir à la sélection ; Espace ou bouton du milieu : déplacer la vue).",
            Outil::Gomme => "Planche : gomme, elle enlève le trait entier qu'elle touche (Échap pour revenir à la sélection).",
        }
        .into();
    }

    /// Le texte qu'on écrit sur place est posé avant de changer d'outil.
    fn refs_edit_leave_now(&mut self) {
        if self.refs.edit.is_some() {
            self.refs_end_edit();
        }
    }

    /// Les commandes du crayon (la palette Ctrl+K, les icônes, les raccourcis choisis par l'utilisateur) ; vrai si `action` en était une.
    pub(crate) fn planchdessin_action(&mut self, action: &str) -> bool {
        let tool = match action {
            "references.crayon" => Outil::Crayon,
            "references.gomme" => Outil::Gomme,
            _ => return false,
        };
        self.show_pane(Pane::References);
        let next = if self.refs.pen.outil == tool { Outil::Selection } else { tool };
        self.refs_set_tool(next);
        true
    }

    /// Les icônes Crayon et Gomme ; avec l'un des deux, l'encre (huit pastilles et un sélecteur) et la taille.
    pub(crate) fn refs_pen_icons(&mut self, ui: &mut egui::Ui) {
        use crate::bulles;
        let tool = self.refs.pen.outil;
        if self.bouton_etat(ui, tool == Outil::Crayon, ph::PENCIL_SIMPLE, "Crayon", "Dessine sur la planche, par-dessus les images, avec la pression du stylet. Les traits suivent le zoom ; l'outil de sélection les déplace et les supprime. Échap revient à la sélection.", "references.crayon").clicked() {
            self.run_action("references.crayon");
        }
        if self.bouton_etat(ui, tool == Outil::Gomme, ph::ERASER, "Gomme de la planche", "Enlève le trait entier que touche le cercle de la gomme (sa taille est celle du crayon). Un passage = une étape d'annulation. Ne touche ni aux images ni aux post-it.", "references.gomme").clicked() {
            self.run_action("references.gomme");
        }
        if tool == Outil::Selection {
            return;
        }
        if tool == Outil::Crayon {
            for (i, ink) in INKS.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                let on = self.refs.pen.encre == *ink;
                let color = Color32::from_rgb(ink[0], ink[1], ink[2]);
                ui.painter().rect_filled(rect.shrink(1.0), 3.0, color);
                ui.painter().rect_stroke(rect.shrink(if on { 0.0 } else { 1.0 }), 3.0, Stroke::new(if on { 2.0_f32 } else { 1.0_f32 }, if on { theme::accent() } else { theme::bord() }), StrokeKind::Inside);
                bulles::bulle(resp.clone(), INK_NAMES[i], "L'encre du crayon.", "");
                if resp.clicked() {
                    self.refs.pen.encre = *ink;
                }
            }
            let mut rgb = self.refs.pen.encre;
            let resp = crate::champcouleur::srgb(ui, &mut rgb);
            bulles::bulle(resp.clone(), "Autre encre", "Choisir une couleur libre pour le crayon.", "");
            if resp.changed() {
                self.refs.pen.encre = rgb;
            }
        }
        let mut width = self.refs.pen.epaisseur;
        let resp = ui.add(egui::DragValue::new(&mut width).range(WIDTH_MIN..=WIDTH_MAX).speed(0.1).max_decimals(1).suffix(" pt"));
        bulles::bulle(resp.clone(), "Taille", "L'épaisseur du trait à pression pleine, en points de la planche (elle grossit avec le zoom). Donne aussi la taille de la gomme.", "");
        if resp.changed() {
            self.refs.pen.epaisseur = width.clamp(WIDTH_MIN, WIDTH_MAX);
        }
    }

    /// Chaque image d'écran, avant tout geste : la pression du stylet et les positions du pointeur arrivées pendant l'image (un
    /// trait suit ainsi tous les échantillons, pas seulement le dernier).
    pub(crate) fn refs_pen_events(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let pen = &mut self.refs.pen;
        pen.mouvements.clear();
        if pen.outil == Outil::Selection {
            return;
        }
        let (mut pression, mut vu) = (pen.pression, pen.tactile_vu);
        let moves = &mut pen.mouvements;
        ctx.input(|i| {
            for e in &i.events {
                match e {
                    egui::Event::Touch { force, .. } => {
                        vu = now;
                        if let Some(f) = force {
                            pression = f.clamp(0.0, 1.0);
                        }
                    }
                    egui::Event::PointerMoved(p) => moves.push((*p, 0.0)),
                    _ => {}
                }
            }
        });
        pen.pression = pression;
        pen.tactile_vu = vu;
        // Une souris : pression pleine (un événement tactile récent, ou en cours, dit que c'est un stylet).
        let stylet = now - vu < 0.6;
        let p = if stylet { pression.max(0.05) } else { 1.0 };
        for m in moves.iter_mut() {
            m.1 = p;
        }
        pen.courante = p;
    }

    /// Un appui sur le tableau avec le crayon ou la gomme : le geste commence (Espace tenu : on déplace la vue).
    pub(crate) fn refs_pen_press(&mut self, view: &View, rect: Rect, p: Pos2, space: bool) {
        // (Un geste fige la vue : le cadrage automatique ne doit pas la déplacer pendant qu'on trace.)
        if self.refs.view.is_none() {
            self.refs.view = self.refs.shown_view;
        }
        if space {
            self.refs_begin_drag(Grab::Pan, p);
            return;
        }
        let tool = self.refs.pen.outil;
        let pressure = self.refs.pen.courante;
        let at = view.to_board(rect, p);
        self.refs.pen.geste = Some(Geste { outil: tool, points: vec![(at, pressure)], epoch: self.doc_epoch, serial: self.history.serial(), snap: None, gommes: 0 });
        if tool == Outil::Gomme {
            self.refs_pen_erase(view, rect, p);
        }
    }

    /// Le geste suit la souris ; il se pose quand le bouton est lâché, s'abandonne avec Échap (le crayon) ou si le document a changé
    /// sous lui (annuler, autre projet).
    pub(crate) fn refs_pen_update(&mut self, view: &View, rect: Rect, down: bool, latest: Option<Pos2>, escape: bool) {
        let Some(g) = self.refs.pen.geste.as_ref() else { return };
        if g.epoch != self.doc_epoch || g.serial != self.history.serial() {
            self.refs.pen.geste = None;
            return;
        }
        if escape && g.outil == Outil::Crayon {
            self.refs.pen.geste = None;
            self.status = "Trait annulé.".into();
            return;
        }
        let tool = g.outil;
        let spots: Vec<(Pos2, f32)> = {
            let mut v = std::mem::take(&mut self.refs.pen.mouvements);
            if v.is_empty() {
                if let Some(p) = latest {
                    v.push((p, self.refs.pen.courante));
                }
            }
            v
        };
        if down {
            match tool {
                Outil::Crayon => {
                    // (Un échantillon à moins d'un demi-point d'écran du précédent n'apporte rien : le trait en cours ne grossit pas pour rien.)
                    let min = 0.5 / view.zoom.max(1e-3);
                    if let Some(g) = self.refs.pen.geste.as_mut() {
                        for (p, pr) in spots {
                            let at = view.to_board(rect, p);
                            if g.points.last().is_none_or(|l| (l.0 - at).length() >= min) {
                                g.points.push((at, pr));
                            }
                        }
                    }
                }
                _ => {
                    for (p, _) in spots {
                        self.refs_pen_erase(view, rect, p);
                    }
                }
            }
        } else {
            self.refs_pen_finish(view);
        }
    }

    /// Le bouton est lâché : le trait entre dans la planche (une étape), ou le passage de gomme finit (une étape s'il a enlevé quelque chose).
    fn refs_pen_finish(&mut self, view: &View) {
        let Some(g) = self.refs.pen.geste.take() else { return };
        match g.outil {
            Outil::Crayon => {
                let (ink, width) = (self.refs.pen.encre, self.refs.pen.epaisseur);
                let id = self.doc.new_reference_id();
                let Some(mut stroke) = Reference::stroke(id, &g.points, ink, width, 0.5 / view.zoom.max(1e-3)) else { return };
                stroke.stamp(&crate::commentaires::author_name(&self.settings.comment_author));
                let snap = self.snapshot();
                self.doc.add_reference(stroke);
                self.refs_push(snap, "Trait de crayon");
            }
            _ => {
                if g.gommes > 0 {
                    if let Some(snap) = g.snap {
                        self.refs_push(snap, "Gommer des traits");
                        self.status = format!("{} trait(s) gommé(s) (Ctrl+Z les rend).", g.gommes);
                    }
                }
            }
        }
    }

    /// La gomme passe en `screen` : tous les traits que son cercle touche s'en vont (le geste n'a qu'une étape d'annulation).
    fn refs_pen_erase(&mut self, view: &View, rect: Rect, screen: Pos2) {
        let at = view.to_board(rect, screen);
        let slack = self.refs_eraser_radius(view) / view.zoom.max(1e-3);
        let hit: Vec<u64> = self.doc.references.iter().filter(|r| r.kind == Kind::Stroke && r.hit_near(at, slack)).map(|r| r.id).collect();
        if hit.is_empty() {
            return;
        }
        let snap = self.refs.pen.geste.as_ref().is_some_and(|g| g.snap.is_none()).then(|| self.snapshot());
        if let Some(g) = self.refs.pen.geste.as_mut() {
            if g.snap.is_none() {
                g.snap = snap;
            }
            g.gommes += hit.len();
        }
        for id in hit {
            self.doc.remove_reference(id);
            self.refs.sel.retain(|s| *s != id);
        }
    }

    /// Le rayon du cercle de la gomme, en points d'écran.
    fn refs_eraser_radius(&self, view: &View) -> f32 {
        (self.refs.pen.epaisseur * view.zoom).clamp(12.0, 80.0) * 0.5
    }

    /// Le trait en cours, et le repère de l'outil sous la souris (un cercle de la taille du crayon ou de la gomme).
    pub(crate) fn refs_pen_paint(&self, ctx: &egui::Context, painter: &egui::Painter, view: &View, rect: Rect, hover: Option<Pos2>, win_alpha: f32) {
        let pen = &self.refs.pen;
        if pen.outil == Outil::Selection {
            return;
        }
        if let Some(Geste { outil: Outil::Crayon, points, .. }) = &pen.geste {
            planchdessin::paint_live(painter, points, pen.encre, pen.epaisseur, view, rect, win_alpha);
        }
        if let Some(p) = hover {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            let radius = if pen.outil == Outil::Gomme { self.refs_eraser_radius(view) } else { (pen.epaisseur * view.zoom * 0.5).max(1.5) };
            painter.circle_stroke(p, radius + 0.5, Stroke::new(1.0_f32, Color32::from_white_alpha(230)));
            painter.circle_stroke(p, radius, Stroke::new(1.0_f32, Color32::from_black_alpha(190)));
        }
    }
}
