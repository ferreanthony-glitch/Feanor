//! L'outil Trajectoire à l'écran (Anthony, 2026-11-07 ; le modèle : `trajectoire.rs`).
//!
//! Avec l'outil, sur le calque courant :
//! * un trait sur la feuille **trace le chemin** du calque (un second trait long le retrace, les marques restent) ;
//! * un petit trait **en travers du chemin pose une marque** ;
//! * un appui sur une marque la **glisse** le long du chemin ; tirée loin du chemin, elle est **arrachée** (retirée au lâcher) ;
//! * un appui sur le chemin le **déplace** en bloc ; Échap abandonne le geste en cours.
//!
//! À chaque changement, les clés de position du calque sont reposées d'après le chemin (`Trajectoire::cles`) : pendant un
//! glisser, à chaque image d'écran, sans rejeu d'aucun dessin (seuls les paramètres du plan à composer changent). Une étape
//! d'annulation par geste. Le chemin ne se voit qu'avec l'outil ; il est enregistré avec le calque et revient avec l'outil.
//! Les silhouettes pâles du calque à chaque jalon sont posées dans le plan à composer (`App::build_plan_avec`).
//!
//! Retoucher les clés à la main (timeline, montage, outil Transformer) **détache** la trajectoire : les clés restent, le
//! chemin s'en va, et la ligne d'état le dit (`App::trajectoire_detacher`).

use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, FontId, Key, Pos2, RichText, Shape, Stroke, Vec2};

use crate::app::{App, Tool};
use crate::history::Edit;
use crate::poskeys::PosKey;
use crate::trajectoire::{distance, Trajectoire};

/// La teinte du chemin, de ses marques et des silhouettes : celle des poignées du calque.
pub(crate) const COULEUR: Color32 = crate::theme::POIGNEE_CALQUE;
/// La teinte des silhouettes du calque aux jalons (dans le plan à composer).
pub(crate) const FANTOME: [u8; 3] = [120, 220, 140];
/// L'opacité des silhouettes.
pub(crate) const FANTOME_OPACITE: u8 = 70;
/// À moins de tant de pixels d'écran, l'appui prend une marque, ou le chemin.
pub(crate) const TOL_ECRAN: f32 = 10.0;
/// Une marque tirée à plus de tant de pixels d'écran du chemin est arrachée (retirée au lâcher).
pub(crate) const ARRACHE_ECRAN: f32 = 40.0;
/// La demi-longueur d'une marque à l'écran.
const DEMI_MARQUE: f32 = 7.0;
/// Ce qu'une commande des clés ajoute à sa ligne d'état quand elle a détaché une trajectoire.
pub(crate) const DETACHEE: &str = " La trajectoire du calque est détachée : le chemin s'en va, les clés restent.";

/// Un geste de l'outil en cours.
pub(crate) enum Geste {
    /// Un trait : le chemin, ou une marque, on le saura au lâcher.
    Trait { points: Vec<[f32; 2]> },
    /// La marque `i` prise : glissée le long du chemin, ou arrachée.
    Marque { i: usize, avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool, arrachee: bool },
    /// Le chemin pris : déplacé en bloc.
    Chemin { depart: [f32; 2], avant: Trajectoire, cles_avant: Vec<PosKey>, snap: Edit, bouge: bool },
}

/// L'état de l'outil pour un projet.
#[derive(Default)]
pub(crate) struct Etat {
    pub(crate) geste: Option<Geste>,
    /// « Une marque = N images », proposé à la prochaine trajectoire tracée (rien : la durée du plan partagée entre les marques).
    pub(crate) pas_propose: Option<usize>,
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 { "s" } else { "" }
}

impl App {
    /// Combien de pixels d'écran vaut un pixel de la feuille, autour du point `p` de la feuille.
    fn trajectoire_echelle(&self, p: Pos2) -> f32 {
        let (a, b) = (self.stage_to_screen(p), self.stage_to_screen(p + vec2(1.0, 0.0)));
        (b - a).length().max(1e-3)
    }

    /// La marque sous le point `p` de la feuille (à `TOL_ECRAN` près), s'il y en a une ; sinon vrai si le chemin est sous `p`.
    fn trajectoire_sous(&self, p: Pos2) -> (Option<usize>, bool) {
        let Some(t) = self.layers().get(self.cur_layer).and_then(|l| l.trajectoire.as_ref()) else { return (None, false) };
        let tol = TOL_ECRAN / self.trajectoire_echelle(p);
        let q = [p.x, p.y];
        match t.marque_pres_de(q, tol) {
            Some(i) => (Some(i), true),
            None => (None, t.plus_proche(q).1 <= tol),
        }
    }

    /// Appui avec l'outil : une marque, le chemin, ou le début d'un trait.
    pub(crate) fn trajectoire_presser(&mut self, stage: Pos2) {
        self.end_stroke();
        self.playing = false;
        if self.content_refuses() {
            return;
        }
        if !self.layer().drawable() {
            self.layer_editable();
            return;
        }
        if self.layer().keys_follow_line() {
            self.status = "Ce calque de couleur suit son calque de trait : trace la trajectoire sur le calque de trait.".into();
            return;
        }
        let p = [stage.x, stage.y];
        let (marque, chemin) = self.trajectoire_sous(stage);
        let l = self.layer();
        if let (Some(avant), true) = (l.trajectoire.clone(), chemin) {
            let cles_avant = l.pos_keys.clone();
            let snap = self.snapshot();
            self.traj.geste = Some(match marque {
                Some(i) => Geste::Marque { i, avant, cles_avant, snap, bouge: false, arrachee: false },
                None => Geste::Chemin { depart: p, avant, cles_avant, snap, bouge: false },
            });
            return;
        }
        self.traj.geste = Some(Geste::Trait { points: vec![p] });
    }

    /// Le pointeur bouge pendant un geste de l'outil ; vrai si un geste est en cours.
    pub(crate) fn trajectoire_glisser(&mut self, stage: Pos2) -> bool {
        let Some(mut g) = self.traj.geste.take() else { return false };
        let p = [stage.x, stage.y];
        let li = self.cur_layer;
        match &mut g {
            Geste::Trait { points } => {
                if points.last().is_none_or(|l| distance(*l, p) > 0.5) {
                    points.push(p);
                }
            }
            Geste::Marque { i, avant, bouge, arrachee, .. } => {
                let (f, d) = avant.plus_proche(p);
                let loin = d > ARRACHE_ECRAN / self.trajectoire_echelle(stage);
                let mut t = avant.clone();
                if loin {
                    t.retirer_marque(*i);
                } else {
                    t.deplacer_marque(*i, f);
                }
                *arrachee = loin;
                *bouge = true;
                if let Some(l) = self.layers_mut().get_mut(li) {
                    l.trajectoire = Some(t);
                }
                self.trajectoire_recalculer(li);
                self.renderer.invalidate_params();
                self.status = if loin { "Marque arrachée : lâcher la retire.".into() } else { format!("Marque à {} % du chemin.", (f * 100.0).round()) };
            }
            Geste::Chemin { depart, avant, bouge, .. } => {
                let mut t = avant.clone();
                t.deplacer(p[0] - depart[0], p[1] - depart[1]);
                *bouge = true;
                if let Some(l) = self.layers_mut().get_mut(li) {
                    l.trajectoire = Some(t);
                }
                self.trajectoire_recalculer(li);
                self.renderer.invalidate_params();
            }
        }
        self.traj.geste = Some(g);
        true
    }

    /// Lâcher : le trait devient marque ou chemin ; une marque glissée, arrachée, un chemin déplacé sont gardés, en une étape.
    pub(crate) fn trajectoire_lacher(&mut self) {
        let Some(g) = self.traj.geste.take() else { return };
        let li = self.cur_layer;
        if li >= self.layers().len() {
            return;
        }
        match g {
            Geste::Trait { points } => self.trajectoire_poser_le_trait(li, &points),
            Geste::Marque { snap, bouge, arrachee, .. } => {
                if arrachee {
                    self.trajectoire_valider(snap, "Retirer une marque");
                    self.status = self.trajectoire_resume(li, "Marque retirée");
                } else if bouge {
                    self.trajectoire_valider(snap, "Glisser une marque");
                }
            }
            Geste::Chemin { snap, bouge, .. } => {
                if bouge {
                    self.trajectoire_valider(snap, "Déplacer la trajectoire");
                }
            }
        }
    }

    /// Un trait lâché : une marque s'il croise le chemin en étant court, sinon le chemin (neuf, ou retracé), sinon rien.
    fn trajectoire_poser_le_trait(&mut self, li: usize, points: &[[f32; 2]]) {
        let existante = self.layers()[li].trajectoire.clone();
        if let Some(t) = &existante {
            if let Some(f) = t.marque_de(points) {
                let snap = self.snapshot();
                let posee = self.layers_mut()[li].trajectoire.as_mut().and_then(|t| t.ajouter_marque(f)).is_some();
                if posee {
                    self.trajectoire_recalculer(li);
                    self.trajectoire_valider(snap, "Marque de trajectoire");
                    self.status = self.trajectoire_resume(li, "Marque posée");
                } else {
                    self.status = "Une marque est déjà là (ou c'est un bout du chemin) : glisse-la plutôt.".into();
                }
                return;
            }
        }
        match Trajectoire::nouvelle(points) {
            Some(mut t) => {
                let snap = self.snapshot();
                let nom = if existante.is_some() { "Retracer la trajectoire" } else { "Trajectoire" };
                match existante {
                    Some(e) => {
                        t.marques = e.marques;
                        t.images_par_marque = e.images_par_marque;
                    }
                    None => t.images_par_marque = self.traj.pas_propose,
                }
                self.layers_mut()[li].trajectoire = Some(t);
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, nom);
                self.status = self.trajectoire_resume(li, if nom == "Trajectoire" { "Trajectoire tracée" } else { "Chemin retracé, marques gardées" });
            }
            None if existante.is_none() => {
                self.status = "Trajectoire : trace d'un trait le chemin du calque, puis de petits traits en travers pour les marques.".into();
            }
            None => {}
        }
    }

    /// Les clés de position du calque `li` reposées d'après sa trajectoire (rien s'il n'en a pas). La pose au départ reste
    /// celle que le calque a à l'image 0 ; un chemin qui ne couvre aucune image (plan d'une image) laisse le calque fixe.
    pub(crate) fn trajectoire_recalculer(&mut self, li: usize) {
        let duree = self.frame_count();
        let Some(l) = self.layers_mut().get_mut(li) else { return };
        let Some(t) = &l.trajectoire else { return };
        let base = crate::poskeys::pose_at(&l.pos_keys, 0, l.transform);
        l.pos_keys = t.cles(duree, base);
        if l.pos_keys.is_empty() {
            l.transform = base;
            l.fold_dz();
        }
    }

    /// Une étape d'annulation au nom `nom`, depuis l'état `snap` d'avant le geste.
    fn trajectoire_valider(&mut self, snap: Edit, nom: &str) {
        self.modified = true;
        self.commit_named(snap, nom);
    }

    /// « Marque posée : 3 marques, le chemin dure de l'image 1 à 36. »
    fn trajectoire_resume(&self, li: usize, debut: &str) -> String {
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.as_ref()) else { return debut.to_string() };
        let images = t.images_des_jalons(self.frame_count());
        let n = t.marques.len();
        match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("{debut} : {n} marque{}, le chemin dure de l'image {} à l'image {}.", pluriel(n), a + 1, b + 1),
            _ => format!("{debut} : {n} marque{} ; le plan n'a qu'une image, le calque ne bouge pas encore.", pluriel(n)),
        }
    }

    /// Échap pendant un geste de l'outil : tout revient comme avant le geste ; vrai si la touche a servi.
    pub(crate) fn trajectoire_touche(&mut self, key: Key) -> bool {
        if key != Key::Escape || self.tool != Tool::Trajectoire {
            return false;
        }
        let Some(g) = self.traj.geste.take() else { return false };
        let li = self.cur_layer;
        if let Geste::Marque { avant, cles_avant, .. } | Geste::Chemin { avant, cles_avant, .. } = g {
            if let Some(l) = self.layers_mut().get_mut(li) {
                l.trajectoire = Some(avant);
                l.pos_keys = cles_avant;
            }
            self.renderer.invalidate_params();
        }
        self.status = "Geste de trajectoire abandonné.".into();
        true
    }

    /// Les clés du calque `li` viennent d'être retouchées à la main (`pourquoi`) : sa trajectoire ne les commande plus, elle s'en va.
    /// Vrai si une trajectoire a été détachée (la ligne d'état le dit ; un appelant qui écrit la sienne ensuite y ajoute `DETACHEE`).
    pub(crate) fn trajectoire_detacher(&mut self, li: usize, pourquoi: &str) -> bool {
        let Some(l) = self.layers_mut().get_mut(li) else { return false };
        if l.trajectoire.take().is_none() {
            return false;
        }
        self.status = format!("Trajectoire détachée ({pourquoi}) : les clés de position restent, à ta main.");
        true
    }

    /// Le curseur de l'outil au point `p` de l'écran : la main sur une marque ou le chemin, la croix ailleurs.
    pub(crate) fn trajectoire_curseur(&self, p: Pos2) -> CursorIcon {
        match &self.traj.geste {
            Some(Geste::Marque { .. } | Geste::Chemin { .. }) => return CursorIcon::Grabbing,
            Some(Geste::Trait { .. }) => return CursorIcon::Crosshair,
            None => {}
        }
        let stage = self.view.to_canvas(self.canvas_rect, self.canvas_size(), p);
        match self.trajectoire_sous(stage) {
            (Some(_), _) => CursorIcon::Grab,
            (None, true) => CursorIcon::Move,
            _ => CursorIcon::Crosshair,
        }
    }

    // ------------------------------------------------------------------ dessin par-dessus

    /// Le chemin, sa barre de départ, sa flèche d'arrivée, ses marques avec l'image où elles tombent, le point de l'image courante,
    /// et le trait en cours.
    pub(crate) fn paint_trajectoire(&self, painter: &egui::Painter) {
        if self.tool != Tool::Trajectoire || self.in_card {
            return;
        }
        let ecran = |p: [f32; 2]| self.stage_to_screen(pos2(p[0], p[1]));
        if let Some(Geste::Trait { points }) = &self.traj.geste {
            if points.len() >= 2 {
                let pts: Vec<Pos2> = points.iter().map(|p| ecran(*p)).collect();
                painter.add(Shape::line(pts, Stroke::new(1.5_f32, COULEUR.gamma_multiply(0.7))));
            }
        }
        let Some(l) = self.layers().get(self.cur_layer) else { return };
        let Some(t) = &l.trajectoire else { return };
        let duree = self.frame_count();
        let pts: Vec<Pos2> = t.points.iter().map(|p| ecran(*p)).collect();
        painter.add(Shape::line(pts.clone(), Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
        painter.add(Shape::line(pts, Stroke::new(1.5_f32, COULEUR)));
        // La direction du chemin à l'écran, à la fraction `f`.
        let direction = |f: f32| -> Vec2 {
            let (a, b) = (ecran(t.point_a((f - 0.01).max(0.0))), ecran(t.point_a((f + 0.01).min(1.0))));
            let d = b - a;
            if d.length() > 1e-3 { d.normalized() } else { vec2(1.0, 0.0) }
        };
        let normale = |d: Vec2| vec2(-d.y, d.x);
        // La marque prise, ou survolée.
        let tenue = match &self.traj.geste {
            Some(Geste::Marque { i, arrachee: false, .. }) => Some(*i),
            Some(Geste::Marque { arrachee: true, .. }) => None,
            _ => self.hover_canvas.and_then(|h| self.trajectoire_sous(h).0),
        };
        let images = t.images_des_jalons(duree);
        let jalons = t.jalons();
        let dernier = jalons.len() - 1;
        for (k, &f) in jalons.iter().enumerate() {
            let c = ecran(t.point_a(f));
            let d = direction(f);
            let n = normale(d);
            if k == 0 {
                // Le départ : une barre.
                painter.line_segment([c - n * (DEMI_MARQUE + 2.0), c + n * (DEMI_MARQUE + 2.0)], Stroke::new(2.5_f32, COULEUR));
            } else if k == dernier {
                // L'arrivée : une pointe de flèche.
                let (g, h) = (c - d * 11.0 + n * 6.0, c - d * 11.0 - n * 6.0);
                painter.add(Shape::line(vec![g, c, h], Stroke::new(3.5_f32, Color32::from_black_alpha(90))));
                painter.add(Shape::line(vec![g, c, h], Stroke::new(2.0_f32, COULEUR)));
            } else {
                let i = k - 1;
                let prise = tenue == Some(i);
                let (a, b) = (c - n * DEMI_MARQUE, c + n * DEMI_MARQUE);
                painter.line_segment([a, b], Stroke::new(if prise { 5.0_f32 } else { 3.5_f32 }, Color32::from_black_alpha(90)));
                painter.line_segment([a, b], Stroke::new(2.0_f32, if prise { Color32::WHITE } else { COULEUR }));
            }
            if let Some(img) = images.get(k) {
                let cote = if n.y <= 0.0 { n } else { -n };
                painter.text(c + cote * (DEMI_MARQUE + 4.0), Align2::CENTER_CENTER, format!("{}", img + 1), FontId::proportional(10.0), COULEUR);
            }
        }
        // Où en est le calque à l'image courante.
        if let Some(f) = t.fraction_a_l_image(self.cur_frame, duree) {
            let c = ecran(t.point_a(f));
            painter.circle(c, 4.0, COULEUR, Stroke::new(1.5_f32, Color32::from_black_alpha(140)));
        }
    }

    // ------------------------------------------------------------------ réglages

    /// Réglages de l'outil (panneau Réglages, fenêtre du clic droit) : le temps entre deux marques, les commandes de la trajectoire du
    /// calque courant.
    pub(crate) fn trajectoire_options_ui(&mut self, ui: &mut egui::Ui) {
        let li = self.cur_layer;
        let duree = self.frame_count();
        let Some(t) = self.layers().get(li).and_then(|l| l.trajectoire.clone()) else {
            ui.label(
                RichText::new(
                    "Trace le chemin du calque d'un trait : le calque le suit sur la durée du plan, en partant de là où il est. Puis de petits \
                     traits en travers du chemin : les marques. Le temps est le même entre deux marques ; serrées, le calque va lentement ; \
                     espacées, il va vite.",
                )
                .small()
                .weak(),
            );
            let mut pas = self.traj.pas_propose;
            if pas_ui(ui, &mut pas) {
                self.traj.pas_propose = pas;
            }
            return;
        };
        let n = t.marques.len();
        let images = t.images_des_jalons(duree);
        ui.label(match (images.first(), images.last()) {
            (Some(a), Some(b)) => format!("{n} marque{} · le chemin dure de l'image {} à l'image {} · {} clés de position.", pluriel(n), a + 1, b + 1, self.layers()[li].pos_keys.len()),
            _ => format!("{n} marque{} · le plan n'a qu'une image : le calque ne bouge pas encore.", pluriel(n)),
        });
        let mut pas = t.images_par_marque;
        if pas_ui(ui, &mut pas) {
            self.named_step("Pas des marques", |app| app.note_change(egui::Id::new("trajectoire.pas")));
            if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                tr.images_par_marque = pas;
            }
            self.traj.pas_propose = pas;
            self.trajectoire_recalculer(li);
            self.renderer.invalidate_all();
        }
        ui.horizontal_wrapped(|ui| {
            if n > 0 && ui.button("Retirer les marques").on_hover_text("Le chemin reste, à vitesse constante.").clicked() {
                let snap = self.snapshot();
                if let Some(tr) = self.layers_mut()[li].trajectoire.as_mut() {
                    tr.marques.clear();
                }
                self.trajectoire_recalculer(li);
                self.trajectoire_valider(snap, "Retirer les marques");
            }
            if ui.button("Détacher").on_hover_text("Les clés de position restent telles quelles, le chemin s'en va : tu les retouches à la main.").clicked() {
                let snap = self.snapshot();
                self.layers_mut()[li].trajectoire = None;
                self.trajectoire_valider(snap, "Détacher la trajectoire");
                self.status = "Trajectoire détachée : les clés de position restent, à ta main.".into();
            }
            if ui.button("Effacer").on_hover_text("Le chemin et ses clés s'en vont ; le calque garde la pose qu'il a au départ.").clicked() {
                let snap = self.snapshot();
                let l = &mut self.layers_mut()[li];
                l.transform = crate::poskeys::pose_at(&l.pos_keys, 0, l.transform);
                l.pos_keys.clear();
                l.fold_dz();
                l.trajectoire = None;
                self.trajectoire_valider(snap, "Effacer la trajectoire");
                self.status = "Trajectoire effacée : le calque reste où il était au départ.".into();
            }
        });
        ui.label(
            RichText::new(
                "Glisser une marque le long du chemin la déplace ; la tirer loin du chemin la retire. Glisser le chemin le déplace en bloc. \
                 Un trait long retrace le chemin, les marques restent. Retoucher une clé à la main (timeline, Transformer) détache la trajectoire.",
            )
            .small()
            .weak(),
        );
    }
}

/// « Le temps entre deux marques » : la durée du plan partagée, ou un nombre d'images ; vrai si le choix a changé.
fn pas_ui(ui: &mut egui::Ui, pas: &mut Option<usize>) -> bool {
    let avant = *pas;
    ui.label("Le temps entre deux marques :");
    let mut fixe = pas.is_some();
    ui.horizontal(|ui| {
        if ui.radio(!fixe, "la durée du plan, partagée").clicked() {
            fixe = false;
        }
        if ui.radio(fixe, "un nombre d'images").clicked() {
            fixe = true;
        }
    });
    if fixe {
        let mut n = pas.unwrap_or(4).max(1);
        ui.add(egui::Slider::new(&mut n, 1..=48).text("images par marque"));
        *pas = Some(n);
    } else {
        *pas = None;
    }
    *pas != avant
}
