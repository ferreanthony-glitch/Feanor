//! La pile des versions dans le montage (`pilevers.rs` pour la logique) : l'étiquette « V2 » sur le bloc d'un
//! plan qui a plusieurs versions, et, quand on la survole, une carte par version posée **au-dessus du bloc** (un
//! aperçu, le nom, une croix pour supprimer). Un clic sur une carte passe le plan à cette version.
//!
//! # Zéro latence
//!
//! - Sans plan à plusieurs versions, rien ne se fait (pas d'étiquette, pas de pile).
//! - Les aperçus viennent de la file des vignettes de fond (`thumbs.rs`, `texture_rangee` pour une version
//!   rangée) : l'interface ne fait que la clé et le cliché, au plus `THUMB_TIME` par image d'écran, et pose un
//!   gabarit gris tant que l'aperçu n'est pas prêt. Rien n'est composé ici.
//! - Seules les cartes **visibles** (six au plus) demandent un aperçu, et seulement tant que la pile est ouverte.
//! - Fermée, la pile ne coûte qu'une comparaison de rectangles par image d'écran ; elle ne demande une image que
//!   pour compter son petit délai de grâce.
//!
//! # Gestes
//!
//! Survol de l'étiquette : la pile s'ouvre. Elle reste tant que le pointeur est sur l'étiquette, sur les cartes
//! ou dans le petit vide entre les deux, et se ferme un quart de seconde après qu'il les a quittés. Un clic sur
//! l'étiquette l'**épingle** (pour un stylet qui ne survole pas) : elle reste jusqu'à un appui ailleurs.
//! Carte : un clic passe à la version (« Passer à la version V1 », une étape d'annulation nommée) ; la croix
//! demande confirmation sur la carte même, puis supprime (« Supprimer la version V1 », annulable). La version
//! affichée n'a pas de croix : on ne supprime jamais la dernière version.

use std::time::{Duration, Instant};

use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke, StrokeKind};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::envois::date_courte;
use crate::pilevers::{self, Etiquette, Pointeur};
use crate::planvers::etiquette;
use crate::theme;

/// Une étiquette posée sur un bloc de la bande à cette image d'écran.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EtiquetteBloc {
    /// Où la souris la survole.
    pub zone: Rect,
    /// Le plan (son numéro) et sa place dans le film.
    pub plan: u64,
    pub index: usize,
}

/// Ce qu'on a fait sur une carte à cette image d'écran.
enum Geste {
    Passer(u32),
    Demander(u32),
    Annuler,
    Supprimer(u32),
}

impl App {
    /// Peint l'étiquette de version du plan `index` sur son bloc `bloc`, s'il a plusieurs versions ; rend sa zone.
    /// `repere` : le bloc porte la pastille « modifié depuis l'envoi » en haut à droite.
    pub(crate) fn peindre_etiquette_version(&self, painter: &egui::Painter, bloc: Rect, index: usize, repere: bool) -> Option<EtiquetteBloc> {
        let plan = self.doc.panels.get(index)?.id;
        if !pilevers::etiquette_visible(self.doc.nombre_de_versions(plan)) {
            return None;
        }
        let ouverte = self.pile_versions.ouverte() == Some(plan);
        let ink = if ouverte { theme::sur_accent() } else { theme::texte_fort() };
        let nom = painter.layout_no_wrap(etiquette(self.doc.numero_de_version(plan)), FontId::proportional(10.5), ink);
        let place = pilevers::placer_etiquette(bloc, nom.size().x, repere);
        match place {
            Etiquette::Texte(r) => {
                let fond = if ouverte { theme::accent() } else { theme::fenetre().gamma_multiply(0.94) };
                painter.rect_filled(r, 7.0, fond);
                painter.rect_stroke(r, 7.0, Stroke::new(1.0_f32, if ouverte { theme::contour() } else { theme::bord() }), StrokeKind::Inside);
                painter.galley(r.center() - nom.size() / 2.0, nom, ink);
            }
            Etiquette::Point(r) => {
                let fond = if ouverte { theme::accent() } else { theme::texte_fort() };
                painter.circle(r.center(), 4.5, fond, Stroke::new(1.5_f32, theme::fenetre()));
            }
            Etiquette::Aucune => {}
        }
        place.zone().map(|zone| EtiquetteBloc { zone, plan, index })
    }

    /// Un clic sur l'étiquette du plan `plan` : la pile s'ouvre et reste (elle se ferme à un appui ailleurs).
    pub(crate) fn pile_epingler(&mut self, plan: u64) {
        if self.pile_versions.ouverte() != Some(plan) {
            self.pile_versions.ouvrir(plan);
        }
        self.pile_versions.epinglee = true;
    }

    /// La pile des versions : son état (survol), puis, si elle est ouverte, ses cartes. `occupe` : un geste de la
    /// bande est en cours ; `clip` : la partie visible de la bande (une étiquette hors d'elle ne se survole pas).
    pub(crate) fn pile_versions_ui(&mut self, ui: &mut egui::Ui, etiquettes: &[EtiquetteBloc], occupe: bool, clip: Rect) {
        let ctx = ui.ctx().clone();
        if !etiquettes.is_empty() || !self.pile_versions.etiquettes.is_empty() {
            self.pile_versions.etiquettes = etiquettes.iter().map(|e| (e.zone, e.plan)).collect();
        }
        // Rien à faire : pas d'étiquette et pas de pile (le cas de presque tous les projets).
        if etiquettes.is_empty() && self.pile_versions.ouverte().is_none() {
            return;
        }
        // La pile d'un plan qui n'est plus montré (la bande a défilé, la version a été supprimée) se ferme.
        if let Some(p) = self.pile_versions.ouverte() {
            if !etiquettes.iter().any(|e| e.plan == p) || self.doc.nombre_de_versions(p) < 2 {
                self.pile_versions.fermer();
            }
        }
        let (now, hover, appui) = ui.input(|i| (i.time, i.pointer.hover_pos(), i.pointer.any_pressed()));
        // (Un menu ouvert — le clic droit sur le plan — ferme la pile.)
        let popup = ctx.memory(|m| m.any_popup_open());
        let pointeur = match hover {
            Some(p) if clip.contains(p) && etiquettes.iter().any(|e| e.zone.contains(p)) => {
                Pointeur::Etiquette(etiquettes.iter().find(|e| e.zone.contains(p)).map_or(0, |e| e.plan))
            }
            Some(p) if self.pile_versions.zone.is_some_and(|z| z.contains(p)) => Pointeur::Pile,
            _ => Pointeur::Ailleurs,
        };
        if matches!(pointeur, Pointeur::Etiquette(_)) {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        self.pile_versions.avancer(pointeur, appui, occupe || popup, now);
        if let Some(reste) = self.pile_versions.reveil(now) {
            ctx.request_repaint_after(Duration::from_secs_f64(reste + 0.02));
        }
        let Some(plan) = self.pile_versions.ouverte() else { return };
        let Some(e) = etiquettes.iter().find(|e| e.plan == plan).copied() else { return };
        let cartes = pilevers::cartes(&self.doc, plan);
        if cartes.len() < 2 {
            self.pile_versions.fermer();
            return;
        }

        let ecran = ctx.screen_rect();
        let visibles = pilevers::cartes_visibles(cartes.len(), e.zone, ecran);
        self.pile_versions.premiere = self.pile_versions.premiere.min(cartes.len() - visibles);
        // La molette fait défiler quand il y a plus de cartes que de place.
        if cartes.len() > visibles && matches!(pointeur, Pointeur::Pile) {
            let dy = ui.input(|i| i.raw_scroll_delta.y);
            if dy != 0.0 {
                self.pile_versions.defiler(if dy > 0.0 { -1 } else { 1 }, cartes.len(), visibles);
            }
        }
        let premiere = self.pile_versions.premiere;
        let dispo = pilevers::disposer(e.zone, visibles, ecran);
        self.pile_versions.zone = Some(dispo.zone);

        // Les aperçus : la file de fond, jamais composés ici. Une carte sans aperçu prêt montre un gabarit.
        let deadline = Instant::now() + crate::thumbs::THUMB_TIME;
        let largeur = crate::thumbs::width_for(80.0);
        let montrees = &cartes[premiere..premiere + visibles];
        let mut textures = Vec::with_capacity(montrees.len());
        for c in montrees {
            let t = if c.actuelle {
                self.thumbs.texture(&ctx, &self.doc, e.index, largeur, deadline)
            } else {
                match self.doc.versions_du_plan(plan).and_then(|v| v.rangee(c.numero)) {
                    Some(r) => self.thumbs.texture_rangee(&ctx, &self.doc, &r.panel, largeur, deadline),
                    None => None,
                }
            };
            textures.push(t);
        }
        let aspect = self.doc.frame[0] as f32 / self.doc.frame[1].max(1) as f32;
        let confirme = self.pile_versions.confirme;
        let affichee = etiquette(cartes.iter().find(|c| c.actuelle).map_or(1, |c| c.numero));
        let plus_haut = premiere > 0;
        let plus_bas = premiere + visibles < cartes.len();

        let mut geste: Option<Geste> = None;
        egui::Area::new(egui::Id::new("pile_versions")).order(egui::Order::Foreground).fixed_pos(dispo.cadre.min).constrain(false).show(&ctx, |ui| {
            ui.allocate_exact_size(dispo.cadre.size(), Sense::hover());
            let p = ui.painter().clone();
            p.add(ui.visuals().popup_shadow.as_shape(dispo.cadre, 6.0));
            p.rect_filled(dispo.cadre, 6.0, theme::fenetre());
            p.rect_stroke(dispo.cadre, 6.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
            if plus_haut {
                p.text(pos2(dispo.cadre.center().x, dispo.cadre.top() + 1.0), Align2::CENTER_TOP, ph::CARET_UP, FontId::proportional(9.0), theme::texte_doux());
            }
            if plus_bas {
                p.text(pos2(dispo.cadre.center().x, dispo.cadre.bottom() - 1.0), Align2::CENTER_BOTTOM, ph::CARET_DOWN, FontId::proportional(9.0), theme::texte_doux());
            }
            for (k, c) in montrees.iter().enumerate() {
                let rect = dispo.cartes[k];
                let nom = etiquette(c.numero);
                let en_confirmation = confirme == Some(c.numero) && !c.actuelle;
                let carte = ui.interact(rect, egui::Id::new(("carte_version", plan, c.numero)), Sense::click());
                let survol = carte.hovered() && !en_confirmation;
                let fond = if c.actuelle {
                    theme::accent()
                } else if en_confirmation {
                    theme::erreur().gamma_multiply(0.3)
                } else if survol {
                    theme::controle_survol()
                } else {
                    theme::controle()
                };
                p.rect_filled(rect, 5.0, fond);
                if c.actuelle {
                    p.rect_stroke(rect, 5.0, Stroke::new(1.5_f32, theme::contour()), StrokeKind::Inside);
                } else if en_confirmation {
                    p.rect_stroke(rect, 5.0, Stroke::new(1.0_f32, theme::erreur()), StrokeKind::Inside);
                }
                let (ink, doux) = if c.actuelle { (theme::sur_accent(), theme::sur_accent()) } else { (theme::texte_fort(), theme::texte_doux()) };
                // L'aperçu, à gauche.
                let h = CARTE_APERCU;
                let tr = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2((h * aspect).clamp(30.0, 80.0), h));
                match textures[k] {
                    Some(tex) => {
                        p.image(tex, tr, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                    }
                    None => {
                        p.rect_filled(tr, 2.0, theme::champ());
                        p.text(tr.center(), Align2::CENTER_CENTER, ph::IMAGE, FontId::proportional(14.0), theme::texte_doux());
                    }
                }
                p.rect_stroke(tr, 2.0, Stroke::new(1.0_f32, theme::bord()), StrokeKind::Inside);
                let x = tr.right() + 7.0;
                if en_confirmation {
                    p.text(pos2(x, rect.top() + 5.0), Align2::LEFT_TOP, format!("Supprimer {nom} ?"), FontId::proportional(11.0), theme::texte_fort());
                    let bas = rect.bottom() - 4.0;
                    let oui = Rect::from_min_max(pos2(x, bas - 18.0), pos2(x + 62.0, bas));
                    let non = Rect::from_min_max(pos2(oui.right() + 4.0, bas - 18.0), pos2(oui.right() + 4.0 + 50.0, bas));
                    if ui.put(oui, egui::Button::new(RichText::new("Supprimer").size(11.0).color(theme::erreur()))).clicked() {
                        geste = Some(Geste::Supprimer(c.numero));
                    }
                    if ui.put(non, egui::Button::new(RichText::new("Garder").size(11.0))).clicked() {
                        geste = Some(Geste::Annuler);
                    }
                    continue;
                }
                // Le nom, et dessous : « affichée » ou la date de la version.
                p.text(pos2(x, rect.top() + 7.0), Align2::LEFT_TOP, &nom, FontId::proportional(14.0), ink);
                let sous = if c.actuelle {
                    "affichée".to_string()
                } else if c.date > 0 {
                    date_courte(c.date)
                } else {
                    String::new()
                };
                p.text(pos2(x, rect.top() + 26.0), Align2::LEFT_TOP, sous, FontId::proportional(10.0), doux);
                // La croix : supprimer la version (jamais celle qui est affichée).
                let mut sur_croix = false;
                if !c.actuelle {
                    let zone = Rect::from_min_size(pos2(rect.right() - 21.0, rect.top() + 3.0), vec2(18.0, 18.0));
                    let croix = ui.interact(zone, egui::Id::new(("carte_version_croix", plan, c.numero)), Sense::click());
                    sur_croix = croix.hovered();
                    let couleur = if croix.hovered() { theme::erreur() } else { doux };
                    p.text(zone.center(), Align2::CENTER_CENTER, ph::X_CIRCLE, FontId::proportional(14.0), couleur);
                    let clique = croix.clicked();
                    bulles::bulle(croix, "Supprimer la version", "Elle sort du plan, après confirmation (Ctrl+Z la ramène).", "");
                    if clique {
                        geste = Some(Geste::Demander(c.numero));
                    }
                }
                let clique = carte.clicked() && !c.actuelle && !sur_croix;
                if survol && !c.actuelle && !sur_croix {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if c.actuelle {
                    bulles::bulle(carte, &format!("{nom}, version affichée"), "C'est celle que montrent le film, l'export et les envois.", "");
                } else {
                    bulles::bulle(carte, &format!("Passer à la {nom}"), &format!("Elle devient la version affichée ; la {affichee} est gardée."), "");
                }
                if clique {
                    geste = Some(Geste::Passer(c.numero));
                }
            }
        });
        match geste {
            Some(Geste::Passer(n)) => {
                self.pile_versions.confirme = None;
                self.passer_a_la_version(e.index, n);
            }
            Some(Geste::Demander(n)) => self.pile_versions.confirme = Some(n),
            Some(Geste::Annuler) => self.pile_versions.confirme = None,
            Some(Geste::Supprimer(n)) => {
                self.pile_versions.confirme = None;
                self.supprimer_la_version(e.index, n);
            }
            None => {}
        }
    }
}

/// Hauteur de l'aperçu d'une carte.
const CARTE_APERCU: f32 = pilevers::CARTE_H - 8.0;
