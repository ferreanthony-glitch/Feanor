//! Régler le stylet à sa main (passe « simplicité », 2026-10-21) : une petite fenêtre (Aide > Régler le stylet…, ou
//! Préférences > Dessin) où l'on trace trois traits, comme on écrit : un trait léger, un trait moyen, un trait appuyé.
//! SHOT mesure l'appui de chacun et en tire la **courbe de pression** du stylet : celle que le panneau Réglages de
//! l'outil montre déjà, en bas des réglages du pinceau (« Toucher du stylet », `Toucher::pressure_curve`, réglage de
//! l'application commun à tous les pinceaux). Le but : que « léger » donne un trait fin mais présent, « moyen » un trait
//! de corps moyen et « appuyé » le trait plein, quelle que soit la main ou la tablette.
//!
//! - **Aperçu avant de poser** : les trois traits sont redessinés avec le réglage d'aujourd'hui et avec celui qu'on
//!   propose, côte à côte, et les deux courbes sont tracées. Rien ne change dans SHOT tant qu'on n'a pas cliqué
//!   « Appliquer ».
//! - **Rétablir** : après « Appliquer », « Rétablir » rend la courbe d'avant. (Le bouton « Droite » du panneau Réglages de
//!   l'outil, lui, remet la courbe d'origine.)
//! - **La souris** appuie toujours à 100 % (`canvas.rs` : un appui qui ne vient pas du stylet vaut 1) : il n'y a rien à
//!   régler avec elle, la fenêtre le dit et ne retient pas ses traits.
//!
//! La mesure (`appui_typique`, `courbe_pour`) est séparée de la fenêtre pour être essayée sans écran.

use eframe::egui::{self, pos2, vec2, Align2, Color32, Event, Pos2, Rect, RichText, Sense, Stroke, TouchPhase};

use crate::app::App;
use crate::brush::Curve;
use crate::theme;

/// Les trois sortes de traits demandés, dans l'ordre.
pub const SORTES: [&str; 3] = ["léger", "moyen", "appuyé"];

/// L'appui voulu en sortie de la courbe pour chacun des trois traits : fin mais présent, de corps moyen, plein.
pub const CIBLES: [f32; 3] = [0.2, 0.55, 1.0];

/// Au moins ce nombre d'échantillons pour qu'un trait compte (un clic ou un point ne dit rien de l'appui).
pub const ECHANTILLONS_MIN: usize = 6;

/// Écart d'appui minimal entre deux sortes de traits : plus proches, on ne peut pas dire que la main les distingue.
pub const ECART_MIN: f32 = 0.04;

/// Taille de la zone où l'on trace, en points.
const ZONE: [f32; 2] = [400.0, 110.0];

/// Pourquoi la courbe ne peut pas être proposée.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refus {
    /// Moins de trois traits mesurés.
    Incomplet,
    /// Deux sortes de traits ont à peu près le même appui.
    TropProches,
}

impl Refus {
    pub fn texte(self) -> &'static str {
        match self {
            Refus::Incomplet => "Trace les trois traits pour voir le réglage proposé.",
            Refus::TropProches => "Les trois traits se ressemblent trop : appuie vraiment plus fort au troisième, et plus doucement au premier.",
        }
    }
}

/// L'appui typique d'un trait : la médiane de son milieu (le début et la fin, où l'appui monte et retombe, ne comptent
/// pas). Rien si le trait a trop peu d'échantillons.
pub fn appui_typique(appuis: &[f32]) -> Option<f32> {
    let v: Vec<f32> = appuis.iter().copied().filter(|p| p.is_finite()).map(|p| p.clamp(0.0, 1.0)).collect();
    if v.len() < ECHANTILLONS_MIN {
        return None;
    }
    let n = v.len();
    let (a, b) = (n / 5, n - n / 5);
    let mut milieu = v[a..b.max(a + 1)].to_vec();
    milieu.sort_by(|x, y| x.total_cmp(y));
    Some(milieu[milieu.len() / 2])
}

/// La courbe de pression pour une main dont les trois traits (léger, moyen, appuyé) ont ces appuis typiques : elle passe par
/// (léger, 0,2), (moyen, 0,55) et (appuyé, 1), de la même façon pour toutes les mains. Au-dessus de l'appui du trait appuyé,
/// la réponse reste pleine.
pub fn courbe_pour(appuis: [f32; 3]) -> Result<Curve, Refus> {
    let [l, m, f] = appuis.map(|p| p.clamp(0.01, 1.0));
    if m - l < ECART_MIN || f - m < ECART_MIN {
        return Err(Refus::TropProches);
    }
    let mut pts = vec![[0.0, 0.0], [l, CIBLES[0]], [m, CIBLES[1]], [f, CIBLES[2]]];
    if f < 0.999 {
        pts.push([1.0, 1.0]);
    }
    Ok(Curve::from_points(&pts))
}

/// Un trait tracé dans la zone : ses points et l'appui à chacun.
#[derive(Clone, Debug, Default)]
pub struct Trait {
    pub points: Vec<Pos2>,
    pub appuis: Vec<f32>,
}

/// La fenêtre.
pub struct Reglage {
    /// Les traits finis (au plus trois), dans l'ordre léger, moyen, appuyé.
    pub traits: Vec<Trait>,
    en_cours: Option<(Trait, bool)>,
    /// Un appui de stylet (événement tactile) a été vu juste avant l'appui du pointeur.
    touche_debut: bool,
    derniere: f32,
    /// Un trait à la souris a été tracé (et refusé).
    pub souris_vue: bool,
    /// Un trait au stylet a été reçu.
    pub stylet_vu: bool,
    /// Après « Appliquer » : la courbe d'avant, que « Rétablir » rend.
    pub a_retablir: Option<Curve>,
}

impl Reglage {
    pub fn nouveau() -> Self {
        Reglage { traits: Vec::new(), en_cours: None, touche_debut: false, derniere: 0.0, souris_vue: false, stylet_vu: false, a_retablir: None }
    }

    /// Le trait à demander maintenant (0, 1, 2), ou rien quand les trois sont faits.
    pub fn etape(&self) -> Option<usize> {
        (self.traits.len() < SORTES.len()).then_some(self.traits.len())
    }

    pub fn recommencer(&mut self) {
        self.traits.clear();
        self.en_cours = None;
        self.souris_vue = false;
    }

    /// Lit les événements d'une image d'écran dans la zone `zone`. Même lecture que le dessin (`canvas.rs`) : un événement
    /// tactile porte l'appui du stylet, le pointeur donne la position ; sans événement tactile avant l'appui, c'est la
    /// souris.
    pub fn lire(&mut self, zone: Rect, events: &[Event]) {
        for e in events {
            match e {
                Event::Touch { phase, force, .. } => {
                    if let Some(f) = force {
                        self.derniere = f.clamp(0.0, 1.0);
                    }
                    if *phase == TouchPhase::Start {
                        self.touche_debut = true;
                    }
                }
                Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, .. } => {
                    let stylet = std::mem::take(&mut self.touche_debut);
                    if zone.contains(*pos) && self.etape().is_some() {
                        let appui = if stylet { self.derniere } else { 1.0 };
                        self.en_cours = Some((Trait { points: vec![*pos], appuis: vec![appui] }, stylet));
                    }
                }
                Event::PointerMoved(pos) => {
                    if let Some((t, stylet)) = self.en_cours.as_mut() {
                        t.points.push(*pos);
                        t.appuis.push(if *stylet { self.derniere } else { 1.0 });
                    }
                }
                Event::PointerButton { button: egui::PointerButton::Primary, pressed: false, .. } => {
                    if let Some((t, stylet)) = self.en_cours.take() {
                        if !stylet {
                            self.souris_vue = true;
                        } else if appui_typique(&t.appuis).is_some() {
                            self.stylet_vu = true;
                            self.traits.push(t);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Les appuis typiques des trois traits, une fois tous tracés.
    pub fn appuis(&self) -> Option<[f32; 3]> {
        if self.traits.len() < SORTES.len() {
            return None;
        }
        let mut out = [0.0; 3];
        for (i, t) in self.traits.iter().take(3).enumerate() {
            out[i] = appui_typique(&t.appuis)?;
        }
        Some(out)
    }

    /// La courbe proposée.
    pub fn proposition(&self) -> Result<Curve, Refus> {
        courbe_pour(self.appuis().ok_or(Refus::Incomplet)?)
    }
}

/// L'épaisseur, en points, d'un trait d'aperçu à cet appui.
fn epaisseur(courbe: &Curve, appui: f32) -> f32 {
    1.0 + 9.0 * courbe.eval(appui)
}

/// Un trait redessiné dans `cellule`, à l'échelle, avec l'épaisseur que `courbe` donne à chacun de ses appuis.
fn dessiner_trait(painter: &egui::Painter, cellule: Rect, t: &Trait, courbe: &Curve, couleur: Color32) {
    let Some(boite) = t.points.iter().fold(None::<Rect>, |b, p| Some(b.map_or(Rect::from_min_max(*p, *p), |b| b.union(Rect::from_min_max(*p, *p))))) else { return };
    let echelle = (cellule.width() / boite.width().max(1.0)).min(cellule.height() / boite.height().max(1.0)).min(1.0);
    let place = |p: Pos2| cellule.center() + (p - boite.center()) * echelle;
    let mut precedent: Option<(Pos2, f32)> = None;
    for (p, a) in t.points.iter().zip(&t.appuis) {
        let (q, w) = (place(*p), epaisseur(courbe, *a));
        painter.circle_filled(q, w / 2.0, couleur);
        if let Some((r, wr)) = precedent {
            painter.line_segment([r, q], Stroke::new((w + wr) / 2.0, couleur));
        }
        precedent = Some((q, w));
    }
}

/// Le graphique des deux courbes : l'actuelle (grise) et la proposée (accent).
fn dessiner_courbes(painter: &egui::Painter, zone: Rect, actuelle: &Curve, proposee: Option<&Curve>) {
    painter.rect_filled(zone, 4.0, theme::apercu_trait());
    painter.rect_stroke(zone, 4.0, Stroke::new(1.0_f32, theme::bord()), egui::StrokeKind::Inside);
    let en_points = |c: &Curve| -> Vec<Pos2> { (0..=32).map(|k| { let x = k as f32 / 32.0; pos2(zone.left() + x * zone.width(), zone.bottom() - c.eval(x) * zone.height()) }).collect() };
    painter.add(egui::Shape::line(en_points(actuelle), Stroke::new(1.5_f32, theme::texte_doux())));
    if let Some(c) = proposee {
        painter.add(egui::Shape::line(en_points(c), Stroke::new(2.0_f32, theme::accent_texte())));
    }
}

impl App {
    /// Ouvre la fenêtre (Aide > Régler le stylet…, Préférences > Dessin).
    pub(crate) fn ouvrir_reglage_stylet(&mut self) {
        self.confort.stylet = Some(Reglage::nouveau());
    }

    /// Chaque image d'écran, la fenêtre si elle est ouverte.
    pub(crate) fn reglage_stylet_ui(&mut self, ctx: &egui::Context) {
        let Some(mut r) = self.confort.stylet.take() else { return };
        let mut ouverte = true;
        let mut appliquer = None;
        let mut retablir = false;
        let mut recommencer = false;
        let actuelle = self.toucher.pressure_curve;
        egui::Window::new("Régler le stylet à ma main").open(&mut ouverte).collapsible(false).resizable(false).anchor(Align2::CENTER_TOP, [0.0, 90.0]).show(ctx, |ui| {
            ui.set_width(ZONE[0] + 8.0);
            ui.add(egui::Label::new(RichText::new("Trace trois traits dans le cadre, comme tu écris : un trait léger, un trait moyen, puis un trait appuyé. SHOT règle la courbe de pression sur ta main.").color(theme::texte_doux())).wrap());
            ui.add_space(4.0);
            match r.etape() {
                Some(i) => ui.label(RichText::new(format!("{}/3  ·  un trait {}", i + 1, SORTES[i])).strong().size(15.0).color(theme::texte_fort())),
                None => ui.label(RichText::new("Les trois traits sont tracés.").strong().size(15.0).color(theme::texte_fort())),
            };
            let (reponse, painter) = ui.allocate_painter(vec2(ZONE[0], ZONE[1]), Sense::hover());
            let zone = reponse.rect;
            painter.rect_filled(zone, 6.0, theme::apercu_trait());
            painter.rect_stroke(zone, 6.0, Stroke::new(1.0_f32, theme::bord()), egui::StrokeKind::Inside);
            let events = ui.ctx().input(|i| i.events.clone());
            r.lire(zone, &events);
            // Les traits déjà tracés et celui qui l'est, avec le réglage d'aujourd'hui.
            let teinte = theme::texte_fort();
            let clair = painter.with_clip_rect(zone);
            for t in &r.traits {
                for (p, a) in t.points.iter().zip(&t.appuis) {
                    clair.circle_filled(*p, epaisseur(&actuelle, *a) / 2.0, teinte.gamma_multiply(0.8));
                }
            }
            if let Some((t, _)) = r.en_cours.as_ref() {
                for (p, a) in t.points.iter().zip(&t.appuis) {
                    clair.circle_filled(*p, epaisseur(&actuelle, *a) / 2.0, teinte);
                }
                ui.ctx().request_repaint();
            }
            if r.traits.is_empty() && r.en_cours.is_none() {
                painter.text(zone.center(), Align2::CENTER_CENTER, "Trace ici avec le stylet", egui::FontId::proportional(14.0), theme::texte_doux());
            }
            if r.souris_vue && !r.stylet_vu {
                ui.add_space(4.0);
                ui.add(egui::Label::new(RichText::new("Avec la souris, l'appui est toujours de 100 % : il n'y a rien à régler. Prends le stylet.").color(theme::avertissement())).wrap());
            }
            // Les appuis mesurés.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (i, nom) in SORTES.iter().enumerate() {
                    let texte = match r.traits.get(i).and_then(|t| appui_typique(&t.appuis)) {
                        Some(a) => format!("{nom} : {:.0} %", a * 100.0),
                        None => format!("{nom} : -"),
                    };
                    ui.label(RichText::new(texte).small().color(theme::texte_doux()));
                    ui.add_space(8.0);
                }
            });
            // L'aperçu : les trois traits avec le réglage d'aujourd'hui, puis avec celui qu'on propose.
            let proposition = r.proposition();
            if r.etape().is_none() {
                ui.separator();
                match &proposition {
                    Ok(neuve) => {
                        ui.horizontal(|ui| {
                            for (titre, courbe) in [("Aujourd'hui", actuelle), ("Proposé pour ta main", *neuve)] {
                                ui.vertical(|ui| {
                                    ui.label(RichText::new(titre).strong().color(if titre == "Aujourd'hui" { theme::texte_doux() } else { theme::accent_texte() }));
                                    let (rep, p) = ui.allocate_painter(vec2(190.0, 56.0), Sense::hover());
                                    p.rect_filled(rep.rect, 4.0, theme::apercu_trait());
                                    let largeur = rep.rect.width() / 3.0;
                                    for (i, t) in r.traits.iter().enumerate() {
                                        let cellule = Rect::from_min_size(rep.rect.min + vec2(largeur * i as f32, 0.0), vec2(largeur, rep.rect.height())).shrink(5.0);
                                        dessiner_trait(&p, cellule, t, &courbe, theme::texte_fort());
                                    }
                                });
                            }
                        });
                        ui.add_space(4.0);
                        let (rep, p) = ui.allocate_painter(vec2(ZONE[0], 70.0), Sense::hover());
                        // Les deux courbes, dans un carré à gauche, et une phrase à droite.
                        let carre = Rect::from_min_size(rep.rect.min, vec2(70.0, 70.0));
                        dessiner_courbes(&p, carre, &actuelle, Some(neuve));
                        p.text(rep.rect.min + vec2(82.0, 4.0), Align2::LEFT_TOP, "Courbe de pression : l'appui du stylet (en bas)\ndevient la réponse du trait (en haut).", egui::FontId::proportional(12.0), theme::texte_doux());
                        // La légende des deux courbes, avec leurs couleurs.
                        for (k, (couleur, nom)) in [(theme::texte_doux(), "aujourd'hui"), (theme::accent_texte(), "proposée")].into_iter().enumerate() {
                            let y = rep.rect.min.y + 36.0 + k as f32 * 15.0;
                            p.line_segment([pos2(rep.rect.min.x + 82.0, y + 6.0), pos2(rep.rect.min.x + 106.0, y + 6.0)], Stroke::new(2.0_f32, couleur));
                            p.text(pos2(rep.rect.min.x + 112.0, y), Align2::LEFT_TOP, nom, egui::FontId::proportional(12.0), theme::texte_doux());
                        }
                    }
                    Err(refus) => {
                        ui.add(egui::Label::new(RichText::new(refus.texte()).color(theme::avertissement())).wrap());
                    }
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let propose = proposition.as_ref().ok().copied();
                let b = ui.add_enabled(propose.is_some(), egui::Button::new(RichText::new("Appliquer").strong()));
                if crate::bulles::bulle(b, "Appliquer", "Pose cette courbe de pression pour tous les pinceaux. « Rétablir » rend celle d'avant.", "").clicked() {
                    appliquer = propose;
                }
                let b = ui.add_enabled(r.a_retablir.is_some(), egui::Button::new("Rétablir"));
                if crate::bulles::bulle(b, "Rétablir", "Rend la courbe de pression d'avant « Appliquer ».", "").clicked() {
                    retablir = true;
                }
                let b = ui.add_enabled(!r.traits.is_empty() || r.souris_vue, egui::Button::new("Recommencer"));
                if crate::bulles::bulle(b, "Recommencer", "Efface les traits pour en tracer trois autres. Rien n'est changé dans SHOT.", "").clicked() {
                    recommencer = true;
                }
            });
            ui.label(RichText::new("Ne change rien tant que tu n'as pas cliqué « Appliquer ». Avec la souris, l'appui vaut toujours 100 %.").small().weak());
        });
        if let Some(neuve) = appliquer {
            r.a_retablir = Some(self.toucher.pressure_curve);
            self.toucher.pressure_curve = neuve;
            self.status = "Courbe de pression réglée sur ta main (Réglages de l'outil > Toucher du stylet pour la voir ; Rétablir dans la fenêtre pour revenir).".into();
        }
        if retablir {
            if let Some(avant) = r.a_retablir.take() {
                self.toucher.pressure_curve = avant;
                self.status = "Courbe de pression d'avant rétablie.".into();
            }
        }
        if recommencer {
            r.recommencer();
        }
        if ouverte {
            self.confort.stylet = Some(r);
        }
    }
}
