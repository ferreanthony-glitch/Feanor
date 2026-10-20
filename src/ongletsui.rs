//! La bande des onglets de projets (`onglets.rs`), sous la barre de menus : un onglet par projet ouvert, avec
//! son nom, une pastille « • » s'il n'est pas enregistré et une croix ; le clic du milieu ferme, le clic droit
//! ouvre le menu de l'onglet, glisser un onglet le déplace, « + » en ouvre un neuf (l'accueil).
//!
//! La bande ne coûte rien au repos : elle ne demande aucune image de plus, et ne calcule que le nom et l'état de
//! chaque onglet (quelques chaînes).

use eframe::egui::{self, pos2, vec2, Align2, CursorIcon, FontId, Rect, Sense, Stroke};
use egui_phosphor::regular as ph;

use crate::app::App;
use crate::bulles;
use crate::onglets::InfosOnglet;
use crate::theme;

/// Hauteur de la bande.
const HAUTEUR: f32 = 26.0;
/// Largeur d'un onglet : entre ces bornes (au-delà, la bande défile).
const LARGEUR_MIN: f32 = 96.0;
const LARGEUR_MAX: f32 = 220.0;
/// Côté du bouton « fermer » d'un onglet.
const CROIX: f32 = 16.0;

/// Ce que la souris a demandé à la bande pendant cette image.
enum Demande {
    Activer(usize),
    Fermer(usize),
    FermerLesAutres(u64),
    Explorateur(usize),
    Nouveau,
    Deplacer(usize, usize),
}

impl App {
    /// Le texte de la bulle d'un onglet : où est le projet, et ce qu'il y a à savoir.
    pub(crate) fn bulle_onglet(&self, info: &InfosOnglet) -> String {
        let lieu = match &info.chemin {
            Some(p) => p.display().to_string(),
            None if info.vierge => "L'accueil : Nouveau, Ouvrir ou un projet récent.".to_string(),
            None => "Pas encore enregistré sur le disque.".to_string(),
        };
        let etat = if info.ecriture {
            " Enregistrement en cours."
        } else if info.modifie {
            " Modifié depuis le dernier enregistrement."
        } else {
            ""
        };
        format!("{lieu}{etat}")
    }

    /// La bande d'onglets.
    pub(crate) fn onglets_bar(&mut self, ui: &mut egui::Ui) {
        let infos = self.infos_onglets();
        let pal = theme::palette();
        // Une boîte de dialogue ou un travail modal tient la main : la bande attend.
        let libre = !self.modal_open();
        let mut demande: Option<Demande> = None;
        let plus = 30.0;
        let place = (ui.available_width() - plus - 8.0).max(LARGEUR_MIN);
        let n = infos.len().max(1) as f32;
        let largeur = ((place - 2.0 * n) / n).clamp(LARGEUR_MIN, LARGEUR_MAX);
        let raccourci_fermer = self.raccourci("fichier.fermer");
        let raccourci_nouveau = self.raccourci("onglet.nouveau");
        // L'onglet affiché vient d'en changer : la bande le ramène en vue (elle défile quand les onglets débordent).
        let dernier_id = egui::Id::new("onglets_dernier_affiche");
        let dernier: Option<u64> = ui.data(|d| d.get_temp(dernier_id));
        let affiche = infos.iter().find(|i| i.actif).map(|i| i.id);
        if dernier != affiche {
            ui.data_mut(|d| match affiche {
                Some(id) => d.insert_temp(dernier_id, id),
                None => d.remove::<u64>(dernier_id),
            });
        }
        ui.add_enabled_ui(libre, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let bande = Rect::from_min_size(ui.cursor().min, vec2(place, HAUTEUR));
                egui::ScrollArea::horizontal()
                    .id_salt("onglets_defilement")
                    .max_width(place)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .auto_shrink([true, true])
                    .show(ui, |ui| {
                        // La molette fait défiler la bande quand elle déborde.
                        if ui.ctx().pointer_hover_pos().is_some_and(|p| bande.contains(p)) {
                            let dy = ui.input(|i| i.smooth_scroll_delta.y);
                            if dy != 0.0 {
                                ui.scroll_with_delta(vec2(dy, 0.0));
                            }
                        }
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            let mut rects: Vec<Rect> = Vec::with_capacity(infos.len());
                            #[cfg(test)]
                            let mut places: Vec<(u64, Rect, Rect)> = Vec::new();
                            let mut tire: Option<usize> = None;
                            for (i, info) in infos.iter().enumerate() {
                                let (rect, resp) = ui.allocate_exact_size(vec2(largeur, HAUTEUR), Sense::click_and_drag());
                                rects.push(rect);
                                if info.actif && dernier != Some(info.id) {
                                    ui.scroll_to_rect(rect, None);
                                }
                                let survol = resp.hovered();
                                let croix_rect = Rect::from_center_size(pos2(rect.right() - CROIX * 0.5 - 4.0, rect.center().y), vec2(CROIX, CROIX));
                                #[cfg(test)]
                                places.push((info.id, rect, croix_rect));
                                let croix = ui.interact(croix_rect, resp.id.with("croix"), Sense::click());
                                // Fond : l'onglet affiché est du ton du panneau ; les autres, un cran plus bas.
                                let fond = if info.actif {
                                    pal.panneau
                                } else if survol {
                                    pal.eleve
                                } else {
                                    pal.entete
                                };
                                let painter = ui.painter();
                                painter.rect_filled(rect, egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, fond);
                                if info.actif {
                                    painter.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), 2.0)), 0.0, pal.accent);
                                } else {
                                    painter.line_segment([rect.left_top() + vec2(0.0, 5.0), rect.left_bottom() - vec2(0.0, 5.0)], Stroke::new(1.0_f32, pal.bord));
                                }
                                // La pastille d'un projet pas enregistré, puis le nom.
                                let encre = if info.actif { pal.texte_fort } else { pal.texte_doux };
                                let mut x = rect.left() + 9.0;
                                if info.a_perdre() {
                                    painter.circle_filled(pos2(x + 3.0, rect.center().y), 3.0, pal.avertissement);
                                    x += 12.0;
                                }
                                let texte_max = (croix_rect.left() - 4.0 - x).max(10.0);
                                let galerie = painter.layout(info.nom.clone(), FontId::proportional(13.0), encre, texte_max);
                                let y = rect.center().y - galerie.size().y * 0.5;
                                painter.with_clip_rect(Rect::from_min_max(pos2(x, rect.top()), pos2(croix_rect.left() - 2.0, rect.bottom()))).galley(pos2(x, y), galerie, encre);
                                // La croix : visible sur l'onglet affiché, ou quand la souris est sur l'onglet.
                                if info.actif || survol || croix.hovered() {
                                    if croix.hovered() {
                                        painter.rect_filled(croix_rect, 3.0, pal.controle_survol);
                                    }
                                    painter.text(croix_rect.center(), Align2::CENTER_CENTER, ph::X, FontId::proportional(12.0), if croix.hovered() { pal.texte_fort } else { encre });
                                }
                                let bulle = self.bulle_onglet(info);
                                let resp = bulles::bulle(resp, &info.nom, &bulle, "");
                                let croix = bulles::bulle(croix, "Fermer", "Ferme ce projet.", &raccourci_fermer);
                                if croix.clicked() {
                                    demande = Some(Demande::Fermer(i));
                                } else if resp.middle_clicked() {
                                    demande = Some(Demande::Fermer(i));
                                } else if resp.clicked() && !info.actif {
                                    demande = Some(Demande::Activer(i));
                                }
                                if resp.dragged() {
                                    tire = Some(i);
                                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                                }
                                if resp.drag_stopped() {
                                    if let Some(p) = ui.ctx().pointer_latest_pos() {
                                        let vers = rects.iter().position(|r| p.x < r.center().x).unwrap_or(rects.len().saturating_sub(1));
                                        if vers != i {
                                            demande = Some(Demande::Deplacer(i, vers));
                                        }
                                    }
                                }
                                resp.context_menu(|ui| {
                                    if ui.button("Fermer").clicked() {
                                        demande = Some(Demande::Fermer(i));
                                        ui.close_menu();
                                    }
                                    if ui.add_enabled(infos.len() > 1, egui::Button::new("Fermer les autres")).clicked() {
                                        demande = Some(Demande::FermerLesAutres(info.id));
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.add_enabled(info.chemin.is_some(), egui::Button::new("Afficher dans l'Explorateur")).clicked() {
                                        demande = Some(Demande::Explorateur(i));
                                        ui.close_menu();
                                    }
                                });
                            }
                            // (Essais : où sont les onglets et leurs croix, pour cliquer dessus.)
                            #[cfg(test)]
                            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("onglets_places"), places));
                            // Le repère de dépose pendant qu'on glisse un onglet.
                            if let (Some(_), Some(p)) = (tire, ui.ctx().pointer_latest_pos()) {
                                if let Some(r) = rects.iter().find(|r| p.x < r.center().x).or(rects.last()) {
                                    let x = if p.x < r.center().x { r.left() - 1.0 } else { r.right() + 1.0 };
                                    ui.painter().line_segment([pos2(x, r.top()), pos2(x, r.bottom())], Stroke::new(2.0_f32, pal.accent));
                                }
                            }
                        });
                    });
                let nouveau = bulles::icone(ui, ph::PLUS, "Nouvel onglet", "Un onglet vierge : l'accueil, pour créer ou ouvrir un autre projet.", &raccourci_nouveau);
                if nouveau.clicked() {
                    demande = Some(Demande::Nouveau);
                }
            });
        });
        match demande {
            Some(Demande::Activer(i)) => {
                self.activer_onglet(i);
            }
            Some(Demande::Fermer(i)) => self.fermer_onglet(i),
            Some(Demande::FermerLesAutres(id)) => self.fermer_les_autres(id),
            Some(Demande::Explorateur(i)) => {
                if let Some(p) = infos.get(i).and_then(|info| info.chemin.clone()) {
                    crate::exportui::reveal_in_explorer(&p);
                }
            }
            Some(Demande::Nouveau) => {
                self.nouvel_onglet();
            }
            Some(Demande::Deplacer(de, vers)) => self.deplacer_onglet(de, vers),
            None => {}
        }
    }
}
